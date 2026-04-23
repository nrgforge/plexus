//! Core adapter layer types
//!
//! Domain vocabulary from domain-model.md:
//! - Emission: bundle of annotated nodes, edges, and removals
//! - AnnotatedNode: Node paired with optional Annotation
//! - AnnotatedEdge: Edge paired with optional Annotation
//! - Annotation: adapter-provided extraction metadata
//! - Removal: a node ID to remove (edges cascade)

use crate::graph::{Edge, Node, NodeId, PropertyValue};
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

/// A request to remove a specific edge by its source, target, and relationship.
#[derive(Debug, Clone)]
pub struct EdgeRemoval {
    pub source: NodeId,
    pub target: NodeId,
    pub relationship: String,
}

impl EdgeRemoval {
    pub fn new(
        source: NodeId,
        target: NodeId,
        relationship: impl Into<String>,
    ) -> Self {
        Self {
            source,
            target,
            relationship: relationship.into(),
        }
    }
}

/// A property merge on an existing node (ADR-023).
///
/// Unlike full node upsert, this only adds or updates the specified properties
/// without replacing the entire node. If the node doesn't exist, this is a no-op.
#[derive(Debug, Clone)]
pub struct PropertyUpdate {
    pub node_id: NodeId,
    pub properties: HashMap<String, PropertyValue>,
}

impl PropertyUpdate {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            properties: HashMap::new(),
        }
    }

    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: PropertyValue,
    ) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

/// The data payload of a single `sink.emit()` call.
///
/// A bundle of annotated nodes, annotated edges, removals, and property updates.
/// Each emission is validated and committed atomically by the engine.
/// Valid items commit; invalid items are rejected individually.
#[derive(Debug, Clone)]
pub struct Emission {
    pub nodes: Vec<AnnotatedNode>,
    pub edges: Vec<AnnotatedEdge>,
    pub removals: Vec<Removal>,
    pub edge_removals: Vec<EdgeRemoval>,
    pub property_updates: Vec<PropertyUpdate>,
}

impl Emission {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            removals: Vec::new(),
            edge_removals: Vec::new(),
            property_updates: Vec::new(),
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

    pub fn with_edge_removal(mut self, edge_removal: EdgeRemoval) -> Self {
        self.edge_removals.push(edge_removal);
        self
    }

    pub fn with_property_update(mut self, update: PropertyUpdate) -> Self {
        self.property_updates.push(update);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.edges.is_empty()
            && self.removals.is_empty()
            && self.edge_removals.is_empty()
            && self.property_updates.is_empty()
    }

    /// Merge another emission into this one by appending all vectors.
    pub fn merge(mut self, other: Emission) -> Self {
        self.nodes.extend(other.nodes);
        self.edges.extend(other.edges);
        self.removals.extend(other.removals);
        self.edge_removals.extend(other.edge_removals);
        self.property_updates.extend(other.property_updates);
        self
    }
}

impl Default for Emission {
    fn default() -> Self {
        Self::new()
    }
}

/// A domain-meaningful event for the consumer (ADR-011).
///
/// Translated from raw graph events by the adapter's `transform_events()`.
/// The consumer receives outbound events, never raw graph events.
/// Deliberately unstructured — the consumer defines what `kind` values
/// it cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundEvent {
    /// Event type in the consumer's vocabulary (e.g., "concepts_detected")
    pub kind: String,
    /// Human-readable detail (e.g., "travel, provence")
    pub detail: String,
}

impl OutboundEvent {
    pub fn new(kind: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            detail: detail.into(),
        }
    }
}

// === Construction helpers ===

/// Current UTC time as an ISO-8601 / RFC-3339 string — the authoritative
/// format for timestamp-based enrichments per ADR-039. Adapters that write
/// `created_at` (or other timestamp properties `TemporalProximityEnrichment`
/// reads) should use this helper rather than constructing the string inline,
/// so the format decision stays in one place.
pub fn rfc3339_now() -> PropertyValue {
    PropertyValue::String(chrono::Utc::now().to_rfc3339())
}

/// Create a concept node with deterministic ID, label property, and
/// `created_at` timestamp (ADR-039).
///
/// Normalizes `label` to lowercase for the ID (`concept:<lowercase>`) and the
/// `label` property. Writes the current UTC timestamp as an ISO-8601 /
/// RFC-3339 string to `properties["created_at"]` — the authoritative surface
/// for timestamp-based enrichments. Callers that want a different ingest time
/// can overwrite the property on the returned node; subsequent call sites
/// that re-derive the same `concept:<normalized>` node via upsert will not
/// overwrite the original timestamp.
///
/// Returns the `(NodeId, Node)` pair so callers can add additional properties
/// before wrapping in `AnnotatedNode`.
pub fn concept_node(label: &str) -> (NodeId, Node) {
    use crate::graph::{dimension, ContentType};
    let normalized = label.to_lowercase();
    let id = NodeId::from_string(format!("concept:{}", normalized));
    let mut node = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
    node.id = id.clone();
    node.properties
        .insert("label".to_string(), PropertyValue::String(normalized));
    node.properties
        .insert("created_at".to_string(), rfc3339_now());
    (id, node)
}

/// Create a chain node with the given ID.
///
/// Sets `node_type = "chain"`, `ContentType::Provenance`, `dimension::PROVENANCE`.
/// Caller is responsible for setting `name`, `status`, and other properties.
pub fn chain_node(id: &str) -> Node {
    use crate::graph::{dimension, ContentType};
    let mut node = Node::new_in_dimension("chain", ContentType::Provenance, dimension::PROVENANCE);
    node.id = NodeId::from(id);
    node
}

/// Create a mark node with the given ID.
///
/// Sets `node_type = "mark"`, `ContentType::Provenance`, `dimension::PROVENANCE`.
/// Caller is responsible for setting `file`, `line`, `annotation`, and other properties.
pub fn mark_node(id: &str) -> Node {
    use crate::graph::{dimension, ContentType};
    let mut node = Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
    node.id = NodeId::from(id);
    node
}

/// Create a file node with a deterministic `file:{path}` ID.
///
/// Sets `node_type = "file"`, `ContentType::Document`, `dimension::STRUCTURE`.
pub fn file_node(path: &str) -> Node {
    use crate::graph::{dimension, ContentType};
    let mut node = Node::new_in_dimension("file", ContentType::Document, dimension::STRUCTURE);
    node.id = NodeId::from_string(format!("file:{}", path));
    node
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
