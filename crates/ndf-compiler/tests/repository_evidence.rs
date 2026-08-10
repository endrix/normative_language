use std::path::{Path, PathBuf};

use ndf_compiler::adapters::adapter_for;
use ndf_compiler::adapters::evidence::parse_evidence_manifest;
use ndf_compiler::adapters::repository::index_repository_paths;
use ndf_compiler::manifest::ProjectManifest;
use ndf_core::model::NodeKind;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn repository_paths_get_deterministic_source_test_nodes_and_edges() {
    let root = repository().join("tests/fixtures/repository");
    let project = ProjectManifest::for_test("fixture", &root, ["REQ"], ["core"]);
    let forward =
        index_repository_paths(&root, &["src/**/*.py", "tests/**/*.py"], &project).unwrap();
    let reverse =
        index_repository_paths(&root, &["tests/**/*.py", "src/**/*.py"], &project).unwrap();

    assert_eq!(forward, reverse);
    assert_eq!(forward.nodes[0].kind, NodeKind::Source);
    assert_eq!(forward.nodes[1].kind, NodeKind::Test);
    assert_eq!(forward.edges[0].edge_type, "implements");
    assert_eq!(forward.edges[1].edge_type, "verifies");
    assert_eq!(forward.nodes[0].attributes["path"], "src/core.py");
    assert_eq!(forward.nodes[0].attributes["content_hash"].len(), 64);
}

#[test]
fn evidence_manifest_is_validated_and_linked_to_its_clause() {
    let root = repository().join("tests/fixtures/repository");
    let project = ProjectManifest::for_test("fixture", &root, ["EVID"], ["core"]);
    let parsed = parse_evidence_manifest(&root.join("evidence/core.json"), &project).unwrap();

    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.nodes[0].kind, NodeKind::Evidence);
    assert_eq!(parsed.nodes[0].id.local_id(), "EVID-CORE-001");
    assert_eq!(parsed.edges[0].source.to_string(), "ndf://fixture/REQ-001");
    assert_eq!(
        parsed.edges[0].target.to_string(),
        "ndf://fixture/EVID-CORE-001"
    );
    assert_eq!(parsed.edges[0].edge_type, "evidenced-by");
}

#[test]
fn built_in_adapter_registry_is_static_and_extension_selected() {
    assert_eq!(
        adapter_for(Path::new("design.md")).unwrap().name(),
        "markdown"
    );
    assert_eq!(
        adapter_for(Path::new("spec.asl")).unwrap().name(),
        "pto-asl"
    );
    assert_eq!(
        adapter_for(Path::new("evidence.json")).unwrap().name(),
        "evidence"
    );
    assert!(adapter_for(Path::new("generated.bin")).is_none());
}

#[test]
fn invalid_evidence_is_a_structured_diagnostic() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.json");
    std::fs::write(&path, r#"{"format_version":"0.1","id":"EVID-1"}"#).unwrap();
    let project = ProjectManifest::for_test("fixture", directory.path(), ["EVID"], ["core"]);

    let parsed = parse_evidence_manifest(&path, &project).unwrap();

    assert!(parsed.nodes.is_empty());
    assert_eq!(parsed.diagnostics[0].code, "NDF-EVID-001");
}
