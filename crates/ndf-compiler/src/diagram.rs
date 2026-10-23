//! An NDF project as diagrams: the graph documents a Dialogram consumer
//! reads through its `cli-plan` acquisition.
//!
//! The unit shown is a document, not the project. Its clauses are the nodes,
//! the typed edges between them the connections, and a clause referenced from
//! outside the document is drawn once at its edge, navigable into the
//! document (or the dependency checkout) that holds it. Two more units share
//! the same shape: the project itself, drawn as one node per document with
//! the edges between documents added up, and a trace, which follows one edge
//! type from a document or a clause through the whole project. Every
//! document carries the outline of the project, so a consumer lists its
//! documents without a second export.
//!
//! The shape mirrors what `graph-payload-to-doc.ts` produces, so the platform
//! consumes it without normalisation: `{ version, graph: { id, nodes, edges,
//! subgraphs, meta }, hierarchy }`, with `diagnostics`, `partial` and `errors`
//! beside it. Every clause carries one `in` and one `out` pin because the
//! platform draws edges between ports; the pins carry no meaning of their
//! own. Ids follow the scheme every `cli-plan` producer uses: `scope:root`,
//! `node:scope:root:<LOCAL-ID>`, `port:<nodeId>:in|out`, and
//! `edge:<fromPortId>-><toPortId>:<type>`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, Graph, LifecycleStatus, Modality, Node, NodeKind,
};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::CompilerError;
use crate::adapters::pto_asl::{
    asl_declarations, display_title, implied_definitions, instruction_contract,
};
use crate::dependencies::DependencyGraph;
use crate::diagram_links::{
    FileIndex, LinkTarget, has_file_extension, link_markdown, link_path, parse_file_ref,
    symbol_line,
};
use crate::manifest::ProjectManifest;

/// The document version the platform's reader expects.
pub const DOCUMENT_VERSION: &str = "1.0";
/// The scope every node of a flat document sits in.
pub const ROOT_SCOPE: &str = "scope:root";
/// The file that names a project; the project overview is its diagram.
pub const MANIFEST_FILE: &str = "ndf.yaml";
/// How many documents the outline lists before it says it stopped.
pub const OUTLINE_BUDGET: usize = 2000;

/// The views offered of every unit besides its structure: a trace along one
/// edge type, as `(id, label)`. The id is the edge type the trace follows.
pub const VIEWS: &[(&str, &str)] = &[
    ("refines", "Refinement trace"),
    ("verifies", "Verification"),
    ("depends-on", "Dependencies"),
];

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
    /// The outline of the project this unit belongs to: the root is the
    /// project, one child per document. Absent from a failure document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchy: Option<HierarchyEntry>,
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

/// One entry of the project outline, in the shape a consumer's hierarchy
/// outline reads: the project at the root (`path: []`), one child per
/// document, named by its document id.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct HierarchyEntry {
    pub path: Vec<String>,
    #[serde(rename = "workflowName")]
    pub workflow_name: String,
    #[serde(rename = "sourceUri")]
    pub source_uri: String,
    #[serde(rename = "nodeCount", skip_serializing_if = "Option::is_none")]
    pub node_count: Option<usize>,
    /// `group` for a folder or a layer (a heading, not a view), `node` for a
    /// clause (found in its document's view); none for a document or project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<&'static str>,
    /// What the row says: a document's title, a clause's id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Said after the label: a document's id, a clause's title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// A clause's kind, which the panel colours as the diagram does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<Badge>,
    /// A clause row's node, as `ndf diagram` names it in its document.
    #[serde(rename = "nodeId", skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    pub children: Vec<HierarchyEntry>,
}

impl HierarchyEntry {
    fn row(path: Vec<String>, workflow_name: String, source_uri: String) -> Self {
        Self {
            path,
            workflow_name,
            source_uri,
            node_count: None,
            role: None,
            label: None,
            detail: None,
            family: None,
            badges: Vec::new(),
            node_id: None,
            truncated: false,
            children: Vec::new(),
        }
    }

    /// A heading over other rows: transparent to paths, so its children's
    /// paths are written as if it were not there.
    fn group(path: Vec<String>, label: &str, children: Vec<HierarchyEntry>) -> Self {
        Self {
            role: Some("group"),
            label: Some(label.to_owned()),
            children,
            ..Self::row(path, label.to_owned(), String::new())
        }
    }
}

/// How the clauses of a unit are grouped into containers.
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

/// What a diagram is built from.
pub struct DiagramRequest<'a> {
    /// The project root the graph's source paths are relative to.
    pub project_root: &'a Path,
    pub manifest: &'a ProjectManifest,
    pub graph: &'a Graph,
    /// The dependency graphs the project resolved, by project id, each with
    /// the checkout it was read from. A target in one of them is drawn as the
    /// clause it is, with its source in that checkout.
    pub dependencies: &'a BTreeMap<String, DependencyGraph>,
    /// The file asked for, absolute or relative to the working directory:
    /// a document, or the project's `ndf.yaml` for the project overview.
    pub file: &'a Path,
    /// How the unit's clauses are grouped; clauses drawn from outside the
    /// unit stay at the root.
    pub group: GroupBy,
    /// The unit by name, when the consumer names one: the project (its id),
    /// a document (its id or title) or a clause (its id). Absent, the unit is
    /// the document at `file` (or the project when `file` is its manifest).
    pub workflow: Option<&'a str>,
    /// A view of the unit other than its structure: the edge type to trace
    /// (one of `VIEWS`, though any edge type traces).
    pub view: Option<&'a str>,
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
    let stem = file_stem(file);
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
        hierarchy: None,
    }
}

/// Build the diagram the request asks for.
pub fn build_diagram(request: &DiagramRequest<'_>) -> Result<GraphDocument, CompilerError> {
    let relative = document_relative_path(request.project_root, request.file)?;
    let project = Project::new(request)?;
    let unit = project.select_unit(&relative, request.workflow)?;
    match request.view {
        Some(edge_type) => project.trace_view(&unit, edge_type, request.group),
        None => match &unit {
            Unit::Project => project.project_view(),
            Unit::Document(document) => project.document_view(document, request.group),
            Unit::Clause(_, document) => project.document_view(document, request.group),
        },
    }
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

/// The views a consumer may ask for, as `meta.views` lists them.
pub fn views_meta() -> Value {
    Value::Array(
        VIEWS
            .iter()
            .map(|(id, label)| json!({ "id": id, "label": label }))
            .collect(),
    )
}

/// The label of a view, or the edge type itself for one not in `VIEWS`.
pub fn view_label(edge_type: &str) -> String {
    VIEWS
        .iter()
        .find(|(id, _)| *id == edge_type)
        .map(|(_, label)| (*label).to_owned())
        .unwrap_or_else(|| edge_type.to_owned())
}

// ---------------------------------------------------------------------------
// The project as the diagrams see it
// ---------------------------------------------------------------------------

/// One document of the project: a file with a `document` node, or a file
/// whose clauses have no document node (a PTO ASL unit file), which goes by
/// its file stem.
#[derive(Debug, Clone)]
struct DocumentInfo<'a> {
    id: String,
    title: String,
    /// Relative to the project root, `/` separated.
    path: String,
    node: Option<&'a Node>,
    clause_count: usize,
}

/// The unit a request names.
enum Unit<'a> {
    Project,
    Document(DocumentInfo<'a>),
    Clause(&'a Node, DocumentInfo<'a>),
}

/// Where a node resolves to.
enum Target<'a> {
    Local {
        node: &'a Node,
        document: &'a DocumentInfo<'a>,
    },
    Dependency {
        node: &'a Node,
        root: &'a Path,
        document: &'a DocumentInfo<'a>,
    },
    Unresolved,
}

/// How a node is drawn relative to the unit shown.
struct Placement {
    /// Drawn as an external node: a clause the unit refers to but does not own.
    external: bool,
    /// The document a double-click opens, when the node lives in another one.
    navigate: Option<Navigation>,
}

struct Navigation {
    /// The id of the document a double-click opens.
    document: String,
    /// The project the document belongs to, when not the one shown.
    project: Option<String>,
    /// Where the clause is, absolute.
    file: String,
    line: u32,
}

struct Project<'a> {
    root: PathBuf,
    id: &'a str,
    graph: &'a Graph,
    nodes_by_id: BTreeMap<&'a NodeId, &'a Node>,
    documents: Vec<DocumentInfo<'a>>,
    document_by_path: BTreeMap<String, usize>,
    dependencies: BTreeMap<&'a str, Dependency<'a>>,
    diagnostics_by_subject: BTreeMap<&'a NodeId, Vec<&'a Diagnostic>>,
    /// The edges a reader sees that the graph leaves implicit (a clause
    /// written in an ASL unit is defined in it); numbered after the graph's.
    implied_edges: Vec<Edge>,
    /// Every edge touching a node, by either end.
    edges_by_end: BTreeMap<&'a NodeId, Vec<usize>>,
    /// The project's files, for the ones its clauses name.
    files: FileIndex,
    /// The domains the manifest declares, offered when a domain is edited.
    domains: Vec<String>,
    /// The values the project's clauses already use, offered as suggestions.
    used: UsedValues,
    /// How the diagram groups clauses, which the outline follows.
    group: GroupBy,
}

/// The values a project's clauses already give the free-text fields.
#[derive(Default)]
struct UsedValues {
    refinements: BTreeSet<String>,
    domains: BTreeSet<String>,
    owners: BTreeSet<String>,
}

struct Dependency<'a> {
    root: &'a Path,
    nodes_by_id: BTreeMap<&'a NodeId, &'a Node>,
    documents: Vec<DocumentInfo<'a>>,
    document_by_path: BTreeMap<String, usize>,
}

impl<'a> Project<'a> {
    fn new(request: &DiagramRequest<'a>) -> Result<Self, CompilerError> {
        let graph = request.graph;
        let documents = documents_of(graph);
        let document_by_path = documents
            .iter()
            .enumerate()
            .map(|(index, document)| (document.path.clone(), index))
            .collect();
        let mut edges_by_end: BTreeMap<&NodeId, Vec<usize>> = BTreeMap::new();
        for (index, edge) in graph.edges.iter().enumerate() {
            edges_by_end.entry(&edge.source).or_default().push(index);
            if edge.target != edge.source {
                edges_by_end.entry(&edge.target).or_default().push(index);
            }
        }
        let mut implied_edges = Vec::new();
        for (clause, unit) in implied_definitions(graph) {
            let index = graph.edges.len() + implied_edges.len();
            edges_by_end.entry(&clause.id).or_default().push(index);
            edges_by_end.entry(&unit.id).or_default().push(index);
            implied_edges.push(
                Edge::new(
                    clause.id.clone(),
                    unit.id.clone(),
                    "defined-in",
                    Some(clause.source.clone()),
                )
                .expect("non-empty built-in edge type"),
            );
        }
        let dependencies = request
            .dependencies
            .iter()
            .map(|(project_id, dependency)| {
                let documents = documents_of(&dependency.graph);
                let document_by_path = documents
                    .iter()
                    .enumerate()
                    .map(|(index, document)| (document.path.clone(), index))
                    .collect();
                (
                    project_id.as_str(),
                    Dependency {
                        root: dependency.path.as_path(),
                        nodes_by_id: dependency
                            .graph
                            .nodes
                            .iter()
                            .map(|node| (&node.id, node))
                            .collect(),
                        documents,
                        document_by_path,
                    },
                )
            })
            .collect();
        let root = canonical(request.project_root)?;
        let mut used = UsedValues::default();
        for node in &graph.nodes {
            used.refinements.extend(node.refinement.clone());
            used.domains.extend(node.domain.clone());
            used.owners.extend(node.owner.clone());
        }
        Ok(Self {
            files: FileIndex::new(&root),
            domains: request.manifest.domains.clone(),
            used,
            group: request.group,
            root,
            id: request.manifest.project.as_str(),
            graph,
            nodes_by_id: graph.nodes.iter().map(|node| (&node.id, node)).collect(),
            documents,
            document_by_path,
            dependencies,
            diagnostics_by_subject: graph.diagnostics.iter().fold(
                BTreeMap::new(),
                |mut map, diagnostic| {
                    if let Some(subject) = &diagnostic.subject {
                        map.entry(subject).or_default().push(diagnostic);
                    }
                    map
                },
            ),
            implied_edges,
            edges_by_end,
        })
    }

    // -- units ---------------------------------------------------------------

    fn select_unit(
        &self,
        relative: &str,
        workflow: Option<&str>,
    ) -> Result<Unit<'a>, CompilerError> {
        if let Some(name) = workflow.map(str::trim).filter(|name| !name.is_empty()) {
            if name == self.id {
                return Ok(Unit::Project);
            }
            if let Some(document) = self
                .documents
                .iter()
                .find(|document| document.id == name)
                .or_else(|| {
                    self.documents
                        .iter()
                        .find(|document| document.title == name)
                })
            {
                return Ok(Unit::Document(document.clone()));
            }
            if let Some(node) = self.graph.nodes.iter().find(|node| {
                node.kind != NodeKind::Document
                    && node.id.project_id() == self.id
                    && node.id.local_id() == name
            }) {
                let document = self
                    .document_of(node)
                    .cloned()
                    .unwrap_or_else(|| stem_document(&node.source.path, std::iter::once(node)));
                return Ok(Unit::Clause(node, document));
            }
            return Err(CompilerError::Contract(format!(
                "no document or clause named {name} in project {}",
                self.id
            )));
        }
        if relative == MANIFEST_FILE {
            return Ok(Unit::Project);
        }
        if let Some(&index) = self.document_by_path.get(relative) {
            return Ok(Unit::Document(self.documents[index].clone()));
        }
        // A file with nothing in it but diagnostics (a document that did not
        // parse) is still a document: an empty one that shows why.
        if self
            .graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.primary.path == relative)
        {
            return Ok(Unit::Document(stem_document(relative, std::iter::empty())));
        }
        Err(CompilerError::Contract(format!(
            "{relative} is not an NDF document of project {}",
            self.id
        )))
    }

    fn document_of(&self, node: &Node) -> Option<&DocumentInfo<'a>> {
        self.document_by_path
            .get(&node.source.path)
            .map(|&index| &self.documents[index])
    }

    fn resolve(&self, id: &NodeId) -> Target<'_> {
        if id.project_id() == self.id {
            return match self.nodes_by_id.get(id) {
                Some(node) => match self.document_of(node) {
                    Some(document) => Target::Local { node, document },
                    None => Target::Unresolved,
                },
                None => Target::Unresolved,
            };
        }
        let Some(dependency) = self.dependencies.get(id.project_id()) else {
            return Target::Unresolved;
        };
        match dependency.nodes_by_id.get(id) {
            Some(node) => match dependency
                .document_by_path
                .get(&node.source.path)
                .map(|&index| &dependency.documents[index])
            {
                Some(document) => Target::Dependency {
                    node,
                    root: dependency.root,
                    document,
                },
                None => Target::Unresolved,
            },
            None => Target::Unresolved,
        }
    }

    // -- the document view ---------------------------------------------------

    /// An edge by its number: the graph's, then the implied ones.
    fn edge(&self, index: usize) -> &Edge {
        self.graph
            .edges
            .get(index)
            .unwrap_or_else(|| &self.implied_edges[index - self.graph.edges.len()])
    }

    /// The graph's edges, then the implied ones.
    fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.graph.edges.iter().chain(&self.implied_edges)
    }

    /// One document: its clauses, the typed edges between them, and every
    /// clause outside it that an edge reaches, drawn once as an external node.
    fn document_view(
        &self,
        document: &DocumentInfo<'a>,
        group: GroupBy,
    ) -> Result<GraphDocument, CompilerError> {
        let clauses: Vec<&Node> = self
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind != NodeKind::Document && node.source.path == document.path)
            .collect();
        let clause_ids: BTreeSet<&NodeId> = clauses.iter().map(|node| &node.id).collect();
        let edges: Vec<&Edge> = self
            .edges()
            .filter(|edge| clause_ids.contains(&edge.source) || clause_ids.contains(&edge.target))
            .collect();

        let mut foreign: BTreeSet<&NodeId> = BTreeSet::new();
        for edge in &edges {
            for end in [&edge.source, &edge.target] {
                if !clause_ids.contains(end) {
                    foreign.insert(end);
                }
            }
        }

        let mut nodes = Vec::with_capacity(clauses.len() + foreign.len());
        let mut scopes = Scopes::default();
        for clause in &clauses {
            let scope = scopes.place(clause, group, self.id);
            nodes.push(self.clause_node(
                clause,
                scope,
                Placement {
                    external: false,
                    navigate: None,
                },
            ));
        }
        for id in foreign {
            nodes.push(self.foreign_node(id, true, ROOT_SCOPE.to_owned()));
        }
        let diagram_edges = edges
            .iter()
            .map(|edge| self.diagram_edge(edge, None))
            .collect();

        let mut meta = self.unit_meta(
            &document.id,
            &document.title,
            &self.absolute(&document.path),
        );
        if let Some(node) = document.node {
            meta.insert("ndf".to_owned(), ndf_facts(node, true));
        }
        Ok(self.finish(
            format!("wf:{}", document.id),
            nodes,
            diagram_edges,
            scopes.into_scopes(),
            meta,
            self.file_diagnostics(&document.path),
        ))
    }

    // -- the trace view ------------------------------------------------------

    /// One edge type followed from the unit's clauses, both ways, through the
    /// project: every clause reached is drawn as itself, navigable into its
    /// document when that is another one.
    fn trace_view(
        &self,
        unit: &Unit<'a>,
        edge_type: &str,
        group: GroupBy,
    ) -> Result<GraphDocument, CompilerError> {
        let (name, title, file, seeds): (String, String, String, Vec<&NodeId>) = match unit {
            Unit::Project => (
                self.id.to_owned(),
                self.id.to_owned(),
                self.absolute(MANIFEST_FILE),
                self.graph
                    .nodes
                    .iter()
                    .filter(|node| node.kind != NodeKind::Document)
                    .map(|node| &node.id)
                    .collect(),
            ),
            Unit::Document(document) => (
                document.id.clone(),
                document.title.clone(),
                self.absolute(&document.path),
                self.graph
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind != NodeKind::Document && node.source.path == document.path
                    })
                    .map(|node| &node.id)
                    .collect(),
            ),
            Unit::Clause(node, _) => (
                node.id.local_id().to_owned(),
                title_of(node),
                self.absolute(&node.source.path),
                vec![&node.id],
            ),
        };
        let shown_path = match unit {
            Unit::Project => None,
            Unit::Document(document) | Unit::Clause(_, document) => Some(document.path.as_str()),
        };

        // Breadth first from the seeds, over edges of the one type, either way.
        let mut reached: Vec<&NodeId> = Vec::new();
        let mut seen: BTreeSet<&NodeId> = BTreeSet::new();
        let mut edge_indices: BTreeSet<usize> = BTreeSet::new();
        let mut queue: VecDeque<&NodeId> = VecDeque::new();
        for seed in seeds {
            if seen.insert(seed) {
                reached.push(seed);
                queue.push_back(seed);
            }
        }
        while let Some(id) = queue.pop_front() {
            let Some(indices) = self.edges_by_end.get(id) else {
                continue;
            };
            for &index in indices {
                let edge = self.edge(index);
                if edge.edge_type != edge_type {
                    continue;
                }
                edge_indices.insert(index);
                let other = if &edge.source == id {
                    &edge.target
                } else {
                    &edge.source
                };
                if seen.insert(other) {
                    reached.push(other);
                    queue.push_back(other);
                }
            }
        }

        let mut nodes = Vec::with_capacity(reached.len());
        let mut scopes = Scopes::default();
        for id in reached {
            match self.resolve(id) {
                Target::Local { node, document } => {
                    let scope = scopes.place(node, group, self.id);
                    let navigate = (Some(document.path.as_str()) != shown_path)
                        .then(|| self.navigation(document, node));
                    nodes.push(self.clause_node(
                        node,
                        scope,
                        Placement {
                            external: false,
                            navigate,
                        },
                    ));
                }
                Target::Dependency {
                    node,
                    root,
                    document,
                } => {
                    let scope = scopes.place(node, group, self.id);
                    nodes.push(self.clause_node(
                        node,
                        scope,
                        Placement {
                            external: false,
                            navigate: Some(Navigation {
                                document: document.id.clone(),
                                project: Some(id.project_id().to_owned()),
                                file: absolute_in(root, &node.source.path),
                                line: node.source.line,
                            }),
                        },
                    ));
                }
                Target::Unresolved => nodes.push(self.blackbox_node(id, ROOT_SCOPE.to_owned())),
            }
        }
        let diagram_edges = edge_indices
            .iter()
            .map(|&index| self.diagram_edge(self.edge(index), None))
            .collect();

        let label = view_label(edge_type);
        let mut meta = self.unit_meta(&name, &format!("{label}: {title}"), &file);
        meta.insert("view".to_owned(), Value::String(edge_type.to_owned()));
        meta.insert("viewLabel".to_owned(), Value::String(label));
        meta.insert("unitTitle".to_owned(), Value::String(title));
        if let Unit::Clause(node, _) = unit {
            meta.insert("ndf".to_owned(), ndf_facts(node, true));
        }
        let diagnostics = match shown_path {
            Some(path) => self.file_diagnostics(path),
            None => self.all_diagnostics(),
        };
        Ok(self.finish(
            format!("wf:{name}"),
            nodes,
            diagram_edges,
            scopes.into_scopes(),
            meta,
            diagnostics,
        ))
    }

    // -- the project view ----------------------------------------------------

    /// The project: one node per document, drilling into it, and the edges
    /// between documents added up by type. A target in another project is one
    /// blackbox per project.
    fn project_view(&self) -> Result<GraphDocument, CompilerError> {
        let mut nodes: Vec<DiagramNode> = self
            .documents
            .iter()
            .map(|document| self.document_node(document))
            .collect();

        // (from document, to document or project, type) -> count and first span.
        let mut aggregated: BTreeMap<(String, String, String), (usize, Option<Location>)> =
            BTreeMap::new();
        let mut foreign_projects: BTreeSet<&str> = BTreeSet::new();
        for edge in self.edges() {
            let Target::Local {
                document: from_document,
                ..
            } = self.resolve(&edge.source)
            else {
                continue;
            };
            let to = match self.resolve(&edge.target) {
                Target::Local { document, .. } => {
                    if document.path == from_document.path {
                        continue;
                    }
                    document_node_id(&document.id)
                }
                Target::Dependency { .. } | Target::Unresolved => {
                    if edge.target.project_id() == self.id {
                        // A dangling target of this project: the document
                        // view shows it where it belongs.
                        continue;
                    }
                    foreign_projects.insert(edge.target.project_id());
                    project_node_id(edge.target.project_id())
                }
            };
            let entry = aggregated
                .entry((
                    document_node_id(&from_document.id),
                    to,
                    edge.edge_type.clone(),
                ))
                .or_insert((0, None));
            entry.0 += 1;
            if entry.1.is_none() {
                entry.1 = edge.source_span.as_ref().map(|span| Location {
                    file: self.absolute(&span.path),
                    line: span.line,
                });
            }
        }
        for project_id in foreign_projects {
            nodes.push(self.project_node(project_id));
        }
        let edges = aggregated
            .into_iter()
            .map(|((from, to, edge_type), (count, source))| {
                let from = port_id(&from, "out");
                let to = port_id(&to, "in");
                let mut meta = Map::new();
                meta.insert("role".to_owned(), Value::String(edge_type.clone()));
                meta.insert(
                    "label".to_owned(),
                    Value::String(if count > 1 {
                        format!("{edge_type} ×{count}")
                    } else {
                        edge_type.clone()
                    }),
                );
                meta.insert("count".to_owned(), Value::from(count));
                if UNDIRECTED_EDGE_TYPES.contains(&edge_type.as_str()) {
                    meta.insert("undirected".to_owned(), Value::Bool(true));
                }
                DiagramEdge {
                    id: format!("edge:{from}->{to}:{edge_type}"),
                    from,
                    to,
                    scope: ROOT_SCOPE.to_owned(),
                    source,
                    meta,
                }
            })
            .collect();

        let mut meta = self.unit_meta(self.id, self.id, &self.absolute(MANIFEST_FILE));
        meta.insert(
            "facts".to_owned(),
            json!([
                { "label": "Project", "value": self.id },
                { "label": "Documents", "value": self.documents.len().to_string() },
                { "label": "Clauses", "value": self.graph.nodes.iter().filter(|node| node.kind != NodeKind::Document).count().to_string() },
                { "label": "Edges", "value": self.graph.edges.len().to_string() },
            ]),
        );
        Ok(self.finish(
            format!("wf:{}", self.id),
            nodes,
            edges,
            Vec::new(),
            meta,
            self.all_diagnostics(),
        ))
    }

    // -- nodes ---------------------------------------------------------------

    fn navigation(&self, document: &DocumentInfo<'_>, node: &Node) -> Navigation {
        Navigation {
            document: document.id.clone(),
            project: None,
            file: self.absolute(&node.source.path),
            line: node.source.line,
        }
    }

    fn clause_node(&self, node: &Node, scope: String, placement: Placement) -> DiagramNode {
        let id = node_id(&node.id, self.id);
        let mut meta = Map::new();
        let mut badges = badges(node);
        if let Some(project) = placement
            .navigate
            .as_ref()
            .and_then(|nav| nav.project.as_deref())
        {
            badges.push(Badge {
                text: project.to_owned(),
                tone: "info",
            });
        }
        meta.insert(
            "badges".to_owned(),
            serde_json::to_value(&badges).unwrap_or(Value::Array(Vec::new())),
        );
        let location = match &placement.navigate {
            Some(nav) => json!({ "file": nav.file, "line": nav.line }),
            None => location_value(&self.root, &node.source.path, node.source.line),
        };
        meta.insert("source".to_owned(), location.clone());
        meta.insert(
            "definitionAnnotations".to_owned(),
            definition_annotations(node),
        );
        meta.insert("ndf".to_owned(), ndf_facts(node, true));
        if let Some(open) = self.open_target(node) {
            meta.insert("open".to_owned(), open);
        }
        // A clause the panel edits shows its fields there, not twice.
        // Only Markdown clauses are edited in place; the others (an ASL
        // unit's, an instruction's) belong to the file their adapter reads.
        let editable = node.id.project_id() == self.id
            && !placement.external
            && node.source.path.ends_with(".md");
        let mut facts = self.clause_facts(node, !editable);
        if placement.external {
            meta.insert("external".to_owned(), Value::Bool(true));
            meta.insert("referencedSource".to_owned(), location.clone());
        }
        if let Some(nav) = &placement.navigate {
            meta.insert("networkInstance".to_owned(), Value::Bool(true));
            meta.insert(
                "referencedEntityName".to_owned(),
                Value::String(nav.document.clone()),
            );
            meta.insert(
                "instanceName".to_owned(),
                Value::String(nav.document.clone()),
            );
            meta.insert("referencedSource".to_owned(), location);
            let mut where_facts = vec![json!({ "label": "Document", "value": nav.document })];
            if let Some(project) = &nav.project {
                where_facts.push(json!({ "label": "Project", "value": project }));
            }
            if let Value::Array(items) = &mut facts {
                // The body (or contract) stays last: it is the long one.
                let body = match items.last() {
                    Some(last) if last["label"] == "Body" || last["label"] == "Contract" => {
                        items.pop()
                    }
                    _ => None,
                };
                items.extend(where_facts);
                items.extend(body);
            }
        }
        meta.insert("facts".to_owned(), facts);
        meta.insert("views".to_owned(), views_meta());
        if editable {
            // The fields a panel edits in place (`updateNodeParameter`): the
            // canonical ones, each with its value or empty when unset.
            meta.insert("parameters".to_owned(), self.parameters(node));
        }
        if node.id.project_id() == self.id
            && let Some(diagnostics) = self.diagnostics_by_subject.get(&node.id)
        {
            attach_diagnostics(&mut meta, diagnostics);
        }
        DiagramNode {
            kind: kind_name(node.kind),
            label: title_of(node),
            footer: Some(node.id.local_id().to_owned()),
            scope,
            ports: pins(&id),
            meta,
            id,
        }
    }

    /// A clause outside the unit: the clause it is, drawn external and
    /// navigable into its document, or a blackbox when nothing resolves it.
    fn foreign_node(&self, id: &NodeId, external: bool, scope: String) -> DiagramNode {
        match self.resolve(id) {
            Target::Local { node, document } => self.clause_node(
                node,
                scope,
                Placement {
                    external,
                    navigate: Some(self.navigation(document, node)),
                },
            ),
            Target::Dependency {
                node,
                root,
                document,
            } => self.clause_node(
                node,
                scope,
                Placement {
                    external,
                    navigate: Some(Navigation {
                        document: document.id.clone(),
                        project: Some(id.project_id().to_owned()),
                        file: absolute_in(root, &node.source.path),
                        line: node.source.line,
                    }),
                },
            ),
            Target::Unresolved => self.blackbox_node(id, scope),
        }
    }

    fn blackbox_node(&self, id: &NodeId, scope: String) -> DiagramNode {
        let diagram_id = node_id(id, self.id);
        let label = if id.project_id() == self.id {
            id.local_id().to_owned()
        } else {
            format!("{}/{}", id.project_id(), id.local_id())
        };
        let mut meta = Map::new();
        meta.insert("external".to_owned(), Value::Bool(true));
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
            scope,
            ports: pins(&diagram_id),
            meta,
            id: diagram_id,
        }
    }

    /// A document in the project view: drills into the document.
    fn document_node(&self, document: &DocumentInfo<'a>) -> DiagramNode {
        let id = document_node_id(&document.id);
        let file = self.absolute(&document.path);
        let line = document.node.map(|node| node.source.line).unwrap_or(1);
        let location = json!({ "file": file, "line": line });
        let mut meta = Map::new();
        meta.insert("networkInstance".to_owned(), Value::Bool(true));
        meta.insert(
            "referencedEntityName".to_owned(),
            Value::String(document.id.clone()),
        );
        meta.insert(
            "instanceName".to_owned(),
            Value::String(document.id.clone()),
        );
        meta.insert("source".to_owned(), location.clone());
        meta.insert("referencedSource".to_owned(), location);
        let mut badges = vec![Badge {
            text: format!(
                "{} clause{}",
                document.clause_count,
                if document.clause_count == 1 { "" } else { "s" }
            ),
            tone: "neutral",
        }];
        let mut arguments = vec![
            json!({ "name": "id", "value": document.id }),
            json!({ "name": "title", "value": document.title }),
            json!({ "name": "file", "value": document.path }),
            json!({ "name": "clauses", "value": document.clause_count.to_string() }),
        ];
        let mut facts = vec![
            json!({ "label": "ID", "value": document.id }),
            json!({ "label": "Title", "value": document.title }),
            json!({ "label": "File", "value": document.path }),
            json!({ "label": "Clauses", "value": document.clause_count.to_string() }),
        ];
        if let Some(node) = document.node {
            if let Some(status) = enum_name(&node.status) {
                arguments.push(json!({ "name": "status", "value": status }));
                facts.push(json!({ "label": "Status", "value": status }));
                if let Some(tone) = status_tone(node.status) {
                    badges.push(Badge {
                        text: status.clone(),
                        tone,
                    });
                }
            }
            if let Some(owner) = &node.owner {
                arguments.push(json!({ "name": "owner", "value": owner }));
                facts.push(json!({ "label": "Owner", "value": owner }));
            }
            for (key, value) in &node.attributes {
                arguments.push(json!({ "name": key, "value": value }));
                facts.push(json!({ "label": key, "value": value }));
            }
            meta.insert("ndf".to_owned(), ndf_facts(node, true));
        }
        meta.insert(
            "badges".to_owned(),
            serde_json::to_value(&badges).unwrap_or(Value::Array(Vec::new())),
        );
        meta.insert(
            "definitionAnnotations".to_owned(),
            json!([{ "name": "document" }, { "name": "ndf", "arguments": arguments }]),
        );
        meta.insert("facts".to_owned(), Value::Array(facts));
        meta.insert("views".to_owned(), views_meta());
        let diagnostics: Vec<&Diagnostic> = self
            .graph
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.primary.path == document.path)
            .collect();
        if !diagnostics.is_empty() {
            attach_diagnostics(&mut meta, &diagnostics);
        }
        DiagramNode {
            kind: "document".to_owned(),
            label: document.id.clone(),
            footer: (document.title != document.id).then(|| document.title.clone()),
            scope: ROOT_SCOPE.to_owned(),
            ports: pins(&id),
            meta,
            id,
        }
    }

    /// Another project in the project view: one blackbox, whatever the
    /// edges into it reach.
    fn project_node(&self, project_id: &str) -> DiagramNode {
        let id = project_node_id(project_id);
        let mut meta = Map::new();
        meta.insert("external".to_owned(), Value::Bool(true));
        meta.insert(
            "definitionAnnotations".to_owned(),
            json!([{ "name": "blackbox" }, { "name": "ndf", "arguments": [
                { "name": "project", "value": project_id }
            ] }]),
        );
        let mut facts = vec![json!({ "label": "Project", "value": project_id })];
        if let Some(dependency) = self.dependencies.get(project_id) {
            let checkout = dependency.root.to_string_lossy().into_owned();
            facts.push(json!({ "label": "Checkout", "value": checkout }));
            meta.insert(
                "referencedSource".to_owned(),
                json!({ "file": absolute_in(dependency.root, MANIFEST_FILE), "line": 1 }),
            );
        }
        meta.insert("facts".to_owned(), Value::Array(facts));
        DiagramNode {
            kind: "blackbox".to_owned(),
            label: project_id.to_owned(),
            footer: None,
            scope: ROOT_SCOPE.to_owned(),
            ports: pins(&id),
            meta,
            id,
        }
    }

    // -- edges, diagnostics, meta --------------------------------------------

    fn diagram_edge(&self, edge: &Edge, scope: Option<&str>) -> DiagramEdge {
        let from = port_id(&node_id(&edge.source, self.id), "out");
        let to = port_id(&node_id(&edge.target, self.id), "in");
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
            scope: scope.unwrap_or(ROOT_SCOPE).to_owned(),
            source: edge.source_span.as_ref().map(|span| Location {
                file: self.absolute(&span.path),
                line: span.line,
            }),
            meta,
        }
    }

    fn file_diagnostics(&self, path: &str) -> Vec<GraphDiagnostic> {
        self.graph
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.primary.path == path)
            .map(|diagnostic| self.graph_diagnostic(diagnostic))
            .collect()
    }

    fn all_diagnostics(&self) -> Vec<GraphDiagnostic> {
        self.graph
            .diagnostics
            .iter()
            .map(|diagnostic| self.graph_diagnostic(diagnostic))
            .collect()
    }

    fn graph_diagnostic(&self, diagnostic: &Diagnostic) -> GraphDiagnostic {
        GraphDiagnostic {
            uri: self.absolute(&diagnostic.primary.path),
            severity: severity_name(diagnostic.severity),
            message: match &diagnostic.repair {
                Some(repair) => {
                    format!("{}. {}", diagnostic.message.trim_end_matches('.'), repair)
                }
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

    fn unit_meta(&self, workflow: &str, title: &str, file: &str) -> Map<String, Value> {
        let mut meta = Map::new();
        meta.insert("workflow".to_owned(), Value::String(workflow.to_owned()));
        meta.insert("title".to_owned(), Value::String(title.to_owned()));
        meta.insert("file".to_owned(), Value::String(file.to_owned()));
        meta.insert("project".to_owned(), Value::String(self.id.to_owned()));
        meta.insert("views".to_owned(), views_meta());
        meta
    }

    fn finish(
        &self,
        id: String,
        nodes: Vec<DiagramNode>,
        edges: Vec<DiagramEdge>,
        subgraphs: Vec<DiagramScope>,
        meta: Map<String, Value>,
        diagnostics: Vec<GraphDiagnostic>,
    ) -> GraphDocument {
        GraphDocument {
            version: DOCUMENT_VERSION,
            partial: None,
            errors: Vec::new(),
            diagnostics,
            graph: DiagramGraph {
                id,
                nodes,
                edges,
                subgraphs,
                meta,
            },
            hierarchy: Some(self.outline()),
        }
    }

    /// The project outline, for the diagram's outline panel: the project at
    /// the root; its documents by folder (the specification's chapters), each
    /// by its title, its clauses beneath it grouped as the diagram groups them;
    /// then the documents of its locked dependencies. Folders and layers are
    /// `group` rows, clauses `node` rows the panel finds in their document's
    /// view. Past `OUTLINE_BUDGET` rows a document's clauses are left out
    /// (it is marked truncated), never a document.
    fn outline(&self) -> HierarchyEntry {
        let mut clauses_by_path: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
        for node in &self.graph.nodes {
            if node.kind != NodeKind::Document {
                clauses_by_path
                    .entry(node.source.path.as_str())
                    .or_default()
                    .push(node);
            }
        }
        for clauses in clauses_by_path.values_mut() {
            clauses.sort_by_key(|node| (node.source.line, node.source.column));
        }
        let dependency_documents: usize = self
            .dependencies
            .values()
            .map(|dependency| dependency.documents.len())
            .sum();
        let mut budget =
            OUTLINE_BUDGET.saturating_sub(1 + self.documents.len() + dependency_documents);

        let mut children: Vec<HierarchyEntry> = Vec::new();
        for document in self.documents.iter().take(OUTLINE_BUDGET) {
            let clauses = clauses_by_path
                .get(document.path.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default();
            let entry = self.document_outline(document, clauses, &mut budget);
            // Documents come in path order, so a folder's are together.
            match document.path.rsplit_once('/') {
                None => children.push(entry),
                Some((folder, _)) => match children.last_mut() {
                    Some(last)
                        if last.role == Some("group") && last.label.as_deref() == Some(folder) =>
                    {
                        last.children.push(entry);
                    }
                    _ => children.push(HierarchyEntry::group(
                        vec![format!("dir:{folder}")],
                        folder,
                        vec![entry],
                    )),
                },
            }
        }
        if !self.dependencies.is_empty() {
            let projects = self
                .dependencies
                .iter()
                .map(|(project, dependency)| {
                    let documents = dependency
                        .documents
                        .iter()
                        .map(|document| HierarchyEntry {
                            node_count: Some(document.clause_count),
                            label: (!document.title.is_empty() && document.title != document.id)
                                .then(|| document.title.clone()),
                            detail: Some(document.id.clone()),
                            ..HierarchyEntry::row(
                                vec![document.id.clone()],
                                document.id.clone(),
                                file_uri(&absolute_in(dependency.root, &document.path)),
                            )
                        })
                        .collect();
                    HierarchyEntry::group(vec![format!("dep:{project}")], project, documents)
                })
                .collect();
            children.push(HierarchyEntry::group(
                vec!["deps".to_owned()],
                "Dependencies",
                projects,
            ));
        }
        HierarchyEntry {
            node_count: Some(self.documents.len()),
            truncated: self.documents.len() > OUTLINE_BUDGET,
            children,
            ..HierarchyEntry::row(
                Vec::new(),
                self.id.to_owned(),
                file_uri(&self.absolute(MANIFEST_FILE)),
            )
        }
    }

    /// A document's outline row, its clauses beneath it while the budget
    /// lasts, grouped by refinement layer or domain as the diagram is.
    fn document_outline(
        &self,
        document: &DocumentInfo<'_>,
        clauses: &[&Node],
        budget: &mut usize,
    ) -> HierarchyEntry {
        let source_uri = file_uri(&self.absolute(&document.path));
        let mut entry = HierarchyEntry {
            node_count: Some(document.clause_count),
            label: (!document.title.is_empty() && document.title != document.id)
                .then(|| document.title.clone()),
            detail: Some(document.id.clone()),
            ..HierarchyEntry::row(
                vec![document.id.clone()],
                document.id.clone(),
                source_uri.clone(),
            )
        };
        let key = |clause: &Node| -> Option<String> {
            match self.group {
                GroupBy::None => None,
                GroupBy::Refinement => Some(
                    clause
                        .refinement
                        .clone()
                        .unwrap_or_else(|| "unlayered".to_owned()),
                ),
                GroupBy::Domain => Some(
                    clause
                        .domain
                        .clone()
                        .unwrap_or_else(|| "no domain".to_owned()),
                ),
            }
        };
        let mut groups: BTreeMap<String, Vec<&Node>> = BTreeMap::new();
        if self.group != GroupBy::None {
            for clause in clauses {
                groups
                    .entry(key(clause).unwrap_or_default())
                    .or_default()
                    .push(clause);
            }
        }
        let rows = clauses.len() + groups.len();
        if rows > *budget {
            entry.truncated = !clauses.is_empty();
            return entry;
        }
        *budget -= rows;
        let clause_row = |clause: &Node| -> HierarchyEntry {
            let local = clause.id.local_id().to_owned();
            let mut marks = badges(clause);
            if self.group == GroupBy::Refinement {
                marks.retain(|badge| Some(&badge.text) != clause.refinement.as_ref());
            }
            HierarchyEntry {
                role: Some("node"),
                label: Some(local.clone()),
                detail: Some(title_of(clause)),
                family: Some(kind_name(clause.kind)),
                badges: marks,
                node_id: Some(node_id(&clause.id, self.id)),
                ..HierarchyEntry::row(
                    vec![document.id.clone(), local.clone()],
                    local,
                    source_uri.clone(),
                )
            }
        };
        let prefix = if self.group == GroupBy::Domain {
            "domain"
        } else {
            "layer"
        };
        entry.children = if self.group == GroupBy::None {
            clauses.iter().map(|clause| clause_row(clause)).collect()
        } else {
            groups
                .into_iter()
                .map(|(key, members)| {
                    HierarchyEntry::group(
                        vec![document.id.clone(), format!("{prefix}:{key}")],
                        &key,
                        members.into_iter().map(clause_row).collect(),
                    )
                })
                .collect()
        };
        entry
    }

    fn absolute(&self, relative: &str) -> String {
        absolute_in(&self.root, relative)
    }

    // -- what a panel shows of a clause --------------------------------------

    /// A clause of the project or of a dependency, with the root its source
    /// path is relative to.
    fn find_node(&self, id: &NodeId) -> Option<(&Path, &'a Node)> {
        if let Some(node) = self.nodes_by_id.get(id) {
            return Some((self.root.as_path(), *node));
        }
        let dependency = self.dependencies.get(id.project_id())?;
        dependency
            .nodes_by_id
            .get(id)
            .map(|node| (dependency.root, *node))
    }

    /// Where a link to a clause goes: its node in the diagram, and its file
    /// and line for when the diagram does not draw it.
    fn clause_target(&self, id: &NodeId) -> Option<LinkTarget> {
        let (root, node) = self.find_node(id)?;
        let file = PathBuf::from(absolute_in(root, &node.source.path));
        Some(LinkTarget {
            href: format!("#{}", node_id(id, self.id)),
            title: Some(format!("{}#L{}", link_path(&file), node.source.line)),
        })
    }

    /// Where a code span that names a file goes, read from a document in
    /// `base_dir`: the file the project holds, at the line or symbol named;
    /// else, for a path with a known extension, the text itself, for the
    /// editor to look for.
    fn file_target(&self, base_dir: &Path, text: &str) -> Option<LinkTarget> {
        let reference = parse_file_ref(text)?;
        let Some(file) = self.files.resolve(base_dir, reference.path) else {
            return has_file_extension(reference.path).then(|| LinkTarget {
                href: text.to_owned(),
                title: None,
            });
        };
        let line = reference.line.or_else(|| {
            reference
                .symbol
                .and_then(|symbol| symbol_line(&file, symbol))
        });
        let place = match (line, reference.end_line) {
            (Some(start), Some(end)) => format!("#L{start}-L{end}"),
            (Some(start), None) => format!("#L{start}"),
            _ => String::new(),
        };
        Some(LinkTarget {
            href: format!("{}{place}", link_path(&file)),
            title: None,
        })
    }

    /// The folder of the document a clause is written in.
    fn base_dir(&self, node: &Node) -> PathBuf {
        let root = self
            .find_node(&node.id)
            .map_or(self.root.as_path(), |(root, _)| root);
        let file = PathBuf::from(absolute_in(root, &node.source.path));
        file.parent()
            .map_or_else(|| self.root.clone(), Path::to_path_buf)
    }

    /// Markdown with the clause's mentions and file references made links.
    fn linked(&self, node: &Node, text: &str) -> String {
        let base_dir = self.base_dir(node);
        link_markdown(
            text,
            |mention| {
                NodeId::parse(mention.trim(), Some(node.id.project_id()))
                    .ok()
                    .and_then(|id| self.clause_target(&id))
            },
            |code| self.file_target(&base_dir, code),
        )
    }

    /// The file a clause stands for, when its title names one the project
    /// holds (a `source` clause's `` `path::Symbol` ``): `{ file, line }`.
    fn open_target(&self, node: &Node) -> Option<Value> {
        let title = title_of(node);
        let reference = parse_file_ref(title.trim().trim_matches('`'))?;
        let file = self.files.resolve(&self.base_dir(node), reference.path)?;
        let line = reference.line.or_else(|| {
            reference
                .symbol
                .and_then(|symbol| symbol_line(&file, symbol))
        });
        let mut open = json!({ "file": file.to_string_lossy() });
        if let Some(line) = line {
            open["line"] = Value::from(line);
        }
        Some(open)
    }

    /// The file a node was read from, absolute, and its text.
    fn source_text(&self, node: &Node) -> Option<(PathBuf, String)> {
        let root = self
            .find_node(&node.id)
            .map_or(self.root.as_path(), |(root, _)| root);
        let file = PathBuf::from(absolute_in(root, &node.source.path));
        let text = std::fs::read_to_string(&file).ok()?;
        Some((file, text))
    }

    /// The contract an ASL instruction's metadata line states, as Markdown:
    /// its text in the graph is empty, the contract being structured data.
    fn contract(&self, node: &Node) -> Option<String> {
        if node.attributes.get("pto_entity").map(String::as_str) != Some("instruction") {
            return None;
        }
        let (_, text) = self.source_text(node)?;
        let line = text
            .lines()
            .nth(node.source.line.checked_sub(1)? as usize)?;
        instruction_contract(line)
    }

    /// What an ASL unit's file declares, each name a link to its line:
    /// `Types`, `Functions`, then `Globals` (variables, constants, configs).
    fn declaration_facts(&self, node: &Node) -> Vec<Value> {
        if node.kind != NodeKind::Source || !node.source.path.ends_with(".asl") {
            return Vec::new();
        }
        let Some((file, text)) = self.source_text(node) else {
            return Vec::new();
        };
        let declarations = asl_declarations(&text);
        let href = link_path(&file);
        [
            ("Types", &["type"][..]),
            ("Functions", &["func"][..]),
            ("Globals", &["var", "constant", "config"][..]),
        ]
        .into_iter()
        .filter_map(|(label, keywords)| {
            let links = declarations
                .iter()
                .filter(|declaration| keywords.contains(&declaration.keyword))
                .map(|declaration| {
                    format!("[`{}`]({href}#L{})", declaration.name, declaration.line)
                })
                .collect::<Vec<_>>();
            (!links.is_empty())
                .then(|| json!({ "label": label, "value": links.join(", "), "markdown": true }))
        })
        .collect()
    }

    /// The clause's typed edges, each end a link: `refines → X Title` for its
    /// own, `← verifies Y Title` for the ones that point at it.
    fn trace_fact(&self, node: &Node) -> Option<Value> {
        let mut outgoing = Vec::new();
        let mut incoming = Vec::new();
        for &index in self
            .edges_by_end
            .get(&node.id)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let edge = self.edge(index);
            let (other, list) = if edge.source == node.id {
                (&edge.target, &mut outgoing)
            } else {
                (&edge.source, &mut incoming)
            };
            let title = self
                .find_node(other)
                .map(|(_, other)| title_of(other))
                .unwrap_or_default();
            let name = if other.project_id() == self.id {
                other.local_id().to_owned()
            } else {
                other.uri()
            };
            let target = match self.clause_target(other) {
                Some(target) => format!(
                    "[{name}]({} \"{}\")",
                    target.href,
                    target.title.unwrap_or_default()
                ),
                None => format!("`{name}`"),
            };
            list.push((edge.edge_type.clone(), target, title));
        }
        if outgoing.is_empty() && incoming.is_empty() {
            return None;
        }
        outgoing.sort();
        incoming.sort();
        let lines: Vec<String> = outgoing
            .into_iter()
            .map(|(kind, target, title)| format!("- {kind} → {target} {title}"))
            .chain(
                incoming
                    .into_iter()
                    .map(|(kind, target, title)| format!("- ← {kind} {target} {title}")),
            )
            .map(|line| line.trim_end().to_owned())
            .collect();
        Some(json!({ "label": "Trace", "value": lines.join("\n"), "markdown": true }))
    }

    /// The facts a property panel lists for a clause, in reading order: what
    /// it is, its fields (unless the panel edits them, `with_fields` false)
    /// and attributes (a file one a link), the clauses it is tied to, then its
    /// body with its mentions and files as links.
    fn clause_facts(&self, node: &Node, with_fields: bool) -> Value {
        let mut facts = Vec::new();
        let mut push = |label: &str, value: &str| {
            facts.push(json!({ "label": label, "value": value }));
        };
        push("ID", &node.id.uri());
        push("Kind", &kind_name(node.kind));
        if with_fields {
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
        }
        let base_dir = self.base_dir(node);
        for (key, value) in &node.attributes {
            match self.file_target(&base_dir, value) {
                Some(target) => facts.push(json!({
                    "label": key,
                    "value": format!("[`{value}`]({})", target.href),
                    "markdown": true
                })),
                None => facts.push(json!({ "label": key, "value": value })),
            }
        }
        if let Some(open) = self.open_target(node)
            && let Some(file) = open["file"].as_str()
        {
            let place = open["line"]
                .as_u64()
                .map(|line| format!("#L{line}"))
                .unwrap_or_default();
            facts.push(json!({
                "label": "File",
                "value": format!("[`{}`]({}{place})", title_of(node).trim().trim_matches('`'), link_path(Path::new(file))),
                "markdown": true
            }));
        }
        facts.extend(self.declaration_facts(node));
        facts.extend(self.trace_fact(node));
        if !node.body.trim().is_empty() {
            facts.push(json!({ "label": "Body", "value": self.linked(node, &node.body), "markdown": true }));
        } else if let Some(contract) = self.contract(node) {
            facts.push(json!({ "label": "Contract", "value": self.linked(node, &contract), "markdown": true }));
        }
        Value::Array(facts)
    }

    /// The canonical fields as a panel's editable parameters: name and value
    /// (empty when unset), the closed ones with their `choices`, the open ones
    /// with what the manifest and the project's clauses already use as
    /// `suggestions`.
    fn parameters(&self, node: &Node) -> Value {
        let mut refinements: BTreeSet<String> = ["L0", "L1", "L2", "L3"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        refinements.extend(self.used.refinements.iter().cloned());
        let mut domains: BTreeSet<String> = self.domains.iter().cloned().collect();
        domains.extend(self.used.domains.iter().cloned());
        let field = |name: &str, value: Option<String>| json!({ "name": name, "value": value.unwrap_or_default(), "editable": true });
        let with = |mut field: Value, key: &str, values: Vec<String>| {
            if !values.is_empty() {
                field[key] = json!(values);
            }
            field
        };
        Value::Array(vec![
            with(
                field("modality", node.modality.as_ref().and_then(enum_name)),
                "choices",
                [
                    Modality::Must,
                    Modality::Should,
                    Modality::May,
                    Modality::Tbd,
                ]
                .iter()
                .filter_map(enum_name)
                .collect(),
            ),
            with(
                field("refinement", node.refinement.clone()),
                "suggestions",
                refinements.into_iter().collect(),
            ),
            with(
                field("domain", node.domain.clone()),
                "suggestions",
                domains.into_iter().collect(),
            ),
            with(
                field("status", enum_name(&node.status)),
                "choices",
                [
                    LifecycleStatus::Draft,
                    LifecycleStatus::Active,
                    LifecycleStatus::Deprecated,
                    LifecycleStatus::Superseded,
                    LifecycleStatus::Retired,
                    LifecycleStatus::Open,
                    LifecycleStatus::Resolved,
                ]
                .iter()
                .filter_map(enum_name)
                .collect(),
            ),
            with(
                field("owner", node.owner.clone()),
                "suggestions",
                self.used.owners.iter().cloned().collect(),
            ),
        ])
    }
}

/// The containers a grouping puts clauses in, in first-seen order of their
/// keys sorted (`L0` before `L1`; domains alphabetical).
#[derive(Default)]
struct Scopes {
    scopes: BTreeMap<String, DiagramScope>,
}

impl Scopes {
    /// The scope id the clause goes in, recording its membership.
    fn place(&mut self, clause: &Node, group: GroupBy, project: &str) -> String {
        let key = match group {
            GroupBy::None => None,
            GroupBy::Refinement => clause.refinement.clone(),
            GroupBy::Domain => clause.domain.clone(),
        };
        match key {
            Some(key) => {
                let id = format!("scope:group:{key}");
                self.scopes
                    .entry(key.clone())
                    .or_insert_with(|| DiagramScope {
                        id: id.clone(),
                        kind: "group",
                        label: key,
                        nodes: Vec::new(),
                    })
                    .nodes
                    .push(node_id(&clause.id, project));
                id
            }
            None => ROOT_SCOPE.to_owned(),
        }
    }

    fn into_scopes(self) -> Vec<DiagramScope> {
        self.scopes.into_values().collect()
    }
}

/// The documents of a graph, in path order: one per file, named by its
/// `document` node when the file has one and by its stem otherwise.
fn documents_of(graph: &Graph) -> Vec<DocumentInfo<'_>> {
    let mut by_path: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
    for node in &graph.nodes {
        by_path
            .entry(node.source.path.as_str())
            .or_default()
            .push(node);
    }
    by_path
        .into_iter()
        .map(|(path, nodes)| {
            let document = nodes.iter().find(|node| node.kind == NodeKind::Document);
            let clause_count = nodes
                .iter()
                .filter(|node| node.kind != NodeKind::Document)
                .count();
            match document {
                Some(node) => DocumentInfo {
                    id: node.id.local_id().to_owned(),
                    title: node.title.clone(),
                    path: path.to_owned(),
                    node: Some(node),
                    clause_count,
                },
                None => stem_document(path, nodes.into_iter()),
            }
        })
        .collect()
}

/// A document with no `document` node: named by its file stem, titled by it
/// too.
fn stem_document<'a>(path: &str, nodes: impl Iterator<Item = &'a Node>) -> DocumentInfo<'a> {
    let stem = file_stem(Path::new(path));
    DocumentInfo {
        id: stem.clone(),
        title: stem,
        path: path.to_owned(),
        node: None,
        clause_count: nodes.filter(|node| node.kind != NodeKind::Document).count(),
    }
}

fn document_node_id(doc_id: &str) -> String {
    format!("node:{ROOT_SCOPE}:{doc_id}")
}

fn project_node_id(project_id: &str) -> String {
    format!("node:{ROOT_SCOPE}:{project_id}")
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

/// The tone a status badge takes, or none for an active item.
fn status_tone(status: LifecycleStatus) -> Option<&'static str> {
    match status {
        LifecycleStatus::Active => None,
        LifecycleStatus::Draft | LifecycleStatus::Resolved => Some("neutral"),
        LifecycleStatus::Deprecated
        | LifecycleStatus::Superseded
        | LifecycleStatus::Retired
        | LifecycleStatus::Open => Some("warning"),
    }
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
    if let Some(tone) = status_tone(node.status) {
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

/// The diagnostics about a node, on the node: the list, and the first error
/// as the reason it is drawn red.
fn attach_diagnostics(meta: &mut Map<String, Value>, diagnostics: &[&Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }
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
    let errors: Vec<&&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .collect();
    if let Some(first) = errors.first() {
        meta.insert("isErrored".to_owned(), Value::Bool(true));
        let message = format!("{}: {}", first.code, first.message);
        meta.insert(
            "errorMessage".to_owned(),
            Value::String(if errors.len() > 1 {
                format!("{message} (+{} more)", errors.len() - 1)
            } else {
                message
            }),
        );
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

/// A node's title as the diagram shows it: the adapter's reader-facing name
/// when it has one (an ASL unit's file, an instruction's mnemonic), else the
/// title the graph records.
fn title_of(node: &Node) -> String {
    display_title(node).unwrap_or_else(|| node.title.clone())
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

fn file_stem(file: &Path) -> String {
    file.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_owned())
}

fn canonical(path: &Path) -> Result<PathBuf, CompilerError> {
    path.canonicalize().map_err(|source| CompilerError::Read {
        path: path.to_path_buf(),
        source,
    })
}

/// An absolute path as a `file:` URI, the way a consumer's URI library writes
/// one: every byte outside the unreserved set and `/` percent-encoded.
pub fn file_uri(absolute: &str) -> String {
    let mut uri = String::with_capacity(absolute.len() + 8);
    uri.push_str("file://");
    if !absolute.starts_with('/') {
        uri.push('/');
    }
    for byte in absolute.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(byte as char);
            }
            other => uri.push_str(&format!("%{other:02X}")),
        }
    }
    uri
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
