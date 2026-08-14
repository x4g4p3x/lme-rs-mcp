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
| `lme_anova` | `LmeAgentApi::anova` | implemented |
| `lme_boot` | `LmeAgentApi::bootstrap` | implemented |

All MCP responses use typed `rmcp::Json<T>` results. This gives clients both an MCP `outputSchema` and `structuredContent` while retaining text content for backwards compatibility.

## Target semantic surface

When the server moves to the current `lme-rs` feature set, prefer a small set of intent-level tools instead of one MCP tool per Rust function:

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

The current implementation stores Gaussian LMMs as `fit_id -> LmeFit` in memory. Supporting GLMM/NLMM should first generalize the protocol-neutral session to a model enum or another typed model handle before adding new MCP tools.

A future model record should expose at least:

- `model_id`
- model kind (`lm`, `lmm`, `glmm`, `nlmm`)
- formula
- data source
- fit options relevant to that model kind
- number of observations
- convergence state
- warnings

Protocol adapters should never need to inspect concrete `lme-rs` fit types directly.

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

1. Keep the current six MCP tool names working while structured output and the protocol-neutral API settle.
2. Upgrade the dependency from `lme-rs 0.1.11` to the current `0.2.x` release and refresh `Cargo.lock`.
3. Generalize the session beyond `LmeFit`.
4. Introduce the semantic tool names above, initially retaining compatibility aliases where practical.
5. Add GLMM/NLMM, prediction, model comparison, confidence intervals, cross-validation, and marginal means incrementally.
6. Map expensive operations onto MCP Tasks once the server's supported MCP SDK/spec version is upgraded accordingly.
7. Add an SCP adapter only as an optional outer integration layer.
