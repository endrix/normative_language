use std::collections::HashSet;

use serde::Serialize;

use crate::CompilerError;
use crate::index::{IndexedEdge, NdfIndex, NodeRecord};

const QUERY_FIELDS: &[&str] = &[
    "kind",
    "modality",
    "refinement",
    "domain",
    "status",
    "owner",
];

pub fn query_nodes(index: &NdfIndex, expression: &str) -> Result<Vec<NodeRecord>, CompilerError> {
    let mut filters = Vec::new();
    for clause in expression.split(" and ") {
        let Some((field, value)) = clause.split_once('=') else {
            return Err(CompilerError::Contract(format!(
                "invalid query clause: {clause}"
            )));
        };
        let field = field.trim();
        let value = value.trim();
        if !QUERY_FIELDS.contains(&field) {
            return Err(CompilerError::Contract(format!(
                "unsupported query field: {field}"
            )));
        }
        filters.push((field, value));
    }
    index.filtered_nodes(&filters)
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct TraceEdge {
    pub source: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub target: String,
}

impl From<IndexedEdge> for TraceEdge {
    fn from(edge: IndexedEdge) -> Self {
        Self {
            source: edge.source,
            edge_type: edge.edge_type,
            target: edge.target,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct TraceTree {
    pub root: String,
    pub nodes: Vec<String>,
    pub edges: Vec<TraceEdge>,
    pub cycles: Vec<TraceEdge>,
}

struct Frame {
    uri: String,
    edges: Vec<IndexedEdge>,
    next_edge: usize,
}

pub fn trace(index: &NdfIndex, uri: &str, edge_types: &[&str]) -> Result<TraceTree, CompilerError> {
    let root = index.resolve_uri(uri)?;
    let mut visited = HashSet::from([root.clone()]);
    let mut active = HashSet::from([root.clone()]);
    let mut nodes = vec![root.clone()];
    let mut edges = Vec::new();
    let mut cycles = Vec::new();
    let mut stack = vec![Frame {
        edges: index.out_edges_by_types(&root, edge_types)?,
        uri: root.clone(),
        next_edge: 0,
    }];

    while let Some(frame) = stack.last_mut() {
        if frame.next_edge == frame.edges.len() {
            let completed = stack.pop().expect("stack contains current frame");
            active.remove(&completed.uri);
            continue;
        }
        let edge = frame.edges[frame.next_edge].clone();
        frame.next_edge += 1;
        let trace_edge = TraceEdge::from(edge.clone());
        edges.push(trace_edge.clone());
        if active.contains(&edge.target) {
            cycles.push(trace_edge);
            continue;
        }
        if visited.insert(edge.target.clone()) {
            nodes.push(edge.target.clone());
            active.insert(edge.target.clone());
            stack.push(Frame {
                edges: index.out_edges_by_types(&edge.target, edge_types)?,
                uri: edge.target,
                next_edge: 0,
            });
        }
    }

    Ok(TraceTree {
        root,
        nodes,
        edges,
        cycles,
    })
}
