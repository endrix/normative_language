use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::adapters::parse_project;
use ndf_compiler::adapters::pto_asl::{instruction_clause_id, parse_pto_asl, parse_pto_asl_text};
use ndf_compiler::manifest::ProjectManifest;
use ndf_core::model::{Graph, LifecycleStatus, NodeKind};
use serde_json::Value;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn pto_asl_matches_the_frozen_oracle() {
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test(
        "pto-spec",
        &root,
        ["PTO"],
        [
            "architecture",
            "scalar",
            "block",
            "tile",
            "state",
            "memory",
            "concurrency",
        ],
    );
    let expected: BTreeMap<String, Value> = serde_json::from_str(
        &fs::read_to_string(repository().join("tests/golden/pto-asl.json")).unwrap(),
    )
    .unwrap();
    let actual: BTreeMap<String, Value> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "asl"))
        .map(|path| {
            let parsed = parse_pto_asl(&path, &project).unwrap();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                serde_json::to_value(Graph {
                    nodes: parsed.nodes,
                    edges: parsed.edges,
                    diagnostics: parsed.diagnostics,
                })
                .unwrap(),
            )
        })
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn instruction_identity_and_clause_contracts_are_preserved() {
    assert_eq!(
        instruction_clause_id("block", "BSTART.TLOAD").unwrap(),
        "PTO-INST-BLOCK-BSTART-TLOAD"
    );
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test("pto-spec", &root, ["PTO"], ["tile"]);
    let parsed = parse_pto_asl(&root.join("accept.asl"), &project).unwrap();
    let unit = parsed
        .nodes
        .iter()
        .find(|node| node.id.local_id() == "PTO-TILE-TLOAD")
        .unwrap();
    assert_eq!(unit.kind, NodeKind::Source);
    assert_eq!(unit.attributes["pto_entity"], "unit");
    assert!(parsed.edges.iter().any(|edge| {
        edge.edge_type == "defined-in"
            && edge.source.local_id() == "PTO-INST-TILE-TLOAD"
            && edge.target.local_id() == "PTO-TILE-TLOAD"
    }));
    let clause = parsed
        .nodes
        .iter()
        .find(|node| node.id.local_id() == "PTO-TILE-CAPACITY")
        .unwrap();
    assert_eq!(clause.kind, NodeKind::Requirement);
    assert_eq!(clause.refinement.as_deref(), Some("L1"));
    assert_eq!(clause.status, LifecycleStatus::Active);
    assert!(
        parsed
            .edges
            .iter()
            .any(|edge| edge.edge_type == "references")
    );
}

#[test]
fn instruction_identity_ignores_source_path() {
    let directory = tempfile::tempdir().unwrap();
    let project = ProjectManifest::for_test("pto-spec", directory.path(), ["PTO"], ["tile"]);
    let text = concat!(
        "// PTO-INSTRUCTION: ",
        r#"{"id":"PTO-TILE-TLOAD","surface":"tile","classification":["memory-and-data-movement","regular"],"depends_on":[],"mnemonic":"TLOAD","catalog_records":[{"semantic_handler":"TLOAD","contract_status":"reviewed-complete"}]}"#,
        "\n",
    );

    let original =
        parse_pto_asl_text(text, &directory.path().join("original/TLOAD.asl"), &project).unwrap();
    let moved =
        parse_pto_asl_text(text, &directory.path().join("moved/TLOAD.asl"), &project).unwrap();

    assert_eq!(original.nodes[0].id, moved.nodes[0].id);
    assert_eq!(original.nodes[0].attributes, moved.nodes[0].attributes);
    assert_ne!(original.nodes[0].source.path, moved.nodes[0].source.path);
}

#[test]
fn instruction_facets_are_indexed() {
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test("pto-spec", &root, ["PTO"], ["tile"]);
    let parsed = parse_pto_asl(&root.join("accept.asl"), &project).unwrap();
    let instruction = parsed
        .nodes
        .iter()
        .find(|node| node.id.local_id() == "PTO-INST-TILE-TLOAD")
        .unwrap();

    assert_eq!(instruction.attributes["surface"], "tile");
    assert_eq!(instruction.attributes["mnemonic"], "TLOAD");
    assert_eq!(instruction.attributes["semantic_handler"], "TLOAD");
    assert_eq!(instruction.attributes["status"], "reviewed-complete");
    assert_eq!(
        instruction.attributes["semantic_facets"],
        "architectural-state,instruction-semantics"
    );
}

#[test]
fn colliding_instruction_identities_report_asl_005() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("first.asl"),
        "// PTO-INSTRUCTION: {\"id\":\"PTO-TILE-TLOAD-DOT\",\"surface\":\"tile\",\"classification\":[\"test\"],\"depends_on\":[],\"mnemonic\":\"T.LOAD\"}\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("second.asl"),
        "// PTO-INSTRUCTION: {\"id\":\"PTO-TILE-TLOAD-DASH\",\"surface\":\"tile\",\"classification\":[\"test\"],\"depends_on\":[],\"mnemonic\":\"T-LOAD\"}\n",
    )
    .unwrap();
    let mut project = ProjectManifest::for_test("pto-spec", directory.path(), ["PTO"], ["tile"]);
    project.roots = vec!["*.asl".to_owned()];

    let graph = parse_project(&project).unwrap();

    assert_eq!(graph.diagnostics.len(), 1);
    assert_eq!(graph.diagnostics[0].code, "NDF-ASL-005");
    assert_eq!(graph.diagnostics[0].related.len(), 1);
}

#[test]
fn invalid_and_duplicate_unit_metadata_report_asl_006() {
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test("pto-spec", &root, ["PTO"], ["architecture"]);
    let parsed = parse_pto_asl(&root.join("reject-unit.asl"), &project).unwrap();

    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>(),
        vec!["NDF-ASL-006"; 4]
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        diagnostic
            .repair
            .as_deref()
            .is_some_and(|repair| !repair.is_empty())
    }));
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unsupported ASL unit dependency: generated:other")
    }));
}

#[test]
fn synthetic_decoder_identity_is_coalesced_per_project() {
    let directory = tempfile::tempdir().unwrap();
    for (file, id) in [
        ("first.asl", "PTO-TILE-FIRST"),
        ("second.asl", "PTO-TILE-SECOND"),
    ] {
        fs::write(
            directory.path().join(file),
            format!(
                "// PTO-UNIT: {{\"id\":\"{id}\",\"surface\":\"tile\",\"classification\":[\"test\"],\"depends_on\":[\"generated:decoders\"]}}\n"
            ),
        )
        .unwrap();
    }
    let mut project = ProjectManifest::for_test("pto-spec", directory.path(), ["PTO"], ["tile"]);
    project.roots = vec!["*.asl".to_owned()];

    let graph = parse_project(&project).unwrap();

    assert!(graph.diagnostics.is_empty());
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.id.local_id() == "PTO-SYNTHETIC-GENERATED-DECODERS")
            .count(),
        1
    );
}

#[test]
fn state_metadata_emits_architecture_node_and_provenance_edges() {
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test("pto-spec", &root, ["PTO"], ["tile"]);
    let parsed = parse_pto_asl(&root.join("accept.asl"), &project).unwrap();
    let state = parsed
        .nodes
        .iter()
        .find(|node| node.id.local_id() == "PTO-STATE-TILE-LOCAL")
        .unwrap();

    assert_eq!(state.kind, NodeKind::Architecture);
    assert_eq!(state.attributes["pto_entity"], "state");
    assert_eq!(state.attributes["scope"], "core");
    assert_eq!(state.attributes["members"], "_Tiles,_TileAllocationMasks");
    assert!(parsed.edges.iter().any(|edge| {
        edge.edge_type == "defined-in"
            && edge.source.local_id() == "PTO-STATE-TILE-LOCAL"
            && edge.target.local_id() == "PTO-TILE-TLOAD"
    }));
    assert!(parsed.edges.iter().any(|edge| {
        edge.edge_type == "owns-state"
            && edge.source.local_id() == "PTO-TILE-TLOAD"
            && edge.target.local_id() == "PTO-STATE-TILE-LOCAL"
    }));
}

#[test]
fn invalid_state_metadata_and_references_fail_closed() {
    let root = repository().join("tests/fixtures/pto-asl");
    let project = ProjectManifest::for_test("pto-spec", &root, ["PTO"], ["state"]);
    let parsed = parse_pto_asl(&root.join("reject-state.asl"), &project).unwrap();
    let codes = parsed
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        codes,
        vec![
            "NDF-ASL-008",
            "NDF-ASL-009",
            "NDF-ASL-009",
            "NDF-ASL-008",
            "NDF-ASL-008",
            "NDF-ASL-008",
            "NDF-ASL-008",
            "NDF-ASL-009",
        ]
    );
    assert!(parsed.diagnostics.iter().all(|diagnostic| {
        diagnostic
            .repair
            .as_deref()
            .is_some_and(|repair| !repair.is_empty())
    }));
}

fn parse_complete_pto_project(files: &[(&str, &str)]) -> Graph {
    let directory = tempfile::tempdir().unwrap();
    for (name, text) in files {
        fs::write(directory.path().join(name), text).unwrap();
    }
    let mut project = ProjectManifest::for_test("pto-spec", directory.path(), ["PTO"], ["state"]);
    project.roots = vec!["*.asl".to_owned()];
    parse_project(&project).unwrap()
}

fn diagnostic_codes(graph: &Graph) -> Vec<&str> {
    graph
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

#[test]
fn complete_pto_graph_validation_reports_dependencies_and_cycles() {
    let unknown_unit = parse_complete_pto_project(&[(
        "unknown-unit.asl",
        "// PTO-UNIT: {\"id\":\"PTO-ARCH-UNKNOWN-UNIT\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[\"PTO-ARCH-MISSING\"]}\n",
    )]);
    assert_eq!(diagnostic_codes(&unknown_unit), ["NDF-ASL-007"]);

    let unit_cycle = parse_complete_pto_project(&[(
        "unit-cycle.asl",
        "// PTO-UNIT: {\"id\":\"PTO-ARCH-UNIT-CYCLE\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[\"PTO-ARCH-UNIT-CYCLE\"]}\n",
    )]);
    assert_eq!(diagnostic_codes(&unit_cycle), ["NDF-ASL-007"]);

    let unknown_state = parse_complete_pto_project(&[(
        "unknown-state.asl",
        concat!(
            "// PTO-UNIT: {\"id\":\"PTO-ARCH-UNKNOWN-STATE\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
            "// PTO-STATE: {\"id\":\"PTO-STATE-ARCH-UNKNOWN\",\"classification\":[\"arch\",\"test\"],\"scope\":\"core\",\"owner\":\"PTO-ARCH-UNKNOWN-STATE\",\"members\":[\"_Known\"],\"depends_on\":[\"PTO-STATE-ARCH-MISSING\"]}\n",
            "var _Known : bits(32);\n",
        ),
    )]);
    assert_eq!(diagnostic_codes(&unknown_state), ["NDF-ASL-009"]);

    let state_cycle = parse_complete_pto_project(&[(
        "state-cycle.asl",
        concat!(
            "// PTO-UNIT: {\"id\":\"PTO-ARCH-STATE-CYCLE\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
            "// PTO-STATE: {\"id\":\"PTO-STATE-ARCH-CYCLE\",\"classification\":[\"arch\",\"test\"],\"scope\":\"core\",\"owner\":\"PTO-ARCH-STATE-CYCLE\",\"members\":[\"_Known\"],\"depends_on\":[\"PTO-STATE-ARCH-CYCLE\"]}\n",
            "var _Known : bits(32);\n",
        ),
    )]);
    assert_eq!(diagnostic_codes(&state_cycle), ["NDF-ASL-009"]);
}

#[test]
fn complete_pto_graph_is_valid_and_deterministic() {
    let owner = (
        "owner.asl",
        concat!(
            "// PTO-UNIT: {\"id\":\"PTO-ARCH-OWNER\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
            "// PTO-STATE: {\"id\":\"PTO-STATE-ARCH-OWNER\",\"classification\":[\"arch\",\"test\"],\"scope\":\"core\",\"owner\":\"PTO-ARCH-OWNER\",\"members\":[\"_Owner\"],\"depends_on\":[]}\n",
            "var _Owner : bits(32);\n",
        ),
    );
    let dependency = (
        "dependency.asl",
        concat!(
            "// PTO-UNIT: {\"id\":\"PTO-ARCH-DEPENDENCY\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[\"PTO-ARCH-OWNER\"]}\n",
            "// PTO-STATE: {\"id\":\"PTO-STATE-ARCH-DEPENDENCY\",\"classification\":[\"arch\",\"test\"],\"scope\":\"core\",\"owner\":\"PTO-ARCH-DEPENDENCY\",\"members\":[\"_Dependency\"],\"depends_on\":[\"PTO-STATE-ARCH-OWNER\"]}\n",
            "var _Dependency : bits(32);\n",
        ),
    );

    let forward = parse_complete_pto_project(&[owner, dependency]);
    let reverse = parse_complete_pto_project(&[dependency, owner]);

    assert!(forward.diagnostics.is_empty());
    assert_eq!(
        serde_json::to_vec(&forward).unwrap(),
        serde_json::to_vec(&reverse).unwrap()
    );
}

#[test]
fn complete_pto_graph_fixture_reports_each_cycle_source_once() {
    let root = repository().join("tests/fixtures/pto-asl-project");
    let project = ProjectManifest::load(&root.join("ndf.yaml")).unwrap();

    let graph = parse_project(&project).unwrap();

    assert_eq!(diagnostic_codes(&graph), ["NDF-ASL-007", "NDF-ASL-007"]);
    assert_eq!(
        graph
            .diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.subject.as_ref())
            .map(|subject| subject.local_id())
            .collect::<Vec<_>>(),
        ["PTO-ARCH-CYCLE-A", "PTO-ARCH-CYCLE-B"]
    );
}

#[test]
fn complete_pto_graph_uses_pto_diagnostics_for_cross_file_duplicates() {
    let duplicate_unit = parse_complete_pto_project(&[
        (
            "first.asl",
            "// PTO-UNIT: {\"id\":\"PTO-ARCH-DUPLICATE\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
        ),
        (
            "second.asl",
            "// PTO-UNIT: {\"id\":\"PTO-ARCH-DUPLICATE\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
        ),
    ]);
    assert_eq!(diagnostic_codes(&duplicate_unit), ["NDF-ASL-006"]);

    let duplicate_state = parse_complete_pto_project(&[
        (
            "first.asl",
            concat!(
                "// PTO-UNIT: {\"id\":\"PTO-ARCH-FIRST\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
                "// PTO-STATE: {\"id\":\"PTO-STATE-ARCH-DUPLICATE\",\"classification\":[\"arch\",\"test\"],\"scope\":\"core\",\"owner\":\"PTO-ARCH-FIRST\",\"members\":[\"_First\"],\"depends_on\":[]}\n",
                "var _First : bits(32);\n",
            ),
        ),
        (
            "second.asl",
            concat!(
                "// PTO-UNIT: {\"id\":\"PTO-ARCH-SECOND\",\"surface\":\"arch\",\"classification\":[\"test\"],\"depends_on\":[]}\n",
                "// PTO-STATE: {\"id\":\"PTO-STATE-ARCH-DUPLICATE\",\"classification\":[\"arch\",\"test\"],\"scope\":\"core\",\"owner\":\"PTO-ARCH-SECOND\",\"members\":[\"_Second\"],\"depends_on\":[]}\n",
                "var _Second : bits(32);\n",
            ),
        ),
    ]);
    assert_eq!(diagnostic_codes(&duplicate_state), ["NDF-ASL-008"]);
}
