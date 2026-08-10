use std::collections::HashMap;

use crate::identity::NodeId;
use crate::model::{Diagnostic, DiagnosticSeverity, Edge, Graph, Node};

#[derive(Debug, Default)]
pub struct GraphBuilder {
    nodes: HashMap<NodeId, Node>,
    edges: Vec<Edge>,
    diagnostics: Vec<Diagnostic>,
}

impl GraphBuilder {
    pub fn add_node(&mut self, node: Node) {
        if let Some(existing) = self.nodes.get(&node.id) {
            self.diagnostics.push(Diagnostic {
                code: "NDF-ID-001".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!("duplicate canonical NDF ID: {}", node.id),
                primary: node.source,
                related: vec![existing.source.clone()],
                repair: Some(
                    "Assign a unique explicit ID or remove the duplicate clause.".to_owned(),
                ),
                subject: None,
            });
            return;
        }
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn finish(self) -> Graph {
        let mut nodes: Vec<_> = self.nodes.into_values().collect();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));

        let mut edges = self.edges;
        edges.sort_by(|left, right| {
            (&left.source, &left.edge_type, &left.target).cmp(&(
                &right.source,
                &right.edge_type,
                &right.target,
            ))
        });

        let mut diagnostics = self.diagnostics;
        diagnostics.sort_by(|left, right| {
            (&left.primary.path, left.primary.line, &left.code).cmp(&(
                &right.primary.path,
                right.primary.line,
                &right.code,
            ))
        });

        Graph {
            nodes,
            edges,
            diagnostics,
        }
    }
}
