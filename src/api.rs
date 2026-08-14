//! Protocol-neutral agent API around `lme-rs`.
//!
//! This module owns statistical operations and session semantics without depending on
//! MCP transport types. Protocol adapters (MCP today, SCP or others later) should map
//! their wire-level requests and errors onto this API instead of calling `lme-rs`
//! directly.

use std::sync::Arc;

use lme_rs::family::{Family, Link};
use lme_rs::{
    boot_lmer, glmer_with_link, lmer, nlmer_with_options, AnovaType, BootLmerMethod, DdfMethod,
    FixedEffectsAnovaResult, NlmerOptions, NlmmStart,
};
use uuid::Uuid;

use crate::data::load_csv;
use crate::dto::{
    AnovaRequest, AnovaRow, AnovaSummary, BootConfintRow, BootSummary, BootstrapRequest,
    FitListEntry, FitListSummary, FitLmmRequest, FitModelRequest, FitSummary, ForgetFitResult,
    ModelKind,
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

    /// Fit an LMM, GLMM, or NLMM through the protocol-neutral semantic API.
    pub fn fit_model(&self, request: FitModelRequest) -> Result<FitSummary, AgentApiError> {
        validate_fit_input(&request.formula, &request.data_path)?;

        match request.model_kind {
            ModelKind::Lmm => self.fit_model_lmm(request),
            ModelKind::Glmm => self.fit_model_glmm(request),
            ModelKind::Nlmm => self.fit_model_nlmm(request),
            ModelKind::Lm => Err(AgentApiError::InvalidInput(
                "fit_model does not expose lm yet; use model_kind 'lmm', 'glmm', or 'nlmm'"
                    .to_string(),
            )),
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

    pub fn fit_summary(&self, fit_id: &str) -> Result<FitSummary, AgentApiError> {
        let cached = self.cached_fit(fit_id)?;
        Ok(fit_summary_from_cached(fit_id, &cached))
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

    pub fn anova(&self, request: AnovaRequest) -> Result<AnovaSummary, AgentApiError> {
        let mut cached = self.cached_fit(&request.fit_id)?;
        ensure_lmm_operation(&cached, "anova")?;
        let ddf = parse_ddf_method(&request.ddf_method)?;
        let anova_type = parse_anova_type(&request.anova_type)?;

        let (_, df) = load_csv(cached.data_path.to_string_lossy().as_ref())
            .map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
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

    pub fn bootstrap(&self, request: BootstrapRequest) -> Result<BootSummary, AgentApiError> {
        if request.nsim == 0 {
            return Err(AgentApiError::InvalidInput(
                "nsim must be greater than zero".to_string(),
            ));
        }
        if !request.level.is_finite() || request.level <= 0.0 || request.level >= 1.0 {
            return Err(AgentApiError::InvalidInput(
                "level must be finite and strictly between 0 and 1".to_string(),
            ));
        }
        if request.n_jobs == Some(0) {
            return Err(AgentApiError::InvalidInput(
                "n_jobs must be greater than zero when provided".to_string(),
            ));
        }

        let cached = self.cached_fit(&request.fit_id)?;
        ensure_lmm_operation(&cached, "bootstrap")?;
        let (_, df) = load_csv(cached.data_path.to_string_lossy().as_ref())
            .map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
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
        let ci = boot
            .confint_percentile(request.level)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;

        Ok(BootSummary {
            fit_id: request.fit_id,
            method: request.method,
            nsim: request.nsim,
            prop_converged: boot.prop_converged,
            level: request.level,
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
