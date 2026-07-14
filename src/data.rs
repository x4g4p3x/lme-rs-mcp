//! Load tabular data from local CSV paths for MCP tool calls.

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use polars::prelude::*;

/// Resolve and load a CSV file referenced by an MCP tool.
///
/// When `LME_MCP_DATA_ROOT` is set, relative `data_path` values are resolved under that
/// directory before canonicalization. The canonical path must still lie under the root.
pub fn load_csv(data_path: &str) -> Result<(PathBuf, DataFrame)> {
    let path = Path::new(data_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(root) = optional_data_root() {
        root.join(path)
    } else {
        path.to_path_buf()
    };

    let canonical = resolved
        .canonicalize()
        .with_context(|| format!("data_path not found or inaccessible: {data_path}"))?;

    if let Some(root) = optional_data_root() {
        let root_canonical = root
            .canonicalize()
            .with_context(|| format!("LME_MCP_DATA_ROOT not found or inaccessible: {root:?}"))?;
        if !canonical.starts_with(&root_canonical) {
            anyhow::bail!(
                "data_path {:?} is outside LME_MCP_DATA_ROOT {:?}",
                canonical,
                root_canonical
            );
        }
    }

    if canonical.extension().and_then(|s| s.to_str()) != Some("csv") {
        anyhow::bail!("only CSV files are supported in this MCP server version");
    }

    let file = File::open(&canonical).with_context(|| format!("failed to open {canonical:?}"))?;
    let df = CsvReader::new(file)
        .finish()
        .with_context(|| format!("failed to parse CSV {canonical:?}"))?;

    Ok((canonical, df))
}

fn optional_data_root() -> Option<PathBuf> {
    std::env::var_os("LME_MCP_DATA_ROOT").map(PathBuf::from)
}
