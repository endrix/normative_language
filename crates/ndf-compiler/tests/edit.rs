use std::path::{Path, PathBuf};

use ndf_compiler::edit::{
    CLAUSE_KINDS, Document, EDGE_ROLES, add_target, allocate_clause_id, dependents, endpoint_name,
    new_clause_fields, remove_target, resolve_target, set_field, verify,
};
use ndf_compiler::project::load_project;
use serde_json::json;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const TEXT: &str = "---\ndoc_id: DOC-T\nstatus: active\nauthority: normative\nowner: core\n---\n\n# Title\n\n## First {#T-001}\n<!-- ndf: kind=requirement modality=must refinement=L1 status=active refines=T-000 -->\n\nBody one.\n\n```md\n## Not a clause {#T-999}\n```\n\n### Second {#T-002}\n<!-- ndf: kind=question status=open blocks=T-001 -->\n\nBody two.\n\n## Plain heading\n\nProse.\n";

#[test]
fn a_document_locates_its_clauses_outside_code_fences() {
    let document = Document::parse(TEXT);
    #[allow(clippy::type_complexity)]
    let spans: Vec<(&str, &str, usize, usize, Option<usize>, usize)> = document
        .clauses
        .iter()
        .map(|span| {
            (
                span.local_id.as_str(),
                span.title.as_str(),
                span.level,
                span.heading_line,
                span.metadata_line,
                span.end_line,
            )
        })
        .collect();
    assert_eq!(
        spans,
        [
            ("T-001", "First", 2, 9, Some(10), 18),
            ("T-002", "Second", 3, 18, Some(19), 23)
        ]
    );
    assert_eq!(document.heading_level(), 2);
    assert_eq!(
        document.text(),
        TEXT,
        "parsing and printing is the identity"
    );
    assert_eq!(document.find("T-002").unwrap().title, "Second");
    assert_eq!(document.find("First").unwrap().local_id, "T-001");
    assert_eq!(document.find("Third").unwrap_err().code, "node_not_found");
}

#[test]
fn metadata_edits_keep_the_canonical_order_and_the_rest_of_the_line() {
    let document = Document::parse(TEXT);
    let span = document.find("T-001").unwrap().clone();
    let mut fields = document.metadata(&span);
    assert_eq!(fields[0], ("kind".to_owned(), "requirement".to_owned()));
    add_target(&mut fields, "verifies", "T-002").unwrap();
    add_target(&mut fields, "refines", "T-003").unwrap();
    assert_eq!(
        add_target(&mut fields, "refines", "T-003")
            .unwrap_err()
            .code,
        "edge_exists"
    );
    assert_eq!(
        add_target(&mut fields, "kind", "x").unwrap_err().code,
        "invalid_role"
    );
    set_field(&mut fields, "domain", "core").unwrap();
    set_field(&mut fields, "modality", "").unwrap();
    assert_eq!(
        set_field(&mut fields, "kind", "").unwrap_err().code,
        "invalid_value"
    );
    assert_eq!(
        set_field(&mut fields, "colour", "red").unwrap_err().code,
        "invalid_parameter"
    );
    let next = document.with_metadata(&span, &fields);
    let line = next.text().lines().nth(10).unwrap().to_owned();
    assert_eq!(
        line,
        "<!-- ndf: domain=core kind=requirement refinement=L1 refines=T-000,T-003 status=active verifies=T-002 -->"
    );

    let span = next.find("T-001").unwrap().clone();
    let mut fields = next.metadata(&span);
    remove_target(&mut fields, Some("refines"), "T-000").unwrap();
    remove_target(&mut fields, None, "T-002").unwrap();
    assert_eq!(
        remove_target(&mut fields, None, "T-002").unwrap_err().code,
        "edge_not_found"
    );
    let next = next.with_metadata(&span, &fields);
    assert_eq!(
        next.text().lines().nth(10).unwrap(),
        "<!-- ndf: domain=core kind=requirement refinement=L1 refines=T-003 status=active -->"
    );
}

#[test]
fn a_clause_is_appended_retitled_and_removed_without_a_trace() {
    let document = Document::parse(TEXT);
    let fields = new_clause_fields(
        "verification",
        &[("refinement".to_owned(), "L3".to_owned())],
    );
    assert_eq!(
        fields,
        [
            ("modality".to_owned(), "tbd".to_owned()),
            ("status".to_owned(), "draft".to_owned()),
            ("refinement".to_owned(), "L3".to_owned())
        ]
    );
    assert_eq!(
        new_clause_fields("definition", &[]),
        [("status".to_owned(), "draft".to_owned())]
    );
    let with = document
        .with_clause("verification", "  Third   check ", "T-003", &fields)
        .unwrap();
    assert!(with.text().ends_with(
        "Prose.\n\n## Third check {#T-003}\n<!-- ndf: kind=verification modality=tbd refinement=L3 status=draft -->\n\n"
    ));
    assert_eq!(with.clauses.len(), 3);
    assert_eq!(
        document
            .with_clause("verification", "Bad {#X}", "T-003", &fields)
            .unwrap_err()
            .code,
        "invalid_title"
    );

    let span = with.find("T-003").unwrap().clone();
    let renamed = with.retitled(&span, "Third check, renamed").unwrap();
    assert!(
        renamed
            .text()
            .contains("## Third check, renamed {#T-003}\n")
    );

    let span = renamed.find("T-003").unwrap().clone();
    let without = renamed.without(&span);
    assert_eq!(without.text(), TEXT, "append then remove is the identity");

    // Removing a clause in the middle keeps one blank line between its neighbours.
    let span = document.find("T-001").unwrap().clone();
    let middle = document.without(&span);
    assert!(
        middle.text().contains("# Title\n\n### Second {#T-002}\n"),
        "{}",
        middle.text()
    );
    assert!(
        !middle.text().contains("T-999"),
        "the fenced block went with the clause's body"
    );
}

#[test]
fn the_guard_refuses_a_result_that_parses_worse_and_allows_one_that_does_not() {
    let root = repository()
        .join("tests/fixtures/diagram/project")
        .canonicalize()
        .unwrap();
    let loaded = load_project(&root).unwrap();
    let before = std::fs::read_to_string(root.join("docs/forwarding.md")).unwrap();
    let worse = before.replace("kind=architecture", "kind=nonsense");
    let error = verify(&before, &worse, "docs/forwarding.md", &loaded.manifest).unwrap_err();
    assert_eq!(error.code, "invalid_result");
    assert!(error.message.contains("NDF-MD-004"), "{}", error.message);
    // The fixture's dangling edge is an error already; a result that keeps it is not worse.
    let same = before.replace("Frame admission", "Frame admission rule");
    verify(&before, &same, "docs/forwarding.md", &loaded.manifest).unwrap();
}

#[test]
fn ids_and_targets_resolve_across_the_project() {
    let root = repository()
        .join("tests/fixtures/diagram/project")
        .canonicalize()
        .unwrap();
    let loaded = load_project(&root).unwrap();
    let graph = &loaded.graph;
    let document =
        Document::parse(&std::fs::read_to_string(root.join("docs/forwarding.md")).unwrap());
    assert_eq!(
        allocate_clause_id(graph, &loaded.manifest, &document, None).unwrap(),
        "FWD-ADM-003"
    );
    assert_eq!(
        allocate_clause_id(graph, &loaded.manifest, &document, Some("VER-ADM")).unwrap(),
        "VER-ADM-002"
    );
    assert_eq!(
        allocate_clause_id(graph, &loaded.manifest, &document, Some("Q"))
            .unwrap_err()
            .code,
        "invalid_prefix"
    );
    let empty = Document::parse("");
    assert_eq!(
        allocate_clause_id(graph, &loaded.manifest, &empty, None).unwrap(),
        "FWD-001",
        "the manifest's first non-document prefix"
    );

    assert_eq!(
        resolve_target(graph, "diagram", "FWD-ADM-001").unwrap(),
        "FWD-ADM-001"
    );
    assert_eq!(
        resolve_target(graph, "diagram", "Admission buffer depth").unwrap(),
        "PRM-BUF-001"
    );
    assert_eq!(
        resolve_target(graph, "diagram", "pto-spec/PTO-001").unwrap(),
        "ndf://pto-spec/PTO-001"
    );
    assert_eq!(
        resolve_target(graph, "diagram", "ndf://diagram/FWD-ADM-000").unwrap(),
        "FWD-ADM-000"
    );
    assert_eq!(
        resolve_target(graph, "diagram", "MISSING-001").unwrap(),
        "MISSING-001",
        "a bare id passes verbatim"
    );
    assert_eq!(
        resolve_target(graph, "diagram", "no such title")
            .unwrap_err()
            .code,
        "node_not_found"
    );
    assert_eq!(
        dependents(graph, "diagram", "FWD-ADM-000"),
        ["FWD-ADM-001 (refines)"]
    );
    assert!(dependents(graph, "diagram", "VER-ADM-001").is_empty());

    assert_eq!(
        endpoint_name(&json!({ "type": "actor", "actor": "Frame admission", "port": "out" }))
            .as_deref(),
        Some("Frame admission")
    );
    assert_eq!(
        endpoint_name(&json!("Frame admission.out")).as_deref(),
        Some("Frame admission")
    );
    assert_eq!(endpoint_name(&json!("'in'")).as_deref(), Some("in"));
    assert_eq!(endpoint_name(&json!(3)), None);
    assert_eq!(CLAUSE_KINDS.len(), 13);
    assert_eq!(EDGE_ROLES.len(), 15);
}
