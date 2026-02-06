//! ProposalSink: constrained wrapper for reflexive adapters
//!
//! Intercepts emissions before the engine sees them. Enforces the
//! propose-don't-merge invariant structurally:
//! - Only `may_be_related` edges allowed
//! - Edge raw weights clamped to a configurable cap
//! - Node removals rejected
//! - Nodes and annotations pass through

use super::sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
use super::types::{AnnotatedEdge, Emission, Removal};
use async_trait::async_trait;

/// The relationship type reflexive adapters are allowed to emit.
pub const ALLOWED_RELATIONSHIP: &str = "may_be_related";

/// A constrained wrapper around an AdapterSink for reflexive adapters.
///
/// The adapter's `process()` signature is unchanged — it still receives
/// `&dyn AdapterSink` — but the implementation enforces proposal constraints.
pub struct ProposalSink<S: AdapterSink> {
    inner: S,
    weight_cap: f32,
}

impl<S: AdapterSink> ProposalSink<S> {
    pub fn new(inner: S, weight_cap: f32) -> Self {
        Self { inner, weight_cap }
    }
}

#[async_trait]
impl<S: AdapterSink> AdapterSink for ProposalSink<S> {
    async fn emit(&self, emission: Emission) -> Result<EmitResult, AdapterError> {
        if emission.is_empty() {
            return self.inner.emit(emission).await;
        }

        let mut filtered_edges: Vec<AnnotatedEdge> = Vec::new();
        let mut rejections: Vec<Rejection> = Vec::new();
        let filtered_removals: Vec<Removal> = Vec::new();

        // Filter edges: only may_be_related, clamp weights
        for mut annotated_edge in emission.edges {
            if annotated_edge.edge.relationship != ALLOWED_RELATIONSHIP {
                rejections.push(Rejection::new(
                    format!(
                        "edge {}→{} (relationship: {})",
                        annotated_edge.edge.source,
                        annotated_edge.edge.target,
                        annotated_edge.edge.relationship,
                    ),
                    RejectionReason::InvalidRelationshipType(
                        annotated_edge.edge.relationship.clone(),
                    ),
                ));
                continue;
            }

            // Clamp weight to cap
            if annotated_edge.edge.raw_weight > self.weight_cap {
                annotated_edge.edge.raw_weight = self.weight_cap;
            }

            filtered_edges.push(annotated_edge);
        }

        // Reject all removals
        for removal in emission.removals {
            rejections.push(Rejection::new(
                format!("removal of node {}", removal.node_id),
                RejectionReason::RemovalNotAllowed,
            ));
            // Don't add to filtered_removals — it's rejected
        }

        // Forward filtered emission to inner sink
        let filtered_emission = Emission {
            nodes: emission.nodes, // nodes pass through
            edges: filtered_edges,
            removals: filtered_removals,
        };

        let mut inner_result = self.inner.emit(filtered_emission).await?;

        // Merge our rejections with any from the inner sink
        let mut all_rejections = rejections;
        all_rejections.append(&mut inner_result.rejections);
        inner_result.rejections = all_rejections;

        Ok(inner_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{EngineSink, Emission};
    use crate::graph::{ContentType, Context, Edge, Node, NodeId};
    use std::sync::{Arc, Mutex};

    fn make_proposal_sink(weight_cap: f32) -> (ProposalSink<EngineSink>, Arc<Mutex<Context>>) {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let engine_sink = EngineSink::new(ctx.clone());
        let proposal_sink = ProposalSink::new(engine_sink, weight_cap);
        (proposal_sink, ctx)
    }

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    fn may_be_related_edge(source: &str, target: &str, raw_weight: f32) -> Edge {
        let mut e = Edge::new(
            NodeId::from_string(source),
            NodeId::from_string(target),
            "may_be_related",
        );
        e.raw_weight = raw_weight;
        e
    }

    // === Scenario: may_be_related edge passes through ProposalSink ===
    #[tokio::test]
    async fn may_be_related_edge_passes_through() {
        let (sink, ctx) = make_proposal_sink(0.3);

        // Pre-populate nodes
        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let emission = Emission::new()
            .with_edge(may_be_related_edge("A", "B", 0.2));

        let result = sink.emit(emission).await.unwrap();

        assert!(result.is_fully_committed());
        assert_eq!(result.edges_committed, 1);

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].raw_weight, 0.2);
    }

    // === Scenario: Non-may_be_related edge is rejected ===
    #[tokio::test]
    async fn non_may_be_related_edge_rejected() {
        let (sink, ctx) = make_proposal_sink(0.3);

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let edge = Edge::new(
            NodeId::from_string("A"),
            NodeId::from_string("B"),
            "related_to",
        );
        let emission = Emission::new().with_edge(edge);

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.edges_committed, 0);
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(
            result.rejections[0].reason,
            RejectionReason::InvalidRelationshipType("related_to".to_string())
        );

        // Edge should not reach the engine
        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edge_count(), 0);
    }

    // === Scenario: Edge raw weight exceeding cap is clamped ===
    #[tokio::test]
    async fn edge_weight_exceeding_cap_is_clamped() {
        let (sink, ctx) = make_proposal_sink(0.3);

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let emission = Emission::new()
            .with_edge(may_be_related_edge("A", "B", 0.8));

        let result = sink.emit(emission).await.unwrap();

        assert!(result.is_fully_committed());

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].raw_weight, 0.3); // clamped
    }

    // === Scenario: Node removal is rejected ===
    #[tokio::test]
    async fn node_removal_rejected() {
        let (sink, ctx) = make_proposal_sink(0.3);

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
        }

        let emission = Emission::new()
            .with_removal(NodeId::from_string("A"));

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.removals_committed, 0);
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(result.rejections[0].reason, RejectionReason::RemovalNotAllowed);

        // Node should still exist
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
    }

    // === Scenario: Node emission is allowed ===
    #[tokio::test]
    async fn node_emission_allowed() {
        let (sink, ctx) = make_proposal_sink(0.3);

        let emission = Emission::new()
            .with_node(node("M"));

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 1);
        assert!(result.is_fully_committed());

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("M")).is_some());
    }

    // === Scenario: Annotation on node passes through ===
    #[tokio::test]
    async fn annotation_on_node_passes_through() {
        let (sink, ctx) = make_proposal_sink(0.3);

        use crate::adapter::{Annotation, AnnotatedNode};

        let annotation = Annotation::new()
            .with_confidence(0.7)
            .with_method("near-miss-detection");

        let annotated = AnnotatedNode::new(node("M"))
            .with_annotation(annotation);

        let emission = Emission::new().with_node(annotated);

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 1);
        assert!(result.is_fully_committed());

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("M")).is_some());
    }

    // === Scenario: Mixed emission with valid nodes and invalid edge type ===
    #[tokio::test]
    async fn mixed_emission_valid_nodes_invalid_edge_type() {
        let (sink, ctx) = make_proposal_sink(0.3);

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
        }

        let contains_edge = Edge::new(
            NodeId::from_string("M"),
            NodeId::from_string("A"),
            "contains",
        );

        let emission = Emission::new()
            .with_node(node("M"))
            .with_edge(contains_edge);

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 1); // M committed
        assert_eq!(result.edges_committed, 0); // contains rejected
        assert_eq!(result.rejections.len(), 1);
        assert_eq!(
            result.rejections[0].reason,
            RejectionReason::InvalidRelationshipType("contains".to_string())
        );

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("M")).is_some());
        assert_eq!(ctx.edge_count(), 0);
    }

    // === Scenario: Weight at exactly the cap is not clamped ===
    #[tokio::test]
    async fn weight_at_cap_not_clamped() {
        let (sink, ctx) = make_proposal_sink(0.3);

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let emission = Emission::new()
            .with_edge(may_be_related_edge("A", "B", 0.3));

        let result = sink.emit(emission).await.unwrap();

        assert!(result.is_fully_committed());

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].raw_weight, 0.3); // exactly at cap, no change
    }
}
