//! Context: A bounded subgraph representing a workspace or project

use super::edge::Edge;
use super::node::{Node, NodeId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a context
///
/// Serializes as a plain string (UUID or semantic ID like "ctx:workspace-name")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextId(String);

impl ContextId {
    /// Create a new random ContextId (UUID-based)
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a ContextId from a string (semantic ID)
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ContextId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ContextId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ContextId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ContextId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Metadata about a context
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// When the context was created
    pub created_at: Option<DateTime<Utc>>,
    /// When the context was last updated
    pub updated_at: Option<DateTime<Utc>>,
    /// Owner/creator of the context
    pub owner: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// A bounded subgraph representing a workspace or project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier
    pub id: ContextId,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Nodes in this context
    pub nodes: HashMap<NodeId, Node>,
    /// Edges in this context
    pub edges: Vec<Edge>,
    /// Context metadata
    pub metadata: ContextMetadata,
}

impl Context {
    /// Create a new context with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: ContextId::new(),
            name: name.into(),
            description: None,
            nodes: HashMap::new(),
            edges: Vec::new(),
            metadata: ContextMetadata {
                created_at: Some(Utc::now()),
                ..Default::default()
            },
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.metadata.tags.push(tag.into());
        self
    }

    /// Add a node to the context
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = node.id.clone();
        self.nodes.insert(id.clone(), node);
        self.touch();
        id
    }

    /// Add an edge to the context
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
        self.touch();
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get a mutable reference to a node
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Get all nodes
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Get all edges
    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.iter()
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Update the last modified timestamp
    fn touch(&mut self) {
        self.metadata.updated_at = Some(Utc::now());
    }
}
