use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, LifecycleStatus, Node, NodeKind, ParseResult, SourceSpan,
};
use serde_json::Value;

use crate::CompilerError;
use crate::manifest::ProjectManifest;

const METADATA_PREFIX: &str = "// ndf: ";
const INSTRUCTION_PREFIX: &str = "// PTO-INSTRUCTION: ";
const LAYERS: &[&str] = &[
    "architecture",
    "scalar",
    "block",
    "tile",
    "state",
    "memory",
    "concurrency",
];

pub fn instruction_clause_id(surface: &str, mnemonic: &str) -> Result<String, CompilerError> {
    let surface = slug(surface);
    let mnemonic = slug(mnemonic);
    if surface.is_empty() || mnemonic.is_empty() {
        return Err(CompilerError::Contract(
            "instruction NDF identity requires surface and mnemonic".to_owned(),
        ));
    }
    Ok(format!("PTO-INST-{surface}-{mnemonic}"))
}

pub fn parse_pto_asl(path: &Path, project: &ProjectManifest) -> Result<ParseResult, CompilerError> {
    let text = fs::read_to_string(path).map_err(|source| CompilerError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    parse_pto_asl_text(&text, path, project)
}

pub fn parse_pto_asl_text(
    text: &str,
    source: &Path,
    project: &ProjectManifest,
) -> Result<ParseResult, CompilerError> {
    let path = relative_path(source, project)?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut diagnostics = Vec::new();
    let mut active_id: Option<String> = None;
    let mut active_line = 0;
    let mut metadata_line = 0;
    let mut metadata_raw: Option<String> = None;
    let mut body_lines = Vec::new();
    let mut region_invalid = false;

    for (index, line) in text.lines().enumerate() {
        let line_number = (index + 1) as u32;
        if active_id.is_none() && line.starts_with(INSTRUCTION_PREFIX) {
            match instruction_node(line, &path, line_number, project)? {
                Ok(node) => nodes.push(node),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
            continue;
        }

        let begin = marker(line, "// NDF-BEGIN: ");
        let end = marker(line, "// NDF-END: ");
        if let Some(begin) = begin {
            if let Some(active) = active_id.as_deref() {
                diagnostics.push(diagnostic(
                    "NDF-ASL-001",
                    format!("nested NDF region: {begin}"),
                    &path,
                    line_number,
                    "Close the active region before starting another.",
                    Some(node_id(project, active)?),
                ));
                region_invalid = true;
                continue;
            }
            active_id = Some(begin.to_owned());
            active_line = line_number;
            metadata_line = 0;
            metadata_raw = None;
            body_lines.clear();
            region_invalid = false;
            continue;
        }
        if let Some(end) = end {
            let Some(active) = active_id.as_deref() else {
                diagnostics.push(diagnostic(
                    "NDF-ASL-001",
                    format!("unmatched NDF end: {end}"),
                    &path,
                    line_number,
                    "Remove the end marker or add its begin marker.",
                    None,
                ));
                continue;
            };
            let subject = node_id(project, active)?;
            if end != active {
                diagnostics.push(diagnostic(
                    "NDF-ASL-001",
                    format!("mismatched NDF end {end} for {active}"),
                    &path,
                    line_number,
                    "Use the active clause ID in the end marker.",
                    Some(subject.clone()),
                ));
                region_invalid = true;
            }
            let values = if let Some(raw) = metadata_raw.as_deref() {
                let (values, mut metadata_diagnostics) =
                    parse_metadata(raw, &path, metadata_line, &subject);
                if !metadata_diagnostics.is_empty() {
                    region_invalid = true;
                }
                diagnostics.append(&mut metadata_diagnostics);
                values
            } else {
                diagnostics.push(diagnostic(
                    "NDF-ASL-002",
                    format!("NDF clause {active} has no metadata"),
                    &path,
                    active_line,
                    "Add the required PTO NDF metadata line.",
                    Some(subject.clone()),
                ));
                region_invalid = true;
                BTreeMap::new()
            };
            let body = body_lines.join("\n").trim().to_owned();
            if body.is_empty() {
                diagnostics.push(diagnostic(
                    "NDF-ASL-003",
                    format!("NDF clause {active} has an empty body"),
                    &path,
                    active_line,
                    "Add one or more ASL comment body lines.",
                    Some(subject.clone()),
                ));
                region_invalid = true;
            }
            if !region_invalid {
                let kind = &values["kind"];
                let node = Node {
                    id: subject,
                    kind: kind_contract(kind).expect("validated kind").1,
                    title: clause_title(active),
                    source: SourceSpan::new(&path, active_line).expect("positive ASL line"),
                    modality: None,
                    refinement: Some(values["level"].clone()),
                    domain: Some(values["layer"].clone()),
                    status: status(&values["status"]).expect("validated status"),
                    owner: Some("pto-spec".to_owned()),
                    body: body.clone(),
                    attributes: BTreeMap::from([("pto_kind".to_owned(), kind.clone())]),
                };
                for target in references(&body) {
                    edges.push(
                        Edge::new(
                            node.id.clone(),
                            node_id(project, target)?,
                            "references",
                            Some(node.source.clone()),
                        )
                        .expect("non-empty built-in edge type"),
                    );
                }
                nodes.push(node);
            }
            active_id = None;
            continue;
        }

        let Some(active) = active_id.as_deref() else {
            if line.starts_with("// NDF-BEGIN:") {
                diagnostics.push(diagnostic(
                    "NDF-ASL-001",
                    "invalid PTO NDF clause ID",
                    &path,
                    line_number,
                    "Use a PTO-prefixed uppercase clause ID.",
                    None,
                ));
            }
            continue;
        };
        if let Some(raw) = line.strip_prefix(METADATA_PREFIX) {
            if metadata_raw.is_some() {
                diagnostics.push(diagnostic(
                    "NDF-ASL-002",
                    "duplicate NDF metadata line",
                    &path,
                    line_number,
                    "Keep exactly one metadata line per region.",
                    Some(node_id(project, active)?),
                ));
                region_invalid = true;
            } else {
                metadata_raw = Some(raw.to_owned());
                metadata_line = line_number;
            }
        } else if let Some(comment) = line.strip_prefix("//") {
            body_lines.push(comment.strip_prefix(' ').unwrap_or(comment).to_owned());
        } else {
            diagnostics.push(diagnostic(
                "NDF-ASL-003",
                "NDF body line must be an ASL comment",
                &path,
                line_number,
                "Prefix normative body lines with //.",
                Some(node_id(project, active)?),
            ));
            region_invalid = true;
        }
    }

    if let Some(active) = active_id {
        diagnostics.push(diagnostic(
            "NDF-ASL-001",
            format!("unterminated NDF clause {active}"),
            &path,
            active_line,
            "Add the matching NDF-END marker.",
            Some(node_id(project, &active)?),
        ));
    }
    Ok(ParseResult {
        nodes,
        edges,
        diagnostics,
    })
}

fn instruction_node(
    line: &str,
    path: &str,
    line_number: u32,
    project: &ProjectManifest,
) -> Result<Result<Node, Diagnostic>, CompilerError> {
    let parsed: Result<Value, _> = serde_json::from_str(&line[INSTRUCTION_PREFIX.len()..]);
    let metadata = match parsed {
        Ok(Value::Object(metadata)) => metadata,
        Ok(_) => {
            return Ok(Err(diagnostic(
                "NDF-ASL-004",
                "invalid PTO instruction metadata: expected object",
                path,
                line_number,
                "Provide JSON string fields surface and mnemonic.",
                None,
            )));
        }
        Err(error) => {
            return Ok(Err(diagnostic(
                "NDF-ASL-004",
                format!("invalid PTO instruction metadata: {error}"),
                path,
                line_number,
                "Provide JSON string fields surface and mnemonic.",
                None,
            )));
        }
    };
    let (Some(surface), Some(mnemonic)) = (
        metadata.get("surface").and_then(Value::as_str),
        metadata.get("mnemonic").and_then(Value::as_str),
    ) else {
        return Ok(Err(diagnostic(
            "NDF-ASL-004",
            "invalid PTO instruction metadata: missing string surface or mnemonic",
            path,
            line_number,
            "Provide JSON string fields surface and mnemonic.",
            None,
        )));
    };
    Ok(Ok(Node {
        id: node_id(project, &instruction_clause_id(surface, mnemonic)?)?,
        kind: NodeKind::Definition,
        title: format!("{surface} {mnemonic} instruction"),
        source: SourceSpan::new(path, line_number).expect("positive ASL line"),
        modality: None,
        refinement: None,
        domain: Some(surface.to_lowercase()),
        status: LifecycleStatus::Active,
        owner: Some("pto-spec".to_owned()),
        body: String::new(),
        attributes: BTreeMap::from([
            ("surface".to_owned(), surface.to_owned()),
            ("mnemonic".to_owned(), mnemonic.to_owned()),
        ]),
    }))
}

fn parse_metadata(
    raw: &str,
    path: &str,
    line: u32,
    subject: &NodeId,
) -> (BTreeMap<String, String>, Vec<Diagnostic>) {
    let mut values = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for token in raw.split_whitespace() {
        let Some((name, value)) = token.split_once('=') else {
            diagnostics.push(diagnostic(
                "NDF-ASL-002",
                format!("invalid NDF metadata token: {token}"),
                path,
                line,
                "Write metadata as one key=value token per field.",
                Some(subject.clone()),
            ));
            continue;
        };
        if value.contains('=') {
            diagnostics.push(diagnostic(
                "NDF-ASL-002",
                format!("invalid NDF metadata token: {token}"),
                path,
                line,
                "Write metadata as one key=value token per field.",
                Some(subject.clone()),
            ));
            continue;
        }
        if values.insert(name.to_owned(), value.to_owned()).is_some() {
            diagnostics.push(diagnostic(
                "NDF-ASL-002",
                format!("duplicate NDF metadata field: {name}"),
                path,
                line,
                "Keep exactly one value for each metadata field.",
                Some(subject.clone()),
            ));
        }
    }
    for name in ["kind", "layer", "level", "status"] {
        if !values.contains_key(name) {
            diagnostics.push(diagnostic(
                "NDF-ASL-002",
                format!("missing NDF metadata field: {name}"),
                path,
                line,
                "Add all required PTO NDF metadata fields.",
                Some(subject.clone()),
            ));
        }
    }
    for name in values.keys() {
        if !["kind", "level", "layer", "status"].contains(&name.as_str()) {
            diagnostics.push(diagnostic(
                "NDF-ASL-002",
                format!("unknown NDF metadata field: {name}"),
                path,
                line,
                "Remove the unsupported metadata field.",
                Some(subject.clone()),
            ));
        }
    }
    if let Some(kind) = values.get("kind") {
        if let Some((level, _)) = kind_contract(kind) {
            if values.get("level").is_some_and(|actual| actual != level) {
                diagnostics.push(diagnostic(
                    "NDF-ASL-002",
                    format!("kind {kind} requires level {level}"),
                    path,
                    line,
                    "Use the level assigned to the PTO kind.",
                    Some(subject.clone()),
                ));
            }
        } else {
            diagnostics.push(diagnostic(
                "NDF-ASL-002",
                format!("unknown NDF kind: {kind}"),
                path,
                line,
                "Use intent, contract, mechanism, or executable.",
                Some(subject.clone()),
            ));
        }
    }
    if let Some(layer) = values.get("layer")
        && !LAYERS.contains(&layer.as_str())
    {
        diagnostics.push(diagnostic(
            "NDF-ASL-002",
            format!("unknown NDF layer: {layer}"),
            path,
            line,
            "Use a registered PTO layer.",
            Some(subject.clone()),
        ));
    }
    if let Some(value) = values.get("status")
        && status(value).is_none()
    {
        diagnostics.push(diagnostic(
            "NDF-ASL-002",
            format!("unknown NDF status: {value}"),
            path,
            line,
            "Use open or accepted.",
            Some(subject.clone()),
        ));
    }
    (values, diagnostics)
}

fn kind_contract(kind: &str) -> Option<(&'static str, NodeKind)> {
    match kind {
        "intent" => Some(("L0", NodeKind::Architecture)),
        "contract" => Some(("L1", NodeKind::Requirement)),
        "mechanism" => Some(("L2", NodeKind::Architecture)),
        "executable" => Some(("L3", NodeKind::Model)),
        _ => None,
    }
}

fn status(value: &str) -> Option<LifecycleStatus> {
    match value {
        "open" => Some(LifecycleStatus::Open),
        "accepted" => Some(LifecycleStatus::Active),
        _ => None,
    }
}

fn marker<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    line.strip_prefix(prefix)
        .filter(|value| valid_clause_id(value))
}

fn valid_clause_id(value: &str) -> bool {
    value.strip_prefix("PTO-").is_some_and(|rest| {
        !rest.is_empty()
            && rest.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|item| item.is_ascii_uppercase() || item.is_ascii_digit())
            })
    })
}

fn references(body: &str) -> Vec<&str> {
    let mut remaining = body;
    let mut result = Vec::new();
    while let Some(start) = remaining.find("[[") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find("]]") else {
            break;
        };
        let candidate = &remaining[..end];
        if valid_clause_id(candidate) {
            result.push(candidate);
        }
        remaining = &remaining[end + 2..];
    }
    result
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.to_uppercase().chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character);
        } else if !output.is_empty() && !output.ends_with('-') {
            output.push('-');
        }
    }
    output.trim_end_matches('-').to_owned()
}

fn clause_title(value: &str) -> String {
    value
        .strip_prefix("PTO-")
        .unwrap_or(value)
        .split('-')
        .map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| {
                    format!(
                        "{}{}",
                        first.to_ascii_uppercase(),
                        characters.as_str().to_ascii_lowercase()
                    )
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn node_id(project: &ProjectManifest, local: &str) -> Result<NodeId, CompilerError> {
    NodeId::new(&project.project, local).map_err(|error| CompilerError::Contract(error.to_string()))
}

fn relative_path(source: &Path, project: &ProjectManifest) -> Result<String, CompilerError> {
    source
        .strip_prefix(&project.root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            CompilerError::Contract(format!(
                "source is outside project root: {}",
                source.display()
            ))
        })
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    path: &str,
    line: u32,
    repair: &str,
    subject: Option<NodeId>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        primary: SourceSpan::new(path, line).expect("positive ASL line"),
        related: Vec::new(),
        repair: Some(repair.to_owned()),
        subject,
    }
}
