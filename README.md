# lme-rs-mcp

[MCP](https://modelcontextprotocol.io/) server that exposes [**lme-rs**](https://github.com/x4g4p3x/lme-rs) mixed-effects modeling operations to agents and IDEs (Cursor, Claude Desktop, etc.).

The server is a **Rust binary** that calls `lme-rs` from **crates.io** (no Python runtime). It speaks MCP over **stdio**, caches fitted models in memory by `fit_id`, and returns typed **structured MCP results** that agents can parse reliably.

`lme-rs-mcp` is only an adapter. The statistical engine remains in `lme-rs`, and the MCP repository is never required to use the library directly.

## Architecture

Statistical/session semantics are kept separate from MCP transport:

```text
MCP client
    |
    | stdio MCP
    v
LmeMcpServer
    |
    | typed requests / responses
    v
LmeAgentApi
    |
    | direct Rust calls
    v
lme-rs + Polars
```

`LmeAgentApi` is intentionally protocol-neutral. A future Science Context Protocol (SCP) adapter can reuse the same operations and DTOs without depending on MCP code or duplicating statistical logic.

MCP tools return `rmcp::Json<T>`, so clients receive an MCP `outputSchema` plus `structuredContent` (with backwards-compatible text content supplied by `rmcp`).

The current adapter targets **`lme-rs 0.2.1`**. Cached records carry semantic `model_kind` metadata while using the unified upstream `LmeFit` representation, so LMM, GLMM, and NLMM fitting share one session model.

## What it does today

| Capability | MCP tools |
|:-----------|:----------|
| Semantic LMM/GLMM/NLMM fitting from CSV | `fit_model` |
| Backwards-compatible Gaussian LMM fitting | `lme_fit` |
| Session management | `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit` |
| Fixed-effects ANOVA (Type I–III, LMM only) | `lme_anova` |
| Bootstrap CIs (`bootMer`-style, LMM only) | `lme_boot` |

`fit_model` accepts `model_kind = "lmm"`, `"glmm"`, or `"nlmm"`.

GLMM families are **binomial**, **poisson**, **gaussian**, and **gamma**. The link is optional; when omitted the family’s canonical link is used. Explicit supported links are `logit`, `probit`, `cloglog`, `log`, `identity`, `inverse`, and `sqrt`, subject to family/link compatibility. GLMM `n_agq` defaults to `1`.

NLMM uses the upstream three-part `nlmer` formula syntax with built-in nonlinear means such as `SSlogis`, `SSasymp`, `SSfol`, `SSmicmen`, `SSgompertz`, `SSpower`, `SSfpl`, `SSbiexp`, and `SSweibull`. Named `start` values are optional; omitting them requests `lme-rs` self-start heuristics. NLMM defaults to ML (`reml = false`) and `n_agq = 1`.

LM, prediction, model comparison, confidence intervals, and cross-validation are not exposed yet. Fit/list/summary results include `model_kind`, applicable REML/family/link/`n_agq` metadata, and user-supplied NLMM starting values when present.

The target agent-facing surface and migration sequence are documented in **[AGENT_API.md](AGENT_API.md)**.

## Install

### From source

```powershell
git clone https://github.com/x4g4p3x/lme-rs-mcp.git
cd lme-rs-mcp
cargo build --release --locked
```

Binary: `target\release\lme-rs-mcp.exe` (Windows) or `target/release/lme-rs-mcp` (Unix).

### crates.io (when published)

```powershell
cargo install lme-rs-mcp --locked
```

Cargo pulls the `lme-rs` version pinned in this crate's `Cargo.toml` automatically.

## Configure Cursor

Add to **Cursor Settings → MCP** (or your `mcp.json`):

```json
{
  "mcpServers": {
    "lme-rs": {
      "command": "C:\\Users\\YOU\\.cargo\\bin\\lme-rs-mcp.exe",
      "env": {
        "LME_MCP_DATA_ROOT": "C:\\path\\to\\your\\csv\\data"
      }
    }
  }
}
```

Use a **release build path** or the `cargo install` binary. Point `LME_MCP_DATA_ROOT` at a folder of CSV files you want agents to read.

### Typical agent workflows

For a GLMM:

```json
{
  "model_kind": "glmm",
  "formula": "y ~ x + (1 | group)",
  "data_path": "poisson_data.csv",
  "family": "poisson",
  "n_agq": 1
}
```

The canonical `log` link is selected automatically. To request a noncanonical valid link, add `"link": "identity"` (or another family-compatible link).

For a built-in nonlinear mixed model:

```json
{
  "model_kind": "nlmm",
  "formula": "y ~ SSmicmen(x, Vmax, K) ~ Vmax|g",
  "data_path": "enzyme.csv",
  "start": {
    "Vmax": 10.0,
    "K": 1.5
  },
  "n_agq": 1
}
```

`start` can be omitted to use the upstream self-start heuristics. `reml` is also optional for NLMM and defaults to `false` (ML).

For existing LMM clients, `lme_fit` remains supported unchanged:

```json
{
  "formula": "Reaction ~ Days + (1 | Subject)",
  "data_path": "sleepstudy.csv",
  "reml": true
}
```

All fitting entry points return a `fit_id`. Use the existing session tools to inspect or forget that model. `lme_anova` and `lme_boot` currently accept LMM fits only and reject cached GLMM/NLMM fits explicitly.

With `LME_MCP_DATA_ROOT` set, relative `data_path` values resolve under that directory (filename-only is fine).

## Documentation

| Doc | Contents |
|:----|:---------|
| **[AGENT_API.md](AGENT_API.md)** | Protocol-neutral architecture, target semantic tool surface, MCP/SCP migration path |
| **[GUIDE.md](GUIDE.md)** | Session model, full current tool reference, workflows, security, troubleshooting |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | Development, optional patch workflow, Task/CI |
| **[AGENTS.md](AGENTS.md)** | Hooks and preflight for contributors |
| **[RELEASING.md](RELEASING.md)** | Publish order (`lme-rs` first, then MCP) |
| **[CHANGELOG.md](CHANGELOG.md)** | Version history |
| [lme-rs GUIDE](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md) | Underlying library: formulas, inference, bootstrap, prediction, NLMM, and more |

## Requirements

- **Rust** stable ([rustup](https://rustup.rs))
- **`lme-rs`** version pinned in `Cargo.toml` (installed via Cargo)
- MCP client that spawns stdio servers (Cursor, etc.)
- **CSV** data files readable on the machine running the server

## Status

The published **0.1.0** release used `lme-rs 0.1.11`. Current master development targets **`lme-rs 0.2.1`**, uses a model-kind-aware session contract, and exposes semantic LMM/GLMM/NLMM fitting through `fit_model` while retaining `lme_fit` for compatibility. The remaining fitting-family step is ordinary LM support; after that the highest-value semantic expansion is prediction, followed by model comparison/inference operations. Validate publication-critical results against R `lme4` / `lmerTest` as appropriate.

## License

MIT — see [LICENSE](LICENSE).
