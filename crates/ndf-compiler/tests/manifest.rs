use std::fs;

use ndf_compiler::manifest::{ProjectLock, ProjectManifest};

fn fixture(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/markdown/project")
        .join(path)
}

#[test]
fn complete_manifest_and_lock_are_normalized() {
    let manifest = ProjectManifest::load(&fixture("ndf.yaml")).unwrap();
    let lock = ProjectLock::load(&fixture("ndf.lock")).unwrap();

    assert_eq!(manifest.format_version, "0.2");
    assert_eq!(manifest.project, "fixture");
    assert_eq!(manifest.roots, ["docs/**/*.md"]);
    assert_eq!(manifest.id_prefixes, ["DOC", "PIPE"]);
    assert_eq!(lock.dependencies[0].project, "pto-spec");
    assert_eq!(
        lock.dependencies[0].revision,
        "0123456789abcdef0123456789abcdef01234567"
    );
}

#[test]
fn lock_rejects_a_symbolic_revision() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ndf.lock");
    fs::write(
        &path,
        "format_version: '0.1'\ndependencies:\n  pto-spec:\n    uri: https://github.com/PTO-ISA/pto-spec.git\n    revision: main\n    path: external/pto-spec\n",
    )
    .unwrap();

    let error = ProjectLock::load(&path).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("dependency revision must be 40 lowercase hexadecimal characters")
    );
}

#[test]
fn manifest_reports_the_first_missing_required_field() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ndf.yaml");
    fs::write(&path, "format_version: '0.2'\nproject: fixture\n").unwrap();

    let error = ProjectManifest::load(&path).unwrap_err();

    assert!(error.to_string().contains("missing required field: roots"));
}

#[test]
fn dependency_graph_loading_is_explicit_and_defaults_to_tool_only() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ndf.yaml");
    fs::write(
        &path,
        r#"format_version: '0.2'
project: consumer
roots: []
id_prefixes: [REQ]
domains: []
policies: {}
dependencies:
  ndf:
    path: toolchains/ndf
  pto-spec:
    path: isa
    graph: true
"#,
    )
    .unwrap();

    let manifest = ProjectManifest::load(&path).unwrap();

    assert!(!manifest.dependencies["ndf"].graph);
    assert!(manifest.dependencies["pto-spec"].graph);
}

#[test]
fn dependency_graph_flag_must_be_boolean() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ndf.yaml");
    fs::write(
        &path,
        r#"format_version: '0.2'
project: consumer
roots: []
id_prefixes: [REQ]
domains: []
policies: {}
dependencies:
  pto-spec:
    path: isa
    graph: yes
"#,
    )
    .unwrap();

    let error = ProjectManifest::load(&path).unwrap_err();

    assert!(error.to_string().contains("graph must be a boolean"));
}
