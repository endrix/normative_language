use std::collections::{BTreeSet, HashMap};

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Graph, LifecycleStatus, Modality, Node, NodeKind, SourceSpan,
};

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
        if node.modality == Some(Modality::Must) && !contains_word(&node.body, "MUST") {
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
