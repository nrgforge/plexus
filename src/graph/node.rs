//! Node representation in the knowledge graph

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl NodeId {
    /// Create a new random NodeId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a NodeId from an existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
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

/// Content type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ContentType {
    /// Source code with language specification
    Code(String),
    /// Movement/gesture data
    Movement(String),
    /// Narrative/text content
    Narrative,
    /// Abstract concept
    Concept,
    /// Document (markdown, etc.)
    Document,
    /// Agent definition
    Agent,
    /// Custom content type
    Custom(String),
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
