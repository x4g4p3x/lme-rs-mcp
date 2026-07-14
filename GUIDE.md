# lme-rs-mcp User Guide

This guide covers installing, configuring, and using the **lme-rs-mcp** Model Context Protocol server.

For the underlying statistics engine, see the [lme-rs GUIDE](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md).

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

**lme-rs-mcp** is a thin MCP adapter around the [**lme-rs**](https://github.com/x4g4p3x/lme-rs) Rust crate. It does **not** reimplement mixed models; it:

1. Reads a CSV from a **host path**
2. Fits or analyzes models via `lme-rs`
3. Returns **pretty-printed JSON** strings as MCP tool results

Agents never send full datasets in tool arguments — only paths and formulas. That keeps payloads small and matches how local MCP servers usually work.

### What it is not

- Not a hosted statistics API
- Not a replacement for R `lme4` validation on publication-critical work
- Not a general dataframe or plotting service

## Architecture

```text
┌─────────────┐    stdio MCP     ┌──────────────────┐    direct calls    ┌─────────┐
│ MCP client  │ ◄──────────────► │  lme-rs-mcp      │ ◄────────────────► │ lme-rs  │
│ (Cursor, …) │   JSON tools     │  (Tokio + rmcp)  │   LmeFit, etc.     │ + Polars│
└─────────────┘                  └──────────────────┘                    └─────────┘
                                        │
                                        ▼
                                 FitSession (in-memory
                                 fit_id → LmeFit cache)
```

| Component | Role |
|:----------|:-----|
| **`lme-rs-mcp` binary** | MCP transport + tool routing |
| **`LmeMcpServer`** | Holds `FitSession`, implements tools via `rmcp` macros |
| **`FitSession`** | `HashMap<fit_id, CachedFit>` for the process lifetime |
| **`load_csv`** | Resolves path, optional `LME_MCP_DATA_ROOT` guard, parses CSV |

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
- Bootstrap and ANOVA can be **CPU-heavy**; use reasonable `nsim` (e.g. 200–1000, not 10000) unless you intend to wait.

## Tool reference

All tools return a **single string** containing pretty-printed JSON (MCP text content).

### `lme_fit`

Fit a Gaussian linear mixed model (`lmer`).

**Parameters**

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `formula` | string | *required* | Wilkinson formula, e.g. `Reaction ~ Days + (1 \| Subject)` |
| `data_path` | string | *required* | Path to CSV on the server host |
| `reml` | boolean | `true` | `true` = REML, `false` = ML |

**Returns:** `FitSummary` JSON including `fit_id`, coefficients, SEs, σ², AIC/BIC, convergence flag.

### `lme_list_fits`

**Parameters:** none.

**Returns:** `{ "fits": [ FitListEntry, ... ] }` with `fit_id`, `formula`, `data_path`, `reml`, `num_obs`.

### `lme_fit_summary`

**Parameters**

| Field | Type | Description |
|:------|:-----|:------------|
| `fit_id` | string | Id from `lme_fit` |

**Returns:** Same shape as `lme_fit` output.

### `lme_forget_fit`

**Parameters**

| Field | Type | Description |
|:------|:-----|:------------|
| `fit_id` | string | Id to remove |

**Returns:** `{ "forgotten": "<fit_id>" }` or error if unknown.

### `lme_anova`

Fixed-effects ANOVA (Type I / II / III) with Satterthwaite or Kenward–Roger denominator df.

The server reloads the original CSV and applies `with_satterthwaite` or `with_kenward_roger` on the cached fit before computing the table.

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
| `nsim` | integer | `200` | Bootstrap replicates |
| `method` | string | `"parametric"` | `parametric` / `param` or `residual` / `res` |
| `reml` | boolean | `true` | REML/ML for each refit |
| `seed` | integer | null | Optional RNG seed (reproducible across `n_jobs`) |
| `n_jobs` | integer | null | Parallel workers; `null` = all logical CPUs (capped at `nsim`) |
| `level` | number | `0.95` | CI level for percentile intervals |

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
  "formula": "Reaction ~ Days + (1 | Subject)",
  "converged": true,
  "fixed_names": ["(Intercept)", "Days"],
  "coefficients": [ 251.4, 10.46 ],
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
| crates.io `lme-rs` dep | `0.1.11` (bootstrap); optional `[patch]` for unreleased co-dev |

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

- [lme-rs README](https://github.com/x4g4p3x/lme-rs)
- [lme-rs GUIDE — Bootstrap](https://github.com/x4g4p3x/lme-rs/blob/master/GUIDE.md#bootstrap-refits-boot_lmer)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [rmcp Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
