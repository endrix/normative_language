//! One NDF document as a diagram: the graph document a Dialogram consumer
//! reads through its `cli-plan` acquisition.
//!
//! The unit is a document, not the project. Its clauses are the nodes, the
//! typed edges between them the connections, and a clause referenced from
//! outside the document is drawn once at its edge as an external node. The
//! shape mirrors what `graph-payload-to-doc.ts` produces, so the platform
//! consumes it without normalisation: `{ version, graph: { id, nodes, edges,
//! subgraphs, meta } }`, with `diagnostics`, `partial` and `errors` beside it.
//!
//! Every clause carries one `in` and one `out` pin because the platform draws
//! edges between ports; the pins carry no meaning of their own. Ids follow the
//! scheme every `cli-plan` producer uses: `scope:root`,
//! `node:scope:root:<LOCAL-ID>`, `port:<nodeId>:in|out`, and
//! `edge:<fromPortId>-><toPortId>:<type>`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, Graph, LifecycleStatus, Modality, Node, NodeKind,
};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::CompilerError;
use crate::manifest::ProjectManifest;

/// The document version the platform's reader expects.
pub const DOCUMENT_VERSION: &str = "1.0";
/// The scope every node of a flat document sits in.
pub const ROOT_SCOPE: &str = "scope:root";

/// Edge types whose direction carries no meaning, drawn without an arrowhead.
const UNDIRECTED_EDGE_TYPES: &[&str] = &["conflicts-with", "couples-with"];

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct GraphDocument {
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<LoadError>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<GraphDiagnostic>,
    pub graph: DiagramGraph,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct LoadError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

/// The platform's neutral diagnostic: a file, a 0-based position, a severity.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct GraphDiagnostic {
    pub uri: String,
    pub severity: &'static str,
    pub message: String,
    pub code: String,
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "startColumn")]
    pub start_column: u32,
    #[serde(rename = "endLine", skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(rename = "endColumn", skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DiagramGraph {
    pub id: String,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    pub subgraphs: Vec<DiagramScope>,
    pub meta: Map<String, Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DiagramNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    pub scope: String,
    pub ports: Vec<DiagramPort>,
    pub meta: Map<String, Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DiagramPort {
    pub id: String,
    pub name: String,
    pub direction: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DiagramEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Location>,
    pub meta: Map<String, Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct Location {
    pub file: String,
    pub line: u32,
}

/// How the clauses of a document are grouped into containers.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum GroupBy {
    /// A flat document: every clause at the root.
    #[default]
    None,
    /// One container per refinement layer (`L0`, `L1`, ...), in order.
    Refinement,
    /// One container per domain.
    Domain,
}

impl GroupBy {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "refinement" => Some(Self::Refinement),
            "domain" => Some(Self::Domain),
            _ => None,
        }
    }
}

/// What a document is built from.
pub struct DiagramRequest<'a> {
    /// The project root the graph's source paths are relative to.
    pub project_root: &'a Path,
    pub manifest: &'a ProjectManifest,
    pub graph: &'a Graph,
    /// The document file, absolute or relative to the working directory.
    pub file: &'a Path,
    /// How the document's own clauses are grouped; clauses drawn from outside
    /// the document stay at the root.
    pub group: GroupBy,
}

/// A container of nodes: a scope of kind `group` the platform draws as a
/// labelled frame around its members.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DiagramScope {
    pub id: String,
    pub kind: &'static str,
    pub label: String,
    pub nodes: Vec<String>,
}

/// A short mark on a node's header.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct Badge {
    pub text: String,
    pub tone: &'static str,
}

/// The document's path as the graph records it: relative to the project
/// root, with `/` separators.
pub fn document_relative_path(project_root: &Path, file: &Path) -> Result<String, CompilerError> {
    let root = canonical(project_root)?;
    let file = canonical(file)?;
    let relative = file.strip_prefix(&root).map_err(|_| {
        CompilerError::Contract(format!(
            "{} is outside the NDF project at {}",
            file.display(),
            root.display()
        ))
    })?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/"))
}

/// A document that stands in for one that could not be built: empty, marked
/// partial, carrying the reason.
pub fn failure_document(file: &Path, message: impl Into<String>) -> GraphDocument {
    let absolute = absolute_path(file);
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_owned());
    let mut meta = Map::new();
    meta.insert("workflow".to_owned(), Value::String(stem.clone()));
    meta.insert("file".to_owned(), Value::String(absolute.clone()));
    GraphDocument {
        version: DOCUMENT_VERSION,
        partial: Some(true),
        errors: vec![LoadError {
            message: message.into(),
            file: Some(absolute),
            line: None,
            column: None,
        }],
        diagnostics: Vec::new(),
        graph: DiagramGraph {
            id: format!("wf:{stem}"),
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            meta,
        },
    }
}

/// Build the diagram of the document at `request.file`.
pub fn build_diagram(request: &DiagramRequest<'_>) -> Result<GraphDocument, CompilerError> {
    let relative = document_relative_path(request.project_root, request.file)?;
    let graph = request.graph;
    let project = request.manifest.project.as_str();

    let document = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Document && node.source.path == relative);
    let clauses: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| node.kind != NodeKind::Document && node.source.path == relative)
        .collect();
    let document_diagnostics: Vec<&Diagnostic> = graph
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.primary.path == relative)
        .collect();
    if document.is_none() && clauses.is_empty() && document_diagnostics.is_empty() {
        return Err(CompilerError::Contract(format!(
            "{relative} is not an NDF document of project {project}"
        )));
    }

    let clause_ids: BTreeSet<&NodeId> = clauses.iter().map(|node| &node.id).collect();
    let nodes_by_id: BTreeMap<&NodeId, &Node> =
        graph.nodes.iter().map(|node| (&node.id, node)).collect();
    let diagnostics_by_subject: BTreeMap<&NodeId, Vec<&Diagnostic>> = graph
        .diagnostics
        .iter()
        .fold(BTreeMap::new(), |mut map, diagnostic| {
            if let Some(subject) = &diagnostic.subject {
                map.entry(subject).or_default().push(diagnostic);
            }
            map
        });

    let edges: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|edge| clause_ids.contains(&edge.source) || clause_ids.contains(&edge.target))
        .collect();

    // Clauses outside the document that an edge reaches, each drawn once.
    let mut foreign: BTreeSet<&NodeId> = BTreeSet::new();
    for edge in &edges {
        for end in [&edge.source, &edge.target] {
            if !clause_ids.contains(end) {
                foreign.insert(end);
            }
        }
    }

    let mut nodes = Vec::with_capacity(clauses.len() + foreign.len());
    let mut scopes: BTreeMap<String, DiagramScope> = BTreeMap::new();
    for clause in &clauses {
        let group_key = match request.group {
            GroupBy::None => None,
            GroupBy::Refinement => clause.refinement.clone(),
            GroupBy::Domain => clause.domain.clone(),
        };
        let scope = match &group_key {
            Some(key) => {
                let id = format!("scope:group:{key}");
                scopes
                    .entry(key.clone())
                    .or_insert_with(|| DiagramScope {
                        id: id.clone(),
                        kind: "group",
                        label: key.clone(),
                        nodes: Vec::new(),
                    })
                    .nodes
                    .push(node_id(&clause.id, project));
                id
            }
            None => ROOT_SCOPE.to_owned(),
        };
        nodes.push(clause_node(
            clause,
            project,
            request.project_root,
            diagnostics_by_subject.get(&clause.id).map(Vec::as_slice),
            scope,
        ));
    }
    for id in foreign {
        nodes.push(foreign_node(
            id,
            nodes_by_id.get(id).copied(),
            project,
            request.project_root,
        ));
    }

    let diagram_edges = edges
        .iter()
        .map(|edge| diagram_edge(edge, project, request.project_root))
        .collect();

    let (doc_id, title) = match document {
        Some(document) => (document.id.local_id().to_owned(), document.title.clone()),
        None => {
            let stem = request
                .file
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "document".to_owned());
            (stem.clone(), stem)
        }
    };
    let mut meta = Map::new();
    meta.insert("workflow".to_owned(), Value::String(doc_id.clone()));
    meta.insert("title".to_owned(), Value::String(title));
    meta.insert(
        "file".to_owned(),
        Value::String(absolute_in(request.project_root, &relative)),
    );
    meta.insert("project".to_owned(), Value::String(project.to_owned()));
    if let Some(document) = document {
        meta.insert("ndf".to_owned(), ndf_facts(document, true));
    }

    Ok(GraphDocument {
        version: DOCUMENT_VERSION,
        partial: None,
        errors: Vec::new(),
        diagnostics: document_diagnostics
            .iter()
            .map(|diagnostic| graph_diagnostic(diagnostic, request.project_root))
            .collect(),
        graph: DiagramGraph {
            id: format!("wf:{doc_id}"),
            nodes,
            edges: diagram_edges,
            subgraphs: scopes.into_values().collect(),
            meta,
        },
    })
}

/// The node id of a clause: its local ID inside its own project, prefixed
/// with the project for a clause of another one.
pub fn node_id(id: &NodeId, project: &str) -> String {
    if id.project_id() == project {
        format!("node:{ROOT_SCOPE}:{}", id.local_id())
    } else {
        format!("node:{ROOT_SCOPE}:{}.{}", id.project_id(), id.local_id())
    }
}

fn port_id(node_id: &str, direction: &str) -> String {
    format!("port:{node_id}:{direction}")
}

fn pins(node_id: &str) -> Vec<DiagramPort> {
    vec![
        DiagramPort {
            id: port_id(node_id, "in"),
            name: "in".to_owned(),
            direction: "in",
        },
        DiagramPort {
            id: port_id(node_id, "out"),
            name: "out".to_owned(),
            direction: "out",
        },
    ]
}

fn kind_name(kind: NodeKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{kind:?}").to_lowercase())
}

fn enum_name<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
}

fn footer(node: &Node) -> String {
    node.id.local_id().to_owned()
}

/// What a reader must see on the clause itself: its modality, its layer, and
/// its status when that is anything but active. The tone is emphasis only:
/// `must` and an unsettled `tbd` stand out, a retired or open item warns,
/// the rest is quiet.
fn badges(node: &Node) -> Vec<Badge> {
    let mut badges = Vec::new();
    if let Some(modality) = node.modality {
        let tone = match modality {
            Modality::Must => "info",
            Modality::Tbd => "warning",
            Modality::Should | Modality::May => "neutral",
        };
        badges.push(Badge {
            text: enum_name(&modality).unwrap_or_default().to_uppercase(),
            tone,
        });
    }
    if let Some(refinement) = &node.refinement {
        badges.push(Badge {
            text: refinement.clone(),
            tone: "neutral",
        });
    }
    let status_tone = match node.status {
        LifecycleStatus::Active => None,
        LifecycleStatus::Draft | LifecycleStatus::Resolved => Some("neutral"),
        LifecycleStatus::Deprecated
        | LifecycleStatus::Superseded
        | LifecycleStatus::Retired
        | LifecycleStatus::Open => Some("warning"),
    };
    if let Some(tone) = status_tone {
        badges.push(Badge {
            text: enum_name(&node.status).unwrap_or_default(),
            tone,
        });
    }
    badges
}

/// The canonical fields of a node as one object, for the property panel and
/// for anything downstream that wants them structured.
fn ndf_facts(node: &Node, with_attributes: bool) -> Value {
    let mut facts = Map::new();
    facts.insert("id".to_owned(), Value::String(node.id.uri()));
    facts.insert("kind".to_owned(), Value::String(kind_name(node.kind)));
    if let Some(modality) = node.modality.as_ref().and_then(enum_name) {
        facts.insert("modality".to_owned(), Value::String(modality));
    }
    if let Some(refinement) = &node.refinement {
        facts.insert("refinement".to_owned(), Value::String(refinement.clone()));
    }
    if let Some(domain) = &node.domain {
        facts.insert("domain".to_owned(), Value::String(domain.clone()));
    }
    if let Some(status) = enum_name(&node.status) {
        facts.insert("status".to_owned(), Value::String(status));
    }
    if let Some(owner) = &node.owner {
        facts.insert("owner".to_owned(), Value::String(owner.clone()));
    }
    if with_attributes && !node.attributes.is_empty() {
        facts.insert(
            "attributes".to_owned(),
            Value::Object(
                node.attributes
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                    .collect(),
            ),
        );
    }
    Value::Object(facts)
}

/// The facts a property panel lists for a clause, in reading order.
fn facts_list(node: &Node) -> Value {
    let mut facts = Vec::new();
    let mut push = |label: &str, value: &str| {
        facts.push(json!({ "label": label, "value": value }));
    };
    push("ID", &node.id.uri());
    push("Kind", &kind_name(node.kind));
    if let Some(modality) = node.modality.as_ref().and_then(enum_name) {
        push("Modality", &modality);
    }
    if let Some(refinement) = &node.refinement {
        push("Refinement", refinement);
    }
    if let Some(domain) = &node.domain {
        push("Domain", domain);
    }
    if let Some(status) = enum_name(&node.status) {
        push("Status", &status);
    }
    if let Some(owner) = &node.owner {
        push("Owner", owner);
    }
    for (key, value) in &node.attributes {
        push(key, value);
    }
    if !node.body.trim().is_empty() {
        facts.push(json!({ "label": "Body", "value": node.body, "markdown": true }));
    }
    Value::Array(facts)
}

/// The annotations a product's node families and the panel's Annotations
/// section read today: the kind as the family annotation, the canonical
/// fields as arguments of an `ndf` annotation, the body as text.
fn definition_annotations(node: &Node) -> Value {
    let mut arguments = Vec::new();
    let mut push = |name: &str, value: &str| {
        arguments.push(json!({ "name": name, "value": value }));
    };
    push("id", node.id.local_id());
    if let Some(modality) = node.modality.as_ref().and_then(enum_name) {
        push("modality", &modality);
    }
    if let Some(refinement) = &node.refinement {
        push("refinement", refinement);
    }
    if let Some(domain) = &node.domain {
        push("domain", domain);
    }
    if let Some(status) = enum_name(&node.status) {
        push("status", &status);
    }
    if let Some(owner) = &node.owner {
        push("owner", owner);
    }
    for (key, value) in &node.attributes {
        push(key, value);
    }
    let mut annotations = vec![
        json!({ "name": kind_name(node.kind) }),
        json!({ "name": "ndf", "arguments": arguments }),
    ];
    if !node.body.trim().is_empty() {
        annotations.push(json!({ "name": "body", "text": node.body }));
    }
    Value::Array(annotations)
}

fn clause_node(
    node: &Node,
    project: &str,
    project_root: &Path,
    diagnostics: Option<&[&Diagnostic]>,
    scope: String,
) -> DiagramNode {
    let id = node_id(&node.id, project);
    let mut meta = Map::new();
    meta.insert("badges".to_owned(), badge_values(node));
    meta.insert(
        "source".to_owned(),
        location_value(project_root, &node.source.path, node.source.line),
    );
    meta.insert(
        "definitionAnnotations".to_owned(),
        definition_annotations(node),
    );
    meta.insert("ndf".to_owned(), ndf_facts(node, true));
    meta.insert("facts".to_owned(), facts_list(node));
    if let Some(diagnostics) = diagnostics.filter(|list| !list.is_empty()) {
        let errors: Vec<&&Diagnostic> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .collect();
        meta.insert(
            "diagnostics".to_owned(),
            Value::Array(
                diagnostics
                    .iter()
                    .map(|diagnostic| {
                        json!({
                            "severity": severity_name(diagnostic.severity),
                            "code": diagnostic.code,
                            "message": diagnostic.message,
                        })
                    })
                    .collect(),
            ),
        );
        if let Some(first) = errors.first() {
            meta.insert("isErrored".to_owned(), Value::Bool(true));
            meta.insert(
                "errorMessage".to_owned(),
                Value::String(format!("{}: {}", first.code, first.message)),
            );
        }
    }
    DiagramNode {
        kind: kind_name(node.kind),
        label: node.title.clone(),
        footer: Some(footer(node)),
        scope,
        ports: pins(&id),
        meta,
        id,
    }
}

fn badge_values(node: &Node) -> Value {
    serde_json::to_value(badges(node)).unwrap_or(Value::Array(Vec::new()))
}

/// A clause outside the document: a clause of this project drawn as an
/// external node of its own kind with the file it lives in, or a target in
/// another project (or one that does not resolve) drawn as a blackbox.
fn foreign_node(
    id: &NodeId,
    resolved: Option<&Node>,
    project: &str,
    project_root: &Path,
) -> DiagramNode {
    let diagram_id = node_id(id, project);
    let mut meta = Map::new();
    meta.insert("external".to_owned(), Value::Bool(true));
    match resolved {
        Some(node) => {
            let location = location_value(project_root, &node.source.path, node.source.line);
            meta.insert("source".to_owned(), location.clone());
            meta.insert("referencedSource".to_owned(), location);
            meta.insert("badges".to_owned(), badge_values(node));
            meta.insert(
                "referencedEntityName".to_owned(),
                Value::String(node.id.local_id().to_owned()),
            );
            meta.insert(
                "definitionAnnotations".to_owned(),
                definition_annotations(node),
            );
            meta.insert("ndf".to_owned(), ndf_facts(node, true));
            meta.insert("facts".to_owned(), facts_list(node));
            DiagramNode {
                kind: kind_name(node.kind),
                label: node.title.clone(),
                footer: Some(footer(node)),
                scope: ROOT_SCOPE.to_owned(),
                ports: pins(&diagram_id),
                meta,
                id: diagram_id,
            }
        }
        None => {
            let label = if id.project_id() == project {
                id.local_id().to_owned()
            } else {
                format!("{}/{}", id.project_id(), id.local_id())
            };
            meta.insert(
                "definitionAnnotations".to_owned(),
                json!([{ "name": "blackbox" }, { "name": "ndf", "arguments": [
                    { "name": "id", "value": id.uri() },
                    { "name": "project", "value": id.project_id() }
                ] }]),
            );
            meta.insert(
                "ndf".to_owned(),
                json!({ "id": id.uri(), "project": id.project_id() }),
            );
            meta.insert(
                "facts".to_owned(),
                json!([
                    { "label": "ID", "value": id.uri() },
                    { "label": "Project", "value": id.project_id() }
                ]),
            );
            DiagramNode {
                kind: "blackbox".to_owned(),
                label,
                footer: Some(id.uri()),
                scope: ROOT_SCOPE.to_owned(),
                ports: pins(&diagram_id),
                meta,
                id: diagram_id,
            }
        }
    }
}

fn diagram_edge(edge: &Edge, project: &str, project_root: &Path) -> DiagramEdge {
    let from = port_id(&node_id(&edge.source, project), "out");
    let to = port_id(&node_id(&edge.target, project), "in");
    let mut meta = Map::new();
    meta.insert("role".to_owned(), Value::String(edge.edge_type.clone()));
    meta.insert("label".to_owned(), Value::String(edge.edge_type.clone()));
    if UNDIRECTED_EDGE_TYPES.contains(&edge.edge_type.as_str()) {
        meta.insert("undirected".to_owned(), Value::Bool(true));
    }
    if !edge.attributes.is_empty() {
        meta.insert(
            "attributes".to_owned(),
            Value::Object(
                edge.attributes
                    .iter()
                    .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                    .collect(),
            ),
        );
    }
    DiagramEdge {
        id: format!("edge:{from}->{to}:{}", edge.edge_type),
        from,
        to,
        scope: ROOT_SCOPE.to_owned(),
        source: edge.source_span.as_ref().map(|span| Location {
            file: absolute_in(project_root, &span.path),
            line: span.line,
        }),
        meta,
    }
}

fn graph_diagnostic(diagnostic: &Diagnostic, project_root: &Path) -> GraphDiagnostic {
    GraphDiagnostic {
        uri: absolute_in(project_root, &diagnostic.primary.path),
        severity: severity_name(diagnostic.severity),
        message: match &diagnostic.repair {
            Some(repair) => format!("{}. {}", diagnostic.message.trim_end_matches('.'), repair),
            None => diagnostic.message.clone(),
        },
        code: diagnostic.code.clone(),
        start_line: diagnostic.primary.line.saturating_sub(1),
        start_column: diagnostic.primary.column.saturating_sub(1),
        end_line: diagnostic
            .primary
            .end_line
            .map(|line| line.saturating_sub(1)),
        end_column: diagnostic
            .primary
            .end_column
            .map(|column| column.saturating_sub(1)),
    }
}

fn severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Information => "info",
    }
}

fn location_value(project_root: &Path, relative: &str, line: u32) -> Value {
    json!({ "file": absolute_in(project_root, relative), "line": line })
}

fn absolute_in(project_root: &Path, relative: &str) -> String {
    let mut path = canonical(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    for segment in relative.split('/') {
        path.push(segment);
    }
    path.to_string_lossy().into_owned()
}

fn absolute_path(file: &Path) -> String {
    canonical(file)
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|cwd| cwd.join(file))
                .unwrap_or_else(|_| file.to_path_buf())
        })
        .to_string_lossy()
        .into_owned()
}

fn canonical(path: &Path) -> Result<PathBuf, CompilerError> {
    path.canonicalize().map_err(|source| CompilerError::Read {
        path: path.to_path_buf(),
        source,
    })
}

/// The raw export payload a sidecar answers `exportGraph` with: the same
/// document in the pre-normalised shape `graphPayloadToDoc` reads
/// (`metadata` for `meta`, `location` for `source`, `{ portId }` endpoints).
pub fn export_payload(document: &GraphDocument) -> Value {
    let nodes: Vec<Value> = document
        .graph
        .nodes
        .iter()
        .map(|node| {
            let mut value = Map::new();
            value.insert("id".to_owned(), Value::String(node.id.clone()));
            value.insert("kind".to_owned(), Value::String(node.kind.clone()));
            value.insert("label".to_owned(), Value::String(node.label.clone()));
            if let Some(footer) = &node.footer {
                value.insert("type".to_owned(), Value::String(footer.clone()));
            }
            value.insert("scope".to_owned(), Value::String(node.scope.clone()));
            value.insert(
                "ports".to_owned(),
                serde_json::to_value(&node.ports).unwrap_or(Value::Array(Vec::new())),
            );
            value.insert("metadata".to_owned(), Value::Object(node.meta.clone()));
            Value::Object(value)
        })
        .collect();
    let edges: Vec<Value> = document
        .graph
        .edges
        .iter()
        .map(|edge| {
            let mut value = Map::new();
            value.insert("id".to_owned(), Value::String(edge.id.clone()));
            value.insert("from".to_owned(), json!({ "portId": edge.from }));
            value.insert("to".to_owned(), json!({ "portId": edge.to }));
            value.insert("scope".to_owned(), Value::String(edge.scope.clone()));
            if let Some(source) = &edge.source {
                value.insert(
                    "location".to_owned(),
                    json!({ "file": source.file, "line": source.line }),
                );
            }
            value.insert("metadata".to_owned(), Value::Object(edge.meta.clone()));
            Value::Object(value)
        })
        .collect();
    let network = document
        .graph
        .meta
        .get("workflow")
        .and_then(Value::as_str)
        .unwrap_or("document");
    json!({
        "network": network,
        "nodes": nodes,
        "edges": edges,
        "scopes": serde_json::to_value(&document.graph.subgraphs).unwrap_or(Value::Array(Vec::new())),
        "metadata": document.graph.meta,
    })
}
