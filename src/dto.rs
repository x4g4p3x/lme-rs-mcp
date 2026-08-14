//! Protocol-neutral request and response types for agent-facing operations.

use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FitLmmRequest {
    /// Wilkinson formula, e.g. `Reaction ~ Days + (1 | Subject)`.
    pub formula: String,
    /// Absolute or relative path to a CSV file on the server host.
    pub data_path: String,
    /// Use REML when true, ML when false.
    #[serde(default = "default_reml")]
    pub reml: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FitIdRequest {
    /// Identifier returned by `lme_fit`.
    pub fit_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AnovaRequest {
    pub fit_id: String,
    #[serde(default = "default_ddf")]
    pub ddf_method: String,
    #[serde(default = "default_anova_type")]
    pub anova_type: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct BootstrapRequest {
    pub fit_id: String,
    #[serde(default = "default_nsim")]
    pub nsim: usize,
    #[serde(default = "default_boot_method")]
    pub method: String,
    #[serde(default = "default_reml")]
    pub reml: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub n_jobs: Option<usize>,
    #[serde(default = "default_conf_level")]
    pub level: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FitSummary {
    pub fit_id: String,
    pub formula: String,
    pub data_path: String,
    pub reml: bool,
    pub num_obs: usize,
    pub converged: bool,
    pub fixed_names: Vec<String>,
    pub coefficients: Vec<f64>,
    pub beta_se: Option<Vec<f64>>,
    pub sigma2: Option<f64>,
    pub aic: Option<f64>,
    pub bic: Option<f64>,
    pub log_likelihood: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FitListEntry {
    pub fit_id: String,
    pub formula: String,
    pub data_path: String,
    pub reml: bool,
    pub num_obs: usize,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FitListSummary {
    pub fits: Vec<FitListEntry>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ForgetFitResult {
    pub forgotten: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AnovaRow {
    pub term: String,
    pub num_df: f64,
    pub den_df: f64,
    pub f_value: f64,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AnovaSummary {
    pub fit_id: String,
    pub anova_type: String,
    pub method: String,
    pub rows: Vec<AnovaRow>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BootConfintRow {
    pub name: String,
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BootSummary {
    pub fit_id: String,
    pub method: String,
    pub nsim: usize,
    pub prop_converged: f64,
    pub level: f64,
    pub intervals: Vec<BootConfintRow>,
}

fn default_reml() -> bool {
    true
}

fn default_ddf() -> String {
    "satterthwaite".to_string()
}

fn default_anova_type() -> String {
    "III".to_string()
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
