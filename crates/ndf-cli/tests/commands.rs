use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ndf(cwd: &Path, arguments: &[&str]) -> (i32, Value) {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("ndf"))
        .args(arguments)
        .current_dir(cwd)
        .output()
        .unwrap();
    let payload = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON ({error}): stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), payload)
}

fn assert_envelope(status: i32, payload: &Value, command: &str) {
    assert_eq!(status, 0, "{payload}");
    assert_eq!(payload["schema_version"], "0.1");
    assert_eq!(payload["command"], command);
    assert_eq!(payload["ok"], true);
    assert!(payload.get("data").is_some());
    assert!(payload["diagnostics"].is_array());
}

fn git(path: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn initialize_pto_dependency(root: &Path) -> String {
    let dependency = root.join("deps/pto-spec");
    fs::create_dir_all(dependency.join("asl")).unwrap();
    git(&dependency, &["init", "-q"]);
    git(
        &dependency,
        &["config", "user.email", "ndf@example.invalid"],
    );
    git(&dependency, &["config", "user.name", "NDF Test"]);
    fs::write(
        dependency.join("ndf.yaml"),
        "format_version: '0.2'\nproject: pto-spec\nroots: ['asl/**/*.asl']\nid_prefixes: [PTO]\ndomains: [architecture]\npolicies: {}\ndependencies: {}\n",
    )
    .unwrap();
    fs::write(
        dependency.join("asl/contracts.asl"),
        r#"// NDF-BEGIN: PTO-CONTRACT-001
// ndf: kind=contract level=L1 layer=architecture status=accepted
// The released architecture contract is authoritative.
// NDF-END: PTO-CONTRACT-001
// NDF-BEGIN: PTO-INST-TILE-TLOAD
// ndf: kind=contract level=L1 layer=tile status=accepted
// TLOAD is an accepted tile instruction.
// NDF-END: PTO-INST-TILE-TLOAD
"#,
    )
    .unwrap();
    git(&dependency, &["add", "ndf.yaml", "asl/contracts.asl"]);
    git(&dependency, &["commit", "-qm", "PTO graph fixture"]);
    git(&dependency, &["rev-parse", "HEAD"])
}

fn write_lock(root: &Path, revision: &str) {
    fs::write(
        root.join("ndf.lock"),
        format!(
            "format_version: '0.1'\ndependencies:\n  pto-spec:\n    uri: https://example.invalid/pto-spec.git\n    revision: {revision}\n    path: deps/pto-spec\n"
        ),
    )
    .unwrap();
}

fn cross_project_fixture(root: &Path, target: &str) {
    let revision = initialize_pto_dependency(root);

    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/consumer.md"),
        format!(
            r#"---
doc_id: DOC-CONSUMER
status: active
authority: normative
owner: architecture
---

# Consumer

## External contract {{#REQ-001}}
<!-- ndf: kind=requirement modality=must refinement=L1 status=active conforms-to=ndf://pto-spec/{target} -->

The consumer MUST preserve the external contract.
"#
        ),
    )
    .unwrap();
    fs::write(
        root.join("ndf.yaml"),
        "format_version: '0.2'\nproject: consumer\nroots: ['docs/**/*.md']\nid_prefixes: [DOC, REQ]\ndomains: []\npolicies: {}\ndependencies:\n  pto-spec:\n    path: deps/pto-spec\n    graph: true\n",
    )
    .unwrap();
    write_lock(root, &revision);
}

fn graph_command_fixture(root: &Path) {
    let revision = initialize_pto_dependency(root);
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::copy(
        repository().join("tests/fixtures/markdown/project/docs/pipeline.md"),
        root.join("docs/pipeline.md"),
    )
    .unwrap();
    fs::write(
        root.join("ndf.yaml"),
        "format_version: '0.2'\nproject: fixture\nroots: ['docs/**/*.md']\nid_prefixes: [DOC, PIPE]\ndomains: [core]\npolicies: {}\ndependencies:\n  pto-spec:\n    path: deps/pto-spec\n    graph: true\n",
    )
    .unwrap();
    write_lock(root, &revision);
}

#[test]
fn graph_commands_emit_versioned_machine_envelopes() {
    let directory = tempfile::tempdir().unwrap();
    graph_command_fixture(directory.path());
    let root = directory.path().to_string_lossy();
    let commands: &[(&[&str], &str)] = &[
        (
            &[
                "build",
                "--root",
                &root,
                "--output",
                "graph.sqlite",
                "--format",
                "json",
            ],
            "build",
        ),
        (&["check", "--root", &root, "--format", "json"], "check"),
        (
            &[
                "show",
                "ndf://fixture/PIPE-ISSUE-001",
                "--index",
                "graph.sqlite",
                "--format",
                "json",
            ],
            "show",
        ),
        (
            &[
                "trace",
                "PIPE-ISSUE-010",
                "--edge",
                "refines",
                "--format",
                "json",
            ],
            "trace",
        ),
        (
            &["report", "coverage", "--format", "json"],
            "report coverage",
        ),
        (
            &["diff", "graph.sqlite", "graph.sqlite", "--format", "json"],
            "diff",
        ),
        (
            &[
                "export",
                "--index",
                "graph.sqlite",
                "--output",
                "graph.json",
                "--format",
                "ndf-json",
            ],
            "export",
        ),
        (
            &["id", "allocate", "PIPE-ISSUE", "--format", "json"],
            "id allocate",
        ),
        (&["fix", "--generated-only", "--check"], "fix"),
    ];

    for (arguments, command) in commands {
        let (status, payload) = ndf(directory.path(), arguments);
        assert_envelope(status, &payload, command);
    }
}

#[test]
fn dependency_report_reads_an_exact_local_checkout() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let dependency = root.join("deps/tool");
    fs::create_dir_all(&dependency).unwrap();
    for arguments in [
        &["init", "-q"][..],
        &["config", "user.email", "ndf@example.invalid"],
        &["config", "user.name", "NDF Test"],
    ] {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(&dependency)
                .status()
                .unwrap()
                .success()
        );
    }
    fs::write(dependency.join("README.md"), "fixture\n").unwrap();
    for arguments in [&["add", "README.md"][..], &["commit", "-qm", "fixture"]] {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(&dependency)
                .status()
                .unwrap()
                .success()
        );
    }
    let revision = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&dependency)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let revision = revision.trim();
    fs::write(
        root.join("ndf.yaml"),
        "format_version: '0.2'\nproject: fixture\nroots: []\nid_prefixes: [REQ]\ndependencies:\n  tool:\n    path: deps/tool\n",
    )
    .unwrap();
    fs::write(
        root.join("ndf.lock"),
        format!(
            "format_version: '0.1'\ndependencies:\n  tool:\n    uri: https://example.invalid/tool.git\n    revision: {revision}\n    path: deps/tool\n"
        ),
    )
    .unwrap();

    let root_argument = root.to_string_lossy();
    let (status, payload) = ndf(
        root,
        &[
            "report",
            "dependencies",
            "--root",
            &root_argument,
            "--format",
            "json",
        ],
    );

    assert_envelope(status, &payload, "report dependencies");
    assert_eq!(payload["data"][0]["revision"], revision);
}

#[test]
fn check_rejects_a_missing_cross_project_conformance_target() {
    let directory = tempfile::tempdir().unwrap();
    cross_project_fixture(directory.path(), "PTO-MISSING-001");
    let root = directory.path().to_string_lossy();

    let (status, payload) = ndf(
        directory.path(),
        &["check", "--root", &root, "--format", "json"],
    );

    assert_eq!(status, 1, "{payload}");
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["diagnostics"][0]["code"], "NDF-XREF-002");
}
