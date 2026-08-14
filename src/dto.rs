//! Protocol-neutral request and response types for agent-facing operations.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Semantic model family stored in the protocol-neutral session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(crate = "rmcp::schemars")]
pub enum ModelKind {
    /// Ordinary linear model without random effects.
    Lm,
    /// Gaussian linear mixed-effects model.
    Lmm,
    /// Generalized linear mixed-effects model.
    Glmm,
    /// Nonlinear mixed-effects model.
    Nlmm,
}

impl ModelKind {
    /// Stable lowercase name used in diagnostics and protocol metadata.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lm => "lm",
            Self::Lmm => "lmm",
            Self::Glmm => "glmm",
            Self::Nlmm => "nlmm",
        }
    }
}

/// Population-level versus random-effects-conditional prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(crate = "rmcp::schemars")]
pub enum PredictionMode {
    /// Fixed/population effects only.
    Population,
    /// Include fitted random effects for known grouping levels.
    Conditional,
}

/// Prediction scale for generalized models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(crate = "rmcp::schemars")]
pub enum PredictionScale {
    /// Linear-predictor scale.
    Link,
    /// Response scale after applying the inverse link where applicable.
    Response,
}

/// Confidence-interval method exposed by the semantic API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(crate = "rmcp::schemars")]
pub enum ConfidenceIntervalMethod {
    /// Wald interval from the fitted coefficient standard error.
    Wald,
    /// Profile-likelihood interval (LMM/GLMM only).
    Profile,
}

/// Semantic model-fitting request shared by protocol adapters.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct FitModelRequest {
    /// Statistical model family: `lm`, `lmm`, `glmm`, or `nlmm`.
    pub model_kind: ModelKind,
    /// Model formula. LM/LMM/GLMM use Wilkinson syntax; NLMM uses three-part `nlmer` syntax.
    pub formula: String,
    /// Absolute or relative path to a CSV file on the server host.
    pub data_path: String,
    /// REML setting for LMM/NLMM. Defaults to true for LMM and false for NLMM.
    #[serde(default)]
    pub reml: Option<bool>,
    /// GLMM distribution family: binomial, poisson, gaussian, or gamma.
    #[serde(default)]
    pub family: Option<String>,
    /// Optional GLMM link. When omitted, the canonical link for the family is used.
    #[serde(default)]
    pub link: Option<String>,
    /// Adaptive Gauss-Hermite quadrature points for GLMM/NLMM. Defaults to 1.
    #[serde(default)]
    pub n_agq: Option<usize>,
    /// Optional named starting values for NLMM population parameters.
    #[serde(default)]
    pub start: Option<BTreeMap<String, f64>>,
}

#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct FitLmmRequest {
    /// Wilkinson formula, e.g. `Reaction ~ Days + (1 | Subject)`.
    pub formula: String,
    /// Absolute or relative path to a CSV file on the server host.
    pub data_path: String,
    /// Use REML when true, ML when false.
    #[serde(default = "default_reml")]
    pub reml: bool,
}

#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct FitIdRequest {
    /// Identifier returned by a fitting operation.
    pub fit_id: String,
}

#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct AnovaRequest {
    pub fit_id: String,
    #[serde(default = "default_ddf")]
    pub ddf_method: String,
    #[serde(default = "default_anova_type")]
    pub anova_type: String,
}

/// Legacy LMM-only bootstrap request retained for `lme_boot`.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
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

/// Model-aware bootstrap request used by the semantic `bootstrap` operation.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ModelBootstrapRequest {
    pub fit_id: String,
    #[serde(default = "default_nsim")]
    pub nsim: usize,
    #[serde(default = "default_boot_method")]
    pub method: String,
    /// Optional LMM REML/ML refit setting. Invalid for GLMM.
    #[serde(default)]
    pub reml: Option<bool>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub n_jobs: Option<usize>,
    #[serde(default = "default_conf_level")]
    pub level: f64,
}

/// Compare two cached nested models by likelihood-ratio test.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CompareModelsRequest {
    pub fit_id_a: String,
    pub fit_id_b: String,
}

/// Confidence intervals for fixed-effect coefficients.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ConfidenceIntervalsRequest {
    pub fit_id: String,
    #[serde(default = "default_conf_level")]
    pub level: f64,
    #[serde(default = "default_ci_method")]
    pub method: ConfidenceIntervalMethod,
    /// Optional coefficient names. Omit or pass an empty list for all coefficients.
    #[serde(default)]
    pub parameters: Option<Vec<String>>,
}

/// Prediction request for a cached model.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct PredictRequest {
    pub fit_id: String,
    /// CSV path for new data. Omit to predict on the original fitted dataset.
    #[serde(default)]
    pub data_path: Option<String>,
    #[serde(default = "default_prediction_mode")]
    pub mode: PredictionMode,
    #[serde(default = "default_prediction_scale")]
    pub scale: PredictionScale,
    /// Permit unseen grouping levels for conditional mixed-model prediction.
    #[serde(default)]
    pub allow_new_levels: bool,
}

/// Group-preserving cross-validation request.
#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CrossValidateRequest {
    pub fit_id: String,
    /// Grouping column whose levels are kept intact across folds.
    pub group_col: String,
    #[serde(default = "default_n_splits")]
    pub n_splits: usize,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub n_jobs: Option<usize>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct FitSummary {
    pub fit_id: String,
    pub model_kind: ModelKind,
    pub formula: String,
    pub data_path: String,
    pub reml: Option<bool>,
    pub family: Option<String>,
    pub link: Option<String>,
    pub n_agq: Option<usize>,
    pub start: Option<BTreeMap<String, f64>>,
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

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct FitListEntry {
    pub fit_id: String,
    pub model_kind: ModelKind,
    pub formula: String,
    pub data_path: String,
    pub reml: Option<bool>,
    pub family: Option<String>,
    pub link: Option<String>,
    pub n_agq: Option<usize>,
    pub start: Option<BTreeMap<String, f64>>,
    pub num_obs: usize,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct FitListSummary {
    pub fits: Vec<FitListEntry>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ForgetFitResult {
    pub forgotten: String,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct AnovaRow {
    pub term: String,
    pub num_df: f64,
    pub den_df: f64,
    pub f_value: f64,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct AnovaSummary {
    pub fit_id: String,
    pub anova_type: String,
    pub method: String,
    pub rows: Vec<AnovaRow>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct BootConfintRow {
    pub name: String,
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct BootSummary {
    pub fit_id: String,
    pub method: String,
    pub nsim: usize,
    pub prop_converged: f64,
    pub level: f64,
    pub intervals: Vec<BootConfintRow>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CompareModelsSummary {
    pub fit_id_a: String,
    pub fit_id_b: String,
    pub model_kind: ModelKind,
    pub formula_0: String,
    pub formula_1: String,
    pub n_params_0: usize,
    pub n_params_1: usize,
    pub deviance_0: f64,
    pub deviance_1: f64,
    pub chi_sq: f64,
    pub df: usize,
    pub p_value: f64,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ConfidenceIntervalRow {
    pub name: String,
    pub estimate: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct ConfidenceIntervalsSummary {
    pub fit_id: String,
    pub method: ConfidenceIntervalMethod,
    pub level: f64,
    pub intervals: Vec<ConfidenceIntervalRow>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct PredictionSummary {
    pub fit_id: String,
    pub model_kind: ModelKind,
    pub data_path: String,
    pub mode: PredictionMode,
    pub scale: PredictionScale,
    pub allow_new_levels: bool,
    pub predictions: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CrossValidationFold {
    pub fold: usize,
    pub n_train_groups: usize,
    pub n_test_groups: usize,
    pub n_train_obs: usize,
    pub n_test_obs: usize,
    pub rmse: f64,
    pub mae: f64,
    pub mean_log_loss: Option<f64>,
    pub converged: bool,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct CrossValidationSummary {
    pub fit_id: String,
    pub model_kind: ModelKind,
    pub group_col: String,
    pub n_splits: usize,
    pub rmse: f64,
    pub mae: f64,
    pub mean_log_loss: Option<f64>,
    pub all_converged: bool,
    pub oof_predictions: Vec<f64>,
    pub test_fold: Vec<i32>,
    pub folds: Vec<CrossValidationFold>,
}

#[derive(Debug, Clone, Serialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DiagnosticsSummary {
    pub fit_id: String,
    pub model_kind: ModelKind,
    pub converged: bool,
    pub iterations: Option<u64>,
    pub num_obs: usize,
    pub n_fixed: usize,
    pub n_theta: usize,
    pub finite_coefficients: bool,
    pub finite_residuals: bool,
    pub has_standard_errors: bool,
    pub residual_mean: Option<f64>,
    pub residual_rmse: Option<f64>,
    pub max_abs_residual: Option<f64>,
    pub messages: Vec<String>,
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

fn default_ci_method() -> ConfidenceIntervalMethod {
    ConfidenceIntervalMethod::Wald
}

fn default_prediction_mode() -> PredictionMode {
    PredictionMode::Population
}

fn default_prediction_scale() -> PredictionScale {
    PredictionScale::Response
}

fn default_n_splits() -> usize {
    5
}
