# Contributing to lme-rs-mcp

## Scope

This repository contains **only** the MCP transport adapter around `lme-rs-agent`:

- Rust binary `lme-rs-mcp` (stdio server)
- MCP transport adapter in `src/mcp.rs`

Protocol-neutral orchestration, DTOs, validation, CSV loading, and fit sessions live in [**lme-rs-agent**](https://github.com/x4g4p3x/lme-rs-agent). Statistical algorithms live in [**lme-rs**](https://github.com/x4g4p3x/lme-rs).

## Dependency model

Source builds and published installs pull both `lme-rs-agent` and `lme-rs` from crates.io. Sibling checkouts are optional for co-development.

```toml
# Cargo.toml
lme-rs-agent = "=0.1.0"
```

**Contributors** testing unreleased agent or engine APIs on sibling clones:

```powershell
task patch:local
```

That copies `.cargo/config.toml.example` → `.cargo/config.toml` with `[patch.crates-io]`. Remove `.cargo/config.toml` to build against crates.io only.

See [RELEASING.md](RELEASING.md) for publish order (`lme-rs`, then `lme-rs-agent`, then the transport adapters).

## One-time setup

```powershell
mise install
task setup            # hooks
cargo build
```

Use `task patch:local` only when co-developing with sibling agent and engine checkouts.

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
| `src/mcp.rs` | Thin MCP tool router + error mapping |
| `src/tasks.rs` | Negotiation, background task lifecycle, and cancellation |
| `src/lib.rs` | Re-export of shared agent types plus `LmeMcpServer` |
| `tests/data/sleepstudy.csv` | Vendored fixture |
| `.cargo/config.toml.example` | Optional `[patch.crates-io]` for co-dev |

## Adding a capability

1. Decide whether the capability belongs behind an existing semantic operation in [AGENT_API.md](AGENT_API.md) before adding another protocol tool.
2. Put statistical/session behavior in `lme-rs-agent::LmeAgentApi`, not in `LmeMcpServer`.
3. Define or extend protocol-neutral DTOs in `lme-rs-agent`; keep MCP wire mapping here.
4. Keep model-family constraints explicit (`ModelKind`) when an operation is LMM/GLMM/NLMM-specific.
5. Add the MCP mapping in `src/mcp.rs`; return typed `rmcp::Json<T>` so `structuredContent` and `outputSchema` stay available.
6. Document the behavior in **AGENT_API.md**, **GUIDE.md**, and **README.md** as appropriate.
7. Add integration coverage for the core API and protocol shape.

Do not duplicate `lme-rs` numerical logic or `lme-rs-agent` orchestration in this repository. `lme-rs-scp` calls the same `LmeAgentApi` methods as MCP.

## Testing

```powershell
task ci
```

## License

MIT — same as lme-rs.
