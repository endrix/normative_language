use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ndf_compiler::index::{BuildProvenance, build_index};
use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, Graph, LifecycleStatus, Node, NodeKind, SourceSpan};
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

fn impact_node(project: &str, local_id: &str, handler: Option<&str>) -> Node {
    let mut attributes = BTreeMap::new();
    if let Some(handler) = handler {
        attributes.insert("semantic_handler".to_owned(), handler.to_owned());
        attributes.insert(
            "semantic_facets".to_owned(),
            "instruction-semantics".to_owned(),
        );
    }
    Node {
        id: NodeId::new(project, local_id).unwrap(),
        kind: NodeKind::Architecture,
        title: local_id.to_owned(),
        source: SourceSpan::new(format!("docs/{local_id}.md"), 1).unwrap(),
        modality: None,
        refinement: None,
        domain: None,
        status: LifecycleStatus::Active,
        owner: None,
        body: String::new(),
        attributes,
    }
}

fn impact_provenance(project: &str) -> BuildProvenance {
    BuildProvenance {
        project_commits: vec![(project.to_owned(), "0".repeat(40))],
        format_version: "0.2".to_owned(),
        ir_version: "0.1".to_owned(),
        tool_version: "0.1.0".to_owned(),
        normative_roots: Vec::new(),
    }
}

fn impact_indexes(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let before = root.join("before.sqlite");
    let after = root.join("after.sqlite");
    let consumer = root.join("consumer.sqlite");
    let target = "PTO-INST-TILE-TLOAD";
    build_index(
        &Graph {
            nodes: vec![impact_node("pto-spec", target, Some("TLOAD"))],
            ..Graph::default()
        },
        &before,
        &impact_provenance("pto-spec"),
    )
    .unwrap();
    build_index(
        &Graph {
            nodes: vec![impact_node("pto-spec", target, Some("ExecuteTileLoadV2"))],
            ..Graph::default()
        },
        &after,
        &impact_provenance("pto-spec"),
    )
    .unwrap();
    let consumer_node = impact_node("davincioo", "DAV-ISA-TLOAD", None);
    build_index(
        &Graph {
            edges: vec![
                Edge::new(
                    consumer_node.id.clone(),
                    NodeId::new("pto-spec", target).unwrap(),
                    "conforms-to",
                    None,
                )
                .unwrap(),
            ],
            nodes: vec![consumer_node],
            diagnostics: Vec::new(),
        },
        &consumer,
        &impact_provenance("davincioo"),
    )
    .unwrap();
    (before, after, consumer)
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
fn query_command_filters_pto_state_attributes() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("state.sqlite");
    let mut state = impact_node("pto-spec", "PTO-STATE-TILE-SHARED", None);
    state
        .attributes
        .insert("pto_entity".to_owned(), "state".to_owned());
    build_index(
        &Graph {
            nodes: vec![state],
            ..Graph::default()
        },
        &output,
        &impact_provenance("pto-spec"),
    )
    .unwrap();
    let index = output.to_string_lossy();

    let (status, payload) = ndf(
        directory.path(),
        &[
            "query",
            "--index",
            &index,
            "--expression",
            "kind=architecture and attributes.pto_entity=state",
            "--format",
            "json",
        ],
    );

    assert_envelope(status, &payload, "query");
    assert_eq!(payload["data"].as_array().unwrap().len(), 1);
    assert_eq!(
        payload["data"][0]["id"],
        "ndf://pto-spec/PTO-STATE-TILE-SHARED"
    );
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

#[test]
fn pto_release_impact_emits_a_deterministic_versioned_envelope() {
    let directory = tempfile::tempdir().unwrap();
    let (before, after, consumer) = impact_indexes(directory.path());
    let arguments = [
        "impact",
        "pto-release",
        "--before",
        before.to_str().unwrap(),
        "--after",
        after.to_str().unwrap(),
        "--consumer-index",
        consumer.to_str().unwrap(),
        "--format",
        "json",
    ];

    let (first_status, first) = ndf(directory.path(), &arguments);
    let (second_status, second) = ndf(directory.path(), &arguments);

    assert_envelope(first_status, &first, "impact pto-release");
    assert_eq!(second_status, 0);
    assert_eq!(first, second);
    assert_eq!(first["data"]["schema_version"], "1");
    assert_eq!(first["data"]["compatibility"], "breaking");
}

#[test]
fn checked_in_cli_smoke_fixture_is_self_contained() {
    let root = repository().join("tests/fixtures/cli-smoke/project");
    let root_argument = root.to_string_lossy();

    let (status, payload) = ndf(
        &root,
        &["check", "--root", &root_argument, "--format", "json"],
    );

    assert_envelope(status, &payload, "check");
    assert_eq!(payload["data"]["node_count"], 2);
}

fn raw(cwd: &Path, binary: &Path, arguments: &[&str], input: &str) -> (i32, String, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(binary)
        .args(arguments)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn diagram_graph_format_prints_the_bare_document_for_a_diagram_consumer() {
    let root = repository().join("tests/fixtures/cli-smoke/project");
    let file = root.join("docs/smoke.md");

    let (status, stdout, _) = raw(
        &root,
        assert_cmd::cargo::cargo_bin!("ndf"),
        &["diagram", file.to_str().unwrap(), "--format", "graph"],
        "",
    );

    assert_eq!(status, 0);
    let document: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(document["version"], "1.0");
    assert!(document.get("schema_version").is_none(), "no envelope");
    assert_eq!(document["graph"]["id"], "wf:DOC-CLI-SMOKE");
    assert_eq!(document["graph"]["meta"]["workflow"], "DOC-CLI-SMOKE");
    let node = &document["graph"]["nodes"][0];
    assert_eq!(node["id"], "node:scope:root:SMOKE-001");
    assert_eq!(node["kind"], "requirement");
    assert_eq!(node["label"], "Self-contained contract");
    assert_eq!(node["ports"][0]["id"], "port:node:scope:root:SMOKE-001:in");
    assert_eq!(node["ports"][1]["direction"], "out");
    assert_eq!(
        node["meta"]["definitionAnnotations"][0]["name"],
        "requirement"
    );
    assert_eq!(node["meta"]["source"]["line"], 10);
    assert!(
        node["meta"]["source"]["file"]
            .as_str()
            .unwrap()
            .ends_with("docs/smoke.md")
    );
}

#[test]
fn diagram_json_format_wraps_the_document_in_the_envelope() {
    let root = repository().join("tests/fixtures/cli-smoke/project");
    let file = root.join("docs/smoke.md");

    let (status, payload) = ndf(&root, &["diagram", file.to_str().unwrap()]);

    assert_envelope(status, &payload, "diagram");
    assert_eq!(payload["data"]["version"], "1.0");
    assert_eq!(
        payload["data"]["graph"]["nodes"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn diagram_refuses_a_file_outside_every_project_with_exit_two() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("loose.md");
    fs::write(&file, "# Loose\n").unwrap();

    let (status, stdout, stderr) = raw(
        directory.path(),
        assert_cmd::cargo::cargo_bin!("ndf"),
        &["diagram", file.to_str().unwrap(), "--format", "graph"],
        "",
    );

    assert_eq!(status, 2);
    assert_eq!(stdout, "");
    assert!(stderr.contains("not inside an NDF project"), "{stderr}");
}

#[test]
fn diagram_resolves_a_locked_cross_project_target_into_its_checkout() {
    let directory = tempfile::tempdir().unwrap();
    graph_command_fixture(directory.path());
    let file = directory.path().join("docs/pipeline.md");

    let (status, stdout, _) = raw(
        directory.path(),
        assert_cmd::cargo::cargo_bin!("ndf"),
        &["diagram", file.to_str().unwrap(), "--format", "graph"],
        "",
    );

    assert_eq!(status, 0);
    let document: Value = serde_json::from_str(&stdout).unwrap();
    let nodes = document["graph"]["nodes"].as_array().unwrap();
    // The target is a clause of the dependency's graph: drawn as the clause
    // it is, external to the document, with its source in the locked
    // checkout and a drill-down into the file that holds it.
    let target = nodes
        .iter()
        .find(|node| node["id"] == "node:scope:root:pto-spec.PTO-INST-TILE-TLOAD")
        .unwrap();
    assert_ne!(target["kind"], "blackbox");
    assert_eq!(target["meta"]["external"], true);
    assert_eq!(target["meta"]["networkInstance"], true);
    assert_eq!(target["meta"]["referencedEntityName"], "contracts");
    let checkout = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("deps/pto-spec/asl/contracts.asl");
    assert_eq!(
        target["meta"]["referencedSource"]["file"],
        checkout.to_string_lossy().as_ref()
    );
    assert!(
        target["meta"]["badges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|badge| badge["text"] == "pto-spec"),
        "{}",
        target["meta"]["badges"]
    );
    assert!(
        target["meta"]["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["label"] == "Project" && fact["value"] == "pto-spec")
    );
    let conforming = nodes
        .iter()
        .find(|node| node["id"] == "node:scope:root:PIPE-PTO-001")
        .unwrap();
    assert!(
        conforming["meta"].get("isErrored").is_none(),
        "{conforming}"
    );
    assert_eq!(document["graph"]["edges"].as_array().unwrap().len(), 3);

    // The project overview draws the dependency as one blackbox with its
    // checkout, and the edge into it added up.
    let manifest = directory.path().join("ndf.yaml");
    let (status, stdout, _) = raw(
        directory.path(),
        assert_cmd::cargo::cargo_bin!("ndf"),
        &["diagram", manifest.to_str().unwrap(), "--format", "graph"],
        "",
    );
    assert_eq!(status, 0);
    let overview: Value = serde_json::from_str(&stdout).unwrap();
    let project = overview["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "node:scope:root:pto-spec")
        .unwrap();
    assert_eq!(project["kind"], "blackbox");
    assert!(
        project["meta"]["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["label"] == "Checkout")
    );
    let edges = overview["graph"]["edges"].as_array().unwrap();
    assert!(edges.iter().any(|edge| {
        edge["to"] == "port:node:scope:root:pto-spec:in" && edge["meta"]["role"] == "conforms-to"
    }));
}

#[test]
fn sidecar_answers_capabilities_export_and_refuses_unknown_ops() {
    let root = repository().join("tests/fixtures/cli-smoke/project");
    let file = root.join("docs/smoke.md");
    let input = format!(
        "{{\"op\":\"ndf.getCapabilities\"}}\n{{\"file\":{file},\"op\":\"ndf.exportGraph\",\"args\":{{}}}}\n{{\"file\":{file},\"op\":\"ndf.runWorkflow\",\"args\":{{}}}}\nnot json\n",
        file = serde_json::to_string(file.to_str().unwrap()).unwrap()
    );

    let (status, stdout, _) = raw(
        &root,
        assert_cmd::cargo::cargo_bin!("ndf-sidecar"),
        &[],
        &input,
    );

    assert_eq!(status, 0);
    let responses: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 4);

    let capabilities = &responses[0];
    assert_eq!(capabilities["status"], "ok");
    assert_eq!(capabilities["diagnostic"]["protocolVersion"], 2);
    let ops = capabilities["diagnostic"]["supportedOps"]
        .as_array()
        .unwrap();
    for op in [
        "getCapabilities",
        "exportGraph",
        "listTaskTypes",
        "listInstanceNames",
        "createNode",
        "renameNode",
        "deleteNode",
        "connect",
        "deleteEdge",
        "updateNodeParameter",
    ] {
        assert!(ops.contains(&Value::String(op.to_owned())), "{op}");
    }
    assert_eq!(
        capabilities["diagnostic"]["features"]["sourceTruthEdits"],
        true
    );
    assert_eq!(
        capabilities["diagnostic"]["features"]["expectedRevision"],
        true
    );
    let roles = capabilities["diagnostic"]["edgeRoles"].as_array().unwrap();
    assert_eq!(roles[0]["id"], "refines");
    assert!(roles.iter().any(|role| role["id"] == "conforms-to"));

    let export = &responses[1];
    assert_eq!(export["status"], "ok");
    assert_eq!(export["revision"].as_str().unwrap().len(), 64);
    assert_eq!(export["diagnostic"]["partial"], false);
    assert_eq!(export["diagnostic"]["graph"]["network"], "DOC-CLI-SMOKE");
    let node = &export["diagnostic"]["graph"]["nodes"][0];
    assert_eq!(node["id"], "node:scope:root:SMOKE-001");
    assert!(node.get("metadata").is_some());

    assert_eq!(responses[2]["status"], "error");
    assert_eq!(responses[2]["diagnostic"]["code"], "unsupported_op");
    assert_eq!(responses[3]["diagnostic"]["code"], "parse_error");
}

#[test]
fn diagram_groups_clauses_by_domain_on_request() {
    let root = repository().join("tests/fixtures/diagram/project");
    let file = root.join("docs/forwarding.md");

    let (status, stdout, _) = raw(
        &root,
        assert_cmd::cargo::cargo_bin!("ndf"),
        &[
            "diagram",
            file.to_str().unwrap(),
            "--format",
            "graph",
            "--group",
            "domain",
        ],
        "",
    );

    assert_eq!(status, 0);
    let document: Value = serde_json::from_str(&stdout).unwrap();
    let scopes = document["graph"]["subgraphs"].as_array().unwrap();
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0]["kind"], "group");
    assert_eq!(scopes[0]["label"], "forwarding");
    assert_eq!(scopes[0]["nodes"].as_array().unwrap().len(), 5);
    let clause = document["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "node:scope:root:FWD-ADM-001")
        .unwrap();
    assert_eq!(clause["scope"], "scope:group:forwarding");
    assert_eq!(clause["meta"]["badges"][0]["text"], "MUST");
}

#[test]
fn diagram_draws_a_named_unit_and_its_trace_view() {
    let root = repository().join("tests/fixtures/diagram/project");
    let file = root.join("docs/forwarding.md");
    let (status, stdout, _) = raw(
        &root,
        assert_cmd::cargo::cargo_bin!("ndf"),
        &[
            "diagram",
            file.to_str().unwrap(),
            "--format",
            "graph",
            "--workflow",
            "FWD-ADM-001",
            "--view",
            "refines",
            "--group",
            "refinement",
        ],
        "",
    );
    assert_eq!(status, 0);
    let document: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(document["graph"]["id"], "wf:FWD-ADM-001");
    assert_eq!(document["graph"]["meta"]["view"], "refines");
    let ids: Vec<&str> = document["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        ["node:scope:root:FWD-ADM-001", "node:scope:root:FWD-ADM-000"]
    );
    // Grouping applies to a trace too.
    let labels: Vec<&str> = document["graph"]["subgraphs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scope| scope["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["L0", "L1"]);
    // The outline rides on every unit.
    assert_eq!(document["hierarchy"]["workflowName"], "diagram");
    assert_eq!(
        document["hierarchy"]["children"].as_array().unwrap().len(),
        2
    );

    // The sidecar takes the same words as arguments of the export op.
    let request = serde_json::json!({
        "file": file.to_str().unwrap(),
        "op": "ndf.exportGraph",
        "args": { "workflow": "DOC-FORWARDING", "view": "depends-on" }
    });
    let (status, stdout, _) = raw(
        &root,
        assert_cmd::cargo::cargo_bin!("ndf-sidecar"),
        &[],
        &format!("{request}\n"),
    );
    assert_eq!(status, 0);
    let response: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(response["status"], "ok");
    assert_eq!(response["diagnostic"]["graph"]["network"], "DOC-FORWARDING");
    assert_eq!(
        response["diagnostic"]["graph"]["metadata"]["view"],
        "depends-on"
    );
    assert!(
        response["diagnostic"]["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["id"] == "node:scope:root:PRM-BUF-001")
    );

    // A unit the project does not have is the one hard failure.
    let (status, _, stderr) = raw(
        &root,
        assert_cmd::cargo::cargo_bin!("ndf"),
        &[
            "diagram",
            file.to_str().unwrap(),
            "--format",
            "graph",
            "--workflow",
            "NOPE",
        ],
        "",
    );
    assert_eq!(status, 2);
    assert!(
        stderr.contains("no document or clause named NOPE"),
        "{stderr}"
    );
}

/// One sidecar request, its parsed response.
fn sidecar(root: &Path, request: Value) -> Value {
    let (status, stdout, stderr) = raw(
        root,
        assert_cmd::cargo::cargo_bin!("ndf-sidecar"),
        &[],
        &format!("{request}\n"),
    );
    assert_eq!(status, 0, "{stderr}");
    serde_json::from_str(stdout.trim()).unwrap()
}

#[test]
fn sidecar_mutations_round_trip_on_a_copy_of_the_fixture() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let source = repository().join("tests/fixtures/diagram/project");
    fs::create_dir_all(root.join("docs")).unwrap();
    for name in [
        "ndf.yaml",
        "ndf.lock",
        "docs/forwarding.md",
        "docs/options.md",
    ] {
        fs::copy(source.join(name), root.join(name)).unwrap();
    }
    let file = root.join("docs/forwarding.md");
    let file_text = file.to_str().unwrap();
    let original = fs::read_to_string(&file).unwrap();
    let request = |op: &str, args: Value| serde_json::json!({ "file": file_text, "op": format!("ndf.{op}"), "args": args });

    // The listings the palette asks for.
    let names = sidecar(
        root,
        request(
            "listInstanceNames",
            serde_json::json!({ "workflow": "DOC-FORWARDING" }),
        ),
    );
    assert_eq!(names["diagnostic"]["names"][0], "Forwarding intent");
    let types = sidecar(root, request("listTaskTypes", serde_json::json!({})));
    assert_eq!(types["diagnostic"]["types"].as_array().unwrap().len(), 13);
    let instances = sidecar(
        root,
        request("listWorkflowInstances", serde_json::json!({})),
    );
    assert_eq!(
        instances["diagnostic"]["candidates"][0]["name"],
        "DOC-FORWARDING"
    );

    // A clause: the next id of the prefix the document uses, tbd and draft.
    let created = sidecar(
        root,
        request(
            "createNode",
            serde_json::json!({
                "workflow": "DOC-FORWARDING", "type": "requirement", "name": "Admission latency", "params": { "refinement": "L1" }
            }),
        ),
    );
    assert_eq!(created["status"], "ok", "{created}");
    assert_eq!(created["diagnostic"]["id"], "FWD-ADM-003");
    assert_eq!(created["revision"].as_str().unwrap().len(), 64);
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.ends_with(
        "## Admission latency {#FWD-ADM-003}\n<!-- ndf: kind=requirement modality=tbd refinement=L1 status=draft -->\n\n"
    ), "{text}");

    // Edges: by title, the role named; the default role when none is.
    let connected = sidecar(
        root,
        request(
            "connect",
            serde_json::json!({
                "workflow": "DOC-FORWARDING",
                "from_port": { "type": "actor", "actor": "Admission latency", "port": "out" },
                "to_port": { "type": "actor", "actor": "Forwarding intent", "port": "in" },
                "role": "refines"
            }),
        ),
    );
    assert_eq!(connected["status"], "ok", "{connected}");
    assert_eq!(connected["diagnostic"]["target"], "FWD-ADM-000");
    let by_default = sidecar(
        root,
        request(
            "connect",
            serde_json::json!({
                "from_port": "Admission latency.out", "to_port": "Admission buffer depth.in"
            }),
        ),
    );
    assert_eq!(by_default["diagnostic"]["role"], "references");
    assert_eq!(by_default["diagnostic"]["target"], "PRM-BUF-001");
    let twice = sidecar(
        root,
        request(
            "connect",
            serde_json::json!({
                "from_port": "FWD-ADM-003.out", "to_port": "FWD-ADM-000.in", "role": "refines"
            }),
        ),
    );
    assert_eq!(twice["diagnostic"]["code"], "edge_exists");

    // A field; a value the parser refuses leaves the file as it was.
    let set = sidecar(
        root,
        request(
            "updateNodeParameter",
            serde_json::json!({
                "entity": "Admission latency", "parameterName": "modality", "newValue": "must"
            }),
        ),
    );
    assert_eq!(set["status"], "ok", "{set}");
    let before_refusal = fs::read_to_string(&file).unwrap();
    let refused = sidecar(
        root,
        request(
            "updateNodeParameter",
            serde_json::json!({
                "entity": "Admission latency", "parameterName": "kind", "newValue": "bogus"
            }),
        ),
    );
    assert_eq!(refused["status"], "error");
    assert_eq!(refused["diagnostic"]["code"], "invalid_result");
    assert_eq!(fs::read_to_string(&file).unwrap(), before_refusal);
    assert!(before_refusal.contains(
        "<!-- ndf: kind=requirement modality=must references=PRM-BUF-001 refinement=L1 refines=FWD-ADM-000 status=draft -->"
    ), "{before_refusal}");

    // A title; the id never changes.
    let renamed = sidecar(
        root,
        request(
            "renameNode",
            serde_json::json!({
                "old": "Admission latency", "new": "Admission latency bound"
            }),
        ),
    );
    assert_eq!(renamed["diagnostic"]["id"], "FWD-ADM-003");
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("## Admission latency bound {#FWD-ADM-003}")
    );

    // A clause with dependents stays; the edges go first, by name or by id,
    // the dangling one too.
    let held = sidecar(
        root,
        request(
            "deleteNode",
            serde_json::json!({ "name": "Forwarding intent" }),
        ),
    );
    assert_eq!(held["diagnostic"]["code"], "has_dependents");
    assert_eq!(
        held["diagnostic"]["dependents"],
        serde_json::json!(["FWD-ADM-001 (refines)", "FWD-ADM-003 (refines)"])
    );
    let removed = sidecar(
        root,
        request(
            "deleteEdge",
            serde_json::json!({
                "from_expr": "Admission latency bound.out", "to_expr": "Forwarding intent.in", "role": "refines"
            }),
        ),
    );
    assert_eq!(removed["status"], "ok", "{removed}");
    let dangling = sidecar(
        root,
        request(
            "deleteEdge",
            serde_json::json!({
                "from_expr": "Admission under pressure.out", "to_expr": "MISSING-001.in"
            }),
        ),
    );
    assert_eq!(dangling["status"], "ok", "{dangling}");
    assert!(
        !fs::read_to_string(&file)
            .unwrap()
            .contains("depends-on=MISSING-001")
    );
    let missing = sidecar(
        root,
        request(
            "deleteEdge",
            serde_json::json!({
                "from_expr": "Admission under pressure.out", "to_expr": "MISSING-001.in"
            }),
        ),
    );
    assert_eq!(missing["diagnostic"]["code"], "edge_not_found");

    // A stale revision is refused; the right one goes through.
    let stale = sidecar(
        root,
        request(
            "deleteNode",
            serde_json::json!({ "name": "FWD-ADM-003", "expectedRevision": "0000" }),
        ),
    );
    assert_eq!(
        stale["diagnostic"]["code"],
        "concurrent_source_modification"
    );
    assert_eq!(stale["diagnostic"]["expectedRevision"], "0000");
    let revision = stale["revision"].as_str().unwrap().to_owned();
    let deleted = sidecar(
        root,
        request(
            "deleteNode",
            serde_json::json!({ "name": "FWD-ADM-003", "expectedRevision": revision }),
        ),
    );
    assert_eq!(deleted["status"], "ok", "{deleted}");
    let unknown = sidecar(
        root,
        request("deleteNode", serde_json::json!({ "name": "Nobody" })),
    );
    assert_eq!(unknown["diagnostic"]["code"], "node_not_found");

    // What is left is the original with the dangling edge gone (its line in
    // the canonical order an edit writes), and the project checks clean: the
    // clause that was added and removed left no trace.
    let final_text = fs::read_to_string(&file).unwrap();
    let expected: String = original
        .split_inclusive('\n')
        .map(|line| {
            if !line.contains("depends-on=MISSING-001") {
                return line.to_owned();
            }
            let inner = line
                .trim()
                .trim_start_matches("<!-- ndf:")
                .trim_end_matches("-->");
            let mut tokens: Vec<&str> = inner
                .split_whitespace()
                .filter(|token| *token != "depends-on=MISSING-001")
                .collect();
            tokens.sort_unstable();
            format!("<!-- ndf: {} -->\n", tokens.join(" "))
        })
        .collect();
    assert_eq!(final_text, expected);
    let (status, payload) = ndf(root, &["check", "--root", root.to_str().unwrap()]);
    assert_eq!(status, 0, "{payload}");
}
