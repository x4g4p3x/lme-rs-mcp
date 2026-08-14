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

The current adapter targets **`lme-rs 0.2.1`**. Cached records carry semantic `model_kind` metadata while using the unified upstream `LmeFit` representation, so future GLMM/NLMM fitting can reuse the same session layer.

## What it does today

| Capability | MCP tools |
|:-----------|:----------|
| Fit Gaussian LMMs from CSV | `lme_fit` |
| Session management | `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit` |
| Fixed-effects ANOVA (Type I–III) | `lme_anova` |
| Bootstrap CIs (`bootMer`-style) | `lme_boot` |

**Current MCP fitting scope:** Gaussian LMMs only. GLMM, NLMM, prediction, model comparison, confidence intervals, and cross-validation are not exposed yet. Fit/list/summary results already include model-kind metadata, optional REML, and family/link slots for the wider model surface.

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

### Typical agent workflow

1. `lme_fit` with `formula` + `data_path` → receive `fit_id`
2. `lme_anova` or `lme_boot` with that `fit_id`
3. `lme_forget_fit` when done (optional; session is in-process memory only)

Example `lme_fit` arguments:

```json
{
  "formula": "Reaction ~ Days + (1 | Subject)",
  "data_path": "sleepstudy.csv",
  "reml": true
}
```

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
| [lme-rs GUIDE](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md) | Underlying library: formulas, inference, bootstrap, prediction, and more |

## Requirements

- **Rust** stable ([rustup](https://rustup.rs))
- **`lme-rs`** version pinned in `Cargo.toml` (installed via Cargo)
- MCP client that spawns stdio servers (Cursor, etc.)
- **CSV** data files readable on the machine running the server

## Status

The published **0.1.0** release used `lme-rs 0.1.11`. Current master development targets **`lme-rs 0.2.1`** and has a model-kind-aware session contract. The next semantic step is a general `fit_model` operation, starting with GLMM while keeping `lme_fit` as the LMM compatibility entry point. Validate publication-critical results against R `lme4` / `lmerTest` as appropriate.

## License

MIT — see [LICENSE](LICENSE).
