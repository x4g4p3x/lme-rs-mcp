//! Actual binary/client interoperability, including legacy and modern lifecycles.
use rmcp::{model::*, transport::TokioChildProcess, ServiceExt};
use rmcp::{serve_client_with_lifecycle, ClientLifecycleMode};
use serde_json::{json, Value};
use std::time::Duration;

fn command() -> tokio::process::Command {
    let binary = std::env::var_os("LME_MCP_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_lme-rs-mcp").into());
    let mut command = tokio::process::Command::new(binary);
    command.env(
        "LME_DATA_ROOT",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data"),
    );
    command.env_remove("LME_MCP_DATA_ROOT");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    command
}

fn request(name: &str, arguments: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name.to_owned())
        .with_arguments(arguments.as_object().unwrap().clone())
}

#[tokio::test]
async fn legacy_client_fits_predicts_infers_and_uses_aliases() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let transport = TokioChildProcess::new(command()).unwrap();
        let config = ClientConfig::new(ClientCapabilities::default(), Implementation::new("interop-test","1"))
            .with_protocol_version(ProtocolVersion::V_2025_11_25);
        let client = config.serve(transport).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        for name in ["fit_model","lme_fit","bootstrap","marginal_means","pairwise_comparisons"] {
            let tool = tools.iter().find(|tool| tool.name == name).unwrap();
            assert!(tool.output_schema.is_some());
        }
        let fit = client.call_tool(request("lme_fit",json!({"formula":"Reaction ~ Days + (1 | Subject)","data_path":"sleepstudy.csv"}))).await.unwrap();
        let id = fit.structured_content.unwrap()["fit_id"].as_str().unwrap().to_owned();
        let prediction = client.call_tool(request("predict",json!({"fit_id":id}))).await.unwrap();
        assert_eq!(prediction.structured_content.unwrap()["predictions"].as_array().unwrap().len(),180);
        let inference = client.call_tool(request("confidence_intervals",json!({"fit_id":id}))).await.unwrap();
        assert_eq!(inference.structured_content.unwrap()["intervals"].as_array().unwrap().len(),2);
        let aliases = client.call_tool(request("lme_list_fits",json!({}))).await.unwrap();
        assert_eq!(aliases.structured_content.unwrap()["fits"][0]["fit_id"],id);
        client.call_tool(request("forget_model",json!({"fit_id":id}))).await.unwrap();
        let error = client.call_tool(request("model_summary",json!({"fit_id":id}))).await;
        assert!(error.is_err() || error.unwrap().is_error == Some(true));
        let path_error = client.call_tool(request("fit_model",json!({"model_kind":"lm","formula":"y ~ x","data_path":"../outside.csv"}))).await;
        assert!(path_error.is_err() || path_error.unwrap().is_error == Some(true));
        let lm = client.call_tool(request("fit_model",json!({"model_kind":"lm","formula":"y ~ treatment + x","data_path":"factorial.csv"}))).await.unwrap();
        let lm_id = lm.structured_content.unwrap()["fit_id"].as_str().unwrap().to_owned();
        for name in ["marginal_means","pairwise_comparisons"] {
            let result = client.call_tool(request(name,json!({"fit_id":lm_id,"terms":["treatment"],"at":{"x":0.0}}))).await.unwrap();
            assert_eq!(result.is_error,Some(false));
            assert!(result.structured_content.unwrap()["rows"].as_array().unwrap().len() >= 2);
        }
        let malformed = client.call_tool(request("fit_model",json!({"model_kind":"unknown"}))).await;
        assert!(malformed.is_err() || malformed.unwrap().is_error == Some(true));
        assert!(client.call_tool(request("missing",json!({}))).await.is_err());
        client.cancel().await.unwrap();
    }).await.expect("legacy workflow timed out");
}

#[tokio::test]
async fn modern_client_gets_tasks_only_when_declared() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let client = serve_client_with_lifecycle(ClientConfig::new(ClientCapabilities::builder().enable_tasks().build(),Implementation::new("tasks-test","1")), TokioChildProcess::new(command()).unwrap(), ClientLifecycleMode::Discover { preferred_versions: vec![ProtocolVersion::V_2026_07_28] }).await.unwrap();
        let response = client.call_tool_once(request("fit_model",json!({"model_kind":"lm","formula":"y ~ treatment + x","data_path":"factorial.csv"}))).await.unwrap();
        let CallToolResponse::Task(created) = response else { panic!("expected task"); };
        let final_task = loop {
            let result = client.peer().get_task(GetTaskParams::new(created.task.task_id.clone())).await.unwrap();
            if result.task.status().is_terminal() { break result.task; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        let TaskPayload::Completed { result } = final_task.payload else { panic!("fit task failed: {final_task:?}"); };
        let result: CallToolResult = serde_json::from_value(Value::Object(result)).unwrap();
        let id = result.structured_content.unwrap()["fit_id"].as_str().unwrap().to_owned();
        // Lifecycle calls remain ordinary results even for a task-capable client.
        assert!(matches!(client.call_tool_once(request("model_summary",json!({"fit_id":id}))).await.unwrap(), CallToolResponse::Complete(_)));
        let response = client.call_tool_once(request("marginal_means",json!({"fit_id":id,"terms":["treatment"]}))).await.unwrap();
        assert!(matches!(response,CallToolResponse::Task(_)));
        assert!(client.peer().get_task(GetTaskParams::new("missing")).await.is_err());
        client.cancel().await.unwrap();

        let ordinary = serve_client_with_lifecycle(
            ClientConfig::new(ClientCapabilities::default(), Implementation::new("ordinary-modern-test", "1")),
            TokioChildProcess::new(command()).unwrap(),
            ClientLifecycleMode::Discover { preferred_versions: vec![ProtocolVersion::V_2026_07_28] },
        ).await.unwrap();
        assert!(matches!(ordinary.call_tool_once(request("fit_model",json!({"model_kind":"lm","formula":"y ~ treatment + x","data_path":"factorial.csv"}))).await.unwrap(), CallToolResponse::Complete(_)));
        assert!(ordinary.peer().get_task(GetTaskParams::new("missing")).await.is_err());
        ordinary.cancel().await.unwrap();
    }).await.expect("task workflow timed out");
}
