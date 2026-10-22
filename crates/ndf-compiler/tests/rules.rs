use std::collections::BTreeMap;

use ndf_compiler::dependencies::DependencyGraph;
use ndf_compiler::rules::{ValidationPolicy, validate_graph, validate_project_graph};
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

fn project_codes(graph: &Graph, dependencies: &BTreeMap<String, DependencyGraph>) -> Vec<String> {
    validate_project_graph(graph, dependencies, &ValidationPolicy::default())
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn dependency_graph(nodes: Vec<Node>) -> BTreeMap<String, DependencyGraph> {
    BTreeMap::from([(
        "pto-spec".to_owned(),
        DependencyGraph {
            project_id: "pto-spec".to_owned(),
            revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            path: std::path::PathBuf::from("deps/pto-spec"),
            graph: Graph {
                nodes,
                edges: Vec::new(),
                diagnostics: Vec::new(),
            },
        },
    )])
}

fn external_contract(local_id: &str) -> Node {
    Node {
        id: NodeId::new("pto-spec", local_id).unwrap(),
        kind: NodeKind::Definition,
        title: "PTO contract".to_owned(),
        source: SourceSpan::new("asl/contracts.asl", 1).unwrap(),
        modality: None,
        refinement: Some("L1".to_owned()),
        domain: Some("tile".to_owned()),
        status: LifecycleStatus::Active,
        owner: Some("pto-spec".to_owned()),
        body: String::new(),
        attributes: BTreeMap::new(),
    }
}

fn cross_project_graph(edge_type: &str, target: NodeId, expected_absent: bool) -> Graph {
    let mut source = requirement("DavinciOO MUST preserve the PTO contract.", Some("core"));
    if expected_absent {
        source.modality = Some(Modality::Tbd);
        source
            .attributes
            .insert("expected".to_owned(), "absent".to_owned());
    }
    let edge = Edge::new(
        source.id.clone(),
        target,
        edge_type,
        Some(source.source.clone()),
    )
    .unwrap();
    Graph {
        nodes: vec![source],
        edges: vec![edge],
        diagnostics: Vec::new(),
    }
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
fn must_keyword_rule_applies_only_to_normative_statement_kinds() {
    let mut definition = requirement("A CELL is one tile register.", Some("core"));
    definition.id = NodeId::new("fixture", "DEF-CELL-001").unwrap();
    definition.kind = NodeKind::Definition;
    let mut architecture = requirement("The core has four PEs.", Some("core"));
    architecture.id = NodeId::new("fixture", "ARCH-TOP-001").unwrap();
    architecture.kind = NodeKind::Architecture;
    let graph = Graph {
        nodes: vec![definition, architecture],
        edges: Vec::new(),
        diagnostics: Vec::new(),
    };

    assert_eq!(codes(&graph), Vec::<String>::new());
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

#[test]
fn resolved_conformance_target_passes() {
    let target = external_contract("PTO-INST-TILE-TLOAD");
    let graph = cross_project_graph("conforms-to", target.id.clone(), false);

    assert_eq!(
        project_codes(&graph, &dependency_graph(vec![target])),
        Vec::<String>::new()
    );
}

#[test]
fn missing_conformance_target_is_rejected() {
    let graph = cross_project_graph(
        "conforms-to",
        NodeId::new("pto-spec", "PTO-INST-TILE-MISSING").unwrap(),
        false,
    );

    assert_eq!(
        project_codes(&graph, &dependency_graph(Vec::new())),
        ["NDF-XREF-002"]
    );
}

#[test]
fn target_in_undeclared_project_is_rejected() {
    let graph = cross_project_graph(
        "conforms-to",
        NodeId::new("another-spec", "EXT-001").unwrap(),
        false,
    );

    assert_eq!(project_codes(&graph, &BTreeMap::new()), ["NDF-XREF-003"]);
}

#[test]
fn expected_absent_watch_passes_while_target_is_missing() {
    let graph = cross_project_graph(
        "tracks",
        NodeId::new("pto-spec", "PTO-INST-TILE-PEXT").unwrap(),
        true,
    );

    assert_eq!(
        project_codes(&graph, &dependency_graph(Vec::new())),
        Vec::<String>::new()
    );
}

#[test]
fn expected_absent_watch_requires_review_when_target_appears() {
    let target = external_contract("PTO-INST-TILE-PEXT");
    let graph = cross_project_graph("tracks", target.id.clone(), true);

    assert_eq!(
        project_codes(&graph, &dependency_graph(vec![target])),
        ["NDF-XREF-004"]
    );
}

#[test]
fn expected_absent_watch_cannot_target_the_consumer_project() {
    let graph = cross_project_graph(
        "tracks",
        NodeId::new("fixture", "PIPE-PROSPECTIVE-001").unwrap(),
        true,
    );

    assert_eq!(project_codes(&graph, &BTreeMap::new()), ["NDF-XREF-001"]);
}
