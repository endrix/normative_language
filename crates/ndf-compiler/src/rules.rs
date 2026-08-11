use std::collections::{BTreeMap, BTreeSet, HashMap};

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, Graph, LifecycleStatus, Modality, Node, NodeKind,
    SourceSpan,
};

use crate::dependencies::DependencyGraph;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ValidationPolicy {
    pub mandatory_edge_types: BTreeSet<&'static str>,
    pub ownership_kinds: BTreeSet<NodeKind>,
}

impl Default for ValidationPolicy {
    fn default() -> Self {
        Self {
            mandatory_edge_types: ["refines", "depends-on", "verifies", "implements"]
                .into_iter()
                .collect(),
            ownership_kinds: [
                NodeKind::Requirement,
                NodeKind::Constraint,
                NodeKind::Verification,
                NodeKind::Decision,
                NodeKind::Question,
            ]
            .into_iter()
            .collect(),
        }
    }
}

pub fn validate_graph(graph: &Graph, policy: &ValidationPolicy) -> Vec<Diagnostic> {
    let mut diagnostics = graph.diagnostics.clone();
    let nodes_by_id: HashMap<&NodeId, &Node> =
        graph.nodes.iter().map(|node| (&node.id, node)).collect();

    for edge in &graph.edges {
        if !policy
            .mandatory_edge_types
            .contains(edge.edge_type.as_str())
        {
            continue;
        }
        if nodes_by_id.contains_key(&edge.source) && nodes_by_id.contains_key(&edge.target) {
            continue;
        }
        let primary = edge
            .source_span
            .clone()
            .or_else(|| {
                nodes_by_id
                    .get(&edge.source)
                    .map(|node| node.source.clone())
            })
            .unwrap_or_else(|| SourceSpan::new("<graph>", 1).expect("line one is valid"));
        diagnostics.push(Diagnostic {
            code: "NDF-EDGE-001".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: format!("mandatory edge {} has a dangling endpoint", edge.edge_type),
            primary,
            related: Vec::new(),
            repair: Some("Resolve both endpoints or remove the mandatory edge.".to_owned()),
            subject: Some(edge.source.clone()),
        });
    }

    for node in &graph.nodes {
        if matches!(
            node.kind,
            NodeKind::Requirement | NodeKind::Constraint | NodeKind::Verification
        ) && node.modality == Some(Modality::Must)
            && !contains_word(&node.body, "MUST")
        {
            diagnostics.push(node_diagnostic(
                "NDF-MOD-001",
                format!("must requirement {} does not contain MUST", node.id),
                node,
                "Add normative MUST language or change the modality.",
            ));
        }
        if node.kind == NodeKind::Information && contains_word(&node.body, "MUST") {
            diagnostics.push(node_diagnostic(
                "NDF-MOD-002",
                format!("information node {} contains MUST", node.id),
                node,
                "Move the statement to a normative node or remove MUST.",
            ));
        }
        if node.attributes.get("previous_status").map(String::as_str) == Some("active")
            && node.status == LifecycleStatus::Draft
        {
            diagnostics.push(node_diagnostic(
                "NDF-LIFE-001",
                format!("node {} cannot move from active to draft", node.id),
                node,
                "Deprecate or supersede the active node instead.",
            ));
        }
        if policy.ownership_kinds.contains(&node.kind) && node.owner.is_none() {
            diagnostics.push(node_diagnostic(
                "NDF-OWN-001",
                format!("explicit normative node {} has no owner", node.id),
                node,
                "Assign an accountable owner.",
            ));
        }
    }

    diagnostics.sort_by(|left, right| {
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
    diagnostics
}

pub fn validate_project_graph(
    graph: &Graph,
    dependencies: &BTreeMap<String, DependencyGraph>,
    policy: &ValidationPolicy,
) -> Vec<Diagnostic> {
    let mut diagnostics = validate_graph(graph, policy);
    let nodes_by_id: HashMap<&NodeId, &Node> =
        graph.nodes.iter().map(|node| (&node.id, node)).collect();
    let dependency_ids: BTreeMap<_, BTreeSet<_>> = dependencies
        .iter()
        .map(|(project, dependency)| {
            (
                project.as_str(),
                dependency.graph.nodes.iter().map(|node| &node.id).collect(),
            )
        })
        .collect();

    for edge in &graph.edges {
        if !matches!(edge.edge_type.as_str(), "conforms-to" | "tracks") {
            continue;
        }
        let source = nodes_by_id.get(&edge.source).copied();
        if edge.edge_type == "tracks"
            && source.and_then(|node| node.attributes.get("expected").map(String::as_str))
                != Some("absent")
        {
            diagnostics.push(edge_diagnostic(
                "NDF-XREF-001",
                "tracks requires expected=absent on its source node",
                edge,
                source,
                "Set expected=absent or remove the tracks relationship.",
            ));
            continue;
        }
        if source.is_some_and(|node| node.id.project_id() == edge.target.project_id()) {
            let (code, message, repair) = if edge.edge_type == "tracks" {
                (
                    "NDF-XREF-001",
                    "expected-absent tracks relationship must target an external project",
                    "Use a full identity in a declared graph dependency.",
                )
            } else {
                (
                    "NDF-XREF-003",
                    "conforms-to relationship must target a declared external project",
                    "Use a target from a declared graph dependency.",
                )
            };
            diagnostics.push(edge_diagnostic(code, message, edge, source, repair));
            continue;
        }
        let Some(ids) = dependency_ids.get(edge.target.project_id()) else {
            diagnostics.push(edge_diagnostic(
                "NDF-XREF-003",
                format!(
                    "external target project is not a declared graph dependency: {}",
                    edge.target.project_id()
                ),
                edge,
                source,
                "Declare and exactly lock the external graph dependency.",
            ));
            continue;
        };
        let present = ids.contains(&edge.target);
        if edge.edge_type == "conforms-to" && !present {
            diagnostics.push(edge_diagnostic(
                "NDF-XREF-002",
                format!("conforms-to target does not resolve: {}", edge.target),
                edge,
                source,
                "Use a stable identity present in the exact dependency graph.",
            ));
        } else if edge.edge_type == "tracks" && present {
            diagnostics.push(edge_diagnostic(
                "NDF-XREF-004",
                format!(
                    "expected-absent tracked identity is now present: {}",
                    edge.target
                ),
                edge,
                source,
                "Review the upstream contract before changing the consumer clause.",
            ));
        }
    }

    diagnostics.sort_by(|left, right| {
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
    diagnostics
}

fn edge_diagnostic(
    code: &str,
    message: impl Into<String>,
    edge: &Edge,
    source: Option<&Node>,
    repair: &str,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        primary: edge
            .source_span
            .clone()
            .or_else(|| source.map(|node| node.source.clone()))
            .unwrap_or_else(|| SourceSpan::new("<graph>", 1).expect("line one is valid")),
        related: Vec::new(),
        repair: Some(repair.to_owned()),
        subject: Some(edge.source.clone()),
    }
}

fn node_diagnostic(code: &str, message: String, node: &Node, repair: &str) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message,
        primary: node.source.clone(),
        related: Vec::new(),
        repair: Some(repair.to_owned()),
        subject: Some(node.id.clone()),
    }
}

fn contains_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let end = start + word.len();
        let after = text[end..].chars().next();
        !before.is_some_and(is_word_character) && !after.is_some_and(is_word_character)
    })
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}
