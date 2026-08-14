//! MCP transport adapter for the protocol-neutral [`LmeAgentApi`](crate::LmeAgentApi).

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router, ErrorData as McpError, Json};

use crate::api::{AgentApiError, LmeAgentApi};
use crate::dto::{
    AnovaRequest, AnovaSummary, BootSummary, BootstrapRequest, CompareModelsRequest,
    CompareModelsSummary, ConfidenceIntervalsRequest, ConfidenceIntervalsSummary,
    CrossValidateRequest, CrossValidationSummary, DiagnosticsSummary, FitIdRequest, FitListSummary,
    FitLmmRequest, FitModelRequest, FitSummary, ForgetFitResult, ModelBootstrapRequest,
    PredictRequest, PredictionSummary,
};

#[derive(Clone)]
pub struct LmeMcpServer {
    api: LmeAgentApi,
}

impl LmeMcpServer {
    pub fn new() -> Self {
        Self {
            api: LmeAgentApi::new(),
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

#[tool_router(server_handler)]
impl LmeMcpServer {
    #[tool(
        description = "Fit an LM, LMM, GLMM, or built-in formula-based NLMM from a CSV path and formula."
    )]
    fn fit_model(
        &self,
        Parameters(request): Parameters<FitModelRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.api.fit_model(request).map(Json).map_err(to_mcp_error)
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
    fn anova(
        &self,
        Parameters(request): Parameters<AnovaRequest>,
    ) -> Result<Json<AnovaSummary>, McpError> {
        self.api.anova(request).map(Json).map_err(to_mcp_error)
    }

    #[tool(
        description = "Model-aware bootstrap confidence intervals. Supports LMM parametric/residual bootstrap and GLMM parametric bootstrap."
    )]
    fn bootstrap(
        &self,
        Parameters(request): Parameters<ModelBootstrapRequest>,
    ) -> Result<Json<BootSummary>, McpError> {
        self.api
            .bootstrap_model(request)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(
        description = "Likelihood-ratio comparison of two cached nested mixed models fit to the same dataset."
    )]
    fn compare_models(
        &self,
        Parameters(request): Parameters<CompareModelsRequest>,
    ) -> Result<Json<CompareModelsSummary>, McpError> {
        self.api
            .compare_models(request)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(
        description = "Wald or profile-likelihood confidence intervals for cached model fixed effects."
    )]
    fn confidence_intervals(
        &self,
        Parameters(request): Parameters<ConfidenceIntervalsRequest>,
    ) -> Result<Json<ConfidenceIntervalsSummary>, McpError> {
        self.api
            .confidence_intervals(request)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(
        description = "Population-level or conditional predictions on the link or response scale using original or new CSV data."
    )]
    fn predict(
        &self,
        Parameters(request): Parameters<PredictRequest>,
    ) -> Result<Json<PredictionSummary>, McpError> {
        self.api.predict(request).map(Json).map_err(to_mcp_error)
    }

    #[tool(description = "Group-preserving cross-validation for cached LMM or GLMM models.")]
    fn cross_validate(
        &self,
        Parameters(request): Parameters<CrossValidateRequest>,
    ) -> Result<Json<CrossValidationSummary>, McpError> {
        self.api
            .cross_validate(request)
            .map(Json)
            .map_err(to_mcp_error)
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
    fn lme_fit(
        &self,
        Parameters(request): Parameters<FitLmmRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.api.fit_lmm(request).map(Json).map_err(to_mcp_error)
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
    fn lme_anova(
        &self,
        Parameters(request): Parameters<AnovaRequest>,
    ) -> Result<Json<AnovaSummary>, McpError> {
        self.api.anova(request).map(Json).map_err(to_mcp_error)
    }

    #[tool(
        description = "Compatibility parametric or residual bootstrap refits for a cached Gaussian LMM."
    )]
    fn lme_boot(
        &self,
        Parameters(request): Parameters<BootstrapRequest>,
    ) -> Result<Json<BootSummary>, McpError> {
        self.api.bootstrap(request).map(Json).map_err(to_mcp_error)
    }
}
