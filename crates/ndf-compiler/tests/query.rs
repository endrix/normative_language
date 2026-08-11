use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::index::{BuildProvenance, NdfIndex, build_index};
use ndf_compiler::manifest::ProjectManifest;
use ndf_compiler::markdown::parse_markdown;
use ndf_compiler::query::{query_nodes, trace};
use ndf_compiler::report::{CoveragePolicy, coverage};
use ndf_core::identity::NodeId;
use ndf_core::model::{Graph, LifecycleStatus, Node, NodeKind, SourceSpan};
use serde_json::{Value, json};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_index(output: &Path) {
    let root = repository().join("tests/fixtures/markdown/project");
    let manifest = ProjectManifest::load(&root.join("ndf.yaml")).unwrap();
    let parsed = parse_markdown(&root.join("docs/pipeline.md"), &manifest).unwrap();
    let graph = Graph {
        nodes: parsed.nodes,
        edges: parsed.edges,
        diagnostics: parsed.diagnostics,
    };
    build_index(
        &graph,
        output,
        &BuildProvenance {
            project_commits: vec![("fixture".to_owned(), "1".repeat(40))],
            format_version: "0.2".to_owned(),
            ir_version: "0.1".to_owned(),
            tool_version: "0.1.0".to_owned(),
            normative_roots: Vec::new(),
        },
    )
    .unwrap();
}

#[test]
fn query_trace_and_coverage_match_the_frozen_oracle() {
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(repository().join("tests/golden/query-report.json")).unwrap(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("graph.sqlite");
    fixture_index(&output);
    let index = NdfIndex::open(&output).unwrap();

    let actual = json!({
        "query": query_nodes(&index, "kind=requirement and status=active").unwrap(),
        "trace": trace(&index, "PIPE-ISSUE-010", &["refines"]).unwrap(),
        "coverage": coverage(&index, &CoveragePolicy::default()).unwrap(),
    });

    assert_eq!(actual, expected);
}

#[test]
fn query_rejects_invalid_clauses_and_trace_reports_cycles() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("graph.sqlite");
    fixture_index(&output);
    let index = NdfIndex::open(&output).unwrap();

    assert_eq!(
        query_nodes(&index, "title=Issue contract")
            .unwrap_err()
            .to_string(),
        "unsupported query field: title"
    );
    assert_eq!(
        query_nodes(&index, "kind").unwrap_err().to_string(),
        "invalid query clause: kind"
    );

    // Insert a cycle through the database to isolate trace traversal behavior.
    let connection = rusqlite::Connection::open(&output).unwrap();
    connection
        .execute(
            "INSERT INTO edges(edge_id, source_uri, edge_type, target_uri, attributes_json) VALUES (99, ?, 'refines', ?, '{}')",
            [
                "ndf://fixture/PIPE-ISSUE-001",
                "ndf://fixture/PIPE-ISSUE-010",
            ],
        )
        .unwrap();
    drop(connection);

    let cycle = trace(&index, "PIPE-ISSUE-010", &["refines"]).unwrap();
    assert_eq!(cycle.cycles.len(), 1);
    assert_eq!(cycle.cycles[0].target, "ndf://fixture/PIPE-ISSUE-010");
}

#[test]
fn query_filters_decoded_node_attributes() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("state.sqlite");
    let nodes = ["state", "unit"]
        .into_iter()
        .enumerate()
        .map(|(index, entity)| Node {
            id: NodeId::new("pto-spec", format!("PTO-QUERY-{entity}")).unwrap(),
            kind: if entity == "state" {
                NodeKind::Architecture
            } else {
                NodeKind::Source
            },
            title: entity.to_owned(),
            source: SourceSpan::new("query.asl", (index + 1) as u32).unwrap(),
            modality: None,
            refinement: None,
            domain: None,
            status: LifecycleStatus::Active,
            owner: Some("pto-spec".to_owned()),
            body: String::new(),
            attributes: BTreeMap::from([("pto_entity".to_owned(), entity.to_owned())]),
        })
        .collect();
    build_index(
        &Graph {
            nodes,
            ..Graph::default()
        },
        &output,
        &BuildProvenance {
            project_commits: vec![("pto-spec".to_owned(), "1".repeat(40))],
            format_version: "0.2".to_owned(),
            ir_version: "0.1".to_owned(),
            tool_version: "0.1.0".to_owned(),
            normative_roots: Vec::new(),
        },
    )
    .unwrap();
    let index = NdfIndex::open(&output).unwrap();

    let result = query_nodes(&index, "kind=architecture and attributes.pto_entity=state").unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].attributes["pto_entity"], "state");
}
