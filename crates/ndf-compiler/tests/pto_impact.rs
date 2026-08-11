use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::index::{BuildProvenance, NdfIndex, build_index};
use ndf_compiler::pto_impact::{Compatibility, PtoChangeKind, pto_release_impact};
use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan};
use serde_json::Value;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> Value {
    serde_json::from_str(
        &fs::read_to_string(repository().join("tests/fixtures/pto-impact").join(name)).unwrap(),
    )
    .unwrap()
}

fn graph(value: &Value) -> Graph {
    let nodes = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let id = value["id"].as_str().unwrap();
            let mut attributes = BTreeMap::new();
            for name in ["semantic_handler", "semantic_facets", "expected"] {
                if let Some(value) = value.get(name).and_then(Value::as_str) {
                    attributes.insert(name.to_owned(), value.to_owned());
                }
            }
            Node {
                id: NodeId::parse(id, None).unwrap(),
                kind: NodeKind::Architecture,
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                source: SourceSpan::new(value["source"].as_str().unwrap(), 1).unwrap(),
                modality: None,
                refinement: None,
                domain: None,
                status: match value.get("status").and_then(Value::as_str) {
                    Some("superseded") => LifecycleStatus::Superseded,
                    _ => LifecycleStatus::Active,
                },
                owner: None,
                body: value
                    .get("body")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                attributes,
            }
        })
        .collect();
    let edges = value["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            Edge::new(
                NodeId::parse(value["source"].as_str().unwrap(), None).unwrap(),
                NodeId::parse(value["target"].as_str().unwrap(), None).unwrap(),
                value["type"].as_str().unwrap(),
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

fn provenance(project: &str) -> BuildProvenance {
    BuildProvenance {
        project_commits: vec![(project.to_owned(), "0".repeat(40))],
        format_version: "0.2".to_owned(),
        ir_version: "0.1".to_owned(),
        tool_version: "0.1.0".to_owned(),
        normative_roots: Vec::new(),
    }
}

fn index(directory: &Path, name: &str, project: &str) -> PathBuf {
    let output = directory.join(format!("{name}.sqlite"));
    build_index(&graph(&fixture(name)), &output, &provenance(project)).unwrap();
    output
}

fn golden(name: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(repository().join("tests/golden").join(name)).unwrap())
        .unwrap()
}

#[test]
fn unrelated_addition_is_compatible_and_deterministic() {
    let directory = tempfile::tempdir().unwrap();
    let before_path = index(directory.path(), "before.ndf.json", "pto-spec");
    let after_path = index(directory.path(), "after-compatible.ndf.json", "pto-spec");
    let consumer_path = index(directory.path(), "consumer.ndf.json", "davincioo");
    let before = NdfIndex::open(&before_path).unwrap();
    let after = NdfIndex::open(&after_path).unwrap();
    let consumer = NdfIndex::open(&consumer_path).unwrap();

    let first = pto_release_impact(&before, &after, &consumer).unwrap();
    let second = pto_release_impact(&before, &after, &consumer).unwrap();

    assert_eq!(first.compatibility, Compatibility::Compatible);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        golden("pto-impact-compatible.json")
    );
    assert_eq!(
        serde_json::to_vec_pretty(&first).unwrap(),
        serde_json::to_vec_pretty(&second).unwrap()
    );
}

#[test]
fn release_changes_classify_breakage_review_and_transitive_consumers() {
    let directory = tempfile::tempdir().unwrap();
    let before_path = index(directory.path(), "before.ndf.json", "pto-spec");
    let after_path = index(directory.path(), "after-breaking.ndf.json", "pto-spec");
    let consumer_path = index(directory.path(), "consumer.ndf.json", "davincioo");
    let before = NdfIndex::open(&before_path).unwrap();
    let after = NdfIndex::open(&after_path).unwrap();
    let consumer = NdfIndex::open(&consumer_path).unwrap();

    let impact = pto_release_impact(&before, &after, &consumer).unwrap();

    assert_eq!(impact.compatibility, Compatibility::Breaking);
    assert!(impact.changes.iter().any(|change| {
        change.uri == "ndf://pto-spec/PTO-INST-TILE-TLOAD"
            && change.kind == PtoChangeKind::Modified
            && change.facets == ["instruction-semantics"]
    }));
    assert!(impact.changes.iter().any(|change| {
        change.uri == "ndf://pto-spec/PTO-INST-TILE-TSTORE" && change.kind == PtoChangeKind::Removed
    }));
    assert!(impact.changes.iter().any(|change| {
        change.uri == "ndf://pto-spec/PTO-INST-TILE-TMOV" && change.kind == PtoChangeKind::Moved
    }));
    assert!(impact.changes.iter().any(|change| {
        change.uri == "ndf://pto-spec/PTO-INST-TILE-TOLD"
            && change.kind == PtoChangeKind::Superseded
    }));
    assert_eq!(
        serde_json::to_value(&impact).unwrap(),
        golden("pto-impact-breaking.json")
    );
}

#[test]
fn unchanged_release_still_emits_complete_consumer_inventory() {
    let directory = tempfile::tempdir().unwrap();
    let before_path = index(directory.path(), "before.ndf.json", "pto-spec");
    let consumer_path = index(directory.path(), "consumer.ndf.json", "davincioo");
    let before = NdfIndex::open(&before_path).unwrap();
    let consumer = NdfIndex::open(&consumer_path).unwrap();

    let impact = pto_release_impact(&before, &before, &consumer).unwrap();

    assert_eq!(impact.conformance_targets.len(), 4);
    assert_eq!(impact.absence_watches.len(), 1);
    assert!(impact.changes.is_empty());
    assert!(impact.affected_consumers.is_empty());
}
