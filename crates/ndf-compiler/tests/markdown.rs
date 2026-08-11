use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_compiler::manifest::ProjectManifest;
use ndf_compiler::markdown::parse_markdown;
use ndf_core::model::Graph;
use serde_json::Value;
use tempfile::tempdir;

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

fn parse_fixture(text: &str) -> ndf_core::model::ParseResult {
    let directory = tempdir().unwrap();
    let path = directory.path().join("contracts.md");
    fs::write(&path, text).unwrap();
    let manifest =
        ProjectManifest::for_test("davincioo", directory.path(), ["DOC", "ISA"], ["isa"]);
    parse_markdown(&path, &manifest).unwrap()
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

#[test]
fn davincioo_legacy_metadata_preserves_attributes_and_spaced_provenance() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("legacy-davincioo.md");
    fs::write(
        &path,
        r#"---
doc_id: DOC-DAVINCIOO-LEGACY
status: active
authority: normative
owner: core
---

# Legacy DavinciOO

## Parent {#PIPE-001}
<!-- ndf: kind=req level=must layer=L1 status=stable since=0.3 -->

The parent MUST exist.

## Mechanism {#PIPE-010}
<!-- ndf: kind=arch level=must layer=L2 status=stable since=0.3 author=ndf-projection unit=entries couples-with=PIPE-001 derived-from=docs/layouts/shape-m16 for A C.svg -->

The mechanism MUST preserve provenance.

## Legacy definition {#DEF-001}
<!-- ndf: kind=def level=must layer=L1 status=stable since=0.1 -->

This term MUST retain its definition kind.

## Superseded contract {#PIPE-020}
<!-- ndf: kind=req level=must layer=L1 status=superseded-by=PIPE-010 since=0.1 -->

This contract MUST remain traceable after supersession.
"#,
    )
    .unwrap();
    let manifest = ProjectManifest::for_test(
        "davincioo",
        directory.path(),
        ["DEF", "DOC", "PIPE"],
        ["core"],
    );

    let parsed = parse_markdown(&path, &manifest).unwrap();
    let mechanism = parsed
        .nodes
        .iter()
        .find(|node| node.id.to_string() == "ndf://davincioo/PIPE-010")
        .unwrap();

    assert_eq!(mechanism.kind, ndf_core::model::NodeKind::Architecture);
    assert_eq!(mechanism.attributes.get("since").unwrap(), "0.3");
    assert_eq!(
        mechanism.attributes.get("author").unwrap(),
        "ndf-projection"
    );
    assert_eq!(mechanism.attributes.get("unit").unwrap(), "entries");
    assert_eq!(
        mechanism.attributes.get("derived-from").unwrap(),
        "docs/layouts/shape-m16 for A C.svg"
    );
    assert!(parsed.edges.iter().any(|edge| {
        edge.edge_type == "couples-with"
            && edge.source.to_string() == "ndf://davincioo/PIPE-010"
            && edge.target.to_string() == "ndf://davincioo/PIPE-001"
    }));
    let definition = parsed
        .nodes
        .iter()
        .find(|node| node.id.to_string() == "ndf://davincioo/DEF-001")
        .unwrap();
    assert_eq!(definition.kind, ndf_core::model::NodeKind::Definition);
    let superseded = parsed
        .nodes
        .iter()
        .find(|node| node.id.to_string() == "ndf://davincioo/PIPE-020")
        .unwrap();
    assert_eq!(
        superseded.status,
        ndf_core::model::LifecycleStatus::Superseded
    );
    assert_eq!(
        superseded.attributes.get("superseded-by").unwrap(),
        "PIPE-010"
    );
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "NDF-MIG-001")
            .count(),
        4
    );
}

#[test]
fn cross_project_relationships_preserve_edges_and_absence_expectation() {
    let parsed = parse_fixture(
        r#"---
doc_id: DOC-CROSS-PROJECT
status: active
authority: normative
owner: architecture
---

# Cross-project contracts

## Accepted load contract {#ISA-TLOAD-001}
<!-- ndf: kind=requirement modality=must refinement=L1 status=active conforms-to=ndf://pto-spec/PTO-INST-TILE-TLOAD -->

DavinciOO MUST conform to the accepted load contract.

## Prospective extension {#ISA-PEXT-001}
<!-- ndf: kind=requirement modality=tbd refinement=L1 status=draft tracks=ndf://pto-spec/PTO-INST-TILE-PEXT expected=absent -->

The prospective operation remains disabled.
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    assert!(parsed.edges.iter().any(|edge| {
        edge.edge_type == "conforms-to"
            && edge.source.to_string() == "ndf://davincioo/ISA-TLOAD-001"
            && edge.target.to_string() == "ndf://pto-spec/PTO-INST-TILE-TLOAD"
    }));
    assert!(parsed.edges.iter().any(|edge| {
        edge.edge_type == "tracks"
            && edge.source.to_string() == "ndf://davincioo/ISA-PEXT-001"
            && edge.target.to_string() == "ndf://pto-spec/PTO-INST-TILE-PEXT"
    }));
    let watch = parsed
        .nodes
        .iter()
        .find(|node| node.id.to_string() == "ndf://davincioo/ISA-PEXT-001")
        .unwrap();
    assert_eq!(
        watch.attributes.get("expected").map(String::as_str),
        Some("absent")
    );
}

#[test]
fn expected_is_restricted_to_tracks_and_absent() {
    let parsed = parse_fixture(
        r#"---
doc_id: DOC-CROSS-PROJECT-INVALID
status: active
authority: normative
owner: architecture
---

# Invalid cross-project contracts

## Unsupported expectation {#ISA-WATCH-001}
<!-- ndf: kind=requirement modality=tbd refinement=L1 status=draft tracks=ndf://pto-spec/PTO-INST-TILE-PEXT expected=present -->

The expectation is invalid.

## Missing watch {#ISA-WATCH-002}
<!-- ndf: kind=requirement modality=tbd refinement=L1 status=draft expected=absent -->

The expectation has no watched identity.

## Missing expectation {#ISA-WATCH-003}
<!-- ndf: kind=requirement modality=tbd refinement=L1 status=draft tracks=ndf://pto-spec/PTO-INST-TILE-CEXT -->

The watched identity has no absence expectation.
"#,
    );

    let diagnostics: Vec<_> = parsed
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "NDF-XREF-001")
        .collect();
    assert_eq!(diagnostics.len(), 3, "{:#?}", parsed.diagnostics);
    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.severity == ndf_core::model::DiagnosticSeverity::Error
        })
    );
}
