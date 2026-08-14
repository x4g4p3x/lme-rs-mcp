# Changelog

All notable changes to **lme-rs-mcp** are documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`LmeAgentApi`** — protocol-neutral statistical/session API reusable by MCP and future scientific protocol adapters.
- **Complete semantic `fit_model` surface** for LM, LMM, GLMM, and built-in formula-based NLMM models.
- **GLMM fitting** — binomial, Poisson, Gaussian, and Gamma families with canonical or explicit validated links and configurable `n_agq`.
- **NLMM fitting** — upstream three-part `nlmer` formulas, built-in means, optional named starts, REML/ML, and configurable `n_agq`.
- **Semantic lifecycle tools** — `list_models`, `model_summary`, `forget_model`.
- **Semantic inference/workflow tools** — `anova`, `bootstrap`, `compare_models`, `confidence_intervals`, `predict`, `cross_validate`, and `diagnostics`.
- **Model-aware bootstrap** — LMM parametric/residual and GLMM parametric refits while retaining the legacy LMM-only `lme_boot` contract.
- **Likelihood-ratio model comparison** for compatible cached mixed models.
- **Wald/profile confidence intervals** with optional fixed-effect parameter selection; profile CIs follow the released LMM/GLMM engine support.
- **Population/conditional prediction** on link or response scale, with optional new CSV data and explicit new-level handling.
- **Grouped cross-validation** for LMM/GLMM with OOF predictions and global/per-fold metrics.
- **Diagnostics** for convergence, residuals, coefficient finiteness, dimensions, and statistical-output availability.
- Typed request/response DTOs and JSON schemas for the semantic API.
- **`ModelKind` session metadata** (`lm`, `lmm`, `glmm`, `nlmm`) plus applicable REML/family/link/`n_agq`/NLMM-start metadata.
- Poisson GLMM and Michaelis-Menten NLMM integration fixtures and lifecycle coverage.
- Integration coverage for LM fitting, lifecycle aliases, prediction, diagnostics, nested LMM comparison, confidence intervals, grouped CV, and semantic bootstrap validation.
- **AGENT_API.md**, **GUIDE.md**, **CONTRIBUTING.md**, **AGENTS.md**, **RELEASING.md** and Rust development/CI workflow files.
- Relative `data_path` resolution under `LME_MCP_DATA_ROOT`.

### Changed

- MCP handlers are a thin transport layer over `LmeAgentApi`; statistical/session semantics live in the protocol-neutral core.
- MCP tool results return typed `rmcp::Json<T>`, providing `structuredContent` and generated `outputSchema`.
- Existing `lme_*` MCP names remain available for compatibility while semantic names provide the preferred new surface.
- `lme_fit` delegates to the same semantic LMM implementation used by `fit_model`.
- Fit-session records remain backed by the unified upstream `LmeFit` while carrying protocol-neutral model semantics.
- The adapter targets the published crates.io **`lme-rs 0.2.1`** release rather than unreleased upstream revisions.
- Estimated marginal means are intentionally deferred until the post-`v0.2.1` upstream implementation is included in a published `lme-rs` release; statistical code is not duplicated in the MCP adapter.
- **Committed `Cargo.lock`** remains the reproducible binary dependency resolution.

## [0.1.0] - 2026-07-14

### Added

- Initial MCP stdio server (`lme-rs-mcp` binary) using [rmcp](https://github.com/modelcontextprotocol/rust-sdk) 2.2.
- Tools: `lme_fit`, `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit`, `lme_anova`, `lme_boot`.
- In-memory `FitSession` keyed by UUID `fit_id`.
- CSV loader with optional `LME_MCP_DATA_ROOT` path allowlist.
- Integration smoke tests (bootstrap + CSV path resolution).
