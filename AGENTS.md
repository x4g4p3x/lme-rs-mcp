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

## Dependency on lme-rs

- **Published:** `lme-rs = "0.1.11"` from crates.io.
- **Co-dev patch:** `task patch:local` when testing unreleased APIs on a sibling checkout.

Release order: [RELEASING.md](RELEASING.md).

## Docs map

| Doc | Role |
|:----|:-----|
| [GUIDE.md](GUIDE.md) | User + tool reference |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Dev setup, patch workflow, releases |
| [README.md](README.md) | Quick start |
