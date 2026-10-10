//! Task transport lifecycle; numerical work remains in the shared core.
use crate::LmeMcpServer;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::task_manager::{TaskExit, TaskOptions};
use rmcp::{ErrorData as McpError, ServerHandler};

const TASK_TTL_MS: u64 = 900_000;

// A cancelled/expired caller may drop a result after fitting has completed.
// This lease ensures the corresponding fit cannot remain as an orphan.
struct ResponseLease {
    response: Option<CallToolResponse>,
    created_fit: Option<String>,
    api: lme_rs_agent::LmeAgentApi,
}

impl ResponseLease {
    fn deliver(mut self) -> CallToolResponse {
        self.created_fit = None;
        self.response
            .take()
            .expect("response present until delivery")
    }
}

impl Drop for ResponseLease {
    fn drop(&mut self) {
        if let Some(id) = &self.created_fit {
            let _ = self.api.forget_model(id);
        }
    }
}

impl LmeMcpServer {
    fn spawn_call(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> tokio::sync::oneshot::Receiver<Result<ResponseLease, McpError>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = self.clone();
        // The supervisor survives cancellation/TTL while the blocking worker drains.
        tokio::spawn(async move {
            if tx.is_closed() {
                return;
            }
            let fitting = matches!(request.name.as_ref(), "fit_model" | "lme_fit");
            let context = ToolCallContext::new(&server, request, context);
            let response = server.tool_router.call(context).await.map(|response| {
                let created_fit = if fitting {
                    match &response {
                        CallToolResponse::Complete(result) => result
                            .structured_content
                            .as_ref()
                            .and_then(|value| value.get("fit_id"))
                            .and_then(|id| id.as_str())
                            .map(str::to_owned),
                        _ => None,
                    }
                } else {
                    None
                };
                ResponseLease {
                    response: Some(response),
                    created_fit,
                    api: server.api.clone(),
                }
            });
            // If the receiver has gone, dropping the returned lease cleans up the fit.
            let _ = tx.send(response);
        });
        rx
    }
}

fn expensive(name: &str) -> bool {
    matches!(
        name,
        "fit_model"
            | "lme_fit"
            | "anova"
            | "lme_anova"
            | "bootstrap"
            | "lme_boot"
            | "confidence_intervals"
            | "cross_validate"
            | "marginal_means"
            | "pairwise_comparisons"
    )
}

impl ServerHandler for LmeMcpServer {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if self.tool_router.get(&request.name).is_none() {
            return Err(McpError::new(
                ErrorCode::METHOD_NOT_FOUND,
                "unknown tool",
                None,
            ));
        }
        // Never infer modern extension support from a legacy initialize handshake.
        let modern = context
            .protocol_version()
            .is_some_and(|version| !version.has_initialize());
        let supports_tasks = modern
            && context
                .meta
                .client_capabilities()
                .is_some_and(|caps| caps.supports_tasks());
        if expensive(&request.name) && supports_tasks {
            let retention = self
                .retained_tasks
                .clone()
                .try_acquire_owned()
                .map_err(|_| {
                    McpError::invalid_params("task retention capacity reached; retry later", None)
                })?;
            let server = self.clone();
            let task = self.tasks.spawn(TaskOptions::new().with_ttl_ms(Some(TASK_TTL_MS))
                .with_poll_interval_ms(250).with_status_message("Starting statistical operation"), move |task_context| {
                Box::pin(async move {
                    // Bound retained results, including cancelled/failed jobs, across the
                    // complete SDK retention window (creation TTL + terminal TTL).
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(TASK_TTL_MS * 2)).await;
                        drop(retention);
                    });
                    if task_context.is_cancel_requested() { return Err(TaskExit::Cancelled); }
                    let receiver = server.spawn_call(request, context);
                    task_context.set_status_message("Running statistical operation; numerical workers are bounded to two");
                    tokio::select! {
                        biased;
                        _ = task_context.cancelled() => Err(TaskExit::Cancelled),
                        response = receiver => {
                            let response = response.map_err(|_| McpError::internal_error("worker supervisor failed", None))??;
                            match response.deliver() {
                                CallToolResponse::Complete(result) => Ok(result),
                                _ => Err(McpError::internal_error("unexpected nested task", None).into()),
                            }
                        }
                    }
                })
            });
            return Ok(CallToolResponse::Task(CreateTaskResult::new(task)));
        }
        let receiver = self.spawn_call(request, context.clone());
        tokio::select! {
            biased;
            _ = context.ct.cancelled() => Err(McpError::internal_error("request cancelled", None)),
            response = receiver => Ok(response.map_err(|_| McpError::internal_error("worker supervisor failed", None))??.deliver()),
        }
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tool_router.list_all()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, McpError> {
        Ok(GetTaskResult::new(self.tasks.get_task(&request.task_id)?))
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.tasks
            .update_task(&request.task_id, request.input_responses)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.tasks.cancel_task(&request.task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::ServiceExt;
    use serde_json::json;
    use std::time::Duration;

    fn fit(api: &lme_rs_agent::LmeAgentApi) -> lme_rs_agent::FitSummary {
        api.fit_lmm(lme_rs_agent::FitLmmRequest {
            formula: "Reaction ~ Days + (1 | Subject)".into(),
            data_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data/sleepstudy.csv")
                .display()
                .to_string(),
            reml: true,
        })
        .unwrap()
    }

    #[test]
    fn abandoned_fit_response_is_removed_but_delivered_fit_is_retained() {
        let api = lme_rs_agent::LmeAgentApi::new();
        let first = fit(&api);
        drop(ResponseLease {
            response: None,
            created_fit: Some(first.fit_id.clone()),
            api: api.clone(),
        });
        assert!(api.model_summary(&first.fit_id).is_err());
        let second = fit(&api);
        ResponseLease {
            response: Some(CallToolResult::default().into()),
            created_fit: Some(second.fit_id.clone()),
            api: api.clone(),
        }
        .deliver();
        assert!(api.model_summary(&second.fit_id).is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_is_observable_while_workers_drain_and_capacity_is_bounded() {
        tokio::time::timeout(Duration::from_secs(60), async {
            let server = LmeMcpServer::new();
            let fitted = fit(server.api());
            let (server_io, client_io) = tokio::io::duplex(65536);
            let serving = server.clone();
            let service = tokio::spawn(async move {
                serving
                    .serve(server_io)
                    .await
                    .unwrap()
                    .waiting()
                    .await
                    .unwrap();
            });
            let config = ClientConfig::new(
                ClientCapabilities::builder().enable_tasks().build(),
                Implementation::new("cancel-test", "1"),
            );
            let client = rmcp::serve_client_with_lifecycle(
                config,
                client_io,
                rmcp::ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .unwrap();
            let response = client
                .call_tool_once(
                    CallToolRequestParams::new("bootstrap").with_arguments(
                        json!({"fit_id":fitted.fit_id,"nsim":10000,"seed":42,"n_jobs":1})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                )
                .await
                .unwrap();
            let CallToolResponse::Task(task) = response else {
                panic!("expected task");
            };
            while server.jobs.available_permits() == 2 {
                tokio::task::yield_now().await;
            }
            client
                .peer()
                .cancel_task(CancelTaskParams::new(task.task.task_id.clone()))
                .await
                .unwrap();
            loop {
                let result = client
                    .peer()
                    .get_task(GetTaskParams::new(task.task.task_id.clone()))
                    .await
                    .unwrap();
                if result.task.status().is_terminal() {
                    assert_eq!(result.task.status(), TaskStatus::Cancelled);
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(client.list_all_tools().await.unwrap().len(), 19);
            // A full retention budget rejects creation without starting new work.
            let retained = server
                .retained_tasks
                .clone()
                .try_acquire_many_owned(63)
                .unwrap();
            assert!(client
                .call_tool_once(
                    CallToolRequestParams::new("fit_model").with_arguments(
                        json!({
                            "model_kind":"lm","formula":"Reaction ~ Days","data_path":"missing.csv"
                        })
                        .as_object()
                        .unwrap()
                        .clone()
                    )
                )
                .await
                .is_err());
            drop(retained);
            while server.jobs.available_permits() != 2 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            client.cancel().await.unwrap();
            server.shutdown();
            service.abort();
        })
        .await
        .expect("cancellation test timed out");
    }
}
