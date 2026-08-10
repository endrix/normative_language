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

#[test]
fn graph_commands_emit_versioned_machine_envelopes() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = repository().join("tests/fixtures/markdown/project");
    let root = fixture.to_string_lossy();
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
