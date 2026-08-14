# Agent-facing API

`lme-rs-mcp` should expose statistical intent rather than mirror every Rust function in `lme-rs`.
The agent-facing API is therefore split into two layers:

```text
MCP / future SCP adapter
        |
        v
   LmeAgentApi
        |
        v
      lme-rs
```

`LmeAgentApi` owns model/session semantics and typed request/response DTOs. Protocol adapters own transport only.
This keeps the statistical contract reusable if a Science Context Protocol adapter is added later.

## Current implemented surface

The existing MCP names remain stable for compatibility, and the first semantic fitting name is now live:

| MCP tool | Core operation | Status |
|---|---|---|
| `fit_model` | `LmeAgentApi::fit_model` | implemented for LMM + GLMM |
| `lme_fit` | `LmeAgentApi::fit_lmm` | compatibility alias for LMM |
| `lme_list_fits` | `LmeAgentApi::list_fits` | implemented |
| `lme_fit_summary` | `LmeAgentApi::fit_summary` | implemented |
| `lme_forget_fit` | `LmeAgentApi::forget_fit` | implemented |
| `lme_anova` | `LmeAgentApi::anova` | implemented for LMM |
| `lme_boot` | `LmeAgentApi::bootstrap` | implemented for LMM |

All MCP responses use typed `rmcp::Json<T>` results. This gives clients both an MCP `outputSchema` and `structuredContent` while retaining text content for backwards compatibility.

The adapter targets **`lme-rs 0.2.1`**. `fit_model` currently accepts `model_kind = "lmm"` and `"glmm"`. GLMM requests expose family, optional link, and optional `n_agq` directly as semantic fit options.

## `fit_model` semantics

The request shape is intentionally model-family-aware rather than a bag of loosely interpreted options:

- `model_kind`: currently `lmm` or `glmm`
- `formula`: Wilkinson formula
- `data_path`: CSV path on the server host
- `reml`: optional for LMM, defaults to `true`; invalid for GLMM
- `family`: required for GLMM; one of `binomial`, `poisson`, `gaussian`, `gamma`
- `link`: optional for GLMM; canonical family link when omitted
- `n_agq`: optional for GLMM, defaults to `1` and must be greater than zero

Supported explicit links are `logit`, `probit`, `cloglog`, `log`, `identity`, `inverse`, and `sqrt`. Family/link compatibility is validated before data loading. Model-specific fields that do not apply are rejected rather than silently ignored.

`lme_fit` remains available for existing clients and delegates to the same LMM implementation used by `fit_model`, preventing semantic drift between compatibility and new APIs.

## Target semantic surface

Prefer a small set of intent-level tools instead of one MCP tool per Rust function:

| Target tool | Purpose |
|---|---|
| `fit_model` | Fit LM/LMM/GLMM/NLMM from a formula and dataset |
| `model_summary` | Return a typed summary for a cached model |
| `list_models` | List cached model handles and essential metadata |
| `forget_model` | Drop a cached model |
| `anova` | Fixed-effects ANOVA / term tests |
| `compare_models` | Compare nested or candidate models |
| `confidence_intervals` | Wald/profile confidence intervals |
| `bootstrap` | Parametric/residual bootstrap and intervals |
| `predict` | Population-level or conditional predictions |
| `cross_validate` | Group-structure-preserving cross-validation |
| `marginal_means` | Estimated marginal means and pairwise contrasts |
| `diagnostics` | Convergence and model-quality diagnostics |

The exact set should stay deliberately small. New `lme-rs` functions should normally be composed behind one of these semantic operations instead of creating a new protocol tool.

## Model handles

`lme-rs 0.2.1` uses the same `LmeFit` representation for LMM, GLMM, and NLMM fits. The adapter therefore does **not** need a parallel enum of concrete fit types. Instead, each cached record carries protocol-neutral semantic metadata alongside the unified fit:

- `fit_id` (future semantic alias: `model_id`)
- model kind (`lm`, `lmm`, `glmm`, `nlmm`)
- formula
- data source
- REML when applicable
- GLMM family/link when applicable
- GLMM `n_agq` when applicable
- number of observations
- convergence state

This distinction is important: `ModelKind` describes statistical semantics, while `LmeFit` is an implementation detail of the current `lme-rs` engine. Protocol adapters should never need to inspect concrete `lme-rs` fit internals directly.

Operations that remain model-family-specific must reject incompatible cached kinds explicitly. The current Satterthwaite/Kenward–Roger ANOVA and `boot_lmer` adapter paths remain LMM-only and reject GLMM handles.

## Long-running operations

Bootstrap, cross-validation, profile likelihood, and some nonlinear fits can be long-running. Keep these operations isolated in `LmeAgentApi` so the MCP adapter can later map them onto MCP Tasks without changing statistical semantics.

## SCP compatibility

A future Science Context Protocol integration should reuse `LmeAgentApi` and the same DTOs rather than importing MCP transport code. SCP can then add scientific workflow/provenance metadata around the same deterministic statistical operations.

The intended dependency direction is:

```text
lme-rs
  ^
  |
LmeAgentApi + DTOs
  ^          ^
  |          |
 MCP        SCP
```

Neither `lme-rs` nor the protocol-neutral API should depend on SCP.

## Migration sequence

1. Keep the compatibility MCP tool names working while the semantic API evolves.
2. **Done:** upgrade the dependency from `lme-rs 0.1.11` to `lme-rs 0.2.1` and refresh `Cargo.lock`.
3. **Done:** make cached model records model-kind-aware while retaining the unified upstream `LmeFit` representation.
4. **Done:** introduce semantic `fit_model` and add GLMM while retaining `lme_fit` as the LMM compatibility entry point.
5. Add LM/NLMM fitting plus prediction, model comparison, confidence intervals, cross-validation, and marginal means incrementally.
6. Introduce semantic `model_summary`, `list_models`, and `forget_model` names, initially retaining compatibility aliases where practical.
7. Map expensive operations onto MCP Tasks once the server's supported MCP SDK/spec version is upgraded accordingly.
8. Add an SCP adapter only as an optional outer integration layer.
