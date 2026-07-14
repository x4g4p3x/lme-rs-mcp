# Contributing to lme-rs-mcp

## Scope

This repository contains **only** the MCP adapter:

- Rust binary `lme-rs-mcp` (stdio server)
- Tool handlers in `src/lib.rs`
- Session cache, CSV loading, JSON DTOs

Statistical algorithms live in [**lme-rs**](https://github.com/x4g4p3x/lme-rs). Change numerics there, release a new `lme-rs` crate version, then bump the dependency here.

## Dependency model

**End users** install only this repo (or `cargo install lme-rs-mcp`). **`lme-rs`** is pulled from **crates.io** — no sibling checkout.

```toml
# Cargo.toml
lme-rs = "0.1.11"   # bootstrap APIs; bump when lme-rs releases
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
| `src/main.rs` | Tokio entry, stdio transport |
| `src/lib.rs` | `LmeMcpServer` + `#[tool]` handlers |
| `src/session.rs` | In-memory `fit_id` cache |
| `src/data.rs` | CSV load + `LME_MCP_DATA_ROOT` |
| `src/dto.rs` | JSON response structs |
| `tests/data/sleepstudy.csv` | Vendored fixture |
| `.cargo/config.toml.example` | Optional `[patch.crates-io]` for co-dev |

## Adding a new tool

1. Define parameter struct with `serde::Deserialize` + `schemars::JsonSchema`.
2. Add a method on `LmeMcpServer` in `src/lib.rs` with `#[tool(description = "...")]`.
3. Return `Result<String, McpError>` via `tool_json(&dto)`.
4. Document in **GUIDE.md** and **README**.
5. Add integration coverage if user-facing.

## Testing

```powershell
task ci
```

## License

MIT — same as lme-rs.
