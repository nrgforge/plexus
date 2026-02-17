//! Context: A bounded subgraph representing a workspace or project

use super::edge::Edge;
use super::node::{Node, NodeId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::BTreeMap;
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

/// A source (file, directory, URL, or context reference) belonging to a context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source {
    File { path: String },
    Directory { path: String, recursive: bool },
    Url { url: String },
    /// Reference to another context (for nested contexts)
    ContextRef { context_id: String },
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
    /// Application-specific properties (generic key-value bag)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
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
    /// - **Exact duplicate**: When the same edge (source/target/relationship/dimensions) already
    ///   exists, contributions are merged per-adapter-slot (ADR-003) and properties merge.
    ///   For edges without contributions, raw_weight falls back to max for backward compat.
    /// - **Cross-dimensional**: When the same logical edge appears in multiple dimensions,
    ///   a `_cross_dim_count` property tracks how many dimensions it spans.
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
                    break;
                } else {
                    cross_dim_indices.push(i);
                }
            }
        }

        if let Some(idx) = exact_match_idx {
            // Exact duplicate - merge contributions per-adapter (ADR-003)
            let existing = &mut self.edges[idx];
            for (adapter_id, value) in &edge.contributions {
                existing.contributions.insert(adapter_id.clone(), *value);
            }
            // raw_weight: for edges with contributions, the caller is responsible
            // for calling recompute_raw_weights() after all edges are committed.
            // For edges without contributions, fall back to max for backward compat.
            if edge.contributions.is_empty() {
                existing.raw_weight = existing.raw_weight.max(edge.raw_weight);
            }
            for (k, v) in edge.properties {
                existing.properties.insert(k, v);
            }
        } else {
            let cross_dim_count = cross_dim_indices.len();
            let mut new_edge = edge;

            if cross_dim_count > 0 {
                // Track cross-dimensional presence
                for &idx in &cross_dim_indices {
                    let existing = &mut self.edges[idx];
                    existing.properties.insert(
                        "_cross_dim_count".to_string(),
                        PropertyValue::Int((cross_dim_count + 1) as i64),
                    );
                }

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

    /// Floor coefficient for dynamic epsilon in scale normalization (ADR-005).
    /// The weakest real contribution maps to α/(1+α) ≈ 0.0099, not 0.0.
    const FLOOR_ALPHA: f32 = 0.01;

    /// Recompute raw_weight on all edges using scale normalization (ADR-003, ADR-005).
    ///
    /// For each adapter, computes min and max contribution values across all edges.
    /// Each contribution is scale-normalized with dynamic epsilon (ADR-005):
    ///   `(value - min + α·range) / ((1 + α)·range)`
    /// where α = 0.01 (floor coefficient). This maps minimum to ~0.0099, not 0.0.
    /// Degenerate case (min == max) normalizes to 1.0.
    /// raw_weight = sum of scale-normalized contributions across all adapters.
    pub fn recompute_raw_weights(&mut self) {
        use std::collections::HashMap;

        if self.edges.is_empty() {
            return;
        }

        // Collect per-adapter min/max across all edges
        let mut adapter_ranges: HashMap<String, (f32, f32)> = HashMap::new();
        for edge in &self.edges {
            for (adapter_id, value) in &edge.contributions {
                let entry = adapter_ranges.entry(adapter_id.clone()).or_insert((*value, *value));
                if *value < entry.0 {
                    entry.0 = *value;
                }
                if *value > entry.1 {
                    entry.1 = *value;
                }
            }
        }

        let alpha = Self::FLOOR_ALPHA;

        // Recompute raw_weight for each edge
        for edge in &mut self.edges {
            if edge.contributions.is_empty() {
                continue; // Leave raw_weight as-is for edges without contributions
            }

            let mut sum = 0.0f32;
            for (adapter_id, value) in &edge.contributions {
                if let Some(&(min, max)) = adapter_ranges.get(adapter_id) {
                    let range = max - min;
                    let normalized = if range == 0.0 {
                        1.0 // Degenerate case: single value → 1.0
                    } else {
                        // ADR-005: dynamic epsilon proportional to range
                        (value - min + alpha * range) / ((1.0 + alpha) * range)
                    };
                    sum += normalized;
                }
            }
            edge.raw_weight = sum;
        }
    }

    /// Retract all contributions from a named adapter/enrichment (ADR-027).
    ///
    /// Removes the adapter's contribution slot from every edge in the context.
    /// Recomputes raw weights from remaining contributions. Prunes edges
    /// whose contributions map becomes empty (zero evidence).
    ///
    /// Returns (edges_affected, pruned_edge_ids).
    pub fn retract_contributions(&mut self, adapter_id: &str) -> (usize, Vec<super::EdgeId>) {
        // Phase 1: Remove contribution slots, track which edges were affected
        let mut edges_affected = 0;
        let mut was_affected = vec![false; self.edges.len()];
        for (i, edge) in self.edges.iter_mut().enumerate() {
            if edge.contributions.remove(adapter_id).is_some() {
                edges_affected += 1;
                was_affected[i] = true;
            }
        }

        // Phase 2: Collect pruned edge IDs, then retain non-pruned edges
        let mut pruned_ids = Vec::new();
        for (i, edge) in self.edges.iter().enumerate() {
            if was_affected[i] && edge.contributions.is_empty() {
                pruned_ids.push(edge.id.clone());
            }
        }
        if !pruned_ids.is_empty() {
            let pruned_set: std::collections::HashSet<&super::EdgeId> =
                pruned_ids.iter().collect();
            self.edges.retain(|e| !pruned_set.contains(&e.id));
        }

        // Phase 3: Recompute raw weights from remaining contributions
        if edges_affected > 0 {
            self.recompute_raw_weights();
            self.touch();
        }

        (edges_affected, pruned_ids)
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
        edge1.raw_weight = 0.5;
        ctx.add_edge(edge1);

        // Add exact duplicate with higher raw_weight
        let mut edge2 = Edge::new(id_a.clone(), id_b.clone(), "calls");
        edge2.raw_weight = 0.8;
        ctx.add_edge(edge2);

        // Should only have one edge, with the higher raw_weight
        assert_eq!(ctx.edge_count(), 1);
        assert_eq!(ctx.edges[0].raw_weight, 0.8);
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
    fn test_cross_dimensional_tracking() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        // Add edge in structure dimension
        let edge1 = Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "structure");
        ctx.add_edge(edge1);

        // Add same logical edge in semantic dimension
        let edge2 = Edge::new_in_dimension(id_a.clone(), id_b.clone(), "calls", "semantic");
        ctx.add_edge(edge2);

        // Both edges should exist with cross_dim_count
        assert_eq!(ctx.edge_count(), 2);

        // Second edge should have cross_dim_count = 2
        let count = ctx.edges[1].properties.get("_cross_dim_count");
        assert!(count.is_some(), "Should have _cross_dim_count property");
        if let Some(PropertyValue::Int(n)) = count {
            assert_eq!(*n, 2, "cross_dim_count should be 2");
        }
    }

    // === ADR-027: Contribution Retraction ===

    #[test]
    fn retract_removes_adapter_contribution_from_all_edges() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_b = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_c = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));

        // Two edges with contributions from "embedding:model-a"
        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.8)
                .with_contribution("co_occurrence:tagged_with:may_be_related", 0.6),
        );
        ctx.add_edge(
            Edge::new_in_dimension(id_b.clone(), id_c.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.7),
        );
        ctx.recompute_raw_weights();

        let (affected, pruned) = ctx.retract_contributions("embedding:model-a");

        assert_eq!(affected, 2, "both edges had the adapter's contribution");
        // Edge A→B survives (has remaining contribution from co_occurrence)
        // Edge B→C is pruned (only had embedding:model-a)
        assert_eq!(pruned.len(), 1, "one edge should be pruned");
        assert_eq!(ctx.edge_count(), 1, "one edge should remain");

        // Remaining edge should not have embedding:model-a contribution
        let remaining = &ctx.edges[0];
        assert!(!remaining.contributions.contains_key("embedding:model-a"));
        assert!(remaining.contributions.contains_key("co_occurrence:tagged_with:may_be_related"));
    }

    #[test]
    fn retract_prunes_zero_evidence_edges() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_b = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));

        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.9),
        );
        ctx.recompute_raw_weights();

        let (affected, pruned) = ctx.retract_contributions("embedding:model-a");

        assert_eq!(affected, 1);
        assert_eq!(pruned.len(), 1);
        assert_eq!(ctx.edge_count(), 0, "edge with no remaining contributions should be pruned");
    }

    #[test]
    fn retract_preserves_edges_with_remaining_contributions() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_b = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));

        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.8)
                .with_contribution("co_occurrence:tagged_with:may_be_related", 0.6),
        );
        ctx.recompute_raw_weights();

        let (affected, pruned) = ctx.retract_contributions("embedding:model-a");

        assert_eq!(affected, 1);
        assert!(pruned.is_empty(), "edge with remaining contributions should not be pruned");
        assert_eq!(ctx.edge_count(), 1);

        let edge = &ctx.edges[0];
        assert_eq!(edge.contributions.len(), 1);
        assert!(edge.contributions.contains_key("co_occurrence:tagged_with:may_be_related"));
    }

    #[test]
    fn retract_nonexistent_adapter_is_noop() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_b = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));

        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.8),
        );
        ctx.recompute_raw_weights();
        let original_weight = ctx.edges[0].raw_weight;

        let (affected, pruned) = ctx.retract_contributions("nonexistent-adapter");

        assert_eq!(affected, 0);
        assert!(pruned.is_empty());
        assert_eq!(ctx.edge_count(), 1);
        assert_eq!(ctx.edges[0].raw_weight, original_weight, "weights should be unchanged");
    }

    #[test]
    fn retract_recomputes_raw_weights() {
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_b = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));
        let id_c = ctx.add_node(Node::new_in_dimension("concept", ContentType::Concept, "semantic"));

        // Two edges with two adapters each
        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.8)
                .with_contribution("other-adapter", 0.5),
        );
        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_c.clone(), "similar_to", "semantic")
                .with_contribution("embedding:model-a", 0.6)
                .with_contribution("other-adapter", 0.9),
        );
        ctx.recompute_raw_weights();
        let weight_before = ctx.edges[0].raw_weight;

        ctx.retract_contributions("embedding:model-a");

        // After retraction, only "other-adapter" remains
        // With a single adapter, scale normalization changes the weights
        let weight_after = ctx.edges[0].raw_weight;
        assert_ne!(weight_before, weight_after, "raw weight should change after retraction");
        // Both edges still exist (both had remaining contributions)
        assert_eq!(ctx.edge_count(), 2);
    }

    #[test]
    fn test_cross_dimensional_tracking_increments_with_more_dimensions() {
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
    }
}
