use lme_rs::{boot_lmer, lmer, BootLmerMethod};
use lme_rs_mcp::{
    load_csv, AgentApiError, AnovaRequest, CompareModelsRequest, ConfidenceIntervalMethod,
    ConfidenceIntervalsRequest, CrossValidateRequest, FitListSummary, FitLmmRequest,
    FitModelRequest, LmeAgentApi, LmeMcpServer, ModelBootstrapRequest, ModelKind, PredictRequest,
    PredictionMode, PredictionScale,
};
use polars::prelude::*;
use rmcp::{handler::server::tool::IntoCallToolResult, model::CallToolResult, ErrorData, Json};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::PathBuf;

fn data_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(filename)
}

fn sleepstudy_path() -> PathBuf {
    data_path("sleepstudy.csv")
}

fn poisson_glmm_path() -> PathBuf {
    data_path("poisson_glmm.csv")
}

fn nlmm_micmen_path() -> PathBuf {
    data_path("nlmm_micmen.csv")
}

#[test]
fn load_csv_resolves_relative_path_under_data_root() {
    let root = sleepstudy_path().parent().unwrap().to_path_buf();
    std::env::set_var("LME_MCP_DATA_ROOT", &root);
    let (path, df) = load_csv("sleepstudy.csv").expect("load under root");
    assert!(path.ends_with("sleepstudy.csv"));
    assert!(df.height() > 0);
    std::env::remove_var("LME_MCP_DATA_ROOT");
}

#[test]
fn protocol_neutral_api_fit_lifecycle() {
    let api = LmeAgentApi::new();
    let fit = api
        .fit_lmm(FitLmmRequest {
            formula: "Reaction ~ Days + (1 | Subject)".to_string(),
            data_path: sleepstudy_path().display().to_string(),
            reml: true,
        })
        .expect("fit through agent API");

    assert_eq!(fit.model_kind, ModelKind::Lmm);
    assert_eq!(fit.reml, Some(true));
    assert_eq!(fit.family, None);
    assert_eq!(fit.link, None);
    assert_eq!(fit.n_agq, None);
    assert_eq!(fit.start, None);
    assert_eq!(fit.num_obs, 180);
    assert!(!fit.coefficients.is_empty());

    let listed = api.list_fits();
    assert_eq!(listed.fits.len(), 1);
    assert_eq!(listed.fits[0].fit_id, fit.fit_id);
    assert_eq!(listed.fits[0].model_kind, ModelKind::Lmm);
    assert_eq!(listed.fits[0].reml, Some(true));
    assert_eq!(listed.fits[0].n_agq, None);
    assert_eq!(listed.fits[0].start, None);

    let summary = api.fit_summary(&fit.fit_id).expect("cached summary");
    assert_eq!(summary.formula, fit.formula);
    assert_eq!(summary.model_kind, ModelKind::Lmm);

    let forgotten = api.forget_fit(&fit.fit_id).expect("forget fit");
    assert_eq!(forgotten.forgotten, fit.fit_id);
    assert!(api.list_fits().fits.is_empty());
}

#[test]
fn semantic_lm_fit_predict_diagnostics_and_lifecycle_aliases() {
    let api = LmeAgentApi::new();
    let fit = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Lm,
            formula: "Reaction ~ Days".to_string(),
            data_path: sleepstudy_path().display().to_string(),
            reml: None,
            family: None,
            link: None,
            n_agq: None,
            start: None,
        })
        .expect("fit LM through semantic API");

    assert_eq!(fit.model_kind, ModelKind::Lm);
    assert_eq!(fit.num_obs, 180);
    assert_eq!(fit.fixed_names.len(), 2);
    assert_eq!(fit.coefficients.len(), 2);

    let listed = api.list_models();
    assert_eq!(listed.fits.len(), 1);
    assert_eq!(listed.fits[0].fit_id, fit.fit_id);

    let summary = api
        .model_summary(&fit.fit_id)
        .expect("semantic model summary");
    assert_eq!(summary.model_kind, ModelKind::Lm);

    let predictions = api
        .predict(PredictRequest {
            fit_id: fit.fit_id.clone(),
            data_path: None,
            mode: PredictionMode::Population,
            scale: PredictionScale::Response,
            allow_new_levels: false,
        })
        .expect("LM population predictions");
    assert_eq!(predictions.predictions.len(), 180);
    assert!(predictions
        .predictions
        .iter()
        .all(|value| value.is_finite()));

    let diagnostics = api.diagnostics(&fit.fit_id).expect("LM diagnostics");
    assert!(diagnostics.converged);
    assert!(diagnostics.finite_coefficients);
    assert!(diagnostics.finite_residuals);
    assert_eq!(diagnostics.num_obs, 180);

    let forgotten = api
        .forget_model(&fit.fit_id)
        .expect("semantic forget model");
    assert_eq!(forgotten.forgotten, fit.fit_id);
    assert!(api.list_models().fits.is_empty());
}

#[test]
fn protocol_neutral_fit_model_glmm_lifecycle() {
    let api = LmeAgentApi::new();
    let fit = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Glmm,
            formula: "y ~ x + (1 | group)".to_string(),
            data_path: poisson_glmm_path().display().to_string(),
            reml: None,
            family: Some("Poisson".to_string()),
            link: None,
            n_agq: Some(1),
            start: None,
        })
        .expect("fit Poisson GLMM through semantic API");

    assert_eq!(fit.model_kind, ModelKind::Glmm);
    assert_eq!(fit.reml, None);
    assert_eq!(fit.family.as_deref(), Some("poisson"));
    assert_eq!(fit.link.as_deref(), Some("log"));
    assert_eq!(fit.n_agq, Some(1));
    assert_eq!(fit.start, None);
    assert_eq!(fit.num_obs, 36);
    assert!(!fit.coefficients.is_empty());

    let predictions = api
        .predict(PredictRequest {
            fit_id: fit.fit_id.clone(),
            data_path: None,
            mode: PredictionMode::Population,
            scale: PredictionScale::Response,
            allow_new_levels: false,
        })
        .expect("GLMM response-scale prediction");
    assert_eq!(predictions.predictions.len(), 36);
    assert!(predictions
        .predictions
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0));

    let listed = api.list_fits();
    assert_eq!(listed.fits.len(), 1);
    assert_eq!(listed.fits[0].model_kind, ModelKind::Glmm);
    assert_eq!(listed.fits[0].family.as_deref(), Some("poisson"));
    assert_eq!(listed.fits[0].link.as_deref(), Some("log"));
    assert_eq!(listed.fits[0].n_agq, Some(1));
    assert_eq!(listed.fits[0].start, None);

    let summary = api.fit_summary(&fit.fit_id).expect("cached GLMM summary");
    assert_eq!(summary.model_kind, ModelKind::Glmm);
    assert_eq!(summary.family.as_deref(), Some("poisson"));

    let error = api
        .anova(AnovaRequest {
            fit_id: fit.fit_id.clone(),
            ddf_method: "satterthwaite".to_string(),
            anova_type: "III".to_string(),
        })
        .expect_err("LMM ANOVA must reject GLMMs");
    assert!(error.to_string().contains("lmm models only"));

    api.forget_fit(&fit.fit_id).expect("forget GLMM");
    assert!(api.list_fits().fits.is_empty());
}

#[test]
fn semantic_glmm_bootstrap_rejects_residual_method() {
    let api = LmeAgentApi::new();
    let fit = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Glmm,
            formula: "y ~ x + (1 | group)".to_string(),
            data_path: poisson_glmm_path().display().to_string(),
            reml: None,
            family: Some("poisson".to_string()),
            link: None,
            n_agq: Some(1),
            start: None,
        })
        .expect("fit GLMM");

    let error = api
        .bootstrap_model(ModelBootstrapRequest {
            fit_id: fit.fit_id,
            nsim: 2,
            method: "residual".to_string(),
            reml: None,
            seed: Some(1),
            n_jobs: Some(1),
            level: 0.95,
        })
        .expect_err("GLMM residual bootstrap must be rejected");
    assert!(error.to_string().contains("parametric"));
}

#[test]
fn protocol_neutral_fit_model_nlmm_lifecycle() {
    let api = LmeAgentApi::new();
    let start = BTreeMap::from([("K".to_string(), 1.5), ("Vmax".to_string(), 10.0)]);
    let fit = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Nlmm,
            formula: "y ~ SSmicmen(x, Vmax, K) ~ Vmax|g".to_string(),
            data_path: nlmm_micmen_path().display().to_string(),
            reml: Some(false),
            family: None,
            link: None,
            n_agq: Some(1),
            start: Some(start.clone()),
        })
        .expect("fit Michaelis-Menten NLMM through semantic API");

    assert_eq!(fit.model_kind, ModelKind::Nlmm);
    assert_eq!(fit.reml, Some(false));
    assert_eq!(fit.family, None);
    assert_eq!(fit.link, None);
    assert_eq!(fit.n_agq, Some(1));
    assert_eq!(fit.start.as_ref(), Some(&start));
    assert_eq!(fit.num_obs, 40);
    assert_eq!(fit.coefficients.len(), 2);
    assert!(fit.coefficients.iter().all(|value| value.is_finite()));

    let predictions = api
        .predict(PredictRequest {
            fit_id: fit.fit_id.clone(),
            data_path: None,
            mode: PredictionMode::Population,
            scale: PredictionScale::Response,
            allow_new_levels: false,
        })
        .expect("NLMM population prediction");
    assert_eq!(predictions.predictions.len(), 40);
    assert!(predictions
        .predictions
        .iter()
        .all(|value| value.is_finite()));

    let listed = api.list_fits();
    assert_eq!(listed.fits.len(), 1);
    assert_eq!(listed.fits[0].model_kind, ModelKind::Nlmm);
    assert_eq!(listed.fits[0].reml, Some(false));
    assert_eq!(listed.fits[0].family, None);
    assert_eq!(listed.fits[0].link, None);
    assert_eq!(listed.fits[0].n_agq, Some(1));
    assert_eq!(listed.fits[0].start.as_ref(), Some(&start));

    let summary = api.fit_summary(&fit.fit_id).expect("cached NLMM summary");
    assert_eq!(summary.model_kind, ModelKind::Nlmm);
    assert_eq!(summary.start.as_ref(), Some(&start));

    let error = api
        .anova(AnovaRequest {
            fit_id: fit.fit_id.clone(),
            ddf_method: "satterthwaite".to_string(),
            anova_type: "III".to_string(),
        })
        .expect_err("LMM ANOVA must reject NLMMs");
    assert!(error.to_string().contains("lmm models only"));

    api.forget_fit(&fit.fit_id).expect("forget NLMM");
    assert!(api.list_fits().fits.is_empty());
}

#[test]
fn semantic_compare_confint_and_grouped_cv() {
    let api = LmeAgentApi::new();
    let reduced = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Lmm,
            formula: "Reaction ~ 1 + (1 | Subject)".to_string(),
            data_path: sleepstudy_path().display().to_string(),
            reml: Some(false),
            family: None,
            link: None,
            n_agq: None,
            start: None,
        })
        .expect("fit reduced ML LMM");
    let full = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Lmm,
            formula: "Reaction ~ Days + (1 | Subject)".to_string(),
            data_path: sleepstudy_path().display().to_string(),
            reml: Some(false),
            family: None,
            link: None,
            n_agq: None,
            start: None,
        })
        .expect("fit full ML LMM");

    let comparison = api
        .compare_models(CompareModelsRequest {
            fit_id_a: reduced.fit_id,
            fit_id_b: full.fit_id.clone(),
        })
        .expect("nested LRT");
    assert!(comparison.df > 0);
    assert!(comparison.chi_sq.is_finite());
    assert!(comparison.p_value.is_finite());

    let ci = api
        .confidence_intervals(ConfidenceIntervalsRequest {
            fit_id: full.fit_id.clone(),
            level: 0.95,
            method: ConfidenceIntervalMethod::Wald,
            parameters: Some(vec!["Days".to_string()]),
        })
        .expect("Wald confidence interval");
    assert_eq!(ci.intervals.len(), 1);
    assert_eq!(ci.intervals[0].name, "Days");
    assert!(ci.intervals[0].lower.is_finite());
    assert!(ci.intervals[0].upper.is_finite());

    let cv = api
        .cross_validate(CrossValidateRequest {
            fit_id: full.fit_id,
            group_col: "Subject".to_string(),
            n_splits: 3,
            seed: Some(7),
            n_jobs: Some(1),
        })
        .expect("grouped LMM cross-validation");
    assert_eq!(cv.oof_predictions.len(), 180);
    assert_eq!(cv.test_fold.len(), 180);
    assert_eq!(cv.folds.len(), 3);
    assert!(cv.rmse.is_finite());
    assert!(cv.mae.is_finite());
}

#[test]
fn fit_model_rejects_invalid_glmm_family_link_before_io() {
    let api = LmeAgentApi::new();
    let error = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Glmm,
            formula: "y ~ x + (1 | group)".to_string(),
            data_path: "does-not-exist.csv".to_string(),
            reml: None,
            family: Some("poisson".to_string()),
            link: Some("logit".to_string()),
            n_agq: Some(1),
            start: None,
        })
        .expect_err("invalid family/link pair must be rejected");

    match error {
        AgentApiError::InvalidInput(message) => {
            assert!(message.contains("not valid for family"));
        }
        other => panic!("expected InvalidInput, got {other}"),
    }
}

#[test]
fn fit_model_rejects_invalid_nlmm_start_before_io() {
    let api = LmeAgentApi::new();
    let start = BTreeMap::from([("Vmax".to_string(), f64::NAN)]);
    let error = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Nlmm,
            formula: "y ~ SSmicmen(x, Vmax, K) ~ Vmax|g".to_string(),
            data_path: "does-not-exist.csv".to_string(),
            reml: None,
            family: None,
            link: None,
            n_agq: Some(1),
            start: Some(start),
        })
        .expect_err("non-finite NLMM start must be rejected before IO");

    match error {
        AgentApiError::InvalidInput(message) => {
            assert!(message.contains("must be finite"));
        }
        other => panic!("expected InvalidInput, got {other}"),
    }
}

#[test]
fn fit_model_lm_rejects_mixed_model_options_before_io() {
    let api = LmeAgentApi::new();
    let error = api
        .fit_model(FitModelRequest {
            model_kind: ModelKind::Lm,
            formula: "y ~ x".to_string(),
            data_path: "does-not-exist.csv".to_string(),
            reml: Some(false),
            family: None,
            link: None,
            n_agq: None,
            start: None,
        })
        .expect_err("LM-only field validation should happen before IO");
    assert!(error.to_string().contains("do not apply"));
}

#[test]
fn model_kind_and_prediction_enums_have_stable_protocol_names() {
    assert_eq!(serde_json::to_value(ModelKind::Lm).unwrap(), "lm");
    assert_eq!(serde_json::to_value(ModelKind::Lmm).unwrap(), "lmm");
    assert_eq!(serde_json::to_value(ModelKind::Glmm).unwrap(), "glmm");
    assert_eq!(serde_json::to_value(ModelKind::Nlmm).unwrap(), "nlmm");
    assert_eq!(
        serde_json::to_value(PredictionMode::Population).unwrap(),
        "population"
    );
    assert_eq!(
        serde_json::to_value(PredictionScale::Response).unwrap(),
        "response"
    );
    assert_eq!(
        serde_json::to_value(ConfidenceIntervalMethod::Profile).unwrap(),
        "profile"
    );
}

#[test]
fn typed_responses_become_structured_mcp_content() {
    let result: Result<CallToolResult, ErrorData> =
        IntoCallToolResult::into_call_tool_result(Json(FitListSummary { fits: vec![] }));
    let result = result.expect("structured MCP result");

    assert!(result.structured_content.is_some());
    assert!(!result.content.is_empty());
}

#[test]
fn library_bootstrap_smoke() {
    let path = sleepstudy_path();
    let file = File::open(&path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let df = CsvReader::new(file).finish().expect("parse sleepstudy csv");
    let formula = "Reaction ~ Days + (1 | Subject)";
    let fit = lmer(formula, &df, true).expect("lmer");
    let boot = boot_lmer(
        formula,
        &df,
        &fit,
        8,
        BootLmerMethod::Parametric,
        true,
        Some(1),
        Some(1),
    )
    .expect("boot");
    assert_eq!(boot.nsim, 8);
    assert!(boot.prop_converged > 0.0);
    let _ = LmeMcpServer::new();
}
