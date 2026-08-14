//! Protocol-neutral agent API around `lme-rs`.
//!
//! This module owns statistical operations and session semantics without depending on
//! MCP transport types. Protocol adapters (MCP today, SCP or others later) should map
//! their wire-level requests and errors onto this API instead of calling `lme-rs`
//! directly.

use std::sync::Arc;

use lme_rs::{boot_lmer, lmer, AnovaType, BootLmerMethod, DdfMethod, FixedEffectsAnovaResult};
use uuid::Uuid;

use crate::data::load_csv;
use crate::dto::{
    AnovaRequest, AnovaRow, AnovaSummary, BootConfintRow, BootSummary, BootstrapRequest,
    FitListEntry, FitListSummary, FitLmmRequest, FitSummary, ForgetFitResult,
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

    pub fn fit_lmm(&self, request: FitLmmRequest) -> Result<FitSummary, AgentApiError> {
        if request.formula.trim().is_empty() {
            return Err(AgentApiError::InvalidInput(
                "formula must not be empty".to_string(),
            ));
        }
        if request.data_path.trim().is_empty() {
            return Err(AgentApiError::InvalidInput(
                "data_path must not be empty".to_string(),
            ));
        }

        let (path, df) =
            load_csv(&request.data_path).map_err(|e| AgentApiError::InvalidInput(e.to_string()))?;
        let fit = lmer(&request.formula, &df, request.reml)
            .map_err(|e| AgentApiError::Computation(e.to_string()))?;
        let fit_id = Uuid::new_v4().to_string();
        let cached = CachedFit {
            formula: request.formula,
            data_path: path,
            reml: request.reml,
            fit,
        };
        let summary = fit_summary_from_cached(&fit_id, &cached);
        self.session.insert(fit_id, cached);
        Ok(summary)
    }

    pub fn list_fits(&self) -> FitListSummary {
        let fits = self
            .session
            .list()
            .into_iter()
            .map(|(fit_id, cached)| FitListEntry {
                fit_id,
                formula: cached.formula,
                data_path: cached.data_path.display().to_string(),
                reml: cached.reml,
                num_obs: cached.fit.num_obs,
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
        beta_se: fit.beta_se.as_ref().map(|se| se.to_vec()),
        sigma2: fit.sigma2,
        aic: fit.aic,
        bic: fit.bic,
        log_likelihood: fit.log_likelihood,
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
