//! Loading a whole project from its manifest: discovery of the project root
//! from any file inside it, and the parse-then-validate sequence every
//! graph-reading command shares.

use std::path::{Path, PathBuf};

use ndf_core::model::Graph;

use crate::CompilerError;
use crate::adapters::parse_project;
use crate::dependencies::load_dependency_graphs;
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

/// Parse and validate the project rooted at `root`, resolving its locked
/// dependencies exactly as `ndf build` and `ndf check` do.
pub fn load_project_graph(root: &Path) -> Result<(ProjectManifest, Graph), CompilerError> {
    let manifest = ProjectManifest::load(&root.join("ndf.yaml"))?;
    let lock = ProjectLock::load(&root.join("ndf.lock"))?;
    let dependencies = load_dependency_graphs(&manifest, &lock, root)?;
    let parsed = parse_project(&manifest)?;
    let diagnostics = validate_project_graph(&parsed, &dependencies, &ValidationPolicy::default());
    Ok((
        manifest,
        Graph {
            nodes: parsed.nodes,
            edges: parsed.edges,
            diagnostics,
        },
    ))
}
