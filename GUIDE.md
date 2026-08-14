# lme-rs-mcp User Guide

This guide covers installing, configuring, and using the **lme-rs-mcp** Model Context Protocol server.

For the underlying statistics engine, see the [lme-rs GUIDE](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md). For the target agent-facing surface and MCP/SCP architecture, see [AGENT_API.md](AGENT_API.md).

## Table of contents

- [What this server is](#what-this-server-is)
- [Architecture](#architecture)
- [Installation](#installation)
- [Client configuration](#client-configuration)
- [Session model](#session-model)
- [Data paths and security](#data-paths-and-security)
- [Tool reference](#tool-reference)
- [Worked example: sleepstudy](#worked-example-sleepstudy)
- [Agent workflow tips](#agent-workflow-tips)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)

## What this server is

**lme-rs-mcp** is a thin MCP transport adapter around a protocol-neutral agent API for the [**lme-rs**](https://github.com/x4g4p3x/lme-rs) Rust crate. It does **not** reimplement mixed models; it:

1. Receives typed MCP requests
2. Delegates statistical/session operations to `LmeAgentApi`
3. Reads local CSV data and fits/analyzes models via `lme-rs`
4. Returns typed MCP `structuredContent` with generated `outputSchema`

Agents never send full datasets in tool arguments — only paths and formulas. That keeps payloads small and matches how local MCP servers usually work.

`rmcp::Json<T>` also emits serialized text content for backwards compatibility, so existing MCP clients can continue to consume the results even if they do not yet use `structuredContent` directly.

### What it is not

- Not a hosted statistics API
- Not a replacement for R `lme4` validation on publication-critical work
- Not a general dataframe or plotting service

## Architecture

```text
┌─────────────┐    stdio MCP     ┌────────────────┐    typed calls    ┌──────────────┐
│ MCP client  │ ◄──────────────► │ LmeMcpServer   │ ───────────────► │ LmeAgentApi  │
│ (Cursor, …) │ structured JSON  │ transport only │                  │ stats/session│
└─────────────┘                  └────────────────┘                  └──────┬───────┘
                                                                          │
                                                                    direct Rust calls
                                                                          │
                                                                          ▼
                                                                    ┌─────────────┐
                                                                    │ lme-rs      │
                                                                    │ + Polars    │
                                                                    └──────┬──────┘
                                                                           │
                                                                           ▼
                                                                    FitSession
                                                                    fit_id → LmeFit
```

| Component | Role |
|:----------|:-----|
| **`lme-rs-mcp` binary** | Starts the stdio MCP service |
| **`LmeMcpServer`** | Thin MCP transport + error mapping via `rmcp` macros |
| **`LmeAgentApi`** | Protocol-neutral statistical operations and session semantics |
| **typed DTOs** | Shared request/response contract with generated JSON schemas |
| **`FitSession`** | `HashMap<fit_id, CachedFit>` for the process lifetime |
| **`load_csv`** | Resolves path, optional `LME_MCP_DATA_ROOT` guard, parses CSV |

The dependency direction is intentional: protocol adapters depend on `LmeAgentApi`, not the other way around. A future Science Context Protocol adapter can therefore reuse the same core without importing MCP transport code.

**Transport:** stdio only (stdin/stdout). The server process runs until the MCP client disconnects.

**SDK:** [rmcp](https://github.com/modelcontextprotocol/rust-sdk) 2.x with `#[tool_router(server_handler)]`.

## Installation

### Prerequisites

- Rust stable (`rustup default stable`)
- Network access for **crates.io** (pulls `lme-rs` automatically)

You do **not** need a local `lme-rs` git clone unless you are co-developing unreleased library APIs — see [CONTRIBUTING.md](CONTRIBUTING.md#dependency-model).

### Build

```powershell
git clone https://github.com/x4g4p3x/lme-rs-mcp.git
cd lme-rs-mcp
cargo build --release --locked
```

Or, when published:

```powershell
cargo install lme-rs-mcp --locked
```

Debug build (faster compile, slower fits):

```powershell
cargo run
```

### Verify

```powershell
task test
# or: cargo test --locked
```

Uses vendored `tests/data/sleepstudy.csv` (no sibling `lme-rs` tree required).

## Client configuration

### Cursor

1. Build the release binary (see above).
2. Open **Cursor Settings → MCP** (or edit your user `mcp.json`).
3. Add a server entry:

```json
{
  "mcpServers": {
    "lme-rs": {
      "command": "C:\\Users\\x4g4p\\CascadeProjects\\lme-rs-mcp\\target\\release\\lme-rs-mcp.exe",
      "env": {
        "LME_MCP_DATA_ROOT": "C:\\path\\to\\your\\csv\\data"
      }
    }
  }
}
```

4. Restart MCP or reload the window.
5. Confirm the server shows **connected** and lists six tools.

### Environment variables

| Variable | Required | Description |
|:---------|:---------|:------------|
| `LME_MCP_DATA_ROOT` | No | If set, relative `data_path` values resolve under this directory, and every loaded file must canonicalize inside it. Recommended for agent use. |

No other configuration files are read today.

### Working directory

Relative `data_path` values resolve against the **server process cwd** when `LME_MCP_DATA_ROOT` is unset. When the root **is** set, use a filename (e.g. `sleepstudy.csv`) or a path under that directory.

## Session model

Fits are **stateful inside one server process**:

1. **`lme_fit`** stores a full `LmeFit` in memory and returns a UUID **`fit_id`**.
2. **`lme_anova`**, **`lme_boot`**, and **`lme_fit_summary`** take that `fit_id`.
3. **`lme_forget_fit`** removes one entry; **`lme_list_fits`** lists all cached ids.

| Property | Behavior |
|:---------|:---------|
| **Lifetime** | Until the MCP server process exits or the fit is forgotten |
| **Persistence** | None — restarting Cursor/MCP clears all fits |
| **Concurrency** | One process; tools may run in parallel per `rmcp`/client rules |
| **Updates** | Refitting the same formula creates a **new** `fit_id` |

There is no TTL yet. Long sessions with many large fits will grow memory use.

The session stores the unified `lme-rs 0.2.1` `LmeFit` together with protocol-neutral `ModelKind` metadata. Because upstream uses the same fit representation for LMM/GLMM/NLMM, future model families can reuse this cache without introducing another concrete-fit enum; see [AGENT_API.md](AGENT_API.md).

## Data paths and security

### CSV only

`data_path` must point to a **`.csv`** file. Parquet and in-memory tables are not supported in 0.1.0.

### Path allowlist and resolution

When `LME_MCP_DATA_ROOT` is set:

- **Relative** `data_path` values (e.g. `sleepstudy.csv`) are resolved as `{LME_MCP_DATA_ROOT}/{data_path}`.
- **Absolute** paths must still canonicalize to a location **under** the root.
- Both the root and the data file must exist and be readable.

When unset, relative paths resolve from the **server process cwd**; any readable `.csv` is allowed.

### Agent safety

- Treat tool output as **numeric summaries**, not automated statistical conclusions.
- Do not point the server at sensitive paths without `LME_MCP_DATA_ROOT`.
- Bootstrap and ANOVA can be **CPU-heavy**; use reasonable `nsim` (e.g. 200–1000, not 10000) unless the expensive computation is intentional.

## Tool reference

All tools return typed `rmcp::Json<T>` values. MCP clients receive both **`structuredContent`** and an **`outputSchema`** generated from the response DTO. `rmcp` also emits a serialized text representation for backwards compatibility.

### `lme_fit`

Fit a Gaussian linear mixed model (`lmer`).

**Parameters**

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `formula` | string | *required* | Wilkinson formula, e.g. `Reaction ~ Days + (1 \| Subject)` |
| `data_path` | string | *required* | Path to CSV on the server host |
| `reml` | boolean | `true` | `true` = REML, `false` = ML |

**Returns:** `FitSummary` including `fit_id`, `model_kind`, optional `reml` / GLMM `family` / `link` metadata, coefficients, SEs, σ², AIC/BIC, and convergence flag.

### `lme_list_fits`

**Parameters:** none.

**Returns:** `FitListSummary` with `fits[]` entries containing `fit_id`, `model_kind`, `formula`, `data_path`, optional `reml` / `family` / `link`, and `num_obs`.

### `lme_fit_summary`

**Parameters**

| Field | Type | Description |
|:------|:-----|:------------|
| `fit_id` | string | Id from `lme_fit` |

**Returns:** Same `FitSummary` shape as `lme_fit`.

### `lme_forget_fit`

**Parameters**

| Field | Type | Description |
|:------|:-----|:------------|
| `fit_id` | string | Id to remove |

**Returns:** `ForgetFitResult` with `forgotten: <fit_id>` or an error if unknown.

### `lme_anova`

Fixed-effects ANOVA (Type I / II / III) with Satterthwaite or Kenward–Roger denominator df.

The core reloads the original CSV and applies `with_satterthwaite` or `with_kenward_roger` on the cached fit before computing the table.

**Parameters**

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `fit_id` | string | *required* | Cached fit |
| `ddf_method` | string | `"satterthwaite"` | `satterthwaite`, `sat`, `kenward-roger`, `kenward_roger`, `kr` |
| `anova_type` | string | `"III"` | `I`, `II`, `III` (also `1`/`2`/`3`, `TYPE I`, …) |

**Returns:** `AnovaSummary` with `rows[]` of `term`, `num_df`, `den_df`, `f_value`, `p_value`.

### `lme_boot`

Parametric or residual bootstrap refits (`bootMer`-style) with percentile CIs.

**Parameters**

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `fit_id` | string | *required* | Cached Gaussian LMM |
| `nsim` | integer | `200` | Bootstrap replicates; must be > 0 |
| `method` | string | `"parametric"` | `parametric` / `param` or `residual` / `res` |
| `reml` | boolean | `true` | REML/ML for each refit |
| `seed` | integer | null | Optional RNG seed (reproducible across `n_jobs`) |
| `n_jobs` | integer | null | Parallel workers; when provided must be > 0 |
| `level` | number | `0.95` | CI level, strictly between 0 and 1 |

**Returns:** `BootSummary` with `prop_converged`, `intervals[]` (`name`, `estimate`, `lower`, `upper`).

See [lme-rs bootstrap docs](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md#bootstrap-refits-boot_lmer) for method semantics vs R `bootMer`.

## Worked example: sleepstudy

Assume `LME_MCP_DATA_ROOT` points at `lme-rs/tests/data`.

### Step 1 — Fit

Tool: `lme_fit`

```json
{
  "formula": "Reaction ~ Days + (1 | Subject)",
  "data_path": "sleepstudy.csv",
  "reml": true
}
```

Example response (abbreviated):

```json
{
  "fit_id": "a1b2c3d4-....",
  "model_kind": "lmm",
  "formula": "Reaction ~ Days + (1 | Subject)",
  "reml": true,
  "family": null,
  "link": null,
  "converged": true,
  "fixed_names": ["(Intercept)", "Days"],
  "coefficients": [251.4, 10.46],
  "sigma2": 1943.2
}
```

### Step 2 — ANOVA

Tool: `lme_anova`

```json
{
  "fit_id": "a1b2c3d4-....",
  "ddf_method": "satterthwaite",
  "anova_type": "III"
}
```

### Step 3 — Bootstrap CI for `Days`

Tool: `lme_boot`

```json
{
  "fit_id": "a1b2c3d4-....",
  "nsim": 100,
  "method": "parametric",
  "seed": 42,
  "level": 0.95
}
```

### Step 4 — Cleanup

Tool: `lme_forget_fit`

```json
{ "fit_id": "a1b2c3d4-...." }
```

## Agent workflow tips

1. **Always call `lme_fit` first** and keep the returned `fit_id` for follow-up tools.
2. **Pass `formula` exactly** as used in `lme_fit` (bootstrap reloads data from the cached path).
3. **Prefer `LME_MCP_DATA_ROOT`** so agents only need filenames, not full paths.
4. **Check `converged`** in fit summaries before interpreting ANOVA or bootstrap output.
5. **Use `lme_list_fits`** if the conversation lost track of ids in the same session.
6. For Wald CIs with KR/Satterthwaite dfs, use the library directly — MCP does not expose `confint` yet.

## Limitations

| Topic | 0.1.0 status |
|:------|:-------------|
| GLMM / `glmer` | Not exposed |
| NLMM | Not exposed |
| Prediction | Not exposed |
| `cv_grouped` | Not exposed |
| Parquet / Arrow | CSV only |
| Nested LRT (`anova` two models) | Not exposed |
| Session persistence / disk cache | Not implemented |
| crates.io `lme-rs` dep | `0.2.1`; optional `[patch]` for unreleased co-dev |

The protocol-neutral core is the extension point for these capabilities. The dependency/session upgrade to `lme-rs 0.2.1` is complete; the next semantic step is `fit_model` with GLMM support. See [AGENT_API.md](AGENT_API.md).

Statistical scope matches [lme-rs USABILITY.md](https://github.com/x4g4p3x/lme-rs/blob/master/USABILITY.md) green rows for LMM + ANOVA + bootstrap.

## Troubleshooting

### MCP server fails to start

- Run `cargo build --release` in `lme-rs-mcp`.
- Use the **full path** to the binary in MCP config.
- On Windows, use `lme-rs-mcp.exe` in `command`.

### `data_path not found`

- Confirm the file exists on the **same machine** as the server (not the agent’s remote environment).
- Use an absolute path or set `LME_MCP_DATA_ROOT` correctly.
- Remember: only **`.csv`** extensions are accepted.

### `outside LME_MCP_DATA_ROOT`

- Move the CSV under the configured root, or widen `LME_MCP_DATA_ROOT`.
- Use canonical paths (no `..` escape tricks — canonicalization is applied).

### `unknown fit_id`

- The server process may have restarted (session cleared).
- Call `lme_list_fits` or refit with `lme_fit`.

### Fit or bootstrap is slow

- Bootstrap cost scales with `nsim` and model size.
- Set `n_jobs` explicitly; large `nsim` on big datasets can take minutes.
- First compile already happened; slowness is usually refitting, not Rust compile.

### stdout pollution

MCP uses stdout for the protocol. Do not wrap the binary in scripts that `echo` to stdout. Log to **stderr** only if you add logging later.

## Related links

- [Agent-facing API](AGENT_API.md)
- [lme-rs README](https://github.com/x4g4p3x/lme-rs)
- [lme-rs GUIDE — Bootstrap](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md#bootstrap-refits-boot_lmer)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [rmcp Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
