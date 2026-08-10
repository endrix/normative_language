use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::adapters::pto_asl::{instruction_clause_id, parse_pto_asl};
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
