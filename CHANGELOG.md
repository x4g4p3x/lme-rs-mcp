# Changelog

All notable changes to **lme-rs-mcp** are documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **`LmeAgentApi`** — protocol-neutral statistical/session API reusable by MCP and future scientific protocol adapters.
- Typed request DTOs for fit, ANOVA, bootstrap, and fit-id operations.
- Typed response DTOs with JSON schemas, including fit lists and forget-fit results.
- **AGENT_API.md** — target semantic tool surface and migration path to the current `lme-rs 0.2.x` feature set.
- Integration coverage for the protocol-neutral fit lifecycle and MCP structured output conversion.
- **GUIDE.md** — installation, architecture, session model, full tool reference, sleepstudy workflow, troubleshooting.
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
- Existing MCP tool names remain unchanged for compatibility while the semantic API is developed.
- **`lme-rs` dependency** — crates.io pin `0.1.11` (bootstrap); simplified single-repo CI.
- **RELEASING.md** — publish `lme-rs` 0.1.11 before refreshing `Cargo.lock` and tagging MCP.
- **Committed `Cargo.lock`** for reproducible binary builds.

## [0.1.0] - 2026-07-14

### Added

- Initial MCP stdio server (`lme-rs-mcp` binary) using [rmcp](https://github.com/modelcontextprotocol/rust-sdk) 2.2.
- Tools: `lme_fit`, `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit`, `lme_anova`, `lme_boot`.
- In-memory `FitSession` keyed by UUID `fit_id`.
- CSV loader with optional `LME_MCP_DATA_ROOT` path allowlist.
- Integration smoke tests (bootstrap + CSV path resolution).
