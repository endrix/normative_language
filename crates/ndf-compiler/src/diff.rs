use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::CompilerError;
use crate::index::{NdfIndex, NodeRecord};

pub type EdgeTuple = [String; 3];
type SemanticRecord<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
    Option<&'a str>,
    &'a str,
);

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SemanticDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
    pub moved: Vec<String>,
    pub superseded: Vec<String>,
    pub edge_only: Vec<String>,
    pub edges_added: Vec<EdgeTuple>,
    pub edges_removed: Vec<EdgeTuple>,
}

fn semantic_record(node: &NodeRecord) -> SemanticRecord<'_> {
    (
        &node.kind,
        &node.title,
        node.modality.as_deref(),
        node.refinement.as_deref(),
        node.domain.as_deref(),
        &node.status,
        node.owner.as_deref(),
        &node.body,
    )
}

fn edge_set(index: &NdfIndex) -> Result<BTreeSet<EdgeTuple>, CompilerError> {
    Ok(index
        .all_edges()?
        .into_iter()
        .map(|edge| [edge.source, edge.edge_type, edge.target])
        .collect())
}

pub fn semantic_diff(before: &NdfIndex, after: &NdfIndex) -> Result<SemanticDiff, CompilerError> {
    let before_nodes: BTreeMap<_, _> = before
        .all_nodes()?
        .into_iter()
        .map(|node| (node.id.clone(), node))
        .collect();
    let after_nodes: BTreeMap<_, _> = after
        .all_nodes()?
        .into_iter()
        .map(|node| (node.id.clone(), node))
        .collect();
    let before_uris: BTreeSet<_> = before_nodes.keys().cloned().collect();
    let after_uris: BTreeSet<_> = after_nodes.keys().cloned().collect();
    let added: BTreeSet<_> = after_uris.difference(&before_uris).cloned().collect();
    let removed: BTreeSet<_> = before_uris.difference(&after_uris).cloned().collect();
    let mut modified = BTreeSet::new();
    let mut moved = BTreeSet::new();
    let mut superseded = BTreeSet::new();

    for uri in before_uris.intersection(&after_uris) {
        let old = &before_nodes[uri];
        let new = &after_nodes[uri];
        if old.status != "superseded" && new.status == "superseded" {
            superseded.insert(uri.clone());
        } else if semantic_record(old) != semantic_record(new) {
            modified.insert(uri.clone());
        } else if old.source != new.source {
            moved.insert(uri.clone());
        }
    }

    let old_edges = edge_set(before)?;
    let new_edges = edge_set(after)?;
    let edges_added: BTreeSet<_> = new_edges.difference(&old_edges).cloned().collect();
    let edges_removed: BTreeSet<_> = old_edges.difference(&new_edges).cloned().collect();
    let edge_nodes: BTreeSet<_> = edges_added
        .union(&edges_removed)
        .flat_map(|edge| [edge[0].clone(), edge[2].clone()])
        .collect();
    let classified: BTreeSet<_> = added
        .iter()
        .chain(&removed)
        .chain(&modified)
        .chain(&moved)
        .chain(&superseded)
        .cloned()
        .collect();
    let edge_only = edge_nodes.difference(&classified).cloned().collect();

    Ok(SemanticDiff {
        added: added.into_iter().collect(),
        removed: removed.into_iter().collect(),
        modified: modified.into_iter().collect(),
        moved: moved.into_iter().collect(),
        superseded: superseded.into_iter().collect(),
        edge_only,
        edges_added: edges_added.into_iter().collect(),
        edges_removed: edges_removed.into_iter().collect(),
    })
}
