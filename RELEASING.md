# Releasing lme-rs-mcp

This crate depends on **`lme-rs` from crates.io**. Publish the `lme-rs` version pinned in `Cargo.toml` first, then publish **`lme-rs-mcp`**.

Current master development pins **`lme-rs 0.2.1`**.

## 1. Ensure the pinned `lme-rs` release is published

In the [lme-rs](https://github.com/x4g4p3x/lme-rs) repository, run its release checks and publish/tag the version referenced by this repository's `Cargo.toml`.

For the current pin, crates.io must serve `lme-rs 0.2.1` before an MCP release is prepared.

Verify with:

```powershell
cargo search lme-rs
```

## 2. Refresh this repo's lockfile

Remove any local patch (delete `.cargo/config.toml` or drop the `[patch.crates-io]` section), then resolve the exact pinned version:

```powershell
cd lme-rs-mcp
cargo update -p lme-rs --precise 0.2.1
cargo test --locked
task ci
```

Commit the updated **`Cargo.lock`** with the registry checksum for the pinned `lme-rs` release. CI is single-repo and relies on this lockfile.

## 3. Publish `lme-rs-mcp`

Before publishing, bump the MCP crate version in `Cargo.toml` and update `CHANGELOG.md`. Then:

```powershell
cargo publish --dry-run --locked
git tag v<MCP_VERSION>
git push origin HEAD --tags
cargo publish --locked   # when ready
```

Do not reuse an already-published crates.io version.

## Co-development (optional)

You only need `task patch:local` when hacking **unreleased** `lme-rs` APIs on a sibling checkout. Release validation should remove the local patch and resolve the crates.io package into `Cargo.lock`.

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
