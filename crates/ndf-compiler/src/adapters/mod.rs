use std::collections::BTreeMap;
use std::path::Path;

use globset::{Glob, GlobSetBuilder};
use ndf_core::graph::GraphBuilder;
use ndf_core::identity::NodeId;
use ndf_core::model::{Diagnostic, DiagnosticSeverity, Graph, ParseResult, SourceSpan};
use walkdir::WalkDir;

use crate::CompilerError;
use crate::manifest::ProjectManifest;
use crate::markdown::parse_markdown;

pub mod evidence;
pub mod pto_asl;
pub mod repository;

pub trait Adapter: Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError>;
}

struct MarkdownAdapter;
struct PtoAslAdapter;
struct EvidenceAdapter;

impl Adapter for MarkdownAdapter {
    fn name(&self) -> &'static str {
        "markdown"
    }

    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError> {
        parse_markdown(path, manifest)
    }
}

impl Adapter for PtoAslAdapter {
    fn name(&self) -> &'static str {
        "pto-asl"
    }

    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError> {
        pto_asl::parse_pto_asl(path, manifest)
    }
}

impl Adapter for EvidenceAdapter {
    fn name(&self) -> &'static str {
        "evidence"
    }

    fn parse(&self, path: &Path, manifest: &ProjectManifest) -> Result<ParseResult, CompilerError> {
        evidence::parse_evidence_manifest(path, manifest)
    }
}

static MARKDOWN: MarkdownAdapter = MarkdownAdapter;
static PTO_ASL: PtoAslAdapter = PtoAslAdapter;
static EVIDENCE: EvidenceAdapter = EvidenceAdapter;

pub fn adapter_for(path: &Path) -> Option<&'static dyn Adapter> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("md") => Some(&MARKDOWN),
        Some("asl") => Some(&PTO_ASL),
        Some("json") => Some(&EVIDENCE),
        _ => None,
    }
}

pub fn parse_project(manifest: &ProjectManifest) -> Result<Graph, CompilerError> {
    let mut globs = GlobSetBuilder::new();
    for pattern in &manifest.roots {
        globs.add(Glob::new(pattern).map_err(|error| {
            CompilerError::Contract(format!("invalid project root glob {pattern}: {error}"))
        })?);
    }
    let globs = globs
        .build()
        .map_err(|error| CompilerError::Contract(format!("invalid project root globs: {error}")))?;
    let mut paths = Vec::new();
    for entry in WalkDir::new(&manifest.root) {
        let entry = entry.map_err(|error| {
            CompilerError::Contract(format!(
                "failed to walk project {}: {error}",
                manifest.root.display()
            ))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&manifest.root)
            .expect("walked path is under project root");
        if globs.is_match(relative) {
            paths.push(entry.path().to_path_buf());
        }
    }
    paths.sort();

    let mut graph = GraphBuilder::default();
    let mut instruction_identities: BTreeMap<NodeId, (String, String, SourceSpan)> =
        BTreeMap::new();
    for path in paths {
        let adapter = adapter_for(&path).ok_or_else(|| {
            CompilerError::Contract(format!(
                "no built-in adapter for project source: {}",
                path.display()
            ))
        })?;
        let parsed = adapter.parse(&path, manifest)?;
        for node in parsed.nodes {
            if node.id.local_id().starts_with("PTO-INST-")
                && let (Some(surface), Some(mnemonic)) = (
                    node.attributes.get("surface"),
                    node.attributes.get("mnemonic"),
                )
            {
                if let Some((existing_surface, existing_mnemonic, existing_source)) =
                    instruction_identities.get(&node.id)
                    && (existing_surface != surface || existing_mnemonic != mnemonic)
                {
                    graph.add_diagnostic(Diagnostic {
                        code: "NDF-ASL-005".to_owned(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "conflicting PTO instruction identity {}: {existing_surface} {existing_mnemonic} and {surface} {mnemonic}",
                            node.id
                        ),
                        primary: node.source.clone(),
                        related: vec![existing_source.clone()],
                        repair: Some(
                            "Use distinct surface and mnemonic pairs that derive distinct stable identities."
                                .to_owned(),
                        ),
                        subject: Some(node.id.clone()),
                    });
                    continue;
                }
                instruction_identities
                    .entry(node.id.clone())
                    .or_insert_with(|| (surface.clone(), mnemonic.clone(), node.source.clone()));
            }
            graph.add_node(node);
        }
        for edge in parsed.edges {
            graph.add_edge(edge);
        }
        for diagnostic in parsed.diagnostics {
            graph.add_diagnostic(diagnostic);
        }
    }
    Ok(graph.finish())
}
