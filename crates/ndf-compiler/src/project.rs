//! Loading a whole project from its manifest: discovery of the project root
//! from any file inside it, and the parse-then-validate sequence every
//! graph-reading command shares.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ndf_core::model::Graph;

use crate::CompilerError;
use crate::adapters::parse_project;
use crate::dependencies::{DependencyGraph, load_dependency_graphs};
use crate::manifest::{ProjectLock, ProjectManifest};
use crate::rules::{ValidationPolicy, validate_project_graph};

/// The nearest ancestor directory of `path` (or `path` itself, when it is a
/// directory) that holds an `ndf.yaml`.
pub fn find_project_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|candidate| candidate.join("ndf.yaml").is_file())
        .map(Path::to_path_buf)
}

/// A project as the graph-reading commands see it: its manifest, its
/// validated graph, and the graphs of the dependencies it resolved (those
/// declared with `graph: true`), each with the checkout it was read from.
pub struct LoadedProject {
    pub manifest: ProjectManifest,
    pub graph: Graph,
    pub dependencies: BTreeMap<String, DependencyGraph>,
}

/// Parse and validate the project rooted at `root`, resolving its locked
/// dependencies exactly as `ndf build` and `ndf check` do.
pub fn load_project_graph(root: &Path) -> Result<(ProjectManifest, Graph), CompilerError> {
    let manifest = ProjectManifest::load(&root.join("ndf.yaml"))?;
    let lock = ProjectLock::load(&root.join("ndf.lock"))?;
    let loaded = load_with(root, manifest, lock)?;
    Ok((loaded.manifest, loaded.graph))
}

/// Parse and validate the project rooted at `root`, keeping the dependency
/// graphs for anything that draws or navigates across projects.
///
/// A project that declares no dependencies needs no lock: `ndf.lock` may be
/// absent and is then read as empty. A project with dependencies is held to
/// the lock exactly as `load_project_graph` holds it.
pub fn load_project(root: &Path) -> Result<LoadedProject, CompilerError> {
    let manifest = ProjectManifest::load(&root.join("ndf.yaml"))?;
    let lock_path = root.join("ndf.lock");
    let lock = if manifest.dependencies.is_empty() && !lock_path.exists() {
        ProjectLock {
            format_version: "0.1".to_owned(),
            dependencies: Vec::new(),
        }
    } else {
        ProjectLock::load(&lock_path)?
    };
    load_with(root, manifest, lock)
}

fn load_with(
    root: &Path,
    manifest: ProjectManifest,
    lock: ProjectLock,
) -> Result<LoadedProject, CompilerError> {
    let dependencies = load_dependency_graphs(&manifest, &lock, root)?;
    let parsed = parse_project(&manifest)?;
    let diagnostics = validate_project_graph(&parsed, &dependencies, &ValidationPolicy::default());
    Ok(LoadedProject {
        manifest,
        graph: Graph {
            nodes: parsed.nodes,
            edges: parsed.edges,
            diagnostics,
        },
        dependencies,
    })
}
