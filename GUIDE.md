# lme-rs-mcp User Guide

This guide covers installing, configuring, and using the `lme-rs-mcp` Model Context Protocol server. For the engine itself, see the `lme-rs` documentation; for architectural rationale, see [AGENT_API.md](AGENT_API.md).

## Architecture

```text
MCP client
    |
    v
LmeMcpServer
    |
    v
LmeAgentApi
    |
    v
lme-rs 0.2.1 + Polars
    |
    v
FitSession: fit_id -> CachedFit -> LmeFit
```

The MCP layer owns transport. `LmeAgentApi` owns statistical/session semantics. Responses are typed `rmcp::Json<T>` values with generated MCP schemas and structured content.

## Install

```powershell
git clone https://github.com/x4g4p3x/lme-rs-mcp.git
cd lme-rs-mcp
cargo build --release --locked
```

Validate a checkout with:

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo check --locked --all-targets
cargo test --locked
```

## Configure an MCP client

Example Cursor configuration:

```json
{
  "mcpServers": {
    "lme-rs": {
      "command": "C:\\path\\to\\lme-rs-mcp.exe",
      "env": {
        "LME_MCP_DATA_ROOT": "C:\\path\\to\\csv\\data"
      }
    }
  }
}
```

`LME_MCP_DATA_ROOT` is optional but recommended. Relative paths resolve under the configured directory; canonicalized paths outside it are rejected.

## Session model

Fits are stored in process memory and identified by UUID `fit_id`. A server restart clears the cache. All four model kinds use the same upstream `LmeFit` representation plus semantic metadata:

- model kind (`lm`, `lmm`, `glmm`, `nlmm`)
- formula and data path
- REML when applicable
- GLMM family/link
- GLMM/NLMM `n_agq`
- optional user-provided NLMM start values

Use the semantic lifecycle tools for new clients: `list_models`, `model_summary`, `forget_model`. The old `lme_list_fits`, `lme_fit_summary`, and `lme_forget_fit` tools remain available.

## Tool reference

### `fit_model`

Fits LM, LMM, GLMM, or built-in formula-based NLMM models.

Common fields:

| Field | Meaning |
|:--|:--|
| `model_kind` | `lm`, `lmm`, `glmm`, or `nlmm` |
| `formula` | model formula |
| `data_path` | CSV path on the server host |

Model-specific fields:

- **LM:** no additional fitting options.
- **LMM:** optional `reml`, default `true`.
- **GLMM:** required `family`; optional `link`; optional `n_agq`, default `1`.
- **NLMM:** optional `start`; optional `reml`, default `false`; optional `n_agq`, default `1`.

Inapplicable fields are rejected rather than ignored.

LM example:

```json
{
  "model_kind": "lm",
  "formula": "Reaction ~ Days",
  "data_path": "sleepstudy.csv"
}
```

LMM example:

```json
{
  "model_kind": "lmm",
  "formula": "Reaction ~ Days + (1 | Subject)",
  "data_path": "sleepstudy.csv",
  "reml": true
}
```

Poisson GLMM example:

```json
{
  "model_kind": "glmm",
  "formula": "y ~ x + (1 | group)",
  "data_path": "poisson_data.csv",
  "family": "poisson",
  "n_agq": 1
}
```

GLMM families are `binomial`, `poisson`, `gaussian`, and `gamma`. Explicit links are `logit`, `probit`, `cloglog`, `log`, `identity`, `inverse`, and `sqrt`, subject to family/link compatibility.

NLMM example:

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

Omit NLMM `start` to request upstream self-start heuristics. Built-in means include `SSlogis`, `SSasymp`, `SSfol`, `SSmicmen`, `SSgompertz`, `SSpower`, `SSfpl`, `SSbiexp`, and `SSweibull`.

### `list_models`

No parameters. Returns cached model handles and essential metadata.

### `model_summary`

```json
{ "fit_id": "..." }
```

Returns the same structured summary shape produced by fitting.

### `forget_model`

```json
{ "fit_id": "..." }
```

Removes one model from the process-local cache.

### `anova`

LMM fixed-effects Type I/II/III ANOVA using Satterthwaite or Kenward-Roger denominator degrees of freedom.

```json
{
  "fit_id": "...",
  "ddf_method": "satterthwaite",
  "anova_type": "III"
}
```

This operation is intentionally LMM-only in the released engine-backed surface.

### `bootstrap`

Model-aware bootstrap confidence intervals.

```json
{
  "fit_id": "...",
  "nsim": 200,
  "method": "parametric",
  "seed": 42,
  "n_jobs": 4,
  "level": 0.95
}
```

- LMM supports `parametric` and `residual`; optional `reml` overrides the cached fit mode.
- GLMM supports `parametric` only; REML is invalid.
- LM/NLMM are rejected by this release-backed operation.

The legacy `lme_boot` keeps its original LMM-only request shape.

### `compare_models`

Likelihood-ratio test for two cached nested mixed models:

```json
{
  "fit_id_a": "...",
  "fit_id_b": "..."
}
```

Requirements:

- same `model_kind`
- same dataset
- GLMM family/link must match
- when REML applies, fit both candidates with `reml = false`

The released `lm()` fit does not provide deviance, so LM LRT comparison is not exposed.

### `confidence_intervals`

```json
{
  "fit_id": "...",
  "level": 0.95,
  "method": "wald",
  "parameters": ["Days"]
}
```

`parameters` is optional. `wald` uses the engine's fitted standard errors. `profile` refits profile likelihood and is supported for LMM/GLMM. The bare `lm()` implementation in `lme-rs 0.2.1` does not provide coefficient standard errors, so LM Wald CIs are unavailable until the engine supplies them.

### `predict`

```json
{
  "fit_id": "...",
  "mode": "population",
  "scale": "response",
  "allow_new_levels": false
}
```

Optional `data_path` can point to new CSV data. If omitted, prediction uses the original fit data.

- `mode`: `population` or `conditional`
- `scale`: `link` or `response`
- `allow_new_levels`: controls unseen group levels in conditional mixed-model prediction

LM only supports population prediction because it has no random effects.

### `cross_validate`

Group-preserving k-fold CV for LMM/GLMM:

```json
{
  "fit_id": "...",
  "group_col": "Subject",
  "n_splits": 5,
  "seed": 42,
  "n_jobs": 4
}
```

All observations for one group stay in the same fold. Results include OOF predictions, fold assignment, RMSE/MAE, per-fold metrics, convergence state, and mean Bernoulli log loss for binomial GLMMs.

### `diagnostics`

```json
{ "fit_id": "..." }
```

Returns non-destructive diagnostics without refitting: convergence/iterations, observation and parameter dimensions, coefficient/residual finiteness, SE availability, residual mean/RMSE/max-absolute residual, and explanatory messages.

## Compatibility tools

Existing clients may continue using:

- `lme_fit`
- `lme_list_fits`
- `lme_fit_summary`
- `lme_forget_fit`
- `lme_anova`
- `lme_boot`

New integrations should prefer semantic names.

## Typical semantic workflow

1. `fit_model`.
2. Check `converged` and model metadata.
3. Run `diagnostics`.
4. Use `predict`, `confidence_intervals`, `anova`, `compare_models`, `bootstrap`, or `cross_validate` as appropriate for the model family.
5. Inspect with `model_summary` / `list_models`.
6. `forget_model` when the fit is no longer needed.

## Estimated marginal means

A `marginal_means` semantic operation is intentionally not present yet. The implementation exists on upstream `lme-rs` master but was added after the published `v0.2.1` tag. This server stays on the crates.io release and does not duplicate statistical algorithms in the MCP adapter. Add the tool after that upstream feature is published in a subsequent `lme-rs` release.

## Current limitations

| Topic | Status |
|:--|:--|
| LM / LMM / GLMM / built-in NLMM fitting | Exposed |
| Prediction | Exposed |
| Nested mixed-model LRT | Exposed |
| Wald/profile CIs | Exposed within engine support |
| Model-aware bootstrap | Exposed for LMM/GLMM |
| Grouped CV | Exposed for LMM/GLMM |
| Diagnostics | Exposed |
| Estimated marginal means | Waiting for upstream release |
| NLMM custom Rust means/bounds/optimizer internals | Library-level only |
| Parquet / Arrow input | Not exposed; CSV only |
| Session persistence | Not implemented |
| MCP Tasks for expensive operations | Future transport enhancement |

## Troubleshooting

### `unknown fit_id`

The model was forgotten or the MCP server restarted. List models or refit.

### `outside LME_MCP_DATA_ROOT`

The canonicalized path escaped the configured allowlist. Keep data under the root or intentionally widen it.

### `compare_models requires ML fits`

Refit the LMM/NLMM candidates with `reml = false` before likelihood comparison.

### `profile confidence intervals support lmm and glmm models only`

Use Wald intervals where the engine supplies standard errors, or choose an LMM/GLMM for profile likelihood.

### `GLMM bootstrap supports the parametric method only`

Use `method = "parametric"` for GLMMs.

### Expensive operation is slow

Bootstrap, grouped CV, profile CIs, AGQ, and NLMM fitting can be CPU-heavy. Keep `nsim`, `n_splits`, `n_jobs`, and `n_agq` intentional.

### stdout pollution

MCP uses stdout for protocol traffic. Logging should go to stderr.
