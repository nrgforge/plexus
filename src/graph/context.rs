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

/// A source (file, directory, or URL) belonging to a context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source {
    File { path: String },
    Directory { path: String, recursive: bool },
    Url { url: String },
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
    /// Sources (files, directories, URLs) belonging to this context
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
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

    /// Create a new context with a specific ID and name
    pub fn with_id(id: ContextId, name: impl Into<String>) -> Self {
        Self {
            id,
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
    ///
    /// Uses a hybrid deduplication strategy:
    /// - **Dimension-distinct**: Edges with same source/target/relationship but different
    ///   dimensions are stored as separate edges (preserves multi-dimensional richness)
    /// - **Cross-dimensional reinforcement**: When the same logical edge (source/target/relationship)
    ///   appears in multiple dimensions, all instances get a boosted confidence score
    ///   (Hebbian learning: evidence from multiple perspectives reinforces the connection)
    ///
    /// Properties set on edges:
    /// - `_cross_dim_count`: Number of dimensions this edge appears in (1 = single dimension)
    /// - When count > 1, confidence is boosted by 0.1 per additional dimension (capped at 1.0)
    pub fn add_edge(&mut self, edge: Edge) {
        use super::PropertyValue;

        // Single-pass scan: find exact match index and count cross-dimensional matches
        let mut exact_match_idx: Option<usize> = None;
        let mut cross_dim_indices: Vec<usize> = Vec::new();

        for (i, e) in self.edges.iter().enumerate() {
            let same_logical = e.source == edge.source
                && e.target == edge.target
                && e.relationship == edge.relationship;

            if same_logical {
                let same_dimensions = e.source_dimension == edge.source_dimension
                    && e.target_dimension == edge.target_dimension;

                if same_dimensions {
                    exact_match_idx = Some(i);
                    break; // Found exact match, no need to continue
                } else {
                    cross_dim_indices.push(i);
                }
            }
        }

        if let Some(idx) = exact_match_idx {
            // Exact duplicate - update existing edge
            let existing = &mut self.edges[idx];
            existing.strength = existing.strength.max(edge.strength);
            existing.last_reinforced = edge.last_reinforced;
            for (k, v) in edge.properties {
                existing.properties.insert(k, v);
            }
        } else {
            let cross_dim_count = cross_dim_indices.len();
            let mut new_edge = edge;

            if cross_dim_count > 0 {
                // This logical edge exists in other dimensions - apply Hebbian reinforcement
                let reinforcement_bonus = 0.1 * (cross_dim_count as f32);

                // Update existing edges with this logical relationship
                for &idx in &cross_dim_indices {
                    let existing = &mut self.edges[idx];
                    existing.confidence = (existing.confidence + 0.1).min(1.0);
                    existing.properties.insert(
                        "_cross_dim_count".to_string(),
                        PropertyValue::Int((cross_dim_count + 1) as i64),
                    );
                }

                // Set properties on new edge
                new_edge.confidence = (new_edge.confidence + reinforcement_bonus).min(1.0);
                new_edge.properties.insert(
                    "_cross_dim_count".to_string(),
                    PropertyValue::Int((cross_dim_count + 1) as i64),
                );
            }

            self.edges.push(new_edge);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Edge, Node, PropertyValue};

    #[test]
    fn test_add_edge_exact_duplicate_updates_existing() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        // Add first edge
        let mut edge1 = Edge::new(id_a.clone(), id_b.clone(), "calls");
        edge1.strength = 0.5;
        ctx.add_edge(edge1);

        // Add exact duplicate with higher strength
        let mut edge2 = Edge::new(id_a.clone(), id_b.clone(), "calls");
        edge2.strength = 0.8;
        ctx.add_edge(edge2);

        // Should only have one edge, with the higher strength
        assert_eq!(ctx.edge_count(), 1);
        assert_eq!(ctx.edges[0].strength, 0.8);
    }

    #[test]
    fn test_add_edge_different_dimensions_creates_multiple() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        // Add edge in structure dimension
        let edge1 = Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "structure");
        ctx.add_edge(edge1);

        // Add same logical edge in semantic dimension
        let edge2 = Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "semantic");
        ctx.add_edge(edge2);

        // Should have two distinct edges
        assert_eq!(ctx.edge_count(), 2);
    }

    #[test]
    fn test_cross_dimensional_reinforcement_boosts_confidence() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        // Add edge in structure dimension (confidence starts at 0)
        let edge1 = Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "structure");
        ctx.add_edge(edge1);
        assert_eq!(ctx.edges[0].confidence, 0.0);

        // Add same logical edge in semantic dimension
        let edge2 = Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "semantic");
        ctx.add_edge(edge2);

        // Both edges should now have boosted confidence and cross_dim_count
        assert_eq!(ctx.edge_count(), 2);

        // First edge should have been updated with +0.1 confidence
        assert!(ctx.edges[0].confidence > 0.0, "First edge should have boosted confidence");

        // Second edge should have cross_dim_count = 2
        let count = ctx.edges[1].properties.get("_cross_dim_count");
        assert!(count.is_some(), "Should have _cross_dim_count property");
        if let Some(PropertyValue::Int(n)) = count {
            assert_eq!(*n, 2, "cross_dim_count should be 2");
        }
    }

    #[test]
    fn test_cross_dimensional_reinforcement_increments_with_more_dimensions() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        // Add edge in structure dimension
        ctx.add_edge(Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "structure"));

        // Add in semantic dimension
        ctx.add_edge(Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "semantic"));

        // Add in relational dimension
        ctx.add_edge(Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "relational"));

        assert_eq!(ctx.edge_count(), 3);

        // The third edge should have cross_dim_count = 3
        let count = ctx.edges[2].properties.get("_cross_dim_count");
        if let Some(PropertyValue::Int(n)) = count {
            assert_eq!(*n, 3, "cross_dim_count should be 3 for third dimension");
        }

        // Confidence should be boosted (0.1 * 2 = 0.2 for third edge)
        assert!(ctx.edges[2].confidence >= 0.2, "Third edge should have 0.2 confidence boost");
    }
}
