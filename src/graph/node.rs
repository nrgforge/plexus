//! Node representation in the knowledge graph

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for a node
///
/// Serializes as a plain string (UUID or semantic ID like "agent:security-reviewer")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(String);

impl NodeId {
    /// Create a new random NodeId (UUID-based)
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a NodeId from a string (semantic ID)
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Content type classification
///
/// Matches the contract schema: lowercase string enum.
/// For content types with subtypes (e.g., code language), use properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Source code
    Code,
    /// Movement/gesture data
    Movement,
    /// Narrative/text content
    Narrative,
    /// Abstract concept
    Concept,
    /// Document (markdown, etc.)
    Document,
    /// Agent definition
    Agent,
}

impl FromStr for ContentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "code" => Ok(ContentType::Code),
            "movement" => Ok(ContentType::Movement),
            "narrative" => Ok(ContentType::Narrative),
            "concept" => Ok(ContentType::Concept),
            "document" => Ok(ContentType::Document),
            "agent" => Ok(ContentType::Agent),
            _ => Err(format!("Unknown content type: {}", s)),
        }
    }
}

/// Typed property values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<PropertyValue>),
    Object(HashMap<String, PropertyValue>),
}

/// Properties collection
pub type Properties = HashMap<String, PropertyValue>;

/// Node metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// When the node was created
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When the node was last modified
    pub modified_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Source location (file path, line number, etc.)
    pub source: Option<String>,
    /// Version identifier
    pub version: Option<String>,
}

/// A node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier
    pub id: NodeId,
    /// Type of node within content domain (e.g., "function", "class", "pose")
    pub node_type: String,
    /// Primary content domain
    pub content_type: ContentType,
    /// Domain-specific properties
    pub properties: Properties,
    /// Node metadata
    pub metadata: NodeMetadata,
}

impl Node {
    /// Create a new node with the given type and content type
    pub fn new(node_type: impl Into<String>, content_type: ContentType) -> Self {
        Self {
            id: NodeId::new(),
            node_type: node_type.into(),
            content_type,
            properties: HashMap::new(),
            metadata: NodeMetadata {
                created_at: Some(chrono::Utc::now()),
                ..Default::default()
            },
        }
    }

    /// Add a property to the node
    pub fn with_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Set the source location
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.metadata.source = Some(source.into());
        self
    }
}
