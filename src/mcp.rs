//! MCP transport adapter for the protocol-neutral [`LmeAgentApi`](crate::LmeAgentApi).

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router, ErrorData as McpError, Json};

use lme_rs_agent::{
    AgentApiError, AnovaRequest, AnovaSummary, BootSummary, BootstrapRequest, CompareModelsRequest,
    CompareModelsSummary, ConfidenceIntervalsRequest, ConfidenceIntervalsSummary,
    CrossValidateRequest, CrossValidationSummary, DiagnosticsSummary, FitIdRequest, FitListSummary,
    FitLmmRequest, FitModelRequest, FitSummary, ForgetFitResult, LmeAgentApi,
    ModelBootstrapRequest, PredictRequest, PredictionSummary,
};

#[derive(Clone)]
pub struct LmeMcpServer {
    pub(crate) api: LmeAgentApi,
    pub(crate) tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
    pub(crate) jobs: std::sync::Arc<tokio::sync::Semaphore>,
    pub(crate) retained_tasks: std::sync::Arc<tokio::sync::Semaphore>,
    pub(crate) tasks: rmcp::task_manager::TaskManager,
}

impl LmeMcpServer {
    async fn compute<T: Send + 'static>(
        &self,
        operation: impl FnOnce(LmeAgentApi) -> Result<T, AgentApiError> + Send + 'static,
    ) -> Result<Json<T>, McpError> {
        let permit = self.jobs.clone().try_acquire_owned().map_err(|_| {
            McpError::invalid_params(
                "server busy: two statistical jobs are already running; retry after completion",
                None,
            )
        })?;
        let api = self.api.clone();
        tokio::task::spawn_blocking(move || {
            // Cancellation must not free capacity while numerical work still runs.
            let _permit = permit;
            operation(api).map(Json).map_err(to_mcp_error)
        })
        .await
        .map_err(|_| McpError::internal_error("statistical worker failed", None))?
    }

    pub fn shutdown(&self) {
        self.tasks.shutdown();
    }
    pub fn new() -> Self {
        Self {
            api: LmeAgentApi::new(),
            tool_router: Self::tool_router(),
            jobs: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
            retained_tasks: std::sync::Arc::new(tokio::sync::Semaphore::new(64)),
            tasks: rmcp::task_manager::TaskManager::new(),
        }
    }

    /// Access the protocol-neutral API used by this adapter.
    pub fn api(&self) -> &LmeAgentApi {
        &self.api
    }
}

impl Default for LmeMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

fn to_mcp_error(error: AgentApiError) -> McpError {
    match error {
        AgentApiError::InvalidInput(message) | AgentApiError::NotFound(message) => {
            McpError::invalid_params(message, None)
        }
        AgentApiError::Computation(message) => McpError::internal_error(message, None),
    }
}

#[tool_router]
impl LmeMcpServer {
    #[tool(
        description = "Estimated marginal means for categorical fixed-effect cells, with explicit numeric references, nuisance weighting, and model-aware degrees of freedom. LM/LMM only; GLMM/NLMM marginal grids are unsupported."
    )]
    async fn marginal_means(
        &self,
        Parameters(request): Parameters<lme_rs_agent::MarginalMeansRequest>,
    ) -> Result<Json<lme_rs_agent::MarginalMeansSummary>, McpError> {
        self.compute(move |api| api.marginal_means(request)).await
    }

    #[tool(
        description = "Pairwise marginal-mean comparisons, with Holm, Bonferroni, Tukey, or no adjustment applied separately within conditioning strata. LM/LMM only; GLMM/NLMM marginal grids are unsupported."
    )]
    async fn pairwise_comparisons(
        &self,
        Parameters(request): Parameters<lme_rs_agent::PairwiseComparisonsRequest>,
    ) -> Result<Json<lme_rs_agent::PairwiseComparisonsSummary>, McpError> {
        self.compute(move |api| api.pairwise_comparisons(request))
            .await
    }
    #[tool(
        description = "Fit an LM, LMM, GLMM, or built-in formula-based NLMM from a CSV path and formula."
    )]
    async fn fit_model(
        &self,
        Parameters(request): Parameters<FitModelRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.compute(move |api| api.fit_model(request)).await
    }

    #[tool(description = "List cached models and essential model metadata.")]
    fn list_models(&self) -> Json<FitListSummary> {
        Json(self.api.list_models())
    }

    #[tool(description = "Return a structured summary for a cached model.")]
    fn model_summary(
        &self,
        Parameters(FitIdRequest { fit_id }): Parameters<FitIdRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.api
            .model_summary(&fit_id)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(description = "Drop a cached model from the current server session.")]
    fn forget_model(
        &self,
        Parameters(FitIdRequest { fit_id }): Parameters<FitIdRequest>,
    ) -> Result<Json<ForgetFitResult>, McpError> {
        self.api
            .forget_model(&fit_id)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(
        description = "Type I/II/III fixed-effects ANOVA for a cached LMM using Satterthwaite or Kenward-Roger denominator degrees of freedom."
    )]
    async fn anova(
        &self,
        Parameters(request): Parameters<AnovaRequest>,
    ) -> Result<Json<AnovaSummary>, McpError> {
        self.compute(move |api| api.anova(request)).await
    }

    #[tool(
        description = "Model-aware bootstrap confidence intervals. Supports LMM parametric/residual bootstrap and GLMM parametric bootstrap."
    )]
    async fn bootstrap(
        &self,
        Parameters(request): Parameters<ModelBootstrapRequest>,
    ) -> Result<Json<BootSummary>, McpError> {
        self.compute(move |api| api.bootstrap_model(request)).await
    }

    #[tool(
        description = "Likelihood-ratio comparison of two cached nested mixed models fit to the same dataset."
    )]
    async fn compare_models(
        &self,
        Parameters(request): Parameters<CompareModelsRequest>,
    ) -> Result<Json<CompareModelsSummary>, McpError> {
        self.compute(move |api| api.compare_models(request)).await
    }

    #[tool(
        description = "Wald or profile-likelihood confidence intervals for cached model fixed effects."
    )]
    async fn confidence_intervals(
        &self,
        Parameters(request): Parameters<ConfidenceIntervalsRequest>,
    ) -> Result<Json<ConfidenceIntervalsSummary>, McpError> {
        self.compute(move |api| api.confidence_intervals(request))
            .await
    }

    #[tool(
        description = "Population-level or conditional predictions on the link or response scale using original or new CSV data."
    )]
    async fn predict(
        &self,
        Parameters(request): Parameters<PredictRequest>,
    ) -> Result<Json<PredictionSummary>, McpError> {
        self.compute(move |api| api.predict(request)).await
    }

    #[tool(description = "Group-preserving cross-validation for cached LMM or GLMM models.")]
    async fn cross_validate(
        &self,
        Parameters(request): Parameters<CrossValidateRequest>,
    ) -> Result<Json<CrossValidationSummary>, McpError> {
        self.compute(move |api| api.cross_validate(request)).await
    }

    #[tool(
        description = "Return convergence, residual, coefficient-finiteness, and availability diagnostics for a cached model."
    )]
    fn diagnostics(
        &self,
        Parameters(FitIdRequest { fit_id }): Parameters<FitIdRequest>,
    ) -> Result<Json<DiagnosticsSummary>, McpError> {
        self.api
            .diagnostics(&fit_id)
            .map(Json)
            .map_err(to_mcp_error)
    }

    // Compatibility surface retained for existing clients.

    #[tool(description = "Fit a Gaussian linear mixed model (lmer) from a CSV path and formula.")]
    async fn lme_fit(
        &self,
        Parameters(request): Parameters<FitLmmRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.compute(move |api| api.fit_lmm(request)).await
    }

    #[tool(description = "List fit_ids currently stored in the MCP session.")]
    fn lme_list_fits(&self) -> Json<FitListSummary> {
        Json(self.api.list_fits())
    }

    #[tool(description = "Return a structured summary for a cached fit.")]
    fn lme_fit_summary(
        &self,
        Parameters(FitIdRequest { fit_id }): Parameters<FitIdRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.api
            .fit_summary(&fit_id)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(description = "Drop a cached fit from the MCP session.")]
    fn lme_forget_fit(
        &self,
        Parameters(FitIdRequest { fit_id }): Parameters<FitIdRequest>,
    ) -> Result<Json<ForgetFitResult>, McpError> {
        self.api.forget_fit(&fit_id).map(Json).map_err(to_mcp_error)
    }

    #[tool(description = "Compatibility Type I/II/III fixed-effects ANOVA for a cached LMM.")]
    async fn lme_anova(
        &self,
        Parameters(request): Parameters<AnovaRequest>,
    ) -> Result<Json<AnovaSummary>, McpError> {
        self.compute(move |api| api.anova(request)).await
    }

    #[tool(
        description = "Compatibility parametric or residual bootstrap refits for a cached Gaussian LMM."
    )]
    async fn lme_boot(
        &self,
        Parameters(request): Parameters<BootstrapRequest>,
    ) -> Result<Json<BootSummary>, McpError> {
        self.compute(move |api| api.bootstrap(request)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_waiter_keeps_worker_capacity_until_computation_finishes() {
        let server = LmeMcpServer::new();
        let started = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker_server = server.clone();
        let worker_started = started.clone();
        let worker_release = release.clone();
        let waiter = tokio::spawn(async move {
            worker_server
                .compute(move |_| {
                    worker_started.wait();
                    worker_release.wait();
                    Ok(())
                })
                .await
        });
        tokio::task::spawn_blocking(move || started.wait())
            .await
            .unwrap();
        waiter.abort();
        assert_eq!(server.jobs.available_permits(), 1);
        let occupied = server.jobs.clone().try_acquire_owned().unwrap();
        assert!(server.compute(|_| Ok(())).await.is_err());
        drop(occupied);
        tokio::task::spawn_blocking(move || release.wait())
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while server.jobs.available_permits() != 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
