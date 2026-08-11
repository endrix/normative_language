use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ndf_core::model::{DiagnosticSeverity, Graph};
use serde::Serialize;
use serde_yaml_ng::Value;

use crate::CompilerError;
use crate::adapters::parse_project;
use crate::manifest::{ProjectLock, ProjectManifest};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ResolvedProject {
    pub project_id: String,
    pub path: PathBuf,
    pub revision: String,
    pub dirty: bool,
}

#[derive(Debug)]
pub struct DependencyGraph {
    pub project_id: String,
    pub revision: String,
    pub graph: Graph,
}

pub fn load_dependency_graphs(
    manifest: &ProjectManifest,
    lock: &ProjectLock,
    workspace: &Path,
) -> Result<BTreeMap<String, DependencyGraph>, CompilerError> {
    let resolved = resolve_dependencies(manifest, lock, workspace)?;
    let mut graphs = BTreeMap::new();
    for dependency in resolved {
        let declaration = &manifest.dependencies[&dependency.project_id];
        if !declaration.graph {
            continue;
        }
        let manifest_path = dependency.path.join("ndf.yaml");
        if !manifest_path.is_file() {
            return Err(contract(format!(
                "NDF-DEP-005: graph dependency has no ndf.yaml: {}",
                dependency.project_id
            )));
        }
        let dependency_manifest = ProjectManifest::load(&manifest_path).map_err(|error| {
            contract(format!(
                "NDF-DEP-005: invalid graph dependency manifest for {}: {error}",
                dependency.project_id
            ))
        })?;
        if dependency_manifest.project != dependency.project_id {
            return Err(contract(format!(
                "NDF-DEP-005: dependency project mismatch: expected {}, got {}",
                dependency.project_id, dependency_manifest.project
            )));
        }
        let graph = parse_project(&dependency_manifest)?;
        if let Some(diagnostic) = graph
            .diagnostics
            .iter()
            .find(|item| item.severity == DiagnosticSeverity::Error)
        {
            return Err(contract(format!(
                "NDF-DEP-006: dependency graph {} is invalid: {}: {}",
                dependency.project_id, diagnostic.code, diagnostic.message
            )));
        }
        graphs.insert(
            dependency.project_id.clone(),
            DependencyGraph {
                project_id: dependency.project_id,
                revision: dependency.revision,
                graph,
            },
        );
    }
    Ok(graphs)
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
