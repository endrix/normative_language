use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use ndf_compiler::fix::{FixMode, safe_fix};
use ndf_compiler::id_allocator::allocate_id;
use serde_json::Value;

#[test]
fn invalid_invocation_exits_two_and_runtime_errors_use_the_envelope() {
    let binary = assert_cmd::cargo::cargo_bin!("ndf");
    let invalid = Command::new(binary)
        .args(["build", "--unknown"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));

    let directory = tempfile::tempdir().unwrap();
    let runtime = Command::new(binary)
        .args(["show", "MISSING", "--index", "missing.sqlite"])
        .current_dir(directory.path())
        .output()
        .unwrap();
    let payload: Value = serde_json::from_slice(&runtime.stdout).unwrap();
    assert_eq!(runtime.status.code(), Some(1));
    assert_eq!(payload["schema_version"], "0.1");
    assert_eq!(payload["command"], "show");
    assert_eq!(payload["ok"], false);
    assert!(payload["diagnostics"][0]["message"].is_string());
}

#[test]
fn id_allocation_is_monotonic_and_safe_fix_refuses_semantic_changes() {
    let occupied = BTreeSet::from(["PIPE-ISSUE-001".to_owned(), "PIPE-ISSUE-003".to_owned()]);
    assert_eq!(
        allocate_id("PIPE-ISSUE", &occupied).unwrap(),
        "PIPE-ISSUE-004"
    );
    assert!(allocate_id("pipe issue", &occupied).is_err());

    let result = safe_fix(
        &[],
        &FixMode {
            requested_changes: BTreeSet::from(["status".to_owned()]),
            ..FixMode::default()
        },
    )
    .unwrap();
    assert_eq!(result.diagnostics[0].code, "NDF-FIX-001");
}

#[test]
fn fix_check_reports_reordering_without_writing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("design.md");
    let original = "<!-- ndf: status=active kind=requirement modality=must -->\n";
    std::fs::write(&path, original).unwrap();

    let result = safe_fix(
        std::slice::from_ref(&path),
        &FixMode {
            check: true,
            ..FixMode::default()
        },
    )
    .unwrap();

    assert_eq!(result.changed, [path.to_string_lossy()]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert_eq!(Path::new(&result.changed[0]), path);
}

#[test]
fn pto_release_impact_rejects_missing_and_corrupt_indexes() {
    let binary = assert_cmd::cargo::cargo_bin!("ndf");
    let directory = tempfile::tempdir().unwrap();
    let corrupt = directory.path().join("corrupt.sqlite");
    std::fs::write(&corrupt, "not a SQLite index").unwrap();

    for before in [directory.path().join("missing.sqlite"), corrupt] {
        let output = Command::new(binary)
            .args([
                "impact",
                "pto-release",
                "--before",
                before.to_str().unwrap(),
                "--after",
                before.to_str().unwrap(),
                "--consumer-index",
                before.to_str().unwrap(),
                "--format",
                "json",
            ])
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(payload["command"], "impact pto-release");
        assert_eq!(payload["ok"], false);
        assert!(payload["diagnostics"][0]["message"].is_string());
    }
}
