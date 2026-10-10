# Agent-facing API

`lme-rs-mcp` exposes statistical **intent**, not a one-to-one mirror of every Rust function in `lme-rs`.

```text
MCP adapter       SCP adapter
      \             /
       v           v
        lme-rs-agent
        LmeAgentApi
             |
             v
           lme-rs
```

The separate `lme-rs-agent` crate owns model/session semantics and typed DTOs. MCP owns transport only. `lme-rs-scp` reuses the same core and adds scientific context/provenance mapping.

## Implemented semantic surface

The adapter targets the published **`lme-rs 0.2.6`** release.

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
| `marginal_means` | `marginal_means` | LM/LMM reference grids |
| `pairwise_comparisons` | `pairwise_comparisons` | LM/LMM adjusted comparisons |

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

`compare_models` uses the released engine's likelihood-ratio test. It requires matching model kinds and data. GLMMs must share family/link. When REML applies, comparison requires models fit with ML (`reml = false`). LM LRT comparison remains outside this adapter contract.

### Confidence intervals

`confidence_intervals` accepts coefficient names and `wald` or `profile` methods. Profile intervals are exposed only for LMM/GLMM, matching `lme-rs 0.2.6`. Wald intervals require standard errors from the engine; LM now supplies them through `lm_df()`.

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

## Estimated marginal means

`lme-rs 0.2.6` contains the engine implementation for estimated marginal means and pairwise comparisons. The shared core now defines marginal-means and pairwise-comparison DTOs and orchestration. MCP exposes both operations; SCP can reuse them without copying statistical code.

## Long-running operations and MCP Tasks

Bootstrap, grouped CV, profile likelihood, AGQ, and nonlinear fitting can be expensive. The adapter uses rmcp 3.5.1 and maps eligible calls onto the SEP-2663 Tasks extension when the client declares it in modern per-request capabilities. Other clients receive ordinary results. Numerical work runs outside the asynchronous transport runtime, bounded to two simultaneous operations.

## SCP compatibility

The SCP integration remains an outer orchestration adapter:

```text
lme-rs
  ^
  |
lme-rs-agent: LmeAgentApi + DTOs
  ^          ^
  |          |
 MCP        SCP
```

Neither `lme-rs` nor the protocol-neutral core should depend on MCP/SCP orchestration details.

## Migration status

1. **Done:** protocol-neutral core extracted to `lme-rs-agent`.
2. **Done:** structured MCP outputs.
3. **Done:** `lme-rs 0.2.6` upgrade and model-kind-aware session.
4. **Done:** semantic `fit_model` for LMM/GLMM/NLMM.
5. **Done:** LM, completing `fit_model = lm | lmm | glmm | nlmm`.
6. **Done:** semantic lifecycle names.
7. **Done:** prediction, comparison, confidence intervals, model-aware bootstrap, grouped CV, diagnostics.
8. **Done:** estimated marginal means and adjusted pairwise comparisons use shared-core contracts.
9. **Done:** negotiated MCP Tasks, bounded background computation, cancellation, TTL, and cancelled-fit cleanup.
10. **Done (initial harness):** `lme-rs-scp` maps one real `fit_model` workflow to experiment context and provenance.
