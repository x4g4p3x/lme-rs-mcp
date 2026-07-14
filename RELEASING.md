# Releasing lme-rs-mcp

This crate depends on **`lme-rs` from crates.io**. Publish **`lme-rs` first**, then **`lme-rs-mcp`**.

## 1. Publish `lme-rs` 0.1.11

In the [lme-rs](https://github.com/x4g4p3x/lme-rs) repository (see [RELEASING.md](../lme-rs/RELEASING.md)):

```powershell
task ci                    # or full pre-release checks
cargo publish --dry-run --allow-dirty
git add -A && git commit -m "Release 0.1.11"
git tag v0.1.11
git push origin HEAD --tags
```

Push the `v0.1.11` tag (or run `cargo publish` locally with `CARGO_REGISTRY_TOKEN`) so **crates.io** serves `lme-rs` 0.1.11.

Wait until `cargo search lme-rs` shows `0.1.11`.

## 2. Refresh this repo's lockfile

Remove any local patch (delete `.cargo/config.toml` or drop the `[patch.crates-io]` section), then:

```powershell
cd lme-rs-mcp
cargo update -p lme-rs
cargo test --locked
task ci
```

Commit the updated **`Cargo.lock`** (registry checksums for `lme-rs` 0.1.11). CI is single-repo and needs this lockfile.

## 3. Publish `lme-rs-mcp`

```powershell
cargo publish --dry-run --locked
git tag v0.1.0
git push origin HEAD --tags
cargo publish --locked   # when ready
```

## Co-development (optional)

After 0.1.11 is on crates.io, you only need `task patch:local` when hacking **unreleased** `lme-rs` APIs on a sibling checkout.

## MCP client config

Point Cursor (or other MCP clients) at the release binary:

```json
{
  "mcpServers": {
    "lme-rs": {
      "command": "lme-rs-mcp",
      "env": {
        "LME_MCP_DATA_ROOT": "C:\\path\\to\\csv\\data"
      }
    }
  }
}
```

See [GUIDE.md](GUIDE.md) for tool reference.
