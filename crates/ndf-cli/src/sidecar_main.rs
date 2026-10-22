//! `ndf-sidecar`: the sidecar a Dialogram consumer spawns beside the `ndf` CLI.
//!
//! It speaks the sidecar protocol (`wf.sidecar-op/v1`): one JSON request per
//! line on stdin, `{ "file", "op", "args" }`, one JSON response per line on
//! stdout, `{ "status", "file", "revision", "message", "diagnostic" }`. This
//! release answers `getCapabilities` and the graph export, and advertises no
//! mutation, so a platform's edit handlers stay inert. Every other op is
//! refused with `unsupported_op`.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ndf_compiler::CompilerError;
use ndf_compiler::diagram::{
    DiagramRequest, GroupBy, build_diagram, export_payload, failure_document,
};
use ndf_compiler::project::{find_project_root, load_project};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: u32 = 2;
const PREFIX: &str = "ndf";
const SUPPORTED_OPS: &[&str] = &["getCapabilities", "exportGraph"];
const EXPORT_ALIASES: &[&str] = &["exportGraph", "exportWorkflowGraph", "exportNetworkGraph"];

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
                    "sourceTruthEdits": false,
                    "expectedRevision": false,
                    "checkConnection": false,
                },
                "contracts": { "sidecarOperation": "wf.sidecar-op/v1" },
            }
        });
    }
    let Some(file) = request
        .get("file")
        .and_then(Value::as_str)
        .filter(|file| !file.trim().is_empty())
    else {
        return error_response("", "", "missing 'file'", "parse_error");
    };
    let path = PathBuf::from(file);
    let revision = revision_of(&path);
    if EXPORT_ALIASES.contains(&bare) {
        // `args.group`, `args.workflow` and `args.view`, as a consumer's
        // export op passes them: the same words as the CLI's `--group`,
        // `--workflow` and `--view`; `none` for a group absent or unknown.
        let argument = |name: &str| {
            request
                .get("args")
                .and_then(|args| args.get(name))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
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
            Err(error) => error_response(file, &revision, &error.to_string(), "op_failed"),
        };
    }
    error_response(
        file,
        &revision,
        &format!("unsupported operation: {op}"),
        "unsupported_op",
    )
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

fn revision_of(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        Err(_) => String::new(),
    }
}

fn error_response(file: &str, revision: &str, message: &str, code: &str) -> Value {
    json!({
        "status": "error",
        "file": file,
        "revision": revision,
        "message": message,
        "diagnostic": { "code": code }
    })
}
