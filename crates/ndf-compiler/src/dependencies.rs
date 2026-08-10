use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;
use serde_yaml_ng::Value;

use crate::CompilerError;
use crate::manifest::{ProjectLock, ProjectManifest};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ResolvedProject {
    pub project_id: String,
    pub path: PathBuf,
    pub revision: String,
    pub dirty: bool,
}

pub fn resolve_dependencies(
    manifest: &ProjectManifest,
    lock: &ProjectLock,
    workspace: &Path,
) -> Result<Vec<ResolvedProject>, CompilerError> {
    let allow_dirty = manifest
        .policies
        .get("ndf")
        .and_then(Value::as_mapping)
        .and_then(|mapping| mapping.get(Value::String("allow_dirty".to_owned())))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut resolved = Vec::new();
    for project in manifest.dependencies.keys() {
        let dependency = lock
            .dependencies
            .iter()
            .find(|dependency| &dependency.project == project)
            .ok_or_else(|| {
                contract(format!(
                    "NDF-DEP-001: dependency is missing from lock: {project}"
                ))
            })?;
        if !exact_revision(&dependency.revision) {
            return Err(contract(format!(
                "NDF-DEP-002: dependency revision is not an exact commit: {project}"
            )));
        }
        let path = workspace.join(&dependency.path);
        if !path.is_dir() {
            return Err(contract(format!(
                "NDF-DEP-001: dependency checkout is missing: {project}"
            )));
        }
        let path = path.canonicalize().map_err(|source| CompilerError::Read {
            path: path.clone(),
            source,
        })?;
        let revision_output = git(&path, &["rev-parse", "HEAD"])?;
        if !revision_output.status.success() {
            return Err(contract(format!(
                "NDF-DEP-001: dependency is not a Git checkout: {project}"
            )));
        }
        let revision = String::from_utf8_lossy(&revision_output.stdout)
            .trim()
            .to_owned();
        if revision != dependency.revision {
            return Err(contract(format!(
                "NDF-DEP-003: dependency revision mismatch for {project}: expected {}, got {revision}",
                dependency.revision
            )));
        }
        let status = git(&path, &["status", "--porcelain"])?;
        if !status.status.success() {
            return Err(contract(format!(
                "NDF-DEP-001: dependency is not a Git checkout: {project}"
            )));
        }
        let dirty = !status.stdout.is_empty();
        if dirty && !allow_dirty {
            return Err(contract(format!(
                "NDF-DEP-004: dependency checkout is dirty: {project}"
            )));
        }
        resolved.push(ResolvedProject {
            project_id: project.clone(),
            path,
            revision,
            dirty,
        });
    }
    Ok(resolved)
}

fn git(path: &Path, arguments: &[&str]) -> Result<Output, CompilerError> {
    Command::new("git")
        .args(arguments)
        .current_dir(path)
        .output()
        .map_err(|source| CompilerError::Read {
            path: path.to_path_buf(),
            source,
        })
}

fn exact_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn contract(message: impl Into<String>) -> CompilerError {
    CompilerError::Contract(message.into())
}
