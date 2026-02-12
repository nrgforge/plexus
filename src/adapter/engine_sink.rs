//! EngineSink: the AdapterSink implementation backed by the graph engine
//!
//! Validates each item in an emission independently:
//! - Nodes: upsert (duplicate ID updates properties)
//! - Edges: reject if either endpoint missing from graph or same emission
//! - Removals: no-op if node doesn't exist; cascade connected edges
//! - Empty emission: no-op

use super::enrichment::EnrichmentRegistry;
use super::events::GraphEvent;
use super::provenance::{FrameworkContext, ProvenanceEntry};
use super::sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
use super::types::Emission;
use crate::graph::{Context, ContextId, EdgeId, NodeId, PlexusEngine};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};

/// The backend that provides mutable context access.
enum SinkBackend {
    /// Test path: direct mutex around a context (no persistence)
    Mutex(Arc<Mutex<Context>>),
    /// Engine path: routes through PlexusEngine with persist-per-emission (ADR-006)
    Engine {
        engine: Arc<PlexusEngine>,
        context_id: ContextId,
    },
}

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
///
/// Two construction paths (ADR-006):
/// - `new()`: test path using `Arc<Mutex<Context>>`, no persistence
/// - `for_engine()`: engine path using PlexusEngine with persist-per-emission
pub struct EngineSink {
    backend: SinkBackend,
    framework: Option<FrameworkContext>,
    enrichments: Option<Arc<EnrichmentRegistry>>,
    /// Events accumulated across all emit() calls (for pipeline event collection).
    accumulated_events: Mutex<Vec<GraphEvent>>,
}

impl EngineSink {
    /// Create a sink backed by a bare Mutex (test path, no persistence).
    pub fn new(context: Arc<Mutex<Context>>) -> Self {
        Self {
            backend: SinkBackend::Mutex(context),
            framework: None,
            enrichments: None,
            accumulated_events: Mutex::new(Vec::new()),
        }
    }

    /// Create a sink backed by PlexusEngine (ADR-006).
    ///
    /// Emissions route through `engine.with_context_mut()`, which persists
    /// the context to storage after each emission completes.
    pub fn for_engine(engine: Arc<PlexusEngine>, context_id: ContextId) -> Self {
        Self {
            backend: SinkBackend::Engine { engine, context_id },
            framework: None,
            enrichments: None,
            accumulated_events: Mutex::new(Vec::new()),
        }
    }

    pub fn with_framework_context(mut self, framework: FrameworkContext) -> Self {
        self.framework = Some(framework);
        self
    }

    /// Attach an enrichment registry (ADR-010).
    ///
    /// After each primary emission (Engine backend only), the enrichment loop
    /// runs: enrichments receive events and a context snapshot, and may produce
    /// additional emissions committed through the same path.
    pub fn with_enrichments(mut self, registry: Arc<EnrichmentRegistry>) -> Self {
        self.enrichments = Some(registry);
        self
    }

    /// Drain and return all events accumulated across emit() calls.
    ///
    /// Used by the ingest pipeline to collect events from adapter processing
    /// without going through the AdapterSink trait.
    pub fn take_accumulated_events(&self) -> Vec<GraphEvent> {
        std::mem::take(&mut *self.accumulated_events.lock().unwrap())
    }

    /// Core emission logic operating on a mutable Context reference.
    ///
    /// Extracted so both the Mutex path and future engine path share
    /// identical validation, contribution tracking, and event logic.
    pub(crate) fn emit_inner(
        ctx: &mut Context,
        emission: Emission,
        framework: &Option<FrameworkContext>,
    ) -> Result<EmitResult, AdapterError> {
        if emission.is_empty() {
            return Ok(EmitResult::empty());
        }

        let mut result = EmitResult::empty();
        let timestamp = Utc::now();

        let adapter_id = framework.as_ref().map(|fw| fw.adapter_id.clone())
            .unwrap_or_default();
        let context_id = framework.as_ref().map(|fw| fw.context_id.clone())
            .unwrap_or_default();

        // Phase 1: Commit nodes (upsert semantics)
        let mut committed_node_ids: Vec<NodeId> = Vec::new();
        for annotated_node in emission.nodes {
            let node_id = annotated_node.node.id.clone();
            let annotation = annotated_node.annotation;

            ctx.add_node(annotated_node.node);
            result.nodes_committed += 1;
            committed_node_ids.push(node_id.clone());

            if let Some(ref fw) = framework {
                let entry = ProvenanceEntry::from_context(fw, timestamp, annotation);
                result.provenance.push((node_id, entry));
            }
        }

        // Phase 2: Validate and commit edges
        let mut committed_edge_ids: Vec<EdgeId> = Vec::new();
        let mut weights_changed_edge_ids: Vec<EdgeId> = Vec::new();
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

            let mut edge_to_commit = annotated_edge.edge;

            // ADR-003: Set contribution for the emitting adapter
            let contribution_value = edge_to_commit.raw_weight;
            if !adapter_id.is_empty() {
                edge_to_commit.contributions.insert(
                    adapter_id.clone(),
                    contribution_value,
                );
            }

            // ADR-003: Detect contribution change for WeightsChanged event
            let mut contribution_changed = false;
            if !adapter_id.is_empty() {
                if let Some(existing) = ctx.edges.iter().find(|e| {
                    e.source == edge_to_commit.source
                        && e.target == edge_to_commit.target
                        && e.relationship == edge_to_commit.relationship
                        && e.source_dimension == edge_to_commit.source_dimension
                        && e.target_dimension == edge_to_commit.target_dimension
                }) {
                    let old_value = existing.contributions.get(&adapter_id);
                    let new_value = Some(&contribution_value);
                    contribution_changed = old_value != new_value;
                }
            }

            let edge_id = edge_to_commit.id.clone();
            ctx.add_edge(edge_to_commit);
            result.edges_committed += 1;
            committed_edge_ids.push(edge_id.clone());

            if contribution_changed {
                weights_changed_edge_ids.push(edge_id);
            }
        }

        // ADR-003: Recompute raw weights via scale normalization
        if !committed_edge_ids.is_empty() {
            ctx.recompute_raw_weights();
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
        if !weights_changed_edge_ids.is_empty() {
            result.events.push(GraphEvent::WeightsChanged {
                edge_ids: weights_changed_edge_ids,
                adapter_id: adapter_id.clone(),
                context_id: context_id.clone(),
            });
        }

        Ok(result)
    }

    /// Map a PlexusError to an AdapterError.
    pub(crate) fn map_engine_error(e: crate::graph::PlexusError) -> AdapterError {
        match e {
            crate::graph::PlexusError::ContextNotFound(id) =>
                AdapterError::ContextNotFound(id.to_string()),
            other => AdapterError::Internal(other.to_string()),
        }
    }

    /// Run the enrichment loop after a primary emission (ADR-010).
    ///
    /// Per-round events: each round sees only events from the previous round.
    /// All enrichments in a round see the same context snapshot.
    /// The loop terminates when all enrichments return None (quiescence)
    /// or the safety valve (max rounds) is reached.
    ///
    /// Returns an EmitResult accumulating all enrichment rounds' mutations.
    pub(crate) fn run_enrichment_loop(
        engine: &PlexusEngine,
        context_id: &ContextId,
        registry: &EnrichmentRegistry,
        trigger_events: &[GraphEvent],
    ) -> Result<EmitResult, AdapterError> {
        let mut accumulated = EmitResult::empty();
        let mut round_events: Vec<GraphEvent> = trigger_events.to_vec();
        let mut round = 0;

        while round < registry.max_rounds() && !round_events.is_empty() {
            // Snapshot the context (clone for consistent, immutable view)
            let snapshot = engine.get_context(context_id)
                .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;

            // Run all enrichments with the same snapshot
            let mut round_emissions: Vec<(String, Emission)> = Vec::new();
            for enrichment in registry.enrichments() {
                if let Some(emission) = enrichment.enrich(&round_events, &snapshot) {
                    round_emissions.push((enrichment.id().to_string(), emission));
                }
            }

            // Quiescence: all enrichments returned None
            if round_emissions.is_empty() {
                break;
            }

            // Commit each enrichment's emission through the same path
            let mut new_events: Vec<GraphEvent> = Vec::new();
            for (enrichment_id, emission) in round_emissions {
                let enrichment_framework = Some(FrameworkContext {
                    adapter_id: enrichment_id,
                    context_id: context_id.to_string(),
                    input_summary: None,
                });

                let enrichment_result = engine.with_context_mut(context_id, |ctx| {
                    Self::emit_inner(ctx, emission, &enrichment_framework)
                }).map_err(Self::map_engine_error)??;

                new_events.extend(enrichment_result.events.clone());

                // Accumulate enrichment results
                accumulated.nodes_committed += enrichment_result.nodes_committed;
                accumulated.edges_committed += enrichment_result.edges_committed;
                accumulated.removals_committed += enrichment_result.removals_committed;
                accumulated.rejections.extend(enrichment_result.rejections);
                accumulated.provenance.extend(enrichment_result.provenance);
                accumulated.events.extend(enrichment_result.events);
            }

            round_events = new_events;
            round += 1;
        }

        if round >= registry.max_rounds() && !round_events.is_empty() {
            eprintln!(
                "warning: enrichment loop aborted after {} rounds (safety valve)",
                registry.max_rounds()
            );
        }

        Ok(accumulated)
    }
}

#[async_trait]
impl AdapterSink for EngineSink {
    async fn emit(&self, emission: Emission) -> Result<EmitResult, AdapterError> {
        match &self.backend {
            SinkBackend::Mutex(context) => {
                let mut ctx = context.lock().map_err(|e| {
                    AdapterError::Internal(format!("lock poisoned: {}", e))
                })?;
                let result = Self::emit_inner(&mut ctx, emission, &self.framework)?;
                self.accumulated_events.lock().unwrap()
                    .extend(result.events.clone());
                Ok(result)
            }
            SinkBackend::Engine { engine, context_id } => {
                let framework = self.framework.clone();
                let mut result = engine.with_context_mut(context_id, |ctx| {
                    Self::emit_inner(ctx, emission, &framework)
                }).map_err(Self::map_engine_error)??;

                // Run enrichment loop if enrichments are registered (ADR-010)
                if let Some(ref registry) = self.enrichments {
                    if !registry.enrichments().is_empty() {
                        let primary_events = result.events.clone();
                        let enrichment_result = Self::run_enrichment_loop(
                            engine,
                            context_id,
                            registry,
                            &primary_events,
                        )?;

                        // Merge enrichment results into primary result
                        result.nodes_committed += enrichment_result.nodes_committed;
                        result.edges_committed += enrichment_result.edges_committed;
                        result.removals_committed += enrichment_result.removals_committed;
                        result.rejections.extend(enrichment_result.rejections);
                        result.provenance.extend(enrichment_result.provenance);
                        result.events.extend(enrichment_result.events);
                    }
                }

                // Accumulate events for pipeline collection
                self.accumulated_events.lock().unwrap()
                    .extend(result.events.clone());

                Ok(result)
            }
        }
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

    // ================================================================
    // ADR-003 Reinforcement Mechanics Scenarios
    // ================================================================

    fn make_sink_with_adapter(adapter_id: &str) -> (EngineSink, Arc<Mutex<Context>>) {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let fw = FrameworkContext {
            adapter_id: adapter_id.to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::new(ctx.clone()).with_framework_context(fw);
        (sink, ctx)
    }

    // === Scenario: First emission creates contribution slot ===
    #[tokio::test]
    async fn first_emission_creates_contribution_slot() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let mut e = edge("A", "B");
        e.raw_weight = 5.0;

        let result = sink.emit(Emission::new().with_edge(e)).await.unwrap();
        assert_eq!(result.edges_committed, 1);

        let ctx = ctx.lock().unwrap();
        let edge = &ctx.edges[0];
        assert_eq!(edge.contributions.get("code-coverage"), Some(&5.0));
    }

    // === Scenario: Same adapter re-emits same value — idempotent ===
    #[tokio::test]
    async fn same_adapter_same_value_idempotent() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        // First emission
        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        sink.emit(Emission::new().with_edge(e1)).await.unwrap();

        // Re-emit same value
        let mut e2 = edge("A", "B");
        e2.raw_weight = 5.0;
        let result = sink.emit(Emission::new().with_edge(e2)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edge_count(), 1);
        assert_eq!(ctx.edges[0].contributions.get("code-coverage"), Some(&5.0));

        // No WeightsChanged event should fire
        let weights_changed = result.events.iter()
            .any(|e| matches!(e, GraphEvent::WeightsChanged { .. }));
        assert!(!weights_changed, "idempotent re-emission should not fire WeightsChanged");
    }

    // === Scenario: Same adapter emits higher value — contribution increases ===
    #[tokio::test]
    async fn same_adapter_higher_value_increases() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        sink.emit(Emission::new().with_edge(e1)).await.unwrap();

        let mut e2 = edge("A", "B");
        e2.raw_weight = 8.0;
        let result = sink.emit(Emission::new().with_edge(e2)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].contributions.get("code-coverage"), Some(&8.0));

        // WeightsChanged event should fire
        let weights_changed = result.events.iter()
            .any(|e| matches!(e, GraphEvent::WeightsChanged { .. }));
        assert!(weights_changed, "changed contribution should fire WeightsChanged");
    }

    // === Scenario: Same adapter emits lower value — contribution decreases ===
    #[tokio::test]
    async fn same_adapter_lower_value_decreases() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let mut e1 = edge("A", "B");
        e1.raw_weight = 8.0;
        sink.emit(Emission::new().with_edge(e1)).await.unwrap();

        let mut e2 = edge("A", "B");
        e2.raw_weight = 3.0;
        let result = sink.emit(Emission::new().with_edge(e2)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].contributions.get("code-coverage"), Some(&3.0));

        let weights_changed = result.events.iter()
            .any(|e| matches!(e, GraphEvent::WeightsChanged { .. }));
        assert!(weights_changed, "changed contribution should fire WeightsChanged");
    }

    // === Scenario: Different adapter emits same edge — cross-source reinforcement ===
    #[tokio::test]
    async fn different_adapter_cross_source_reinforcement() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        {
            let mut c = ctx.lock().unwrap();
            c.add_node(node("A"));
            c.add_node(node("B"));
        }

        // First adapter
        let fw1 = FrameworkContext {
            adapter_id: "code-coverage".to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let sink1 = EngineSink::new(ctx.clone()).with_framework_context(fw1);

        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        sink1.emit(Emission::new().with_edge(e1)).await.unwrap();

        // Second adapter
        let fw2 = FrameworkContext {
            adapter_id: "systems-architecture".to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let sink2 = EngineSink::new(ctx.clone()).with_framework_context(fw2);

        let mut e2 = edge("A", "B");
        e2.raw_weight = 0.7;
        let result = sink2.emit(Emission::new().with_edge(e2)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edge_count(), 1);
        let edge = &ctx.edges[0];
        assert_eq!(edge.contributions.get("code-coverage"), Some(&5.0));
        assert_eq!(edge.contributions.get("systems-architecture"), Some(&0.7));

        let weights_changed = result.events.iter()
            .any(|e| matches!(e, GraphEvent::WeightsChanged { .. }));
        assert!(weights_changed, "cross-source reinforcement should fire WeightsChanged");
    }

    // === Scenario: Re-processing with unchanged results is idempotent across all edges ===
    #[tokio::test]
    async fn reprocessing_unchanged_is_idempotent() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
        }

        // First processing
        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 3.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2)).await.unwrap();

        // Re-processing with same values
        let mut e1r = edge("A", "B");
        e1r.raw_weight = 5.0;
        let mut e2r = edge("A", "C");
        e2r.raw_weight = 3.0;
        let result = sink.emit(Emission::new().with_edge(e1r).with_edge(e2r)).await.unwrap();

        // No contribution changes
        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edges[0].contributions.get("code-coverage"), Some(&5.0));
        assert_eq!(ctx.edges[1].contributions.get("code-coverage"), Some(&3.0));

        // No WeightsChanged events
        let weights_changed = result.events.iter()
            .any(|e| matches!(e, GraphEvent::WeightsChanged { .. }));
        assert!(!weights_changed, "idempotent re-processing should not fire WeightsChanged");
    }

    // === Scenario: Two adapters contribute independently to the same edge ===
    #[tokio::test]
    async fn two_adapters_contribute_to_same_edge() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        {
            let mut c = ctx.lock().unwrap();
            c.add_node(node("sudden"));
            c.add_node(node("abrupt"));
        }

        // First adapter contributes
        let fw_first = FrameworkContext {
            adapter_id: "enrichment-adapter".to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let first_sink = EngineSink::new(ctx.clone()).with_framework_context(fw_first);

        let mut first_edge = Edge::new(
            NodeId::from_string("sudden"),
            NodeId::from_string("abrupt"),
            "may_be_related",
        );
        first_edge.raw_weight = 0.2;
        first_sink.emit(Emission::new().with_edge(first_edge)).await.unwrap();

        {
            let c = ctx.lock().unwrap();
            assert_eq!(c.edges[0].contributions.get("enrichment-adapter"), Some(&0.2));
        }

        // Second adapter confirms independently
        let fw_second = FrameworkContext {
            adapter_id: "document-adapter".to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let second_sink = EngineSink::new(ctx.clone()).with_framework_context(fw_second);

        let mut confirm_edge = Edge::new(
            NodeId::from_string("sudden"),
            NodeId::from_string("abrupt"),
            "may_be_related",
        );
        confirm_edge.raw_weight = 0.85;
        let result = second_sink.emit(Emission::new().with_edge(confirm_edge)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        let edge = &ctx.edges[0];
        assert_eq!(edge.contributions.get("enrichment-adapter"), Some(&0.2));
        assert_eq!(edge.contributions.get("document-adapter"), Some(&0.85));
        assert_eq!(edge.contributions.len(), 2);

        // Raw weight: each adapter has one edge (degenerate case → 1.0 each)
        // raw_weight = 1.0 + 1.0 = 2.0
        assert!((edge.raw_weight - 2.0).abs() < 1e-6,
            "raw weight should be sum of scale-normalized contributions: expected 2.0, got {}",
            edge.raw_weight);

        let weights_changed = result.events.iter()
            .any(|e| matches!(e, GraphEvent::WeightsChanged { .. }));
        assert!(weights_changed);
    }

    // ================================================================
    // ADR-003 Scale Normalization Scenarios
    // ================================================================

    // === Scenario: Single adapter, single edge — degenerate case normalizes to 1.0 ===
    #[tokio::test]
    async fn scale_norm_degenerate_case() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let mut e = edge("A", "B");
        e.raw_weight = 5.0;
        sink.emit(Emission::new().with_edge(e)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        let edge = &ctx.edges[0];
        // Single edge from single adapter: min=5, max=5, range=0 → normalize to 1.0
        assert!((edge.raw_weight - 1.0).abs() < 1e-6,
            "degenerate case should normalize to 1.0, got {}", edge.raw_weight);
    }

    // === Scenario: Single adapter, multiple edges — min→0.0, max→1.0 ===
    #[tokio::test]
    async fn scale_norm_min_max_mapping() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
            ctx.add_node(node("D"));
        }

        let mut e1 = edge("A", "B");
        e1.raw_weight = 2.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 10.0;
        let mut e3 = edge("A", "D");
        e3.raw_weight = 18.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2).with_edge(e3)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // code-coverage min=2, max=18, range=16, α=0.01
        // ADR-005: (value - min + α·range) / ((1+α)·range)
        // A→B: (0 + 0.16) / 16.16 ≈ 0.00990 (floor, not 0.0)
        // A→C: (8 + 0.16) / 16.16 ≈ 0.50495
        // A→D: (16 + 0.16) / 16.16 = 1.0
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
        let ad = ctx.edges.iter().find(|e| e.target == NodeId::from_string("D")).unwrap();

        let floor = 0.01 / 1.01; // α/(1+α) ≈ 0.00990
        assert!((ab.raw_weight - floor).abs() < 1e-4, "A→B should be ~{:.4} (floor), got {}", floor, ab.raw_weight);
        assert!(ab.raw_weight > 0.0, "A→B should be non-zero (ADR-005 normalization floor)");
        assert!((ac.raw_weight - 0.505).abs() < 1e-2, "A→C should be ~0.505, got {}", ac.raw_weight);
        assert!((ad.raw_weight - 1.0).abs() < 1e-6, "A→D should be 1.0, got {}", ad.raw_weight);
    }

    // === Scenario: Two adapters on different scales — normalization prevents dominance ===
    #[tokio::test]
    async fn scale_norm_prevents_dominance() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        {
            let mut c = ctx.lock().unwrap();
            c.add_node(node("A"));
            c.add_node(node("B"));
            c.add_node(node("C"));
            c.add_node(node("D"));
        }

        // code-coverage: A→B=2, A→C=18, A→D=14
        let fw_cc = FrameworkContext {
            adapter_id: "code-coverage".to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let sink_cc = EngineSink::new(ctx.clone()).with_framework_context(fw_cc);

        let mut e1 = edge("A", "B");
        e1.raw_weight = 2.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 18.0;
        let mut e3 = edge("A", "D");
        e3.raw_weight = 14.0;
        sink_cc.emit(Emission::new().with_edge(e1).with_edge(e2).with_edge(e3)).await.unwrap();

        // movement: A→B=400, A→C=100, A→D=350
        let fw_mv = FrameworkContext {
            adapter_id: "movement".to_string(),
            context_id: "test-context".to_string(),
            input_summary: None,
        };
        let sink_mv = EngineSink::new(ctx.clone()).with_framework_context(fw_mv);

        let mut e4 = edge("A", "B");
        e4.raw_weight = 400.0;
        let mut e5 = edge("A", "C");
        e5.raw_weight = 100.0;
        let mut e6 = edge("A", "D");
        e6.raw_weight = 350.0;
        sink_mv.emit(Emission::new().with_edge(e4).with_edge(e5).with_edge(e6)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
        let ad = ctx.edges.iter().find(|e| e.target == NodeId::from_string("D")).unwrap();

        // ADR-005: with α=0.01 floor, minimums are non-zero
        // code-coverage (min=2, max=18, range=16): B≈0.0099, C=1.0, D≈0.7525
        // movement (min=100, max=400, range=300): B=1.0, C≈0.0099, D≈0.835
        // A→D still highest (both adapters contribute strongly)
        assert!(ad.raw_weight > ab.raw_weight,
            "A→D ({}) should rank higher than A→B ({})", ad.raw_weight, ab.raw_weight);
        assert!(ad.raw_weight > ac.raw_weight,
            "A→D ({}) should rank higher than A→C ({})", ad.raw_weight, ac.raw_weight);

        // A→D: cc=(12+0.16)/16.16 + mv=(250+3)/303
        let expected_cc = (14.0 - 2.0 + 0.01 * 16.0) / (1.01 * 16.0);
        let expected_mv = (350.0 - 100.0 + 0.01 * 300.0) / (1.01 * 300.0);
        let expected_ad = expected_cc + expected_mv;
        assert!((ad.raw_weight - expected_ad).abs() < 1e-3,
            "A→D should be ~{:.3}, got {}", expected_ad, ad.raw_weight);
    }

    // === Scenario: Signed adapter range normalizes correctly ===
    #[tokio::test]
    async fn scale_norm_signed_range() {
        let (sink, ctx) = make_sink_with_adapter("sentiment");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
            ctx.add_node(node("D"));
        }

        let mut e1 = edge("A", "B");
        e1.raw_weight = -0.8;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 0.5;
        let mut e3 = edge("A", "D");
        e3.raw_weight = 1.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2).with_edge(e3)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // sentiment min=-0.8, max=1.0, range=1.8, α=0.01
        // ADR-005: (value - min + α·range) / ((1+α)·range)
        // A→B: (0 + 0.018) / 1.818 ≈ 0.00990 (floor)
        // A→C: (1.3 + 0.018) / 1.818 ≈ 0.72497
        // A→D: (1.8 + 0.018) / 1.818 = 1.0
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
        let ad = ctx.edges.iter().find(|e| e.target == NodeId::from_string("D")).unwrap();

        let floor = 0.01 / 1.01;
        assert!((ab.raw_weight - floor).abs() < 1e-4, "A→B should be ~{:.4} (floor), got {}", floor, ab.raw_weight);
        assert!(ab.raw_weight > 0.0, "A→B should be non-zero (ADR-005)");
        assert!((ac.raw_weight - 0.725).abs() < 1e-2, "A→C should be ~0.725, got {}", ac.raw_weight);
        assert!((ad.raw_weight - 1.0).abs() < 1e-6, "A→D should be 1.0, got {}", ad.raw_weight);
    }

    // === Scenario: New emission extending range shifts all that adapter's values ===
    #[tokio::test]
    async fn scale_norm_range_extension_shifts() {
        let (sink, ctx) = make_sink_with_adapter("code-coverage");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
            ctx.add_node(node("D"));
        }

        // First emission: A→B=5, A→C=15
        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 15.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2)).await.unwrap();

        {
            let c = ctx.lock().unwrap();
            let ac = c.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
            assert!((ac.raw_weight - 1.0).abs() < 1e-6, "A→C should be 1.0 before range extension");
        }

        // New emission extends range: A→D=25
        let mut e3 = edge("A", "D");
        e3.raw_weight = 25.0;
        sink.emit(Emission::new().with_edge(e3)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // code-coverage min=5, max=25, range=20, α=0.01
        // ADR-005: (value - min + α·range) / ((1+α)·range)
        // A→B: (0 + 0.2) / 20.2 ≈ 0.00990 (floor)
        // A→C: (10 + 0.2) / 20.2 ≈ 0.50495 (was 1.0 — shifted, with floor)
        // A→D: (20 + 0.2) / 20.2 = 1.0
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
        let ad = ctx.edges.iter().find(|e| e.target == NodeId::from_string("D")).unwrap();

        let floor = 0.01 / 1.01;
        assert!((ab.raw_weight - floor).abs() < 1e-4, "A→B should be ~{:.4} (floor), got {}", floor, ab.raw_weight);
        assert!(ab.raw_weight > 0.0, "A→B should be non-zero (ADR-005)");
        assert!((ac.raw_weight - 0.505).abs() < 1e-2, "A→C should be ~0.505 (shifted, with floor), got {}", ac.raw_weight);
        assert!((ad.raw_weight - 1.0).abs() < 1e-6, "A→D should be 1.0, got {}", ad.raw_weight);
    }

    // ================================================================
    // Normalization Floor (ADR-005) — Scenario Group 4
    // ================================================================

    // === Scenario: Minimum contribution maps to floor, not zero ===
    #[tokio::test]
    async fn norm_floor_min_maps_to_floor() {
        let (sink, ctx) = make_sink_with_adapter("co-occurrence");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
        }

        // co-occurrence contributions: A→B = 0.5, A→C = 1.0
        let mut e1 = edge("A", "B");
        e1.raw_weight = 0.5;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 1.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // min=0.5, max=1.0, range=0.5, α=0.01
        // ε = 0.01 × 0.5 = 0.005
        // A→B: (0.5 - 0.5 + 0.005) / (0.5 + 0.005) = 0.005 / 0.505 ≈ 0.00990
        // A→C: (1.0 - 0.5 + 0.005) / 0.505 = 0.505 / 0.505 = 1.0
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();

        let expected_floor = 0.01 / 1.01;
        assert!((ab.raw_weight - expected_floor).abs() < 1e-4,
            "A→B should be ~{:.4}, got {}", expected_floor, ab.raw_weight);
        assert!(ab.raw_weight > 0.0, "A→B raw weight must be non-zero (ADR-005)");
        assert!((ac.raw_weight - 1.0).abs() < 1e-6, "A→C should be 1.0, got {}", ac.raw_weight);
    }

    // === Scenario: Floor is proportionally equal across adapters with different ranges ===
    #[tokio::test]
    async fn norm_floor_proportionally_equal() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        {
            let mut c = ctx.lock().unwrap();
            c.add_node(node("A"));
            c.add_node(node("B"));
            c.add_node(node("C"));
            c.add_node(node("D"));
        }

        // co-occurrence: range 0.5 (min=0.5, max=1.0)
        let fw_co = FrameworkContext {
            adapter_id: "co-occurrence".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        };
        let sink_co = EngineSink::new(ctx.clone()).with_framework_context(fw_co);
        let mut e1 = edge("A", "B");
        e1.raw_weight = 0.5; // min
        let mut e2 = edge("A", "C");
        e2.raw_weight = 1.0; // max
        sink_co.emit(Emission::new().with_edge(e1).with_edge(e2)).await.unwrap();

        // code-coverage: range 99 (min=1, max=100)
        let fw_cc = FrameworkContext {
            adapter_id: "code-coverage".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        };
        let sink_cc = EngineSink::new(ctx.clone()).with_framework_context(fw_cc);
        let mut e3 = edge("A", "C");
        e3.raw_weight = 1.0; // min for code-coverage
        let mut e4 = edge("A", "D");
        e4.raw_weight = 100.0; // max for code-coverage
        sink_cc.emit(Emission::new().with_edge(e3).with_edge(e4)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // A→B has co-occurrence min (0.5): floor = α/(1+α) ≈ 0.00990
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let co_floor = ab.raw_weight; // Only co-occurrence contributes to A→B

        // A→D has code-coverage min (contribution is max=100, but A→C has code-coverage min=1)
        // Actually we need a separate edge at code-coverage min
        // A→C has: co-occurrence max (1.0) + code-coverage min (1.0)
        // The code-coverage contribution on A→C is at code-coverage min
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
        // ac has two contributions: co-occurrence=1.0 (maps to 1.0) and code-coverage=1.0 (maps to floor)
        let cc_floor = ac.raw_weight - 1.0; // subtract co-occurrence's normalized contribution (1.0)

        // Both floors should be approximately equal: α/(1+α) ≈ 0.00990
        let expected_floor = 0.01 / 1.01;
        assert!((co_floor - expected_floor).abs() < 1e-4,
            "co-occurrence floor should be ~{:.4}, got {}", expected_floor, co_floor);
        assert!((cc_floor - expected_floor).abs() < 1e-4,
            "code-coverage floor should be ~{:.4}, got {}", expected_floor, cc_floor);
    }

    // === Scenario: Degenerate case unchanged — single value normalizes to 1.0 ===
    // (This is already covered by scale_norm_degenerate_case, but we verify
    //  that ADR-005 doesn't change the behavior when range = 0)
    #[tokio::test]
    async fn norm_floor_degenerate_unchanged() {
        let (sink, ctx) = make_sink_with_adapter("co-occurrence");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
        }

        let mut e1 = edge("A", "B");
        e1.raw_weight = 0.7;
        sink.emit(Emission::new().with_edge(e1)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // Single value: min=0.7, max=0.7, range=0.0 → degenerate → 1.0
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        assert!((ab.raw_weight - 1.0).abs() < 1e-6, "degenerate case should be 1.0, got {}", ab.raw_weight);
    }

    // === Scenario: Normalization floor preserves relative ordering ===
    #[tokio::test]
    async fn norm_floor_preserves_ordering() {
        let (sink, ctx) = make_sink_with_adapter("co-occurrence");

        {
            let mut ctx = ctx.lock().unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
            ctx.add_node(node("D"));
        }

        let mut e1 = edge("A", "B");
        e1.raw_weight = 1.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 3.0;
        let mut e3 = edge("A", "D");
        e3.raw_weight = 5.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2).with_edge(e3)).await.unwrap();

        let ctx = ctx.lock().unwrap();
        let ab = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap();
        let ac = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap();
        let ad = ctx.edges.iter().find(|e| e.target == NodeId::from_string("D")).unwrap();

        // Ordering preserved: A→B < A→C < A→D
        assert!(ab.raw_weight < ac.raw_weight, "A→B ({}) < A→C ({})", ab.raw_weight, ac.raw_weight);
        assert!(ac.raw_weight < ad.raw_weight, "A→C ({}) < A→D ({})", ac.raw_weight, ad.raw_weight);
        // Maximum = 1.0
        assert!((ad.raw_weight - 1.0).abs() < 1e-6, "A→D should be 1.0, got {}", ad.raw_weight);
        // Minimum > 0.0
        assert!(ab.raw_weight > 0.0, "A→B should be > 0.0 (ADR-005), got {}", ab.raw_weight);
    }
}
