//! Provenance data types for MCP responses and chain/mark modeling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a provenance chain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainStatus {
    Active,
    Archived,
}

impl std::str::FromStr for ChainStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("unknown chain status: {}", s)),
        }
    }
}

/// View of a chain (returned by API methods)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainView {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: ChainStatus,
    pub created_at: DateTime<Utc>,
}

/// View of a mark (returned by API methods)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkView {
    pub id: String,
    pub chain_id: String,
    pub file: String,
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    pub annotation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    pub created_at: DateTime<Utc>,
}
