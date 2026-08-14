# Agent-facing API

`lme-rs-mcp` exposes statistical **intent**, not a one-to-one mirror of every Rust function in `lme-rs`.

```text
MCP / future SCP adapter
        |
        v
   LmeAgentApi
        |
        v
      lme-rs
```

`LmeAgentApi` owns model/session semantics and typed DTOs. MCP owns transport only. A future Science Context Protocol adapter should reuse the same core rather than duplicate statistical logic.

## Implemented semantic surface

The adapter targets the published **`lme-rs 0.2.1`** release.

| Semantic MCP tool | Core operation | Current scope |
|---|---|---|
| `fit_model` | `fit_model` | LM / LMM / GLMM / NLMM |
| `list_models` | `list_models` | all cached models |
| `model_summary` | `model_summary` | all cached models |
| `forget_model` | `forget_model` | all cached models |
| `anova` | `anova` | LMM fixed-effects Type I/II/III |
| `bootstrap` | `bootstrap_model` | LMM parametric/residual; GLMM parametric |
| `compare_models` | `compare_models` | LRT for compatible mixed models |
| `confidence_intervals` | `confidence_intervals` | Wald; profile for LMM/GLMM |
| `predict` | `predict` | population/conditional, link/response |
| `cross_validate` | `cross_validate` | grouped LMM/GLMM CV |
| `diagnostics` | `diagnostics` | all model kinds |

Compatibility remains explicit rather than implicit: `lme_fit`, `lme_list_fits`, `lme_fit_summary`, `lme_forget_fit`, `lme_anova`, and `lme_boot` continue to work for existing clients.

All MCP results use typed `rmcp::Json<T>`, giving clients generated `outputSchema` and structured content.

## `fit_model`

The request is model-family-aware:

- **LM**: `formula`, `data_path`; mixed-model-only fields are rejected.
- **LMM**: optional `reml`, default `true`.
- **GLMM**: required `family`; optional validated `link`; optional `n_agq`, default `1`; REML/start invalid.
- **NLMM**: built-in three-part `nlmer` formula; optional named `start`; optional `reml`, default `false`; optional `n_agq`, default `1`; family/link invalid.

GLMM families: `binomial`, `poisson`, `gaussian`, `gamma`.

Explicit links: `logit`, `probit`, `cloglog`, `log`, `identity`, `inverse`, `sqrt`, subject to family compatibility.

Built-in NLMM means include `SSlogis`, `SSasymp`, `SSfol`, `SSmicmen`, `SSgompertz`, `SSpower`, `SSfpl`, `SSbiexp`, and `SSweibull`.

Advanced custom NLMM Rust closures, bounds, and optimizer controls remain library-level; they are not useful as generic agent wire fields yet.

## Model handles

The server caches one unified upstream `LmeFit` plus protocol-neutral metadata:

- `fit_id`
- `model_kind` (`lm`, `lmm`, `glmm`, `nlmm`)
- formula / data source
- REML where applicable
- GLMM family/link
- GLMM/NLMM `n_agq`
- user-supplied NLMM starts
- engine fit data and convergence metadata

`ModelKind` is a protocol semantic. `LmeFit` is an engine implementation detail.

## Operation semantics

### Prediction

`predict` supports population versus conditional prediction and link versus response scale. It accepts optional new CSV data; otherwise the original fitted data are used. Conditional LM prediction is rejected because LM has no random effects. New grouping levels are controlled explicitly with `allow_new_levels`.

### Model comparison

`compare_models` uses the released engine's likelihood-ratio test. It requires matching model kinds and data. GLMMs must share family/link. When REML applies, comparison requires models fit with ML (`reml = false`). The released `lm()` does not expose deviance, so LM LRT comparison is intentionally unavailable.

### Confidence intervals

`confidence_intervals` accepts coefficient names and `wald` or `profile` methods. Profile intervals are exposed only for LMM/GLMM, matching `lme-rs 0.2.1`. Wald intervals require standard errors from the engine; the bare release `lm()` does not currently supply them.

### Bootstrap

The semantic `bootstrap` is model-aware:

- LMM: parametric or residual.
- GLMM: parametric.
- LM/NLMM: rejected by this release-backed adapter.

The compatibility `lme_boot` retains its previous LMM-only request contract.

### Cross-validation

`cross_validate` preserves grouping units across folds. LMM uses `cv_grouped`; GLMM uses `cv_grouped_glmer` with cached family/link/AGQ metadata. Results contain out-of-fold predictions, fold assignment, global and per-fold metrics, and binomial log loss where applicable.

### Diagnostics

`diagnostics` is deliberately non-destructive and does not refit. It reports convergence/iterations, dimensions, coefficient and residual finiteness, standard-error availability, residual mean/RMSE/max-absolute residual, and explanatory messages.

## Release boundary: estimated marginal means

The target semantic surface includes `marginal_means`, but the estimated-marginal-means implementation currently visible on upstream `lme-rs` master was added **after tag `v0.2.1`**. `lme-rs-mcp` remains pinned to the published crates.io release, so it will not:

1. copy statistical implementation into the protocol adapter, or
2. replace a releasable crates.io dependency with an unreleased git revision merely to expose one tool.

Add `marginal_means` after a new `lme-rs` release contains that API.

## Long-running operations and MCP Tasks

Bootstrap, grouped CV, profile likelihood, AGQ, and nonlinear fitting can be expensive. They already live behind protocol-neutral operations, so a future MCP SDK/spec upgrade can map them onto MCP Tasks without changing statistical request/response semantics.

## SCP compatibility

A future SCP integration should remain an outer orchestration adapter:

```text
lme-rs
  ^
  |
LmeAgentApi + DTOs
  ^          ^
  |          |
 MCP        SCP
```

Neither `lme-rs` nor the protocol-neutral core should depend on MCP/SCP orchestration details.

## Migration status

1. **Done:** protocol-neutral agent core.
2. **Done:** structured MCP outputs.
3. **Done:** `lme-rs 0.2.1` upgrade and model-kind-aware session.
4. **Done:** semantic `fit_model` for LMM/GLMM/NLMM.
5. **Done:** LM, completing `fit_model = lm | lmm | glmm | nlmm`.
6. **Done:** semantic lifecycle names.
7. **Done:** prediction, comparison, confidence intervals, model-aware bootstrap, grouped CV, diagnostics.
8. **Blocked on upstream release:** estimated marginal means.
9. **Future transport enhancement:** map expensive operations to MCP Tasks.
10. **Optional:** add SCP adapter around the same `LmeAgentApi`.
