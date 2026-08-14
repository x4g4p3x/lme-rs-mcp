//! MCP transport adapter for the protocol-neutral [`LmeAgentApi`](crate::LmeAgentApi).

use rmcp::{
    handler::server::wrapper::Parameters, tool, tool_router, ErrorData as McpError, Json,
};

use crate::api::{AgentApiError, LmeAgentApi};
use crate::dto::{
    AnovaRequest, AnovaSummary, BootSummary, BootstrapRequest, FitIdRequest, FitListSummary,
    FitLmmRequest, FitSummary, ForgetFitResult,
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
    #[tool(description = "Fit a Gaussian linear mixed model (lmer) from a CSV path and formula.")]
    fn lme_fit(
        &self,
        Parameters(request): Parameters<FitLmmRequest>,
    ) -> Result<Json<FitSummary>, McpError> {
        self.api
            .fit_lmm(request)
            .map(Json)
            .map_err(to_mcp_error)
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
        self.api
            .forget_fit(&fit_id)
            .map(Json)
            .map_err(to_mcp_error)
    }

    #[tool(
        description = "Type I/II/III fixed-effects ANOVA for a cached LMM. Applies Satterthwaite or Kenward-Roger dfs automatically when requested."
    )]
    fn lme_anova(
        &self,
        Parameters(request): Parameters<AnovaRequest>,
    ) -> Result<Json<AnovaSummary>, McpError> {
        self.api.anova(request).map(Json).map_err(to_mcp_error)
    }

    #[tool(
        description = "Parametric or residual bootstrap refits (bootMer-style) for a cached Gaussian LMM."
    )]
    fn lme_boot(
        &self,
        Parameters(request): Parameters<BootstrapRequest>,
    ) -> Result<Json<BootSummary>, McpError> {
        self.api
            .bootstrap(request)
            .map(Json)
            .map_err(to_mcp_error)
    }
}
