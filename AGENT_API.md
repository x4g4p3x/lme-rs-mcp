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

The existing MCP names remain stable for compatibility:

| MCP tool | Core operation | Status |
|---|---|---|
| `lme_fit` | `LmeAgentApi::fit_lmm` | implemented |
| `lme_list_fits` | `LmeAgentApi::list_fits` | implemented |
| `lme_fit_summary` | `LmeAgentApi::fit_summary` | implemented |
| `lme_forget_fit` | `LmeAgentApi::forget_fit` | implemented |
| `lme_anova` | `LmeAgentApi::anova` | implemented for LMM |
| `lme_boot` | `LmeAgentApi::bootstrap` | implemented for LMM |

All MCP responses use typed `rmcp::Json<T>` results. This gives clients both an MCP `outputSchema` and `structuredContent` while retaining text content for backwards compatibility.

The adapter now targets **`lme-rs 0.2.1`**. The current MCP fitting entry point still creates Gaussian LMMs only; the dependency upgrade and model-kind-aware session are groundwork for exposing the wider 0.2.x model surface without changing transport architecture again.

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
- number of observations
- convergence state

This distinction is important: `ModelKind` describes statistical semantics, while `LmeFit` is an implementation detail of the current `lme-rs` engine. Protocol adapters should never need to inspect concrete `lme-rs` fit internals directly.

The current `fit_lmm` path records `model_kind = "lmm"`, `reml = true/false`, and no family/link. Future GLMM/NLMM fit operations can reuse the same session without changing its stored fit type.

Operations that remain model-family-specific must reject incompatible cached kinds explicitly. For example, the current Satterthwaite/Kenward–Roger ANOVA and `boot_lmer` adapter paths are guarded as LMM-only until family-specific semantics are added.

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

1. Keep the current six MCP tool names working while the semantic API evolves.
2. **Done:** upgrade the dependency from `lme-rs 0.1.11` to `lme-rs 0.2.1` and refresh `Cargo.lock`.
3. **Done:** make cached model records model-kind-aware while retaining the unified upstream `LmeFit` representation.
4. Introduce a semantic `fit_model` operation and add GLMM first, retaining `lme_fit` as the LMM compatibility entry point.
5. Add NLMM/LM fitting plus prediction, model comparison, confidence intervals, cross-validation, and marginal means incrementally.
6. Introduce semantic `model_summary`, `list_models`, and `forget_model` names, initially retaining compatibility aliases where practical.
7. Map expensive operations onto MCP Tasks once the server's supported MCP SDK/spec version is upgraded accordingly.
8. Add an SCP adapter only as an optional outer integration layer.
