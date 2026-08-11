use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use ndf_compiler::dependencies::{load_dependency_graphs, resolve_dependencies};
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
            graph: false,
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
    assert!(
        load_dependency_graphs(&manifest, &lock, directory.path())
            .unwrap()
            .is_empty()
    );
    assert_eq!(git(&dependency, &["status", "--porcelain=v1"]), before);
}

fn graph_contracts(root: &Path, project: &str, revision: &str) -> (ProjectManifest, ProjectLock) {
    let mut manifest = ProjectManifest::for_test("consumer", root, ["REQ"], []);
    manifest.dependencies = BTreeMap::from([(
        project.to_owned(),
        DependencyDeclaration {
            path: format!("deps/{project}"),
            graph: true,
        },
    )]);
    let lock = ProjectLock {
        format_version: "0.1".to_owned(),
        dependencies: vec![LockedDependency {
            project: project.to_owned(),
            uri: format!("https://example.invalid/{project}.git"),
            revision: revision.to_owned(),
            path: format!("deps/{project}"),
        }],
    };
    (manifest, lock)
}

fn commit_graph_project(path: &Path, project: &str, asl: &str) -> String {
    fs::create_dir_all(path.join("asl")).unwrap();
    git(path, &["init", "-q"]);
    git(path, &["config", "user.email", "ndf@example.invalid"]);
    git(path, &["config", "user.name", "NDF Test"]);
    fs::write(
        path.join("ndf.yaml"),
        format!(
            "format_version: '0.2'\nproject: {project}\nroots: ['asl/**/*.asl']\nid_prefixes: [PTO]\ndomains: [architecture]\npolicies: {{}}\ndependencies: {{}}\n"
        ),
    )
    .unwrap();
    fs::write(path.join("asl/contracts.asl"), asl).unwrap();
    git(path, &["add", "ndf.yaml", "asl/contracts.asl"]);
    git(path, &["commit", "-qm", "graph fixture"]);
    git(path, &["rev-parse", "HEAD"])
}

#[test]
fn resolved_dependency_graphs_are_exact_read_only_namespaces() {
    let directory = tempfile::tempdir().unwrap();
    let dependency = directory.path().join("deps/pto-spec");
    let revision = commit_graph_project(
        &dependency,
        "pto-spec",
        r#"// NDF-BEGIN: PTO-CONTRACT-001
// ndf: kind=contract level=L1 layer=architecture status=accepted
// The released architecture contract is authoritative.
// NDF-END: PTO-CONTRACT-001
"#,
    );
    let before = git(&dependency, &["status", "--porcelain=v1"]);
    let (manifest, lock) = graph_contracts(directory.path(), "pto-spec", &revision);

    let graphs = load_dependency_graphs(&manifest, &lock, directory.path()).unwrap();

    assert_eq!(graphs.len(), 1);
    let graph = &graphs["pto-spec"];
    assert_eq!(graph.project_id, "pto-spec");
    assert_eq!(graph.revision, revision);
    assert!(
        graph
            .graph
            .nodes
            .iter()
            .any(|node| { node.id.to_string() == "ndf://pto-spec/PTO-CONTRACT-001" })
    );
    assert_eq!(git(&dependency, &["status", "--porcelain=v1"]), before);
}

#[test]
fn graph_dependencies_reject_missing_mismatched_and_duplicate_projects() {
    let missing = tempfile::tempdir().unwrap();
    let missing_dependency = missing.path().join("deps/pto-spec");
    let revision = git_repository(&missing_dependency);
    let (manifest, lock) = graph_contracts(missing.path(), "pto-spec", &revision);
    assert!(
        load_dependency_graphs(&manifest, &lock, missing.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-005")
    );

    let mismatched = tempfile::tempdir().unwrap();
    let mismatched_dependency = mismatched.path().join("deps/pto-spec");
    let revision = commit_graph_project(
        &mismatched_dependency,
        "another-project",
        r#"// NDF-BEGIN: PTO-CONTRACT-001
// ndf: kind=contract level=L1 layer=architecture status=accepted
// The released architecture contract is authoritative.
// NDF-END: PTO-CONTRACT-001
"#,
    );
    let (manifest, lock) = graph_contracts(mismatched.path(), "pto-spec", &revision);
    assert!(
        load_dependency_graphs(&manifest, &lock, mismatched.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-005")
    );

    let duplicate = tempfile::tempdir().unwrap();
    let duplicate_dependency = duplicate.path().join("deps/pto-spec");
    let revision = commit_graph_project(
        &duplicate_dependency,
        "pto-spec",
        r#"// NDF-BEGIN: PTO-CONTRACT-001
// ndf: kind=contract level=L1 layer=architecture status=accepted
// The first contract is authoritative.
// NDF-END: PTO-CONTRACT-001
// NDF-BEGIN: PTO-CONTRACT-001
// ndf: kind=contract level=L1 layer=architecture status=accepted
// The duplicate contract is invalid.
// NDF-END: PTO-CONTRACT-001
"#,
    );
    let (manifest, lock) = graph_contracts(duplicate.path(), "pto-spec", &revision);
    assert!(
        load_dependency_graphs(&manifest, &lock, duplicate.path())
            .unwrap_err()
            .to_string()
            .contains("NDF-DEP-006")
    );
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
