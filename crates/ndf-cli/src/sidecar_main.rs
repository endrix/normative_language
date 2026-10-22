//! `ndf-sidecar`: the sidecar a Dialogram consumer spawns beside the `ndf` CLI.
//!
//! It speaks the sidecar protocol (`wf.sidecar-op/v1`): one JSON request per
//! line on stdin, `{ "file", "op", "args" }`, one JSON response per line on
//! stdout, `{ "status", "file", "revision", "message", "diagnostic" }`.
//!
//! Queries: `getCapabilities`, the graph export (`exportGraph`, with
//! `args.group`, `args.workflow`, `args.view`), `listTaskTypes` (the clause
//! kinds), `listWorkflowTypes` and `listWorkflowInstances` (the documents),
//! `listInstanceNames` (the titles of the document's clauses).
//!
//! Mutations, each an edit to the text of the document that owns `file`:
//! `createNode` (a clause of `type` titled `name`, its id allocated),
//! `renameNode` (the title; the id never changes), `deleteNode` (refused
//! while any edge of the project targets the clause), `connect` and
//! `deleteEdge` (an entry of `role` in the source clause's metadata),
//! `updateNodeParameter` (a canonical or attribute field). A mutation with
//! `args.expectedRevision` is refused when the file's revision differs
//! (`concurrent_source_modification`). Every result is re-parsed before it is
//! written and refused when it parses worse than the original
//! (`invalid_result`), the file untouched. Every other op is refused with
//! `unsupported_op`.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ndf_compiler::CompilerError;
use ndf_compiler::diagram::{
    DiagramRequest, GroupBy, build_diagram, document_relative_path, export_payload,
    failure_document,
};
use ndf_compiler::edit::{
    CLAUSE_KINDS, DEFAULT_EDGE_ROLE, Document, EditError, add_target, allocate_clause_id,
    dependents, edge_roles_value, endpoint_name, new_clause_fields, remove_target, resolve_target,
    set_field, verify,
};
use ndf_compiler::project::{LoadedProject, find_project_root, load_project};
use ndf_core::model::NodeKind;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: u32 = 2;
const PREFIX: &str = "ndf";
const SUPPORTED_OPS: &[&str] = &[
    "getCapabilities",
    "exportGraph",
    "listTaskTypes",
    "listWorkflowTypes",
    "listWorkflowInstances",
    "listInstanceNames",
    "createNode",
    "renameNode",
    "deleteNode",
    "connect",
    "deleteEdge",
    "updateNodeParameter",
];
const EXPORT_ALIASES: &[&str] = &["exportGraph", "exportWorkflowGraph", "exportNetworkGraph"];
const MUTATIONS: &[&str] = &[
    "createNode",
    "renameNode",
    "deleteNode",
    "connect",
    "deleteEdge",
    "updateNodeParameter",
];

fn main() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(&request),
            Err(error) => error_response(
                "",
                "",
                &format!("invalid JSON request: {error}"),
                "parse_error",
                Value::Null,
            ),
        };
        if writeln!(out, "{response}")
            .and_then(|()| out.flush())
            .is_err()
        {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

fn handle(request: &Value) -> Value {
    let op = request.get("op").and_then(Value::as_str).unwrap_or("");
    let bare = op.strip_prefix(&format!("{PREFIX}.")).unwrap_or(op);
    let bare = bare.rsplit('.').next().unwrap_or(bare);
    if bare == "getCapabilities" || bare == "protocolContracts" {
        return json!({
            "status": "ok",
            "file": "",
            "revision": "",
            "message": "ok",
            "diagnostic": {
                "protocolVersion": PROTOCOL_VERSION,
                "supportedOps": SUPPORTED_OPS,
                "features": {
                    "stableIds": true,
                    "sourceLocations": true,
                    "partialGraph": true,
                    "sourceTruthEdits": true,
                    "expectedRevision": true,
                    "checkConnection": false,
                },
                "edgeRoles": edge_roles_value(),
                "contracts": { "sidecarOperation": "wf.sidecar-op/v1" },
            }
        });
    }
    let Some(file) = request
        .get("file")
        .and_then(Value::as_str)
        .filter(|file| !file.trim().is_empty())
    else {
        return error_response("", "", "missing 'file'", "parse_error", Value::Null);
    };
    let path = PathBuf::from(file);
    let revision = revision_of(&path);
    let empty = Map::new();
    let args = request
        .get("args")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let argument = |name: &str| {
        args.get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };

    if EXPORT_ALIASES.contains(&bare) {
        // `args.group`, `args.workflow` and `args.view`, as a consumer's
        // export op passes them: the same words as the CLI's `--group`,
        // `--workflow` and `--view`; `none` for a group absent or unknown.
        let group = argument("group").unwrap_or("none");
        return match export(&path, group, argument("workflow"), argument("view")) {
            Ok((document, partial, errors)) => json!({
                "status": "ok",
                "file": file,
                "revision": revision,
                "message": "ok",
                "diagnostic": {
                    "graph": export_payload(&document),
                    "partial": partial,
                    "errors": errors,
                    "diagnostics": document.diagnostics,
                }
            }),
            Err(error) => error_response(
                file,
                &revision,
                &error.to_string(),
                "op_failed",
                Value::Null,
            ),
        };
    }

    let outcome = match bare {
        "listTaskTypes" => Ok(json!({ "types": CLAUSE_KINDS })),
        "listWorkflowTypes" | "listWorkflowInstances" | "listInstanceNames" => {
            with_project(&path, |project, relative| {
                Ok(list(bare, project, relative, file))
            })
        }
        _ if MUTATIONS.contains(&bare) => {
            if let Some(expected) = args
                .get("expectedRevision")
                .and_then(Value::as_str)
                .filter(|expected| !expected.is_empty())
                && expected != revision
            {
                return error_response(
                    file,
                    &revision,
                    "source changed since expectedRevision",
                    "concurrent_source_modification",
                    json!({ "expectedRevision": expected, "actualRevision": revision }),
                );
            }
            mutate(&path, bare, args)
        }
        _ => {
            return error_response(
                file,
                &revision,
                &format!("unsupported operation: {op}"),
                "unsupported_op",
                Value::Null,
            );
        }
    };
    match outcome {
        Ok(diagnostic) => json!({
            "status": "ok",
            "file": file,
            "revision": revision_of(&path),
            "message": "ok",
            "diagnostic": diagnostic,
        }),
        Err(error) => error_response(
            file,
            &revision_of(&path),
            &error.message,
            error.code,
            error.details,
        ),
    }
}

type Export = (
    ndf_compiler::diagram::GraphDocument,
    bool,
    Vec<ndf_compiler::diagram::LoadError>,
);

fn export(
    file: &Path,
    group: &str,
    workflow: Option<&str>,
    view: Option<&str>,
) -> Result<Export, CompilerError> {
    let root = find_project_root(file).ok_or_else(|| {
        CompilerError::Contract(format!(
            "{} is not inside an NDF project (no ndf.yaml in any parent directory)",
            file.display()
        ))
    })?;
    let loaded = match load_project(&root) {
        Ok(loaded) => loaded,
        Err(error) => {
            let document = failure_document(file, error.to_string());
            let errors = document.errors.clone();
            return Ok((document, true, errors));
        }
    };
    let document = build_diagram(&DiagramRequest {
        project_root: &root,
        manifest: &loaded.manifest,
        graph: &loaded.graph,
        dependencies: &loaded.dependencies,
        file,
        group: GroupBy::parse(group).unwrap_or_default(),
        workflow,
        view,
    })?;
    Ok((document, false, Vec::new()))
}

/// The project that owns `file`, loaded, and the file's path within it.
fn with_project<T>(
    file: &Path,
    body: impl FnOnce(&LoadedProject, &str) -> Result<T, EditError>,
) -> Result<T, EditError> {
    let root = find_project_root(file).ok_or_else(|| EditError {
        code: "op_failed",
        message: format!(
            "{} is not inside an NDF project (no ndf.yaml in any parent directory)",
            file.display()
        ),
        details: Value::Null,
    })?;
    let loaded = load_project(&root).map_err(|error| EditError {
        code: "op_failed",
        message: error.to_string(),
        details: Value::Null,
    })?;
    let relative = document_relative_path(&root, file).map_err(|error| EditError {
        code: "op_failed",
        message: error.to_string(),
        details: Value::Null,
    })?;
    body(&loaded, &relative)
}

/// The listings a consumer's node creation asks for.
fn list(op: &str, project: &LoadedProject, relative: &str, file: &str) -> Value {
    let manifest = &project.manifest;
    let documents: Vec<&ndf_core::model::Node> = project
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Document)
        .collect();
    match op {
        "listWorkflowTypes" => json!({
            "types": documents.iter().map(|node| node.id.local_id()).collect::<Vec<_>>()
        }),
        "listWorkflowInstances" => {
            let candidates: Vec<Value> = documents
                .iter()
                .filter(|node| node.source.path == relative)
                .map(|node| json!({ "name": node.id.local_id(), "file": file }))
                .collect();
            json!({ "candidates": candidates })
        }
        _ => json!({
            "names": project
                .graph
                .nodes
                .iter()
                .filter(|node| {
                    node.kind != NodeKind::Document
                        && node.source.path == relative
                        && node.id.project_id() == manifest.project
                })
                .map(|node| node.title.as_str())
                .collect::<Vec<_>>()
        }),
    }
}

/// One mutation of the document at `file`: computed on its text, verified,
/// then written. The ok payload says what changed.
fn mutate(file: &Path, op: &str, args: &Map<String, Value>) -> Result<Value, EditError> {
    with_project(file, |project, relative| {
        let before = std::fs::read_to_string(file).map_err(|error| EditError {
            code: "op_failed",
            message: format!("failed to read {}: {error}", file.display()),
            details: Value::Null,
        })?;
        let document = Document::parse(&before);
        let text = |name: &str| {
            args.get(name)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
        let missing = |name: &str| EditError {
            code: "parse_error",
            message: format!("{op} needs args.{name}"),
            details: Value::Null,
        };
        let graph = &project.graph;
        let manifest = &project.manifest;

        let (next, outcome) = match op {
            "createNode" => {
                let kind = text("type").ok_or_else(|| missing("type"))?;
                if !CLAUSE_KINDS.contains(&kind) {
                    return Err(EditError {
                        code: "invalid_type",
                        message: format!("{kind:?} is not a clause kind"),
                        details: json!({ "types": CLAUSE_KINDS }),
                    });
                }
                let title = text("name").ok_or_else(|| missing("name"))?;
                let params = args.get("params").and_then(Value::as_object);
                let param = |name: &str| {
                    params
                        .and_then(|params| params.get(name))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                };
                let local_id = match param("id") {
                    Some(id) => id.to_owned(),
                    None => allocate_clause_id(graph, manifest, &document, param("prefix"))?,
                };
                if document
                    .clauses
                    .iter()
                    .any(|span| span.local_id == local_id)
                    || graph.nodes.iter().any(|node| {
                        node.id.project_id() == manifest.project && node.id.local_id() == local_id
                    })
                {
                    return Err(EditError {
                        code: "id_exists",
                        message: format!("the project already has a clause {local_id}"),
                        details: Value::Null,
                    });
                }
                let extra: Vec<(String, String)> =
                    ["modality", "refinement", "domain", "status", "owner"]
                        .iter()
                        .filter_map(|name| {
                            param(name).map(|value| ((*name).to_owned(), value.to_owned()))
                        })
                        .collect();
                let fields = new_clause_fields(kind, &extra);
                let next = document.with_clause(kind, title, &local_id, &fields)?;
                (
                    next,
                    json!({ "id": local_id, "kind": kind, "title": title }),
                )
            }
            "renameNode" => {
                let old = text("old").ok_or_else(|| missing("old"))?;
                let new = text("new").ok_or_else(|| missing("new"))?;
                let span = document.find(old)?.clone();
                (
                    document.retitled(&span, new)?,
                    json!({ "id": span.local_id, "title": new }),
                )
            }
            "deleteNode" => {
                let name = text("name").ok_or_else(|| missing("name"))?;
                let span = document.find(name)?.clone();
                let holders = dependents(graph, &manifest.project, &span.local_id);
                if !holders.is_empty() {
                    return Err(EditError {
                        code: "has_dependents",
                        message: format!(
                            "{} is the target of {} edge{}: {}. Remove them first.",
                            span.local_id,
                            holders.len(),
                            if holders.len() == 1 { "" } else { "s" },
                            holders.join(", ")
                        ),
                        details: json!({ "dependents": holders }),
                    });
                }
                (document.without(&span), json!({ "id": span.local_id }))
            }
            "connect" | "deleteEdge" => {
                let (from, to) = if op == "connect" {
                    (
                        args.get("from_port").or_else(|| args.get("source")),
                        args.get("to_port").or_else(|| args.get("target")),
                    )
                } else {
                    (
                        args.get("from_expr").or_else(|| args.get("source")),
                        args.get("to_expr").or_else(|| args.get("target")),
                    )
                };
                let from = from
                    .and_then(endpoint_name)
                    .ok_or_else(|| missing("from_port"))?;
                let to = to
                    .and_then(endpoint_name)
                    .ok_or_else(|| missing("to_port"))?;
                let span = document.find(&from)?.clone();
                let target = resolve_target(graph, &manifest.project, &to)?;
                let mut fields = document.metadata(&span);
                let role = text("role").or_else(|| text("type"));
                if op == "connect" {
                    let role = role.unwrap_or(DEFAULT_EDGE_ROLE);
                    add_target(&mut fields, role, &target)?;
                    (
                        document.with_metadata(&span, &fields),
                        json!({ "source": span.local_id, "target": target, "role": role }),
                    )
                } else {
                    remove_target(&mut fields, role, &target)?;
                    (
                        document.with_metadata(&span, &fields),
                        json!({ "source": span.local_id, "target": target }),
                    )
                }
            }
            "updateNodeParameter" => {
                let entity = text("entity")
                    .or_else(|| text("node"))
                    .or_else(|| text("name"))
                    .ok_or_else(|| missing("entity"))?;
                let name = text("parameterName")
                    .or_else(|| text("parameter"))
                    .ok_or_else(|| missing("parameterName"))?;
                let value = args
                    .get("newValue")
                    .or_else(|| args.get("value"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let span = document.find(entity)?.clone();
                let mut fields = document.metadata(&span);
                set_field(&mut fields, name, value)?;
                (
                    document.with_metadata(&span, &fields),
                    json!({ "id": span.local_id, "parameter": name, "value": value }),
                )
            }
            _ => unreachable!("mutations are dispatched by name"),
        };

        let after = next.text();
        verify(&before, &after, relative, manifest)?;
        std::fs::write(file, after).map_err(|error| EditError {
            code: "op_failed",
            message: format!("failed to write {}: {error}", file.display()),
            details: Value::Null,
        })?;
        Ok(outcome)
    })
}

fn revision_of(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        Err(_) => String::new(),
    }
}

fn error_response(file: &str, revision: &str, message: &str, code: &str, details: Value) -> Value {
    let mut diagnostic = match details {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    diagnostic.insert("code".to_owned(), Value::String(code.to_owned()));
    json!({
        "status": "error",
        "file": file,
        "revision": revision,
        "message": message,
        "diagnostic": Value::Object(diagnostic)
    })
}
