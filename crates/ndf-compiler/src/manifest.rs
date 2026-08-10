use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml_ng::{Mapping, Value};

use crate::CompilerError;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DependencyDeclaration {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectManifest {
    pub format_version: String,
    pub project: String,
    pub roots: Vec<String>,
    pub id_prefixes: Vec<String>,
    pub domains: Vec<String>,
    pub policies: BTreeMap<String, Value>,
    pub root: PathBuf,
    pub dependencies: BTreeMap<String, DependencyDeclaration>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LockedDependency {
    pub project: String,
    pub uri: String,
    pub revision: String,
    pub path: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProjectLock {
    pub format_version: String,
    pub dependencies: Vec<LockedDependency>,
}

impl ProjectManifest {
    pub fn load(path: &Path) -> Result<Self, CompilerError> {
        let mapping = load_mapping(path)?;
        reject_unknown(
            &mapping,
            &[
                "format_version",
                "project",
                "roots",
                "id_prefixes",
                "domains",
                "policies",
                "dependencies",
            ],
            "manifest",
        )?;
        let format_version = required_string(&mapping, "format_version")?;
        let project = required_string(&mapping, "project")?;
        let roots = required_strings(&mapping, "roots")?;
        let id_prefixes = required_strings(&mapping, "id_prefixes")?;
        let domains = optional_strings(&mapping, "domains")?;
        let policies = optional_named_mapping(&mapping, "policies")?;
        let dependency_values = optional_named_mapping(&mapping, "dependencies")?;
        let mut dependencies = BTreeMap::new();
        for (name, value) in dependency_values {
            let declaration = value
                .as_mapping()
                .ok_or_else(|| contract("each dependency declaration must be a named mapping"))?;
            reject_unknown(declaration, &["path"], "dependency declaration")?;
            if declaration.len() != 1 {
                return Err(contract(
                    "dependency declaration must contain only a string path",
                ));
            }
            dependencies.insert(
                name,
                DependencyDeclaration {
                    path: required_string(declaration, "path")?,
                },
            );
        }
        Ok(Self {
            format_version,
            project,
            roots,
            id_prefixes,
            domains,
            policies,
            root: path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .canonicalize()
                .map_err(|source| CompilerError::Read {
                    path: path.to_path_buf(),
                    source,
                })?,
            dependencies,
        })
    }

    pub fn for_test<const P: usize, const D: usize>(
        project: &str,
        root: &Path,
        id_prefixes: [&str; P],
        domains: [&str; D],
    ) -> Self {
        Self {
            format_version: "0.2".to_owned(),
            project: project.to_owned(),
            roots: vec!["*.md".to_owned()],
            id_prefixes: id_prefixes.into_iter().map(str::to_owned).collect(),
            domains: domains.into_iter().map(str::to_owned).collect(),
            policies: BTreeMap::new(),
            root: root.to_path_buf(),
            dependencies: BTreeMap::new(),
        }
    }
}

impl ProjectLock {
    pub fn load(path: &Path) -> Result<Self, CompilerError> {
        let mapping = load_mapping(path)?;
        reject_unknown(&mapping, &["format_version", "dependencies"], "lock")?;
        let format_version = required_string(&mapping, "format_version")?;
        let dependency_values = required_named_mapping(&mapping, "dependencies")?;
        let mut dependencies = Vec::new();
        for (project, value) in dependency_values {
            let fields = value
                .as_mapping()
                .ok_or_else(|| contract("each dependency must be a named mapping"))?;
            reject_unknown(fields, &["uri", "revision", "path"], "dependency")?;
            let uri = required_string(fields, "uri")?;
            let revision = required_string(fields, "revision")?;
            let dependency_path = required_string(fields, "path")?;
            if revision.len() != 40
                || !revision
                    .chars()
                    .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
            {
                return Err(contract(
                    "dependency revision must be 40 lowercase hexadecimal characters",
                ));
            }
            dependencies.push(LockedDependency {
                project,
                uri,
                revision,
                path: dependency_path,
            });
        }
        Ok(Self {
            format_version,
            dependencies,
        })
    }
}

fn contract(message: impl Into<String>) -> CompilerError {
    CompilerError::Contract(message.into())
}

fn load_mapping(path: &Path) -> Result<Mapping, CompilerError> {
    let text = fs::read_to_string(path).map_err(|source| CompilerError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let value: Value = serde_yaml_ng::from_str(&text).map_err(|source| CompilerError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    value
        .as_mapping()
        .cloned()
        .ok_or_else(|| contract(format!("expected a YAML mapping in {}", path.display())))
}

fn key(name: &str) -> Value {
    Value::String(name.to_owned())
}

fn required<'a>(mapping: &'a Mapping, name: &str) -> Result<&'a Value, CompilerError> {
    mapping
        .get(key(name))
        .ok_or_else(|| contract(format!("missing required field: {name}")))
}

fn required_string(mapping: &Mapping, name: &str) -> Result<String, CompilerError> {
    required(mapping, name)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| contract(format!("{name} must be a string")))
}

fn required_strings(mapping: &Mapping, name: &str) -> Result<Vec<String>, CompilerError> {
    string_list(required(mapping, name)?, name)
}

fn optional_strings(mapping: &Mapping, name: &str) -> Result<Vec<String>, CompilerError> {
    mapping
        .get(key(name))
        .map_or_else(|| Ok(Vec::new()), |value| string_list(value, name))
}

fn string_list(value: &Value, name: &str) -> Result<Vec<String>, CompilerError> {
    let items = value
        .as_sequence()
        .ok_or_else(|| contract(format!("{name} must be a list of strings")))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| contract(format!("{name} must be a list of strings")))
        })
        .collect()
}

fn required_named_mapping(
    mapping: &Mapping,
    name: &str,
) -> Result<BTreeMap<String, Value>, CompilerError> {
    named_mapping(required(mapping, name)?, name)
}

fn optional_named_mapping(
    mapping: &Mapping,
    name: &str,
) -> Result<BTreeMap<String, Value>, CompilerError> {
    mapping
        .get(key(name))
        .map_or_else(|| Ok(BTreeMap::new()), |value| named_mapping(value, name))
}

fn named_mapping(value: &Value, name: &str) -> Result<BTreeMap<String, Value>, CompilerError> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| contract(format!("{name} must be a mapping")))?;
    mapping
        .iter()
        .map(|(key, value)| {
            key.as_str()
                .map(|name| (name.to_owned(), value.clone()))
                .ok_or_else(|| contract(format!("{name} keys must be strings")))
        })
        .collect()
}

fn reject_unknown(mapping: &Mapping, allowed: &[&str], context: &str) -> Result<(), CompilerError> {
    let mut unknown: Vec<_> = mapping
        .keys()
        .filter_map(Value::as_str)
        .filter(|name| !allowed.contains(name))
        .collect();
    unknown.sort_unstable();
    if let Some(name) = unknown.first() {
        let label = match context {
            "manifest" => "unsupported manifest field",
            "lock" => "unsupported lock field",
            "dependency" => "unsupported dependency field",
            _ => "unsupported dependency declaration field",
        };
        return Err(contract(format!("{label}: {name}")));
    }
    Ok(())
}
