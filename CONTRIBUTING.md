# Contributing to lme-rs-mcp

## Scope

This repository contains **only** the agent/protocol adapter around `lme-rs`:

- Rust binary `lme-rs-mcp` (stdio server)
- Protocol-neutral statistical/session API in `src/api.rs`
- Typed agent DTOs in `src/dto.rs`
- MCP transport adapter in `src/mcp.rs`
- Session cache and CSV loading

Statistical algorithms live in [**lme-rs**](https://github.com/x4g4p3x/lme-rs). Change numerics there, release a new `lme-rs` crate version, then bump the dependency here.

## Dependency model

**End users** install only this repo (or `cargo install lme-rs-mcp`). **`lme-rs`** is pulled from **crates.io** — no sibling checkout.

```toml
# Cargo.toml
lme-rs = "0.2.1"
```

**Contributors** testing unreleased `lme-rs` APIs on a sibling clone:

```powershell
task patch:local
```

That copies `.cargo/config.toml.example` → `.cargo/config.toml` with `[patch.crates-io]`. Remove `.cargo/config.toml` to build against crates.io only.

See [RELEASING.md](RELEASING.md) for publish order (**`lme-rs` first**, then refresh `Cargo.lock` here).

## One-time setup

```powershell
mise install
task setup            # hooks
cargo build
```

Use `task patch:local` only when co-developing with a sibling `lme-rs` checkout.

## Day-to-day commands

| Command | Purpose |
|:--------|:--------|
| `task lint` | `rustfmt --check` + clippy |
| `task test` | `cargo test --locked` |
| `task ci` | lint + check + test |
| `task preflight` | pre-push: lint + check + `cargo audit` |
| `task build:release` | release binary for MCP clients |

## Project layout

| Path | Purpose |
|:-----|:--------|
| `src/main.rs` | Tokio entry + stdio transport startup |
| `src/api.rs` | Protocol-neutral `LmeAgentApi` operations |
| `src/mcp.rs` | Thin MCP tool router + error mapping |
| `src/session.rs` | In-memory model cache keyed by `fit_id` |
| `src/data.rs` | CSV load + `LME_MCP_DATA_ROOT` |
| `src/dto.rs` | Shared request/response DTOs and JSON schemas |
| `tests/data/sleepstudy.csv` | Vendored fixture |
| `.cargo/config.toml.example` | Optional `[patch.crates-io]` for co-dev |

## Adding a capability

1. Decide whether the capability belongs behind an existing semantic operation in [AGENT_API.md](AGENT_API.md) before adding another protocol tool.
2. Put statistical/session behavior in `LmeAgentApi`, not in `LmeMcpServer`.
3. Define or extend protocol-neutral request/response DTOs in `src/dto.rs` using `serde` and `rmcp::schemars::JsonSchema`.
4. Keep model-family constraints explicit (`ModelKind`) when an operation is LMM/GLMM/NLMM-specific.
5. Add the MCP mapping in `src/mcp.rs`; return typed `rmcp::Json<T>` so `structuredContent` and `outputSchema` stay available.
6. Document the behavior in **AGENT_API.md**, **GUIDE.md**, and **README.md** as appropriate.
7. Add integration coverage for the core API and protocol shape.

Do not duplicate `lme-rs` numerical logic in this repository. A future SCP adapter should call the same `LmeAgentApi` methods as MCP.

## Testing

```powershell
task ci
```

## License

MIT — same as lme-rs.
