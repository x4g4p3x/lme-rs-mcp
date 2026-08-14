# Changelog

All notable changes to **lme-rs-mcp** are documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`LmeAgentApi`** — protocol-neutral statistical/session API reusable by MCP and future scientific protocol adapters.
- **`fit_model` semantic fitting API/MCP tool** — supports Gaussian LMMs, GLMMs, and built-in formula-based NLMMs while retaining `lme_fit` as the backwards-compatible LMM entry point.
- **GLMM fitting** — binomial, Poisson, Gaussian, and Gamma families with canonical or explicit validated links and configurable `n_agq` (default `1`).
- **NLMM fitting** — upstream three-part `nlmer` formulas with built-in nonlinear means, optional named `start` values, REML/ML selection (ML by default), and configurable `n_agq` (default `1`).
- `FitModelRequest` with model-family-aware validation: inapplicable options are rejected rather than silently ignored.
- Typed request DTOs for fit, ANOVA, bootstrap, and fit-id operations.
- Typed response DTOs with JSON schemas, including fit lists and forget-fit results.
- **`ModelKind` session metadata** (`lm`, `lmm`, `glmm`, `nlmm`) plus optional REML/family/link/`n_agq` and NLMM start metadata in fit summaries and listings.
- Explicit model-family guards for current LMM-only ANOVA and bootstrap adapter operations.
- Poisson random-intercept GLMM integration fixture and protocol-neutral GLMM lifecycle coverage.
- Michaelis-Menten random-intercept NLMM integration fixture and protocol-neutral NLMM lifecycle coverage.
- **AGENT_API.md** — target semantic tool surface and migration path to the current `lme-rs 0.2.x` feature set.
- Integration coverage for the protocol-neutral fit lifecycle, stable model-kind names, and MCP structured output conversion.
- **GUIDE.md** — installation, architecture, session model, full tool reference, workflows, troubleshooting.
- **CONTRIBUTING.md** — development layout and tool-addition checklist.
- **AGENTS.md** — contributor hooks and preflight.
- **Taskfile.yml**, **mise.toml**, **lefthook.yml** — Rust-only dev workflow (`task ci`, `task preflight`).
- **`.github/workflows/ci.yml`** — fmt, clippy, check, test (ubuntu + windows).
- **`.cargo/config.toml.example`** — optional `[patch.crates-io]` for sibling `lme-rs` co-development.
- Vendored **`tests/data/sleepstudy.csv`** — tests no longer depend on a sibling checkout.
- Relative `data_path` resolution under `LME_MCP_DATA_ROOT` (filename-only workflow).

### Changed

- MCP handlers are now a thin transport layer over `LmeAgentApi`; statistical logic no longer lives in the MCP router.
- MCP tool results now return typed `rmcp::Json<T>` values, providing `structuredContent` and generated `outputSchema` instead of manually serialized JSON strings.
- Existing MCP tool names remain unchanged for compatibility while semantic names are added incrementally.
- `lme_fit` now delegates to the same semantic LMM implementation used by `fit_model`.
- GLMM family/link fields in session summaries are explicitly scoped to GLMM fits; NLMM metadata uses REML, `n_agq`, and optional user-supplied start values instead.
- **`lme-rs` dependency** — upgraded from crates.io `0.1.11` to **`0.2.1`** and regenerated `Cargo.lock` with Cargo.
- Fit-session records remain backed by the unified upstream `LmeFit` type, but now carry protocol-neutral model semantics so GLMM/NLMM support does not require a parallel concrete-fit enum.
- **RELEASING.md** — publish the matching `lme-rs` release before refreshing `Cargo.lock` and tagging MCP.
- **Committed `Cargo.lock`** for reproducible binary builds.

## [0.1.0] - 2026-07-14

### Added

- Initial MCP stdio server (`lme-rs-mcp` binary) using [rmcp](https://github.com/modelcontextprotocol/rust-sdk) 2.2.
- Tools: `lme_fit`, `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit`, `lme_anova`, `lme_boot`.
- In-memory `FitSession` keyed by UUID `fit_id`.
- CSV loader with optional `LME_MCP_DATA_ROOT` path allowlist.
- Integration smoke tests (bootstrap + CSV path resolution).
