use std::collections::BTreeMap;

use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan};

pub const NODE_COUNT: usize = 100_000;
pub const EDGE_COUNT: usize = 1_000_000;
pub const CONSUMER_NODE_COUNT: usize = 10_000;
pub const CONFORMANCE_EDGE_COUNT: usize = CONSUMER_NODE_COUNT / 10;

pub fn graph() -> Graph {
    let nodes: Vec<_> = (0..NODE_COUNT)
        .map(|index| Node {
            id: NodeId::new("scale", format!("NODE-{index:06}")).unwrap(),
            kind: NodeKind::Information,
            title: format!("Node {index}"),
            source: SourceSpan::new("generated/scale.md", (index + 1) as u32).unwrap(),
            modality: None,
            refinement: None,
            domain: None,
            status: LifecycleStatus::Active,
            owner: None,
            body: String::new(),
            attributes: BTreeMap::new(),
        })
        .collect();
    let edges = (0..EDGE_COUNT)
        .map(|index| {
            Edge::new(
                nodes[index % NODE_COUNT].id.clone(),
                nodes[(index + 1) % NODE_COUNT].id.clone(),
                "references",
                None,
            )
            .unwrap()
        })
        .collect();
    Graph {
        nodes,
        edges,
        diagnostics: Vec::new(),
    }
}

pub fn pto_graph(modified: bool) -> Graph {
    let nodes = (0..CONFORMANCE_EDGE_COUNT)
        .map(|index| {
            let handler = if modified && index == 0 {
                "ExecuteTileOperationV2"
            } else {
                "ExecuteTileOperation"
            };
            Node {
                id: NodeId::new("pto-spec", format!("PTO-INST-TILE-T{index:04}")).unwrap(),
                kind: NodeKind::Definition,
                title: format!("Tile operation {index}"),
                source: SourceSpan::new("asl/tile/generated.asl", (index + 1) as u32).unwrap(),
                modality: None,
                refinement: None,
                domain: Some("tile".to_owned()),
                status: LifecycleStatus::Active,
                owner: Some("pto-spec".to_owned()),
                body: String::new(),
                attributes: BTreeMap::from([
                    ("semantic_handler".to_owned(), handler.to_owned()),
                    (
                        "semantic_facets".to_owned(),
                        "instruction-semantics".to_owned(),
                    ),
                ]),
            }
        })
        .collect();
    Graph {
        nodes,
        edges: Vec::new(),
        diagnostics: Vec::new(),
    }
}

pub fn consumer_graph() -> Graph {
    let nodes: Vec<_> = (0..CONSUMER_NODE_COUNT)
        .map(|index| Node {
            id: NodeId::new("davincioo", format!("DAV-NODE-{index:05}")).unwrap(),
            kind: NodeKind::Architecture,
            title: format!("Consumer node {index}"),
            source: SourceSpan::new("docs/generated.md", (index + 1) as u32).unwrap(),
            modality: None,
            refinement: None,
            domain: None,
            status: LifecycleStatus::Active,
            owner: Some("davincioo".to_owned()),
            body: String::new(),
            attributes: BTreeMap::new(),
        })
        .collect();
    let mut edges: Vec<_> = (0..CONFORMANCE_EDGE_COUNT)
        .map(|index| {
            Edge::new(
                nodes[index].id.clone(),
                NodeId::new("pto-spec", format!("PTO-INST-TILE-T{index:04}")).unwrap(),
                "conforms-to",
                None,
            )
            .unwrap()
        })
        .collect();
    edges.extend((1..CONSUMER_NODE_COUNT).map(|index| {
        Edge::new(
            nodes[index].id.clone(),
            nodes[index - 1].id.clone(),
            if index % 2 == 0 {
                "refines"
            } else {
                "verifies"
            },
            None,
        )
        .unwrap()
    }));
    Graph {
        nodes,
        edges,
        diagnostics: Vec::new(),
    }
}
