use std::collections::BTreeMap;

use ndf_compiler::rules::{ValidationPolicy, validate_graph};
use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Modality, Node, NodeKind, SourceSpan};

fn requirement(body: &str, owner: Option<&str>) -> Node {
    Node {
        id: NodeId::new("fixture", "PIPE-001").unwrap(),
        kind: NodeKind::Requirement,
        title: "Issue contract".to_owned(),
        source: SourceSpan::new("docs/pipeline.md", 10).unwrap(),
        modality: Some(Modality::Must),
        refinement: Some("L1".to_owned()),
        domain: Some("core".to_owned()),
        status: LifecycleStatus::Active,
        owner: owner.map(str::to_owned),
        body: body.to_owned(),
        attributes: BTreeMap::new(),
    }
}

fn codes(graph: &Graph) -> Vec<String> {
    validate_graph(graph, &ValidationPolicy::default())
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn dangling_mandatory_edge_is_rejected() {
    let source = requirement("The pipeline MUST issue in order.", Some("core"));
    let edge = Edge::new(
        source.id.clone(),
        NodeId::new("fixture", "PIPE-MISSING").unwrap(),
        "refines",
        None,
    )
    .unwrap();
    let graph = Graph {
        nodes: vec![source],
        edges: vec![edge],
        diagnostics: Vec::new(),
    };

    assert_eq!(codes(&graph), ["NDF-EDGE-001"]);
}

#[test]
fn must_and_information_language_are_checked_in_both_directions() {
    let must = requirement("The pipeline issues in order.", Some("core"));
    let information = Node {
        id: NodeId::new("fixture", "PIPE-INFO-001").unwrap(),
        kind: NodeKind::Information,
        title: "Background".to_owned(),
        source: SourceSpan::new("docs/pipeline.md", 20).unwrap(),
        modality: None,
        refinement: None,
        domain: None,
        status: LifecycleStatus::Draft,
        owner: None,
        body: "The implementation MUST issue in order.".to_owned(),
        attributes: BTreeMap::new(),
    };
    let graph = Graph {
        nodes: vec![information, must],
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };

    assert_eq!(codes(&graph), ["NDF-MOD-001", "NDF-MOD-002"]);
}

#[test]
fn lifecycle_and_owner_invariants_are_enforced() {
    let mut node = requirement("The pipeline MUST issue in order.", None);
    node.status = LifecycleStatus::Draft;
    node.attributes
        .insert("previous_status".to_owned(), "active".to_owned());
    let graph = Graph {
        nodes: vec![node],
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };

    assert_eq!(codes(&graph), ["NDF-LIFE-001", "NDF-OWN-001"]);
}
