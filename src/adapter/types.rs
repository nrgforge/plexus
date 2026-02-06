//! Core adapter layer types
//!
//! Domain vocabulary from domain-model.md:
//! - Emission: bundle of annotated nodes, edges, and removals
//! - AnnotatedNode: Node paired with optional Annotation
//! - AnnotatedEdge: Edge paired with optional Annotation
//! - Annotation: adapter-provided extraction metadata
//! - Removal: a node ID to remove (edges cascade)

use crate::graph::{Edge, Node, NodeId};
use std::collections::HashMap;

/// Adapter-provided metadata about a single extraction.
///
/// Describes *how* the adapter came to know something.
/// The engine wraps this with framework context to create a ProvenanceEntry.
#[derive(Debug, Clone)]
pub struct Annotation {
    /// Adapter's certainty about this extraction (0.0–1.0)
    pub confidence: Option<f64>,
    /// How the knowledge was extracted (e.g., "llm-extraction", "label-mapping")
    pub method: Option<String>,
    /// Where in the source input this was found (e.g., "file.md:87")
    pub source_location: Option<String>,
    /// Additional adapter-specific detail
    pub detail: HashMap<String, String>,
}

impl Annotation {
    pub fn new() -> Self {
        Self {
            confidence: None,
            method: None,
            source_location: None,
            detail: HashMap::new(),
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.method = Some(method.into());
        self
    }

    pub fn with_source_location(mut self, location: impl Into<String>) -> Self {
        self.source_location = Some(location.into());
        self
    }
}

impl Default for Annotation {
    fn default() -> Self {
        Self::new()
    }
}

/// A node paired with an optional annotation.
#[derive(Debug, Clone)]
pub struct AnnotatedNode {
    pub node: Node,
    pub annotation: Option<Annotation>,
}

impl AnnotatedNode {
    pub fn new(node: Node) -> Self {
        Self { node, annotation: None }
    }

    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.annotation = Some(annotation);
        self
    }
}

impl From<Node> for AnnotatedNode {
    fn from(node: Node) -> Self {
        Self::new(node)
    }
}

/// An edge paired with an optional annotation.
#[derive(Debug, Clone)]
pub struct AnnotatedEdge {
    pub edge: Edge,
    pub annotation: Option<Annotation>,
}

impl AnnotatedEdge {
    pub fn new(edge: Edge) -> Self {
        Self { edge, annotation: None }
    }

    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.annotation = Some(annotation);
        self
    }
}

impl From<Edge> for AnnotatedEdge {
    fn from(edge: Edge) -> Self {
        Self::new(edge)
    }
}

/// A request to remove a node. Connected edges cascade.
#[derive(Debug, Clone)]
pub struct Removal {
    pub node_id: NodeId,
}

impl Removal {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }
}

/// The data payload of a single `sink.emit()` call.
///
/// A bundle of annotated nodes, annotated edges, and removals.
/// Each emission is validated and committed atomically by the engine.
/// Valid items commit; invalid items are rejected individually.
#[derive(Debug, Clone)]
pub struct Emission {
    pub nodes: Vec<AnnotatedNode>,
    pub edges: Vec<AnnotatedEdge>,
    pub removals: Vec<Removal>,
}

impl Emission {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            removals: Vec::new(),
        }
    }

    pub fn with_node(mut self, node: impl Into<AnnotatedNode>) -> Self {
        self.nodes.push(node.into());
        self
    }

    pub fn with_edge(mut self, edge: impl Into<AnnotatedEdge>) -> Self {
        self.edges.push(edge.into());
        self
    }

    pub fn with_removal(mut self, node_id: NodeId) -> Self {
        self.removals.push(Removal::new(node_id));
        self
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty() && self.removals.is_empty()
    }
}

impl Default for Emission {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Edge, Node, NodeId};

    #[test]
    fn emission_builder_constructs_complete_emission() {
        let node_a = Node::new("concept", ContentType::Concept);
        let node_b = Node::new("concept", ContentType::Concept);
        let edge = Edge::new(node_a.id.clone(), node_b.id.clone(), "may_be_related");

        let emission = Emission::new()
            .with_node(node_a)
            .with_node(node_b)
            .with_edge(edge);

        assert_eq!(emission.nodes.len(), 2);
        assert_eq!(emission.edges.len(), 1);
        assert!(emission.removals.is_empty());
        assert!(!emission.is_empty());
    }

    #[test]
    fn empty_emission_reports_empty() {
        let emission = Emission::new();
        assert!(emission.is_empty());
    }

    #[test]
    fn annotated_node_carries_annotation() {
        let node = Node::new("concept", ContentType::Concept);
        let annotation = Annotation::new()
            .with_confidence(0.85)
            .with_method("llm-extraction")
            .with_source_location("file.md:87");

        let annotated = AnnotatedNode::new(node).with_annotation(annotation);

        let ann = annotated.annotation.as_ref().unwrap();
        assert_eq!(ann.confidence, Some(0.85));
        assert_eq!(ann.method.as_deref(), Some("llm-extraction"));
        assert_eq!(ann.source_location.as_deref(), Some("file.md:87"));
    }

    #[test]
    fn emission_with_removal() {
        let node_id = NodeId::from_string("node-to-remove");
        let emission = Emission::new().with_removal(node_id.clone());

        assert_eq!(emission.removals.len(), 1);
        assert_eq!(emission.removals[0].node_id.as_str(), "node-to-remove");
    }
}
