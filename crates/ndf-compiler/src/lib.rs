//! NDF parsing, validation, indexing, and query workflows.

pub mod adapters;
pub mod baseline;
pub mod dependencies;
pub mod diff;
pub mod export;
pub mod fix;
pub mod id_allocator;
pub mod index;
pub mod manifest;
pub mod markdown;
pub mod query;
pub mod report;
pub mod rules;

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid YAML in {path}: {source}")]
    Yaml {
        path: PathBuf,
        source: serde_yaml_ng::Error,
    },
    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Contract(String),
}
