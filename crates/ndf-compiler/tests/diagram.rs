use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::diagram::{
    DiagramRequest, GraphDocument, GroupBy, ROOT_SCOPE, VIEWS, build_diagram,
    document_relative_path, export_payload, failure_document, file_uri,
};
use ndf_compiler::project::{find_project_root, load_project};
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
    let loaded = load_project(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join("docs/pipeline.md"),
        group: GroupBy::None,
        workflow: None,
        view: None,
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
    let loaded = load_project(&root).unwrap();
    let error = build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join("ndf.lock"),
        group: GroupBy::None,
        workflow: None,
        view: None,
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
    let loaded = load_project(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join(format!("docs/{name}.md")),
        group: GroupBy::None,
        workflow: None,
        view: None,
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
    // A double-click drills into the document that holds it, as that
    // document: the entity name, and the crumb's instance name, are its id.
    assert_eq!(option.meta["networkInstance"], true);
    assert_eq!(option.meta["referencedEntityName"], "DOC-OPTIONS");
    assert_eq!(option.meta["instanceName"], "DOC-OPTIONS");
    assert_eq!(option.meta["definitionAnnotations"][0]["name"], "option");
    let facts = option.meta["facts"].as_array().unwrap();
    assert!(
        facts
            .iter()
            .any(|fact| fact["label"] == "Document" && fact["value"] == "DOC-OPTIONS")
    );
    assert_eq!(
        facts.last().unwrap()["label"],
        "Body",
        "the body stays last"
    );
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
    let loaded = load_project(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join(format!("docs/{name}.md")),
        group,
        workflow: None,
        view: None,
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

/// A diagram of the `tests/fixtures/diagram/project` fixture by unit name
/// and view, flat.
fn unit_document(
    file: &str,
    workflow: Option<&str>,
    view: Option<&str>,
) -> Result<GraphDocument, String> {
    let root = diagram_fixture_root();
    let loaded = load_project(&root).unwrap();
    build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join(file),
        group: GroupBy::None,
        workflow,
        view,
    })
    .map_err(|error| error.to_string())
}

fn golden(name: &str) -> Value {
    serde_json::from_str(
        &fs::read_to_string(repository().join(format!("tests/golden/diagram-{name}.json")))
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn every_document_carries_the_project_outline() {
    let root = diagram_fixture_root();
    for name in ["forwarding", "options"] {
        let document = fixture_document(name);
        let outline = document
            .hierarchy
            .as_ref()
            .expect("outline on every document");
        assert!(outline.path.is_empty());
        assert_eq!(outline.workflow_name, "diagram");
        assert_eq!(
            outline.source_uri,
            file_uri(&root.join("ndf.yaml").to_string_lossy())
        );
        assert_eq!(outline.node_count, Some(2));
        assert!(!outline.truncated);
        // The documents' folder, a heading transparent to paths.
        assert_eq!(outline.children.len(), 1);
        let folder = &outline.children[0];
        assert_eq!(folder.role, Some("group"));
        assert_eq!(folder.label.as_deref(), Some("docs"));
        let documents: Vec<(Vec<String>, String, Option<usize>)> = folder
            .children
            .iter()
            .map(|child| {
                (
                    child.path.clone(),
                    child.workflow_name.clone(),
                    child.node_count,
                )
            })
            .collect();
        assert_eq!(
            documents,
            [
                (
                    vec!["DOC-FORWARDING".to_owned()],
                    "DOC-FORWARDING".to_owned(),
                    Some(5)
                ),
                (
                    vec!["DOC-OPTIONS".to_owned()],
                    "DOC-OPTIONS".to_owned(),
                    Some(2)
                )
            ]
        );
        assert!(folder.children[1].source_uri.ends_with("/docs/options.md"));
        // Each document's clauses, in document order, as rows that name their node.
        let options = &folder.children[1];
        assert_eq!(options.children.len(), 2);
        assert!(
            options
                .children
                .iter()
                .all(|clause| clause.role == Some("node")
                    && clause.path.len() == 2
                    && clause.path[0] == "DOC-OPTIONS"
                    && clause.node_id.as_deref()
                        == Some(format!("node:{}:{}", ROOT_SCOPE, clause.path[1]).as_str())
                    && clause.children.is_empty())
        );
    }
    // Every unit lists the same views, on the root and on each clause.
    let document = fixture_document("forwarding");
    let views: Vec<&str> = document.graph.meta["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|view| view["id"].as_str().unwrap())
        .collect();
    assert_eq!(views, VIEWS.iter().map(|(id, _)| *id).collect::<Vec<_>>());
    assert!(
        document
            .graph
            .nodes
            .iter()
            .filter(|node| node.kind != "blackbox")
            .all(|node| node.meta["views"] == document.graph.meta["views"])
    );
}

#[test]
fn the_project_overview_draws_one_node_per_document_and_adds_up_the_edges() {
    let root = diagram_fixture_root();
    let overview = unit_document("ndf.yaml", None, None).unwrap();
    assert_eq!(
        portable(serde_json::to_value(&overview).unwrap(), &root),
        golden("project")
    );
    // Naming the project from any file of it is the same overview.
    let by_name = unit_document("docs/options.md", Some("diagram"), None).unwrap();
    assert_eq!(by_name.graph, overview.graph);

    assert_eq!(overview.graph.id, "wf:diagram");
    assert_eq!(overview.graph.meta["workflow"], "diagram");
    let documents: Vec<(&str, &str, Option<&str>)> = overview
        .graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.as_str(),
                node.label.as_str(),
                node.footer.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        documents,
        [
            (
                "node:scope:root:DOC-FORWARDING",
                "DOC-FORWARDING",
                Some("Forwarding")
            ),
            (
                "node:scope:root:DOC-OPTIONS",
                "DOC-OPTIONS",
                Some("Design options")
            )
        ]
    );
    for node in &overview.graph.nodes {
        assert_eq!(node.kind, "document");
        assert_eq!(node.meta["networkInstance"], true);
        assert_eq!(node.meta["referencedEntityName"], node.label);
        assert_eq!(node.meta["instanceName"], node.label);
        assert_eq!(node.meta["definitionAnnotations"][0]["name"], "document");
    }
    let forwarding = &overview.graph.nodes[0];
    assert_eq!(forwarding.meta["badges"][0]["text"], "5 clauses");
    assert!(
        forwarding.meta["referencedSource"]["file"]
            .as_str()
            .unwrap()
            .ends_with("docs/forwarding.md")
    );
    // The one dangling edge of forwarding.md reddens its document node.
    assert_eq!(forwarding.meta["isErrored"], true);
    // One edge between the two documents: the depends-on from FWD-ADM-001 to
    // PRM-BUF-001. A dangling in-project target is not drawn here.
    let edges: Vec<(&str, &str, &str)> = overview
        .graph
        .edges
        .iter()
        .map(|edge| {
            (
                edge.from.as_str(),
                edge.to.as_str(),
                edge.meta["label"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        edges,
        [(
            "port:node:scope:root:DOC-FORWARDING:out",
            "port:node:scope:root:DOC-OPTIONS:in",
            "depends-on"
        )]
    );
    assert_eq!(overview.graph.edges[0].meta["count"], 1);
    // Every diagnostic of the project, for the Problems panel.
    assert_eq!(overview.diagnostics.len(), 1);
}

#[test]
fn a_trace_follows_one_edge_type_across_documents() {
    let root = diagram_fixture_root();
    let trace = unit_document(
        "docs/forwarding.md",
        Some("DOC-FORWARDING"),
        Some("depends-on"),
    )
    .unwrap();
    assert_eq!(
        portable(serde_json::to_value(&trace).unwrap(), &root),
        golden("forwarding-depends-on-trace")
    );
    assert_eq!(trace.graph.id, "wf:DOC-FORWARDING");
    assert_eq!(trace.graph.meta["view"], "depends-on");
    assert_eq!(trace.graph.meta["viewLabel"], "Dependencies");
    assert_eq!(trace.graph.meta["title"], "Dependencies: Forwarding");
    // Every clause of the document is a seed, the targets are reached: the
    // option in the other document (navigable, not external) and the dangling
    // MISSING-001 (a blackbox).
    let ids: Vec<&str> = trace
        .graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "node:scope:root:FWD-ADM-000",
            "node:scope:root:FWD-ADM-001",
            "node:scope:root:FWD-ADM-002",
            "node:scope:root:Q-001",
            "node:scope:root:VER-ADM-001",
            "node:scope:root:PRM-BUF-001",
            "node:scope:root:MISSING-001"
        ]
    );
    let option = trace
        .graph
        .nodes
        .iter()
        .find(|node| node.id.ends_with("PRM-BUF-001"))
        .unwrap();
    assert_eq!(option.kind, "option");
    assert!(option.meta.get("external").is_none());
    assert_eq!(option.meta["networkInstance"], true);
    assert_eq!(option.meta["referencedEntityName"], "DOC-OPTIONS");
    let missing = trace
        .graph
        .nodes
        .iter()
        .find(|node| node.id.ends_with("MISSING-001"))
        .unwrap();
    assert_eq!(missing.kind, "blackbox");
    // Only edges of the traced type.
    let roles: Vec<&str> = trace
        .graph
        .edges
        .iter()
        .map(|edge| edge.meta["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["depends-on", "depends-on"]);

    // From one clause, both ways: what it refines and what refines it.
    let from_clause =
        unit_document("docs/forwarding.md", Some("FWD-ADM-000"), Some("refines")).unwrap();
    assert_eq!(from_clause.graph.id, "wf:FWD-ADM-000");
    assert_eq!(
        from_clause.graph.meta["title"],
        "Refinement trace: Forwarding intent"
    );
    assert_eq!(
        from_clause.graph.meta["ndf"]["id"],
        "ndf://diagram/FWD-ADM-000"
    );
    let ids: Vec<&str> = from_clause
        .graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["node:scope:root:FWD-ADM-000", "node:scope:root:FWD-ADM-001"]
    );
    assert_eq!(from_clause.graph.edges.len(), 1);
    // Every clause in the trace is in the shown document: nothing to navigate.
    assert!(
        from_clause
            .graph
            .nodes
            .iter()
            .all(|node| node.meta.get("networkInstance").is_none())
    );
}

#[test]
fn a_unit_is_named_by_document_id_title_or_clause_id() {
    let by_id = unit_document("docs/forwarding.md", Some("DOC-OPTIONS"), None).unwrap();
    let by_title = unit_document("docs/forwarding.md", Some("Design options"), None).unwrap();
    let by_file = fixture_document("options");
    assert_eq!(by_id.graph, by_file.graph);
    assert_eq!(by_title.graph, by_file.graph);
    // A clause names the document that holds it when no view is asked for.
    let by_clause = unit_document("docs/forwarding.md", Some("PRM-BUF-001"), None).unwrap();
    assert_eq!(by_clause.graph, by_file.graph);
    let error = unit_document("docs/forwarding.md", Some("NOPE"), None).unwrap_err();
    assert!(
        error.contains("no document or clause named NOPE"),
        "{error}"
    );
}

#[test]
fn an_asl_unit_reads_as_its_file_its_instruction_and_contract_and_is_not_edited() {
    let directory = tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    fs::copy(
        repository().join("tests/fixtures/pto-asl-project/ndf.yaml"),
        root.join("ndf.yaml"),
    )
    .unwrap();
    fs::write(
        root.join("X.LD.asl"),
        concat!(
            "// NDF-BEGIN: PTO-X-LD-001\n",
            "// ndf: kind=contract level=L1 layer=architecture status=accepted\n",
            "// X.LD MUST load one aligned word. It never writes back.\n",
            "// NDF-END: PTO-X-LD-001\n",
            r#"// PTO-INSTRUCTION: {"id":"PTO-SCALAR-X-LD","surface":"scalar","classification":["agu"],"depends_on":[],"mnemonic":"X.LD","summary":"X.LD loads a word.","catalog_records":[{"asm":"x.ld Dst","encoding_kind":"L32","length_bits":32,"fields":[{"name":"Dst","pieces":[{"instruction_lsb":7,"width":5}]}]}],"contract":{"legality":["The address is aligned."]}}"#,
            "\n",
            "pure func XLoadSize() => integer\nbegin\n    return 4;\nend;\n",
        ),
    )
    .unwrap();
    let loaded = load_project(&root).unwrap();
    let document = build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join("X.LD.asl"),
        group: GroupBy::None,
        workflow: None,
        view: None,
    })
    .unwrap();
    let node = |local: &str| {
        document
            .graph
            .nodes
            .iter()
            .find(|node| node.id.ends_with(&format!(":{local}")))
            .unwrap()
    };
    let fact = |local: &str, label: &str| {
        node(local).meta["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["label"] == label)
            .map(|fact| fact["value"].as_str().unwrap().to_owned())
    };
    // Named the way a reader looks for them.
    assert_eq!(node("PTO-SCALAR-X-LD").label, "X.LD.asl");
    assert_eq!(node("PTO-INST-SCALAR-X-LD").label, "X.LD");
    assert_eq!(
        node("PTO-X-LD-001").label,
        "X.LD MUST load one aligned word"
    );
    // The unit stands for its file, and lists what it declares.
    assert_eq!(
        node("PTO-SCALAR-X-LD").meta["open"]["file"],
        root.join("X.LD.asl").to_string_lossy().as_ref()
    );
    assert!(
        fact("PTO-SCALAR-X-LD", "Functions")
            .unwrap()
            .contains("[`XLoadSize`](")
    );
    // The instruction's contract, from its metadata line.
    let contract = fact("PTO-INST-SCALAR-X-LD", "Contract").unwrap();
    assert!(contract.starts_with("X.LD loads a word."), "{contract}");
    assert!(contract.contains("- `Dst` [11:7]"), "{contract}");
    assert!(contract.contains("**Legality**"), "{contract}");
    // The clause is drawn defined in the unit, though its header comes after.
    let roles: BTreeSet<(String, &str)> = document
        .graph
        .edges
        .iter()
        .map(|edge| (edge.id.clone(), edge.meta["role"].as_str().unwrap()))
        .collect();
    assert!(roles.iter().any(|(id, role)| *role == "defined-in"
        && id.contains("PTO-X-LD-001:out")
        && id.contains("PTO-SCALAR-X-LD:in")));
    assert!(
        fact("PTO-X-LD-001", "Trace")
            .unwrap()
            .contains("defined-in → [PTO-SCALAR-X-LD]")
    );
    // An adapter's clauses are not edited in the diagram: their fields are
    // facts, with no parameters to change.
    for drawn in &document.graph.nodes {
        assert!(drawn.meta.get("parameters").is_none(), "{}", drawn.id);
    }
    assert_eq!(fact("PTO-X-LD-001", "Status").as_deref(), Some("active"));
}

#[test]
fn an_asl_file_is_a_document_named_by_its_stem_and_needs_no_lock() {
    let root = repository()
        .join("tests/fixtures/pto-asl-project")
        .canonicalize()
        .unwrap();
    assert!(
        !root.join("ndf.lock").exists(),
        "the fixture has no lock on purpose"
    );
    let loaded = load_project(&root).unwrap();
    assert!(loaded.dependencies.is_empty());
    let document = build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file: &root.join("owner.asl"),
        group: GroupBy::None,
        workflow: None,
        view: None,
    })
    .unwrap();
    assert_eq!(
        portable(serde_json::to_value(&document).unwrap(), &root),
        golden("asl-owner")
    );
    assert_eq!(document.graph.id, "wf:owner");
    assert_eq!(document.graph.meta["workflow"], "owner");
    let kinds: Vec<(&str, &str)> = document
        .graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("node:scope:root:PTO-ARCH-OWNER", "source"),
            ("node:scope:root:PTO-STATE-ARCH-OWNER", "architecture"),
            ("node:scope:root:PTO-ARCH-DEPENDENCY", "source"),
            ("node:scope:root:PTO-STATE-ARCH-DEPENDENCY", "architecture")
        ]
    );
    let roles: Vec<&str> = document
        .graph
        .edges
        .iter()
        .map(|edge| edge.meta["role"].as_str().unwrap())
        .collect();
    assert_eq!(
        roles,
        ["depends-on", "owns-state", "depends-on", "defined-in"]
    );
    // The unit in the other file drills into that file, by its stem.
    let dependency = document
        .graph
        .nodes
        .iter()
        .find(|node| node.id.ends_with(":PTO-ARCH-DEPENDENCY"))
        .unwrap();
    assert_eq!(dependency.meta["external"], true);
    assert_eq!(dependency.meta["referencedEntityName"], "dependency");
    let outline = document.hierarchy.unwrap();
    assert_eq!(outline.workflow_name, "pto-spec");
    let names: Vec<&str> = outline
        .children
        .iter()
        .map(|child| child.workflow_name.as_str())
        .collect();
    assert_eq!(names, ["cycle-a", "cycle-b", "dependency", "owner"]);
    // A project with dependencies is still held to its lock.
    let error = load_project(&repository().join("tests/fixtures/diagram/project")).map(|_| ());
    assert!(error.is_ok());
}

#[test]
fn a_file_uri_percent_encodes_what_a_consumer_would() {
    assert_eq!(
        file_uri("/home/e/spec/docs/pipeline.md"),
        "file:///home/e/spec/docs/pipeline.md"
    );
    assert_eq!(
        file_uri("/home/e/my spec/d%c.md"),
        "file:///home/e/my%20spec/d%25c.md"
    );
    assert_eq!(
        file_uri("/tmp/ünïcode.md"),
        "file:///tmp/%C3%BCn%C3%AFcode.md"
    );
}
