//! Enrichment loop: runs enrichments after a primary emission (ADR-010).
//!
//! Extracted from engine_sink.rs to resolve the logical dependency:
//! the loop imports from both `engine_sink` (for `emit_inner`) and
//! `enrichment` (for `EnrichmentRegistry`).

use crate::adapter::sink::{EngineSink, FrameworkContext, AdapterError, EmitResult};
use super::traits::EnrichmentRegistry;
use crate::graph::events::GraphEvent;
use crate::graph::{ContextId, PlexusEngine};

/// Enrichment loop telemetry: result plus convergence metadata.
pub(crate) struct EnrichmentLoopResult {
    pub result: EmitResult,
    /// Number of enrichment rounds executed.
    pub rounds: usize,
    /// True if terminated by quiescence, false if safety valve.
    pub quiesced: bool,
}

/// Run the enrichment loop after a primary emission (ADR-010).
///
/// Per-round events: each round sees only events from the previous round.
/// All enrichments in a round see the same context snapshot.
/// The loop terminates when all enrichments return None (quiescence)
/// or the safety valve (max rounds) is reached.
///
/// Returns an EnrichmentLoopResult with the accumulated EmitResult
/// plus convergence telemetry (rounds, quiesced).
pub(crate) fn run_enrichment_loop(
    engine: &PlexusEngine,
    context_id: &ContextId,
    registry: &EnrichmentRegistry,
    trigger_events: &[GraphEvent],
) -> Result<EnrichmentLoopResult, AdapterError> {
    let mut accumulated = EmitResult::empty();
    let mut round_events: Vec<GraphEvent> = trigger_events.to_vec();
    let mut round = 0;
    let mut quiesced = false;

    while round < registry.max_rounds() && !round_events.is_empty() {
        // Snapshot the context (clone for consistent, immutable view)
        let snapshot = engine.get_context(context_id)
            .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;

        // Run all enrichments with the same snapshot
        let mut round_emissions: Vec<(String, crate::adapter::types::Emission)> = Vec::new();
        for enrichment in registry.enrichments() {
            if let Some(emission) = enrichment.enrich(&round_events, &snapshot) {
                round_emissions.push((enrichment.id().to_string(), emission));
            }
        }

        // Quiescence: all enrichments returned None
        if round_emissions.is_empty() {
            quiesced = true;
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
                EngineSink::emit_inner(ctx, emission, &enrichment_framework)
            }).map_err(EngineSink::map_engine_error)??;

            // Persist enrichment events to event log (ADR-035)
            engine.persist_events(&enrichment_result.events);

            new_events.extend(enrichment_result.events.clone());

            // Accumulate enrichment results
            accumulated.nodes_committed += enrichment_result.nodes_committed;
            accumulated.edges_committed += enrichment_result.edges_committed;
            accumulated.removals_committed += enrichment_result.removals_committed;
            accumulated.edge_removals_committed += enrichment_result.edge_removals_committed;
            accumulated.rejections.extend(enrichment_result.rejections);
            accumulated.provenance.extend(enrichment_result.provenance);
            accumulated.events.extend(enrichment_result.events);
        }

        round_events = new_events;
        round += 1;

        // Also quiesced if no new events were produced
        if round_events.is_empty() {
            quiesced = true;
        }
    }

    if !quiesced {
        tracing::warn!(
            rounds = registry.max_rounds(),
            "enrichment loop aborted (safety valve)"
        );
    }

    Ok(EnrichmentLoopResult {
        result: accumulated,
        rounds: round,
        quiesced,
    })
}

#[cfg(test)]
mod tests {
    //! Enrichment loop scenarios (ADR-010), relocated from
    //! adapter/integration_tests.rs. Mock enrichments exercise the loop's
    //! round semantics, quiescence, safety valve, and registry dedup.

    use super::*;
    use super::super::traits::Enrichment;
    use crate::adapter::sink::AdapterSink;
    use crate::adapter::types::Emission;
    use crate::graph::{ContentType, Context, Edge, Node, NodeId};
    use std::sync::{Arc, Mutex};

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    /// Test enrichment that records its inputs and returns None.
    struct RecordingEnrichment {
        id: String,
        calls: Mutex<Vec<(Vec<GraphEvent>, Context)>>,
    }

    impl RecordingEnrichment {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }

        fn last_call(&self) -> Option<(Vec<GraphEvent>, Context)> {
            self.calls.lock().unwrap().last().cloned()
        }
    }

    impl Enrichment for RecordingEnrichment {
        fn id(&self) -> &str {
            &self.id
        }
        fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
            self.calls
                .lock()
                .unwrap()
                .push((events.to_vec(), context.clone()));
            None
        }
    }

    /// Test enrichment that emits a may_be_related edge once, then returns None.
    struct OneShotEdgeEnrichment {
        id: String,
        source: NodeId,
        target: NodeId,
        fired: Mutex<bool>,
    }

    impl OneShotEdgeEnrichment {
        fn new(id: &str, source: &str, target: &str) -> Self {
            Self {
                id: id.to_string(),
                source: NodeId::from_string(source),
                target: NodeId::from_string(target),
                fired: Mutex::new(false),
            }
        }
    }

    impl Enrichment for OneShotEdgeEnrichment {
        fn id(&self) -> &str {
            &self.id
        }
        fn enrich(&self, _events: &[GraphEvent], _context: &Context) -> Option<Emission> {
            let mut fired = self.fired.lock().unwrap();
            if *fired {
                return None;
            }
            *fired = true;
            let mut edge = Edge::new_in_dimension(
                self.source.clone(),
                self.target.clone(),
                "may_be_related",
                "semantic",
            );
            // emit_inner uses raw_weight as the contribution value
            edge.combined_weight = 0.75;
            Some(Emission::new().with_edge(edge))
        }
    }

    /// Enrichment that emits a node on the first call (round 0), then returns None.
    struct RoundZeroNodeEnrichment {
        id: String,
        node_id: String,
        fired: Mutex<bool>,
    }

    impl RoundZeroNodeEnrichment {
        fn new(id: &str, node_id: &str) -> Self {
            Self {
                id: id.to_string(),
                node_id: node_id.to_string(),
                fired: Mutex::new(false),
            }
        }
    }

    impl Enrichment for RoundZeroNodeEnrichment {
        fn id(&self) -> &str {
            &self.id
        }
        fn enrich(&self, _events: &[GraphEvent], _context: &Context) -> Option<Emission> {
            let mut fired = self.fired.lock().unwrap();
            if *fired {
                return None;
            }
            *fired = true;
            Some(Emission::new().with_node(node(&self.node_id)))
        }
    }

    /// Enrichment that waits for a specific node to exist, then emits an edge.
    struct WaitForNodeEnrichment {
        id: String,
        wait_for: String,
        source: String,
        target: String,
        fired: Mutex<bool>,
    }

    impl WaitForNodeEnrichment {
        fn new(id: &str, wait_for: &str, source: &str, target: &str) -> Self {
            Self {
                id: id.to_string(),
                wait_for: wait_for.to_string(),
                source: source.to_string(),
                target: target.to_string(),
                fired: Mutex::new(false),
            }
        }
    }

    impl Enrichment for WaitForNodeEnrichment {
        fn id(&self) -> &str {
            &self.id
        }
        fn enrich(&self, events: &[GraphEvent], _context: &Context) -> Option<Emission> {
            let mut fired = self.fired.lock().unwrap();
            if *fired {
                return None;
            }
            // Only fire when we see the node we're waiting for
            let node_appeared = events.iter().any(|e| {
                if let GraphEvent::NodesAdded { node_ids, .. } = e {
                    node_ids.iter().any(|id| id.to_string() == self.wait_for)
                } else {
                    false
                }
            });
            if !node_appeared {
                return None;
            }
            *fired = true;
            let edge = Edge::new(
                NodeId::from_string(&self.source),
                NodeId::from_string(&self.target),
                "depends_on",
            )
            .with_contribution(&self.id, 1.0);
            Some(Emission::new().with_edge(edge))
        }
    }

    // === Scenario: Enrichment receives events and context snapshot after primary emission ===
    #[tokio::test]
    async fn enrichment_receives_events_and_snapshot_after_primary_emission() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let enrichment = Arc::new(RecordingEnrichment::new("test-enrichment"));
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment.clone() as Arc<dyn Enrichment>,
        ]));

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        // Emit a node
        let result = sink
            .emit(Emission::new().with_node(node("concept:travel")))
            .await
            .unwrap();
        assert_eq!(result.nodes_committed, 1, "primary emission commits one node");

        // Run enrichment loop with primary events
        let _enrichment_result = run_enrichment_loop(
            &engine, &ctx_id, &registry, &result.events,
        ).unwrap();

        // Enrichment was called exactly once (one round, then quiescent)
        assert_eq!(enrichment.call_count(), 1, "enrichment called once before quiescence");

        // It received the NodesAdded event
        let (events, snapshot) = enrichment.last_call().unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, GraphEvent::NodesAdded { .. })), "enrichment received NodesAdded event");

        // The snapshot contains the newly added node
        assert!(snapshot
            .get_node(&NodeId::from_string("concept:travel"))
            .is_some(), "snapshot contains the newly added node");
    }

    // === Scenario: Enrichment returning Some(Emission) causes mutations to be committed ===
    #[tokio::test]
    async fn enrichment_emission_causes_mutations_to_be_committed() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");

        // Pre-populate with two concept nodes
        let mut ctx = Context::with_id(ctx_id.clone(), "provence-research");
        ctx.add_node(node("concept:travel"));
        ctx.add_node(node("concept:avignon"));
        engine.upsert_context(ctx).unwrap();

        let enrichment = Arc::new(OneShotEdgeEnrichment::new(
            "co-occurrence",
            "concept:travel",
            "concept:avignon",
        ));
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment.clone() as Arc<dyn Enrichment>,
        ]));

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        // Emit a no-op node to trigger the enrichment loop
        let primary_result = sink
            .emit(Emission::new().with_node(node("trigger-node")))
            .await
            .unwrap();

        // Run enrichment loop with primary events
        let enrichment_result = run_enrichment_loop(
            &engine, &ctx_id, &registry, &primary_result.events,
        ).unwrap();

        // Primary emission: 1 node. Enrichment: 1 edge.
        assert!(primary_result.nodes_committed >= 1, "primary emission commits at least one node");
        assert_eq!(enrichment_result.result.edges_committed, 1, "enrichment commits one edge");

        // The may_be_related edge exists in the context
        let ctx = engine.get_context(&ctx_id).unwrap();
        let edge = ctx
            .edges
            .iter()
            .find(|e| e.relationship == "may_be_related")
            .expect("may_be_related edge should exist");

        assert_eq!(edge.source, NodeId::from_string("concept:travel"), "may_be_related edge source is concept:travel");
        assert_eq!(edge.target, NodeId::from_string("concept:avignon"), "may_be_related edge target is concept:avignon");

        // The edge has a contribution from the enrichment's id with the emitted value
        assert_eq!(edge.contributions.get("co-occurrence"), Some(&0.75), "enrichment contribution value is 0.75");
    }

    // === Scenario: Enrichment returning None means quiescent ===
    #[tokio::test]
    async fn enrichment_returning_none_means_quiescent() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Enrichment that always returns None
        let enrichment = Arc::new(RecordingEnrichment::new("quiet-enrichment"));
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment.clone() as Arc<dyn Enrichment>,
        ]));

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        let result = sink
            .emit(Emission::new().with_node(node("A")))
            .await
            .unwrap();

        // Run enrichment loop with primary events
        let _enrichment_result = run_enrichment_loop(
            &engine, &ctx_id, &registry, &result.events,
        ).unwrap();

        // Loop completed in one round (enrichment called once, returned None)
        assert_eq!(enrichment.call_count(), 1, "enrichment called once before quiescence");

        // No additional mutations beyond the primary emission
        assert_eq!(result.nodes_committed, 1, "primary emission commits one node");
        assert_eq!(result.edges_committed, 0, "no edges committed when enrichment returns none");
    }

    // === Scenario: Loop runs multiple rounds until quiescence ===
    #[tokio::test]
    async fn loop_runs_multiple_rounds_until_quiescence() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        let mut ctx = Context::with_id(ctx_id.clone(), "provence-research");
        ctx.add_node(node("existing-node"));
        engine.upsert_context(ctx).unwrap();

        // Enrichment A: emits a node in round 0
        let enrichment_a = Arc::new(RoundZeroNodeEnrichment::new("enrichment-a", "new-node"));

        // Enrichment B: waits for "new-node" to appear, then emits an edge
        let enrichment_b = Arc::new(WaitForNodeEnrichment::new(
            "enrichment-b",
            "new-node",
            "existing-node",
            "new-node",
        ));

        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment_a.clone() as Arc<dyn Enrichment>,
            enrichment_b.clone() as Arc<dyn Enrichment>,
        ]));

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        // Primary emission triggers the loop
        let primary_result = sink
            .emit(Emission::new().with_node(node("trigger")))
            .await
            .unwrap();

        // Run enrichment loop with primary events
        let enrichment_result = run_enrichment_loop(
            &engine, &ctx_id, &registry, &primary_result.events,
        ).unwrap();

        // Primary: 1 node (trigger). Round 0: enrichment A adds new-node.
        // Round 1: enrichment B sees new-node event, emits edge.
        // Round 2: both return None → quiescence.
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("new-node")).is_some(), "enrichment A created new-node");
        assert!(ctx
            .edges
            .iter()
            .any(|e| e.source == NodeId::from_string("existing-node")
                && e.target == NodeId::from_string("new-node")
                && e.relationship == "depends_on"), "enrichment B created depends_on edge after new-node appeared");

        // Total: trigger (primary) = 1 node; enrichment: 1 node + 1 edge
        assert_eq!(primary_result.nodes_committed, 1, "primary emission commits one node");
        assert_eq!(enrichment_result.result.nodes_committed, 1, "enrichment commits one node across rounds");
        assert_eq!(enrichment_result.result.edges_committed, 1, "enrichment commits one edge across rounds");
    }

    // === Scenario: Per-round events — enrichment sees only previous round's events ===
    #[tokio::test]
    async fn per_round_events_enrichment_sees_only_previous_round() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Recording enrichment that also emits a node on round 0
        struct RecordAndEmitEnrichment {
            id: String,
            calls: Mutex<Vec<Vec<GraphEvent>>>,
        }

        impl RecordAndEmitEnrichment {
            fn new(id: &str) -> Self {
                Self {
                    id: id.to_string(),
                    calls: Mutex::new(Vec::new()),
                }
            }
        }

        impl Enrichment for RecordAndEmitEnrichment {
            fn id(&self) -> &str {
                &self.id
            }
            fn enrich(&self, events: &[GraphEvent], _context: &Context) -> Option<Emission> {
                let mut calls = self.calls.lock().unwrap();
                calls.push(events.to_vec());
                if calls.len() == 1 {
                    // Round 0: emit a node (will produce events for round 1)
                    Some(Emission::new().with_node(node("enrichment-node")))
                } else {
                    None
                }
            }
        }

        let enrichment = Arc::new(RecordAndEmitEnrichment::new("recorder"));
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment.clone() as Arc<dyn Enrichment>,
        ]));

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        let primary_result = sink.emit(Emission::new().with_node(node("primary-node")))
            .await
            .unwrap();

        // Run enrichment loop with primary events
        run_enrichment_loop(&engine, &ctx_id, &registry, &primary_result.events).unwrap();

        let calls = enrichment.calls.lock().unwrap();
        // Round 0 saw primary events (NodesAdded for primary-node)
        assert!(calls[0]
            .iter()
            .any(|e| matches!(e, GraphEvent::NodesAdded { .. })), "round 0 received NodesAdded event");

        // Round 1 saw enrichment events (NodesAdded for enrichment-node), NOT primary events
        assert_eq!(calls.len(), 2, "enrichment called exactly 2 times (round 0 + round 1)");
        if let GraphEvent::NodesAdded { node_ids, adapter_id, .. } = &calls[1][0] {
            // Round 1 events come from the enrichment, not the primary adapter
            assert_eq!(adapter_id, "recorder", "round 1 events come from the enrichment");
            assert!(node_ids.iter().any(|id| id.to_string() == "enrichment-node"), "round 1 contains enrichment-node");
        } else {
            panic!("expected NodesAdded in round 1");
        }
    }

    // === Scenario: Safety valve aborts after maximum rounds ===
    #[tokio::test]
    async fn safety_valve_aborts_after_maximum_rounds() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Enrichment that never quiesces: always emits a new node
        struct InfiniteEnrichment {
            counter: Mutex<usize>,
        }

        impl InfiniteEnrichment {
            fn new() -> Self {
                Self {
                    counter: Mutex::new(0),
                }
            }
        }

        impl Enrichment for InfiniteEnrichment {
            fn id(&self) -> &str {
                "infinite"
            }
            fn enrich(&self, _events: &[GraphEvent], _context: &Context) -> Option<Emission> {
                let mut c = self.counter.lock().unwrap();
                *c += 1;
                Some(Emission::new().with_node(node(&format!("node-{}", c))))
            }
        }

        let enrichment = Arc::new(InfiniteEnrichment::new());
        let registry = Arc::new(
            EnrichmentRegistry::new(vec![enrichment.clone() as Arc<dyn Enrichment>])
                .with_max_rounds(3),
        );

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        let primary_result = sink
            .emit(Emission::new().with_node(node("trigger")))
            .await
            .unwrap();

        // Run enrichment loop with primary events
        let enrichment_result = run_enrichment_loop(
            &engine, &ctx_id, &registry, &primary_result.events,
        ).unwrap();

        // Safety valve at 3 rounds: primary (1 node) + 3 enrichment rounds (3 nodes) = 4
        assert_eq!(primary_result.nodes_committed, 1, "primary emission commits one node");
        assert_eq!(enrichment_result.result.nodes_committed, 3, "enrichment commits 3 nodes across 3 capped rounds");

        // Safety valve: loop did NOT quiesce, stopped by max_rounds
        assert!(!enrichment_result.quiesced, "loop should not reach quiescence");
        assert_eq!(enrichment_result.rounds, 3, "loop should run exactly max_rounds");

        // The enrichment was called exactly 3 times (the max rounds)
        assert_eq!(*enrichment.counter.lock().unwrap(), 3, "enrichment called exactly max_rounds times");
    }

    // === Scenario: Enrichments shared across integrations are deduplicated ===
    #[tokio::test]
    async fn enrichments_deduplicated_by_id() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let enrichment_a = Arc::new(RecordingEnrichment::new("tag-bridger"));
        let enrichment_b = Arc::new(RecordingEnrichment::new("tag-bridger"));

        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment_a.clone() as Arc<dyn Enrichment>,
            enrichment_b.clone() as Arc<dyn Enrichment>,
        ]));

        let framework = FrameworkContext {
            adapter_id: "test-adapter".to_string(),
            context_id: "provence-research".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(framework);

        let primary_result = sink.emit(Emission::new().with_node(node("A")))
            .await
            .unwrap();

        // Run enrichment loop with primary events
        run_enrichment_loop(&engine, &ctx_id, &registry, &primary_result.events).unwrap();

        // Only the first enrichment instance was called (dedup by id)
        assert_eq!(enrichment_a.call_count(), 1, "first enrichment instance called once");
        assert_eq!(enrichment_b.call_count(), 0, "duplicate enrichment id skipped");
    }
}
