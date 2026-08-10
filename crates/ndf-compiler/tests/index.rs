use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::index::{BuildProvenance, NdfIndex, build_index};
use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan};
use rusqlite::types::ValueRef;
use serde_json::{Value, json};

const TABLES: &[&str] = &[
    "projects",
    "sources",
    "nodes",
    "node_attrs",
    "aliases",
    "edges",
    "diagnostics",
    "baselines",
    "builds",
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn node(local_id: &str, kind: NodeKind, title: &str, line: u32, body: &str) -> Node {
    Node {
        id: NodeId::new("fixture", local_id).unwrap(),
        kind,
        title: title.to_owned(),
        source: SourceSpan::new("docs/pipeline.md", line).unwrap(),
        modality: None,
        refinement: None,
        domain: None,
        status: LifecycleStatus::Active,
        owner: Some("core".to_owned()),
        body: body.to_owned(),
        attributes: BTreeMap::new(),
    }
}

fn graph(reverse: bool) -> Graph {
    let contract = node(
        "PIPE-010",
        NodeKind::Requirement,
        "Contract",
        10,
        "The core MUST issue in order.",
    );
    let mechanism = node("PIPE-020", NodeKind::Architecture, "Mechanism", 20, "");
    let edge = Edge::new(
        mechanism.id.clone(),
        contract.id.clone(),
        "refines",
        Some(mechanism.source.clone()),
    )
    .unwrap();
    Graph {
        nodes: if reverse {
            vec![contract, mechanism]
        } else {
            vec![mechanism, contract]
        },
        edges: vec![edge],
        diagnostics: Vec::new(),
    }
}

fn provenance() -> BuildProvenance {
    BuildProvenance {
        project_commits: vec![("fixture".to_owned(), "1".repeat(40))],
        format_version: "0.2".to_owned(),
        ir_version: "0.1".to_owned(),
        tool_version: "0.1.0".to_owned(),
        normative_roots: Vec::new(),
    }
}

fn logical_rows(path: &Path) -> Value {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mut tables = serde_json::Map::new();
    for table in TABLES {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table}"))
            .unwrap();
        let column_count = statement.column_count();
        let mut rows: Vec<Value> = statement
            .query_map([], |row| {
                let values = (0..column_count)
                    .map(|index| match row.get_ref(index).unwrap() {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(value) => json!(value),
                        ValueRef::Real(value) => json!(value),
                        ValueRef::Text(value) => {
                            Value::String(String::from_utf8(value.to_vec()).unwrap())
                        }
                        ValueRef::Blob(value) => Value::String(format!("blob:{}", value.len())),
                    })
                    .collect();
                Ok(Value::Array(values))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.sort_by_key(|row| serde_json::to_string(row).unwrap());
        tables.insert((*table).to_owned(), Value::Array(rows));
    }
    Value::Object(tables)
}

#[test]
fn index_hash_and_rows_match_the_frozen_contract() {
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(repository().join("tests/golden/index.json")).unwrap(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("graph.sqlite");

    let build = build_index(&graph(false), &output, &provenance()).unwrap();

    assert_eq!(build.semantic_hash, expected["semantic_hash"]);
    assert_eq!(build.node_count, expected["node_count"]);
    assert_eq!(build.edge_count, expected["edge_count"]);
    assert_eq!(logical_rows(&output), expected["rows"]);
    let index = NdfIndex::open(&output).unwrap();
    assert!(index.foreign_keys_enabled().unwrap());
}

#[test]
fn insertion_order_does_not_change_hash_or_rows() {
    let directory = tempfile::tempdir().unwrap();
    let forward = directory.path().join("forward.sqlite");
    let reverse = directory.path().join("reverse.sqlite");

    let first = build_index(&graph(false), &forward, &provenance()).unwrap();
    let second = build_index(&graph(true), &reverse, &provenance()).unwrap();

    assert_eq!(first.semantic_hash, second.semantic_hash);
    assert_eq!(logical_rows(&forward), logical_rows(&reverse));
}

#[test]
fn failed_build_preserves_existing_output_and_rejects_normative_root() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("graph.sqlite");
    fs::write(&output, b"original").unwrap();
    let mut missing_revision = provenance();
    missing_revision.project_commits.clear();

    let error = build_index(&graph(false), &output, &missing_revision).unwrap_err();
    assert!(error.to_string().contains("missing project revision"));
    assert_eq!(fs::read(&output).unwrap(), b"original");

    let docs = directory.path().join("docs");
    fs::create_dir(&docs).unwrap();
    let mut protected = provenance();
    protected.normative_roots.push(docs.clone());
    let error = build_index(&graph(false), &docs.join("graph.sqlite"), &protected).unwrap_err();
    assert!(error.to_string().contains("normative root"));
}
