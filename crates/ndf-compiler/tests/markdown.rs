use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::manifest::ProjectManifest;
use ndf_compiler::markdown::parse_markdown;
use ndf_core::model::Graph;
use serde_json::Value;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn graph_value(path: &Path, manifest: &ProjectManifest) -> Value {
    let parsed = parse_markdown(path, manifest).unwrap();
    serde_json::to_value(Graph {
        nodes: parsed.nodes,
        edges: parsed.edges,
        diagnostics: parsed.diagnostics,
    })
    .unwrap()
}

#[test]
fn valid_markdown_matches_the_frozen_contract() {
    let root = repository().join("tests/fixtures/markdown/project");
    let manifest = ProjectManifest::load(&root.join("ndf.yaml")).unwrap();
    let expected: Value = serde_json::from_str(
        &fs::read_to_string(repository().join("tests/golden/markdown-valid.json")).unwrap(),
    )
    .unwrap();

    let actual = graph_value(&root.join("docs/pipeline.md"), &manifest);

    assert_eq!(actual, expected);
}

#[test]
fn invalid_and_legacy_markdown_match_the_frozen_contract() {
    let root = repository().join("tests/fixtures/oracle/markdown-invalid");
    let manifest = ProjectManifest::for_test("fixture", &root, ["DOC", "PIPE"], ["core"]);
    let expected: BTreeMap<String, Value> = serde_json::from_str(
        &fs::read_to_string(repository().join("tests/golden/markdown-invalid.json")).unwrap(),
    )
    .unwrap();

    let actual: BTreeMap<String, Value> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                graph_value(&path, &manifest),
            )
        })
        .collect();

    assert_eq!(actual, expected);
}
