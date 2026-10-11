# Agent and contributor pre-flights

Thin Rust MCP adapter around [**lme-rs**](https://github.com/x4g4p3x/lme-rs). No Python, no `lme_ci.py`.

## One-time setup

```powershell
mise install
task setup            # hooks
task patch:local      # only when co-developing with a sibling lme-rs checkout
```

## Before commit

Lefthook (parallel, staged `*.rs`):

- `cargo fmt` (auto-staged)
- `cargo clippy --all-targets --locked -- -D warnings`
- `cargo check` when `Cargo.toml` / `Cargo.lock` change

Bypass once: `git commit --no-verify`.

## Before push

```powershell
task preflight        # lint + check + cargo audit
```

Bypass: `git push --no-verify`.

## Before PR

```powershell
task ci               # lint + check + test
```

## Dependencies

- **Shared core:** `lme-rs-agent = "=0.2.1"` from crates.io.
- **Statistical engine:** `lme-rs = "0.2.7"` through `lme-rs-agent` (and directly in tests).
- **Co-dev patch:** `task patch:local` when testing unreleased agent/engine APIs on sibling checkouts.

Release order: [RELEASING.md](RELEASING.md).

## Docs map

| Doc | Role |
|:----|:-----|
| [GUIDE.md](GUIDE.md) | User + tool reference |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Dev setup, patch workflow, releases |
| [README.md](README.md) | Quick start |
