//! Protocol-neutral agent API around `lme-rs`.
//!
//! This module owns statistical operations and session semantics without depending on
//! MCP transport types. Protocol adapters (MCP today, SCP or others later) should map
//! their wire-level requests and errors onto this API instead of calling `lme-rs`
//! directly.

use std::sync::Arc;

use lme_rs::family::{Family, Link};
use lme_rs::{
    boot_lmer, cv_grouped, cv_grouped_glmer, glmer_with_link, lm, lmer, nlmer_with_options,
    AnovaType, BootLmerMethod, BootLmerResult, CvGroupedResult, DdfMethod, FixedEffectsAnovaResult,
    NlmerOptions, NlmmStart,
};
use uuid::Uuid;

use crate::data::load_csv;
use crate::dto::{
    AnovaRequest, AnovaRow, AnovaSummary, BootConfintRow, BootSummary, BootstrapRequest,
    CompareModelsRequest, CompareModelsSummary, ConfidenceIntervalMethod, ConfidenceIntervalRow,
    ConfidenceIntervalsRequest, ConfidenceIntervalsSummary, CrossValidateRequest,
    CrossValidationFold, CrossValidationSummary, DiagnosticsSummary, FitListEntry, FitListSummary,
    FitLmmRequest, FitModelRequest, FitSummary, ForgetFitResult, ModelBootstrapRequest, ModelKind,
    PredictRequest, PredictionMode, PredictionScale, PredictionSummary,
};
use crate::session::{CachedFit, FitSession};

#[derive(Debug, thiserror::Error)]
pub enum AgentApiError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Computation(String),
}

#[derive(Clone)]
pub struct LmeAgentApi {
    session: Arc<FitSession>,
}

impl LmeAgentApi {
    pub fn new() -> Self {
        Self {
            session: Arc::new(FitSession::new()),
        }
    }

    /// Fit an LM, LMM, GLMM, or NLMM through the protocol-neutral semantic API.
    pub fn fit_model(&self, request: FitModelRequest) -> Result<FitSummary, AgentApiError> {
        validate_fit_input(&request.formula, &request.data_path)?;

        match request.model_kind {
            ModelKind::Lm => self.fit_model_lm(request),
            ModelKind::Lmm => self.fit_model_lmm(request),
            ModelKind::Glmm => self.fit_model_glmm(request),
            ModelKind::Nlmm => self.fit_model_nlmm(request),
        }
    }

    /// Backwards-compatible Gaussian LMM fitting entry point used by `lme_fit`.
    pub fn fit_lmm(&self, request: FitLmmRequest) -> Result<FitSummary, AgentApiError> {
        self.fit_model(FitModelRequest {
            model_kind: ModelKind::Lmm,
            formula: request.formula,
            data_path: request.data_path,
            reml: Some(request.reml),
            family: None,
            link: None,
            n_agq: None,
            start: None,
        })
    }

    pub fn list_fits(&self) -> FitListSummary {
        let fits = self
            .session
            .list()
            .into_iter()
            .map(|(fit_id, cached)| {
                let (family, link) = glmm_family_link(&cached);
                FitListEntry {
                    fit_id,
                    model_kind: cached.model_kind,
                    formula: cached.formula,
                    data_path: cached.data_path.display().to_string(),
                    reml: cached.reml,
                    family,
                    link,
                    n_agq: cached.n_agq,
                    start: cached.start,
                    num_obs: cached.fit.num_obs,
                }
            })
            .collect();
        FitListSummary { fits }
    }

    /// Semantic lifecycle alias for [`Self::list_fits`].
    pub fn list_models(&self) -> FitListSummary {
        self.list_fits()
    }

    pub fn fit_summary(&self, fit_id: &str) -> Result<FitSummary, AgentApiError> {
        let cached = self.cached_fit(fit_id)?;
        Ok(fit_summary_from_cached(fit_id, &cached))
    }

    /// Semantic lifecycle alias for [`Self::fit_summary`].
    pub fn model_summary(&self, fit_id: &str) -> Result<FitSummary, AgentApiError> {
        self.fit_summary(fit_id)
    }

    pub fn forget_fit(&self, fit_id: &str) -> Result<ForgetFitResult, AgentApiError> {
        if self.session.remove(fit_id) {
            Ok(ForgetFitResult {
                forgotten: fit_id.to_string(),
            })
        } else {
            Err(AgentApiError::NotFound(format!(
                "unknown fit_id '{fit_id}'"
            )))
        }
    }

    /// Semantic lifecycle alias for [`Self::forget_fit`].
    pub fn forget_model(&self, fit_id: &str) -> Result<ForgetFitResult, AgentApiError> {
        self.forget_fit(fit_id)
    }

    /// Type I/II/III fixed-effects ANOVA for cached Gaussian LMMs.
    pub fn anova(&self, request: AnovaRequest) -> Result<AnovaSummary, AgentApiError> {
        let mut cached = self.cached_fit(&request.fit_id)?;
        ensure_lmm_operation(&cached, "anova")?;
        let ddf = parse_ddf_method(&request.ddf_method)?;
        let anova_type = parse_anova_type(&request.anova_type)?;

        let (_, df) = load_cached_data(&cached)?;
        match ddf {
            DdfMethod::Satterthwaite => {
                cached
                    .fit
                    .with_satterthwaite(&df)
                    .map_err(|e| AgentApiError::Computation(e.to_string()))?;
            }
            DdfMethod::KenwardRoger => {
                cached
                    .fit
                    .with_kenward_roger(&df)
                    .map_err(|e| AgentApiError::Computation(e.to_string()))?;
            }
        }

        let table = cached
            .fit
            .anova_typed(anova_type, ddf)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;
        Ok(anova_to_summary(&request.fit_id, table))
    }

    /// Legacy LMM-only bootstrap used by the `lme_boot` compatibility tool.
    pub fn bootstrap(&self, request: BootstrapRequest) -> Result<BootSummary, AgentApiError> {
        validate_bootstrap_request(request.nsim, request.n_jobs, request.level)?;

        let cached = self.cached_fit(&request.fit_id)?;
        ensure_lmm_operation(&cached, "bootstrap")?;
        let (_, df) = load_cached_data(&cached)?;
        let method = parse_boot_method(&request.method)?;
        let boot = boot_lmer(
            &cached.formula,
            &df,
            &cached.fit,
            request.nsim,
            method,
            request.reml,
            request.seed,
            request.n_jobs,
        )
        .map_err(|e| AgentApiError::Computation(e.to_string()))?;

        boot_to_summary(
            request.fit_id,
            request.method,
            request.nsim,
            request.level,
            boot,
        )
    }

    /// Model-aware semantic bootstrap. Supports LMM and parametric GLMM refits.
    pub fn bootstrap_model(
        &self,
        request: ModelBootstrapRequest,
    ) -> Result<BootSummary, AgentApiError> {
        validate_bootstrap_request(request.nsim, request.n_jobs, request.level)?;
        let cached = self.cached_fit(&request.fit_id)?;
        let (_, df) = load_cached_data(&cached)?;
        let method = parse_boot_method(&request.method)?;

        let boot = match cached.model_kind {
            ModelKind::Lmm => {
                let reml = request.reml.unwrap_or(cached.reml.unwrap_or(true));
                cached
                    .fit
                    .boot(
                        &cached.formula,
                        &df,
                        request.nsim,
                        method,
                        reml,
                        request.seed,
                        request.n_jobs,
                    )
                    .map_err(|e| AgentApiError::Computation(e.to_string()))?
            }
            ModelKind::Glmm => {
                if request.reml.is_some() {
                    return Err(AgentApiError::InvalidInput(
                        "reml does not apply to GLMM bootstrap".to_string(),
                    ));
                }
                match method {
                    BootLmerMethod::Parametric => cached
                        .fit
                        .boot_glmer(
                            &cached.formula,
                            &df,
                            request.nsim,
                            BootLmerMethod::Parametric,
                            request.seed,
                            request.n_jobs,
                        )
                        .map_err(|e| AgentApiError::Computation(e.to_string()))?,
                    BootLmerMethod::Residual => {
                        return Err(AgentApiError::InvalidInput(
                            "GLMM bootstrap supports the parametric method only".to_string(),
                        ));
                    }
                }
            }
            other => {
                return Err(AgentApiError::InvalidInput(format!(
                    "bootstrap currently supports lmm and glmm models only; fit is {}",
                    other.as_str()
                )));
            }
        };

        boot_to_summary(
            request.fit_id,
            request.method,
            request.nsim,
            request.level,
            boot,
        )
    }

    /// Compare two cached nested models using the engine's likelihood-ratio test.
    pub fn compare_models(
        &self,
        request: CompareModelsRequest,
    ) -> Result<CompareModelsSummary, AgentApiError> {
        let fit_a = self.cached_fit(&request.fit_id_a)?;
        let fit_b = self.cached_fit(&request.fit_id_b)?;

        if fit_a.model_kind != fit_b.model_kind {
            return Err(AgentApiError::InvalidInput(format!(
                "compare_models requires the same model_kind; got {} and {}",
                fit_a.model_kind.as_str(),
                fit_b.model_kind.as_str()
            )));
        }
        if fit_a.model_kind == ModelKind::Lm {
            return Err(AgentApiError::InvalidInput(
                "compare_models requires mixed-model likelihood metadata; lm fits from lme-rs 0.2.1 do not provide deviance"
                    .to_string(),
            ));
        }
        if fit_a.data_path != fit_b.data_path {
            return Err(AgentApiError::InvalidInput(
                "compare_models requires fits from the same dataset".to_string(),
            ));
        }
        if fit_a.reml == Some(true) || fit_b.reml == Some(true) {
            return Err(AgentApiError::InvalidInput(
                "compare_models requires ML fits (reml=false) when REML is applicable".to_string(),
            ));
        }
        if fit_a.model_kind == ModelKind::Glmm {
            let (family_a, link_a) = glmm_family_link(&fit_a);
            let (family_b, link_b) = glmm_family_link(&fit_b);
            if family_a != family_b || link_a != link_b {
                return Err(AgentApiError::InvalidInput(
                    "compare_models requires GLMMs with the same family and link".to_string(),
                ));
            }
        }

        let result = lme_rs::anova(&fit_a.fit, &fit_b.fit)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;

        Ok(CompareModelsSummary {
            fit_id_a: request.fit_id_a,
            fit_id_b: request.fit_id_b,
            model_kind: fit_a.model_kind,
            formula_0: result.formula_0,
            formula_1: result.formula_1,
            n_params_0: result.n_params_0,
            n_params_1: result.n_params_1,
            deviance_0: result.deviance_0,
            deviance_1: result.deviance_1,
            chi_sq: result.chi_sq,
            df: result.df,
            p_value: result.p_value,
        })
    }

    /// Fixed-effect confidence intervals for a cached model.
    pub fn confidence_intervals(
        &self,
        request: ConfidenceIntervalsRequest,
    ) -> Result<ConfidenceIntervalsSummary, AgentApiError> {
        validate_conf_level(request.level)?;
        let cached = self.cached_fit(&request.fit_id)?;
        let indices = resolve_parameter_indices(&cached, request.parameters.as_deref())?;

        let (lower, upper, names) = match request.method {
            ConfidenceIntervalMethod::Wald => {
                let ci = cached
                    .fit
                    .confint(request.level)
                    .map_err(|e| AgentApiError::Computation(e.to_string()))?;
                let mut lower = Vec::with_capacity(indices.len());
                let mut upper = Vec::with_capacity(indices.len());
                let mut names = Vec::with_capacity(indices.len());
                for &index in &indices {
                    lower.push(ci.lower[index]);
                    upper.push(ci.upper[index]);
                    names.push(ci.names[index].clone());
                }
                (lower, upper, names)
            }
            ConfidenceIntervalMethod::Profile => {
                if !matches!(cached.model_kind, ModelKind::Lmm | ModelKind::Glmm) {
                    return Err(AgentApiError::InvalidInput(format!(
                        "profile confidence intervals support lmm and glmm models only; fit is {}",
                        cached.model_kind.as_str()
                    )));
                }
                let (_, df) = load_cached_data(&cached)?;
                let ci = if request.parameters.as_ref().is_some_and(|p| !p.is_empty()) {
                    cached
                        .fit
                        .confint_profile_parms(request.level, &df, &indices)
                } else {
                    cached.fit.confint_profile(request.level, &df)
                }
                .map_err(|e| AgentApiError::Computation(e.to_string()))?;
                (ci.lower.to_vec(), ci.upper.to_vec(), ci.names)
            }
        };

        let mut intervals = Vec::with_capacity(names.len());
        for (position, name) in names.into_iter().enumerate() {
            intervals.push(ConfidenceIntervalRow {
                name,
                estimate: cached.fit.coefficients[indices[position]],
                lower: lower[position],
                upper: upper[position],
            });
        }

        Ok(ConfidenceIntervalsSummary {
            fit_id: request.fit_id,
            method: request.method,
            level: request.level,
            intervals,
        })
    }

    /// Predict from a cached model on original or new CSV data.
    pub fn predict(&self, request: PredictRequest) -> Result<PredictionSummary, AgentApiError> {
        let cached = self.cached_fit(&request.fit_id)?;
        if cached.model_kind == ModelKind::Lm && request.mode == PredictionMode::Conditional {
            return Err(AgentApiError::InvalidInput(
                "conditional prediction does not apply to lm models".to_string(),
            ));
        }

        let (path, df) = match request.data_path.as_deref() {
            Some(path) => load_csv(path).map_err(|e| AgentApiError::InvalidInput(e.to_string()))?,
            None => load_cached_data(&cached)?,
        };

        let predictions = match (request.mode, request.scale) {
            (PredictionMode::Population, PredictionScale::Link) => cached.fit.predict(&df),
            (PredictionMode::Population, PredictionScale::Response) => {
                cached.fit.predict_response(&df)
            }
            (PredictionMode::Conditional, PredictionScale::Link) => cached
                .fit
                .predict_conditional(&df, request.allow_new_levels),
            (PredictionMode::Conditional, PredictionScale::Response) => cached
                .fit
                .predict_conditional_response(&df, request.allow_new_levels),
        }
        .map_err(|e| AgentApiError::Computation(e.to_string()))?;

        Ok(PredictionSummary {
            fit_id: request.fit_id,
            model_kind: cached.model_kind,
            data_path: path.display().to_string(),
            mode: request.mode,
            scale: request.scale,
            allow_new_levels: request.allow_new_levels,
            predictions: predictions.to_vec(),
        })
    }

    /// Group-preserving cross-validation for cached LMM/GLMM models.
    pub fn cross_validate(
        &self,
        request: CrossValidateRequest,
    ) -> Result<CrossValidationSummary, AgentApiError> {
        if request.group_col.trim().is_empty() {
            return Err(AgentApiError::InvalidInput(
                "group_col must not be empty".to_string(),
            ));
        }
        if request.n_splits < 2 {
            return Err(AgentApiError::InvalidInput(
                "n_splits must be at least 2".to_string(),
            ));
        }
        if request.n_jobs == Some(0) {
            return Err(AgentApiError::InvalidInput(
                "n_jobs must be greater than zero when provided".to_string(),
            ));
        }

        let cached = self.cached_fit(&request.fit_id)?;
        let (_, df) = load_cached_data(&cached)?;
        let result = match cached.model_kind {
            ModelKind::Lmm => cv_grouped(
                &cached.formula,
                &df,
                &request.group_col,
                request.n_splits,
                cached.reml.unwrap_or(true),
                request.seed,
                request.n_jobs,
            )
            .map_err(|e| AgentApiError::Computation(e.to_string()))?,
            ModelKind::Glmm => {
                let family = cached.fit.family.clone().ok_or_else(|| {
                    AgentApiError::Computation(
                        "cached GLMM is missing its distribution family".to_string(),
                    )
                })?;
                let link = cached
                    .fit
                    .link_name
                    .as_deref()
                    .map(Link::parse)
                    .transpose()
                    .map_err(|e| AgentApiError::Computation(e.to_string()))?
                    .unwrap_or_else(|| Link::default_for(family));
                cv_grouped_glmer(
                    &cached.formula,
                    &df,
                    &request.group_col,
                    request.n_splits,
                    family,
                    link,
                    cached.n_agq.unwrap_or(1),
                    cached.fit.weights.clone(),
                    request.seed,
                    request.n_jobs,
                )
                .map_err(|e| AgentApiError::Computation(e.to_string()))?
            }
            other => {
                return Err(AgentApiError::InvalidInput(format!(
                    "cross_validate currently supports lmm and glmm models only; fit is {}",
                    other.as_str()
                )));
            }
        };

        Ok(cv_to_summary(request.fit_id, cached.model_kind, result))
    }

    /// Return convergence and fit-quality diagnostics without refitting the model.
    pub fn diagnostics(&self, fit_id: &str) -> Result<DiagnosticsSummary, AgentApiError> {
        let cached = self.cached_fit(fit_id)?;
        let fit = &cached.fit;
        let finite_coefficients = fit.coefficients.iter().all(|value| value.is_finite());
        let finite_residuals = fit.residuals.iter().all(|value| value.is_finite());
        let has_standard_errors = fit.beta_se.is_some();

        let (residual_mean, residual_rmse, max_abs_residual) = if fit.residuals.is_empty() {
            (None, None, None)
        } else {
            let n = fit.residuals.len() as f64;
            let sum = fit.residuals.iter().sum::<f64>();
            let sum_sq = fit
                .residuals
                .iter()
                .map(|value| *value * *value)
                .sum::<f64>();
            let max_abs = fit
                .residuals
                .iter()
                .map(|value| value.abs())
                .fold(0.0_f64, f64::max);
            (Some(sum / n), Some((sum_sq / n).sqrt()), Some(max_abs))
        };

        let converged = fit.converged.unwrap_or(false);
        let mut messages = Vec::new();
        if !converged {
            messages.push("optimizer did not report convergence".to_string());
        }
        if !finite_coefficients {
            messages.push("one or more fitted coefficients are non-finite".to_string());
        }
        if !finite_residuals {
            messages.push("one or more residuals are non-finite".to_string());
        }
        if !has_standard_errors {
            messages.push("fixed-effect standard errors are unavailable".to_string());
        }
        if cached.model_kind == ModelKind::Lm {
            messages.push(
                "lme-rs 0.2.1 lm() does not report likelihood/AIC/BIC or coefficient standard errors"
                    .to_string(),
            );
        }

        Ok(DiagnosticsSummary {
            fit_id: fit_id.to_string(),
            model_kind: cached.model_kind,
            converged,
            iterations: fit.iterations,
            num_obs: fit.num_obs,
            n_fixed: fit.coefficients.len(),
            n_theta: fit.theta.as_ref().map_or(0, |theta| theta.len()),
            finite_coefficients,
            finite_residuals,
            has_standard_errors,
            residual_mean,
            residual_rmse,
            max_abs_residual,
            messages,
        })
    }

    fn fit_model_lm(&self, request: FitModelRequest) -> Result<FitSummary, AgentApiError> {
        reject_fields_for_lm(&request)?;
        let (path, df) =
            load_csv(&request.data_path).map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;

        let ast = lme_rs::formula::parse(&request.formula)
            .map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
        let matrices = lme_rs::model_matrix::build_design_matrices(&ast, &df)
            .map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
        if !matrices.re_blocks.is_empty() {
            return Err(AgentApiError::InvalidInput(
                "model_kind 'lm' does not accept random-effects terms; use lmm instead".to_string(),
            ));
        }
        if matrices.offset.is_some() {
            return Err(AgentApiError::InvalidInput(
                "model_kind 'lm' does not expose offset terms in lme-rs 0.2.1".to_string(),
            ));
        }

        let mut fit =
            lm(&matrices.y, &matrices.x).map_err(|e| AgentApiError::Computation(e.to_string()))?;
        fit.formula = Some(request.formula.clone());
        fit.fixed_names = Some(matrices.fixed_names.clone());
        fit.fixed_term_assign = Some(matrices.fixed_term_assign.clone());
        fit.fixed_design_x = Some(matrices.x.clone());
        fit.categorical_levels = Some(matrices.categorical_levels.clone());
        fit.num_obs = matrices.y.len();
        fit.converged = Some(true);

        self.cache_fit(CachedFit {
            model_kind: ModelKind::Lm,
            formula: request.formula,
            data_path: path,
            reml: None,
            n_agq: None,
            start: None,
            fit,
        })
    }

    fn fit_model_lmm(&self, request: FitModelRequest) -> Result<FitSummary, AgentApiError> {
        if request.family.is_some() {
            return Err(AgentApiError::InvalidInput(
                "family applies to glmm models only".to_string(),
            ));
        }
        if request.link.is_some() {
            return Err(AgentApiError::InvalidInput(
                "link applies to glmm models only".to_string(),
            ));
        }
        if request.n_agq.is_some() {
            return Err(AgentApiError::InvalidInput(
                "n_agq applies to glmm and nlmm models only".to_string(),
            ));
        }
        if request.start.is_some() {
            return Err(AgentApiError::InvalidInput(
                "start applies to nlmm models only".to_string(),
            ));
        }

        let reml = request.reml.unwrap_or(true);
        let (path, df) =
            load_csv(&request.data_path).map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
        let fit = lmer(&request.formula, &df, reml)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;
        self.cache_fit(CachedFit {
            model_kind: ModelKind::Lmm,
            formula: request.formula,
            data_path: path,
            reml: Some(reml),
            n_agq: None,
            start: None,
            fit,
        })
    }

    fn fit_model_glmm(&self, request: FitModelRequest) -> Result<FitSummary, AgentApiError> {
        if request.reml.is_some() {
            return Err(AgentApiError::InvalidInput(
                "reml does not apply to glmm models".to_string(),
            ));
        }
        if request.start.is_some() {
            return Err(AgentApiError::InvalidInput(
                "start applies to nlmm models only".to_string(),
            ));
        }

        let family = parse_glmm_family(request.family.as_deref())?;
        let link = parse_glmm_link(request.link.as_deref(), family)?;
        let n_agq = request.n_agq.unwrap_or(1);
        validate_n_agq(n_agq)?;

        let (path, df) =
            load_csv(&request.data_path).map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
        let fit = glmer_with_link(&request.formula, &df, family, link, n_agq)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;
        self.cache_fit(CachedFit {
            model_kind: ModelKind::Glmm,
            formula: request.formula,
            data_path: path,
            reml: None,
            n_agq: Some(n_agq),
            start: None,
            fit,
        })
    }

    fn fit_model_nlmm(&self, request: FitModelRequest) -> Result<FitSummary, AgentApiError> {
        if request.family.is_some() {
            return Err(AgentApiError::InvalidInput(
                "family applies to glmm models only".to_string(),
            ));
        }
        if request.link.is_some() {
            return Err(AgentApiError::InvalidInput(
                "link applies to glmm models only".to_string(),
            ));
        }

        let reml = request.reml.unwrap_or(false);
        let n_agq = request.n_agq.unwrap_or(1);
        validate_n_agq(n_agq)?;
        let start = parse_nlmm_start(request.start.as_ref())?;
        let start_metadata = request.start.clone();

        let (path, df) =
            load_csv(&request.data_path).map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
        let options = NlmerOptions {
            reml,
            start,
            n_agq,
            ..NlmerOptions::default()
        };
        let fit = nlmer_with_options(&request.formula, &df, &options)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;
        self.cache_fit(CachedFit {
            model_kind: ModelKind::Nlmm,
            formula: request.formula,
            data_path: path,
            reml: Some(reml),
            n_agq: Some(n_agq),
            start: start_metadata,
            fit,
        })
    }

    fn cache_fit(&self, cached: CachedFit) -> Result<FitSummary, AgentApiError> {
        let fit_id = Uuid::new_v4().to_string();
        let summary = fit_summary_from_cached(&fit_id, &cached);
        self.session.insert(fit_id, cached);
        Ok(summary)
    }

    fn cached_fit(&self, fit_id: &str) -> Result<CachedFit, AgentApiError> {
        self.session
            .get(fit_id)
            .ok_or_else(|| AgentApiError::NotFound(format!("unknown fit_id '{fit_id}'")))
    }
}

impl Default for LmeAgentApi {
    fn default() -> Self {
        Self::new()
    }
}

fn reject_fields_for_lm(request: &FitModelRequest) -> Result<(), AgentApiError> {
    if request.reml.is_some()
        || request.family.is_some()
        || request.link.is_some()
        || request.n_agq.is_some()
        || request.start.is_some()
    {
        return Err(AgentApiError::InvalidInput(
            "lm accepts formula and data_path only; reml, family, link, n_agq, and start do not apply"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_fit_input(formula: &str, data_path: &str) -> Result<(), AgentApiError> {
    if formula.trim().is_empty() {
        return Err(AgentApiError::InvalidInput(
            "formula must not be empty".to_string(),
        ));
    }
    if data_path.trim().is_empty() {
        return Err(AgentApiError::InvalidInput(
            "data_path must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_n_agq(n_agq: usize) -> Result<(), AgentApiError> {
    if n_agq == 0 {
        Err(AgentApiError::InvalidInput(
            "n_agq must be greater than zero".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_conf_level(level: f64) -> Result<(), AgentApiError> {
    if !level.is_finite() || level <= 0.0 || level >= 1.0 {
        Err(AgentApiError::InvalidInput(
            "level must be finite and strictly between 0 and 1".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn validate_bootstrap_request(
    nsim: usize,
    n_jobs: Option<usize>,
    level: f64,
) -> Result<(), AgentApiError> {
    if nsim == 0 {
        return Err(AgentApiError::InvalidInput(
            "nsim must be greater than zero".to_string(),
        ));
    }
    validate_conf_level(level)?;
    if n_jobs == Some(0) {
        return Err(AgentApiError::InvalidInput(
            "n_jobs must be greater than zero when provided".to_string(),
        ));
    }
    Ok(())
}

fn parse_nlmm_start(
    start: Option<&std::collections::BTreeMap<String, f64>>,
) -> Result<NlmmStart, AgentApiError> {
    let mut parsed = NlmmStart::new();
    if let Some(start) = start {
        for (name, value) in start {
            if name.trim().is_empty() {
                return Err(AgentApiError::InvalidInput(
                    "nlmm start parameter names must not be empty".to_string(),
                ));
            }
            if !value.is_finite() {
                return Err(AgentApiError::InvalidInput(format!(
                    "nlmm start value for '{name}' must be finite"
                )));
            }
            parsed.insert(name.clone(), *value);
        }
    }
    Ok(parsed)
}

fn parse_glmm_family(family: Option<&str>) -> Result<Family, AgentApiError> {
    let family = family.ok_or_else(|| {
        AgentApiError::InvalidInput(
            "family is required for glmm models (binomial, poisson, gaussian, or gamma)"
                .to_string(),
        )
    })?;

    match family.trim().to_ascii_lowercase().as_str() {
        "binomial" => Ok(Family::Binomial),
        "poisson" => Ok(Family::Poisson),
        "gaussian" | "normal" => Ok(Family::Gaussian),
        "gamma" => Ok(Family::Gamma),
        other => Err(AgentApiError::InvalidInput(format!(
            "unknown glmm family '{other}' (use binomial, poisson, gaussian, or gamma)"
        ))),
    }
}

fn parse_glmm_link(link: Option<&str>, family: Family) -> Result<Link, AgentApiError> {
    let link = match link {
        Some(raw) => Link::parse(raw).map_err(|_| {
            AgentApiError::InvalidInput(format!(
                "unknown glmm link '{}' (use logit, probit, cloglog, log, identity, inverse, or sqrt)",
                raw.trim()
            ))
        })?,
        None => Link::default_for(family),
    };

    if !link.valid_for(family) {
        return Err(AgentApiError::InvalidInput(format!(
            "link '{}' is not valid for family '{}'",
            link.name(),
            family
        )));
    }
    Ok(link)
}

fn glmm_family_link(cached: &CachedFit) -> (Option<String>, Option<String>) {
    if cached.model_kind == ModelKind::Glmm {
        (
            normalized_name(cached.fit.family_name.as_deref()),
            normalized_name(cached.fit.link_name.as_deref()),
        )
    } else {
        (None, None)
    }
}

fn fit_summary_from_cached(fit_id: &str, cached: &CachedFit) -> FitSummary {
    let fit = &cached.fit;
    let (family, link) = glmm_family_link(cached);
    FitSummary {
        fit_id: fit_id.to_string(),
        model_kind: cached.model_kind,
        formula: cached.formula.clone(),
        data_path: cached.data_path.display().to_string(),
        reml: cached.reml,
        family,
        link,
        n_agq: cached.n_agq,
        start: cached.start.clone(),
        num_obs: fit.num_obs,
        converged: fit.converged.unwrap_or(false),
        fixed_names: fit.fixed_names.clone().unwrap_or_default(),
        coefficients: fit.coefficients.to_vec(),
        beta_se: fit.beta_se.as_ref().map(|se| se.to_vec()),
        sigma2: fit.sigma2,
        aic: fit.aic,
        bic: fit.bic,
        log_likelihood: fit.log_likelihood,
    }
}

fn load_cached_data(
    cached: &CachedFit,
) -> Result<(std::path::PathBuf, polars::prelude::DataFrame), AgentApiError> {
    load_csv(cached.data_path.to_string_lossy().as_ref())
        .map_err(|e| AgentApiError::InvalidInput(e.to_string()))
}

fn resolve_parameter_indices(
    cached: &CachedFit,
    requested: Option<&[String]>,
) -> Result<Vec<usize>, AgentApiError> {
    let names = cached
        .fit
        .fixed_names
        .as_ref()
        .cloned()
        .unwrap_or_else(|| default_coefficient_names(cached.fit.coefficients.len()));

    match requested {
        Some(parameters) if !parameters.is_empty() => parameters
            .iter()
            .map(|name| {
                names
                    .iter()
                    .position(|candidate| candidate == name)
                    .ok_or_else(|| {
                        AgentApiError::InvalidInput(format!(
                            "unknown fixed-effect parameter '{name}'; available: {}",
                            names.join(", ")
                        ))
                    })
            })
            .collect(),
        _ => Ok((0..cached.fit.coefficients.len()).collect()),
    }
}

fn default_coefficient_names(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("beta_{index}")).collect()
}

fn boot_to_summary(
    fit_id: String,
    method: String,
    nsim: usize,
    level: f64,
    boot: BootLmerResult,
) -> Result<BootSummary, AgentApiError> {
    let ci = boot
        .confint_percentile(level)
        .map_err(|e| AgentApiError::Computation(e.to_string()))?;
    Ok(BootSummary {
        fit_id,
        method,
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
    })
}

fn cv_to_summary(
    fit_id: String,
    model_kind: ModelKind,
    result: CvGroupedResult,
) -> CrossValidationSummary {
    CrossValidationSummary {
        fit_id,
        model_kind,
        group_col: result.group_col,
        n_splits: result.n_splits,
        rmse: result.rmse,
        mae: result.mae,
        mean_log_loss: result.mean_log_loss,
        all_converged: result.all_converged,
        oof_predictions: result.oof_predictions.to_vec(),
        test_fold: result.test_fold.to_vec(),
        folds: result
            .folds
            .into_iter()
            .map(|fold| CrossValidationFold {
                fold: fold.fold,
                n_train_groups: fold.n_train_groups,
                n_test_groups: fold.n_test_groups,
                n_train_obs: fold.n_train_obs,
                n_test_obs: fold.n_test_obs,
                rmse: fold.rmse,
                mae: fold.mae,
                mean_log_loss: fold.mean_log_loss,
                converged: fold.converged,
            })
            .collect(),
    }
}

fn normalized_name(name: Option<&str>) -> Option<String> {
    name.map(str::to_ascii_lowercase)
}

fn ensure_lmm_operation(cached: &CachedFit, operation: &str) -> Result<(), AgentApiError> {
    if cached.model_kind == ModelKind::Lmm {
        Ok(())
    } else {
        Err(AgentApiError::InvalidInput(format!(
            "{operation} currently supports lmm models only; fit is {}",
            cached.model_kind.as_str()
        )))
    }
}

fn parse_ddf_method(method: &str) -> Result<DdfMethod, AgentApiError> {
    match method.to_lowercase().as_str() {
        "satterthwaite" | "sat" => Ok(DdfMethod::Satterthwaite),
        "kenward-roger" | "kenward_roger" | "kr" => Ok(DdfMethod::KenwardRoger),
        other => Err(AgentApiError::InvalidInput(format!(
            "unknown ddf_method '{other}' (use satterthwaite or kenward-roger)"
        ))),
    }
}

fn parse_anova_type(anova_type: &str) -> Result<AnovaType, AgentApiError> {
    match anova_type.to_uppercase().as_str() {
        "III" | "3" | "TYPE3" | "TYPE III" => Ok(AnovaType::Type3),
        "II" | "2" | "TYPE2" | "TYPE II" => Ok(AnovaType::Type2),
        "I" | "1" | "TYPE1" | "TYPE I" => Ok(AnovaType::Type1),
        other => Err(AgentApiError::InvalidInput(format!(
            "unknown anova_type '{other}' (use I, II, or III)"
        ))),
    }
}

fn parse_boot_method(method: &str) -> Result<BootLmerMethod, AgentApiError> {
    match method.to_lowercase().as_str() {
        "parametric" | "param" => Ok(BootLmerMethod::Parametric),
        "residual" | "res" => Ok(BootLmerMethod::Residual),
        other => Err(AgentApiError::InvalidInput(format!(
            "unknown boot method '{other}' (use parametric or residual)"
        ))),
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
