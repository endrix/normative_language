use std::fmt::{Display, Formatter};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Clone, Eq, Error, PartialEq)]
pub enum IdentityError {
    #[error("invalid project ID: '{0}'")]
    InvalidProject(String),
    #[error("invalid local ID: '{0}'")]
    InvalidLocal(String),
    #[error("current project is required for a local NDF ID")]
    MissingCurrentProject,
    #[error("invalid NDF URI: '{0}'")]
    InvalidUri(String),
}

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId {
    project_id: String,
    local_id: String,
}

impl NodeId {
    pub fn new(
        project_id: impl Into<String>,
        local_id: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        let project_id = project_id.into();
        let local_id = local_id.into();
        validate_project_id(&project_id)?;
        validate_local_id(&local_id)?;
        Ok(Self {
            project_id,
            local_id,
        })
    }

    pub fn parse(value: &str, current_project: Option<&str>) -> Result<Self, IdentityError> {
        let Some(rest) = value.strip_prefix("ndf://") else {
            let project = current_project.ok_or(IdentityError::MissingCurrentProject)?;
            return Self::new(project, value);
        };
        if value.contains('?') || value.contains('#') {
            return Err(IdentityError::InvalidUri(value.to_owned()));
        }
        let Some((project, local_id)) = rest.split_once('/') else {
            return Err(IdentityError::InvalidUri(value.to_owned()));
        };
        if project.is_empty() || local_id.is_empty() || local_id.contains('/') {
            return Err(IdentityError::InvalidUri(value.to_owned()));
        }
        Self::new(project, local_id)
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn local_id(&self) -> &str {
        &self.local_id
    }

    pub fn uri(&self) -> String {
        format!("ndf://{}/{}", self.project_id, self.local_id)
    }
}

impl Display for NodeId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.uri())
    }
}

impl Serialize for NodeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.uri())
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value, None).map_err(serde::de::Error::custom)
    }
}

fn validate_project_id(value: &str) -> Result<(), IdentityError> {
    let mut characters = value.chars();
    if !characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        || !characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
    {
        return Err(IdentityError::InvalidProject(value.to_owned()));
    }
    Ok(())
}

fn validate_local_id(value: &str) -> Result<(), IdentityError> {
    let mut characters = value.chars();
    if !characters
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric())
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
    {
        return Err(IdentityError::InvalidLocal(value.to_owned()));
    }
    Ok(())
}
