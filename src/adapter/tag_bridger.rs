//! TagConceptBridger enrichment (ADR-009, ADR-010, ADR-022)
//!
//! Bridges marks and concepts bidirectionally via tag matching:
//! - New mark with tags → bridge edges to matching concepts
//! - New concept → bridge edges from existing marks with matching tags
//!
//! Default relationship: `references` (backward compatible).
//! Parameterized instances use a different relationship (ADR-022).
//!
//! Idempotent: checks for existing edges before emitting.

use super::enrichment::Enrichment;
use super::events::GraphEvent;
use super::types::Emission;
use crate::graph::{dimension, Context, Edge, NodeId, PropertyValue};
use std::collections::HashSet;

/// Enrichment that creates cross-dimensional edges between marks
/// (provenance) and concepts (semantic) when their tags match.
///
/// Default relationship: `references` (backward compatible).
/// Parameterized instances use a different relationship (ADR-022).
pub struct TagConceptBridger {
    relationship: String,
    id: String,
}

impl TagConceptBridger {
    pub fn new() -> Self {
        Self {
            relationship: "references".to_string(),
            id: "tag_bridger:references".to_string(),
        }
    }

    pub fn with_relationship(relationship: &str) -> Self {
        Self {
            id: format!("tag_bridger:{}", relationship),
            relationship: relationship.to_string(),
        }
    }
}

impl Enrichment for TagConceptBridger {
    fn id(&self) -> &str {
        &self.id
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        let mut emission = Emission::new();

        for event in events {
            if let GraphEvent::NodesAdded { node_ids, .. } = event {
                for node_id in node_ids {
                    if let Some(node) = context.get_node(node_id) {
                        if node.dimension == dimension::SEMANTIC
                            && node_id.to_string().starts_with("concept:")
                        {
                            // New concept: scan marks for matching tags
                            let concept_tag = &node_id.to_string()["concept:".len()..];
                            for mark in context.nodes().filter(|n| n.dimension == dimension::PROVENANCE) {
                                if mark_has_tag(&mark, concept_tag)
                                    && !bridge_edge_exists(context, &mark.id, node_id, &self.relationship)
                                {
                                    emission = emission.with_edge(
                                        make_bridge_edge(&mark.id, node_id, &self.relationship),
                                    );
                                }
                            }
                        } else if node.dimension == dimension::PROVENANCE {
                            // New mark: find matching concepts (deduplicate normalized tags)
                            let mut seen_concepts = HashSet::new();
                            for tag in extract_normalized_tags(node) {
                                if !seen_concepts.insert(tag.clone()) {
                                    continue;
                                }
                                let concept_id =
                                    NodeId::from_string(format!("concept:{}", tag));
                                if context.get_node(&concept_id).is_some()
                                    && !bridge_edge_exists(context, node_id, &concept_id, &self.relationship)
                                {
                                    emission = emission.with_edge(
                                        make_bridge_edge(node_id, &concept_id, &self.relationship),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        if emission.is_empty() {
            None
        } else {
            Some(emission)
        }
    }
}

/// Check if a node's "tags" property contains a matching tag (after normalization).
fn mark_has_tag(node: &crate::graph::Node, concept_tag: &str) -> bool {
    extract_normalized_tags(node)
        .any(|t| t == concept_tag)
}

/// Extract normalized tags from a node's "tags" property.
///
/// Normalization: strip `#` prefix, lowercase.
fn extract_normalized_tags(node: &crate::graph::Node) -> impl Iterator<Item = String> + '_ {
    node.properties
        .get("tags")
        .into_iter()
        .flat_map(|pv| match pv {
            PropertyValue::Array(arr) => arr.as_slice(),
            _ => &[],
        })
        .filter_map(|v| match v {
            PropertyValue::String(s) => Some(
                s.trim_start_matches('#').to_lowercase(),
            ),
            _ => None,
        })
}

/// Check if a bridge edge from source to target with the given relationship already exists.
fn bridge_edge_exists(context: &Context, source: &NodeId, target: &NodeId, relationship: &str) -> bool {
    context.edges().any(|e| {
        e.source == *source && e.target == *target && e.relationship == relationship
    })
}

/// Create a cross-dimensional bridge edge from mark (provenance) to concept (semantic).
fn make_bridge_edge(mark_id: &NodeId, concept_id: &NodeId, relationship: &str) -> crate::adapter::types::AnnotatedEdge {
    let mut edge = Edge::new_cross_dimensional(
        mark_id.clone(),
        dimension::PROVENANCE,
        concept_id.clone(),
        dimension::SEMANTIC,
        relationship,
    );
    edge.raw_weight = 1.0;
    crate::adapter::types::AnnotatedEdge::new(edge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Node, NodeId};

    fn concept_node(tag: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(format!("concept:{}", tag));
        n.dimension = dimension::SEMANTIC.to_string();
        n.properties.insert(
            "label".to_string(),
            PropertyValue::String(tag.to_string()),
        );
        n
    }

    fn mark_node(id: &str, tags: &[&str]) -> Node {
        let mut n = Node::new("mark", ContentType::Provenance);
        n.id = NodeId::from_string(id);
        n.dimension = dimension::PROVENANCE.to_string();
        let tag_vals: Vec<PropertyValue> = tags
            .iter()
            .map(|t| PropertyValue::String(t.to_string()))
            .collect();
        n.properties
            .insert("tags".to_string(), PropertyValue::Array(tag_vals));
        n
    }

    #[test]
    fn new_mark_bridges_to_existing_concept() {
        let bridger = TagConceptBridger::new();
        let mark_id = NodeId::from_string("mark-1");

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("travel"));
        ctx.add_node(mark_node("mark-1", &["#travel"]));

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![mark_id.clone()],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        let emission = bridger.enrich(&events, &ctx).expect("should emit");
        assert_eq!(emission.edges.len(), 1);

        let edge = &emission.edges[0].edge;
        assert_eq!(edge.source, mark_id);
        assert_eq!(edge.target, NodeId::from_string("concept:travel"));
        assert_eq!(edge.relationship, "references");
        assert_eq!(edge.source_dimension, dimension::PROVENANCE);
        assert_eq!(edge.target_dimension, dimension::SEMANTIC);
    }

    #[test]
    fn new_concept_bridges_to_existing_mark() {
        let bridger = TagConceptBridger::new();
        let concept_id = NodeId::from_string("concept:travel");

        let mut ctx = Context::new("test");
        ctx.add_node(mark_node("mark-1", &["#travel"]));
        ctx.add_node(concept_node("travel"));

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![concept_id.clone()],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        let emission = bridger.enrich(&events, &ctx).expect("should emit");
        assert_eq!(emission.edges.len(), 1);

        let edge = &emission.edges[0].edge;
        assert_eq!(edge.source, NodeId::from_string("mark-1"));
        assert_eq!(edge.target, concept_id);
        assert_eq!(edge.relationship, "references");
    }

    #[test]
    fn idempotent_no_duplicate_edges() {
        let bridger = TagConceptBridger::new();
        let mark_id = NodeId::from_string("mark-1");
        let concept_id = NodeId::from_string("concept:travel");

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("travel"));
        ctx.add_node(mark_node("mark-1", &["#travel"]));

        // Pre-existing edge
        ctx.add_edge(Edge::new_cross_dimensional(
            mark_id.clone(),
            dimension::PROVENANCE,
            concept_id.clone(),
            dimension::SEMANTIC,
            "references",
        ));

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![mark_id],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        // Should be quiescent — edge already exists
        assert!(bridger.enrich(&events, &ctx).is_none());
    }

    #[test]
    fn tag_normalization_strips_hash_and_lowercases() {
        let bridger = TagConceptBridger::new();

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("travel"));
        ctx.add_node(mark_node("mark-1", &["#Travel", "TRAVEL", "#travel"]));

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![NodeId::from_string("mark-1")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        let emission = bridger.enrich(&events, &ctx).expect("should emit");
        // All three tags normalize to "travel" — one edge, not three
        assert_eq!(emission.edges.len(), 1);
    }

    // --- Scenario: TagConceptBridger accepts relationship parameter ---

    #[test]
    fn accepts_relationship_parameter() {
        let references_bridger = TagConceptBridger::new();
        let categorized_bridger = TagConceptBridger::with_relationship("categorized_by");

        let mark_id = NodeId::from_string("mark-1");

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("travel"));
        ctx.add_node(mark_node("mark-1", &["travel"]));

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![mark_id.clone()],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        // First bridger: creates "references" edge
        let emission1 = references_bridger.enrich(&events, &ctx).expect("should emit");
        assert_eq!(emission1.edges.len(), 1);
        assert_eq!(emission1.edges[0].edge.relationship, "references");
        assert_eq!(emission1.edges[0].edge.source, mark_id);
        assert_eq!(emission1.edges[0].edge.target, NodeId::from_string("concept:travel"));

        // Second bridger: creates "categorized_by" edge
        let emission2 = categorized_bridger.enrich(&events, &ctx).expect("should emit");
        assert_eq!(emission2.edges.len(), 1);
        assert_eq!(emission2.edges[0].edge.relationship, "categorized_by");
        assert_eq!(emission2.edges[0].edge.source, mark_id);
        assert_eq!(emission2.edges[0].edge.target, NodeId::from_string("concept:travel"));

        // Distinct IDs
        assert_eq!(references_bridger.id(), "tag_bridger:references");
        assert_eq!(categorized_bridger.id(), "tag_bridger:categorized_by");
        assert_ne!(references_bridger.id(), categorized_bridger.id());
    }

    #[test]
    fn no_bridge_when_concept_missing() {
        let bridger = TagConceptBridger::new();

        let mut ctx = Context::new("test");
        ctx.add_node(mark_node("mark-1", &["#travel"]));
        // No concept:travel node exists

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![NodeId::from_string("mark-1")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        assert!(bridger.enrich(&events, &ctx).is_none());
    }
}
