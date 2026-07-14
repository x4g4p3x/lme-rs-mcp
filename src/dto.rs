//! JSON-serializable summaries returned by MCP tools.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct FitListEntry {
    pub fit_id: String,
    pub formula: String,
    pub data_path: String,
    pub reml: bool,
    pub num_obs: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnovaRow {
    pub term: String,
    pub num_df: f64,
    pub den_df: f64,
    pub f_value: f64,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnovaSummary {
    pub fit_id: String,
    pub anova_type: String,
    pub method: String,
    pub rows: Vec<AnovaRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootConfintRow {
    pub name: String,
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootSummary {
    pub fit_id: String,
    pub method: String,
    pub nsim: usize,
    pub prop_converged: f64,
    pub level: f64,
    pub intervals: Vec<BootConfintRow>,
}
