# lme-rs-mcp

[MCP](https://modelcontextprotocol.io/) server that exposes [**lme-rs**](https://github.com/x4g4p3x/lme-rs) linear mixed-model tools to agents and IDEs (Cursor, Claude Desktop, etc.).

The server is a **Rust binary** that calls `lme-rs` from **crates.io** (no Python runtime). It speaks MCP over **stdio**, caches fitted models in memory by `fit_id`, and returns **JSON** summaries agents can parse.

**`lme-rs` is optional only in the sense that you don't clone it** — `cargo` fetches it as a dependency. You never need the MCP repo to use the library.

## What it does

| Capability | MCP tools |
|:-----------|:----------|
| Fit Gaussian LMMs from CSV | `lme_fit` |
| Session management | `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit` |
| Fixed-effects ANOVA (Type I–III) | `lme_anova` |
| Bootstrap CIs (`bootMer`-style) | `lme_boot` |

**Scope (0.1.0):** Gaussian LMMs only. Not GLMM, NLMM, prediction, or cross-validation yet.

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

Pulls **`lme-rs`** automatically at the version pinned in this crate's `Cargo.toml`.

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
| **[GUIDE.md](GUIDE.md)** | Architecture, session model, full tool reference, workflows, security, troubleshooting |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | Development, optional patch workflow, Task/CI |
| **[AGENTS.md](AGENTS.md)** | Hooks and preflight for contributors |
| **[RELEASING.md](RELEASING.md)** | Publish order (`lme-rs` first, then MCP) |
| **[CHANGELOG.md](CHANGELOG.md)** | Version history |
| [lme-rs GUIDE](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md) | Underlying library: formulas, REML/ML, bootstrap |

## Requirements

- **Rust** stable ([rustup](https://rustup.rs))
- **`lme-rs`** ≥ pinned version in `Cargo.toml` (installed via Cargo)
- MCP client that spawns stdio servers (Cursor, etc.)
- **CSV** data files readable on the machine running the server

## Status

**0.1.0** — requires **`lme-rs` 0.1.11** (bootstrap). Install via Cargo; no sibling `lme-rs` clone. See [RELEASING.md](RELEASING.md) for publish steps. Validate important results against R `lme4` / `lmerTest` before publication.

## License

MIT — see [LICENSE](LICENSE).
