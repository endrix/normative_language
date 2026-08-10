//! NDF parsing, validation, indexing, and query workflows.

pub mod baseline;
pub mod manifest;
pub mod markdown;
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
    #[error("{0}")]
    Contract(String),
}
