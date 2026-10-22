//! Edits to an NDF Markdown document, as a diagram's sidecar makes them.
//!
//! Every edit is a change to the text of one document: a clause appended,
//! a heading retitled, a clause removed, a typed edge added to or removed
//! from a clause's metadata comment, a canonical field set. The text is the
//! truth, so each edit is computed on the text, the result is parsed with the
//! same parser `ndf check` uses, and a result that parses worse than the
//! original is refused with the file untouched (`verify`). The metadata line
//! an edit writes is in the canonical order `ndf fix` keeps.
//!
//! Nothing here touches a file: the caller reads the text, applies an edit,
//! verifies and writes, so the sidecar's revision discipline stays in one
//! place.

use std::collections::BTreeMap;
use std::fmt;

use ndf_core::model::{DiagnosticSeverity, Graph, NodeKind};
use serde_json::{Value, json};

use crate::fix::sort_metadata_line;
use crate::id_allocator::allocate_id;
use crate::manifest::ProjectManifest;
use crate::markdown::{
    ATTRIBUTE_FIELDS, CANONICAL_FIELDS, EDGE_FIELDS, fence_marker, metadata_tokens, parse_heading,
    parse_markdown_text,
};

/// The kinds a clause may be created as: every NDF kind but `document`.
pub const CLAUSE_KINDS: &[&str] = &[
    "requirement",
    "definition",
    "architecture",
    "constraint",
    "option",
    "verification",
    "decision",
    "question",
    "model",
    "source",
    "test",
    "evidence",
    "information",
];

/// The typed edges a clause may declare, with a label for a picker, in the
/// order a reader meets them: structure first, then evidence, then the
/// record-keeping and cross-project ones.
pub const EDGE_ROLES: &[(&str, &str)] = &[
    ("refines", "Refines: this clause narrows the target"),
    ("depends-on", "Depends on: this clause needs the target"),
    ("implements", "Implements: this clause realises the target"),
    ("verifies", "Verifies: this clause checks the target"),
    (
        "evidenced-by",
        "Evidenced by: the target records a result for this clause",
    ),
    (
        "derived-from",
        "Derived from: this clause was taken from the target",
    ),
    ("references", "References: an informative mention"),
    ("affects", "Affects: a change here reaches the target"),
    ("conflicts-with", "Conflicts with: the two cannot both hold"),
    (
        "couples-with",
        "Couples with: the two options move together",
    ),
    ("blocks", "Blocks: this open item holds the target"),
    ("blocks-by", "Blocked by: the target holds this clause"),
    ("supersedes", "Supersedes: this clause replaces the target"),
    ("conforms-to", "Conforms to: a contract in another project"),
    ("tracks", "Tracks: a watch on another project's clause"),
];

/// The edge type a connection takes when the consumer names none.
pub const DEFAULT_EDGE_ROLE: &str = "references";

/// The kinds whose clauses state an obligation and so carry a modality.
const NORMATIVE_KINDS: &[&str] = &["requirement", "constraint", "verification", "architecture"];

/// Why an edit was not made. `code` is the sidecar's machine word, `details`
/// what a consumer may show beside the message (candidates, dependents).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError {
    pub code: &'static str,
    pub message: String,
    pub details: Value,
}

impl EditError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Value::Null,
        }
    }

    fn with(mut self, key: &str, value: Value) -> Self {
        if !self.details.is_object() {
            self.details = json!({});
        }
        self.details[key] = value;
        self
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EditError {}

/// Where a clause is in the text: its heading, its metadata line when it has
/// one, and the line after its last (the next heading, or the end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseSpan {
    pub local_id: String,
    pub title: String,
    pub level: usize,
    pub heading_line: usize,
    pub metadata_line: Option<usize>,
    pub end_line: usize,
}

/// One document's text, split into lines, with its clauses located.
#[derive(Debug, Clone)]
pub struct Document {
    lines: Vec<String>,
    newline: &'static str,
    trailing_newline: bool,
    pub clauses: Vec<ClauseSpan>,
}

impl Document {
    pub fn parse(text: &str) -> Self {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let clauses = clause_spans(&lines);
        Self {
            lines,
            newline,
            trailing_newline: text.ends_with('\n') || text.is_empty(),
            clauses,
        }
    }

    pub fn text(&self) -> String {
        let mut text = self.lines.join(self.newline);
        if self.trailing_newline && !self.lines.is_empty() {
            text.push_str(self.newline);
        }
        text
    }

    /// The clause named `name`: by its local id, or by its title when exactly
    /// one clause of the document has it.
    pub fn find(&self, name: &str) -> Result<&ClauseSpan, EditError> {
        let name = name.trim();
        if let Some(span) = self.clauses.iter().find(|span| span.local_id == name) {
            return Ok(span);
        }
        let by_title: Vec<&ClauseSpan> = self
            .clauses
            .iter()
            .filter(|span| span.title == name)
            .collect();
        match by_title.as_slice() {
            [one] => Ok(one),
            [] => Err(EditError::new(
                "node_not_found",
                format!("no clause named {name:?} in this document"),
            )
            .with(
                "candidates",
                Value::Array(
                    self.clauses
                        .iter()
                        .map(|span| Value::String(span.local_id.clone()))
                        .collect(),
                ),
            )),
            many => Err(EditError::new(
                "ambiguous_name",
                format!(
                    "{} clauses are titled {name:?}; name one by its id",
                    many.len()
                ),
            )
            .with(
                "candidates",
                Value::Array(
                    many.iter()
                        .map(|span| Value::String(span.local_id.clone()))
                        .collect(),
                ),
            )),
        }
    }

    /// The metadata tokens of a clause, in the order written.
    pub fn metadata(&self, span: &ClauseSpan) -> Vec<(String, String)> {
        let Some(line) = span.metadata_line else {
            return Vec::new();
        };
        let text = self.lines[line].trim();
        let Some(content) = text
            .strip_prefix("<!-- ndf:")
            .and_then(|rest| rest.strip_suffix("-->"))
        else {
            return Vec::new();
        };
        metadata_tokens(content)
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect()
    }

    /// The document with the clause's metadata line rewritten from `fields`,
    /// in canonical order; a clause without one gets one under its heading.
    pub fn with_metadata(&self, span: &ClauseSpan, fields: &[(String, String)]) -> Document {
        let line = metadata_line(fields);
        let mut next = self.clone();
        match span.metadata_line {
            Some(index) => next.lines[index] = line,
            None => next.lines.insert(span.heading_line + 1, line),
        }
        next.relocate();
        next
    }

    /// The document with the clause retitled; the id stays.
    pub fn retitled(&self, span: &ClauseSpan, title: &str) -> Result<Document, EditError> {
        let title = clean_title(title)?;
        let mut next = self.clone();
        next.lines[span.heading_line] = heading_line(span.level, &title, &span.local_id);
        next.relocate();
        Ok(next)
    }

    /// The document without the clause: its heading, metadata and body, and
    /// the blank line that separated it from the next.
    pub fn without(&self, span: &ClauseSpan) -> Document {
        let mut next = self.clone();
        let mut end = span.end_line;
        // Keep one blank line between what came before and what comes after.
        let mut start = span.heading_line;
        while start > 0 && next.lines[start - 1].trim().is_empty() && end < next.lines.len() {
            start -= 1;
        }
        if start == span.heading_line {
            while end < next.lines.len() && next.lines[end].trim().is_empty() {
                end += 1;
            }
        } else {
            start += 1;
        }
        next.lines.drain(start..end);
        while next.lines.last().is_some_and(|line| line.trim().is_empty()) {
            next.lines.pop();
        }
        next.relocate();
        next
    }

    /// The document with a clause appended: a heading at the level the
    /// document's clauses use, the metadata line, and a blank line.
    pub fn with_clause(
        &self,
        kind: &str,
        title: &str,
        local_id: &str,
        fields: &[(String, String)],
    ) -> Result<Document, EditError> {
        let title = clean_title(title)?;
        let mut all = vec![("kind".to_owned(), kind.to_owned())];
        all.extend(fields.iter().cloned());
        let mut next = self.clone();
        while next.lines.last().is_some_and(|line| line.trim().is_empty()) {
            next.lines.pop();
        }
        if !next.lines.is_empty() {
            next.lines.push(String::new());
        }
        next.lines
            .push(heading_line(self.heading_level(), &title, local_id));
        next.lines.push(metadata_line(&all));
        next.lines.push(String::new());
        next.trailing_newline = true;
        next.relocate();
        Ok(next)
    }

    /// The heading level the document's clauses use most, `2` for a document
    /// without clauses.
    pub fn heading_level(&self) -> usize {
        let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
        for span in &self.clauses {
            *counts.entry(span.level).or_default() += 1;
        }
        counts
            .into_iter()
            .max_by_key(|(level, count)| (*count, usize::MAX - *level))
            .map(|(level, _)| level)
            .unwrap_or(2)
    }

    fn relocate(&mut self) {
        self.clauses = clause_spans(&self.lines);
    }
}

/// The clauses of a document's lines: a heading with `{#ID}` outside a code
/// fence starts one, the next heading (any level) ends it.
pub fn clause_spans<S: AsRef<str>>(lines: &[S]) -> Vec<ClauseSpan> {
    let mut spans: Vec<ClauseSpan> = Vec::new();
    let mut fence: Option<char> = None;
    for (index, line) in lines.iter().enumerate() {
        let line = line.as_ref();
        if let Some(marker) = fence_marker(line) {
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
        let Some(heading) = parse_heading(line) else {
            continue;
        };
        if let Some(last) = spans.last_mut()
            && last.end_line == usize::MAX
        {
            last.end_line = index;
        }
        let Some(local_id) = heading.local_id else {
            continue;
        };
        let metadata_line = lines
            .get(index + 1)
            .filter(|next| next.as_ref().trim_start().starts_with("<!-- ndf:"))
            .map(|_| index + 1);
        spans.push(ClauseSpan {
            local_id: local_id.to_owned(),
            title: heading.title,
            level: line
                .chars()
                .take_while(|character| *character == '#')
                .count(),
            heading_line: index,
            metadata_line,
            end_line: usize::MAX,
        });
    }
    if let Some(last) = spans.last_mut()
        && last.end_line == usize::MAX
    {
        last.end_line = lines.len();
    }
    spans
}

fn heading_line(level: usize, title: &str, local_id: &str) -> String {
    format!("{} {title} {{#{local_id}}}", "#".repeat(level.clamp(1, 6)))
}

fn metadata_line(fields: &[(String, String)]) -> String {
    let tokens: Vec<String> = fields
        .iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(name, value)| format!("{name}={}", value.trim()))
        .collect();
    sort_metadata_line(&format!("<!-- ndf: {} -->", tokens.join(" ")))
}

fn clean_title(title: &str) -> Result<String, EditError> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return Err(EditError::new("invalid_title", "a clause needs a title"));
    }
    if title.contains("{#") || title.ends_with('}') {
        return Err(EditError::new(
            "invalid_title",
            "a title cannot contain an id anchor ({#...}) or end with '}'",
        ));
    }
    Ok(title)
}

// ---------------------------------------------------------------------------
// Metadata fields
// ---------------------------------------------------------------------------

/// Whether `role` is a typed edge a clause may declare.
pub fn is_edge_role(role: &str) -> bool {
    EDGE_FIELDS.contains(&role)
}

/// Add `target` to the clause's `role` list.
pub fn add_target(
    fields: &mut Vec<(String, String)>,
    role: &str,
    target: &str,
) -> Result<(), EditError> {
    if !is_edge_role(role) {
        return Err(
            EditError::new("invalid_role", format!("{role:?} is not an NDF edge type")).with(
                "roles",
                Value::Array(
                    EDGE_FIELDS
                        .iter()
                        .map(|field| Value::String((*field).to_owned()))
                        .collect(),
                ),
            ),
        );
    }
    match fields.iter_mut().find(|(name, _)| name == role) {
        Some((_, value)) => {
            let mut targets: Vec<&str> = value.split(',').map(str::trim).collect();
            if targets.contains(&target) {
                return Err(EditError::new(
                    "edge_exists",
                    format!("the clause already declares {role}={target}"),
                ));
            }
            targets.push(target);
            *value = targets.join(",");
        }
        None => fields.push((role.to_owned(), target.to_owned())),
    }
    Ok(())
}

/// Remove `target` from the clause's `role` list, or from whichever edge
/// list holds it when no role is named.
pub fn remove_target(
    fields: &mut Vec<(String, String)>,
    role: Option<&str>,
    target: &str,
) -> Result<(), EditError> {
    let position = fields.iter().position(|(name, value)| {
        role.is_none_or(|role| name == role)
            && is_edge_role(name)
            && value.split(',').map(str::trim).any(|item| item == target)
    });
    let Some(position) = position else {
        return Err(EditError::new(
            "edge_not_found",
            match role {
                Some(role) => format!("the clause declares no {role}={target}"),
                None => format!("the clause declares no edge to {target}"),
            },
        ));
    };
    let remaining: Vec<&str> = fields[position]
        .1
        .split(',')
        .map(str::trim)
        .filter(|item| *item != target)
        .collect();
    if remaining.is_empty() {
        fields.remove(position);
    } else {
        fields[position].1 = remaining.join(",");
    }
    Ok(())
}

/// Set a canonical or attribute field; an empty value removes it. The kind
/// cannot be removed, and an edge list is not a field (see `add_target`).
pub fn set_field(
    fields: &mut Vec<(String, String)>,
    name: &str,
    value: &str,
) -> Result<(), EditError> {
    let settable = CANONICAL_FIELDS.contains(&name) || ATTRIBUTE_FIELDS.contains(&name);
    if !settable {
        return Err(EditError::new(
            "invalid_parameter",
            format!("{name:?} is not a clause field"),
        )
        .with(
            "parameters",
            Value::Array(
                CANONICAL_FIELDS
                    .iter()
                    .chain(ATTRIBUTE_FIELDS)
                    .map(|field| Value::String((*field).to_owned()))
                    .collect(),
            ),
        ));
    }
    let value = value.trim();
    if value.contains(char::is_whitespace) {
        return Err(EditError::new(
            "invalid_value",
            format!("a {name} value cannot contain whitespace: {value:?}"),
        ));
    }
    if value.is_empty() {
        if name == "kind" {
            return Err(EditError::new("invalid_value", "a clause must have a kind"));
        }
        fields.retain(|(field, _)| field != name);
        return Ok(());
    }
    match fields.iter_mut().find(|(field, _)| field == name) {
        Some((_, current)) => *current = value.to_owned(),
        None => fields.push((name.to_owned(), value.to_owned())),
    }
    Ok(())
}

/// The metadata a new clause of `kind` starts with: a `tbd` modality for a
/// kind that states an obligation, `draft` status, and what the caller
/// adds (a layer, a domain).
pub fn new_clause_fields(kind: &str, extra: &[(String, String)]) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if NORMATIVE_KINDS.contains(&kind) {
        fields.push(("modality".to_owned(), "tbd".to_owned()));
    }
    fields.push(("status".to_owned(), "draft".to_owned()));
    for (name, value) in extra {
        if let Some(existing) = fields.iter_mut().find(|(field, _)| field == name) {
            existing.1 = value.clone();
        } else {
            fields.push((name.clone(), value.clone()));
        }
    }
    fields
}

// ---------------------------------------------------------------------------
// Names and ids across the project
// ---------------------------------------------------------------------------

/// The metadata value that names `name` as an edge target: a clause of this
/// project by id or by unique title (its local id), a clause of another
/// project (`project/ID` or `ndf://project/ID`, as a URI), or a bare id the
/// project does not have (verbatim: a dangling target one may still remove).
pub fn resolve_target(graph: &Graph, project: &str, name: &str) -> Result<String, EditError> {
    let name = name.trim();
    if let Some(rest) = name.strip_prefix("ndf://") {
        return match rest.split_once('/') {
            Some((other, id)) if other == project => Ok(id.to_owned()),
            Some(_) => Ok(name.to_owned()),
            None => Err(EditError::new(
                "node_not_found",
                format!("{name:?} is not a clause reference"),
            )),
        };
    }
    if let Some((other, id)) = name.split_once('/') {
        return Ok(if other == project {
            id.to_owned()
        } else {
            format!("ndf://{other}/{id}")
        });
    }
    if graph
        .nodes
        .iter()
        .any(|node| node.id.project_id() == project && node.id.local_id() == name)
    {
        return Ok(name.to_owned());
    }
    let by_title: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind != NodeKind::Document && node.id.project_id() == project && node.title == name
        })
        .map(|node| node.id.local_id())
        .collect();
    match by_title.as_slice() {
        [one] => Ok((*one).to_owned()),
        [] if looks_like_id(name) => Ok(name.to_owned()),
        [] => Err(EditError::new(
            "node_not_found",
            format!("no clause named {name:?} in project {project}"),
        )),
        many => Err(EditError::new(
            "ambiguous_name",
            format!(
                "{} clauses are titled {name:?}; name one by its id",
                many.len()
            ),
        )
        .with(
            "candidates",
            Value::Array(
                many.iter()
                    .map(|id| Value::String((*id).to_owned()))
                    .collect(),
            ),
        )),
    }
}

fn looks_like_id(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        && characters.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '-'
        })
}

/// The edges of the project that point at the clause, as `SOURCE (type)`.
pub fn dependents(graph: &Graph, project: &str, local_id: &str) -> Vec<String> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target.project_id() == project
                && edge.target.local_id() == local_id
                && edge.source.local_id() != local_id
        })
        .map(|edge| format!("{} ({})", edge.source.local_id(), edge.edge_type))
        .collect()
}

/// A free id for a new clause: `prefix` when given, else the prefix the
/// document's clauses use most, else the manifest's first prefix that is not
/// the document prefix. Numbered after the highest the project has.
pub fn allocate_clause_id(
    graph: &Graph,
    manifest: &ProjectManifest,
    document: &Document,
    prefix: Option<&str>,
) -> Result<String, EditError> {
    let prefix = match prefix.map(str::trim).filter(|prefix| !prefix.is_empty()) {
        Some(prefix) => prefix.to_owned(),
        None => {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for span in &document.clauses {
                if let Some(prefix) = id_prefix(&span.local_id) {
                    *counts.entry(prefix).or_default() += 1;
                }
            }
            counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(prefix, _)| prefix.to_owned())
                .or_else(|| {
                    manifest
                        .id_prefixes
                        .iter()
                        .find(|prefix| prefix.as_str() != "DOC")
                        .cloned()
                })
                .ok_or_else(|| {
                    EditError::new(
                        "no_prefix",
                        "no id prefix to allocate from: pass params.prefix or list one in ndf.yaml",
                    )
                })?
        }
    };
    let occupied = graph
        .nodes
        .iter()
        .filter(|node| node.id.project_id() == manifest.project)
        .map(|node| node.id.local_id().to_owned())
        .chain(document.clauses.iter().map(|span| span.local_id.clone()))
        .collect();
    allocate_id(&prefix, &occupied)
        .map_err(|error| EditError::new("invalid_prefix", error.to_string()))
}

/// The prefix of a numbered id: `FWD-ADM` for `FWD-ADM-001`.
fn id_prefix(local_id: &str) -> Option<&str> {
    let (prefix, number) = local_id.rsplit_once('-')?;
    (!number.is_empty() && number.chars().all(|character| character.is_ascii_digit()))
        .then_some(prefix)
}

// ---------------------------------------------------------------------------
// The guard
// ---------------------------------------------------------------------------

/// Refuse a result that parses worse than the original: every error the new
/// text raises that the old one did not is a reason not to write it.
pub fn verify(
    before: &str,
    after: &str,
    relative_path: &str,
    manifest: &ProjectManifest,
) -> Result<(), EditError> {
    let parsed_before = parse_markdown_text(before, relative_path, manifest)
        .map_err(|error| EditError::new("invalid_result", error.to_string()))?;
    let parsed_after = parse_markdown_text(after, relative_path, manifest)
        .map_err(|error| EditError::new("invalid_result", error.to_string()))?;
    let mut known: BTreeMap<(String, String), usize> = BTreeMap::new();
    for diagnostic in parsed_before
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        *known
            .entry((diagnostic.code.clone(), diagnostic.message.clone()))
            .or_default() += 1;
    }
    for diagnostic in parsed_after
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        let key = (diagnostic.code.clone(), diagnostic.message.clone());
        match known.get_mut(&key) {
            Some(count) if *count > 0 => *count -= 1,
            _ => {
                return Err(EditError::new(
                    "invalid_result",
                    format!(
                        "the edit would not parse: {} at line {}: {}",
                        diagnostic.code, diagnostic.primary.line, diagnostic.message
                    ),
                )
                .with("code", Value::String(diagnostic.code.clone()))
                .with("line", Value::from(diagnostic.primary.line)));
            }
        }
    }
    Ok(())
}

/// The names a consumer's endpoint argument may take: `{ actor, port }`,
/// `{ type: 'workflow', name }`, or `Name.port` as text.
pub fn endpoint_name(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = text.trim().trim_matches('\'').trim_matches('"');
            let name = match text.rsplit_once('.') {
                Some((name, port)) if port == "in" || port == "out" => name,
                _ => text,
            };
            (!name.is_empty()).then(|| name.to_owned())
        }
        Value::Object(map) => ["actor", "entity", "node", "name"]
            .iter()
            .find_map(|key| map.get(*key).and_then(Value::as_str))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        _ => None,
    }
}

/// The edge roles as a capabilities listing: `[{ id, label }]`.
pub fn edge_roles_value() -> Value {
    Value::Array(
        EDGE_ROLES
            .iter()
            .map(|(id, label)| json!({ "id": id, "label": label }))
            .collect(),
    )
}
