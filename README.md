# lme-rs-mcp

[MCP](https://modelcontextprotocol.io/) server exposing [**lme-rs**](https://github.com/x4g4p3x/lme-rs) statistical modeling operations to agents and IDEs such as Cursor and Claude Desktop.

The server is a **Rust binary** with no Python runtime. It speaks MCP over **stdio**, reads CSV data on the server host, stores fitted models in an in-process session, and returns typed `rmcp::Json<T>` responses with MCP `outputSchema` and `structuredContent`.

`lme-rs-mcp` is an adapter only. Statistical algorithms stay in `lme-rs`.

## Architecture

```text
MCP client
    |
    v
LmeMcpServer        transport only
    |
    v
lme-rs-agent        LmeAgentApi, DTOs, validation, FitSession
    |
    v
lme-rs 0.2.6 + Polars
```

The protocol-neutral core now lives in the separate [`lme-rs-agent`](https://github.com/x4g4p3x/lme-rs-agent) crate. Both this MCP adapter and [`lme-rs-scp`](https://github.com/x4g4p3x/lme-rs-scp) reuse it without depending on each other.

## Semantic API

The current adapter targets the published **`lme-rs 0.2.6`** release.

| Capability | Semantic MCP tool | Scope |
|:--|:--|:--|
| Fit a model | `fit_model` | LM, LMM, GLMM, built-in NLMM |
| List cached models | `list_models` | all model kinds |
| Model summary | `model_summary` | all model kinds |
| Forget a model | `forget_model` | all model kinds |
| Fixed-effects ANOVA | `anova` | LMM |
| Bootstrap CIs | `bootstrap` | LMM parametric/residual; GLMM parametric |
| Nested-model LRT | `compare_models` | compatible mixed models |
| Confidence intervals | `confidence_intervals` | Wald where SEs exist; profile for LMM/GLMM |
| Prediction | `predict` | population/conditional, link/response |
| Grouped cross-validation | `cross_validate` | LMM/GLMM |
| Diagnostics | `diagnostics` | all model kinds |
| Marginal means | `marginal_means` | LM/LMM |
| Pairwise comparisons | `pairwise_comparisons` | LM/LMM |

The original compatibility tools remain available unchanged: `lme_fit`, `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit`, `lme_anova`, and `lme_boot`.

### `fit_model`

`model_kind` accepts:

- `lm` — ordinary linear model; `formula` and `data_path` only.
- `lmm` — Gaussian mixed model; `reml` defaults to `true`.
- `glmm` — requires `family`; optional compatible `link`; `n_agq` defaults to `1`.
- `nlmm` — built-in three-part `nlmer` formula; optional named `start`; `reml` defaults to `false`; `n_agq` defaults to `1`.

GLMM families are `binomial`, `poisson`, `gaussian`, and `gamma`. Supported explicit links are `logit`, `probit`, `cloglog`, `log`, `identity`, `inverse`, and `sqrt`, subject to family/link compatibility.

Built-in NLMM means include `SSlogis`, `SSasymp`, `SSfol`, `SSmicmen`, `SSgompertz`, `SSpower`, `SSfpl`, `SSbiexp`, and `SSweibull`.

Example GLMM:

```json
{
  "model_kind": "glmm",
  "formula": "y ~ x + (1 | group)",
  "data_path": "poisson_data.csv",
  "family": "poisson",
  "n_agq": 1
}
```

Example NLMM:

```json
{
  "model_kind": "nlmm",
  "formula": "y ~ SSmicmen(x, Vmax, K) ~ Vmax|g",
  "data_path": "enzyme.csv",
  "start": { "Vmax": 10.0, "K": 1.5 }
}
```

Example LM:

```json
{
  "model_kind": "lm",
  "formula": "Reaction ~ Days",
  "data_path": "sleepstudy.csv"
}
```

All fitting operations return a `fit_id`, which is used by subsequent semantic operations.

### Prediction

`predict` accepts `mode = "population" | "conditional"` and `scale = "link" | "response"`. `data_path` is optional; when omitted, predictions use the original fitted data. `allow_new_levels` controls unseen grouping levels for conditional prediction.

### Comparison and confidence intervals

`compare_models` uses the `lme-rs` likelihood-ratio test. Models must use the same model kind and dataset; GLMMs must also share family/link. When REML applies, compare ML fits (`reml = false`).

`confidence_intervals` supports `method = "wald" | "profile"` and optional coefficient-name selection. Profile intervals are supported for LMM/GLMM by `lme-rs 0.2.6`.

### Cross-validation

`cross_validate` performs group-preserving k-fold CV for LMM/GLMM. Supply a grouping column, fold count, and optional `seed`/`n_jobs`. The response includes OOF predictions, fold assignments, RMSE/MAE, per-fold metrics, and binomial log loss where applicable.

## Marginal means

`marginal_means` and `pairwise_comparisons` support categorical interaction grids, conditioning factors, numeric reference values, equal/proportional nuisance weighting, and Holm/Bonferroni/Tukey adjustments. LM uses residual degrees of freedom, LMM defaults to Satterthwaite (Kenward-Roger is optional), and asymptotic inference can be requested explicitly. GLMM/NLMM marginal grids are unsupported by the pinned engine. The shared core owns these contracts.

## Install

```powershell
cargo install lme-rs-mcp --version 0.2.0 --locked
```

Or build from source:

```powershell
git clone https://github.com/x4g4p3x/lme-rs-mcp.git
cd lme-rs-mcp
cargo build --release --locked
```

Source builds and published installs resolve `lme-rs-agent 0.1.0` and `lme-rs 0.2.6` from crates.io. Sibling checkouts are optional for co-development.

Binary: `target\release\lme-rs-mcp.exe` on Windows or `target/release/lme-rs-mcp` on Unix.

## Configure Cursor

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

`LME_MCP_DATA_ROOT` is recommended for agent use. Relative paths resolve below the configured root, and canonicalized paths outside it are rejected.

## Documentation

| Doc | Contents |
|:--|:--|
| **[AGENT_API.md](AGENT_API.md)** | Protocol-neutral architecture, semantic surface, release boundaries, MCP/SCP path |
| **[GUIDE.md](GUIDE.md)** | Installation, tool reference, workflows, security, troubleshooting |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | Development workflow |
| **[RELEASING.md](RELEASING.md)** | Release/publish order |
| **[CHANGELOG.md](CHANGELOG.md)** | Version history |

## Requirements

- Rust stable
- `lme-rs` version pinned in `Cargo.toml`
- MCP client that spawns stdio servers
- CSV data readable on the MCP server host

## Status

Version 0.2.0 targets published `lme-rs 0.2.6`: LM/LMM/GLMM/NLMM fitting, lifecycle management, prediction, model comparison, confidence intervals, model-aware bootstrap, grouped CV, and diagnostics. The original `lme_*` tools remain for compatibility. LM/LMM marginal means and adjusted pairwise comparisons are exposed through the shared core. Expensive tools support the negotiated MCP Tasks extension.

Validate publication-critical statistical results against R `lme4` / `lmerTest` as appropriate.

## License

MIT — see [LICENSE](LICENSE).
