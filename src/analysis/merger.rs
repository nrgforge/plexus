//! Result merger for combining outputs from multiple analyzers
//!
//! Handles deduplication, conflict resolution, and cross-dimensional edge creation.

use super::types::{AnalysisResult, AnalysisScope, ContentId, GraphMutation};
use crate::graph::{Edge, Node, NodeId};
use std::collections::{HashMap, HashSet};

/// Merges analysis results from multiple analyzers
pub struct ResultMerger {
    /// Strategy for handling node conflicts
    pub conflict_strategy: ConflictStrategy,
}

/// How to handle conflicts when multiple analyzers produce overlapping nodes
#[derive(Debug, Clone, Copy, Default)]
pub enum ConflictStrategy {
    /// Last write wins (default)
    #[default]
    LastWriteWins,
    /// First write wins
    FirstWriteWins,
    /// Merge properties from all sources
    MergeProperties,
}

impl Default for ResultMerger {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultMerger {
    /// Create a new merger with default settings
    pub fn new() -> Self {
        Self {
            conflict_strategy: ConflictStrategy::default(),
        }
    }

    /// Create with a specific conflict strategy
    pub fn with_strategy(mut self, strategy: ConflictStrategy) -> Self {
        self.conflict_strategy = strategy;
        self
    }

    /// Merge multiple analysis results into a single graph mutation
    pub fn merge(&self, results: Vec<AnalysisResult>, scope: &AnalysisScope) -> GraphMutation {
        let mut mutation = GraphMutation::new();

        // Collect all nodes, handling duplicates
        let mut node_map: HashMap<NodeDedupeKey, Node> = HashMap::new();
        let mut provenance_map: HashMap<NodeId, ContentId> = HashMap::new();

        for result in &results {
            for node in &result.nodes {
                let key = NodeDedupeKey::from_node(node);

                match self.conflict_strategy {
                    ConflictStrategy::LastWriteWins => {
                        node_map.insert(key, node.clone());
                    }
                    ConflictStrategy::FirstWriteWins => {
                        node_map.entry(key).or_insert_with(|| node.clone());
                    }
                    ConflictStrategy::MergeProperties => {
                        node_map
                            .entry(key)
                            .and_modify(|existing| {
                                // Merge properties from new node into existing
                                for (k, v) in &node.properties {
                                    existing.properties.insert(k.clone(), v.clone());
                                }
                            })
                            .or_insert_with(|| node.clone());
                    }
                }

                // Track provenance
                if let Some(source) = result.provenance.get(&node.id) {
                    provenance_map.insert(node.id.clone(), source.clone());
                }
            }
        }

        mutation.upsert_nodes = node_map.into_values().collect();

        // Collect all edges, handling duplicates
        let mut edge_set: HashSet<EdgeDedupeKey> = HashSet::new();
        let mut edges: Vec<Edge> = Vec::new();

        for result in &results {
            for edge in &result.edges {
                let key = EdgeDedupeKey::from_edge(edge);
                if edge_set.insert(key) {
                    edges.push(edge.clone());
                }
            }
        }

        mutation.upsert_edges = edges;

        // Handle incremental updates: remove nodes from changed content
        if scope.is_incremental() {
            // Find nodes that were previously created from changed content
            // This requires the current_graph to be set
            if let Some(ref current) = scope.current_graph {
                for changed_id in &scope.changed_items {
                    // Find nodes in current graph that came from this content
                    // Note: This requires provenance tracking to be stored in node properties
                    for node in &current.nodes {
                        if let Some(crate::graph::PropertyValue::String(s)) =
                            node.properties.get("_source_content")
                        {
                            if s == changed_id.as_str() {
                                mutation.remove_nodes.push(node.id.clone());
                            }
                        }
                    }
                }
            }
        }

        // Create cross-dimensional edges
        self.create_cross_dimensional_edges(&mut mutation, &provenance_map);

        mutation
    }

    /// Create edges between nodes in different dimensions that share source content
    fn create_cross_dimensional_edges(
        &self,
        mutation: &mut GraphMutation,
        provenance: &HashMap<NodeId, ContentId>,
    ) {
        // Group nodes by source content
        let mut by_source: HashMap<&ContentId, Vec<&Node>> = HashMap::new();
        for node in &mutation.upsert_nodes {
            if let Some(source) = provenance.get(&node.id) {
                by_source.entry(source).or_default().push(node);
            }
        }

        // For each source with multiple nodes in different dimensions,
        // create cross-dimensional edges
        for (_source, nodes) in by_source {
            if nodes.len() < 2 {
                continue;
            }

            // Find pairs in different dimensions
            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    let node_a = nodes[i];
                    let node_b = nodes[j];

                    // Only create edge if dimensions differ
                    if node_a.dimension != node_b.dimension {
                        let relationship = self.infer_cross_dimensional_relationship(node_a, node_b);

                        // Create edge from structure to semantic (or other ordering)
                        let (source, target) = self.order_cross_dimensional_pair(node_a, node_b);

                        let edge = Edge::new_cross_dimensional(
                            source.id.clone(),
                            source.dimension.clone(),
                            target.id.clone(),
                            target.dimension.clone(),
                            relationship,
                        );

                        mutation.upsert_edges.push(edge);
                    }
                }
            }
        }
    }

    /// Infer relationship type for cross-dimensional edges
    fn infer_cross_dimensional_relationship(&self, node_a: &Node, node_b: &Node) -> String {
        // Determine relationship based on dimension pairs
        match (node_a.dimension.as_str(), node_b.dimension.as_str()) {
            ("structure", "semantic") | ("semantic", "structure") => "implements".to_string(),
            ("structure", "relational") | ("relational", "structure") => "contains".to_string(),
            ("semantic", "relational") | ("relational", "semantic") => "describes".to_string(),
            _ => "relates_to".to_string(),
        }
    }

    /// Order nodes for cross-dimensional edge (source -> target)
    fn order_cross_dimensional_pair<'a>(
        &self,
        node_a: &'a Node,
        node_b: &'a Node,
    ) -> (&'a Node, &'a Node) {
        // Ordering: structure -> semantic -> relational -> temporal
        let dimension_order = |dim: &str| match dim {
            "structure" => 0,
            "semantic" => 1,
            "relational" => 2,
            "temporal" => 3,
            _ => 4,
        };

        if dimension_order(&node_a.dimension) <= dimension_order(&node_b.dimension) {
            (node_a, node_b)
        } else {
            (node_b, node_a)
        }
    }
}

/// Key for deduplicating nodes
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct NodeDedupeKey {
    /// Node type
    node_type: String,
    /// Dimension
    dimension: String,
    /// Content hash (if present in properties)
    content_hash: Option<String>,
    /// Node ID as fallback
    id: String,
}

impl NodeDedupeKey {
    fn from_node(node: &Node) -> Self {
        let content_hash = node
            .properties
            .get("content_hash")
            .and_then(|v| match v {
                crate::graph::PropertyValue::String(s) => Some(s.clone()),
                _ => None,
            });

        Self {
            node_type: node.node_type.clone(),
            dimension: node.dimension.clone(),
            content_hash,
            id: node.id.as_str().to_string(),
        }
    }
}

/// Key for deduplicating edges
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EdgeDedupeKey {
    source: String,
    target: String,
    relationship: String,
    source_dimension: String,
    target_dimension: String,
}

impl EdgeDedupeKey {
    fn from_edge(edge: &Edge) -> Self {
        Self {
            source: edge.source.as_str().to_string(),
            target: edge.target.as_str().to_string(),
            relationship: edge.relationship.clone(),
            source_dimension: edge.source_dimension.clone(),
            target_dimension: edge.target_dimension.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, ContextId, Node, PropertyValue};

    #[test]
    fn test_merge_empty() {
        let merger = ResultMerger::new();
        let scope = AnalysisScope::new(ContextId::from_string("test"), vec![]);
        let mutation = merger.merge(vec![], &scope);
        assert!(mutation.is_empty());
    }

    #[test]
    fn test_merge_single_result() {
        let merger = ResultMerger::new();
        let scope = AnalysisScope::new(ContextId::from_string("test"), vec![]);

        let mut result = AnalysisResult::new();
        let node = Node::new("test", ContentType::Document);
        result.nodes.push(node);

        let mutation = merger.merge(vec![result], &scope);
        assert_eq!(mutation.upsert_nodes.len(), 1);
    }

    #[test]
    fn test_merge_deduplication_last_write_wins() {
        let merger = ResultMerger::new();
        let scope = AnalysisScope::new(ContextId::from_string("test"), vec![]);

        // Create two results with same node but different properties
        let mut result1 = AnalysisResult::new();
        let mut node1 = Node::new("test", ContentType::Document);
        node1.id = NodeId::from_string("same-id".to_string());
        node1
            .properties
            .insert("value".into(), PropertyValue::String("first".into()));
        result1.nodes.push(node1);

        let mut result2 = AnalysisResult::new();
        let mut node2 = Node::new("test", ContentType::Document);
        node2.id = NodeId::from_string("same-id".to_string());
        node2
            .properties
            .insert("value".into(), PropertyValue::String("second".into()));
        result2.nodes.push(node2);

        let mutation = merger.merge(vec![result1, result2], &scope);

        // Last write wins, so we should have "second"
        assert_eq!(mutation.upsert_nodes.len(), 1);
        let value = mutation.upsert_nodes[0].properties.get("value").unwrap();
        assert!(matches!(value, PropertyValue::String(s) if s == "second"));
    }

    #[test]
    fn test_merge_properties_strategy() {
        let merger = ResultMerger::new().with_strategy(ConflictStrategy::MergeProperties);
        let scope = AnalysisScope::new(ContextId::from_string("test"), vec![]);

        let mut result1 = AnalysisResult::new();
        let mut node1 = Node::new("test", ContentType::Document);
        node1.id = NodeId::from_string("same-id".to_string());
        node1
            .properties
            .insert("prop1".into(), PropertyValue::String("value1".into()));
        result1.nodes.push(node1);

        let mut result2 = AnalysisResult::new();
        let mut node2 = Node::new("test", ContentType::Document);
        node2.id = NodeId::from_string("same-id".to_string());
        node2
            .properties
            .insert("prop2".into(), PropertyValue::String("value2".into()));
        result2.nodes.push(node2);

        let mutation = merger.merge(vec![result1, result2], &scope);

        assert_eq!(mutation.upsert_nodes.len(), 1);
        let node = &mutation.upsert_nodes[0];
        assert!(node.properties.contains_key("prop1"));
        assert!(node.properties.contains_key("prop2"));
    }
}
