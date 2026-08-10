use std::collections::BTreeMap;

use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan};

pub const NODE_COUNT: usize = 100_000;
pub const EDGE_COUNT: usize = 1_000_000;

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
