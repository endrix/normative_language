use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::diagram::{
    DiagramRequest, GroupBy, build_diagram, document_relative_path, export_payload,
    failure_document,
};
use ndf_compiler::project::{find_project_root, load_project_graph};
use serde_json::Value;
use tempfile::tempdir;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_root() -> PathBuf {
    repository()
        .join("tests/fixtures/markdown/project")
        .canonicalize()
        .unwrap()
}

/// Absolute paths in the document replaced by `<root>`, so the golden file is
/// the same on every machine.
fn portable(value: Value, root: &Path) -> Value {
    let root = root.to_string_lossy().into_owned();
    match value {
        Value::String(text) => Value::String(text.replace(&root, "<root>")),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|v| portable(v, root.as_ref()))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, v)| (key, portable(v, root.as_ref())))
                .collect(),
        ),
        other => other,
    }
}

fn pipeline_document() -> ndf_compiler::diagram::GraphDocument {
    let root = fixture_root();
    let (manifest, graph) = load_project_graph(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &manifest,
        graph: &graph,
        file: &root.join("docs/pipeline.md"),
        group: GroupBy::None,
    })
    .unwrap()
}

#[test]
fn the_pipeline_document_matches_the_frozen_diagram() {
    let root = fixture_root();
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(repository().join("tests/golden/diagram-pipeline.json")).unwrap(),
    )
    .unwrap();

    let actual = portable(serde_json::to_value(pipeline_document()).unwrap(), &root);

    assert_eq!(actual, expected);
}

#[test]
fn every_edge_names_a_port_of_a_node_in_the_document() {
    let document = pipeline_document();
    let ports: Vec<&str> = document
        .graph
        .nodes
        .iter()
        .flat_map(|node| node.ports.iter().map(|port| port.id.as_str()))
        .collect();
    assert!(!document.graph.edges.is_empty());
    for edge in &document.graph.edges {
        assert!(ports.contains(&edge.from.as_str()), "{}", edge.from);
        assert!(ports.contains(&edge.to.as_str()), "{}", edge.to);
        assert_eq!(edge.meta["role"], edge.meta["label"]);
    }
    let ids: Vec<&str> = document
        .graph
        .edges
        .iter()
        .map(|edge| edge.id.as_str())
        .collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "edge ids are unique");
}

#[test]
fn an_unresolved_external_target_is_a_blackbox_and_its_clause_is_errored() {
    let document = pipeline_document();
    let blackbox = document
        .graph
        .nodes
        .iter()
        .find(|node| node.id == "node:scope:root:pto-spec.PTO-INST-TILE-TLOAD")
        .expect("external target drawn once");
    assert_eq!(blackbox.kind, "blackbox");
    assert_eq!(blackbox.meta["external"], true);
    assert_eq!(blackbox.label, "pto-spec/PTO-INST-TILE-TLOAD");

    let clause = document
        .graph
        .nodes
        .iter()
        .find(|node| node.id == "node:scope:root:PIPE-PTO-001")
        .unwrap();
    assert_eq!(clause.meta["isErrored"], true);
    assert!(
        clause.meta["errorMessage"]
            .as_str()
            .unwrap()
            .starts_with("NDF-XREF-003:")
    );
    assert_eq!(
        clause.meta["definitionAnnotations"][0]["name"],
        "requirement"
    );
    assert_eq!(clause.footer.as_deref(), Some("PIPE-PTO-001"));
    assert_eq!(
        clause.meta["badges"],
        serde_json::json!([{ "text": "MUST", "tone": "info" }, { "text": "L1", "tone": "neutral" }])
    );
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.code == "NDF-XREF-003")
    );
    assert_eq!(document.diagnostics[0].start_line, 24);
}

#[test]
fn a_file_outside_the_project_is_refused() {
    let root = fixture_root();
    let elsewhere = tempdir().unwrap();
    let file = elsewhere.path().join("other.md");
    fs::write(&file, "# Other\n").unwrap();
    let error = document_relative_path(&root, &file).unwrap_err();
    assert!(error.to_string().contains("outside the NDF project"));
    assert!(find_project_root(&file).is_none());
    assert_eq!(
        find_project_root(&root.join("docs/pipeline.md")).unwrap(),
        root
    );
}

#[test]
fn a_file_that_is_not_a_document_is_refused() {
    let root = fixture_root();
    let (manifest, graph) = load_project_graph(&root).unwrap();
    let error = build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &manifest,
        graph: &graph,
        file: &root.join("ndf.yaml"),
        group: GroupBy::None,
    })
    .unwrap_err();
    assert!(error.to_string().contains("not an NDF document"));
}

#[test]
fn a_failure_document_is_partial_and_carries_the_reason() {
    let document = failure_document(Path::new("spec/missing.md"), "NDF-DEP-004: dirty");
    assert_eq!(document.partial, Some(true));
    assert_eq!(document.errors[0].message, "NDF-DEP-004: dirty");
    assert_eq!(document.graph.id, "wf:missing");
    assert!(document.graph.nodes.is_empty());
}

#[test]
fn the_export_payload_is_the_pre_normalised_shape() {
    let payload = export_payload(&pipeline_document());
    assert_eq!(payload["network"], "DOC-FIXTURE-PIPELINE");
    let node = &payload["nodes"][0];
    assert!(node.get("metadata").is_some());
    assert!(node.get("meta").is_none());
    let edge = &payload["edges"][0];
    assert!(
        edge["from"]["portId"]
            .as_str()
            .unwrap()
            .starts_with("port:node:scope:root:")
    );
    assert!(
        edge["to"]["portId"]
            .as_str()
            .unwrap()
            .starts_with("port:node:scope:root:")
    );
    assert_eq!(edge["metadata"]["role"], "refines");
}

fn diagram_fixture_root() -> PathBuf {
    repository()
        .join("tests/fixtures/diagram/project")
        .canonicalize()
        .unwrap()
}

fn fixture_document(name: &str) -> ndf_compiler::diagram::GraphDocument {
    let root = diagram_fixture_root();
    let (manifest, graph) = load_project_graph(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &manifest,
        graph: &graph,
        file: &root.join(format!("docs/{name}.md")),
        group: GroupBy::None,
    })
    .unwrap()
}

#[test]
fn the_diagram_fixture_documents_match_their_frozen_diagrams() {
    let root = diagram_fixture_root();
    for name in ["forwarding", "options"] {
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(repository().join(format!("tests/golden/diagram-{name}.json")))
                .unwrap(),
        )
        .unwrap();
        let actual = portable(serde_json::to_value(fixture_document(name)).unwrap(), &root);
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn a_clause_of_another_document_is_an_external_node_of_its_own_kind() {
    let document = fixture_document("forwarding");
    let option = document
        .graph
        .nodes
        .iter()
        .find(|node| node.id == "node:scope:root:PRM-BUF-001")
        .expect("the depends-on target from docs/options.md is drawn");
    assert_eq!(option.kind, "option");
    assert_eq!(option.meta["external"], true);
    assert_eq!(option.label, "Admission buffer depth");
    assert!(
        option.meta["referencedSource"]["file"]
            .as_str()
            .unwrap()
            .ends_with("docs/options.md")
    );
    assert_eq!(option.meta["referencedSource"]["line"], 10);
    assert_eq!(option.meta["referencedEntityName"], "PRM-BUF-001");
    assert_eq!(option.meta["definitionAnnotations"][0]["name"], "option");
}

#[test]
fn a_conflict_is_undirected_and_a_dangling_mandatory_edge_reddens_its_clause() {
    let document = fixture_document("forwarding");
    let conflict = document
        .graph
        .edges
        .iter()
        .find(|edge| edge.meta["role"] == "conflicts-with")
        .unwrap();
    assert_eq!(conflict.meta["undirected"], true);
    assert!(
        document
            .graph
            .edges
            .iter()
            .filter(|edge| edge.meta["role"] != "conflicts-with")
            .all(|edge| edge.meta.get("undirected").is_none())
    );

    let clause = document
        .graph
        .nodes
        .iter()
        .find(|node| node.id == "node:scope:root:FWD-ADM-002")
        .unwrap();
    assert_eq!(clause.meta["isErrored"], true);
    assert!(
        clause.meta["errorMessage"]
            .as_str()
            .unwrap()
            .starts_with("NDF-EDGE-001:")
    );
    let missing = document
        .graph
        .nodes
        .iter()
        .find(|node| node.id == "node:scope:root:MISSING-001")
        .expect("the dangling target is drawn as a blackbox");
    assert_eq!(missing.kind, "blackbox");
    assert_eq!(missing.label, "MISSING-001");
    assert_eq!(document.diagnostics.len(), 1);
    assert_eq!(document.diagnostics[0].code, "NDF-EDGE-001");
    assert_eq!(document.diagnostics[0].start_line, 22);
}

#[test]
fn the_options_document_draws_its_coupling_and_the_clause_that_depends_on_it() {
    let document = fixture_document("options");
    let ids: Vec<&str> = document
        .graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "node:scope:root:PRM-BUF-001",
            "node:scope:root:PRM-THR-001",
            "node:scope:root:FWD-ADM-001",
        ]
    );
    let roles: Vec<&str> = document
        .graph
        .edges
        .iter()
        .map(|edge| edge.meta["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["depends-on", "couples-with"]);
    assert!(document.diagnostics.is_empty());
}

fn grouped_document(name: &str, group: GroupBy) -> ndf_compiler::diagram::GraphDocument {
    let root = diagram_fixture_root();
    let (manifest, graph) = load_project_graph(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &manifest,
        graph: &graph,
        file: &root.join(format!("docs/{name}.md")),
        group,
    })
    .unwrap()
}

#[test]
fn grouping_by_refinement_puts_each_clause_in_its_layer_and_leaves_foreign_nodes_at_the_root() {
    let root = diagram_fixture_root();
    let document = grouped_document("forwarding", GroupBy::Refinement);
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(
            repository().join("tests/golden/diagram-forwarding-by-refinement.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        portable(serde_json::to_value(&document).unwrap(), &root),
        expected
    );

    let scopes: Vec<(&str, &str, usize)> = document
        .graph
        .subgraphs
        .iter()
        .map(|scope| (scope.id.as_str(), scope.label.as_str(), scope.nodes.len()))
        .collect();
    assert_eq!(
        scopes,
        [
            ("scope:group:L0", "L0", 1),
            ("scope:group:L1", "L1", 2),
            ("scope:group:L3", "L3", 1)
        ]
    );
    assert!(
        document
            .graph
            .subgraphs
            .iter()
            .all(|scope| scope.kind == "group")
    );
    let scope_of = |id: &str| {
        document
            .graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.scope.clone())
            .unwrap()
    };
    assert_eq!(scope_of("node:scope:root:FWD-ADM-001"), "scope:group:L1");
    assert_eq!(
        scope_of("node:scope:root:Q-001"),
        "scope:root",
        "no layer, no group"
    );
    assert_eq!(
        scope_of("node:scope:root:PRM-BUF-001"),
        "scope:root",
        "foreign clauses stay at the root"
    );
    assert_eq!(scope_of("node:scope:root:MISSING-001"), "scope:root");
}

#[test]
fn grouping_by_domain_and_not_at_all() {
    let by_domain = grouped_document("forwarding", GroupBy::Domain);
    assert_eq!(by_domain.graph.subgraphs.len(), 1);
    assert_eq!(by_domain.graph.subgraphs[0].label, "forwarding");
    assert_eq!(by_domain.graph.subgraphs[0].nodes.len(), 5);

    let flat = grouped_document("forwarding", GroupBy::None);
    assert!(flat.graph.subgraphs.is_empty());
    assert!(
        flat.graph
            .nodes
            .iter()
            .all(|node| node.scope == "scope:root")
    );
    assert_eq!(GroupBy::parse("refinement"), Some(GroupBy::Refinement));
    assert_eq!(GroupBy::parse("loud"), None);
}

#[test]
fn badges_say_the_modality_the_layer_and_a_status_that_is_not_active() {
    let document = grouped_document("forwarding", GroupBy::None);
    let badges = |id: &str| {
        document
            .graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.meta["badges"].clone())
            .unwrap()
    };
    assert_eq!(
        badges("node:scope:root:FWD-ADM-002"),
        serde_json::json!([
            { "text": "SHOULD", "tone": "neutral" },
            { "text": "L1", "tone": "neutral" },
            { "text": "draft", "tone": "neutral" }
        ])
    );
    assert_eq!(
        badges("node:scope:root:Q-001"),
        serde_json::json!([{ "text": "open", "tone": "warning" }])
    );
    assert_eq!(
        badges("node:scope:root:FWD-ADM-000"),
        serde_json::json!([{ "text": "L0", "tone": "neutral" }]),
        "an active clause carries no status badge"
    );
    let options = grouped_document("options", GroupBy::None);
    assert_eq!(
        options.graph.nodes[0].meta["badges"][0],
        serde_json::json!({ "text": "TBD", "tone": "warning" })
    );
}
