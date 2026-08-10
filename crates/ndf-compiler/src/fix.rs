use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ndf_core::model::{Diagnostic, DiagnosticSeverity, SourceSpan};
use serde::Serialize;

use crate::CompilerError;

const SEMANTIC_FIELDS: &[&str] = &[
    "prose",
    "authority",
    "status",
    "kind",
    "modality",
    "refinement",
    "domain",
    "edges",
];

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct FixMode {
    pub generated_only: bool,
    pub check: bool,
    pub requested_changes: BTreeSet<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct FixResult {
    pub changed: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn safe_fix(paths: &[PathBuf], mode: &FixMode) -> Result<FixResult, CompilerError> {
    let forbidden: BTreeSet<_> = mode
        .requested_changes
        .iter()
        .filter(|field| SEMANTIC_FIELDS.contains(&field.as_str()))
        .cloned()
        .collect();
    if !forbidden.is_empty() {
        return Ok(FixResult {
            changed: Vec::new(),
            diagnostics: vec![refusal(&forbidden)],
        });
    }

    let mut ordered = paths.to_vec();
    ordered.sort();
    let mut changed = Vec::new();
    for path in ordered {
        let content = fs::read_to_string(&path).map_err(|source| CompilerError::Read {
            path: path.clone(),
            source,
        })?;
        let mut updated = sort_generated_blocks(&content);
        if !mode.generated_only {
            updated = sort_metadata(&updated);
        }
        if updated == content {
            continue;
        }
        changed.push(path.to_string_lossy().into_owned());
        if !mode.check {
            fs::write(&path, updated).map_err(|source| CompilerError::Write {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(FixResult {
        changed,
        diagnostics: Vec::new(),
    })
}

fn refusal(fields: &BTreeSet<String>) -> Diagnostic {
    Diagnostic {
        code: "NDF-FIX-001".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!(
            "safe fix cannot alter semantic fields: {}",
            fields.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
        primary: SourceSpan::new("<command>", 1).expect("line one is valid"),
        related: Vec::new(),
        repair: Some("Edit normative semantics explicitly and request review.".to_owned()),
        subject: None,
    }
}

fn sort_metadata(content: &str) -> String {
    content
        .split_inclusive('\n')
        .map(sort_metadata_line)
        .collect()
}

fn sort_metadata_line(line: &str) -> String {
    let (body, newline) = line
        .strip_suffix('\n')
        .map_or((line, ""), |body| (body, "\n"));
    let leading_len = body.len() - body.trim_start().len();
    let leading = &body[..leading_len];
    let candidate = &body[leading_len..];
    let Some(after_prefix) = candidate.strip_prefix("<!-- ndf:") else {
        return line.to_owned();
    };
    let Some(end) = after_prefix.rfind("-->") else {
        return line.to_owned();
    };
    if !after_prefix[end + 3..].chars().all(char::is_whitespace) {
        return line.to_owned();
    }
    let trailing = &after_prefix[end + 3..];
    let mut tokens: Vec<_> = after_prefix[..end].split_whitespace().collect();
    tokens.sort_unstable();
    format!(
        "{leading}<!-- ndf: {} -->{trailing}{newline}",
        tokens.join(" ")
    )
}

fn sort_generated_blocks(content: &str) -> String {
    const START: &str = "<!-- ndf:index:start -->";
    const END: &str = "<!-- ndf:index:end -->";
    let mut position = 0;
    let mut output = String::new();
    loop {
        let Some(relative_start) = content[position..].find(START) else {
            output.push_str(&content[position..]);
            break;
        };
        let start = position + relative_start;
        let body_start = start + START.len();
        let Some(relative_end) = content[body_start..].find(END) else {
            output.push_str(&content[position..]);
            break;
        };
        let end = body_start + relative_end;
        let mut lines: Vec<_> = content[body_start..end]
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        lines.sort_unstable();
        output.push_str(&content[position..body_start]);
        output.push('\n');
        output.push_str(&lines.join("\n"));
        output.push('\n');
        position = end;
    }
    output
}

pub fn markdown_paths(root: &Path) -> Result<Vec<PathBuf>, CompilerError> {
    let mut pending = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|source| CompilerError::Read {
            path: directory.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| CompilerError::Read {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}
