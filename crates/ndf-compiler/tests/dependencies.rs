use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use ndf_compiler::dependencies::resolve_dependencies;
use ndf_compiler::manifest::{
    DependencyDeclaration, LockedDependency, ProjectLock, ProjectManifest,
};

fn git(path: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn git_repository(path: &Path) -> String {
    fs::create_dir_all(path).unwrap();
    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "ndf@example.invalid"]);
    git(path, &["config", "user.name", "NDF Test"]);
    fs::write(path.join("README.md"), "fixture\n").unwrap();
    git(path, &["add", "README.md"]);
    git(path, &["commit", "-qm", "fixture"]);
    git(path, &["rev-parse", "HEAD"])
}

fn contracts(root: &Path, revision: &str) -> (ProjectManifest, ProjectLock) {
    let mut manifest = ProjectManifest::for_test("fixture", root, ["REQ"], []);
    manifest.dependencies = BTreeMap::from([(
        "tool".to_owned(),
        DependencyDeclaration {
            path: "deps/tool".to_owned(),
        },
    )]);
    let lock = ProjectLock {
        format_version: "0.1".to_owned(),
        dependencies: vec![LockedDependency {
            project: "tool".to_owned(),
            uri: "https://example.invalid/tool.git".to_owned(),
            revision: revision.to_owned(),
            path: "deps/tool".to_owned(),
        }],
    };
    (manifest, lock)
}

#[test]
fn exact_clean_git_revision_is_resolved_without_repository_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let dependency = directory.path().join("deps/tool");
    let revision = git_repository(&dependency);
    let before = git(&dependency, &["status", "--porcelain=v1"]);
    let (manifest, lock) = contracts(directory.path(), &revision);

    let resolved = resolve_dependencies(&manifest, &lock, directory.path()).unwrap();

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].project_id, "tool");
    assert_eq!(resolved[0].revision, revision);
    assert!(!resolved[0].dirty);
    assert_eq!(git(&dependency, &["status", "--porcelain=v1"]), before);
}

#[test]
fn missing_symbolic_mismatched_and_dirty_dependencies_are_rejected() {
    let missing = tempfile::tempdir().unwrap();
    let (manifest, lock) = contracts(missing.path(), &"0".repeat(40));
    assert!(
        resolve_dependencies(&manifest, &lock, missing.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-001")
    );

    let directory = tempfile::tempdir().unwrap();
    let dependency = directory.path().join("deps/tool");
    let revision = git_repository(&dependency);
    let (manifest, mut lock) = contracts(directory.path(), &revision);
    lock.dependencies[0].revision = "main".to_owned();
    assert!(
        resolve_dependencies(&manifest, &lock, directory.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-002")
    );
    lock.dependencies[0].revision = "f".repeat(40);
    assert!(
        resolve_dependencies(&manifest, &lock, directory.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-003")
    );
    lock.dependencies[0].revision = revision;
    fs::write(dependency.join("dirty.txt"), "dirty\n").unwrap();
    assert!(
        resolve_dependencies(&manifest, &lock, directory.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-004")
    );
}
