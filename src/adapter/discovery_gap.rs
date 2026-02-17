//! DiscoveryGapEnrichment — latent-structural disagreement detection (ADR-024)
//!
//! Detects concept pairs that are latently similar (connected by a trigger
//! relationship, e.g. `similar_to` from embeddings) but structurally
//! unconnected (no other edges between them). Emits symmetric edge pairs
//! with a configured output relationship (e.g. `discovery_gap`).
//!
//! Structure-aware: fires based on relationship, not node content type
//! (Invariant 50). Idempotent: checks for existing edges before emitting,
//! so the enrichment loop reaches quiescence.

use crate::adapter::enrichment::Enrichment;
use crate::adapter::events::GraphEvent;
use crate::adapter::types::{AnnotatedEdge, Emission};
use crate::graph::{dimension, Context, Edge, NodeId};

/// Enrichment that detects discovery gaps — latent similarity without
/// structural evidence.
///
/// When a trigger relationship edge (e.g., `similar_to`) connects two nodes
/// that have no other edges between them, emits a symmetric output
/// relationship edge pair (e.g., `discovery_gap`).
///
/// Parameterizable on `trigger_relationship` and `output_relationship` (ADR-024).
pub struct DiscoveryGapEnrichment {
    trigger_relationship: String,
    output_relationship: String,
    id: String,
}

impl DiscoveryGapEnrichment {
    pub fn new(trigger_relationship: &str, output_relationship: &str) -> Self {
        Self {
            id: format!("discovery_gap:{}:{}", trigger_relationship, output_relationship),
            trigger_relationship: trigger_relationship.to_string(),
            output_relationship: output_relationship.to_string(),
        }
    }
}

impl Enrichment for DiscoveryGapEnrichment {
    fn id(&self) -> &str {
        &self.id
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        // Only run when edges are added (trigger edges enter the graph)
        if !has_edge_events(events) {
            return None;
        }

        let mut emission = Emission::new();

        // Scan context for trigger relationship edges
        for edge in context.edges() {
            if edge.relationship != self.trigger_relationship {
                continue;
            }

            let (a, b) = (&edge.source, &edge.target);

            // Check for structural evidence: any edge between A and B
            // that is NOT the trigger relationship and NOT the output relationship
            if has_structural_evidence(context, a, b, &self.trigger_relationship, &self.output_relationship) {
                continue;
            }

            // Idempotent guard: skip if output edges already exist
            if output_edge_exists(context, a, b, &self.output_relationship) {
                continue;
            }

            // Emit symmetric discovery_gap pair with trigger edge's weight
            let contribution = edge.raw_weight;

            let mut forward = Edge::new_in_dimension(
                a.clone(),
                b.clone(),
                &self.output_relationship,
                dimension::SEMANTIC,
            );
            forward.raw_weight = contribution;
            emission = emission.with_edge(AnnotatedEdge::new(forward));

            if !output_edge_exists(context, b, a, &self.output_relationship) {
                let mut reverse = Edge::new_in_dimension(
                    b.clone(),
                    a.clone(),
                    &self.output_relationship,
                    dimension::SEMANTIC,
                );
                reverse.raw_weight = contribution;
                emission = emission.with_edge(AnnotatedEdge::new(reverse));
            }
        }

        if emission.is_empty() {
            None
        } else {
            Some(emission)
        }
    }
}

/// Check if events include edge additions.
fn has_edge_events(events: &[GraphEvent]) -> bool {
    events.iter().any(|e| matches!(e, GraphEvent::EdgesAdded { .. }))
}

/// Check if any structural edge exists between two nodes, excluding
/// the trigger and output relationships.
fn has_structural_evidence(
    context: &Context,
    a: &NodeId,
    b: &NodeId,
    trigger_relationship: &str,
    output_relationship: &str,
) -> bool {
    context.edges().any(|e| {
        let connects = (e.source == *a && e.target == *b)
            || (e.source == *b && e.target == *a);
        connects
            && e.relationship != trigger_relationship
            && e.relationship != output_relationship
    })
}

/// Check if an output edge from source to target already exists.
fn output_edge_exists(context: &Context, source: &NodeId, target: &NodeId, relationship: &str) -> bool {
    context.edges().any(|e| {
        e.source == *source && e.target == *target && e.relationship == relationship
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, EdgeId, Node};

    fn concept_node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n.dimension = dimension::SEMANTIC.to_string();
        n
    }

    fn edges_added_event() -> GraphEvent {
        GraphEvent::EdgesAdded {
            edge_ids: vec![EdgeId::from_string("e1")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }
    }

    // === Scenario 1: Discovery gap detected between latently similar
    //     but structurally unconnected nodes ===

    #[test]
    fn discovery_gap_detected_when_no_structural_evidence() {
        let enrichment = DiscoveryGapEnrichment::new("similar_to", "discovery_gap");

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:alpha"));
        ctx.add_node(concept_node("concept:bravo"));

        // Trigger edge: similar_to with contribution 0.85
        let mut trigger = Edge::new_in_dimension(
            NodeId::from_string("concept:alpha"),
            NodeId::from_string("concept:bravo"),
            "similar_to",
            dimension::SEMANTIC,
        );
        trigger.raw_weight = 0.85;
        ctx.add_edge(trigger);

        let emission = enrichment
            .enrich(&[edges_added_event()], &ctx)
            .expect("should emit discovery_gap edges");

        // Symmetric pair
        assert_eq!(emission.edges.len(), 2);

        let alpha = NodeId::from_string("concept:alpha");
        let bravo = NodeId::from_string("concept:bravo");

        let forward = emission.edges.iter().find(|ae| {
            ae.edge.source == alpha && ae.edge.target == bravo
                && ae.edge.relationship == "discovery_gap"
        });
        let reverse = emission.edges.iter().find(|ae| {
            ae.edge.source == bravo && ae.edge.target == alpha
                && ae.edge.relationship == "discovery_gap"
        });

        assert!(forward.is_some(), "alpha→bravo discovery_gap");
        assert!(reverse.is_some(), "bravo→alpha discovery_gap");

        // Contribution equals trigger edge's contribution
        assert_eq!(forward.unwrap().edge.raw_weight, 0.85);
        assert_eq!(reverse.unwrap().edge.raw_weight, 0.85);
    }

    // === Scenario 2: No discovery gap when structural evidence already exists ===

    #[test]
    fn no_gap_when_structural_evidence_exists() {
        let enrichment = DiscoveryGapEnrichment::new("similar_to", "discovery_gap");

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:alpha"));
        ctx.add_node(concept_node("concept:bravo"));

        // Structural edge: already connected
        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("concept:alpha"),
            NodeId::from_string("concept:bravo"),
            "may_be_related",
            dimension::SEMANTIC,
        ));

        // Trigger edge
        let mut trigger = Edge::new_in_dimension(
            NodeId::from_string("concept:alpha"),
            NodeId::from_string("concept:bravo"),
            "similar_to",
            dimension::SEMANTIC,
        );
        trigger.raw_weight = 0.9;
        ctx.add_edge(trigger);

        assert!(
            enrichment.enrich(&[edges_added_event()], &ctx).is_none(),
            "should NOT emit when structural evidence exists"
        );
    }

    // === Scenario 3: Discovery gap enrichment reaches quiescence ===

    #[test]
    fn reaches_quiescence() {
        let enrichment = DiscoveryGapEnrichment::new("similar_to", "discovery_gap");

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:alpha"));
        ctx.add_node(concept_node("concept:bravo"));

        let mut trigger = Edge::new_in_dimension(
            NodeId::from_string("concept:alpha"),
            NodeId::from_string("concept:bravo"),
            "similar_to",
            dimension::SEMANTIC,
        );
        trigger.raw_weight = 0.85;
        ctx.add_edge(trigger);

        // Round 1: productive
        let emission = enrichment
            .enrich(&[edges_added_event()], &ctx)
            .expect("round 1 should emit");

        // Commit round 1 emissions
        for ae in &emission.edges {
            ctx.add_edge(ae.edge.clone());
        }

        // Round 2: quiescent
        let round2_events = vec![GraphEvent::EdgesAdded {
            edge_ids: emission.edges.iter().map(|ae| {
                EdgeId::from_string(&format!("{}->{}",
                    ae.edge.source.as_str(), ae.edge.target.as_str()))
            }).collect(),
            adapter_id: "discovery_gap:similar_to:discovery_gap".to_string(),
            context_id: "test".to_string(),
        }];

        assert!(
            enrichment.enrich(&round2_events, &ctx).is_none(),
            "round 2 should be quiescent"
        );
    }

    // === Scenario 4: DiscoveryGapEnrichment has unique stable ID ===

    #[test]
    fn stable_id() {
        let enrichment = DiscoveryGapEnrichment::new("similar_to", "discovery_gap");
        assert_eq!(enrichment.id(), "discovery_gap:similar_to:discovery_gap");
    }
}
