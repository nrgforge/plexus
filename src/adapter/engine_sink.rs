//! EngineSink: the AdapterSink implementation backed by the graph engine
//!
//! Validates each item in an emission independently:
//! - Nodes: upsert (duplicate ID updates properties)
//! - Edges: reject if either endpoint missing from graph or same emission
//! - Removals: no-op if node doesn't exist; cascade connected edges
//! - Empty emission: no-op

use super::events::GraphEvent;
use super::provenance::{FrameworkContext, ProvenanceEntry};
use super::sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
use super::types::Emission;
use crate::graph::{Context, EdgeId, NodeId};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};

/// An AdapterSink backed by a mutable Context.
///
/// Validates and commits emissions per the rules in ADR-001 §6:
/// - Per-item validation (not all-or-nothing)
/// - Edges with missing endpoints are rejected
/// - Duplicate node IDs upsert
/// - Removal of non-existent nodes is a no-op
/// - Self-referencing edges are allowed
///
/// When a FrameworkContext is provided, constructs ProvenanceEntry records
/// for each committed node by combining adapter annotations with framework context.
pub struct EngineSink {
    context: Arc<Mutex<Context>>,
    framework: Option<FrameworkContext>,
}

impl EngineSink {
    pub fn new(context: Arc<Mutex<Context>>) -> Self {
        Self { context, framework: None }
    }

    pub fn with_framework_context(mut self, framework: FrameworkContext) -> Self {
        self.framework = Some(framework);
        self
    }
}

#[async_trait]
impl AdapterSink for EngineSink {
    async fn emit(&self, emission: Emission) -> Result<EmitResult, AdapterError> {
        let mut ctx = self.context.lock().map_err(|e| {
            AdapterError::Internal(format!("lock poisoned: {}", e))
        })?;

        if emission.is_empty() {
            return Ok(EmitResult::empty());
        }

        let mut result = EmitResult::empty();
        let timestamp = Utc::now();

        let adapter_id = self.framework.as_ref().map(|fw| fw.adapter_id.clone())
            .unwrap_or_default();
        let context_id = self.framework.as_ref().map(|fw| fw.context_id.clone())
            .unwrap_or_default();

        // Phase 1: Commit nodes (upsert semantics)
        let mut committed_node_ids: Vec<NodeId> = Vec::new();
        for annotated_node in emission.nodes {
            let node_id = annotated_node.node.id.clone();
            let annotation = annotated_node.annotation;

            ctx.add_node(annotated_node.node);
            result.nodes_committed += 1;
            committed_node_ids.push(node_id.clone());

            if let Some(ref fw) = self.framework {
                let entry = ProvenanceEntry::from_context(fw, timestamp, annotation);
                result.provenance.push((node_id, entry));
            }
        }

        // Phase 2: Validate and commit edges
        let mut committed_edge_ids: Vec<EdgeId> = Vec::new();
        for annotated_edge in emission.edges {
            let edge = &annotated_edge.edge;
            let source_exists = ctx.get_node(&edge.source).is_some();
            let target_exists = ctx.get_node(&edge.target).is_some();

            if !source_exists {
                result.rejections.push(Rejection::new(
                    format!("edge {}→{}", edge.source, edge.target),
                    RejectionReason::MissingEndpoint(edge.source.clone()),
                ));
                continue;
            }

            if !target_exists {
                result.rejections.push(Rejection::new(
                    format!("edge {}→{}", edge.source, edge.target),
                    RejectionReason::MissingEndpoint(edge.target.clone()),
                ));
                continue;
            }

            let edge_id = annotated_edge.edge.id.clone();
            ctx.add_edge(annotated_edge.edge);
            result.edges_committed += 1;
            committed_edge_ids.push(edge_id);
        }

        // Phase 3: Process removals
        let mut removed_node_ids: Vec<NodeId> = Vec::new();
        let mut cascaded_edge_ids: Vec<EdgeId> = Vec::new();
        for removal in emission.removals {
            if ctx.get_node(&removal.node_id).is_some() {
                // Collect cascaded edge IDs before removing
                for edge in ctx.edges.iter() {
                    if edge.source == removal.node_id || edge.target == removal.node_id {
                        cascaded_edge_ids.push(edge.id.clone());
                    }
                }
                ctx.nodes.remove(&removal.node_id);
                ctx.edges.retain(|e| {
                    e.source != removal.node_id && e.target != removal.node_id
                });
                result.removals_committed += 1;
                removed_node_ids.push(removal.node_id);
            }
        }

        // Phase 4: Fire graph events (order: NodesAdded, EdgesAdded, NodesRemoved, EdgesRemoved)
        if !committed_node_ids.is_empty() {
            result.events.push(GraphEvent::NodesAdded {
                node_ids: committed_node_ids,
                adapter_id: adapter_id.clone(),
                context_id: context_id.clone(),
            });
        }
        if !committed_edge_ids.is_empty() {
            result.events.push(GraphEvent::EdgesAdded {
                edge_ids: committed_edge_ids,
                adapter_id: adapter_id.clone(),
                context_id: context_id.clone(),
            });
        }
        if !removed_node_ids.is_empty() {
            result.events.push(GraphEvent::NodesRemoved {
                node_ids: removed_node_ids,
                adapter_id: adapter_id.clone(),
                context_id: context_id.clone(),
            });
        }
        if !cascaded_edge_ids.is_empty() {
            result.events.push(GraphEvent::EdgesRemoved {
                edge_ids: cascaded_edge_ids,
                adapter_id: adapter_id.clone(),
                context_id: context_id.clone(),
                reason: "cascade".to_string(),
            });
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Emission;
    use crate::graph::{ContentType, Edge, Node, NodeId};

    fn make_sink() -> (EngineSink, Arc<Mutex<Context>>) {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        (sink, ctx)
    }

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    fn edge(source: &str, target: &str) -> Edge {
        Edge::new(
            NodeId::from_string(source),
            NodeId::from_string(target),
            "related_to",
        )
    }

    // === Scenario: Valid emission with nodes and edges commits successfully ===
    #[tokio::test]
    async fn valid_emission_commits_all() {
        let (sink, ctx) = make_sink();

        let a = node("A");
        let b = node("B");
        let e = edge("A", "B");

        let emission = Emission::new()
            .with_node(a)
            .with_node(b)
            .with_edge(e);

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 2);
        assert_eq!(result.edges_committed, 1);
        assert!(result.is_fully_committed());

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("B")).is_some());
        assert_eq!(ctx.edge_count(), 1);
    }

    // === Scenario: Edge referencing missing endpoint rejected; valid items commit ===
    #[tokio::test]
    async fn edge_missing_endpoint_rejected_valid_items_commit() {
        let (sink, ctx) = make_sink();

        // Pre-populate node A
        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
        }

        let emission = Emission::new()
            .with_node(node("B"))
            .with_edge(edge("A", "B"))
            .with_edge(edge("B", "C")); // C doesn't exist

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 1); // B
        assert_eq!(result.edges_committed, 1); // A→B
        assert_eq!(result.rejections.len(), 1); // B→C rejected

        let rejection = &result.rejections[0];
        assert_eq!(
            rejection.reason,
            RejectionReason::MissingEndpoint(NodeId::from_string("C"))
        );
    }

    // === Scenario: Edge endpoints satisfied within same emission ===
    #[tokio::test]
    async fn edge_endpoints_from_same_emission() {
        let (sink, ctx) = make_sink();

        let emission = Emission::new()
            .with_node(node("X"))
            .with_node(node("Y"))
            .with_edge(edge("X", "Y"));

        let result = sink.emit(emission).await.unwrap();

        assert!(result.is_fully_committed());
        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edge_count(), 1);
    }

    // === Scenario: Edge endpoint from prior emission ===
    #[tokio::test]
    async fn edge_endpoint_from_prior_emission() {
        let (sink, ctx) = make_sink();

        // Prior emission
        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
        }

        let emission = Emission::new()
            .with_node(node("B"))
            .with_edge(edge("A", "B"));

        let result = sink.emit(emission).await.unwrap();
        assert!(result.is_fully_committed());
    }

    // === Scenario: Duplicate node ID causes upsert ===
    #[tokio::test]
    async fn duplicate_node_id_upserts() {
        let (sink, ctx) = make_sink();

        // First emission: node A with name=alpha
        let mut a1 = node("A");
        a1.properties.insert(
            "name".to_string(),
            crate::graph::PropertyValue::String("alpha".to_string()),
        );
        sink.emit(Emission::new().with_node(a1)).await.unwrap();

        // Second emission: node A with name=alpha-updated
        let mut a2 = node("A");
        a2.properties.insert(
            "name".to_string(),
            crate::graph::PropertyValue::String("alpha-updated".to_string()),
        );
        sink.emit(Emission::new().with_node(a2)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        let updated = ctx.get_node(&NodeId::from_string("A")).unwrap();
        assert_eq!(
            updated.properties.get("name"),
            Some(&crate::graph::PropertyValue::String("alpha-updated".to_string()))
        );
        // Only one node with ID A
        assert_eq!(ctx.node_count(), 1);
    }

    // === Scenario: Removal of non-existent node is no-op ===
    #[tokio::test]
    async fn removal_nonexistent_is_noop() {
        let (sink, ctx) = make_sink();

        let emission = Emission::new().with_removal(NodeId::from_string("Z"));
        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.removals_committed, 0);
        assert!(result.rejections.is_empty());

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.node_count(), 0);
    }

    // === Scenario: Empty emission is no-op ===
    #[tokio::test]
    async fn empty_emission_is_noop() {
        let (sink, _ctx) = make_sink();

        let result = sink.emit(Emission::new()).await.unwrap();
        assert!(result.is_noop());
    }

    // === Scenario: Self-referencing edge is allowed ===
    #[tokio::test]
    async fn self_referencing_edge_allowed() {
        let (sink, ctx) = make_sink();

        let emission = Emission::new()
            .with_node(node("A"))
            .with_edge(edge("A", "A"));

        let result = sink.emit(emission).await.unwrap();

        assert!(result.is_fully_committed());
        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edge_count(), 1);
        assert_eq!(ctx.edges[0].source, ctx.edges[0].target);
    }

    // === Scenario: Bad edge rejected; valid items in same emission commit ===
    #[tokio::test]
    async fn bad_edge_rejected_valid_items_commit() {
        let (sink, ctx) = make_sink();

        let emission = Emission::new()
            .with_node(node("A"))
            .with_node(node("B"))
            .with_edge(edge("A", "B"))
            .with_edge(edge("A", "Z")); // Z doesn't exist

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 2);
        assert_eq!(result.edges_committed, 1);
        assert_eq!(result.rejections.len(), 1);

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("B")).is_some());
        assert_eq!(ctx.edge_count(), 1);
    }

    // === Scenario: Node removal cascades to connected edges ===
    #[tokio::test]
    async fn node_removal_cascades_edges() {
        let (sink, ctx) = make_sink();

        // Setup: A, B, edge A→B
        let setup = Emission::new()
            .with_node(node("A"))
            .with_node(node("B"))
            .with_edge(edge("A", "B"));
        sink.emit(setup).await.unwrap();

        // Remove A
        let removal = Emission::new().with_removal(NodeId::from_string("A"));
        let result = sink.emit(removal).await.unwrap();

        assert_eq!(result.removals_committed, 1);

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_none());
        assert_eq!(ctx.edge_count(), 0); // cascade
    }

    // === Scenario: All edges missing endpoints; nodes still commit ===
    #[tokio::test]
    async fn all_edges_rejected_nodes_commit() {
        let (sink, ctx) = make_sink();

        let emission = Emission::new()
            .with_node(node("A"))
            .with_edge(edge("A", "Z")); // Z doesn't exist

        let result = sink.emit(emission).await.unwrap();

        assert_eq!(result.nodes_committed, 1);
        assert_eq!(result.edges_committed, 0);
        assert_eq!(result.rejections.len(), 1);

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert_eq!(ctx.edge_count(), 0);
    }

    // === Additional: Edge with raw weight is preserved ===
    #[tokio::test]
    async fn edge_raw_weight_preserved() {
        let (sink, ctx) = make_sink();

        let mut e = edge("A", "B");
        e.raw_weight = 0.42;

        let emission = Emission::new()
            .with_node(node("A"))
            .with_node(node("B"))
            .with_edge(e);

        sink.emit(emission).await.unwrap();

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].raw_weight, 0.42);
    }

    // ================================================================
    // Provenance Construction Scenarios
    // ================================================================

    fn make_sink_with_provenance() -> (EngineSink, Arc<Mutex<Context>>) {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let fw = FrameworkContext {
            adapter_id: "document-adapter".to_string(),
            context_id: "manza-session-1".to_string(),
            input_summary: Some("file.md".to_string()),
        };
        let sink = EngineSink::new(ctx.clone()).with_framework_context(fw);
        (sink, ctx)
    }

    // === Scenario: Annotated node receives full provenance entry ===
    #[tokio::test]
    async fn annotated_node_receives_full_provenance() {
        use crate::adapter::Annotation;

        let (sink, _ctx) = make_sink_with_provenance();

        let annotation = Annotation::new()
            .with_confidence(0.85)
            .with_method("llm-extraction")
            .with_source_location("file.md:87");

        let annotated = crate::adapter::AnnotatedNode::new(node("A"))
            .with_annotation(annotation);

        let result = sink.emit(Emission::new().with_node(annotated)).await.unwrap();

        assert_eq!(result.provenance.len(), 1);
        let (ref node_id, ref entry) = result.provenance[0];
        assert_eq!(node_id.as_str(), "A");
        assert_eq!(entry.adapter_id, "document-adapter");
        assert_eq!(entry.context_id, "manza-session-1");
        assert_eq!(entry.input_summary.as_deref(), Some("file.md"));

        let ann = entry.annotation.as_ref().unwrap();
        assert_eq!(ann.confidence, Some(0.85));
        assert_eq!(ann.method.as_deref(), Some("llm-extraction"));
        assert_eq!(ann.source_location.as_deref(), Some("file.md:87"));
    }

    // === Scenario: Node without annotation receives structural provenance ===
    #[tokio::test]
    async fn node_without_annotation_gets_structural_provenance() {
        let (sink, _ctx) = make_sink_with_provenance();

        let result = sink.emit(Emission::new().with_node(node("B"))).await.unwrap();

        assert_eq!(result.provenance.len(), 1);
        let (ref node_id, ref entry) = result.provenance[0];
        assert_eq!(node_id.as_str(), "B");
        assert_eq!(entry.adapter_id, "document-adapter");
        assert_eq!(entry.context_id, "manza-session-1");
        assert!(entry.annotation.is_none());
    }

    // === Scenario: Each emission gets its own timestamp ===
    #[tokio::test]
    async fn each_emission_gets_own_timestamp() {
        let (sink, _ctx) = make_sink_with_provenance();

        let r1 = sink.emit(Emission::new().with_node(node("A"))).await.unwrap();
        // Small delay to ensure distinct timestamps
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let r2 = sink.emit(Emission::new().with_node(node("B"))).await.unwrap();

        let t1 = r1.provenance[0].1.timestamp;
        let t2 = r2.provenance[0].1.timestamp;
        assert!(t2 >= t1, "second emission timestamp should be >= first");
    }

    // === Scenario: Multiple nodes in one emission share framework context ===
    #[tokio::test]
    async fn multiple_nodes_share_framework_context() {
        use crate::adapter::{Annotation, AnnotatedNode};

        let (sink, _ctx) = make_sink_with_provenance();

        let a = AnnotatedNode::new(node("A"))
            .with_annotation(Annotation::new().with_confidence(0.9));
        let b = AnnotatedNode::new(node("B"))
            .with_annotation(Annotation::new().with_confidence(0.6));

        let result = sink.emit(Emission::new().with_node(a).with_node(b)).await.unwrap();

        assert_eq!(result.provenance.len(), 2);

        // Both share adapter_id, context_id, timestamp
        let (_, ref e1) = result.provenance[0];
        let (_, ref e2) = result.provenance[1];
        assert_eq!(e1.adapter_id, e2.adapter_id);
        assert_eq!(e1.context_id, e2.context_id);
        assert_eq!(e1.timestamp, e2.timestamp);

        // But annotations differ
        assert_eq!(e1.annotation.as_ref().unwrap().confidence, Some(0.9));
        assert_eq!(e2.annotation.as_ref().unwrap().confidence, Some(0.6));
    }

    // === No provenance when FrameworkContext not set ===
    #[tokio::test]
    async fn no_provenance_without_framework_context() {
        let (sink, _ctx) = make_sink();

        let result = sink.emit(Emission::new().with_node(node("A"))).await.unwrap();
        assert!(result.provenance.is_empty());
    }

    // ================================================================
    // Graph Events Scenarios
    // ================================================================

    // === Scenario: NodesAdded event on successful emission ===
    #[tokio::test]
    async fn nodes_added_event_fires() {
        let (sink, _ctx) = make_sink();

        let result = sink.emit(
            Emission::new().with_node(node("A")).with_node(node("B"))
        ).await.unwrap();

        let nodes_added = result.events.iter().find(|e| matches!(e, GraphEvent::NodesAdded { .. }));
        assert!(nodes_added.is_some());
        if let Some(GraphEvent::NodesAdded { node_ids, .. }) = nodes_added {
            assert_eq!(node_ids.len(), 2);
            assert!(node_ids.contains(&NodeId::from_string("A")));
            assert!(node_ids.contains(&NodeId::from_string("B")));
        }
    }

    // === Scenario: EdgesAdded event on successful emission ===
    #[tokio::test]
    async fn edges_added_event_fires() {
        let (sink, ctx) = make_sink();

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let result = sink.emit(
            Emission::new().with_edge(edge("A", "B"))
        ).await.unwrap();

        let edges_added = result.events.iter().find(|e| matches!(e, GraphEvent::EdgesAdded { .. }));
        assert!(edges_added.is_some());
        if let Some(GraphEvent::EdgesAdded { edge_ids, .. }) = edges_added {
            assert_eq!(edge_ids.len(), 1);
        }
    }

    // === Scenario: NodesRemoved event on removal ===
    #[tokio::test]
    async fn nodes_removed_event_fires() {
        let (sink, ctx) = make_sink();

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
        }

        let result = sink.emit(
            Emission::new().with_removal(NodeId::from_string("A"))
        ).await.unwrap();

        let nodes_removed = result.events.iter().find(|e| matches!(e, GraphEvent::NodesRemoved { .. }));
        assert!(nodes_removed.is_some());
        if let Some(GraphEvent::NodesRemoved { node_ids, .. }) = nodes_removed {
            assert_eq!(node_ids.len(), 1);
            assert!(node_ids.contains(&NodeId::from_string("A")));
        }
    }

    // === Scenario: EdgesRemoved event on cascade from node removal ===
    #[tokio::test]
    async fn edges_removed_cascade_event_fires() {
        let (sink, _ctx) = make_sink();

        // Setup
        sink.emit(
            Emission::new()
                .with_node(node("A"))
                .with_node(node("B"))
                .with_edge(edge("A", "B"))
        ).await.unwrap();

        // Remove A — should cascade edge A→B
        let result = sink.emit(
            Emission::new().with_removal(NodeId::from_string("A"))
        ).await.unwrap();

        let nodes_removed = result.events.iter().find(|e| matches!(e, GraphEvent::NodesRemoved { .. }));
        assert!(nodes_removed.is_some());

        let edges_removed = result.events.iter().find(|e| matches!(e, GraphEvent::EdgesRemoved { .. }));
        assert!(edges_removed.is_some());
        if let Some(GraphEvent::EdgesRemoved { edge_ids, reason, .. }) = edges_removed {
            assert_eq!(edge_ids.len(), 1);
            assert_eq!(reason, "cascade");
        }
    }

    // === Scenario: No events for rejected items; events for committed ===
    #[tokio::test]
    async fn no_events_for_rejected_items() {
        let (sink, _ctx) = make_sink();

        // Node A committed; edges A→B and A→C rejected (B, C missing)
        let result = sink.emit(
            Emission::new()
                .with_node(node("A"))
                .with_edge(edge("A", "B"))
                .with_edge(edge("A", "C"))
        ).await.unwrap();

        assert_eq!(result.nodes_committed, 1);
        assert_eq!(result.edges_committed, 0);

        // NodesAdded event fires for A
        let nodes_added = result.events.iter().find(|e| matches!(e, GraphEvent::NodesAdded { .. }));
        assert!(nodes_added.is_some());

        // No EdgesAdded event
        let edges_added = result.events.iter().find(|e| matches!(e, GraphEvent::EdgesAdded { .. }));
        assert!(edges_added.is_none());
    }

    // === Scenario: Only invalid edges — no edge events ===
    #[tokio::test]
    async fn only_invalid_edges_no_events() {
        let (sink, _ctx) = make_sink();

        let result = sink.emit(
            Emission::new().with_edge(edge("X", "Y"))
        ).await.unwrap();

        // No events at all
        assert!(result.events.is_empty());
    }

    // === Scenario: No events on empty emission ===
    #[tokio::test]
    async fn no_events_on_empty_emission() {
        let (sink, _ctx) = make_sink();

        let result = sink.emit(Emission::new()).await.unwrap();
        assert!(result.events.is_empty());
    }

    // === Scenario: Events include nodes and edges, NodesAdded before EdgesAdded ===
    #[tokio::test]
    async fn events_order_nodes_before_edges() {
        let (sink, _ctx) = make_sink();

        let result = sink.emit(
            Emission::new()
                .with_node(node("A"))
                .with_node(node("B"))
                .with_edge(edge("A", "B"))
        ).await.unwrap();

        assert_eq!(result.events.len(), 2);
        assert!(matches!(&result.events[0], GraphEvent::NodesAdded { .. }));
        assert!(matches!(&result.events[1], GraphEvent::EdgesAdded { .. }));
    }
}
