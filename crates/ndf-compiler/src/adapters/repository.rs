use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use globset::{Glob, GlobSetBuilder};
use ndf_core::identity::NodeId;
use ndf_core::model::{Edge, LifecycleStatus, Node, NodeKind, ParseResult, SourceSpan};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::CompilerError;
use crate::manifest::ProjectManifest;

const EDGE_TYPES: &[&str] = &["implements", "verifies"];

pub fn index_repository_paths(
    root: &Path,
    globs: &[&str],
    project: &ProjectManifest,
) -> Result<ParseResult, CompilerError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in globs {
        builder.add(Glob::new(pattern).map_err(|error| {
            CompilerError::Contract(format!("invalid glob {pattern}: {error}"))
        })?);
    }
    let matcher = builder
        .build()
        .map_err(|error| CompilerError::Contract(format!("invalid repository globs: {error}")))?;
    let mut paths = BTreeSet::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| {
            CompilerError::Contract(format!("failed to walk {}: {error}", root.display()))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("walked path is inside root");
        if matcher.is_match(relative) {
            paths.insert(relative.to_path_buf());
        }
    }

    let revision = revision(root);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for relative_path in paths {
        let path = root.join(&relative_path);
        let relative = relative_path.to_string_lossy().replace('\\', "/");
        let content = fs::read_to_string(&path).map_err(|source| CompilerError::Read {
            path: path.clone(),
            source,
        })?;
        let kind = kind(&relative_path);
        let node = Node {
            id: NodeId::new(
                &project.project,
                derived_id(&project.project, &relative, kind),
            )
            .map_err(|error| CompilerError::Contract(error.to_string()))?,
            kind,
            title: relative.clone(),
            source: SourceSpan::new(&relative, 1).expect("line one is valid"),
            modality: None,
            refinement: None,
            domain: None,
            status: LifecycleStatus::Active,
            owner: Some(project.project.clone()),
            body: String::new(),
            attributes: BTreeMap::from([
                ("path".to_owned(), relative),
                ("revision".to_owned(), revision.clone()),
                ("content_hash".to_owned(), sha256(&content)),
            ]),
        };
        edges.extend(edges_for(&node, &content, project)?);
        nodes.push(node);
    }
    Ok(ParseResult {
        nodes,
        edges,
        diagnostics: Vec::new(),
    })
}

fn revision(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output();
    let value = output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default();
    if exact_revision(&value) {
        value
    } else {
        "0".repeat(40)
    }
}

fn kind(path: &Path) -> NodeKind {
    if path.components().any(|part| part.as_os_str() == "tests")
        || path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("test_"))
    {
        NodeKind::Test
    } else {
        NodeKind::Source
    }
}

fn derived_id(project: &str, path: &str, kind: NodeKind) -> String {
    let kind = if kind == NodeKind::Test {
        "test"
    } else {
        "source"
    };
    let digest = sha256(&format!("{project}\0{kind}\0{path}"));
    let prefix = if kind == "test" { "TEST" } else { "SRC" };
    format!("{prefix}-{}", digest[..16].to_ascii_uppercase())
}

fn edges_for(
    node: &Node,
    content: &str,
    project: &ProjectManifest,
) -> Result<Vec<Edge>, CompilerError> {
    let mut result = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        let comment = trimmed
            .strip_prefix('#')
            .or_else(|| trimmed.strip_prefix("//"));
        let Some(payload) = comment
            .map(str::trim_start)
            .and_then(|comment| comment.strip_prefix("ndf:"))
        else {
            continue;
        };
        for token in payload.split_whitespace() {
            let Some((edge_type, targets)) = token.split_once('=') else {
                continue;
            };
            if !EDGE_TYPES.contains(&edge_type) {
                continue;
            }
            for target in targets.split(',') {
                let target = NodeId::parse(target, Some(&project.project))
                    .map_err(|error| CompilerError::Contract(error.to_string()))?;
                result.push(
                    Edge::new(
                        node.id.clone(),
                        target,
                        edge_type,
                        Some(
                            SourceSpan::new(&node.source.path, (index + 1) as u32)
                                .expect("repository line is positive"),
                        ),
                    )
                    .expect("registered edge type is non-empty"),
                );
            }
        }
    }
    Ok(result)
}

fn sha256(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn exact_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}
