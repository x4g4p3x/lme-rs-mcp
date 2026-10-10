# Releasing lme-rs-mcp

The dependency order is intentional:

```text
lme-rs 0.2.6 -> lme-rs-agent 0.1.0 -> lme-rs-mcp 0.2.x
                                      -> lme-rs-scp 0.1.x
```

## 1. Publish the statistical engine

Verify that crates.io serves the `lme-rs` version pinned by `lme-rs-agent`:

```powershell
cargo search lme-rs
```

## 2. Publish the neutral agent core

In the sibling `lme-rs-agent` repository, remove any local engine patch, run its full checks,
then publish the version required by this manifest:

```powershell
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo publish --dry-run --locked
cargo publish --locked
```

## 3. Resolve the registry dependency here

The release manifest uses the registry dependency `lme-rs-agent = "=0.1.0"`.
Remove any local `.cargo/config.toml` patches, then verify the package without
relying on sibling checkouts:

```powershell
cargo update -p lme-rs-agent --precise 0.1.0
task ci
task preflight
cargo publish --dry-run --locked
```

Commit the lockfile with registry checksums before the MCP release.

## 4. Publish lme-rs-mcp

Bump the MCP crate version and update `CHANGELOG.md`, then:

```powershell
cargo publish --dry-run --locked
git tag v<MCP_VERSION>
git push origin HEAD --tags
cargo publish --locked
```

Do not reuse an already-published crates.io version. See [GUIDE.md](GUIDE.md) for client
configuration.
