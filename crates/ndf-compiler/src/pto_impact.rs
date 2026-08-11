use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use crate::CompilerError;
use crate::index::{NdfIndex, NodeRecord};

const PROPAGATION_EDGES: &[&str] = &[
    "affects",
    "blocks",
    "depends-on",
    "implements",
    "references",
    "refines",
    "verifies",
];

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Compatibility {
    Compatible,
    ReviewRequired,
    Breaking,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PtoChangeKind {
    Added,
    Removed,
    Modified,
    Moved,
    Superseded,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct PtoChange {
    pub uri: String,
    pub kind: PtoChangeKind,
    pub facets: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ConsumerImpact {
    pub uri: String,
    pub direct: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ConsumerConformance {
    pub consumer_uri: String,
    pub target_uri: String,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ConsumerWatch {
    pub consumer_uri: String,
    pub target_uri: String,
    pub expected: String,
    pub present: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct PtoReleaseImpact {
    pub schema_version: String,
    pub compatibility: Compatibility,
    pub conformance_targets: Vec<ConsumerConformance>,
    pub absence_watches: Vec<ConsumerWatch>,
    pub changes: Vec<PtoChange>,
    pub affected_consumers: Vec<ConsumerImpact>,
    pub presence_changes: Vec<String>,
    pub deterministic_actions: Vec<String>,
    pub human_decisions: Vec<String>,
}

pub fn pto_release_impact(
    before: &NdfIndex,
    after: &NdfIndex,
    consumer: &NdfIndex,
) -> Result<PtoReleaseImpact, CompilerError> {
    let before_nodes = nodes_by_uri(before)?;
    let after_nodes = nodes_by_uri(after)?;
    let consumer_nodes = nodes_by_uri(consumer)?;
    let before_uris: BTreeSet<_> = before_nodes.keys().cloned().collect();
    let after_uris: BTreeSet<_> = after_nodes.keys().cloned().collect();
    let superseded_targets: BTreeSet<_> = after
        .all_edges()?
        .into_iter()
        .filter(|edge| edge.edge_type == "supersedes")
        .map(|edge| edge.target)
        .collect();

    let mut changes = Vec::new();
    for uri in after_uris.difference(&before_uris) {
        changes.push(PtoChange {
            uri: uri.clone(),
            kind: PtoChangeKind::Added,
            facets: node_facets(&after_nodes[uri]),
        });
    }
    for uri in before_uris.difference(&after_uris) {
        changes.push(PtoChange {
            uri: uri.clone(),
            kind: PtoChangeKind::Removed,
            facets: node_facets(&before_nodes[uri]),
        });
    }
    for uri in before_uris.intersection(&after_uris) {
        let old = &before_nodes[uri];
        let new = &after_nodes[uri];
        let kind = if (old.status != "superseded" && new.status == "superseded")
            || superseded_targets.contains(uri)
        {
            Some(PtoChangeKind::Superseded)
        } else if !semantically_equal(old, new) {
            Some(PtoChangeKind::Modified)
        } else if old.source != new.source {
            Some(PtoChangeKind::Moved)
        } else {
            None
        };
        if let Some(kind) = kind {
            changes.push(PtoChange {
                uri: uri.clone(),
                kind,
                facets: if kind == PtoChangeKind::Modified {
                    modified_facets(old, new)
                } else {
                    merged_facets(old, new)
                },
            });
        }
    }
    changes.sort_by(|left, right| (&left.uri, left.kind).cmp(&(&right.uri, right.kind)));

    let consumer_edges = consumer.all_edges()?;
    let mut conformance_targets: Vec<_> = consumer_edges
        .iter()
        .filter(|edge| edge.edge_type == "conforms-to")
        .map(|edge| ConsumerConformance {
            consumer_uri: edge.source.clone(),
            target_uri: edge.target.clone(),
        })
        .collect();
    conformance_targets.sort();
    let mut absence_watches: Vec<_> = consumer_edges
        .iter()
        .filter(|edge| edge.edge_type == "tracks")
        .map(|edge| ConsumerWatch {
            consumer_uri: edge.source.clone(),
            target_uri: edge.target.clone(),
            expected: consumer_nodes
                .get(&edge.source)
                .and_then(|node| node.attributes.get("expected"))
                .cloned()
                .unwrap_or_else(|| "unspecified".to_owned()),
            present: after_uris.contains(&edge.target),
        })
        .collect();
    absence_watches.sort();

    let changes_by_uri: BTreeMap<_, Vec<_>> = changes.iter().fold(
        BTreeMap::<&str, Vec<&PtoChange>>::new(),
        |mut result, change| {
            result.entry(&change.uri).or_default().push(change);
            result
        },
    );
    let mut compatibility = Compatibility::Compatible;
    let mut direct_reasons: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for conformance in &conformance_targets {
        for change in changes_by_uri
            .get(conformance.target_uri.as_str())
            .into_iter()
            .flatten()
        {
            let required = match change.kind {
                PtoChangeKind::Removed | PtoChangeKind::Modified => Compatibility::Breaking,
                PtoChangeKind::Moved | PtoChangeKind::Superseded | PtoChangeKind::Added => {
                    Compatibility::ReviewRequired
                }
            };
            compatibility = compatibility.max(required);
            direct_reasons
                .entry(conformance.consumer_uri.clone())
                .or_default()
                .insert(format!(
                    "{} conformance target: {}",
                    change_kind_label(change.kind),
                    change.uri
                ));
        }
    }

    let mut presence_changes = BTreeSet::new();
    for watch in &absence_watches {
        if watch.expected == "absent" && !before_uris.contains(&watch.target_uri) && watch.present {
            compatibility = compatibility.max(Compatibility::ReviewRequired);
            presence_changes.insert(watch.target_uri.clone());
            direct_reasons
                .entry(watch.consumer_uri.clone())
                .or_default()
                .insert(format!(
                    "expected-absent target became present: {}",
                    watch.target_uri
                ));
        }
    }

    let affected_consumers =
        propagate_consumer_impact(&consumer_nodes, &consumer_edges, &direct_reasons);
    let (deterministic_actions, human_decisions) = actions(compatibility);

    Ok(PtoReleaseImpact {
        schema_version: "1".to_owned(),
        compatibility,
        conformance_targets,
        absence_watches,
        changes,
        affected_consumers,
        presence_changes: presence_changes.into_iter().collect(),
        deterministic_actions,
        human_decisions,
    })
}

fn nodes_by_uri(index: &NdfIndex) -> Result<BTreeMap<String, NodeRecord>, CompilerError> {
    Ok(index
        .all_nodes()?
        .into_iter()
        .map(|node| (node.id.clone(), node))
        .collect())
}

fn semantically_equal(left: &NodeRecord, right: &NodeRecord) -> bool {
    left.kind == right.kind
        && left.title == right.title
        && left.modality == right.modality
        && left.refinement == right.refinement
        && left.domain == right.domain
        && left.status == right.status
        && left.owner == right.owner
        && left.body == right.body
        && left.attributes == right.attributes
}

fn node_facets(node: &NodeRecord) -> Vec<String> {
    node.attributes
        .get("semantic_facets")
        .into_iter()
        .flat_map(|value| value.split(','))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn merged_facets(left: &NodeRecord, right: &NodeRecord) -> Vec<String> {
    node_facets(left)
        .into_iter()
        .chain(node_facets(right))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn modified_facets(left: &NodeRecord, right: &NodeRecord) -> Vec<String> {
    let mut facets = BTreeSet::new();
    if left.attributes.get("semantic_handler") != right.attributes.get("semantic_handler") {
        facets.insert("instruction-semantics".to_owned());
    }
    if left.attributes.get("semantic_facets") != right.attributes.get("semantic_facets") {
        facets.extend(merged_facets(left, right));
    }
    if facets.is_empty() {
        facets.extend(merged_facets(left, right));
    }
    if facets.is_empty() {
        facets.insert("contract".to_owned());
    }
    facets.into_iter().collect()
}

fn propagate_consumer_impact(
    consumer_nodes: &BTreeMap<String, NodeRecord>,
    consumer_edges: &[crate::index::IndexedEdge],
    direct_reasons: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<ConsumerImpact> {
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in consumer_edges {
        if PROPAGATION_EDGES.contains(&edge.edge_type.as_str())
            && consumer_nodes.contains_key(&edge.source)
            && consumer_nodes.contains_key(&edge.target)
        {
            adjacency
                .entry(&edge.source)
                .or_default()
                .insert(&edge.target);
            adjacency
                .entry(&edge.target)
                .or_default()
                .insert(&edge.source);
        }
    }

    let mut affected: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (direct, reasons) in direct_reasons {
        let mut queue = VecDeque::from([direct.as_str()]);
        let mut visited = BTreeSet::new();
        while let Some(uri) = queue.pop_front() {
            if !visited.insert(uri) {
                continue;
            }
            affected
                .entry(uri.to_owned())
                .or_default()
                .extend(reasons.iter().cloned());
            if let Some(neighbors) = adjacency.get(uri) {
                queue.extend(neighbors.iter().copied());
            }
        }
    }

    affected
        .into_iter()
        .map(|(uri, reasons)| ConsumerImpact {
            direct: direct_reasons.contains_key(&uri),
            uri,
            reasons: reasons.into_iter().collect(),
        })
        .collect()
}

fn change_kind_label(kind: PtoChangeKind) -> &'static str {
    match kind {
        PtoChangeKind::Added => "added",
        PtoChangeKind::Removed => "removed",
        PtoChangeKind::Modified => "modified",
        PtoChangeKind::Moved => "moved",
        PtoChangeKind::Superseded => "superseded",
    }
}

fn actions(compatibility: Compatibility) -> (Vec<String>, Vec<String>) {
    match compatibility {
        Compatibility::Compatible => (
            vec!["record compatible PTO release impact".to_owned()],
            Vec::new(),
        ),
        Compatibility::ReviewRequired => (
            vec!["hold automatic PTO release adoption pending review".to_owned()],
            vec!["review moved, superseded, or newly present PTO contracts".to_owned()],
        ),
        Compatibility::Breaking => (
            vec![
                "block automatic PTO release adoption".to_owned(),
                "rebuild and validate affected consumer closure".to_owned(),
            ],
            vec!["approve each consumer conformance migration or waiver".to_owned()],
        ),
    }
}
