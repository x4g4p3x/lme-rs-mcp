//! MCP tool handlers for [`lme-rs`](https://github.com/x4g4p3x/lme-rs).

mod data;
mod dto;
mod session;

pub use data::load_csv;
pub use dto::{AnovaRow, AnovaSummary, BootConfintRow, BootSummary, FitListEntry, FitSummary};
pub use session::{CachedFit, FitSession};

use std::sync::Arc;

use anyhow::Result;
use lme_rs::{boot_lmer, lmer, AnovaType, BootLmerMethod, DdfMethod, FixedEffectsAnovaResult};
use rmcp::{
    handler::server::wrapper::Parameters, schemars, tool, tool_router, ErrorData as McpError,
};
use serde_json::json;
use uuid::Uuid;

fn tool_json<T: serde::Serialize>(value: &T) -> Result<String, McpError> {
    serde_json::to_string_pretty(value).map_err(|e| McpError::invalid_params(e.to_string(), None))
}

fn fit_summary_from_cached(fit_id: &str, cached: &CachedFit) -> FitSummary {
    let fit = &cached.fit;
    FitSummary {
        fit_id: fit_id.to_string(),
        formula: cached.formula.clone(),
        data_path: cached.data_path.display().to_string(),
        reml: cached.reml,
        num_obs: fit.num_obs,
        converged: fit.converged.unwrap_or(false),
        fixed_names: fit.fixed_names.clone().unwrap_or_default(),
        coefficients: fit.coefficients.to_vec(),
        beta_se: fit.beta_se.as_ref().map(|s| s.to_vec()),
        sigma2: fit.sigma2,
        aic: fit.aic,
        bic: fit.bic,
        log_likelihood: fit.log_likelihood,
    }
}

fn parse_ddf_method(method: &str) -> Result<DdfMethod, McpError> {
    match method.to_lowercase().as_str() {
        "satterthwaite" | "sat" => Ok(DdfMethod::Satterthwaite),
        "kenward-roger" | "kenward_roger" | "kr" => Ok(DdfMethod::KenwardRoger),
        other => Err(McpError::invalid_params(
            format!("unknown ddf_method '{other}' (use satterthwaite or kenward-roger)"),
            None,
        )),
    }
}

fn parse_anova_type(anova_type: &str) -> Result<AnovaType, McpError> {
    match anova_type.to_uppercase().as_str() {
        "III" | "3" | "TYPE3" | "TYPE III" => Ok(AnovaType::Type3),
        "II" | "2" | "TYPE2" | "TYPE II" => Ok(AnovaType::Type2),
        "I" | "1" | "TYPE1" | "TYPE I" => Ok(AnovaType::Type1),
        other => Err(McpError::invalid_params(
            format!("unknown anova_type '{other}' (use I, II, or III)"),
            None,
        )),
    }
}

fn parse_boot_method(method: &str) -> Result<BootLmerMethod, McpError> {
    match method.to_lowercase().as_str() {
        "parametric" | "param" => Ok(BootLmerMethod::Parametric),
        "residual" | "res" => Ok(BootLmerMethod::Residual),
        other => Err(McpError::invalid_params(
            format!("unknown boot method '{other}' (use parametric or residual)"),
            None,
        )),
    }
}

fn anova_to_summary(fit_id: &str, table: FixedEffectsAnovaResult) -> AnovaSummary {
    AnovaSummary {
        fit_id: fit_id.to_string(),
        anova_type: format!("{:?}", table.anova_type),
        method: format!("{:?}", table.method),
        rows: table
            .terms
            .iter()
            .enumerate()
            .map(|(i, term)| AnovaRow {
                term: term.clone(),
                num_df: table.num_df[i],
                den_df: table.den_df[i],
                f_value: table.f_value[i],
                p_value: table.p_value[i],
            })
            .collect(),
    }
}

#[derive(Clone)]
pub struct LmeMcpServer {
    session: Arc<FitSession>,
}

impl LmeMcpServer {
    pub fn new() -> Self {
        Self {
            session: Arc::new(FitSession::new()),
        }
    }
}

impl Default for LmeMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LmeFitParams {
    /// Wilkinson formula, e.g. `Reaction ~ Days + (1 | Subject)`.
    formula: String,
    /// Absolute or relative path to a CSV file on the server host.
    data_path: String,
    /// Use REML when true, ML when false.
    #[serde(default = "default_reml")]
    reml: bool,
}

fn default_reml() -> bool {
    true
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FitIdParams {
    /// Identifier returned by `lme_fit`.
    fit_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LmeAnovaParams {
    fit_id: String,
    #[serde(default = "default_ddf")]
    ddf_method: String,
    #[serde(default = "default_anova_type")]
    anova_type: String,
}

fn default_ddf() -> String {
    "satterthwaite".to_string()
}

fn default_anova_type() -> String {
    "III".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LmeBootParams {
    fit_id: String,
    #[serde(default = "default_nsim")]
    nsim: usize,
    #[serde(default = "default_boot_method")]
    method: String,
    #[serde(default = "default_reml")]
    reml: bool,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    n_jobs: Option<usize>,
    #[serde(default = "default_conf_level")]
    level: f64,
}

fn default_nsim() -> usize {
    200
}

fn default_boot_method() -> String {
    "parametric".to_string()
}

fn default_conf_level() -> f64 {
    0.95
}

#[tool_router(server_handler)]
impl LmeMcpServer {
    #[tool(description = "Fit a Gaussian linear mixed model (lmer) from a CSV path and formula.")]
    fn lme_fit(
        &self,
        Parameters(LmeFitParams {
            formula,
            data_path,
            reml,
        }): Parameters<LmeFitParams>,
    ) -> Result<String, McpError> {
        let (path, df) =
            load_csv(&data_path).map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let fit =
            lmer(&formula, &df, reml).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let fit_id = Uuid::new_v4().to_string();
        let cached = CachedFit {
            formula: formula.clone(),
            data_path: path,
            reml,
            fit,
        };
        let summary = fit_summary_from_cached(&fit_id, &cached);
        self.session.insert(fit_id, cached);
        tool_json(&summary)
    }

    #[tool(description = "List fit_ids currently stored in the MCP session.")]
    fn lme_list_fits(&self) -> Result<String, McpError> {
        let entries: Vec<FitListEntry> = self
            .session
            .list()
            .into_iter()
            .map(|(id, c)| FitListEntry {
                fit_id: id,
                formula: c.formula,
                data_path: c.data_path.display().to_string(),
                reml: c.reml,
                num_obs: c.fit.num_obs,
            })
            .collect();
        tool_json(&json!({ "fits": entries }))
    }

    #[tool(description = "Return a JSON summary for a cached fit.")]
    fn lme_fit_summary(
        &self,
        Parameters(FitIdParams { fit_id }): Parameters<FitIdParams>,
    ) -> Result<String, McpError> {
        let cached = self
            .session
            .get(&fit_id)
            .ok_or_else(|| McpError::invalid_params(format!("unknown fit_id '{fit_id}'"), None))?;
        tool_json(&fit_summary_from_cached(&fit_id, &cached))
    }

    #[tool(description = "Drop a cached fit from the MCP session.")]
    fn lme_forget_fit(
        &self,
        Parameters(FitIdParams { fit_id }): Parameters<FitIdParams>,
    ) -> Result<String, McpError> {
        if self.session.remove(&fit_id) {
            tool_json(&json!({ "forgotten": fit_id }))
        } else {
            Err(McpError::invalid_params(
                format!("unknown fit_id '{fit_id}'"),
                None,
            ))
        }
    }

    #[tool(
        description = "Type I/II/III fixed-effects ANOVA for a cached LMM. Applies Satterthwaite or Kenward-Roger dfs automatically when requested."
    )]
    fn lme_anova(
        &self,
        Parameters(LmeAnovaParams {
            fit_id,
            ddf_method,
            anova_type,
        }): Parameters<LmeAnovaParams>,
    ) -> Result<String, McpError> {
        let mut cached = self
            .session
            .get(&fit_id)
            .ok_or_else(|| McpError::invalid_params(format!("unknown fit_id '{fit_id}'"), None))?;
        let ddf = parse_ddf_method(&ddf_method)?;
        let atype = parse_anova_type(&anova_type)?;

        if matches!(ddf, DdfMethod::Satterthwaite) {
            let (_, df) = load_csv(cached.data_path.to_string_lossy().as_ref())
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
            cached
                .fit
                .with_satterthwaite(&df)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        } else if matches!(ddf, DdfMethod::KenwardRoger) {
            let (_, df) = load_csv(cached.data_path.to_string_lossy().as_ref())
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
            cached
                .fit
                .with_kenward_roger(&df)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        let table = cached
            .fit
            .anova_typed(atype, ddf)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        tool_json(&anova_to_summary(&fit_id, table))
    }

    #[tool(
        description = "Parametric or residual bootstrap refits (bootMer-style) for a cached Gaussian LMM."
    )]
    fn lme_boot(
        &self,
        Parameters(LmeBootParams {
            fit_id,
            nsim,
            method,
            reml,
            seed,
            n_jobs,
            level,
        }): Parameters<LmeBootParams>,
    ) -> Result<String, McpError> {
        let cached = self
            .session
            .get(&fit_id)
            .ok_or_else(|| McpError::invalid_params(format!("unknown fit_id '{fit_id}'"), None))?;
        let (_, df) = load_csv(cached.data_path.to_string_lossy().as_ref())
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let boot_method = parse_boot_method(&method)?;
        let boot = boot_lmer(
            &cached.formula,
            &df,
            &cached.fit,
            nsim,
            boot_method,
            reml,
            seed,
            n_jobs,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let ci = boot
            .confint_percentile(level)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let summary = BootSummary {
            fit_id,
            method: method.clone(),
            nsim,
            prop_converged: boot.prop_converged,
            level,
            intervals: ci
                .names
                .iter()
                .enumerate()
                .map(|(i, name)| BootConfintRow {
                    name: name.clone(),
                    estimate: ci.estimate[i],
                    lower: ci.lower[i],
                    upper: ci.upper[i],
                })
                .collect(),
        };
        tool_json(&summary)
    }
}
