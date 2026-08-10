use std::collections::BTreeMap;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

use crate::identity::NodeId;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Document,
    Requirement,
    Definition,
    Architecture,
    Constraint,
    Option,
    Verification,
    Decision,
    Question,
    Model,
    Source,
    Test,
    Evidence,
    Information,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modality {
    Must,
    Should,
    May,
    Tbd,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleStatus {
    Draft,
    Active,
    Deprecated,
    Superseded,
    Retired,
    Open,
    Resolved,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Information,
    Warning,
    Error,
}

#[derive(Debug, Clone, Eq, Error, PartialEq)]
pub enum ModelError {
    #[error("line must be positive")]
    InvalidLine,
    #[error("column must be positive")]
    InvalidColumn,
    #[error("end line must not precede start line")]
    InvalidEndLine,
    #[error("end column must be positive")]
    InvalidEndColumn,
    #[error("edge type must not be empty")]
    EmptyEdgeType,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SourceSpan {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

impl SourceSpan {
    pub fn new(path: impl Into<String>, line: u32) -> Result<Self, ModelError> {
        Self::with_bounds(path, line, 1, None, None)
    }

    pub fn with_bounds(
        path: impl Into<String>,
        line: u32,
        column: u32,
        end_line: Option<u32>,
        end_column: Option<u32>,
    ) -> Result<Self, ModelError> {
        if line == 0 {
            return Err(ModelError::InvalidLine);
        }
        if column == 0 {
            return Err(ModelError::InvalidColumn);
        }
        if end_line.is_some_and(|end| end < line) {
            return Err(ModelError::InvalidEndLine);
        }
        if end_column == Some(0) {
            return Err(ModelError::InvalidEndColumn);
        }
        Ok(Self {
            path: path.into(),
            line,
            column,
            end_line,
            end_column,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub title: String,
    pub source: SourceSpan,
    pub modality: Option<Modality>,
    pub refinement: Option<String>,
    pub domain: Option<String>,
    pub status: LifecycleStatus,
    pub owner: Option<String>,
    pub body: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    #[serde(rename = "type")]
    pub edge_type: String,
    pub source_span: Option<SourceSpan>,
    pub attributes: BTreeMap<String, String>,
}

impl Edge {
    pub fn new(
        source: NodeId,
        target: NodeId,
        edge_type: impl Into<String>,
        source_span: Option<SourceSpan>,
    ) -> Result<Self, ModelError> {
        let edge_type = edge_type.into();
        if edge_type.trim().is_empty() {
            return Err(ModelError::EmptyEdgeType);
        }
        Ok(Self {
            source,
            target,
            edge_type,
            source_span,
            attributes: BTreeMap::new(),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub primary: SourceSpan,
    pub related: Vec<SourceSpan>,
    pub repair: Option<String>,
    pub subject: Option<NodeId>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Serialize for Graph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Graph", 4)?;
        state.serialize_field("schema_version", "0.1")?;
        state.serialize_field("nodes", &self.nodes)?;
        state.serialize_field("edges", &self.edges)?;
        state.serialize_field("diagnostics", &self.diagnostics)?;
        state.end()
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ParseResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagnostics: Vec<Diagnostic>,
}
