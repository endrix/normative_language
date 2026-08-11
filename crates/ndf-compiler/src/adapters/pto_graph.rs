use std::collections::{BTreeMap, BTreeSet};

use ndf_core::graph::GraphBuilder;
use ndf_core::identity::NodeId;
use ndf_core::model::{Diagnostic, DiagnosticSeverity, Graph, SourceSpan};

pub(crate) fn validate_pto_graph(graph: Graph) -> Graph {
    let sources = graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.source.clone()))
        .collect::<BTreeMap<_, _>>();
    let units = entity_nodes(&graph, &["unit", "synthetic-unit"]);
    let states = entity_nodes(&graph, &["state"]);
    let mut unit_edges = BTreeMap::<NodeId, BTreeSet<NodeId>>::new();
    let mut state_edges = BTreeMap::<NodeId, BTreeSet<NodeId>>::new();
    let mut unit_errors = BTreeMap::<NodeId, BTreeSet<String>>::new();
    let mut state_errors = BTreeMap::<NodeId, BTreeSet<String>>::new();

    for edge in &graph.edges {
        if edge.edge_type == "depends-on" && units.contains(&edge.source) {
            if units.contains(&edge.target) {
                unit_edges
                    .entry(edge.source.clone())
                    .or_default()
                    .insert(edge.target.clone());
            } else {
                add_error(
                    &mut unit_errors,
                    &edge.source,
                    format!("unresolved PTO ASL unit dependency: {}", edge.target),
                );
            }
        } else if edge.edge_type == "depends-on" && states.contains(&edge.source) {
            if states.contains(&edge.target) {
                state_edges
                    .entry(edge.source.clone())
                    .or_default()
                    .insert(edge.target.clone());
            } else {
                add_error(
                    &mut state_errors,
                    &edge.source,
                    format!("unresolved PTO state dependency: {}", edge.target),
                );
            }
        }
    }

    for state in &states {
        let owners = graph
            .edges
            .iter()
            .filter(|edge| edge.edge_type == "defined-in" && edge.source == *state)
            .map(|edge| edge.target.clone())
            .collect::<BTreeSet<_>>();
        if owners.len() != 1 || owners.iter().any(|owner| !units.contains(owner)) {
            add_error(
                &mut state_errors,
                state,
                "state must have exactly one defined-in unit owner",
            );
            continue;
        }
        let owner = owners.iter().next().expect("one owner was required");
        if !graph.edges.iter().any(|edge| {
            edge.edge_type == "owns-state" && edge.source == *owner && edge.target == *state
        }) {
            add_error(
                &mut state_errors,
                state,
                "state owner is missing the reciprocal owns-state edge",
            );
        }
    }

    for node in cycle_nodes(&units, &unit_edges) {
        add_error(&mut unit_errors, &node, "cyclic PTO ASL unit dependency");
    }
    for node in cycle_nodes(&states, &state_edges) {
        add_error(&mut state_errors, &node, "cyclic PTO state dependency");
    }

    let mut builder = GraphBuilder::default();
    for node in graph.nodes {
        builder.add_node(node);
    }
    for edge in graph.edges {
        builder.add_edge(edge);
    }
    for diagnostic in graph.diagnostics {
        builder.add_diagnostic(diagnostic);
    }
    append_diagnostics(
        &mut builder,
        unit_errors,
        &sources,
        "NDF-ASL-007",
        "Resolve every unit dependency and remove dependency cycles.",
    );
    append_diagnostics(
        &mut builder,
        state_errors,
        &sources,
        "NDF-ASL-009",
        "Resolve every state owner and dependency and remove dependency cycles.",
    );
    builder.finish()
}

fn entity_nodes(graph: &Graph, entities: &[&str]) -> BTreeSet<NodeId> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            node.attributes
                .get("pto_entity")
                .is_some_and(|entity| entities.contains(&entity.as_str()))
        })
        .map(|node| node.id.clone())
        .collect()
}

fn add_error(
    errors: &mut BTreeMap<NodeId, BTreeSet<String>>,
    source: &NodeId,
    message: impl Into<String>,
) {
    errors
        .entry(source.clone())
        .or_default()
        .insert(message.into());
}

fn append_diagnostics(
    builder: &mut GraphBuilder,
    errors: BTreeMap<NodeId, BTreeSet<String>>,
    sources: &BTreeMap<NodeId, SourceSpan>,
    code: &str,
    repair: &str,
) {
    for (subject, messages) in errors {
        let Some(primary) = sources.get(&subject) else {
            continue;
        };
        builder.add_diagnostic(Diagnostic {
            code: code.to_owned(),
            severity: DiagnosticSeverity::Error,
            message: messages.into_iter().collect::<Vec<_>>().join("; "),
            primary: primary.clone(),
            related: Vec::new(),
            repair: Some(repair.to_owned()),
            subject: Some(subject),
        });
    }
}

fn cycle_nodes(
    nodes: &BTreeSet<NodeId>,
    edges: &BTreeMap<NodeId, BTreeSet<NodeId>>,
) -> BTreeSet<NodeId> {
    let mut incoming = nodes
        .iter()
        .map(|node| (node.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    for targets in edges.values() {
        for target in targets {
            if let Some(count) = incoming.get_mut(target) {
                *count += 1;
            }
        }
    }
    let mut ready = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(node, _)| node.clone())
        .collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::new();
    while let Some(node) = ready.pop_first() {
        visited.insert(node.clone());
        if let Some(targets) = edges.get(&node) {
            for target in targets {
                let count = incoming
                    .get_mut(target)
                    .expect("validated dependency target is a known node");
                *count -= 1;
                if *count == 0 {
                    ready.insert(target.clone());
                }
            }
        }
    }
    nodes.difference(&visited).cloned().collect()
}
