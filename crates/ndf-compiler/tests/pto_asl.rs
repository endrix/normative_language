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
    assert_eq!(parsed.nodes[1].kind, NodeKind::Requirement);
    assert_eq!(parsed.nodes[1].refinement.as_deref(), Some("L1"));
    assert_eq!(parsed.nodes[1].status, LifecycleStatus::Active);
    assert_eq!(parsed.edges[0].edge_type, "references");
}

#[test]
fn instruction_identity_ignores_source_path() {
    let directory = tempfile::tempdir().unwrap();
    let project = ProjectManifest::for_test("pto-spec", directory.path(), ["PTO"], ["tile"]);
    let text = concat!(
        "// PTO-INSTRUCTION: ",
        r#"{"surface":"tile","mnemonic":"TLOAD","catalog_records":[{"semantic_handler":"TLOAD","contract_status":"reviewed-complete"}]}"#,
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
    let instruction = &parsed.nodes[0];

    assert_eq!(instruction.attributes["surface"], "tile");
    assert_eq!(instruction.attributes["mnemonic"], "TLOAD");
    assert_eq!(instruction.attributes["semantic_handler"], "TLOAD");
    assert_eq!(instruction.attributes["status"], "reviewed-complete");
    assert_eq!(
        instruction.attributes["semantic_facets"],
        "architectural-state,completion,encoding,fault,instruction-semantics,legality,memory-ordering"
    );
}

#[test]
fn colliding_instruction_identities_report_asl_005() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("first.asl"),
        "// PTO-INSTRUCTION: {\"surface\":\"tile\",\"mnemonic\":\"T.LOAD\"}\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("second.asl"),
        "// PTO-INSTRUCTION: {\"surface\":\"tile\",\"mnemonic\":\"T-LOAD\"}\n",
    )
    .unwrap();
    let mut project = ProjectManifest::for_test("pto-spec", directory.path(), ["PTO"], ["tile"]);
    project.roots = vec!["*.asl".to_owned()];

    let graph = parse_project(&project).unwrap();

    assert_eq!(graph.diagnostics.len(), 1);
    assert_eq!(graph.diagnostics[0].code, "NDF-ASL-005");
    assert_eq!(graph.diagnostics[0].related.len(), 1);
}
