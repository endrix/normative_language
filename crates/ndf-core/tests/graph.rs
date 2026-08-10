use std::collections::BTreeMap;

use ndf_core::graph::GraphBuilder;
use ndf_core::identity::NodeId;
use ndf_core::model::{DiagnosticSeverity, Edge, LifecycleStatus, Node, NodeKind, SourceSpan};
use serde_json::json;

fn node(local_id: &str, line: u32) -> Node {
    Node {
        id: NodeId::new("fixture", local_id).unwrap(),
        kind: NodeKind::Requirement,
        title: local_id.to_owned(),
        source: SourceSpan::new("docs/design.md", line).unwrap(),
        modality: None,
        refinement: Some("L1".to_owned()),
        domain: Some("core".to_owned()),
        status: LifecycleStatus::Active,
        owner: Some("core".to_owned()),
        body: "The core MUST preserve behavior.".to_owned(),
        attributes: BTreeMap::new(),
    }
}

#[test]
fn duplicate_id_keeps_the_first_node_and_emits_ndf_id_001() {
    let mut builder = GraphBuilder::default();
    builder.add_node(node("REQ-001", 4));
    builder.add_node(node("REQ-001", 9));

    let graph = builder.finish();

    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].source.line, 4);
    assert_eq!(graph.diagnostics.len(), 1);
    assert_eq!(graph.diagnostics[0].code, "NDF-ID-001");
    assert_eq!(graph.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(graph.diagnostics[0].primary.line, 9);
    assert_eq!(graph.diagnostics[0].related[0].line, 4);
}

#[test]
fn finish_sorts_nodes_and_serializes_explicit_null_fields() {
    let mut builder = GraphBuilder::default();
    builder.add_node(node("REQ-020", 20));
    builder.add_node(node("REQ-010", 10));

    let graph = builder.finish();
    let value = serde_json::to_value(&graph).unwrap();

    assert_eq!(graph.nodes[0].id.local_id(), "REQ-010");
    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["nodes"][0]["modality"], json!(null));
    assert_eq!(value["nodes"][0]["source"]["end_line"], json!(null));
    assert_eq!(value["nodes"][0]["attributes"], json!({}));
}

#[test]
fn source_span_rejects_line_zero() {
    let error = SourceSpan::new("docs/design.md", 0).unwrap_err();

    assert_eq!(error.to_string(), "line must be positive");
}

#[test]
fn edge_rejects_an_empty_type() {
    let source = NodeId::new("fixture", "REQ-001").unwrap();
    let target = NodeId::new("fixture", "REQ-002").unwrap();

    let error = Edge::new(source, target, "", None).unwrap_err();

    assert_eq!(error.to_string(), "edge type must not be empty");
}
