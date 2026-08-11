use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ndf_core::graph::GraphBuilder;
use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, LifecycleStatus, Modality, Node, NodeKind, ParseResult,
    SourceSpan,
};
use serde_yaml_ng::{Mapping, Value};

use crate::CompilerError;
use crate::manifest::ProjectManifest;

const EDGE_FIELDS: &[&str] = &[
    "refines",
    "depends-on",
    "conflicts-with",
    "verifies",
    "derived-from",
    "implements",
    "affects",
    "blocks",
    "supersedes",
    "references",
    "evidenced-by",
    "couples-with",
    "blocks-by",
    "conforms-to",
    "tracks",
];
const CANONICAL_FIELDS: &[&str] = &[
    "kind",
    "modality",
    "refinement",
    "domain",
    "status",
    "owner",
];
const ATTRIBUTE_FIELDS: &[&str] = &[
    "since",
    "date",
    "author",
    "unit",
    "default",
    "explore",
    "origin",
    "origin-status",
    "model",
    "superseded-by",
    "expected",
];

pub fn parse_markdown(
    path: &Path,
    project: &ProjectManifest,
) -> Result<ParseResult, CompilerError> {
    let relative_path = relative_path(path, project)?;
    let text = fs::read_to_string(path).map_err(|source| CompilerError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let lines: Vec<_> = text.lines().collect();
    let (front, content_start, front_diagnostics) = parse_front_matter(&lines, &relative_path)?;
    let Some(front) = front else {
        return Ok(ParseResult {
            diagnostics: front_diagnostics,
            ..ParseResult::default()
        });
    };
    let required = ["doc_id", "status", "authority", "owner"];
    for name in required {
        if !front.contains_key(Value::String(name.to_owned())) {
            return Ok(only_diagnostic(diagnostic(
                "NDF-MD-003",
                DiagnosticSeverity::Error,
                format!("missing front matter field: {name}"),
                &relative_path,
                1,
                "Add all required document front matter fields.",
            )));
        }
    }
    let mut unknown_front: Vec<_> = front
        .keys()
        .filter_map(Value::as_str)
        .filter(|name| !required.contains(name))
        .collect();
    unknown_front.sort_unstable();
    if let Some(name) = unknown_front.first() {
        return Ok(only_diagnostic(diagnostic(
            "NDF-MD-003",
            DiagnosticSeverity::Error,
            format!("unsupported front matter field: {name}"),
            &relative_path,
            1,
            "Remove unsupported document front matter.",
        )));
    }

    let doc_id = front_string(&front, "doc_id")?;
    let owner = front_string(&front, "owner")?;
    let authority = front_string(&front, "authority")?;
    let status_value = front_string(&front, "status")?;
    let Some(document_status) = lifecycle_status(&status_value) else {
        return Ok(only_diagnostic(diagnostic(
            "NDF-MD-004",
            DiagnosticSeverity::Error,
            format!("invalid document status: {status_value}"),
            &relative_path,
            1,
            "Use a canonical lifecycle status.",
        )));
    };

    let title = first_document_title(&lines[content_start..]).unwrap_or_else(|| doc_id.clone());
    let mut attributes = BTreeMap::new();
    attributes.insert("authority".to_owned(), authority);
    let document = Node {
        id: NodeId::new(&project.project, &doc_id)
            .map_err(|error| CompilerError::Contract(error.to_string()))?,
        kind: NodeKind::Document,
        title,
        source: SourceSpan::new(&relative_path, 1).expect("line one is valid"),
        modality: None,
        refinement: None,
        domain: None,
        status: document_status,
        owner: Some(owner.clone()),
        body: String::new(),
        attributes,
    };

    let mut builder = GraphBuilder::default();
    builder.add_node(document);
    for item in front_diagnostics {
        builder.add_diagnostic(item);
    }
    let mut fence: Option<char> = None;
    for index in content_start..lines.len() {
        if let Some(marker) = fence_marker(lines[index]) {
            fence = match fence {
                None => Some(marker),
                Some(active) if active == marker => None,
                active => active,
            };
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let Some(heading) = parse_heading(lines[index]) else {
            continue;
        };
        let Some(local_id) = heading.local_id else {
            continue;
        };
        let metadata_index = index + 1;
        if metadata_index >= lines.len() {
            continue;
        }
        let (metadata, diagnostics) = parse_metadata(
            lines[metadata_index],
            &relative_path,
            (metadata_index + 1) as u32,
        );
        for item in diagnostics {
            builder.add_diagnostic(item);
        }
        let Some(metadata) = metadata else {
            continue;
        };
        let (metadata, migration) =
            normalize_metadata(metadata, &relative_path, (metadata_index + 1) as u32);
        if let Some(migration) = migration {
            builder.add_diagnostic(migration);
        }
        if let Some(item) = cross_project_metadata_diagnostic(
            &metadata,
            &relative_path,
            (metadata_index + 1) as u32,
        ) {
            builder.add_diagnostic(item);
            continue;
        }
        let Some(kind) = node_kind(metadata.get("kind").map_or("information", String::as_str))
        else {
            builder.add_diagnostic(invalid_value(&relative_path, metadata_index + 1, "kind"));
            continue;
        };
        let modality = match metadata.get("modality") {
            Some(value) => match modality(value) {
                Some(value) => Some(value),
                None => {
                    builder.add_diagnostic(invalid_value(
                        &relative_path,
                        metadata_index + 1,
                        "modality",
                    ));
                    continue;
                }
            },
            None => None,
        };
        let status_text = metadata.get("status").map_or("draft", String::as_str);
        let Some(status) = lifecycle_status(status_text) else {
            builder.add_diagnostic(invalid_value(&relative_path, metadata_index + 1, "status"));
            continue;
        };
        let mut attributes = BTreeMap::new();
        for field in ATTRIBUTE_FIELDS {
            if let Some(value) = metadata.get(*field) {
                attributes.insert((*field).to_owned(), value.clone());
            }
        }
        if let Some(value) = metadata.get("derived-from")
            && value
                .split(',')
                .map(str::trim)
                .any(|target| NodeId::parse(target, Some(&project.project)).is_err())
        {
            attributes.insert("derived-from".to_owned(), value.clone());
        }
        let node = Node {
            id: NodeId::new(&project.project, local_id)
                .map_err(|error| CompilerError::Contract(error.to_string()))?,
            kind,
            title: heading.title,
            source: SourceSpan::new(&relative_path, (index + 1) as u32)
                .expect("document line is positive"),
            modality,
            refinement: metadata.get("refinement").cloned(),
            domain: metadata.get("domain").cloned(),
            status,
            owner: Some(
                metadata
                    .get("owner")
                    .cloned()
                    .unwrap_or_else(|| owner.clone()),
            ),
            body: body(&lines, metadata_index + 1),
            attributes,
        };
        let source_id = node.id.clone();
        builder.add_node(node);
        for edge_type in EDGE_FIELDS {
            let Some(targets) = metadata.get(*edge_type) else {
                continue;
            };
            for target in targets.split(',').map(str::trim) {
                let target_id = match NodeId::parse(target, Some(&project.project)) {
                    Ok(target_id) => target_id,
                    Err(_) if *edge_type == "derived-from" => continue,
                    Err(error) => return Err(CompilerError::Contract(error.to_string())),
                };
                builder.add_edge(
                    Edge::new(
                        source_id.clone(),
                        target_id,
                        *edge_type,
                        Some(
                            SourceSpan::new(&relative_path, (metadata_index + 1) as u32)
                                .expect("metadata line is positive"),
                        ),
                    )
                    .expect("registered edge types are not empty"),
                );
            }
        }
    }
    let graph = builder.finish();
    Ok(ParseResult {
        nodes: graph.nodes,
        edges: graph.edges,
        diagnostics: graph.diagnostics,
    })
}

fn cross_project_metadata_diagnostic(
    metadata: &BTreeMap<String, String>,
    path: &str,
    line: u32,
) -> Option<Diagnostic> {
    match (
        metadata.get("tracks").map(String::as_str),
        metadata.get("expected").map(String::as_str),
    ) {
        (None, None) | (Some(_), Some("absent")) => None,
        (None, Some(_)) => Some(diagnostic(
            "NDF-XREF-001",
            DiagnosticSeverity::Error,
            "expected is valid only with a tracks relationship",
            path,
            line,
            "Add tracks=<external NDF URI> or remove expected.",
        )),
        (Some(_), _) => Some(diagnostic(
            "NDF-XREF-001",
            DiagnosticSeverity::Error,
            "tracks requires expected=absent",
            path,
            line,
            "Set expected=absent for the tracked external identity.",
        )),
    }
}

struct Heading<'a> {
    title: String,
    local_id: Option<&'a str>,
}

fn parse_heading(line: &str) -> Option<Heading<'_>> {
    let hashes = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&hashes) || !line[hashes..].starts_with(char::is_whitespace) {
        return None;
    }
    let content = line[hashes..].trim();
    if let Some(prefix) = content.strip_suffix('}')
        && let Some((title, local_id)) = prefix.rsplit_once(" {#")
        && !local_id.is_empty()
    {
        return Some(Heading {
            title: title.to_owned(),
            local_id: Some(local_id),
        });
    }
    Some(Heading {
        title: content.to_owned(),
        local_id: None,
    })
}

fn fence_marker(line: &str) -> Option<char> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if !matches!(marker, '`' | '~')
        || trimmed.chars().take_while(|value| *value == marker).count() < 3
    {
        return None;
    }
    Some(marker)
}

fn first_document_title(lines: &[&str]) -> Option<String> {
    let mut fence = None;
    for line in lines {
        if let Some(marker) = fence_marker(line) {
            fence = match fence {
                None => Some(marker),
                Some(active) if active == marker => None,
                active => active,
            };
            continue;
        }
        if fence.is_none()
            && line.starts_with("# ")
            && let Some(heading) = parse_heading(line)
        {
            return Some(heading.title);
        }
    }
    None
}

fn parse_front_matter(
    lines: &[&str],
    path: &str,
) -> Result<(Option<Mapping>, usize, Vec<Diagnostic>), CompilerError> {
    if lines.first().map(|line| line.trim()) != Some("---") {
        return Ok((
            None,
            0,
            vec![diagnostic(
                "NDF-MD-001",
                DiagnosticSeverity::Error,
                "document front matter must start with ---",
                path,
                1,
                "Add complete YAML front matter at the start of the file.",
            )],
        ));
    }
    let Some(closing) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (line.trim() == "---").then_some(index))
    else {
        return Ok((
            None,
            0,
            vec![diagnostic(
                "NDF-MD-001",
                DiagnosticSeverity::Error,
                "document front matter is not closed",
                path,
                1,
                "Close the YAML front matter with ---. ",
            )],
        ));
    };
    let yaml = lines[1..closing].join("\n");
    let value: Value = match serde_yaml_ng::from_str(&yaml) {
        Ok(value) => value,
        Err(error) => {
            return Ok((
                None,
                closing + 1,
                vec![diagnostic(
                    "NDF-MD-002",
                    DiagnosticSeverity::Error,
                    format!("invalid front matter: {error}"),
                    path,
                    1,
                    "Remove duplicate keys and repair the YAML mapping.",
                )],
            ));
        }
    };
    Ok((value.as_mapping().cloned(), closing + 1, Vec::new()))
}

fn parse_metadata(
    line: &str,
    path: &str,
    line_number: u32,
) -> (Option<BTreeMap<String, String>>, Vec<Diagnostic>) {
    let stripped = line.trim();
    if stripped.starts_with("<!-- ndf:") && !stripped.ends_with("-->") {
        return (
            None,
            vec![diagnostic(
                "NDF-MD-001",
                DiagnosticSeverity::Error,
                "NDF metadata comment is not closed",
                path,
                line_number,
                "Close the metadata comment with --> on the same line.",
            )],
        );
    }
    let Some(content) = stripped
        .strip_prefix("<!-- ndf:")
        .and_then(|value| value.strip_suffix("-->"))
    else {
        return (None, Vec::new());
    };
    let tokens = metadata_tokens(content);
    if tokens.is_empty() && !content.trim().is_empty() {
        return (
            None,
            vec![diagnostic(
                "NDF-MD-001",
                DiagnosticSeverity::Error,
                format!("invalid NDF metadata token: {}", content.trim()),
                path,
                line_number,
                "Write metadata as key=value tokens.",
            )],
        );
    }
    let mut metadata = BTreeMap::new();
    for (name, value) in tokens {
        if metadata.contains_key(name) {
            return (
                None,
                vec![diagnostic(
                    "NDF-MD-002",
                    DiagnosticSeverity::Error,
                    format!("duplicate NDF metadata key: {name}"),
                    path,
                    line_number,
                    "Keep exactly one value for each metadata key.",
                )],
            );
        }
        metadata.insert(name.to_owned(), value.to_owned());
    }
    let allowed: BTreeSet<_> = CANONICAL_FIELDS
        .iter()
        .chain(EDGE_FIELDS)
        .chain(ATTRIBUTE_FIELDS)
        .chain(["level", "layer"].iter())
        .copied()
        .collect();
    if let Some(name) = metadata
        .keys()
        .find(|name| !allowed.contains(name.as_str()))
    {
        return (
            None,
            vec![diagnostic(
                "NDF-MD-003",
                DiagnosticSeverity::Error,
                format!("unsupported NDF metadata field: {name}"),
                path,
                line_number,
                "Remove the field or register it in the project format.",
            )],
        );
    }
    (Some(metadata), Vec::new())
}

fn metadata_tokens(content: &str) -> Vec<(&str, &str)> {
    let bytes = content.as_bytes();
    let mut fields = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'-')
        {
            index += 1;
        }
        if name_start == index || index >= bytes.len() || bytes[index] != b'=' {
            break;
        }
        let name = &content[name_start..index];
        index += 1;
        let value_start = index;
        let mut next_field = bytes.len();
        let mut cursor = index;
        while cursor < bytes.len() {
            if bytes[cursor].is_ascii_whitespace() {
                let mut candidate = cursor;
                while candidate < bytes.len() && bytes[candidate].is_ascii_whitespace() {
                    candidate += 1;
                }
                let candidate_start = candidate;
                while candidate < bytes.len()
                    && (bytes[candidate].is_ascii_alphanumeric() || bytes[candidate] == b'-')
                {
                    candidate += 1;
                }
                if candidate_start < candidate
                    && candidate < bytes.len()
                    && bytes[candidate] == b'='
                {
                    next_field = cursor;
                    break;
                }
            }
            cursor += 1;
        }
        fields.push((name, content[value_start..next_field].trim()));
        index = next_field;
    }
    fields
}

fn normalize_metadata(
    mut metadata: BTreeMap<String, String>,
    path: &str,
    line: u32,
) -> (BTreeMap<String, String>, Option<Diagnostic>) {
    let mut migrated = false;
    if let Some(kind) = metadata.get("kind").cloned() {
        let canonical = match kind.as_str() {
            "req" => Some("requirement"),
            "arch" => Some("architecture"),
            "def" => Some("definition"),
            "verif" => Some("verification"),
            "info" => Some("information"),
            _ => None,
        };
        if let Some(canonical) = canonical {
            metadata.insert("kind".to_owned(), canonical.to_owned());
            migrated = true;
        }
    }
    if let Some(value) = metadata.remove("level") {
        metadata.insert("modality".to_owned(), value);
        migrated = true;
    }
    if let Some(value) = metadata.remove("layer") {
        metadata.insert("refinement".to_owned(), value);
        migrated = true;
    }
    if metadata
        .get("status")
        .is_some_and(|value| value == "stable")
    {
        metadata.insert("status".to_owned(), "active".to_owned());
        migrated = true;
    }
    if let Some(superseding_id) = metadata
        .get("status")
        .and_then(|value| value.strip_prefix("superseded-by="))
        .map(str::to_owned)
    {
        metadata.insert("status".to_owned(), "superseded".to_owned());
        metadata.insert("superseded-by".to_owned(), superseding_id);
        migrated = true;
    }
    let diagnostic = migrated.then(|| {
        diagnostic(
            "NDF-MIG-001",
            DiagnosticSeverity::Information,
            "legacy DavinciOO metadata was mapped to canonical fields",
            path,
            line,
            "Rewrite the clause with kind, modality, refinement, and status.",
        )
    });
    (metadata, diagnostic)
}

fn body(lines: &[&str], start: usize) -> String {
    let mut collected = Vec::new();
    let mut fence = None;
    for line in &lines[start..] {
        if let Some(marker) = fence_marker(line) {
            fence = match fence {
                None => Some(marker),
                Some(active) if active == marker => None,
                active => active,
            };
            collected.push(*line);
            continue;
        }
        if fence.is_none() && parse_heading(line).is_some() {
            break;
        }
        collected.push(*line);
    }
    collected.join("\n").trim().to_owned()
}

fn diagnostic(
    code: &str,
    severity: DiagnosticSeverity,
    message: impl Into<String>,
    path: &str,
    line: u32,
    repair: &str,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity,
        message: message.into(),
        primary: SourceSpan::new(path, line).expect("diagnostic line is positive"),
        related: Vec::new(),
        repair: Some(repair.to_owned()),
        subject: None,
    }
}

fn only_diagnostic(diagnostic: Diagnostic) -> ParseResult {
    ParseResult {
        diagnostics: vec![diagnostic],
        ..ParseResult::default()
    }
}

fn invalid_value(path: &str, line: usize, field: &str) -> Diagnostic {
    diagnostic(
        "NDF-MD-004",
        DiagnosticSeverity::Error,
        format!("invalid canonical metadata value: {field}"),
        path,
        line as u32,
        "Use canonical kind, modality, and lifecycle values.",
    )
}

fn relative_path(path: &Path, project: &ProjectManifest) -> Result<String, CompilerError> {
    path.canonicalize()
        .map_err(|source| CompilerError::Read {
            path: path.to_path_buf(),
            source,
        })?
        .strip_prefix(
            project
                .root
                .canonicalize()
                .map_err(|source| CompilerError::Read {
                    path: project.root.clone(),
                    source,
                })?,
        )
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            CompilerError::Contract(format!(
                "document is outside project root: {}",
                path.display()
            ))
        })
}

fn front_string(mapping: &Mapping, name: &str) -> Result<String, CompilerError> {
    mapping
        .get(Value::String(name.to_owned()))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CompilerError::Contract(format!("front matter field must be a string: {name}"))
        })
}

fn node_kind(value: &str) -> Option<NodeKind> {
    Some(match value {
        "document" => NodeKind::Document,
        "requirement" => NodeKind::Requirement,
        "definition" => NodeKind::Definition,
        "architecture" => NodeKind::Architecture,
        "constraint" => NodeKind::Constraint,
        "option" => NodeKind::Option,
        "verification" => NodeKind::Verification,
        "decision" => NodeKind::Decision,
        "question" => NodeKind::Question,
        "model" => NodeKind::Model,
        "source" => NodeKind::Source,
        "test" => NodeKind::Test,
        "evidence" => NodeKind::Evidence,
        "information" => NodeKind::Information,
        _ => return None,
    })
}

fn modality(value: &str) -> Option<Modality> {
    Some(match value {
        "must" => Modality::Must,
        "should" => Modality::Should,
        "may" => Modality::May,
        "tbd" => Modality::Tbd,
        _ => return None,
    })
}

fn lifecycle_status(value: &str) -> Option<LifecycleStatus> {
    Some(match value {
        "draft" => LifecycleStatus::Draft,
        "active" => LifecycleStatus::Active,
        "deprecated" => LifecycleStatus::Deprecated,
        "superseded" => LifecycleStatus::Superseded,
        "retired" => LifecycleStatus::Retired,
        "open" => LifecycleStatus::Open,
        "resolved" => LifecycleStatus::Resolved,
        _ => return None,
    })
}
