use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ndf_core::identity::NodeId;
use ndf_core::model::{Diagnostic, Graph, SourceSpan};
use rusqlite::{Connection, OpenFlags, Row, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct NodeRecord {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub source: SourceSpan,
    pub modality: Option<String>,
    pub refinement: Option<String>,
    pub domain: Option<String>,
    pub status: String,
    pub owner: Option<String>,
    pub body: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct IndexedEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub source_span: Option<SourceSpan>,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct BuildRecord {
    pub format_version: String,
    pub ir_version: String,
    pub semantic_hash: String,
    pub tool_version: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticRecord {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub subject_uri: Option<String>,
    pub source_path: String,
    pub line: u32,
    pub repair: Option<String>,
}

pub(crate) struct CoverageUris {
    pub unverified: Vec<String>,
    pub open_items: Vec<String>,
    pub tbd: Vec<String>,
}

#[derive(Debug)]
pub struct NdfIndex {
    connection: Connection,
}

impl NdfIndex {
    pub fn open(path: &Path) -> Result<Self, CompilerError> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(Self { connection })
    }

    pub fn foreign_keys_enabled(&self) -> Result<bool, CompilerError> {
        Ok(self
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))?
            != 0)
    }

    pub fn get_node(&self, uri: &str) -> Result<Option<NodeRecord>, CompilerError> {
        let uri = canonical_uri(uri)?;
        let mut statement = self
            .connection
            .prepare("SELECT * FROM nodes WHERE uri = ?")?;
        let mut rows = statement.query([uri])?;
        rows.next()?
            .map(node_record)
            .transpose()
            .map_err(Into::into)
    }

    pub fn all_nodes(&self) -> Result<Vec<NodeRecord>, CompilerError> {
        self.filtered_nodes(&[])
    }

    pub(crate) fn visit_nodes(
        &self,
        mut visitor: impl FnMut(NodeRecord) -> Result<(), CompilerError>,
    ) -> Result<(), CompilerError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM nodes ORDER BY uri")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            visitor(node_record(row)?)?;
        }
        Ok(())
    }

    pub(crate) fn filtered_nodes(
        &self,
        filters: &[(&str, &str)],
    ) -> Result<Vec<NodeRecord>, CompilerError> {
        let where_clause = if filters.is_empty() {
            String::new()
        } else {
            format!(
                " WHERE {}",
                filters
                    .iter()
                    .map(|(field, _)| format!("{field} = ?"))
                    .collect::<Vec<_>>()
                    .join(" AND ")
            )
        };
        let sql = format!("SELECT * FROM nodes{where_clause} ORDER BY uri");
        let mut statement = self.connection.prepare(&sql)?;
        let values = filters.iter().map(|(_, value)| *value);
        let records = statement
            .query_map(params_from_iter(values), node_record)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    pub fn resolve_uri(&self, value: &str) -> Result<String, CompilerError> {
        if value.starts_with("ndf://") {
            return canonical_uri(value);
        }
        let mut statement = self
            .connection
            .prepare("SELECT uri FROM nodes WHERE local_id = ? ORDER BY uri")?;
        let rows = statement
            .query_map([value], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        match rows.as_slice() {
            [] => Err(CompilerError::Contract(format!("unknown NDF ID: {value}"))),
            [uri] => Ok(uri.clone()),
            _ => Err(CompilerError::Contract(format!(
                "ambiguous local NDF ID: {value}"
            ))),
        }
    }

    pub fn out_edges(
        &self,
        uri: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<IndexedEdge>, CompilerError> {
        self.edges("source_uri", uri, edge_type)
    }

    pub fn in_edges(
        &self,
        uri: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<IndexedEdge>, CompilerError> {
        self.edges("target_uri", uri, edge_type)
    }

    pub(crate) fn out_edges_by_types(
        &self,
        uri: &str,
        edge_types: &[&str],
    ) -> Result<Vec<IndexedEdge>, CompilerError> {
        let uri = canonical_uri(uri)?;
        if edge_types.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", edge_types.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT * FROM edges WHERE source_uri = ? AND edge_type IN ({placeholders}) ORDER BY source_uri, edge_type, target_uri, edge_id"
        );
        let mut values = Vec::with_capacity(edge_types.len() + 1);
        values.push(uri.as_str());
        values.extend_from_slice(edge_types);
        let mut statement = self.connection.prepare(&sql)?;
        Ok(statement
            .query_map(params_from_iter(values), edge_record)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn edges(
        &self,
        direction: &str,
        uri: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<IndexedEdge>, CompilerError> {
        let uri = canonical_uri(uri)?;
        let (sql, values) = match edge_type {
            Some(edge_type) => (
                format!(
                    "SELECT * FROM edges WHERE {direction} = ? AND edge_type = ? ORDER BY source_uri, edge_type, target_uri, edge_id"
                ),
                vec![uri.as_str(), edge_type],
            ),
            None => (
                format!(
                    "SELECT * FROM edges WHERE {direction} = ? ORDER BY source_uri, edge_type, target_uri, edge_id"
                ),
                vec![uri.as_str()],
            ),
        };
        let mut statement = self.connection.prepare(&sql)?;
        Ok(statement
            .query_map(params_from_iter(values), edge_record)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn all_edges(&self) -> Result<Vec<IndexedEdge>, CompilerError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM edges ORDER BY source_uri, edge_type, target_uri, edge_id")?;
        Ok(statement
            .query_map([], edge_record)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub(crate) fn visit_edges(
        &self,
        mut visitor: impl FnMut(IndexedEdge) -> Result<(), CompilerError>,
    ) -> Result<(), CompilerError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM edges ORDER BY source_uri, edge_type, target_uri, edge_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            visitor(edge_record(row)?)?;
        }
        Ok(())
    }

    pub fn provenance(&self) -> Result<BuildRecord, CompilerError> {
        self.connection
            .query_row(
                "SELECT format_version, ir_version, semantic_hash, tool_version FROM builds WHERE build_id = 1",
                [],
                |row| {
                    Ok(BuildRecord {
                        format_version: row.get(0)?,
                        ir_version: row.get(1)?,
                        semantic_hash: row.get(2)?,
                        tool_version: row.get(3)?,
                    })
                },
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    CompilerError::Contract("index has no build provenance".to_owned())
                }
                other => other.into(),
            })
    }

    pub fn project_revisions(&self) -> Result<BTreeMap<String, String>, CompilerError> {
        let mut statement = self
            .connection
            .prepare("SELECT project_id, revision FROM projects ORDER BY project_id")?;
        Ok(statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    }

    pub fn diagnostic_records(&self) -> Result<Vec<DiagnosticRecord>, CompilerError> {
        let mut statement = self.connection.prepare(
            "SELECT code, severity, message, subject_uri, source_path, line, repair FROM diagnostics ORDER BY diagnostic_id",
        )?;
        Ok(statement
            .query_map([], diagnostic_record)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub(crate) fn visit_diagnostics(
        &self,
        mut visitor: impl FnMut(DiagnosticRecord) -> Result<(), CompilerError>,
    ) -> Result<(), CompilerError> {
        let mut statement = self.connection.prepare(
            "SELECT code, severity, message, subject_uri, source_path, line, repair FROM diagnostics ORDER BY diagnostic_id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            visitor(diagnostic_record(row)?)?;
        }
        Ok(())
    }

    pub(crate) fn coverage_uris(&self, refinement: &str) -> Result<CoverageUris, CompilerError> {
        let unverified = self.uri_query(
            "SELECT uri FROM nodes n WHERE kind = 'requirement' AND status = 'active' AND modality = 'must' AND refinement = ? AND NOT EXISTS (SELECT 1 FROM edges e WHERE e.target_uri = n.uri AND e.edge_type = 'verifies') ORDER BY uri",
            [refinement],
        )?;
        let open_items = self.uri_query(
            "SELECT uri FROM nodes WHERE kind IN ('question', 'option') AND status = 'open' ORDER BY uri",
            [],
        )?;
        let tbd = self.uri_query(
            "SELECT uri FROM nodes WHERE modality = 'tbd' ORDER BY uri",
            [],
        )?;
        Ok(CoverageUris {
            unverified,
            open_items,
            tbd,
        })
    }

    fn uri_query<const N: usize>(
        &self,
        sql: &str,
        parameters: [&str; N],
    ) -> Result<Vec<String>, CompilerError> {
        let mut statement = self.connection.prepare(sql)?;
        Ok(statement
            .query_map(params_from_iter(parameters), |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn counts(&self) -> Result<(usize, usize), CompilerError> {
        let nodes: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;
        let edges: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
        Ok((nodes as usize, edges as usize))
    }
}

fn canonical_uri(value: &str) -> Result<String, CompilerError> {
    NodeId::parse(value, None)
        .map(|id| id.to_string())
        .map_err(|error| CompilerError::Contract(error.to_string()))
}

fn node_record(row: &Row<'_>) -> rusqlite::Result<NodeRecord> {
    Ok(NodeRecord {
        id: row.get("uri")?,
        kind: row.get("kind")?,
        title: row.get("title")?,
        source: SourceSpan {
            path: row.get("source_path")?,
            line: row.get("line")?,
            column: row.get("column_number")?,
            end_line: None,
            end_column: None,
        },
        modality: row.get("modality")?,
        refinement: row.get("refinement")?,
        domain: row.get("domain")?,
        status: row.get("status")?,
        owner: row.get("owner")?,
        body: row.get("body")?,
        attributes: BTreeMap::new(),
    })
}

fn edge_record(row: &Row<'_>) -> rusqlite::Result<IndexedEdge> {
    let source_path: Option<String> = row.get("source_path")?;
    let source_span = match source_path {
        Some(path) => Some(SourceSpan {
            path,
            line: row.get("line")?,
            column: 1,
            end_line: None,
            end_column: None,
        }),
        None => None,
    };
    Ok(IndexedEdge {
        source: row.get("source_uri")?,
        target: row.get("target_uri")?,
        edge_type: row.get("edge_type")?,
        source_span,
        attributes: BTreeMap::new(),
    })
}

fn diagnostic_record(row: &Row<'_>) -> rusqlite::Result<DiagnosticRecord> {
    Ok(DiagnosticRecord {
        code: row.get(0)?,
        severity: row.get(1)?,
        message: row.get(2)?,
        subject_uri: row.get(3)?,
        source_path: row.get(4)?,
        line: row.get(5)?,
        repair: row.get(6)?,
    })
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
