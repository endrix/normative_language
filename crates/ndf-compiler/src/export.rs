use std::collections::BTreeMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use ndf_core::model::SourceSpan;
use serde::Serialize;

use crate::CompilerError;
use crate::index::{BuildRecord, DiagnosticRecord, IndexedEdge, NdfIndex, NodeRecord};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ExportResult {
    pub output: PathBuf,
    pub semantic_hash: String,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Serialize)]
struct ExportProvenance<'a> {
    format_version: &'a str,
    ir_version: &'a str,
    semantic_hash: &'a str,
    tool_version: &'a str,
    projects: &'a BTreeMap<String, String>,
}

#[derive(Serialize)]
struct ExportDiagnostic {
    code: String,
    severity: String,
    message: String,
    primary: SourceSpan,
    related: Vec<SourceSpan>,
    repair: Option<String>,
    subject: Option<String>,
}

pub fn export_graph(index: &NdfIndex, output: &Path) -> Result<ExportResult, CompilerError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CompilerError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let provenance = index.provenance()?;
    let projects = index.project_revisions()?;
    let (node_count, edge_count) = index.counts()?;
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(
            ".{}.",
            output.file_name().unwrap_or_default().to_string_lossy()
        ))
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|source| CompilerError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        write_bytes(
            &mut writer,
            output,
            b"{\n  \"schema_version\": \"0.1\",\n  \"nodes\": [",
        )?;
        let mut first = true;
        index.visit_nodes(|node| {
            write_record(&mut writer, output, &mut first, &portable_node(node))
        })?;
        write_bytes(&mut writer, output, b"\n  ],\n  \"edges\": [")?;
        let mut first = true;
        index.visit_edges(|edge| {
            write_record(&mut writer, output, &mut first, &portable_edge(edge))
        })?;
        write_bytes(&mut writer, output, b"\n  ],\n  \"diagnostics\": [")?;
        let mut first = true;
        index.visit_diagnostics(|diagnostic| {
            write_record(
                &mut writer,
                output,
                &mut first,
                &portable_diagnostic(diagnostic),
            )
        })?;
        write_bytes(&mut writer, output, b"\n  ],\n  \"provenance\": ")?;
        serde_json::to_writer_pretty(&mut writer, &ExportProvenance::new(&provenance, &projects))?;
        write_bytes(&mut writer, output, b"\n}\n")?;
        writer.flush().map_err(|source| CompilerError::Write {
            path: output.to_path_buf(),
            source,
        })?;
    }
    temporary
        .persist(output)
        .map_err(|error| CompilerError::Write {
            path: output.to_path_buf(),
            source: error.error,
        })?;
    Ok(ExportResult {
        output: output.to_path_buf(),
        semantic_hash: provenance.semantic_hash,
        node_count,
        edge_count,
    })
}

impl<'a> ExportProvenance<'a> {
    fn new(record: &'a BuildRecord, projects: &'a BTreeMap<String, String>) -> Self {
        Self {
            format_version: &record.format_version,
            ir_version: &record.ir_version,
            semantic_hash: &record.semantic_hash,
            tool_version: &record.tool_version,
            projects,
        }
    }
}

fn write_record<T: Serialize>(
    writer: &mut impl Write,
    output: &Path,
    first: &mut bool,
    record: &T,
) -> Result<(), CompilerError> {
    if !*first {
        write_bytes(writer, output, b",")?;
    }
    *first = false;
    write_bytes(writer, output, b"\n    ")?;
    serde_json::to_writer(&mut *writer, record)?;
    Ok(())
}

fn write_bytes(writer: &mut impl Write, output: &Path, bytes: &[u8]) -> Result<(), CompilerError> {
    writer
        .write_all(bytes)
        .map_err(|source| CompilerError::Write {
            path: output.to_path_buf(),
            source,
        })
}

fn portable_path(value: &str) -> String {
    let path = Path::new(value);
    if path.is_absolute() {
        return format!(
            "<external>/{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    value.replace('\\', "/")
}

fn portable_span(mut span: SourceSpan) -> SourceSpan {
    span.path = portable_path(&span.path);
    span
}

fn portable_node(mut node: NodeRecord) -> NodeRecord {
    node.source = portable_span(node.source);
    node
}

fn portable_edge(mut edge: IndexedEdge) -> IndexedEdge {
    edge.source_span = edge.source_span.map(portable_span);
    edge
}

fn portable_diagnostic(record: DiagnosticRecord) -> ExportDiagnostic {
    ExportDiagnostic {
        code: record.code,
        severity: record.severity,
        message: record.message,
        primary: SourceSpan {
            path: portable_path(&record.source_path),
            line: record.line,
            column: 1,
            end_line: None,
            end_column: None,
        },
        related: Vec::new(),
        repair: record.repair,
        subject: record.subject_uri,
    }
}
