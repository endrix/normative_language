use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use ndf_compiler::CompilerError;
use ndf_compiler::dependencies::resolve_dependencies;
use ndf_compiler::diagram::{GraphDocument, failure_document};
use ndf_compiler::diff::semantic_diff;
use ndf_compiler::export::export_graph;
use ndf_compiler::fix::{FixMode, markdown_paths, safe_fix};
use ndf_compiler::id_allocator::allocate_id;
use ndf_compiler::index::{BuildProvenance, NdfIndex, build_index};
use ndf_compiler::manifest::{ProjectLock, ProjectManifest};
use ndf_compiler::project::load_project_graph;
use ndf_compiler::pto_impact::pto_release_impact;
use ndf_compiler::query::{query_nodes, trace};
use ndf_compiler::report::{CoveragePolicy, coverage};
use ndf_core::identity::NodeId;
use ndf_core::model::{Diagnostic, DiagnosticSeverity, Graph};
use ndf_core::version::{FORMAT_VERSION, IR_VERSION, TOOL_VERSION};
use serde_json::{Value, json};

use crate::args::{
    Arguments, Command, DiagramFormat, DiagramGroup, IdCommand, ImpactCommand, ReportCommand,
};
use crate::envelope::Envelope;

pub fn run(arguments: Arguments) -> Result<Envelope, CompilerError> {
    match arguments.command {
        Some(Command::Build {
            root,
            output,
            format: _,
        }) => {
            let (manifest, graph) = project_graph(&root)?;
            let build = build_index(&graph, &output, &provenance(&root, &manifest))?;
            Envelope::checked(
                "build",
                !has_errors(&graph),
                json!({
                    "output": build.output,
                    "semantic_hash": build.semantic_hash,
                    "node_count": build.node_count,
                    "edge_count": build.edge_count,
                }),
                diagnostic_values(&graph.diagnostics)?,
            )
            .map_err(Into::into)
        }
        Some(Command::Check { root, format: _ }) => {
            let (_, graph) = project_graph(&root)?;
            Envelope::checked(
                "check",
                !has_errors(&graph),
                json!({
                    "node_count": graph.nodes.len(),
                    "edge_count": graph.edges.len(),
                }),
                diagnostic_values(&graph.diagnostics)?,
            )
            .map_err(Into::into)
        }
        Some(Command::Show {
            uri,
            index,
            format: _,
        }) => {
            let index = NdfIndex::open(&index)?;
            let resolved = index.resolve_uri(&uri)?;
            let node = index
                .get_node(&resolved)?
                .ok_or_else(|| CompilerError::Contract(format!("unknown NDF ID: {uri}")))?;
            Envelope::success("show", node, Vec::new()).map_err(Into::into)
        }
        Some(Command::Trace {
            uri,
            edges,
            index,
            format: _,
        }) => {
            let index = NdfIndex::open(&index)?;
            let edge_types: Vec<_> = if edges.is_empty() {
                vec!["refines"]
            } else {
                edges.iter().map(String::as_str).collect()
            };
            Envelope::success("trace", trace(&index, &uri, &edge_types)?, Vec::new())
                .map_err(Into::into)
        }
        Some(Command::Query {
            index,
            expression,
            format: _,
        }) => {
            let index = NdfIndex::open(&index)?;
            Envelope::success("query", query_nodes(&index, &expression)?, Vec::new())
                .map_err(Into::into)
        }
        Some(Command::Report { command }) => match command {
            ReportCommand::Coverage { index, format: _ } => {
                let index = NdfIndex::open(&index)?;
                Envelope::success(
                    "report coverage",
                    coverage(&index, &CoveragePolicy::default())?,
                    Vec::new(),
                )
                .map_err(Into::into)
            }
            ReportCommand::Dependencies { root, format: _ } => {
                let manifest = ProjectManifest::load(&root.join("ndf.yaml"))?;
                let lock = ProjectLock::load(&root.join("ndf.lock"))?;
                Envelope::success(
                    "report dependencies",
                    resolve_dependencies(&manifest, &lock, &root)?,
                    Vec::new(),
                )
                .map_err(Into::into)
            }
        },
        Some(Command::Diff {
            before,
            after,
            format: _,
        }) => {
            let before = NdfIndex::open(&before)?;
            let after = NdfIndex::open(&after)?;
            Envelope::success("diff", semantic_diff(&before, &after)?, Vec::new())
                .map_err(Into::into)
        }
        Some(Command::Impact { command }) => match command {
            ImpactCommand::PtoRelease {
                before,
                after,
                consumer_index,
                format: _,
            } => {
                let before = NdfIndex::open(&before)?;
                let after = NdfIndex::open(&after)?;
                let consumer = NdfIndex::open(&consumer_index)?;
                Envelope::success(
                    "impact pto-release",
                    pto_release_impact(&before, &after, &consumer)?,
                    Vec::new(),
                )
                .map_err(Into::into)
            }
        },
        Some(Command::Export {
            index,
            output,
            format: _,
        }) => {
            let index = NdfIndex::open(&index)?;
            Envelope::success("export", export_graph(&index, &output)?, Vec::new())
                .map_err(Into::into)
        }
        Some(Command::Id { command }) => match command {
            IdCommand::Allocate {
                prefix,
                index,
                format: _,
            } => {
                let mut occupied = BTreeSet::new();
                if index.exists() {
                    let index = NdfIndex::open(&index)?;
                    for node in index.all_nodes()? {
                        let id = NodeId::parse(&node.id, None)
                            .map_err(|error| CompilerError::Contract(error.to_string()))?;
                        occupied.insert(id.local_id().to_owned());
                    }
                }
                Envelope::success(
                    "id allocate",
                    json!({"id": allocate_id(&prefix, &occupied)?}),
                    Vec::new(),
                )
                .map_err(Into::into)
            }
        },
        Some(Command::Fix {
            mut paths,
            generated_only,
            check,
        }) => {
            if paths.is_empty() {
                paths = markdown_paths(&std::env::current_dir().map_err(|source| {
                    CompilerError::Read {
                        path: PathBuf::from("."),
                        source,
                    }
                })?)?;
            }
            let result = safe_fix(
                &paths,
                &FixMode {
                    generated_only,
                    check,
                    ..FixMode::default()
                },
            )?;
            Envelope::checked(
                "fix",
                result.diagnostics.is_empty(),
                json!({"changed": result.changed}),
                diagnostic_values(&result.diagnostics)?,
            )
            .map_err(Into::into)
        }
        Some(Command::Diagram {
            file,
            format,
            group,
        }) => {
            debug_assert_eq!(
                format,
                DiagramFormat::Json,
                "graph output is printed by main"
            );
            let (document, ok) = diagram_document(&file, group_by(group))?;
            Envelope::checked("diagram", ok, document, Vec::new()).map_err(Into::into)
        }
        None => Err(CompilerError::Contract("a command is required".to_owned())),
    }
}

/// The diagram of the document at `file`, and whether it is clean: built from
/// a project that loaded and carrying no error diagnostic. A project that
/// fails to load (a missing lock, a dirty dependency) yields a partial document
/// carrying the reason, so a diagram consumer shows the message where the
/// diagram would be. A file that is in no NDF project at all is the one hard
/// error.
pub fn group_by(group: DiagramGroup) -> ndf_compiler::diagram::GroupBy {
    match group {
        DiagramGroup::None => ndf_compiler::diagram::GroupBy::None,
        DiagramGroup::Refinement => ndf_compiler::diagram::GroupBy::Refinement,
        DiagramGroup::Domain => ndf_compiler::diagram::GroupBy::Domain,
    }
}

pub fn diagram_document(
    file: &Path,
    group: ndf_compiler::diagram::GroupBy,
) -> Result<(GraphDocument, bool), CompilerError> {
    let root = ndf_compiler::project::find_project_root(file).ok_or_else(|| {
        CompilerError::Contract(format!(
            "{} is not inside an NDF project (no ndf.yaml in any parent directory)",
            file.display()
        ))
    })?;
    let (manifest, graph) = match load_project_graph(&root) {
        Ok(loaded) => loaded,
        Err(error) => return Ok((failure_document(file, error.to_string()), false)),
    };
    let request = ndf_compiler::diagram::DiagramRequest {
        project_root: &root,
        manifest: &manifest,
        graph: &graph,
        file,
        group,
    };
    let document = ndf_compiler::diagram::build_diagram(&request)?;
    let clean = document
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != "error");
    Ok((document, clean))
}

fn project_graph(root: &Path) -> Result<(ProjectManifest, Graph), CompilerError> {
    load_project_graph(root)
}

fn provenance(root: &Path, manifest: &ProjectManifest) -> BuildProvenance {
    BuildProvenance {
        project_commits: vec![(manifest.project.clone(), revision(root))],
        format_version: FORMAT_VERSION.to_owned(),
        ir_version: IR_VERSION.to_owned(),
        tool_version: TOOL_VERSION.to_owned(),
        normative_roots: normative_roots(root, manifest),
    }
}

fn revision(root: &Path) -> String {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output();
    let revision = output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default();
    if revision.len() == 40 {
        revision
    } else {
        "0".repeat(40)
    }
}

fn normative_roots(root: &Path, manifest: &ProjectManifest) -> Vec<PathBuf> {
    manifest
        .roots
        .iter()
        .map(|pattern| {
            let prefix = pattern
                .find(['*', '?', '['])
                .map_or(pattern.as_str(), |index| &pattern[..index])
                .trim_end_matches('/');
            if prefix.is_empty() {
                root.to_path_buf()
            } else {
                root.join(prefix)
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn diagnostic_values(diagnostics: &[Diagnostic]) -> Result<Vec<Value>, CompilerError> {
    diagnostics
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

fn has_errors(graph: &Graph) -> bool {
    graph
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}
