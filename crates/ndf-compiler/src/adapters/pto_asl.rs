use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ndf_core::identity::NodeId;
use ndf_core::model::{
    Diagnostic, DiagnosticSeverity, Edge, LifecycleStatus, Node, NodeKind, ParseResult, SourceSpan,
};
use serde_json::{Map, Value};

use crate::CompilerError;
use crate::manifest::ProjectManifest;

const METADATA_PREFIX: &str = "// ndf: ";
const UNIT_PREFIX: &str = "// PTO-UNIT: ";
const INSTRUCTION_PREFIX: &str = "// PTO-INSTRUCTION: ";
const STATE_PREFIX: &str = "// PTO-STATE: ";
const SYNTHETIC_DECODER_ALIAS: &str = "generated:decoders";
const SYNTHETIC_DECODER_ID: &str = "PTO-SYNTHETIC-GENERATED-DECODERS";
const STATE_SCOPES: &[&str] = &["pe", "bundle", "core", "memory-agent", "acr", "system"];
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
    let mut unit_sources = BTreeMap::new();
    let mut synthetic_decoder_emitted = false;
    let mut file_unit_id = None;
    let variables = declared_variables(text);
    let mut state_sources = BTreeMap::new();
    let mut state_members = BTreeSet::new();

    for (index, line) in text.lines().enumerate() {
        let line_number = (index + 1) as u32;
        if active_id.is_none() && line.starts_with(STATE_PREFIX) {
            let metadata = match json_object_metadata(line, STATE_PREFIX, &path, line_number) {
                Ok(metadata) => metadata,
                Err(diagnostic) => {
                    diagnostics.push(*diagnostic);
                    continue;
                }
            };
            let Some(unit_id) = file_unit_id.as_deref() else {
                diagnostics.push(invalid_state_reference(
                    "state metadata must follow its same-file PTO unit record",
                    &path,
                    line_number,
                    None,
                ));
                continue;
            };
            let (state, state_edges) =
                match state_node(&metadata, &variables, unit_id, &path, line_number, project) {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        diagnostics.push(*diagnostic);
                        continue;
                    }
                };
            if let Some(existing_source) = state_sources.get(&state.id).cloned() {
                diagnostics.push(Diagnostic {
                    code: "NDF-ASL-008".to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("duplicate PTO state metadata: {}", state.id.local_id()),
                    primary: state.source,
                    related: vec![existing_source],
                    repair: Some("Keep exactly one record for each PTO-STATE identity.".to_owned()),
                    subject: Some(state.id),
                });
                continue;
            }
            let members = state.attributes["members"].split(',').collect::<Vec<_>>();
            if let Some(member) = members
                .iter()
                .find(|member| state_members.contains(**member))
            {
                diagnostics.push(invalid_state_reference(
                    format!("state member is assigned to more than one family: {member}"),
                    &path,
                    line_number,
                    Some(state.id),
                ));
                continue;
            }
            state_sources.insert(state.id.clone(), state.source.clone());
            state_members.extend(members.into_iter().map(str::to_owned));
            nodes.push(state);
            edges.extend(state_edges);
            continue;
        }
        if active_id.is_none()
            && (line.starts_with(UNIT_PREFIX) || line.starts_with(INSTRUCTION_PREFIX))
        {
            let is_instruction = line.starts_with(INSTRUCTION_PREFIX);
            let prefix = if is_instruction {
                INSTRUCTION_PREFIX
            } else {
                UNIT_PREFIX
            };
            let metadata = match json_object_metadata(line, prefix, &path, line_number) {
                Ok(metadata) => metadata,
                Err(diagnostic) => {
                    diagnostics.push(*diagnostic);
                    continue;
                }
            };
            let instruction = if is_instruction {
                match instruction_node(&metadata, &path, line_number, project)? {
                    Ok(node) => Some(node),
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                }
            } else {
                None
            };
            let unit = match unit_node(&metadata, &path, line_number, project)? {
                Ok(node) => node,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if let Some(existing_source) = unit_sources.get(&unit.id).cloned() {
                diagnostics.push(Diagnostic {
                    code: "NDF-ASL-006".to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("duplicate PTO ASL unit metadata: {}", unit.id.local_id()),
                    primary: unit.source,
                    related: vec![existing_source],
                    repair: Some(
                        "Keep exactly one PTO-UNIT or PTO-INSTRUCTION record for each ASL unit ID."
                            .to_owned(),
                    ),
                    subject: Some(unit.id),
                });
                continue;
            }
            unit_sources.insert(unit.id.clone(), unit.source.clone());
            file_unit_id = Some(unit.id.local_id().to_owned());

            let unit_id = unit.id.clone();
            let unit_source = unit.source.clone();
            for dependency in
                required_string_array(&metadata, "depends_on").expect("unit metadata was validated")
            {
                let dependency_id = unit_dependency_id(dependency, project)?;
                edges.push(
                    Edge::new(
                        unit_id.clone(),
                        dependency_id,
                        "depends-on",
                        Some(unit_source.clone()),
                    )
                    .expect("non-empty built-in edge type"),
                );
                if dependency == SYNTHETIC_DECODER_ALIAS && !synthetic_decoder_emitted {
                    nodes.push(synthetic_decoder_node(project)?);
                    synthetic_decoder_emitted = true;
                }
            }
            nodes.push(unit);

            if let Some(node) = instruction {
                edges.push(
                    Edge::new(
                        node.id.clone(),
                        unit_id,
                        "defined-in",
                        Some(node.source.clone()),
                    )
                    .expect("non-empty built-in edge type"),
                );
                nodes.push(node);
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

fn json_object_metadata(
    line: &str,
    prefix: &str,
    path: &str,
    line_number: u32,
) -> Result<Map<String, Value>, Box<Diagnostic>> {
    let (code, label, repair) = if prefix == INSTRUCTION_PREFIX {
        (
            "NDF-ASL-004",
            "PTO instruction",
            "Provide valid PTO instruction JSON metadata.",
        )
    } else if prefix == STATE_PREFIX {
        (
            "NDF-ASL-008",
            "PTO state",
            "Provide valid JSON with the required PTO state fields.",
        )
    } else {
        (
            "NDF-ASL-006",
            "PTO ASL unit",
            "Provide valid JSON with the required ASL unit fields.",
        )
    };
    match serde_json::from_str(&line[prefix.len()..]) {
        Ok(Value::Object(metadata)) => Ok(metadata),
        Ok(_) => Err(Box::new(diagnostic(
            code,
            format!("invalid {label} metadata: expected object"),
            path,
            line_number,
            repair,
            None,
        ))),
        Err(error) => Err(Box::new(diagnostic(
            code,
            format!("invalid {label} metadata: {error}"),
            path,
            line_number,
            repair,
            None,
        ))),
    }
}

fn unit_node(
    metadata: &Map<String, Value>,
    path: &str,
    line: u32,
    project: &ProjectManifest,
) -> Result<Result<Node, Diagnostic>, CompilerError> {
    let Some(id) = metadata.get("id").and_then(Value::as_str) else {
        return Ok(Err(invalid_unit_metadata(
            "missing string id",
            path,
            line,
            None,
        )));
    };
    let subject = node_id(project, id).ok();
    if !valid_clause_id(id) {
        return Ok(Err(invalid_unit_metadata(
            "id must be a PTO-prefixed uppercase identity",
            path,
            line,
            subject,
        )));
    }
    let Some(surface) = metadata.get("surface").and_then(Value::as_str) else {
        return Ok(Err(invalid_unit_metadata(
            "missing string surface",
            path,
            line,
            subject,
        )));
    };
    if surface.is_empty() {
        return Ok(Err(invalid_unit_metadata(
            "surface must not be empty",
            path,
            line,
            subject,
        )));
    }
    let Some(classification) = required_string_array(metadata, "classification") else {
        return Ok(Err(invalid_unit_metadata(
            "classification must be a non-empty array of non-empty strings",
            path,
            line,
            subject,
        )));
    };
    if classification.is_empty() {
        return Ok(Err(invalid_unit_metadata(
            "classification must be a non-empty array of non-empty strings",
            path,
            line,
            subject,
        )));
    }
    if let Some(invalid) = classification.iter().find(|value| {
        !value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    }) {
        return Ok(Err(invalid_unit_metadata(
            format!("invalid classification segment: {invalid}"),
            path,
            line,
            subject,
        )));
    }
    let Some(dependencies) = required_string_array(metadata, "depends_on") else {
        return Ok(Err(invalid_unit_metadata(
            "depends_on must be an array of non-empty strings",
            path,
            line,
            subject,
        )));
    };
    if let Some(unsupported) = dependencies
        .iter()
        .find(|value| !valid_clause_id(value) && **value != SYNTHETIC_DECODER_ALIAS)
    {
        return Ok(Err(invalid_unit_metadata(
            format!("unsupported ASL unit dependency: {unsupported}"),
            path,
            line,
            subject,
        )));
    }

    let mut attributes = BTreeMap::from([
        ("pto_entity".to_owned(), "unit".to_owned()),
        ("surface".to_owned(), surface.to_owned()),
        ("classification".to_owned(), classification.join("/")),
    ]);
    if let Some(mnemonic) = metadata.get("mnemonic").and_then(Value::as_str) {
        if mnemonic.is_empty() {
            return Ok(Err(invalid_unit_metadata(
                "mnemonic must not be empty when present",
                path,
                line,
                subject,
            )));
        }
        attributes.insert("mnemonic".to_owned(), mnemonic.to_owned());
    }

    Ok(Ok(Node {
        id: node_id(project, id)?,
        kind: NodeKind::Source,
        title: format!("{surface} ASL unit {id}"),
        source: SourceSpan::new(path, line).expect("positive ASL line"),
        modality: None,
        refinement: None,
        domain: Some(surface.to_owned()),
        status: LifecycleStatus::Active,
        owner: Some("pto-spec".to_owned()),
        body: String::new(),
        attributes,
    }))
}

fn invalid_unit_metadata(
    detail: impl Into<String>,
    path: &str,
    line: u32,
    subject: Option<NodeId>,
) -> Diagnostic {
    diagnostic(
        "NDF-ASL-006",
        format!("invalid PTO ASL unit metadata: {}", detail.into()),
        path,
        line,
        "Provide id, surface, non-empty classification, and depends_on with the documented types.",
        subject,
    )
}

fn required_string_array<'a>(metadata: &'a Map<String, Value>, name: &str) -> Option<Vec<&'a str>> {
    let values = metadata.get(name)?.as_array()?;
    values
        .iter()
        .map(|value| value.as_str().filter(|value| !value.is_empty()))
        .collect()
}

fn unit_dependency_id(value: &str, project: &ProjectManifest) -> Result<NodeId, CompilerError> {
    if value == SYNTHETIC_DECODER_ALIAS {
        node_id(project, SYNTHETIC_DECODER_ID)
    } else if value.starts_with("PTO-") {
        node_id(project, value)
    } else {
        Err(CompilerError::Contract(format!(
            "unsupported PTO ASL unit dependency: {value}"
        )))
    }
}

fn synthetic_decoder_node(project: &ProjectManifest) -> Result<Node, CompilerError> {
    Ok(Node {
        id: node_id(project, SYNTHETIC_DECODER_ID)?,
        kind: NodeKind::Source,
        title: "Generated decoder projection".to_owned(),
        source: SourceSpan::new(SYNTHETIC_DECODER_ALIAS, 1).expect("positive virtual line"),
        modality: None,
        refinement: None,
        domain: Some("generated".to_owned()),
        status: LifecycleStatus::Active,
        owner: Some("pto-spec".to_owned()),
        body: String::new(),
        attributes: BTreeMap::from([("pto_entity".to_owned(), "synthetic-unit".to_owned())]),
    })
}

fn state_node(
    metadata: &Map<String, Value>,
    declared_variables: &BTreeSet<String>,
    unit_id: &str,
    path: &str,
    line: u32,
    project: &ProjectManifest,
) -> Result<(Node, Vec<Edge>), Box<Diagnostic>> {
    const FIELDS: &[&str] = &[
        "id",
        "classification",
        "scope",
        "owner",
        "members",
        "depends_on",
    ];
    if let Some(field) = metadata
        .keys()
        .find(|field| !FIELDS.contains(&field.as_str()))
    {
        return Err(Box::new(invalid_state_metadata(
            format!("unknown field: {field}"),
            path,
            line,
            None,
        )));
    }
    if let Some(field) = FIELDS.iter().find(|field| !metadata.contains_key(**field)) {
        return Err(Box::new(invalid_state_metadata(
            format!("missing field: {field}"),
            path,
            line,
            None,
        )));
    }

    let Some(id) = metadata.get("id").and_then(Value::as_str) else {
        return Err(Box::new(invalid_state_metadata(
            "id must be a string",
            path,
            line,
            None,
        )));
    };
    let subject = node_id(project, id).ok();
    if !id.starts_with("PTO-STATE-") || !valid_clause_id(id) {
        return Err(Box::new(invalid_state_metadata(
            "id must be a PTO-STATE-prefixed uppercase identity",
            path,
            line,
            subject,
        )));
    }
    let Some(classification) = required_string_array(metadata, "classification") else {
        return Err(Box::new(invalid_state_metadata(
            "classification must be an array of non-empty strings",
            path,
            line,
            subject,
        )));
    };
    if classification.is_empty()
        || classification.iter().any(|value| {
            !value.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        })
    {
        return Err(Box::new(invalid_state_metadata(
            "classification must contain lowercase segments",
            path,
            line,
            subject,
        )));
    }
    let Some(scope) = metadata.get("scope").and_then(Value::as_str) else {
        return Err(Box::new(invalid_state_metadata(
            "scope must be a string",
            path,
            line,
            subject,
        )));
    };
    if !STATE_SCOPES.contains(&scope) {
        return Err(Box::new(invalid_state_metadata(
            format!("unsupported scope: {scope}"),
            path,
            line,
            subject,
        )));
    }
    let Some(owner) = metadata.get("owner").and_then(Value::as_str) else {
        return Err(Box::new(invalid_state_metadata(
            "owner must be a string",
            path,
            line,
            subject,
        )));
    };
    if owner != unit_id {
        return Err(Box::new(invalid_state_reference(
            format!("state owner {owner} does not match same-file unit {unit_id}"),
            path,
            line,
            subject,
        )));
    }
    let Some(members) = required_string_array(metadata, "members") else {
        return Err(Box::new(invalid_state_metadata(
            "members must be an array of non-empty strings",
            path,
            line,
            subject,
        )));
    };
    if members.is_empty() {
        return Err(Box::new(invalid_state_metadata(
            "members must not be empty",
            path,
            line,
            subject,
        )));
    }
    let unique_members = members.iter().copied().collect::<BTreeSet<_>>();
    if unique_members.len() != members.len() {
        return Err(Box::new(invalid_state_metadata(
            "members must not contain duplicates",
            path,
            line,
            subject,
        )));
    }
    if let Some(member) = members
        .iter()
        .find(|member| !declared_variables.contains(**member))
    {
        return Err(Box::new(invalid_state_reference(
            format!("state member is not declared by a same-file ASL var: {member}"),
            path,
            line,
            subject,
        )));
    }
    let Some(dependencies) = required_string_array(metadata, "depends_on") else {
        return Err(Box::new(invalid_state_metadata(
            "depends_on must be an array of non-empty strings",
            path,
            line,
            subject,
        )));
    };
    if let Some(dependency) = dependencies
        .iter()
        .find(|dependency| !dependency.starts_with("PTO-STATE-") || !valid_clause_id(dependency))
    {
        return Err(Box::new(invalid_state_metadata(
            format!("invalid state dependency: {dependency}"),
            path,
            line,
            subject,
        )));
    }

    let state_id = node_id(project, id).map_err(|error| {
        Box::new(invalid_state_metadata(
            error.to_string(),
            path,
            line,
            subject.clone(),
        ))
    })?;
    let owner_id = node_id(project, owner).map_err(|error| {
        Box::new(invalid_state_reference(
            error.to_string(),
            path,
            line,
            subject.clone(),
        ))
    })?;
    let source = SourceSpan::new(path, line).expect("positive ASL line");
    let mut edges = vec![
        Edge::new(
            state_id.clone(),
            owner_id.clone(),
            "defined-in",
            Some(source.clone()),
        )
        .expect("non-empty built-in edge type"),
        Edge::new(
            owner_id,
            state_id.clone(),
            "owns-state",
            Some(source.clone()),
        )
        .expect("non-empty built-in edge type"),
    ];
    for dependency in dependencies {
        edges.push(
            Edge::new(
                state_id.clone(),
                node_id(project, dependency).map_err(|error| {
                    Box::new(invalid_state_metadata(
                        error.to_string(),
                        path,
                        line,
                        subject.clone(),
                    ))
                })?,
                "depends-on",
                Some(source.clone()),
            )
            .expect("non-empty built-in edge type"),
        );
    }

    Ok((
        Node {
            id: state_id,
            kind: NodeKind::Architecture,
            title: clause_title(id),
            source,
            modality: None,
            refinement: None,
            domain: Some("state".to_owned()),
            status: LifecycleStatus::Active,
            owner: Some("pto-spec".to_owned()),
            body: String::new(),
            attributes: BTreeMap::from([
                ("pto_entity".to_owned(), "state".to_owned()),
                ("classification".to_owned(), classification.join("/")),
                ("scope".to_owned(), scope.to_owned()),
                ("members".to_owned(), members.join(",")),
            ]),
        },
        edges,
    ))
}

fn invalid_state_metadata(
    detail: impl Into<String>,
    path: &str,
    line: u32,
    subject: Option<NodeId>,
) -> Diagnostic {
    diagnostic(
        "NDF-ASL-008",
        format!("invalid PTO state metadata: {}", detail.into()),
        path,
        line,
        "Provide exactly id, classification, scope, owner, members, and depends_on with documented types.",
        subject,
    )
}

fn invalid_state_reference(
    detail: impl Into<String>,
    path: &str,
    line: u32,
    subject: Option<NodeId>,
) -> Diagnostic {
    diagnostic(
        "NDF-ASL-009",
        format!("invalid PTO state reference: {}", detail.into()),
        path,
        line,
        "Use the same-file unit owner and declared ASL var members exactly once.",
        subject,
    )
}

fn declared_variables(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| {
            let declaration = line.trim_start().strip_prefix("var ")?;
            let (name, _) = declaration.split_once(':')?;
            let name = name.trim();
            (name.starts_with('_')
                && name[1..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphabetic())
                && name[2..]
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_'))
            .then(|| name.to_owned())
        })
        .collect()
}

fn instruction_node(
    metadata: &Map<String, Value>,
    path: &str,
    line_number: u32,
    project: &ProjectManifest,
) -> Result<Result<Node, Diagnostic>, CompilerError> {
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
        attributes: instruction_attributes(metadata, surface, mnemonic),
    }))
}

fn instruction_attributes(
    metadata: &Map<String, Value>,
    surface: &str,
    mnemonic: &str,
) -> BTreeMap<String, String> {
    let semantic_handlers = string_field_values(metadata, &["semantic_handler"]);
    let statuses = string_field_values(metadata, &["status", "contract_status"]);
    let mut facets = BTreeSet::new();
    for (facet, fields) in [
        (
            "encoding",
            &["encoding", "encoding_kind", "selector", "function"][..],
        ),
        (
            "instruction-semantics",
            &["semantic_handler", "effect_contract", "semantic_summary"][..],
        ),
        ("legality", &["constraints", "legality_handler"][..]),
        ("fault", &["fault_contract"][..]),
        (
            "completion",
            &["completion_contract", "restart_contract"][..],
        ),
        ("architectural-state", &["state_effects"][..]),
        (
            "memory-ordering",
            &[
                "memory_ordering",
                "memory_ordering_contract",
                "ordering_contract",
                "memory_events",
            ][..],
        ),
    ] {
        if metadata_has_any(metadata, fields) {
            facets.insert(facet);
        }
    }

    BTreeMap::from([
        ("pto_entity".to_owned(), "instruction".to_owned()),
        ("surface".to_owned(), surface.to_owned()),
        ("mnemonic".to_owned(), mnemonic.to_owned()),
        (
            "semantic_handler".to_owned(),
            joined_or_unspecified(semantic_handlers),
        ),
        ("status".to_owned(), joined_or_unspecified(statuses)),
        (
            "semantic_facets".to_owned(),
            facets.into_iter().collect::<Vec<_>>().join(","),
        ),
    ])
}

fn joined_or_unspecified(values: BTreeSet<String>) -> String {
    if values.is_empty() {
        "unspecified".to_owned()
    } else {
        values.into_iter().collect::<Vec<_>>().join(",")
    }
}

fn string_field_values(metadata: &Map<String, Value>, fields: &[&str]) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    for (name, value) in metadata {
        if fields.contains(&name.as_str())
            && let Some(value) = value.as_str()
            && !value.is_empty()
        {
            values.insert(value.to_owned());
        }
        collect_string_field_values(value, fields, &mut values);
    }
    values
}

fn collect_string_field_values(value: &Value, fields: &[&str], values: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (name, value) in object {
                if fields.contains(&name.as_str())
                    && let Some(value) = value.as_str()
                    && !value.is_empty()
                {
                    values.insert(value.to_owned());
                }
                collect_string_field_values(value, fields, values);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_string_field_values(item, fields, values);
            }
        }
        _ => {}
    }
}

fn metadata_has_any(metadata: &Map<String, Value>, fields: &[&str]) -> bool {
    metadata.iter().any(|(name, value)| {
        (fields.contains(&name.as_str()) && meaningful(value)) || value_has_any(value, fields)
    })
}

fn value_has_any(value: &Value, fields: &[&str]) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(name, value)| {
            (fields.contains(&name.as_str()) && meaningful(value)) || value_has_any(value, fields)
        }),
        Value::Array(items) => items.iter().any(|item| value_has_any(item, fields)),
        _ => false,
    }
}

fn meaningful(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
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
