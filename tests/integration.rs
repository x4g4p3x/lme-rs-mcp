use lme_rs::{boot_lmer, lmer, BootLmerMethod};
use lme_rs_mcp::{load_csv, LmeMcpServer};
use polars::prelude::*;
use std::fs::File;
use std::path::PathBuf;

fn sleepstudy_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("sleepstudy.csv")
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
