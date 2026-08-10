use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_core::model::{Diagnostic, Graph};
use rusqlite::{Connection, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::CompilerError;
use crate::baseline::diagnostic_fingerprint;

const SCHEMA: &str = include_str!("../../../sql/schema-v1.sql");

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BuildProvenance {
    pub project_commits: Vec<(String, String)>,
    pub format_version: String,
    pub ir_version: String,
    pub tool_version: String,
    pub normative_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IndexBuild {
    pub output: PathBuf,
    pub semantic_hash: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct NdfIndex {
    connection: Connection,
}

impl NdfIndex {
    pub fn open(path: &Path) -> Result<Self, CompilerError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(Self { connection })
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, CompilerError> {
        Ok(self
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))?
            != 0)
    }
}

pub fn build_index(
    graph: &Graph,
    output: &Path,
    provenance: &BuildProvenance,
) -> Result<IndexBuild, CompilerError> {
    validate_build(graph, output, provenance)?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CompilerError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let canonical = canonical_graph(graph);
    let semantic_hash = semantic_hash(&canonical, provenance)?;
    let temporary = tempfile::Builder::new()
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
    let temporary_path = temporary.into_temp_path();
    {
        let mut connection = Connection::open(&temporary_path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.execute_batch(SCHEMA)?;
        let transaction = connection.transaction()?;
        insert_graph(&transaction, &canonical, provenance, &semantic_hash)?;
        let violations: i64 =
            transaction.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if violations != 0 {
            return Err(CompilerError::Contract(format!(
                "SQLite foreign key violations: {violations}"
            )));
        }
        transaction.commit()?;
    }
    temporary_path
        .persist(output)
        .map_err(|error| CompilerError::Write {
            path: output.to_path_buf(),
            source: error.error,
        })?;
    Ok(IndexBuild {
        output: output.to_path_buf(),
        semantic_hash,
        node_count: canonical.nodes.len(),
        edge_count: canonical.edges.len(),
        diagnostics: canonical.diagnostics,
    })
}

fn validate_build(
    graph: &Graph,
    output: &Path,
    provenance: &BuildProvenance,
) -> Result<(), CompilerError> {
    let projects: BTreeSet<_> = provenance
        .project_commits
        .iter()
        .map(|(project, _)| project.as_str())
        .collect();
    let mut missing: Vec<_> = graph
        .nodes
        .iter()
        .map(|node| node.id.project_id())
        .filter(|project| !projects.contains(project))
        .collect();
    missing.sort_unstable();
    missing.dedup();
    if let Some(project) = missing.first() {
        return Err(CompilerError::Contract(format!(
            "missing project revision for: {project}"
        )));
    }
    let resolved_output = absolute_path(output)?;
    for root in &provenance.normative_roots {
        let resolved_root = absolute_path(root)?;
        if resolved_output.starts_with(&resolved_root) {
            return Err(CompilerError::Contract(format!(
                "refusing to write SQLite index inside normative root: {}",
                root.display()
            )));
        }
    }
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf, CompilerError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|source| CompilerError::Read {
            path: path.to_path_buf(),
            source,
        })
}

fn canonical_graph(graph: &Graph) -> Graph {
    let mut canonical = graph.clone();
    canonical
        .nodes
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.edges.sort_by(|left, right| {
        (&left.source, &left.edge_type, &left.target).cmp(&(
            &right.source,
            &right.edge_type,
            &right.target,
        ))
    });
    canonical.diagnostics.sort_by(|left, right| {
        (
            &left.primary.path,
            left.primary.line,
            &left.code,
            left.subject.as_ref(),
        )
            .cmp(&(
                &right.primary.path,
                right.primary.line,
                &right.code,
                right.subject.as_ref(),
            ))
    });
    canonical
}

fn semantic_hash(graph: &Graph, provenance: &BuildProvenance) -> Result<String, CompilerError> {
    let mut project_commits = provenance.project_commits.clone();
    project_commits.sort();
    let payload = json!({
        "format_version": provenance.format_version,
        "graph": graph,
        "ir_version": provenance.ir_version,
        "project_commits": project_commits,
        "tool_version": provenance.tool_version,
    });
    let encoded = serde_json::to_vec(&payload)?;
    let digest = Sha256::digest(encoded);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn insert_graph(
    transaction: &Transaction<'_>,
    graph: &Graph,
    provenance: &BuildProvenance,
    semantic_hash: &str,
) -> Result<(), CompilerError> {
    {
        let mut statement =
            transaction.prepare("INSERT INTO projects(project_id, revision) VALUES (?, ?)")?;
        let mut projects = provenance.project_commits.clone();
        projects.sort();
        for (project, revision) in projects {
            statement.execute(params![project, revision])?;
        }
    }
    {
        let mut statement = transaction.prepare("INSERT INTO sources(path) VALUES (?)")?;
        for path in source_paths(graph) {
            statement.execute(params![path])?;
        }
    }
    {
        let mut node_statement = transaction.prepare(
            "INSERT INTO nodes(uri, project_id, local_id, kind, title, source_path, line, column_number, modality, refinement, domain, status, owner, body) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        let mut attribute_statement =
            transaction.prepare("INSERT INTO node_attrs(node_uri, key, value) VALUES (?, ?, ?)")?;
        for node in &graph.nodes {
            node_statement.execute(params![
                node.id.to_string(),
                node.id.project_id(),
                node.id.local_id(),
                enum_text(node.kind)?,
                node.title,
                node.source.path,
                node.source.line,
                node.source.column,
                optional_enum_text(node.modality)?,
                node.refinement,
                node.domain,
                enum_text(node.status)?,
                node.owner,
                node.body,
            ])?;
            for (key, value) in &node.attributes {
                attribute_statement.execute(params![node.id.to_string(), key, value])?;
            }
        }
    }
    {
        let mut statement = transaction.prepare(
            "INSERT INTO edges(edge_id, source_uri, edge_type, target_uri, source_path, line, attributes_json) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )?;
        for (index, edge) in graph.edges.iter().enumerate() {
            statement.execute(params![
                (index + 1) as i64,
                edge.source.to_string(),
                edge.edge_type,
                edge.target.to_string(),
                edge.source_span.as_ref().map(|span| span.path.as_str()),
                edge.source_span.as_ref().map(|span| span.line),
                serde_json::to_string(&edge.attributes)?,
            ])?;
        }
    }
    {
        let mut statement = transaction.prepare(
            "INSERT INTO diagnostics(diagnostic_id, fingerprint, code, severity, message, subject_uri, source_path, line, repair) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        for (index, diagnostic) in graph.diagnostics.iter().enumerate() {
            statement.execute(params![
                (index + 1) as i64,
                diagnostic_fingerprint(diagnostic),
                diagnostic.code,
                enum_text(diagnostic.severity)?,
                diagnostic.message,
                diagnostic.subject.as_ref().map(ToString::to_string),
                diagnostic.primary.path,
                diagnostic.primary.line,
                diagnostic.repair,
            ])?;
        }
    }
    transaction.execute(
        "INSERT INTO builds(build_id, schema_version, semantic_hash, format_version, ir_version, tool_version) VALUES (1, '1', ?, ?, ?, ?)",
        params![
            semantic_hash,
            provenance.format_version,
            provenance.ir_version,
            provenance.tool_version,
        ],
    )?;
    Ok(())
}

fn source_paths(graph: &Graph) -> BTreeSet<&str> {
    let mut paths = BTreeSet::new();
    for node in &graph.nodes {
        paths.insert(node.source.path.as_str());
    }
    for edge in &graph.edges {
        if let Some(span) = &edge.source_span {
            paths.insert(span.path.as_str());
        }
    }
    for diagnostic in &graph.diagnostics {
        paths.insert(diagnostic.primary.path.as_str());
        for span in &diagnostic.related {
            paths.insert(span.path.as_str());
        }
    }
    paths
}

fn enum_text<T: Serialize>(value: T) -> Result<String, CompilerError> {
    let Value::String(value) = serde_json::to_value(value)? else {
        return Err(CompilerError::Contract(
            "enum did not serialize as a string".to_owned(),
        ));
    };
    Ok(value)
}

fn optional_enum_text<T: Serialize>(value: Option<T>) -> Result<Option<String>, CompilerError> {
    value.map(enum_text).transpose()
}
