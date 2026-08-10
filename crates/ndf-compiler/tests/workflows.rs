use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ndf_compiler::diff::semantic_diff;
use ndf_compiler::export::export_graph;
use ndf_compiler::index::{BuildProvenance, NdfIndex, build_index};
use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan};

fn provenance() -> BuildProvenance {
    BuildProvenance {
        project_commits: vec![("fixture".to_owned(), "0".repeat(40))],
        format_version: "0.2".to_owned(),
        ir_version: "0.1".to_owned(),
        tool_version: "0.1.0".to_owned(),
        normative_roots: Vec::new(),
    }
}

fn node(local_id: &str, title: &str, path: &str, status: LifecycleStatus) -> Node {
    Node {
        id: NodeId::new("fixture", local_id).unwrap(),
        kind: NodeKind::Architecture,
        title: title.to_owned(),
        source: SourceSpan::new(path, 10).unwrap(),
        modality: None,
        refinement: None,
        domain: None,
        status,
        owner: Some("core".to_owned()),
        body: format!("Body for {local_id}"),
        attributes: BTreeMap::new(),
    }
}

#[test]
fn semantic_diff_classifies_node_and_edge_changes() {
    let directory = tempfile::tempdir().unwrap();
    let before_path = directory.path().join("before.sqlite");
    let after_path = directory.path().join("after.sqlite");
    let before_nodes = vec![
        node("MOVED", "Moved", "docs/old.md", LifecycleStatus::Active),
        node(
            "MODIFIED",
            "Before",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
        node(
            "REMOVED",
            "Removed",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
        node(
            "SUPERSEDED",
            "Superseded",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
        node(
            "EDGE-A",
            "Edge A",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
        node(
            "EDGE-B",
            "Edge B",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
    ];
    let after_nodes = vec![
        node("MOVED", "Moved", "docs/new.md", LifecycleStatus::Active),
        node(
            "MODIFIED",
            "After",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
        node("ADDED", "Added", "docs/design.md", LifecycleStatus::Active),
        node(
            "SUPERSEDED",
            "Superseded",
            "docs/design.md",
            LifecycleStatus::Superseded,
        ),
        node(
            "EDGE-A",
            "Edge A",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
        node(
            "EDGE-B",
            "Edge B",
            "docs/design.md",
            LifecycleStatus::Active,
        ),
    ];
    let edge = Edge::new(
        NodeId::new("fixture", "EDGE-A").unwrap(),
        NodeId::new("fixture", "EDGE-B").unwrap(),
        "depends-on",
        None,
    )
    .unwrap();
    build_index(
        &Graph {
            nodes: before_nodes,
            ..Graph::default()
        },
        &before_path,
        &provenance(),
    )
    .unwrap();
    build_index(
        &Graph {
            nodes: after_nodes,
            edges: vec![edge],
            diagnostics: Vec::new(),
        },
        &after_path,
        &provenance(),
    )
    .unwrap();

    let before = NdfIndex::open(&before_path).unwrap();
    let after = NdfIndex::open(&after_path).unwrap();
    let result = semantic_diff(&before, &after).unwrap();

    assert_eq!(result.added, ["ndf://fixture/ADDED"]);
    assert_eq!(result.removed, ["ndf://fixture/REMOVED"]);
    assert_eq!(result.modified, ["ndf://fixture/MODIFIED"]);
    assert_eq!(result.moved, ["ndf://fixture/MOVED"]);
    assert_eq!(result.superseded, ["ndf://fixture/SUPERSEDED"]);
    assert_eq!(
        result.edge_only,
        ["ndf://fixture/EDGE-A", "ndf://fixture/EDGE-B"]
    );
    assert_eq!(
        result.edges_added,
        [[
            "ndf://fixture/EDGE-A".to_owned(),
            "depends-on".to_owned(),
            "ndf://fixture/EDGE-B".to_owned(),
        ]]
    );
}

#[test]
fn export_is_deterministic_portable_and_carries_provenance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let index_path = root.join("graph.sqlite");
    let first_path = root.join("first.json");
    let second_path = root.join("second.json");
    let graph = Graph {
        nodes: vec![node(
            "NODE-001",
            "Node",
            &root.join("docs/node.md").to_string_lossy(),
            LifecycleStatus::Active,
        )],
        ..Graph::default()
    };
    build_index(&graph, &index_path, &provenance()).unwrap();
    let index = NdfIndex::open(&index_path).unwrap();

    let first = export_graph(&index, &first_path).unwrap();
    let second = export_graph(&index, &second_path).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&first_path).unwrap()).unwrap();

    assert_eq!(
        fs::read(&first_path).unwrap(),
        fs::read(&second_path).unwrap()
    );
    assert_eq!(first.semantic_hash, second.semantic_hash);
    assert_eq!(payload["provenance"]["format_version"], "0.2");
    assert_eq!(payload["nodes"][0]["id"], "ndf://fixture/NODE-001");
    assert!(
        !fs::read_to_string(&first_path)
            .unwrap()
            .contains(root.to_string_lossy().as_ref())
    );
    assert_eq!(first.output, Path::new(&first_path));
    assert_eq!(first.node_count, 1);
    assert_eq!(first.edge_count, 0);
}
