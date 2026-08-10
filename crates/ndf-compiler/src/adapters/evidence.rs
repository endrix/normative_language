use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, LifecycleStatus, Node, NodeKind, ParseResult, SourceSpan,
};
use serde_json::{Map, Value};

use crate::CompilerError;
use crate::manifest::ProjectManifest;

const REQUIRED: &[&str] = &[
    "format_version",
    "id",
    "command",
    "result",
    "tool_version",
    "source_revision",
    "timestamp",
    "links",
];

pub fn parse_evidence_manifest(
    path: &Path,
    project: &ProjectManifest,
) -> Result<ParseResult, CompilerError> {
    let relative = path
        .strip_prefix(&project.root)
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            CompilerError::Contract(format!(
                "evidence is outside project root: {}",
                path.display()
            ))
        })?;
    let text = fs::read_to_string(path).map_err(|source| CompilerError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let record: Value = match serde_json::from_str(&text) {
        Ok(record) => record,
        Err(error) => {
            return Ok(invalid(
                &relative,
                error.line() as u32,
                format!("invalid evidence JSON: {error}"),
                "Repair the JSON evidence manifest.",
            ));
        }
    };
    let Some(record) = record.as_object() else {
        return Ok(schema_error(&relative));
    };
    if !valid_schema(record) {
        return Ok(schema_error(&relative));
    }

    let identity = NodeId::new(
        &project.project,
        record["id"].as_str().expect("validated string"),
    )
    .map_err(|error| CompilerError::Contract(error.to_string()))?;
    let node = Node {
        id: identity.clone(),
        kind: NodeKind::Evidence,
        title: record["command"]
            .as_str()
            .expect("validated string")
            .to_owned(),
        source: SourceSpan::new(&relative, 1).expect("line one is valid"),
        modality: None,
        refinement: None,
        domain: None,
        status: LifecycleStatus::Active,
        owner: Some(project.project.clone()),
        body: String::new(),
        attributes: [
            "command",
            "result",
            "tool_version",
            "source_revision",
            "timestamp",
        ]
        .into_iter()
        .map(|key| {
            (
                key.to_owned(),
                record[key].as_str().expect("validated string").to_owned(),
            )
        })
        .collect(),
    };
    let mut edges = Vec::new();
    for link in record["links"].as_array().expect("validated links") {
        let link = link.as_object().expect("validated link object");
        let source = NodeId::parse(
            link["clause"].as_str().expect("validated string"),
            Some(&project.project),
        )
        .map_err(|error| CompilerError::Contract(error.to_string()))?;
        edges.push(
            Edge::new(
                source,
                identity.clone(),
                "evidenced-by",
                Some(SourceSpan::new(&relative, 1).expect("line one is valid")),
            )
            .expect("built-in edge is non-empty"),
        );
    }
    Ok(ParseResult {
        nodes: vec![node],
        edges,
        diagnostics: Vec::new(),
    })
}

fn valid_schema(record: &Map<String, Value>) -> bool {
    let keys: BTreeSet<_> = record.keys().map(String::as_str).collect();
    let required: BTreeSet<_> = REQUIRED.iter().copied().collect();
    if keys != required
        || record["format_version"].as_str() != Some("0.1")
        || !["pass", "fail", "inconclusive"].contains(&record["result"].as_str().unwrap_or(""))
        || ![
            "id",
            "command",
            "tool_version",
            "source_revision",
            "timestamp",
        ]
        .into_iter()
        .all(|key| record[key].is_string())
    {
        return false;
    }
    let revision = record["source_revision"].as_str().expect("string checked");
    let timestamp = record["timestamp"].as_str().expect("string checked");
    if revision.len() != 40
        || !revision
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
        || !timestamp.contains('T')
        || !(timestamp.ends_with('Z') || timestamp.contains('+'))
    {
        return false;
    }
    record["links"].as_array().is_some_and(|links| {
        links.iter().all(|link| {
            let Some(link) = link.as_object() else {
                return false;
            };
            link.len() == 2
                && link.get("clause").is_some_and(Value::is_string)
                && link.get("type").and_then(Value::as_str) == Some("evidenced-by")
        })
    })
}

fn schema_error(path: &str) -> ParseResult {
    invalid(
        path,
        1,
        "evidence manifest does not match the required fields",
        "Provide every reproducibility field and no unknown fields.",
    )
}

fn invalid(path: &str, line: u32, message: impl Into<String>, repair: &str) -> ParseResult {
    ParseResult {
        diagnostics: vec![Diagnostic {
            code: "NDF-EVID-001".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            primary: SourceSpan::new(path, line.max(1)).expect("positive JSON line"),
            related: Vec::new(),
            repair: Some(repair.to_owned()),
            subject: None,
        }],
        ..ParseResult::default()
    }
}
