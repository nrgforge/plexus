//! Integration tests for cancellation, progressive emission, and end-to-end adapter scenarios

#[cfg(test)]
mod tests {
    use crate::adapter::cancel::CancellationToken;
    use crate::adapter::cooccurrence::CoOccurrenceEnrichment;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::enrichment::{Enrichment, EnrichmentRegistry};
    use crate::adapter::events::GraphEvent;
    use crate::adapter::fragment::{FragmentAdapter, FragmentInput};
    use crate::adapter::provenance::FrameworkContext;
    use crate::adapter::sink::{AdapterError, AdapterSink};
    use crate::adapter::traits::{Adapter, AdapterInput};
    use crate::adapter::types::{Emission, OutboundEvent};
    use crate::graph::{ContentType, Context, ContextId, Edge, Node, NodeId, PlexusEngine};
    use crate::storage::{OpenStore, SqliteStore};
    use std::sync::{Arc, Mutex};

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

    // ================================================================
    // Cancellation Scenarios
    // ================================================================

    // === Scenario: Adapter checks cancellation between emissions ===
    #[tokio::test]
    async fn adapter_checks_cancellation_between_emissions() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // E1: committed successfully
        let e1 = Emission::new().with_node(node("A"));
        let r1 = sink.emit(e1).await.unwrap();
        assert_eq!(r1.nodes_committed, 1);

        // Framework signals cancellation
        token.cancel();

        // Adapter checks token before next emission
        assert!(token.is_cancelled());

        // Adapter stops — no further emissions
        // E1 remains committed
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
    }

    // === Scenario: Committed emissions survive cancellation ===
    #[tokio::test]
    async fn committed_emissions_survive_cancellation() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // E1 and E2 committed
        sink.emit(Emission::new().with_node(node("A"))).await.unwrap();
        sink.emit(Emission::new().with_node(node("B"))).await.unwrap();

        // Cancel before E3
        token.cancel();
        assert!(token.is_cancelled());

        // E3 never emitted — adapter checks token and stops
        // E1 and E2 remain
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("B")).is_some());
        assert_eq!(ctx.node_count(), 2);
    }

    // === Scenario: Cancellation during emission has no effect until next check ===
    #[tokio::test]
    async fn cancellation_during_emission_no_effect_until_check() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // Cancel while E2 is "being constructed"
        // (in practice, cancellation is checked between emit calls, not during)
        token.cancel();

        // Adapter may still emit E2 if it hasn't checked the token yet
        let r2 = sink.emit(Emission::new().with_node(node("X"))).await.unwrap();
        assert_eq!(r2.nodes_committed, 1); // committed, because emit() doesn't check token

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("X")).is_some());
    }

    // ================================================================
    // Progressive Emission Scenarios
    // ================================================================

    // === Scenario: Multiple emissions from one adapter, each commits independently ===
    #[tokio::test]
    async fn multiple_emissions_commit_independently() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        // E1: structural nodes
        let e1 = Emission::new()
            .with_node(node("file"))
            .with_node(node("section-1"))
            .with_node(node("section-2"));
        let r1 = sink.emit(e1).await.unwrap();
        assert_eq!(r1.nodes_committed, 3);

        // After E1: structural nodes exist
        {
            let ctx = ctx.lock().unwrap();
            assert!(ctx.get_node(&NodeId::from_string("file")).is_some());
            assert!(ctx.get_node(&NodeId::from_string("section-1")).is_some());
        }

        // E2: semantic nodes + edges
        let e2 = Emission::new()
            .with_node(node("concept-sudden"))
            .with_edge(edge("section-1", "concept-sudden"));
        let r2 = sink.emit(e2).await.unwrap();
        assert_eq!(r2.nodes_committed, 1);
        assert_eq!(r2.edges_committed, 1);

        // After E2: both structural and semantic exist
        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.node_count(), 4);
        assert_eq!(ctx.edge_count(), 1);
    }

    // === Scenario: Graph events fire per emission ===
    #[tokio::test]
    async fn graph_events_fire_per_emission() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        // E1: 3 nodes
        let e1 = Emission::new()
            .with_node(node("A"))
            .with_node(node("B"))
            .with_node(node("C"));
        let r1 = sink.emit(e1).await.unwrap();

        let nodes_event = r1.events.iter().find(|e| matches!(e, GraphEvent::NodesAdded { .. }));
        assert!(nodes_event.is_some());
        if let Some(GraphEvent::NodesAdded { node_ids, .. }) = nodes_event {
            assert_eq!(node_ids.len(), 3);
        }

        // E2: 2 edges
        let e2 = Emission::new()
            .with_edge(edge("A", "B"))
            .with_edge(edge("B", "C"));
        let r2 = sink.emit(e2).await.unwrap();

        let edges_event = r2.events.iter().find(|e| matches!(e, GraphEvent::EdgesAdded { .. }));
        assert!(edges_event.is_some());
        if let Some(GraphEvent::EdgesAdded { edge_ids, .. }) = edges_event {
            assert_eq!(edge_ids.len(), 2);
        }
    }

    // === Scenario: Early emissions visible to queries before later emissions ===
    #[tokio::test]
    async fn early_emissions_visible_before_later() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        // E1: node A
        sink.emit(Emission::new().with_node(node("A"))).await.unwrap();

        // Node A is visible immediately
        {
            let ctx = ctx.lock().unwrap();
            assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
            // E2 not yet emitted — concept-X doesn't exist
            assert!(ctx.get_node(&NodeId::from_string("concept-X")).is_none());
        }

        // E2: concept-X
        sink.emit(Emission::new().with_node(node("concept-X"))).await.unwrap();

        // Now both exist
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("concept-X")).is_some());
    }

    // ================================================================
    // ADR-006: Adapter-to-Engine Wiring Scenarios
    // ================================================================

    fn make_engine_sink(
        engine: &Arc<PlexusEngine>,
        ctx_id: &ContextId,
        adapter_id: &str,
    ) -> EngineSink {
        EngineSink::for_engine(engine.clone(), ctx_id.clone())
            .with_framework_context(FrameworkContext {
                adapter_id: adapter_id.to_string(),
                context_id: ctx_id.as_str().to_string(),
                input_summary: None,
            })
    }

    // === Scenario: Emission through engine-backed sink reaches storage ===
    #[tokio::test]
    async fn emission_through_engine_backed_sink_reaches_storage() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("provence-research");
        engine.upsert_context(Context::with_id(ctx_id.clone(), "provence-research")).unwrap();

        // Create engine-backed sink
        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone());

        // Emit a single node
        let result = sink.emit(Emission::new().with_node(node("concept:travel"))).await.unwrap();
        assert_eq!(result.nodes_committed, 1);

        // Node exists in engine's in-memory context
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());

        // After restarting (new engine from same store, hydrate from storage)
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();
        assert!(ctx2.get_node(&NodeId::from_string("concept:travel")).is_some());
    }

    // === Scenario: Emission through engine-backed sink persists edges with contributions ===
    #[tokio::test]
    async fn emission_persists_edges_with_contributions() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("provence-research");
        engine.upsert_context(Context::with_id(ctx_id.clone(), "provence-research")).unwrap();

        // Pre-populate two nodes
        {
            let mut ctx = engine.get_context(&ctx_id).unwrap();
            ctx.add_node(node("concept:travel"));
            ctx.add_node(node("concept:avignon"));
            engine.upsert_context(ctx).unwrap();
        }

        // Create engine-backed sink with adapter identity
        let sink = make_engine_sink(&engine, &ctx_id, "fragment-manual");

        // Emit an edge with contribution value 0.75
        let mut e = edge("concept:travel", "concept:avignon");
        e.raw_weight = 0.75;
        let result = sink.emit(Emission::new().with_edge(e)).await.unwrap();
        assert_eq!(result.edges_committed, 1);

        // After restarting, edge exists with correct contribution
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();
        assert_eq!(ctx2.edges.len(), 1);
        assert_eq!(
            ctx2.edges[0].contributions.get("fragment-manual"),
            Some(&0.75),
            "contribution should survive persistence round-trip"
        );
    }

    // === Scenario: Emission to a non-existent context returns an error ===
    #[tokio::test]
    async fn emission_to_nonexistent_context_returns_error() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        // No context "does-not-exist" created
        let sink = EngineSink::for_engine(engine.clone(), ContextId::from("does-not-exist"));

        let result = sink.emit(Emission::new().with_node(node("concept:travel"))).await;
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), AdapterError::ContextNotFound(_)),
            "should be ContextNotFound error"
        );

        // No data persisted
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        assert_eq!(engine2.context_count(), 0);
    }

    // === Scenario: Persist-per-emission writes once per emit call ===
    #[tokio::test]
    async fn persist_per_emission_multi_item() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("provence-research");
        engine.upsert_context(Context::with_id(ctx_id.clone(), "provence-research")).unwrap();

        let sink = EngineSink::for_engine(engine.clone(), ctx_id.clone());

        // Emit 3 nodes and 2 edges in one emission
        let emission = Emission::new()
            .with_node(node("A"))
            .with_node(node("B"))
            .with_node(node("C"))
            .with_edge(edge("A", "B"))
            .with_edge(edge("B", "C"));

        let result = sink.emit(emission).await.unwrap();
        assert_eq!(result.nodes_committed, 3);
        assert_eq!(result.edges_committed, 2);

        // All survive a restart (single persist, not per-item)
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();
        assert_eq!(ctx2.node_count(), 3);
        assert_eq!(ctx2.edge_count(), 2);
    }

    // === Scenario: Scale normalization works after reload (ADR-007) ===
    #[tokio::test]
    async fn scale_normalization_works_after_reload() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("provence-research");
        engine.upsert_context(Context::with_id(ctx_id.clone(), "provence-research")).unwrap();

        // Pre-populate nodes
        {
            let mut ctx = engine.get_context(&ctx_id).unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
            engine.upsert_context(ctx).unwrap();
        }

        // Emit two edges from adapter-1 with different contributions
        let sink = make_engine_sink(&engine, &ctx_id, "adapter-1");
        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 10.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2)).await.unwrap();

        // Capture in-memory raw weights
        let ctx = engine.get_context(&ctx_id).unwrap();
        let ab_weight = ctx.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap().raw_weight;
        let ac_weight = ctx.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap().raw_weight;

        // Load from storage (simulating restart)
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let mut ctx2 = engine2.get_context(&ctx_id).unwrap();

        // Recompute raw weights from persisted contributions
        ctx2.recompute_raw_weights();

        let ab_reloaded = ctx2.edges.iter().find(|e| e.target == NodeId::from_string("B")).unwrap().raw_weight;
        let ac_reloaded = ctx2.edges.iter().find(|e| e.target == NodeId::from_string("C")).unwrap().raw_weight;

        assert!((ab_weight - ab_reloaded).abs() < 1e-6,
            "A→B raw weight should match after reload: {} vs {}", ab_weight, ab_reloaded);
        assert!((ac_weight - ac_reloaded).abs() < 1e-6,
            "A→C raw weight should match after reload: {} vs {}", ac_weight, ac_reloaded);
    }

    // ================================================================
    // End-to-End: Provence Travel Research
    // ================================================================

    // === Scenario: Full workflow from ingestion through marking to query ===
    #[tokio::test]
    async fn end_to_end_provence_travel_research() {
        use crate::adapter::ingest::IngestPipeline;
        use crate::adapter::provenance_adapter::{ProvenanceAdapter, ProvenanceInput};
        use crate::adapter::tag_bridger::TagConceptBridger;
        use crate::graph::dimension;

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("provence-research");
        engine.upsert_context(Context::with_id(ctx_id.clone(), "provence-research")).unwrap();

        // Step 1: FragmentAdapter processes a fragment
        let adapter = FragmentAdapter::new("manual-fragment");
        let sink = make_engine_sink(&engine, &ctx_id, "manual-fragment");

        let input = AdapterInput::new(
            "fragment",
            FragmentInput::new(
                "Morning walk in Avignon",
                vec!["travel".to_string(), "avignon".to_string()],
            ),
            "provence-research",
        );
        adapter.process(&input, &sink).await.unwrap();

        // Verify: fragment node + concept nodes + tagged_with edges
        {
            let ctx = engine.get_context(&ctx_id).unwrap();
            let fragments: Vec<_> = ctx.nodes().filter(|n| n.dimension == dimension::STRUCTURE).collect();
            assert_eq!(fragments.len(), 1, "should have 1 fragment node");

            assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());
            assert!(ctx.get_node(&NodeId::from_string("concept:avignon")).is_some());

            let tagged_with: Vec<_> = ctx.edges().filter(|e| e.relationship == "tagged_with").collect();
            assert_eq!(tagged_with.len(), 2, "should have 2 tagged_with edges");
        }

        // Step 2: Create provenance chain and add mark with tags through ingest pipeline
        // (Tag bridging happens via TagConceptBridger enrichment in the pipeline)
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            vec![
                Arc::new(TagConceptBridger::new()),
                Arc::new(CoOccurrenceEnrichment::new()),
            ],
        );

        // Create chain
        let chain_id = NodeId::new().to_string();
        pipeline.ingest(
            ctx_id.as_str(),
            "provenance",
            Box::new(ProvenanceInput::CreateChain {
                chain_id: chain_id.clone(),
                name: "reading-notes".to_string(),
                description: None,
            }),
        ).await.unwrap();

        // Add mark with tags — TagConceptBridger should create references edges
        let mark_id = NodeId::new().to_string();
        pipeline.ingest(
            ctx_id.as_str(),
            "provenance",
            Box::new(ProvenanceInput::AddMark {
                mark_id: mark_id.clone(),
                chain_id: chain_id.clone(),
                file: "notes.md".to_string(),
                line: 10,
                annotation: "walking through Avignon".to_string(),
                column: None,
                mark_type: None,
                tags: Some(vec!["#travel".into(), "#avignon".into()]),
            }),
        ).await.unwrap();

        // Verify: references edges from mark to concepts
        {
            let ctx = engine.get_context(&ctx_id).unwrap();
            let mark_node_id = NodeId::from(mark_id.as_str());
            let refs: Vec<_> = ctx.edges().filter(|e| {
                e.source == mark_node_id && e.relationship == "references"
            }).collect();
            assert_eq!(refs.len(), 2, "mark should have 2 references edges");

            let targets: std::collections::HashSet<String> = refs.iter()
                .map(|e| e.target.to_string()).collect();
            assert!(targets.contains("concept:travel"));
            assert!(targets.contains("concept:avignon"));

            // Traverse from concept:avignon via incoming references to reach the mark
            let avignon_id = NodeId::from_string("concept:avignon");
            let incoming_refs: Vec<_> = ctx.edges().filter(|e| {
                e.target == avignon_id && e.relationship == "references"
            }).collect();
            assert_eq!(incoming_refs.len(), 1);
            assert_eq!(incoming_refs[0].source, mark_node_id);
        }

        // Step 3: Verify persistence after restart
        drop(engine);
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();

        // All nodes survive
        assert!(ctx2.get_node(&NodeId::from_string("concept:travel")).is_some());
        assert!(ctx2.get_node(&NodeId::from_string("concept:avignon")).is_some());
        assert!(ctx2.get_node(&NodeId::from(mark_id.as_str())).is_some());

        // All edges survive
        let tagged_with: Vec<_> = ctx2.edges().filter(|e| e.relationship == "tagged_with").collect();
        assert_eq!(tagged_with.len(), 2);
        let references: Vec<_> = ctx2.edges().filter(|e| e.relationship == "references").collect();
        assert_eq!(references.len(), 2);

        // Contributions survive
        for edge in &tagged_with {
            assert_eq!(edge.contributions.get("manual-fragment"), Some(&1.0));
        }
    }

    // === Scenario: Existing Mutex-based sink still works for tests ===
    #[tokio::test]
    async fn existing_mutex_sink_still_works() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        let result = sink.emit(Emission::new().with_node(node("A"))).await.unwrap();
        assert_eq!(result.nodes_committed, 1);

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        // No persistence (no GraphStore involved) — this is tested implicitly
        // by the fact that no store setup is needed
    }

    // ================================================================
    // Enrichment Loop Scenarios (ADR-010)
    // ================================================================

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
            .with_framework_context(framework)
            .with_enrichments(registry);

        // Emit a node
        let result = sink
            .emit(Emission::new().with_node(node("concept:travel")))
            .await
            .unwrap();
        assert_eq!(result.nodes_committed, 1);

        // Enrichment was called exactly once (one round, then quiescent)
        assert_eq!(enrichment.call_count(), 1);

        // It received the NodesAdded event
        let (events, snapshot) = enrichment.last_call().unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, GraphEvent::NodesAdded { .. })));

        // The snapshot contains the newly added node
        assert!(snapshot
            .get_node(&NodeId::from_string("concept:travel"))
            .is_some());
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
            edge.raw_weight = 0.75;
            Some(Emission::new().with_edge(edge))
        }
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
            .with_framework_context(framework)
            .with_enrichments(registry);

        // Emit a no-op node to trigger the enrichment loop
        let result = sink
            .emit(Emission::new().with_node(node("trigger-node")))
            .await
            .unwrap();

        // Primary emission: 1 node. Enrichment: 1 edge.
        assert!(result.nodes_committed >= 1);
        assert_eq!(result.edges_committed, 1);

        // The may_be_related edge exists in the context
        let ctx = engine.get_context(&ctx_id).unwrap();
        let edge = ctx
            .edges
            .iter()
            .find(|e| e.relationship == "may_be_related")
            .expect("may_be_related edge should exist");

        assert_eq!(edge.source, NodeId::from_string("concept:travel"));
        assert_eq!(edge.target, NodeId::from_string("concept:avignon"));

        // The edge has a contribution from the enrichment's id with the emitted value
        assert_eq!(edge.contributions.get("co-occurrence"), Some(&0.75));
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
            .with_framework_context(framework)
            .with_enrichments(registry);

        let result = sink
            .emit(Emission::new().with_node(node("A")))
            .await
            .unwrap();

        // Loop completed in one round (enrichment called once, returned None)
        assert_eq!(enrichment.call_count(), 1);

        // No additional mutations beyond the primary emission
        assert_eq!(result.nodes_committed, 1);
        assert_eq!(result.edges_committed, 0);
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
            .with_framework_context(framework)
            .with_enrichments(registry);

        // Primary emission triggers the loop
        let result = sink
            .emit(Emission::new().with_node(node("trigger")))
            .await
            .unwrap();

        // Primary: 1 node (trigger). Round 0: enrichment A adds new-node.
        // Round 1: enrichment B sees new-node event, emits edge.
        // Round 2: both return None → quiescence.
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("new-node")).is_some());
        assert!(ctx
            .edges
            .iter()
            .any(|e| e.source == NodeId::from_string("existing-node")
                && e.target == NodeId::from_string("new-node")
                && e.relationship == "depends_on"));

        // Total: trigger (primary) + new-node (enrichment A) = 2 nodes, 1 edge (enrichment B)
        assert_eq!(result.nodes_committed, 2);
        assert_eq!(result.edges_committed, 1);
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
            .with_framework_context(framework)
            .with_enrichments(registry);

        sink.emit(Emission::new().with_node(node("primary-node")))
            .await
            .unwrap();

        let calls = enrichment.calls.lock().unwrap();
        // Round 0 saw primary events (NodesAdded for primary-node)
        assert!(calls[0]
            .iter()
            .any(|e| matches!(e, GraphEvent::NodesAdded { .. })));

        // Round 1 saw enrichment events (NodesAdded for enrichment-node), NOT primary events
        assert_eq!(calls.len(), 2);
        if let GraphEvent::NodesAdded { node_ids, adapter_id, .. } = &calls[1][0] {
            // Round 1 events come from the enrichment, not the primary adapter
            assert_eq!(adapter_id, "recorder");
            assert!(node_ids.iter().any(|id| id.to_string() == "enrichment-node"));
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
            .with_framework_context(framework)
            .with_enrichments(registry);

        let result = sink
            .emit(Emission::new().with_node(node("trigger")))
            .await
            .unwrap();

        // Safety valve at 3 rounds: primary (1 node) + 3 enrichment rounds (3 nodes) = 4
        assert_eq!(result.nodes_committed, 4);

        // The enrichment was called exactly 3 times (the max rounds)
        assert_eq!(*enrichment.counter.lock().unwrap(), 3);
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
            .with_framework_context(framework)
            .with_enrichments(registry);

        sink.emit(Emission::new().with_node(node("A")))
            .await
            .unwrap();

        // Only the first enrichment instance was called (dedup by id)
        assert_eq!(enrichment_a.call_count(), 1);
        assert_eq!(enrichment_b.call_count(), 0);
    }

    // ================================================================
    // Bidirectional Adapter — Outbound Events (ADR-011)
    // ================================================================

    /// Minimal adapter that doesn't override transform_events.
    struct MinimalAdapter;

    #[async_trait::async_trait]
    impl Adapter for MinimalAdapter {
        fn id(&self) -> &str {
            "minimal"
        }
        fn input_kind(&self) -> &str {
            "test"
        }
        async fn process(
            &self,
            _input: &AdapterInput,
            _sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
        // transform_events NOT overridden — uses default
    }

    // === Scenario: Default transform_events returns empty vec ===
    #[test]
    fn default_transform_events_returns_empty_vec() {
        let adapter = MinimalAdapter;
        let ctx = Context::new("test");
        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![NodeId::from_string("A")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        let outbound = adapter.transform_events(&events, &ctx);
        assert!(outbound.is_empty());
    }

    /// Adapter that translates NodesAdded events to "concepts_detected" outbound events.
    struct ConceptDetectingAdapter {
        id: String,
    }

    impl ConceptDetectingAdapter {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Adapter for ConceptDetectingAdapter {
        fn id(&self) -> &str {
            &self.id
        }
        fn input_kind(&self) -> &str {
            "fragment"
        }
        async fn process(
            &self,
            _input: &AdapterInput,
            _sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
        fn transform_events(
            &self,
            events: &[GraphEvent],
            _context: &Context,
        ) -> Vec<OutboundEvent> {
            let mut outbound = Vec::new();
            for event in events {
                if let GraphEvent::NodesAdded { node_ids, .. } = event {
                    let concepts: Vec<String> = node_ids
                        .iter()
                        .filter(|id| id.to_string().starts_with("concept:"))
                        .map(|id| id.to_string().strip_prefix("concept:").unwrap().to_string())
                        .collect();
                    if !concepts.is_empty() {
                        outbound.push(OutboundEvent::new(
                            "concepts_detected",
                            concepts.join(", "),
                        ));
                    }
                }
            }
            outbound
        }
    }

    // === Scenario: Adapter translates graph events to domain-meaningful outbound events ===
    #[test]
    fn adapter_translates_graph_events_to_outbound_events() {
        let adapter = ConceptDetectingAdapter::new("fragment-adapter");
        let ctx = Context::new("test");
        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![
                NodeId::from_string("concept:travel"),
                NodeId::from_string("concept:avignon"),
            ],
            adapter_id: "fragment-adapter".to_string(),
            context_id: "test".to_string(),
        }];

        let outbound = adapter.transform_events(&events, &ctx);
        assert_eq!(outbound.len(), 1);
        assert_eq!(outbound[0].kind, "concepts_detected");
        assert_eq!(outbound[0].detail, "travel, avignon");
    }

    // ================================================================
    // Unified Ingest Pipeline (ADR-012)
    // ================================================================

    use crate::adapter::ingest::IngestPipeline;
    use crate::adapter::tag_bridger::TagConceptBridger;

    /// Test adapter that emits concept nodes from input strings.
    /// Also implements transform_events to detect concept nodes.
    struct EmittingAdapter {
        id: String,
        input_kind: String,
    }

    impl EmittingAdapter {
        fn new(id: &str, input_kind: &str) -> Self {
            Self {
                id: id.to_string(),
                input_kind: input_kind.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Adapter for EmittingAdapter {
        fn id(&self) -> &str {
            &self.id
        }
        fn input_kind(&self) -> &str {
            &self.input_kind
        }
        async fn process(
            &self,
            input: &AdapterInput,
            sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            let concepts = input
                .downcast_data::<Vec<String>>()
                .ok_or(AdapterError::InvalidInput)?;
            for concept in concepts {
                let mut n = node(&format!("concept:{}", concept));
                n.dimension = dimension::SEMANTIC.to_string();
                sink.emit(Emission::new().with_node(n)).await?;
            }
            Ok(())
        }
        fn transform_events(
            &self,
            events: &[GraphEvent],
            _context: &Context,
        ) -> Vec<OutboundEvent> {
            let mut outbound = Vec::new();
            for event in events {
                if let GraphEvent::NodesAdded { node_ids, .. } = event {
                    let concepts: Vec<String> = node_ids
                        .iter()
                        .filter(|id| id.to_string().starts_with("concept:"))
                        .map(|id| {
                            id.to_string()
                                .strip_prefix("concept:")
                                .unwrap()
                                .to_string()
                        })
                        .collect();
                    if !concepts.is_empty() {
                        outbound.push(OutboundEvent::new(
                            "concepts_detected",
                            concepts.join(", "),
                        ));
                    }
                }
            }
            outbound
        }
    }

    // === Scenario: ingest routes to adapter by input_kind ===
    #[tokio::test]
    async fn ingest_routes_to_adapter_by_input_kind() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(adapter);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string(), "avignon".to_string()]);

        let outbound = pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Adapter processed — concept nodes exist
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("concept:avignon")).is_some());

        // Outbound events from transform_events
        assert!(!outbound.is_empty());
        assert!(outbound.iter().any(|e| e.kind == "concepts_detected"));
    }

    // === Scenario: ingest with unknown input_kind returns error ===
    #[tokio::test]
    async fn ingest_unknown_input_kind_returns_error() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let pipeline = IngestPipeline::new(engine.clone());

        let data: Box<dyn std::any::Any + Send + Sync> = Box::new("anything".to_string());
        let result = pipeline
            .ingest("provence-research", "unknown", data)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("no adapter"),
            "error should mention no adapter: {}",
            err
        );
    }

    // === Scenario: Full ingest pipeline end-to-end ===
    #[tokio::test]
    async fn full_ingest_pipeline_end_to_end() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Enrichment that creates may_be_related edges between concept nodes
        let enrichment = Arc::new(OneShotEdgeEnrichment::new(
            "co-occurrence",
            "concept:travel",
            "concept:avignon",
        ));
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment as Arc<dyn Enrichment>,
        ]));

        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(adapter);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string(), "avignon".to_string()]);

        let outbound = pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Primary: concept nodes created
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("concept:avignon")).is_some());

        // Enrichment: may_be_related edge created
        let edge = ctx
            .edges
            .iter()
            .find(|e| e.relationship == "may_be_related")
            .expect("enrichment should create may_be_related edge");
        assert_eq!(edge.source, NodeId::from_string("concept:travel"));
        assert_eq!(edge.target, NodeId::from_string("concept:avignon"));

        // Outbound: includes events from both primary and enrichment rounds
        assert!(!outbound.is_empty());

        // Return type is Vec<OutboundEvent>, not GraphEvent — consumer contract
        let _: Vec<OutboundEvent> = outbound;
    }

    // === Scenario: Fan-out — multiple adapters matching same input_kind ===
    #[tokio::test]
    async fn ingest_fan_out_multiple_adapters() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Two adapters for the same input_kind, each detecting different concepts
        let adapter_a = Arc::new(EmittingAdapter::new("adapter-a", "fragment"));
        let adapter_b = Arc::new(EmittingAdapter::new("adapter-b", "fragment"));

        // An enrichment that records calls to verify it runs once, not per-adapter
        let enrichment = Arc::new(RecordingEnrichment::new("test-enrichment"));
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment.clone() as Arc<dyn Enrichment>,
        ]));

        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(adapter_a);
        pipeline.register_adapter(adapter_b);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string()]);

        let outbound = pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Both adapters processed (each creates concept:travel)
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());

        // Enrichment loop ran ONCE (not per-adapter)
        assert_eq!(enrichment.call_count(), 1);

        // Both adapters' transform_events were called — each sees the full event set.
        // Two adapters × two NodesAdded events (one from each adapter) = 4 outbound events.
        // The key: each adapter independently filters the same accumulated events.
        assert!(
            outbound.iter().filter(|e| e.kind == "concepts_detected").count() >= 2,
            "both adapters should produce outbound events, got {}",
            outbound.len()
        );
    }

    // === Scenario: Integration bundles adapter and enrichments ===
    #[tokio::test]
    async fn integration_bundles_adapter_and_enrichments() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let enrichment = Arc::new(OneShotEdgeEnrichment::new(
            "co-occurrence",
            "concept:travel",
            "concept:avignon",
        ));

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            adapter,
            vec![enrichment as Arc<dyn Enrichment>],
        );

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string(), "avignon".to_string()]);

        pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Adapter processed
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());

        // Enrichment ran (via register_integration, not separate with_enrichments)
        assert!(ctx
            .edges
            .iter()
            .any(|e| e.relationship == "may_be_related"));
    }

    // === Scenario: Enrichments from multiple integrations are deduplicated ===
    #[tokio::test]
    async fn integration_enrichments_deduplicated_across_registrations() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Two integrations register the same enrichment id
        let adapter_a = Arc::new(EmittingAdapter::new("adapter-a", "fragment"));
        let enrichment_a = Arc::new(RecordingEnrichment::new("tag-bridger"));

        let adapter_b = Arc::new(EmittingAdapter::new("adapter-b", "other"));
        let enrichment_b = Arc::new(RecordingEnrichment::new("tag-bridger"));

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            adapter_a,
            vec![enrichment_a.clone() as Arc<dyn Enrichment>],
        );
        pipeline.register_integration(
            adapter_b,
            vec![enrichment_b.clone() as Arc<dyn Enrichment>],
        );

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string()]);

        pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Only the first enrichment instance was called (dedup by id)
        assert_eq!(enrichment_a.call_count(), 1);
        assert_eq!(enrichment_b.call_count(), 0);
    }

    // === Scenario: Consumer receives outbound events, never raw graph events ===
    #[tokio::test]
    async fn consumer_receives_outbound_events_not_graph_events() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(adapter);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string()]);

        // The return type is Vec<OutboundEvent> — this is a compile-time guarantee
        let outbound: Vec<OutboundEvent> = pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // The consumer gets domain-meaningful events, not raw GraphEvents
        assert!(!outbound.is_empty());
        assert_eq!(outbound[0].kind, "concepts_detected");
    }

    // ================================================================
    // TagConceptBridger Integration (ADR-009 + ADR-010)
    // ================================================================

    use crate::graph::{dimension, PropertyValue};

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

    // === Scenario: New concept retroactively bridges to existing mark via enrichment loop ===
    #[tokio::test]
    async fn tag_bridger_new_concept_retroactively_bridges_to_existing_mark() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");

        // Pre-populate with a mark tagged #travel but no concept yet
        let mut ctx = Context::with_id(ctx_id.clone(), "provence-research");
        ctx.add_node(mark_node("mark-1", &["#travel"]));
        engine.upsert_context(ctx).unwrap();

        // Register TagConceptBridger
        let bridger = Arc::new(TagConceptBridger::new());
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            bridger as Arc<dyn Enrichment>,
        ]));

        // Adapter creates concept:travel — triggers enrichment loop
        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(adapter);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["travel".to_string()]);
        pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // TagConceptBridger should have created a references edge from mark to concept
        let ctx = engine.get_context(&ctx_id).unwrap();
        let refs: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from_string("mark-1")
                    && e.target == NodeId::from_string("concept:travel")
                    && e.relationship == "references"
            })
            .collect();
        assert_eq!(refs.len(), 1, "should have exactly one references edge");
        assert_eq!(refs[0].source_dimension, dimension::PROVENANCE);
        assert_eq!(refs[0].target_dimension, dimension::SEMANTIC);
    }

    // === Scenario: TagConceptBridger is idempotent through enrichment loop ===
    #[tokio::test]
    async fn tag_bridger_idempotent_through_enrichment_loop() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");

        // Pre-populate: mark, concept, and an existing references edge
        let mut ctx = Context::with_id(ctx_id.clone(), "provence-research");
        ctx.add_node(mark_node("mark-1", &["#travel"]));
        ctx.add_node(concept_node("travel"));
        ctx.add_edge(Edge::new_cross_dimensional(
            NodeId::from_string("mark-1"),
            dimension::PROVENANCE,
            NodeId::from_string("concept:travel"),
            dimension::SEMANTIC,
            "references",
        ));
        engine.upsert_context(ctx).unwrap();

        let bridger = Arc::new(TagConceptBridger::new());
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            bridger as Arc<dyn Enrichment>,
        ]));

        // Adapter emits an unrelated concept — triggers enrichment loop
        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(adapter);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["avignon".to_string()]);
        pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Should NOT have created a duplicate references edge
        let ctx = engine.get_context(&ctx_id).unwrap();
        let refs: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from_string("mark-1")
                    && e.target == NodeId::from_string("concept:travel")
                    && e.relationship == "references"
            })
            .collect();
        assert_eq!(refs.len(), 1, "should still have exactly one references edge, not a duplicate");
    }

    // ================================================================
    // CoOccurrenceEnrichment through Enrichment Loop (ADR-010)
    // ================================================================

    // === Scenario: Idempotent enrichment does not loop indefinitely ===
    #[tokio::test]
    async fn cooccurrence_enrichment_idempotent_through_loop() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");

        // Pre-populate: two fragments sharing concepts, with existing may_be_related edges
        let mut ctx = Context::with_id(ctx_id.clone(), "provence-research");

        // Add concept nodes
        let mut travel = Node::new("concept", ContentType::Concept);
        travel.id = NodeId::from_string("concept:travel");
        travel.dimension = dimension::SEMANTIC.to_string();
        ctx.add_node(travel);

        let mut avignon = Node::new("concept", ContentType::Concept);
        avignon.id = NodeId::from_string("concept:avignon");
        avignon.dimension = dimension::SEMANTIC.to_string();
        ctx.add_node(avignon);

        // Add a fragment with tagged_with edges to both concepts
        let mut frag = Node::new("fragment", ContentType::Narrative);
        frag.id = NodeId::from_string("frag:f1");
        frag.dimension = dimension::STRUCTURE.to_string();
        ctx.add_node(frag);

        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("frag:f1"),
            NodeId::from_string("concept:travel"),
            "tagged_with",
            dimension::SEMANTIC,
        ));
        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("frag:f1"),
            NodeId::from_string("concept:avignon"),
            "tagged_with",
            dimension::SEMANTIC,
        ));

        // Pre-existing may_be_related edges (from a prior enrichment run)
        let mut edge_ta = Edge::new_in_dimension(
            NodeId::from_string("concept:travel"),
            NodeId::from_string("concept:avignon"),
            "may_be_related",
            dimension::SEMANTIC,
        );
        edge_ta.raw_weight = 1.0;
        ctx.add_edge(edge_ta);

        let mut edge_at = Edge::new_in_dimension(
            NodeId::from_string("concept:avignon"),
            NodeId::from_string("concept:travel"),
            "may_be_related",
            dimension::SEMANTIC,
        );
        edge_at.raw_weight = 1.0;
        ctx.add_edge(edge_at);

        engine.upsert_context(ctx).unwrap();

        // Register CoOccurrenceEnrichment and trigger via unrelated adapter emission
        let enrichment = Arc::new(CoOccurrenceEnrichment::new());
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment as Arc<dyn Enrichment>,
        ]));

        let adapter = Arc::new(EmittingAdapter::new("fragment-adapter", "fragment"));
        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(adapter);

        // Ingest a new concept — triggers enrichment loop
        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(vec!["paris".to_string()]);
        pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // Verify: the enrichment should have detected the new concept and added
        // may_be_related edges for paris (which shares no fragments yet), but
        // should NOT have duplicated the existing travel↔avignon edges.
        let ctx = engine.get_context(&ctx_id).unwrap();
        let travel_avignon: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from_string("concept:travel")
                    && e.target == NodeId::from_string("concept:avignon")
                    && e.relationship == "may_be_related"
            })
            .collect();
        assert_eq!(
            travel_avignon.len(),
            1,
            "should still have exactly one may_be_related edge (travel→avignon), not a duplicate"
        );
    }

    // ================================================================
    // ProvenanceAdapter through Ingest Pipeline (ADR-012)
    // ================================================================

    use crate::adapter::provenance_adapter::{ProvenanceAdapter, ProvenanceInput};

    // === Scenario: AddMark through ingest triggers TagConceptBridger ===
    #[tokio::test]
    async fn provenance_add_mark_through_ingest_triggers_tag_bridger() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");

        // Pre-populate: concept:travel exists, chain exists
        let mut ctx = Context::with_id(ctx_id.clone(), "provence-research");
        ctx.add_node(concept_node("travel"));
        let mut chain = Node::new_in_dimension(
            "chain",
            ContentType::Provenance,
            dimension::PROVENANCE,
        );
        chain.id = NodeId::from("chain-1");
        ctx.add_node(chain);
        engine.upsert_context(ctx).unwrap();

        // Register ProvenanceAdapter + TagConceptBridger
        let adapter = Arc::new(ProvenanceAdapter::new());
        let bridger = Arc::new(TagConceptBridger::new());
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            bridger as Arc<dyn Enrichment>,
        ]));

        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(adapter);

        // Ingest a mark with tag #travel
        let data: Box<dyn std::any::Any + Send + Sync> = Box::new(ProvenanceInput::AddMark {
            mark_id: "mark-1".to_string(),
            chain_id: "chain-1".to_string(),
            file: "notes.md".to_string(),
            line: 42,
            annotation: "walking through Avignon".to_string(),
            column: None,
            mark_type: None,
            tags: Some(vec!["#travel".to_string()]),
        });
        pipeline
            .ingest("provence-research", "provenance", data)
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();

        // Mark node exists
        assert!(ctx.get_node(&NodeId::from("mark-1")).is_some());

        // Contains edge: chain → mark
        let contains: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from("chain-1")
                    && e.target == NodeId::from("mark-1")
                    && e.relationship == "contains"
            })
            .collect();
        assert_eq!(contains.len(), 1);

        // TagConceptBridger should have created references edge: mark → concept:travel
        let refs: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from("mark-1")
                    && e.target == NodeId::from_string("concept:travel")
                    && e.relationship == "references"
            })
            .collect();
        assert_eq!(
            refs.len(),
            1,
            "TagConceptBridger should bridge mark to concept:travel"
        );
        assert_eq!(refs[0].source_dimension, dimension::PROVENANCE);
        assert_eq!(refs[0].target_dimension, dimension::SEMANTIC);
    }

    // === Scenario: CreateChain through ingest produces chain node ===
    #[tokio::test]
    async fn provenance_create_chain_through_ingest() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        let adapter = Arc::new(ProvenanceAdapter::new());
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(adapter);

        let data: Box<dyn std::any::Any + Send + Sync> =
            Box::new(ProvenanceInput::CreateChain {
                chain_id: "chain-1".to_string(),
                name: "reading-notes".to_string(),
                description: Some("Notes from reading".to_string()),
            });
        pipeline
            .ingest("provence-research", "provenance", data)
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();
        let chain = ctx
            .get_node(&NodeId::from("chain-1"))
            .expect("chain node should exist");
        assert_eq!(chain.node_type, "chain");
        assert_eq!(chain.dimension, dimension::PROVENANCE);
    }

    // === Scenario: CoOccurrenceEnrichment fires through the pipeline on new fragments ===
    #[tokio::test]
    async fn cooccurrence_enrichment_fires_in_pipeline() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("provence-research");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provence-research"))
            .unwrap();

        // Use the real FragmentAdapter to create structure + semantic nodes
        let fragment_adapter = Arc::new(FragmentAdapter::new("trellis-fragment"));
        let enrichment = Arc::new(CoOccurrenceEnrichment::new());
        let registry = Arc::new(EnrichmentRegistry::new(vec![
            enrichment as Arc<dyn Enrichment>,
        ]));

        let mut pipeline = IngestPipeline::new(engine.clone())
            .with_enrichments(registry);
        pipeline.register_adapter(fragment_adapter);

        // Ingest a fragment with two tags → two concepts → one co-occurrence pair
        let data: Box<dyn std::any::Any + Send + Sync> = Box::new(FragmentInput::new(
            "Walk in Avignon",
            vec!["travel".to_string(), "avignon".to_string()],
        ));
        pipeline
            .ingest("provence-research", "fragment", data)
            .await
            .unwrap();

        // The enrichment loop should have produced may_be_related edges
        let ctx = engine.get_context(&ctx_id).unwrap();
        let may_be_related: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "may_be_related")
            .collect();

        // Symmetric pair: travel↔avignon
        assert_eq!(may_be_related.len(), 2);

        // Verify contribution tracking
        for edge in &may_be_related {
            assert!(
                edge.contributions.contains_key("co_occurrence:tagged_with:may_be_related"),
                "edge should have co-occurrence contribution"
            );
        }
    }

    // NOTE: The Essay 11 spike test (spike_two_consumer_cross_dimensional_validation)
    // was removed here. Essay 12 changed the architecture (FragmentAdapter now produces
    // provenance alongside semantics), making the Essay 11 test's framing incorrect.
    // Its scenarios are covered at larger scale by the Essay 13 spikes below.
    // See docs/research/semantic/essays/11-two-consumer-validation.md for history.

    // ================================================================
    // Spike: Provenance as Epistemological Infrastructure (Essay 12)
    // ================================================================
    //
    // Validates that FragmentAdapter automatically produces provenance
    // marks alongside semantic output, and TagConceptBridger bridges
    // those marks to concepts — making every concept's origin
    // graph-traversable.

    #[tokio::test]
    async fn spike_fragment_adapter_produces_traversable_provenance() {
        use crate::adapter::ingest::IngestPipeline;
        use crate::adapter::tag_bridger::TagConceptBridger;
        use crate::graph::dimension;

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store));

        let ctx_id = ContextId::from("provenance-spike");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "provenance-spike"))
            .unwrap();

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            Arc::new(FragmentAdapter::new("journal")),
            vec![
                Arc::new(TagConceptBridger::new()),
                Arc::new(CoOccurrenceEnrichment::new()),
            ],
        );

        // Ingest a single fragment
        let input = FragmentInput::new(
            "Walked through Avignon, thinking about distributed systems",
            vec!["travel".to_string(), "distributed-ai".to_string()],
        )
        .with_source("journal-2026-02");

        pipeline
            .ingest("provenance-spike", "fragment", Box::new(input))
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();

        // --- Semantic output: fragment + concepts + tagged_with ---
        let fragments: Vec<_> = ctx
            .nodes()
            .filter(|n| n.dimension == dimension::STRUCTURE && n.node_type == "fragment")
            .collect();
        assert_eq!(fragments.len(), 1, "1 fragment node");

        let concepts: Vec<_> = ctx
            .nodes()
            .filter(|n| n.dimension == dimension::SEMANTIC)
            .collect();
        assert_eq!(concepts.len(), 2, "2 concept nodes (travel, distributed-ai)");

        // --- Provenance output: chain + mark + contains ---
        let chain = ctx.get_node(&NodeId::from_string("chain:journal:journal-2026-02"));
        assert!(chain.is_some(), "chain node should exist");
        let chain = chain.unwrap();
        assert_eq!(chain.dimension, dimension::PROVENANCE);

        let marks: Vec<_> = ctx
            .nodes()
            .filter(|n| n.dimension == dimension::PROVENANCE && n.node_type == "mark")
            .collect();
        assert_eq!(marks.len(), 1, "1 mark node");

        // Mark carries source evidence
        let mark = marks[0];
        assert_eq!(
            mark.properties.get("file"),
            Some(&PropertyValue::String("journal-2026-02".to_string())),
        );
        assert_eq!(
            mark.properties.get("annotation"),
            Some(&PropertyValue::String(
                "Walked through Avignon, thinking about distributed systems".to_string()
            )),
        );

        let contains: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "contains")
            .collect();
        assert_eq!(contains.len(), 1, "chain → mark contains edge");

        // --- Cross-dimensional bridging: TagConceptBridger auto-bridged ---
        let references: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "references")
            .collect();
        assert_eq!(references.len(), 2, "mark references 2 concepts via tags");

        // Each references edge is cross-dimensional: provenance → semantic
        for ref_edge in &references {
            assert_eq!(ref_edge.source_dimension, dimension::PROVENANCE);
            assert_eq!(ref_edge.target_dimension, dimension::SEMANTIC);
        }

        // --- The key test: concept → provenance traversal ---
        // From concept:travel, can we find where the knowledge came from?
        let travel_id = NodeId::from_string("concept:travel");
        let incoming_refs: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == travel_id && e.relationship == "references")
            .collect();
        assert_eq!(incoming_refs.len(), 1, "1 mark references concept:travel");

        // The mark points back to the chain (via contains)
        let mark_id = &incoming_refs[0].source;
        let mark_chain: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == *mark_id && e.relationship == "contains")
            .collect();
        assert_eq!(mark_chain.len(), 1, "mark is contained in a chain");
        assert_eq!(
            mark_chain[0].source,
            NodeId::from_string("chain:journal:journal-2026-02"),
            "chain is the journal source"
        );
    }

    #[tokio::test]
    async fn spike_multi_phase_hebbian_provenance() {
        use crate::adapter::ingest::IngestPipeline;
        use crate::adapter::tag_bridger::TagConceptBridger;

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store));

        let ctx_id = ContextId::from("multi-phase");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "multi-phase"))
            .unwrap();

        // Two adapter instances: manual (L1) and LLM-extracted (L4)
        // Same source, different processing phases.
        // Separate pipelines because both share input_kind="fragment" —
        // in production each phase would run independently.
        let enrichments: Vec<Arc<dyn crate::adapter::enrichment::Enrichment>> = vec![
            Arc::new(TagConceptBridger::new()),
            Arc::new(CoOccurrenceEnrichment::new()),
        ];

        let mut manual_pipeline = IngestPipeline::new(engine.clone());
        manual_pipeline.register_integration(
            Arc::new(FragmentAdapter::new("manual-journal")),
            enrichments.clone(),
        );

        let mut llm_pipeline = IngestPipeline::new(engine.clone());
        llm_pipeline.register_integration(
            Arc::new(FragmentAdapter::new("llm-extract")),
            enrichments,
        );

        // Phase 1: Manual tagging (human applies broad tags)
        let manual_input = FragmentInput::new(
            "The federated approach distributes compute across nodes",
            vec!["distributed-ai".to_string()],
        )
        .with_source("paper-chen-2025");

        manual_pipeline
            .ingest("multi-phase", "fragment", Box::new(manual_input))
            .await
            .unwrap();

        // Phase 2: LLM extraction (richer tags from same source)
        let llm_input = FragmentInput::new(
            "The federated approach distributes compute across nodes",
            vec![
                "distributed-ai".to_string(),
                "federated-learning".to_string(),
                "compute-economics".to_string(),
            ],
        )
        .with_source("paper-chen-2025");

        llm_pipeline
            .ingest("multi-phase", "fragment", Box::new(llm_input))
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();

        // --- Hebbian accumulation: concept:distributed-ai has 2 tagged_with edges ---
        let dai_id = NodeId::from_string("concept:distributed-ai");
        let dai_edges: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == dai_id && e.relationship == "tagged_with")
            .collect();
        assert_eq!(
            dai_edges.len(),
            2,
            "2 fragments both tagged with distributed-ai"
        );

        // Each edge has a different adapter's contribution
        let adapter_ids: std::collections::HashSet<String> = dai_edges
            .iter()
            .flat_map(|e| e.contributions.keys().cloned())
            .collect();
        assert!(adapter_ids.contains("manual-journal"));
        assert!(adapter_ids.contains("llm-extract"));

        // --- Separate chains for same source but different adapters ---
        let manual_chain = ctx.get_node(&NodeId::from_string("chain:manual-journal:paper-chen-2025"));
        let llm_chain = ctx.get_node(&NodeId::from_string("chain:llm-extract:paper-chen-2025"));
        assert!(manual_chain.is_some(), "manual adapter has its own chain");
        assert!(llm_chain.is_some(), "LLM adapter has its own chain");

        // --- Provenance explains each contribution ---
        // Manual mark has 1 tag → 1 references edge
        let manual_marks: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.relationship == "contains"
                    && e.source == NodeId::from_string("chain:manual-journal:paper-chen-2025")
            })
            .collect();
        assert_eq!(manual_marks.len(), 1, "manual chain contains 1 mark");

        let manual_mark_id = &manual_marks[0].target;
        let manual_refs: Vec<_> = ctx
            .edges()
            .filter(|e| e.source == *manual_mark_id && e.relationship == "references")
            .collect();
        assert_eq!(manual_refs.len(), 1, "manual mark references 1 concept");

        // LLM mark has 3 tags → 3 references edges
        let llm_marks: Vec<_> = ctx
            .edges()
            .filter(|e| {
                e.relationship == "contains"
                    && e.source == NodeId::from_string("chain:llm-extract:paper-chen-2025")
            })
            .collect();
        assert_eq!(llm_marks.len(), 1, "LLM chain contains 1 mark");

        let llm_mark_id = &llm_marks[0].target;
        let llm_refs: Vec<_> = ctx
            .edges()
            .filter(|e| e.source == *llm_mark_id && e.relationship == "references")
            .collect();
        assert_eq!(llm_refs.len(), 3, "LLM mark references 3 concepts");

        // --- Progressive enrichment: LLM phase added concepts manual didn't see ---
        assert!(
            ctx.get_node(&NodeId::from_string("concept:federated-learning")).is_some(),
            "federated-learning concept added by LLM phase"
        );
        assert!(
            ctx.get_node(&NodeId::from_string("concept:compute-economics")).is_some(),
            "compute-economics concept added by LLM phase"
        );

        // --- Cross-dimensional traversal: concept → mark → chain → source ---
        // From concept:federated-learning, find where it came from
        let fl_id = NodeId::from_string("concept:federated-learning");
        let fl_incoming: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == fl_id && e.relationship == "references")
            .collect();
        assert_eq!(fl_incoming.len(), 1, "1 mark references federated-learning");

        // That mark is in the LLM chain
        let source_mark = &fl_incoming[0].source;
        let chain_edge: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == *source_mark && e.relationship == "contains")
            .collect();
        assert_eq!(chain_edge.len(), 1);
        assert_eq!(
            chain_edge[0].source,
            NodeId::from_string("chain:llm-extract:paper-chen-2025"),
            "federated-learning was extracted by LLM phase from paper-chen-2025"
        );
    }

    // ================================================================
    // Spike: Two-Consumer Validation Revisited (Essay 13)
    // ================================================================
    //
    // Heterogeneous multi-consumer scenario with real public data.
    // Three consumers feed the same context:
    //
    // 1. Trellis — writer's personal fragments (intuitive tags)
    // 2. Carrel LLM — extracted concepts from real arXiv papers (formal tags)
    // 3. Carrel provenance — human annotations linking research to writing
    //
    // Source material: real arXiv abstracts (2024-2025) on distributed AI,
    // federated learning, AI governance, and human-AI creativity.

    #[tokio::test]
    async fn spike_heterogeneous_multi_consumer_real_data() {
        use crate::adapter::ingest::IngestPipeline;
        use crate::adapter::provenance_adapter::{ProvenanceAdapter, ProvenanceInput};
        use crate::adapter::tag_bridger::TagConceptBridger;
        use crate::graph::dimension;
        use crate::query::{Direction, TraverseQuery};

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("research-workspace");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "research-workspace"))
            .unwrap();

        let enrichments: Vec<Arc<dyn crate::adapter::enrichment::Enrichment>> = vec![
            Arc::new(TagConceptBridger::new()),
            Arc::new(CoOccurrenceEnrichment::new()),
        ];

        // Three separate pipelines (one per consumer) sharing the same engine
        let mut trellis_pipeline = IngestPipeline::new(engine.clone());
        trellis_pipeline.register_integration(
            Arc::new(FragmentAdapter::new("trellis-fragment")),
            enrichments.clone(),
        );

        let mut carrel_llm_pipeline = IngestPipeline::new(engine.clone());
        carrel_llm_pipeline.register_integration(
            Arc::new(FragmentAdapter::new("carrel-llm")),
            enrichments.clone(),
        );

        let mut carrel_prov_pipeline = IngestPipeline::new(engine.clone());
        carrel_prov_pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            enrichments,
        );

        // ================================================================
        // Consumer 1: Trellis — writer's personal fragments
        // ================================================================
        // A writer exploring themes of distributed AI, creativity, and
        // governance through informal notes and observations.

        let trellis_fragments = vec![
            (
                "Distributed compute as insurance against concentration risk — \
                 the bet is that decentralization creates resilience",
                vec!["distributed-computing", "ai-safety", "economic-disruption"],
                "journal",
            ),
            (
                "Federated learning isn't just technical — it's an economic argument \
                 about who controls the training pipeline",
                vec!["federated-learning", "compute-economics", "policy"],
                "journal",
            ),
            (
                "Design constraints don't limit creativity, they channel it — \
                 the architecture of limitation as generative force",
                vec!["creativity", "design-constraints"],
                "journal",
            ),
            (
                "Non-generative AI: tools that mirror and scaffold rather than \
                 replace — the creative amplifier thesis",
                vec!["creativity", "human-ai-collaboration", "non-generative-ai"],
                "journal",
            ),
            (
                "Network effects in decentralized systems create natural resilience \
                 against single points of failure",
                vec!["network-effects", "distributed-computing", "decentralized-governance"],
                "journal",
            ),
            (
                "The governance question: who watches the autonomous agents?",
                vec!["autonomous-agents", "governance", "ai-safety"],
                "journal",
            ),
            (
                "Creative tools should make you think differently, not think less",
                vec!["creativity", "design-constraints", "human-ai-collaboration"],
                "journal",
            ),
            (
                "Economic models need to account for the externalities of AI \
                 concentration — who bears the cost when one company controls inference?",
                vec!["compute-economics", "economic-disruption", "policy"],
                "journal",
            ),
        ];

        for (text, tags, source) in &trellis_fragments {
            let input = FragmentInput::new(
                *text,
                tags.iter().map(|t| t.to_string()).collect(),
            )
            .with_source(*source);
            trellis_pipeline
                .ingest("research-workspace", "fragment", Box::new(input))
                .await
                .unwrap();
        }

        // ================================================================
        // Consumer 2: Carrel LLM — concept extraction from real arXiv papers
        // ================================================================
        // Real abstracts from public arXiv papers (2024-2025), with tags
        // representing what an LLM extractor would identify as key concepts.

        let carrel_papers = vec![
            (
                "Federated learning enables collaborative model training while \
                 preserving data privacy, yet faces practical challenges like the \
                 participation dilemma where entities may refuse to contribute or \
                 free-ride on others' efforts. This work examines incentive mechanisms \
                 in federated systems, applying concepts from economics and game theory.",
                vec!["federated-learning", "incentive-mechanisms", "game-theory", "distributed-computing"],
                "arxiv-2510.14208",  // Incentive-Based Federated Learning
            ),
            (
                "Autonomous AI agents offer significant potential while creating \
                 governance challenges. ETHOS proposes a decentralized governance model \
                 utilizing blockchain, smart contracts, and DAOs, establishing a global \
                 registry for AI agents with dynamic risk classification and automated \
                 compliance monitoring.",
                vec!["decentralized-governance", "ai-safety", "blockchain", "autonomous-agents"],
                "arxiv-2412.17114",  // Decentralized Governance of Autonomous AI Agents
            ),
            (
                "Human-AI co-creativity represents a transformative shift in how humans \
                 and generative AI tools collaborate in creative processes. Unlike earlier \
                 digital tools that mainly supported creative work, generative AI systems \
                 now actively participate, demonstrating autonomous creativity.",
                vec!["human-ai-collaboration", "creativity", "generative-ai"],
                "arxiv-2411.12527",  // Human-AI Co-Creativity
            ),
            (
                "The deployment of multiple advanced AI agents creates unprecedented \
                 multi-agent systems with novel risks. Three primary failure modes: \
                 miscoordination, conflict, and collusion — grounded in agent incentives. \
                 Seven risk factors including information asymmetries and network effects.",
                vec!["multi-agent-systems", "ai-safety", "network-effects", "governance"],
                "arxiv-2502.14143",  // Multi-Agent Risks from Advanced AI
            ),
            (
                "AI technologies are enabling hybrid relational spaces where humans and \
                 machines engage in joint creative activity — Extended Creativity systems \
                 comprising interdependent technologies, human agents, and organizational \
                 contexts. Three modes: Support, Synergy, and Symbiosis.",
                vec!["creativity", "human-ai-collaboration", "distributed-cognition"],
                "arxiv-2506.10249",  // Extended Creativity
            ),
            (
                "Federated Learning has expanded to millions of devices across various \
                 domains while providing differential privacy guarantees. Significant \
                 obstacles persist including managing training across diverse device \
                 ecosystems. Emerging challenges involve large multimodal models.",
                vec!["federated-learning", "privacy", "distributed-computing"],
                "arxiv-2410.08892",  // Federated Learning in Practice
            ),
            (
                "Embodied AI systems can exist in, learn from, reason about, and act \
                 in the physical world. They pose significant risks including physical \
                 harm, mass surveillance, and economic disruption. The importance of \
                 embodied AI will likely exacerbate concentration risks.",
                vec!["embodied-ai", "ai-safety", "economic-disruption", "policy"],
                "arxiv-2509.00117",  // Embodied AI: Risks and Policy
            ),
            (
                "This review examines AI technologies across content creation, analysis, \
                 and post-production. Transformers, LLMs, and diffusion models have \
                 established new capabilities, shifting AI from support tool to core \
                 creative technology. Human oversight remains essential for creative \
                 direction and mitigating AI hallucinations.",
                vec!["creative-industries", "generative-ai", "human-ai-collaboration"],
                "arxiv-2501.02725",  // Advances in AI for Creative Industries
            ),
        ];

        for (text, tags, source) in &carrel_papers {
            let input = FragmentInput::new(
                *text,
                tags.iter().map(|t| t.to_string()).collect(),
            )
            .with_source(*source);
            carrel_llm_pipeline
                .ingest("research-workspace", "fragment", Box::new(input))
                .await
                .unwrap();
        }

        // ================================================================
        // Consumer 3: Carrel provenance — human research annotations
        // ================================================================

        // Writing chain: "The Distributed Bet" essay draft
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::CreateChain {
                    chain_id: "the-distributed-bet".to_string(),
                    name: "The Distributed Bet".to_string(),
                    description: Some(
                        "Essay arguing distributed compute is insurance against AI concentration"
                            .to_string(),
                    ),
                }),
            )
            .await
            .unwrap();

        // Research chain: February 2026 literature scan
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::CreateChain {
                    chain_id: "lit-scan-2026-02".to_string(),
                    name: "Literature Scan Feb 2026".to_string(),
                    description: None,
                }),
            )
            .await
            .unwrap();

        // Writing mark 1: core argument about federated economics
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::AddMark {
                    mark_id: "draft-fed-econ".to_string(),
                    chain_id: "the-distributed-bet".to_string(),
                    file: "drafts/the-distributed-bet.md".to_string(),
                    line: 34,
                    annotation: "Core argument: federated learning as economic redistribution \
                                 of training compute"
                        .to_string(),
                    column: None,
                    mark_type: Some("reference".to_string()),
                    tags: Some(vec![
                        "#federated-learning".to_string(),
                        "#compute-economics".to_string(),
                    ]),
                }),
            )
            .await
            .unwrap();

        // Writing mark 2: section on governance
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::AddMark {
                    mark_id: "draft-governance".to_string(),
                    chain_id: "the-distributed-bet".to_string(),
                    file: "drafts/the-distributed-bet.md".to_string(),
                    line: 87,
                    annotation: "Governance gap: decentralized systems need decentralized oversight"
                        .to_string(),
                    column: None,
                    mark_type: Some("reference".to_string()),
                    tags: Some(vec![
                        "#decentralized-governance".to_string(),
                        "#ai-safety".to_string(),
                        "#autonomous-agents".to_string(),
                    ]),
                }),
            )
            .await
            .unwrap();

        // Writing mark 3: section on creativity
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::AddMark {
                    mark_id: "draft-creativity".to_string(),
                    chain_id: "the-distributed-bet".to_string(),
                    file: "drafts/the-distributed-bet.md".to_string(),
                    line: 142,
                    annotation: "Counterpoint: non-generative tools preserve human agency in \
                                 ways generative AI cannot"
                        .to_string(),
                    column: None,
                    mark_type: Some("reference".to_string()),
                    tags: Some(vec![
                        "#creativity".to_string(),
                        "#human-ai-collaboration".to_string(),
                        "#non-generative-ai".to_string(),
                    ]),
                }),
            )
            .await
            .unwrap();

        // Research mark 1: federated learning incentives paper
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::AddMark {
                    mark_id: "lit-fed-incentives".to_string(),
                    chain_id: "lit-scan-2026-02".to_string(),
                    file: "papers/2510.14208.pdf".to_string(),
                    line: 1,
                    annotation: "Key finding: incentive mechanisms are essential, not optional, \
                                 for federated learning — supports economic argument"
                        .to_string(),
                    column: None,
                    mark_type: Some("reference".to_string()),
                    tags: Some(vec![
                        "#federated-learning".to_string(),
                        "#incentive-mechanisms".to_string(),
                        "#compute-economics".to_string(),
                    ]),
                }),
            )
            .await
            .unwrap();

        // Research mark 2: ETHOS governance paper
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::AddMark {
                    mark_id: "lit-ethos".to_string(),
                    chain_id: "lit-scan-2026-02".to_string(),
                    file: "papers/2412.17114.pdf".to_string(),
                    line: 1,
                    annotation: "ETHOS framework: decentralized governance via blockchain + DAOs \
                                 — addresses the 'who watches' question"
                        .to_string(),
                    column: None,
                    mark_type: Some("reference".to_string()),
                    tags: Some(vec![
                        "#decentralized-governance".to_string(),
                        "#ai-safety".to_string(),
                        "#blockchain".to_string(),
                    ]),
                }),
            )
            .await
            .unwrap();

        // Research mark 3: Extended Creativity paper
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::AddMark {
                    mark_id: "lit-extended-creativity".to_string(),
                    chain_id: "lit-scan-2026-02".to_string(),
                    file: "papers/2506.10249.pdf".to_string(),
                    line: 1,
                    annotation: "Extended Creativity: Support → Synergy → Symbiosis progression \
                                 maps to non-generative thesis"
                        .to_string(),
                    column: None,
                    mark_type: Some("reference".to_string()),
                    tags: Some(vec![
                        "#creativity".to_string(),
                        "#human-ai-collaboration".to_string(),
                        "#distributed-cognition".to_string(),
                    ]),
                }),
            )
            .await
            .unwrap();

        // Link research marks to writing marks
        // Federated incentives paper supports federated economics argument
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::LinkMarks {
                    source_id: "lit-fed-incentives".to_string(),
                    target_id: "draft-fed-econ".to_string(),
                }),
            )
            .await
            .unwrap();

        // ETHOS paper supports governance section
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::LinkMarks {
                    source_id: "lit-ethos".to_string(),
                    target_id: "draft-governance".to_string(),
                }),
            )
            .await
            .unwrap();

        // Extended Creativity paper supports creativity counterpoint
        carrel_prov_pipeline
            .ingest(
                "research-workspace",
                "provenance",
                Box::new(ProvenanceInput::LinkMarks {
                    source_id: "lit-extended-creativity".to_string(),
                    target_id: "draft-creativity".to_string(),
                }),
            )
            .await
            .unwrap();

        // ================================================================
        // Analysis: Full graph topology
        // ================================================================

        let ctx = engine.get_context(&ctx_id).unwrap();

        // --- Node counts by dimension ---
        let structure_nodes: Vec<_> = ctx
            .nodes()
            .filter(|n| n.dimension == dimension::STRUCTURE)
            .collect();
        let semantic_nodes: Vec<_> = ctx
            .nodes()
            .filter(|n| n.dimension == dimension::SEMANTIC)
            .collect();
        let provenance_nodes: Vec<_> = ctx
            .nodes()
            .filter(|n| n.dimension == dimension::PROVENANCE)
            .collect();

        // 16 fragment nodes: 8 Trellis + 8 Carrel LLM
        assert_eq!(structure_nodes.len(), 16, "16 fragments (8 trellis + 8 carrel-llm)");

        // Count unique concepts
        let concept_labels: std::collections::HashSet<String> = semantic_nodes
            .iter()
            .map(|n| n.id.to_string())
            .collect();
        // Expected unique concepts across all sources:
        // distributed-computing, ai-safety, economic-disruption, federated-learning,
        // compute-economics, policy, creativity, design-constraints,
        // human-ai-collaboration, non-generative-ai, network-effects,
        // decentralized-governance, autonomous-agents, governance,
        // incentive-mechanisms, game-theory, blockchain, generative-ai,
        // creative-industries, distributed-cognition, privacy, embodied-ai,
        // multi-agent-systems
        assert_eq!(
            semantic_nodes.len(),
            23,
            "23 unique concept nodes from all consumers"
        );

        // Provenance nodes: chains + marks
        // Chains: 1 (trellis:journal) + 8 (carrel-llm per paper) + 2 (provenance: writing+research) = 11
        // Marks: 8 (trellis) + 8 (carrel-llm) + 6 (provenance: 3 writing + 3 research) = 22
        // Total provenance: 11 + 22 = 33
        let chains: Vec<_> = provenance_nodes
            .iter()
            .filter(|n| n.node_type == "chain")
            .collect();
        let marks: Vec<_> = provenance_nodes
            .iter()
            .filter(|n| n.node_type == "mark")
            .collect();
        assert_eq!(chains.len(), 11, "11 chains (1 trellis + 8 carrel-llm + 2 carrel-prov)");
        assert_eq!(marks.len(), 22, "22 marks (8 trellis + 8 carrel-llm + 6 carrel-prov)");

        let total_nodes = structure_nodes.len() + semantic_nodes.len() + provenance_nodes.len();
        // 16 + 23 + 33 = 72
        assert_eq!(total_nodes, 72, "72 total nodes across 3 dimensions");

        // --- Edge counts by type ---
        let tagged_with: Vec<_> = ctx.edges().filter(|e| e.relationship == "tagged_with").collect();
        let contains: Vec<_> = ctx.edges().filter(|e| e.relationship == "contains").collect();
        let references: Vec<_> = ctx.edges().filter(|e| e.relationship == "references").collect();
        let links_to: Vec<_> = ctx.edges().filter(|e| e.relationship == "links_to").collect();
        let may_be_related: Vec<_> = ctx.edges().filter(|e| e.relationship == "may_be_related").collect();

        // tagged_with: sum of tags per fragment
        // Trellis: 3+3+2+3+3+3+3+3 = 23
        // Carrel LLM: 4+4+3+4+3+3+4+3 = 28
        // Total: 51
        assert_eq!(tagged_with.len(), 51, "51 tagged_with edges");

        // contains: 1 per mark = 22
        // Plus provenance chain→mark: 6 from carrel-prov
        // Wait — provenance marks already counted in the 22
        // Fragment marks: 8 trellis + 8 carrel-llm = 16 contains edges
        // Provenance marks: 6 contains edges from 2 chains
        assert_eq!(contains.len(), 22, "22 contains edges (chain → mark)");

        // references: TagConceptBridger bridges marks with tags to concepts
        // Trellis marks: tag counts 3+3+2+3+3+3+3+3 = 23 references
        // Carrel LLM marks: tag counts 4+4+3+4+3+3+4+3 = 28 references
        // Carrel prov marks: tag counts 2+3+3+3+3+3 = 17 references
        // Total: 23 + 28 + 17 = 68
        assert_eq!(references.len(), 68, "68 references edges (marks → concepts)");

        // links_to: 3 research→writing links
        assert_eq!(links_to.len(), 3, "3 research→writing links");

        // may_be_related: co-occurrence enrichment (many pairs)
        assert!(
            !may_be_related.is_empty(),
            "CoOccurrenceEnrichment should detect concept pairs"
        );

        let total_edges = tagged_with.len()
            + contains.len()
            + references.len()
            + links_to.len()
            + may_be_related.len();

        // ================================================================
        // Analysis: Cross-consumer connections
        // ================================================================

        // KEY TRAVERSAL 1: Writer's intuitive "creativity" fragment → academic research
        //
        // The writer wrote: "Design constraints don't limit creativity, they channel it"
        // Can the graph connect this to formal research on Extended Creativity?
        //
        // Path: concept:creativity ← tagged_with (trellis fragment)
        //       concept:creativity ← references (carrel-llm mark for Extended Creativity)
        //       concept:creativity ← references (carrel-llm mark for Co-Creativity)
        //       concept:creativity ← references (carrel-prov mark: lit-extended-creativity)
        //       concept:creativity ← references (carrel-prov mark: draft-creativity)

        let creativity_id = NodeId::from_string("concept:creativity");
        let creativity_incoming: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == creativity_id)
            .collect();

        // tagged_with from fragments + references from marks
        // Fragments tagged "creativity": trellis 3,4,7 + carrel-llm 3,5 = 5
        // Marks referencing "creativity": trellis marks for 3,4,7 + carrel-llm marks for 3,5
        //   + carrel-prov marks: draft-creativity, lit-extended-creativity = many
        let creativity_fragment_sources: Vec<_> = creativity_incoming
            .iter()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert!(
            creativity_fragment_sources.len() >= 5,
            "at least 5 fragments tagged with creativity (3 trellis + 2 carrel-llm)"
        );

        let creativity_mark_sources: Vec<_> = creativity_incoming
            .iter()
            .filter(|e| e.relationship == "references")
            .collect();
        assert!(
            creativity_mark_sources.len() >= 5,
            "at least 5 marks reference concept:creativity"
        );

        // KEY TRAVERSAL 2: From an arXiv paper → through concepts → to writer's fragments
        //
        // Start at the ETHOS governance paper's mark, traverse to find
        // the writer's "who watches the autonomous agents?" fragment.
        //
        // Path: carrel-llm mark (ETHOS) → references → concept:autonomous-agents
        //       concept:autonomous-agents ← tagged_with ← trellis fragment #6

        // Find the carrel-llm mark for the ETHOS paper
        let ethos_chain = NodeId::from_string("chain:carrel-llm:arxiv-2412.17114");
        let ethos_marks: Vec<_> = ctx
            .edges()
            .filter(|e| e.source == ethos_chain && e.relationship == "contains")
            .collect();
        assert_eq!(ethos_marks.len(), 1, "ETHOS paper has 1 mark");

        let ethos_mark_id = &ethos_marks[0].target;

        // From the mark, follow references to concepts
        let ethos_concepts: std::collections::HashSet<String> = ctx
            .edges()
            .filter(|e| e.source == *ethos_mark_id && e.relationship == "references")
            .map(|e| e.target.to_string())
            .collect();
        assert!(
            ethos_concepts.contains("concept:autonomous-agents"),
            "ETHOS mark references concept:autonomous-agents"
        );
        assert!(
            ethos_concepts.contains("concept:ai-safety"),
            "ETHOS mark references concept:ai-safety"
        );

        // From concept:autonomous-agents, find trellis fragments
        let aa_id = NodeId::from_string("concept:autonomous-agents");
        let aa_fragments: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == aa_id && e.relationship == "tagged_with")
            .collect();
        // Trellis fragment #6 ("who watches the autonomous agents?") + ETHOS paper fragment
        assert!(
            aa_fragments.len() >= 2,
            "at least 2 fragments tagged with autonomous-agents"
        );

        // KEY TRAVERSAL 3: Cross-consumer provenance convergence
        //
        // The writer's draft annotation (draft-governance) and the research
        // annotation (lit-ethos) are explicitly linked. But they ALSO converge
        // on the same concepts via independent tagging.
        //
        // draft-governance tags: decentralized-governance, ai-safety, autonomous-agents
        // lit-ethos tags: decentralized-governance, ai-safety, blockchain
        //
        // Shared concepts: decentralized-governance, ai-safety
        // This means the graph has BOTH explicit links (links_to) AND
        // implicit concept-mediated connections.

        let draft_gov_refs: std::collections::HashSet<String> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from_string("draft-governance")
                    && e.relationship == "references"
            })
            .map(|e| e.target.to_string())
            .collect();

        let lit_ethos_refs: std::collections::HashSet<String> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from_string("lit-ethos")
                    && e.relationship == "references"
            })
            .map(|e| e.target.to_string())
            .collect();

        let shared_concepts: Vec<_> = draft_gov_refs.intersection(&lit_ethos_refs).collect();
        assert!(
            shared_concepts.len() >= 2,
            "draft-governance and lit-ethos share at least 2 concepts (decentralized-governance, ai-safety)"
        );

        // KEY TRAVERSAL 4: Full depth-2 BFS from a concept to discover
        // all three consumers' contributions
        //
        // concept:ai-safety is tagged by fragments from Trellis AND Carrel LLM
        // AND referenced by marks from Carrel provenance.
        // A depth-1 traversal from concept:ai-safety should reach all three consumer types.

        let ai_safety = NodeId::from_string("concept:ai-safety");
        let traversal = TraverseQuery::from(ai_safety.clone())
            .depth(1)
            .direction(Direction::Both)
            .execute(&ctx);

        // Level 0: concept:ai-safety
        assert_eq!(traversal.levels[0].len(), 1);

        // Level 1: fragments (via tagged_with) + marks (via references) + co-occurring concepts
        assert!(
            traversal.levels.len() >= 2,
            "traversal should reach neighbors"
        );

        let level1_dimensions: std::collections::HashSet<String> = traversal.levels[1]
            .iter()
            .map(|n| n.dimension.clone())
            .collect();

        // Should reach structure (fragments), provenance (marks), and semantic (co-occurring concepts)
        assert!(
            level1_dimensions.contains(dimension::STRUCTURE),
            "depth-1 from ai-safety reaches structure (fragments)"
        );
        assert!(
            level1_dimensions.contains(dimension::PROVENANCE),
            "depth-1 from ai-safety reaches provenance (marks)"
        );

        // Count which consumer's fragments we reach
        let level1_structure: Vec<_> = traversal.levels[1]
            .iter()
            .filter(|n| n.dimension == dimension::STRUCTURE)
            .collect();
        // Trellis fragments tagged ai-safety: #1, #6 = 2
        // Carrel LLM papers tagged ai-safety: ETHOS, Multi-Agent, Embodied AI = 3
        // Total: 5
        assert!(
            level1_structure.len() >= 5,
            "at least 5 fragments from both consumers reach ai-safety"
        );

        let level1_provenance: Vec<_> = traversal.levels[1]
            .iter()
            .filter(|n| n.dimension == dimension::PROVENANCE)
            .collect();
        // Marks referencing ai-safety: trellis marks for fragments 1,6 + carrel-llm marks
        // for ETHOS, Multi-Agent, Embodied + carrel-prov marks: draft-governance, lit-ethos
        assert!(
            level1_provenance.len() >= 5,
            "at least 5 provenance marks reference ai-safety"
        );

        // KEY TRAVERSAL 5: Depth-2 BFS from a writer's fragment through
        // provenance to discover related papers
        //
        // Trellis fragment about "who watches the autonomous agents?"
        // → its mark → references → concept:autonomous-agents
        //                         → concept:governance
        //                         → concept:ai-safety
        // → depth 2: other marks referencing those concepts
        //          → fragments tagged with those concepts
        //
        // This should discover the ETHOS paper AND the Multi-Agent Risks paper.

        // Find a specific trellis fragment: "who watches the autonomous agents?"
        let governance_fragment = structure_nodes
            .iter()
            .find(|n| {
                n.properties
                    .get("text")
                    .map(|v| {
                        matches!(v, crate::graph::PropertyValue::String(s) if s.contains("who watches"))
                    })
                    .unwrap_or(false)
            })
            .expect("should find 'who watches' fragment");

        let traversal = TraverseQuery::from(governance_fragment.id.clone())
            .depth(3)
            .direction(Direction::Both)
            .execute(&ctx);

        // Collect all nodes reached across all levels
        let all_reached: std::collections::HashSet<String> = traversal
            .levels
            .iter()
            .flatten()
            .map(|n| n.id.to_string())
            .collect();

        // Should reach concept:autonomous-agents, concept:governance, concept:ai-safety
        assert!(
            all_reached.contains("concept:autonomous-agents"),
            "traversal reaches concept:autonomous-agents"
        );
        assert!(
            all_reached.contains("concept:ai-safety"),
            "traversal reaches concept:ai-safety"
        );

        // Should reach the carrel-prov marks that share these concepts
        assert!(
            all_reached.contains("draft-governance"),
            "traversal reaches draft-governance annotation (shared concepts)"
        );

        // ================================================================
        // Analysis: Summary statistics for the essay
        // ================================================================

        // Print summary (visible in test output with --nocapture)
        eprintln!("\n=== Two-Consumer Validation Revisited: Graph Topology ===\n");
        eprintln!("Nodes: {} total", total_nodes);
        eprintln!("  Structure:  {} (fragments)", structure_nodes.len());
        eprintln!("  Semantic:   {} (concepts)", semantic_nodes.len());
        eprintln!("  Provenance: {} ({} chains + {} marks)", provenance_nodes.len(), chains.len(), marks.len());
        eprintln!("\nEdges: {} total", total_edges);
        eprintln!("  tagged_with:    {} (fragment → concept)", tagged_with.len());
        eprintln!("  contains:       {} (chain → mark)", contains.len());
        eprintln!("  references:     {} (mark → concept)", references.len());
        eprintln!("  links_to:       {} (research → writing)", links_to.len());
        eprintln!("  may_be_related: {} (concept co-occurrence)", may_be_related.len());
        eprintln!("\nConcepts: {:?}", {
            let mut labels: Vec<_> = concept_labels.iter().collect();
            labels.sort();
            labels
        });
        eprintln!("\nCross-consumer concept:creativity connections: {} fragments + {} marks",
            creativity_fragment_sources.len(), creativity_mark_sources.len());
        eprintln!("Cross-consumer concept:ai-safety depth-1 reach: {} fragments + {} marks",
            level1_structure.len(), level1_provenance.len());

        // ================================================================
        // Persistence survives restart
        // ================================================================

        drop(engine);
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();

        assert_eq!(
            ctx2.nodes().filter(|n| n.dimension == dimension::SEMANTIC).count(),
            23,
            "23 concepts survive restart"
        );
        assert_eq!(
            ctx2.nodes().filter(|n| n.dimension == dimension::STRUCTURE).count(),
            16,
            "16 fragments survive restart"
        );
        assert_eq!(
            ctx2.edges().filter(|e| e.relationship == "references").count(),
            68,
            "68 references edges survive restart"
        );
        assert_eq!(
            ctx2.edges().filter(|e| e.relationship == "tagged_with").count(),
            51,
            "51 tagged_with edges survive restart"
        );
    }

    // ================================================================
    // Spike: Creative Writing at Scale (Essay 13 part 2)
    // ================================================================
    //
    // A writer working on a novel about memory and transformation
    // in a coastal setting. 80 Trellis fragments, 15 Carrel sources
    // (previous writing, research, apocrypha), provenance annotations.
    //
    // Source material includes real Heraclitus fragments, Bachelard,
    // Ovid, and cognitive science themes.
    //
    // Key question: can the graph surface thematic clusters that
    // map to potential narrative threads or story outlines?

    #[tokio::test]
    async fn spike_creative_writing_at_scale() {
        use crate::adapter::ingest::IngestPipeline;
        use crate::adapter::provenance_adapter::{ProvenanceAdapter, ProvenanceInput};
        use crate::adapter::tag_bridger::TagConceptBridger;
        use crate::graph::dimension;
        use crate::query::{Direction, TraverseQuery};

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("coastal-novel");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "coastal-novel"))
            .unwrap();

        let enrichments: Vec<Arc<dyn crate::adapter::enrichment::Enrichment>> = vec![
            Arc::new(TagConceptBridger::new()),
            Arc::new(CoOccurrenceEnrichment::new()),
        ];

        let mut trellis = IngestPipeline::new(engine.clone());
        trellis.register_integration(
            Arc::new(FragmentAdapter::new("trellis")),
            enrichments.clone(),
        );

        let mut carrel_llm = IngestPipeline::new(engine.clone());
        carrel_llm.register_integration(
            Arc::new(FragmentAdapter::new("carrel-llm")),
            enrichments.clone(),
        );

        let mut carrel_prov = IngestPipeline::new(engine.clone());
        carrel_prov.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            enrichments,
        );

        // ================================================================
        // Consumer 1: Trellis — 80 writer's fragments
        // ================================================================
        // Organized by thematic cluster with deliberate cross-cluster
        // tag overlaps that should produce interesting connections.

        // (text, tags)
        let fragments: Vec<(&str, Vec<&str>)> = vec![
            // --- Memory / Time cluster ---
            ("The smell of salt air triggers memories I didn't know I had", vec!["memory", "salt", "senses"]),
            ("Time moves differently here — not forward but in circles, returning to the same shore", vec!["time", "return", "water"]),
            ("She kept every letter but couldn't remember writing them", vec!["memory", "letters", "forgetting"]),
            ("The photograph captures a moment that never existed the way I remember it", vec!["memory", "photography", "truth"]),
            ("Nostalgia is not remembering the past — it's mourning a version of yourself", vec!["nostalgia", "identity", "loss"]),
            ("The old songs carry the weight of every time they've been sung", vec!["memory", "repetition", "time"]),
            ("He said forgetting was a kindness but she knew it was theft", vec!["forgetting", "loss", "silence"]),
            ("The tide pools remember the ocean even when the water retreats", vec!["memory", "tides", "water"]),
            ("Anniversaries are rituals for trapping time in place", vec!["time", "ritual", "memory"]),
            ("I found my childhood in a box of someone else's photographs", vec!["memory", "photography", "identity"]),

            // --- Water / Ocean cluster ---
            ("The harbor at dawn — every boat a sentence waiting to become a story", vec!["water", "dawn", "narrative"]),
            ("Storms teach you what the architecture was designed to withstand", vec!["storms", "architecture", "resilience"]),
            ("Salt preserves everything except the living", vec!["salt", "preservation", "decay"]),
            ("The current pulls memory loose like kelp from rock", vec!["water", "memory", "loss"]),
            ("She learned to read the weather the way others read faces", vec!["water", "reading", "knowing"]),
            ("The lighthouse keeper's log: an inventory of what darkness required", vec!["light", "darkness", "naming"]),
            ("Tides erase and rewrite the same shore endlessly", vec!["tides", "writing", "repetition"]),
            ("After the flood the waterline was a new kind of border", vec!["water", "thresholds", "transformation"]),
            ("The drowned village is still visible at low tide — a city of ghosts", vec!["water", "ruins", "ghosts"]),
            ("Sailing is the practice of trusting what you cannot see", vec!["water", "trust", "navigation"]),

            // --- Identity / Transformation cluster ---
            ("She wore names like masks, each one a different version of arriving", vec!["identity", "masks", "naming"]),
            ("Transformation requires first the dissolution of what came before", vec!["transformation", "decay", "becoming"]),
            ("The ship of Theseus applies to people too — when are you someone else?", vec!["identity", "transformation", "myth"]),
            ("Mirrors show you someone who is always just leaving", vec!["identity", "seeing", "departure"]),
            ("He became the role so completely that the original was an understudy", vec!["identity", "masks", "performance"]),
            ("Twins are nature's argument that identity is not located in the body", vec!["identity", "doubling", "body"]),
            ("Immigration is a form of translation — you become a different word for the same meaning", vec!["identity", "translation", "journey"]),
            ("Every metamorphosis carries grief for the earlier form", vec!["transformation", "loss", "myth"]),
            ("The mask doesn't hide the face — it reveals a different one", vec!["masks", "truth", "seeing"]),
            ("You can return to a place but never to the person you were when you left it", vec!["return", "identity", "time"]),

            // --- Architecture / Ruins cluster ---
            ("The house remembers what the family forgot", vec!["architecture", "memory", "family"]),
            ("Ruins are more honest than restoration — they admit to time", vec!["ruins", "time", "truth"]),
            ("A threshold is where one story ends and another begins", vec!["thresholds", "narrative", "architecture"]),
            ("The cathedral's silence is not empty — it's full of accumulated prayers", vec!["architecture", "silence", "accumulation"]),
            ("She built rooms inside her mind and furnished them with light", vec!["architecture", "mind", "light"]),
            ("Decay is architecture learning to let go", vec!["decay", "architecture", "loss"]),
            ("The foundation stone holds the memory of every wall that rose above it", vec!["architecture", "foundation", "memory"]),
            ("Windows are the building's way of looking at itself", vec!["architecture", "seeing", "reflection"]),
            ("Abandoned buildings dream of the lives they contained", vec!["ruins", "memory", "ghosts"]),
            ("The staircase is a negotiation between staying and leaving", vec!["architecture", "thresholds", "departure"]),

            // --- Light / Shadow cluster ---
            ("Photography is memory's most beautiful lie", vec!["photography", "memory", "truth"]),
            ("Shadows are more faithful than light — they never leave you", vec!["shadow", "faithfulness", "light"]),
            ("Dawn reveals what darkness kindly hid", vec!["dawn", "darkness", "revelation"]),
            ("The darkroom is where images learn to exist", vec!["photography", "darkness", "becoming"]),
            ("She painted only at twilight when things revealed their true colors", vec!["light", "truth", "painting"]),
            ("Every photograph is an argument about what mattered", vec!["photography", "narrative", "seeing"]),
            ("The shadow of a lighthouse reaches further than its beam", vec!["shadow", "light", "reach"]),
            ("Candlelight turns faces into landscapes", vec!["light", "faces", "transformation"]),
            ("The negative contains everything the photograph chose to forget", vec!["photography", "forgetting", "shadow"]),
            ("At certain angles the ruin is more beautiful than the building ever was", vec!["light", "ruins", "beauty"]),

            // --- Family / Inheritance cluster ---
            ("We inherit gestures before we inherit property", vec!["inheritance", "family", "body"]),
            ("The family tree is a map of silences", vec!["family", "silence", "naming"]),
            ("Her mother's recipes were coded messages from another life", vec!["family", "language", "secrets"]),
            ("To inherit a house is to inherit its arguments", vec!["inheritance", "architecture", "family"]),
            ("Secrets are the family's most durable architecture", vec!["secrets", "family", "architecture"]),
            ("She found her grandmother's handwriting in her own", vec!["inheritance", "writing", "identity"]),
            ("The attic held everything too important to use and too painful to discard", vec!["family", "memory", "architecture"]),
            ("Photographs of the dead become relics of secular saints", vec!["photography", "family", "myth"]),

            // --- Language / Silence cluster ---
            ("Naming a thing is the first step toward losing it", vec!["naming", "loss", "language"]),
            ("Translation is the art of mourning what won't cross the border", vec!["translation", "loss", "journey"]),
            ("Silence is not the absence of language — it's language's shadow", vec!["silence", "language", "shadow"]),
            ("The untranslatable word is the culture's most private room", vec!["translation", "language", "architecture"]),
            ("She spoke three languages and was homesick in all of them", vec!["language", "identity", "loss"]),
            ("Letters are conversations with ghosts", vec!["letters", "ghosts", "language"]),
            ("The word for home changes depending on how far away you are", vec!["language", "home", "distance"]),
            ("His accent was a map of everywhere he'd been and couldn't return to", vec!["language", "journey", "return"]),

            // --- Myth / Journey cluster ---
            ("Odysseus's real journey was learning to forget the sea", vec!["myth", "journey", "forgetting"]),
            ("Every labyrinth has a center that is also a mirror", vec!["labyrinth", "identity", "myth"]),
            ("Proteus changes form because truth requires multiplicity", vec!["myth", "transformation", "truth"]),
            ("The hero's return is always to a place that no longer exists", vec!["myth", "return", "loss"]),
            ("Myths are the dreams a culture agrees to share", vec!["myth", "dreams", "narrative"]),
            ("The underworld is just memory without light", vec!["myth", "memory", "darkness"]),
            ("She carried a red thread through the labyrinth of her own making", vec!["labyrinth", "navigation", "identity"]),
            ("Persephone chose both kingdoms — that was her real power", vec!["myth", "choice", "transformation"]),

            // --- Cross-cluster bridges (deliberate thematic connections) ---
            ("The sea wall is memory's architecture against forgetting", vec!["water", "architecture", "memory", "forgetting"]),
            ("The lighthouse keeper keeps a journal no one will read — all naming is faith", vec!["light", "naming", "faith", "silence"]),
            ("Storm damage reveals the house's skeleton, its secret geometry", vec!["storms", "architecture", "secrets", "revelation"]),
            ("She translated the poem but lost the silence between the words", vec!["translation", "silence", "loss", "language"]),
            ("The photograph of the ruin creates a double memory — of the building and its decay", vec!["photography", "ruins", "memory", "doubling"]),
            ("The returning tide brings different debris — same ocean, different story", vec!["tides", "return", "narrative", "water"]),
            ("The house by the harbor: where architecture meets the sea", vec!["architecture", "water", "home", "thresholds"]),
            ("Old maps name places that no longer exist — cartography of loss", vec!["naming", "loss", "navigation", "time"]),
        ];

        assert_eq!(fragments.len(), 82, "82 trellis fragments");

        for (text, tags) in &fragments {
            let input = FragmentInput::new(
                *text,
                tags.iter().map(|t| t.to_string()).collect(),
            )
            .with_source("journal");
            trellis
                .ingest("coastal-novel", "fragment", Box::new(input))
                .await
                .unwrap();
        }

        // ================================================================
        // Consumer 2: Carrel LLM — 15 sources (previous writing,
        // research, apocrypha)
        // ================================================================

        // (text, tags, source)
        let carrel_sources: Vec<(&str, Vec<&str>, &str)> = vec![
            // --- Writer's previous work ---
            (
                "How coastlines erode: first the soft parts, then the foundations. \
                 Memory works the same way — the feelings go first, then the facts, \
                 until only the shape of what happened remains.",
                vec!["memory", "erosion", "landscape", "time"],
                "own-essay-coastal-erosion",
            ),
            (
                "The translator lived in a house between languages. Every room held \
                 a different grammar. The kitchen spoke her mother tongue; the study \
                 whispered in the language of her adopted country.",
                vec!["translation", "identity", "architecture", "language"],
                "own-story-translators-house",
            ),
            (
                "I photograph ruins because they have stopped pretending. A ruin is a \
                 building that has finally told the truth about time.",
                vec!["photography", "ruins", "decay", "truth"],
                "own-essay-why-i-photograph-ruins",
            ),
            (
                "The tide comes in like memory — unbidden, carrying debris from depths \
                 you'd forgotten. The tide goes out like forgetting — slowly, leaving \
                 behind what it thinks you need.",
                vec!["tides", "memory", "forgetting", "ritual"],
                "own-poem-tidal-memory",
            ),

            // --- Research papers ---
            (
                "Memory consolidation transforms labile short-term traces into stable \
                 long-term representations. Reconsolidation suggests that each retrieval \
                 opens a window during which memories can be modified or strengthened.",
                vec!["memory", "transformation", "time", "consolidation"],
                "paper-memory-consolidation",
            ),
            (
                "Narrative identity is the internalized, evolving life story that \
                 integrates reconstructed past and imagined future to provide life \
                 with unity and purpose.",
                vec!["narrative", "identity", "memory", "self"],
                "paper-narrative-identity",
            ),
            (
                "The method of loci exploits spatial memory to organize information \
                 in imagined architectural spaces — memory palaces where each room \
                 holds a different piece of knowledge.",
                vec!["architecture", "memory", "navigation", "mind"],
                "paper-memory-palaces",
            ),
            (
                "Metaphors are not merely linguistic ornaments but fundamental \
                 cognitive structures. We understand abstract concepts through \
                 embodied experience: time as a river, arguments as buildings, \
                 emotions as weather.",
                vec!["language", "body", "metaphor", "knowing"],
                "paper-embodied-cognition",
            ),

            // --- Apocrypha ---
            (
                "On those stepping into rivers staying the same, other and other \
                 waters flow. All things are in flux; nothing stays still.",
                vec!["water", "identity", "time", "transformation"],
                "heraclitus-fragments",
            ),
            (
                "The house shelters daydreaming, the house protects the dreamer, \
                 the house allows one to dream in peace. Inhabited space transcends \
                 geometrical space.",
                vec!["architecture", "memory", "home", "dreams"],
                "bachelard-poetics-of-space",
            ),
            (
                "Proteus, the old man of the sea, knows all things — past, present, \
                 and future. But to extract his knowledge you must hold him fast \
                 through every transformation.",
                vec!["myth", "transformation", "truth", "water"],
                "ovid-metamorphoses-proteus",
            ),
            (
                "What can be seen at one time is no more than the island which has \
                 risen into view. The library contains all possible combinations of \
                 letters — somewhere in it exists every story ever told and never told.",
                vec!["labyrinth", "language", "naming", "infinity"],
                "borges-library-of-babel",
            ),
            (
                "Mono no aware: the pathos of things. The gentle sadness at the \
                 passing of all things. Cherry blossoms are beautiful because they fall.",
                vec!["beauty", "loss", "time", "seeing"],
                "japanese-mono-no-aware",
            ),
            (
                "Ship figureheads carried the vessel's soul. To name a ship was to \
                 give it a fate. Sailors who renamed a vessel first had to erase \
                 every trace of the old name — even from the ship's log.",
                vec!["water", "naming", "myth", "navigation"],
                "maritime-folklore-figureheads",
            ),
            (
                "A palimpsest: a manuscript on which earlier writing has been effaced \
                 to make room for later writing, but traces of the original remain \
                 visible. Every old building is a palimpsest of renovations.",
                vec!["architecture", "time", "layers", "memory"],
                "architectural-palimpsest",
            ),
        ];

        for (text, tags, source) in &carrel_sources {
            let input = FragmentInput::new(
                *text,
                tags.iter().map(|t| t.to_string()).collect(),
            )
            .with_source(*source);
            carrel_llm
                .ingest("coastal-novel", "fragment", Box::new(input))
                .await
                .unwrap();
        }

        // ================================================================
        // Consumer 3: Carrel provenance — annotations on sources
        // ================================================================

        // Novel drafting chain
        carrel_prov
            .ingest(
                "coastal-novel",
                "provenance",
                Box::new(ProvenanceInput::CreateChain {
                    chain_id: "novel-draft".to_string(),
                    name: "Coastal Novel Draft".to_string(),
                    description: Some("Novel about memory and transformation in a coastal town".to_string()),
                }),
            )
            .await
            .unwrap();

        // Research chain
        carrel_prov
            .ingest(
                "coastal-novel",
                "provenance",
                Box::new(ProvenanceInput::CreateChain {
                    chain_id: "thematic-research".to_string(),
                    name: "Thematic Research".to_string(),
                    description: None,
                }),
            )
            .await
            .unwrap();

        // Draft marks: annotations on novel sections
        let draft_marks = vec![
            ("draft-ch1-harbor", "novel-draft", "drafts/chapter-1.md", 1,
             "Opening: protagonist returns to the harbor town — everything familiar but changed",
             vec!["#return", "#water", "#identity", "#transformation"]),
            ("draft-ch3-house", "novel-draft", "drafts/chapter-3.md", 1,
             "The inherited house as repository of family memory — each room a different era",
             vec!["#architecture", "#memory", "#family", "#inheritance"]),
            ("draft-ch5-photographs", "novel-draft", "drafts/chapter-5.md", 1,
             "Box of old photographs: protagonist discovers images that contradict her memories",
             vec!["#photography", "#memory", "#truth", "#family"]),
            ("draft-ch7-storm", "novel-draft", "drafts/chapter-7.md", 1,
             "Storm sequence: the sea reveals foundations of a building no one remembers",
             vec!["#storms", "#ruins", "#memory", "#revelation"]),
            ("draft-ch9-translation", "novel-draft", "drafts/chapter-9.md", 1,
             "The translator character: caught between languages, belonging to neither fully",
             vec!["#translation", "#identity", "#language", "#loss"]),
            ("draft-ch11-labyrinth", "novel-draft", "drafts/chapter-11.md", 1,
             "Climax: the protagonist navigates a labyrinth of memory to find what was hidden",
             vec!["#labyrinth", "#memory", "#navigation", "#secrets"]),
        ];

        for (mark_id, chain_id, file, line, annotation, tags) in &draft_marks {
            carrel_prov
                .ingest(
                    "coastal-novel",
                    "provenance",
                    Box::new(ProvenanceInput::AddMark {
                        mark_id: mark_id.to_string(),
                        chain_id: chain_id.to_string(),
                        file: file.to_string(),
                        line: *line,
                        annotation: annotation.to_string(),
                        column: None,
                        mark_type: Some("reference".to_string()),
                        tags: Some(tags.iter().map(|t| t.to_string()).collect()),
                    }),
                )
                .await
                .unwrap();
        }

        // Research marks: annotations on sources
        let research_marks = vec![
            ("research-heraclitus", "thematic-research", "sources/heraclitus.md", 91,
             "Fragment 91: the river metaphor — directly applicable to protagonist's return",
             vec!["#water", "#identity", "#transformation"]),
            ("research-bachelard", "thematic-research", "sources/bachelard.md", 7,
             "The house as dreaming space — the inherited house chapters need this",
             vec!["#architecture", "#memory", "#dreams"]),
            ("research-proteus", "thematic-research", "sources/ovid-proteus.md", 1,
             "Proteus: knowledge requires holding fast through transformation — the protagonist's arc",
             vec!["#myth", "#transformation", "#truth"]),
            ("research-memory-paper", "thematic-research", "sources/memory-consolidation.pdf", 1,
             "Reconsolidation: memories change when retrieved — the unreliable photographs chapter",
             vec!["#memory", "#transformation", "#truth"]),
            ("research-palimpsest", "thematic-research", "sources/palimpsest.md", 1,
             "The house as palimpsest: layers of renovation = layers of family history",
             vec!["#architecture", "#time", "#layers", "#family"]),
        ];

        for (mark_id, chain_id, file, line, annotation, tags) in &research_marks {
            carrel_prov
                .ingest(
                    "coastal-novel",
                    "provenance",
                    Box::new(ProvenanceInput::AddMark {
                        mark_id: mark_id.to_string(),
                        chain_id: chain_id.to_string(),
                        file: file.to_string(),
                        line: *line,
                        annotation: annotation.to_string(),
                        column: None,
                        mark_type: Some("reference".to_string()),
                        tags: Some(tags.iter().map(|t| t.to_string()).collect()),
                    }),
                )
                .await
                .unwrap();
        }

        // Links: research sources → draft chapters they inform
        let links = vec![
            ("research-heraclitus", "draft-ch1-harbor"),    // river metaphor → return chapter
            ("research-bachelard", "draft-ch3-house"),       // poetics of space → house chapter
            ("research-proteus", "draft-ch11-labyrinth"),    // transformation → climax
            ("research-memory-paper", "draft-ch5-photographs"), // reconsolidation → photographs
            ("research-palimpsest", "draft-ch3-house"),      // palimpsest → house chapter
        ];

        for (source, target) in &links {
            carrel_prov
                .ingest(
                    "coastal-novel",
                    "provenance",
                    Box::new(ProvenanceInput::LinkMarks {
                        source_id: source.to_string(),
                        target_id: target.to_string(),
                    }),
                )
                .await
                .unwrap();
        }

        // ================================================================
        // Analysis
        // ================================================================

        let ctx = engine.get_context(&ctx_id).unwrap();

        // --- Node counts ---
        let structure_count = ctx.nodes().filter(|n| n.dimension == dimension::STRUCTURE).count();
        let semantic_nodes: Vec<_> = ctx.nodes().filter(|n| n.dimension == dimension::SEMANTIC).collect();
        let provenance_count = ctx.nodes().filter(|n| n.dimension == dimension::PROVENANCE).count();
        let chains_count = ctx.nodes().filter(|n| n.node_type == "chain").count();
        let marks_count = ctx.nodes().filter(|n| n.node_type == "mark").count();

        // 97 fragments: 82 Trellis + 15 Carrel LLM
        assert_eq!(structure_count, 97, "97 fragment nodes (82 trellis + 15 carrel-llm)");

        // Count unique tags across all fragments
        let concept_ids: std::collections::BTreeSet<String> = semantic_nodes
            .iter()
            .map(|n| n.id.to_string())
            .collect();

        // Provenance: chains + marks
        // Chains: 1 (trellis:journal) + 15 (carrel-llm per source) + 2 (prov: novel-draft + thematic-research) = 18
        // Marks: 82 (trellis) + 15 (carrel-llm) + 11 (prov: 6 draft + 5 research) = 108
        assert_eq!(chains_count, 18, "18 chains");
        assert_eq!(marks_count, 108, "108 marks");

        let total_nodes = structure_count + semantic_nodes.len() + provenance_count;

        // --- Edge counts ---
        let tagged_with_count = ctx.edges().filter(|e| e.relationship == "tagged_with").count();
        let contains_count = ctx.edges().filter(|e| e.relationship == "contains").count();
        let references_count = ctx.edges().filter(|e| e.relationship == "references").count();
        let links_to_count = ctx.edges().filter(|e| e.relationship == "links_to").count();
        let may_be_related: Vec<_> = ctx.edges().filter(|e| e.relationship == "may_be_related").collect();

        assert_eq!(contains_count, 108, "108 contains edges (1 per mark)");
        assert_eq!(links_to_count, 5, "5 research→draft links");

        let total_edges = tagged_with_count + contains_count + references_count
            + links_to_count + may_be_related.len();

        // ================================================================
        // Thematic cluster analysis via co-occurrence
        // ================================================================

        // Find the strongest co-occurrence pairs (highest raw_weight)
        let mut cooccurrence_pairs: Vec<(String, String, f32)> = may_be_related
            .iter()
            .filter(|e| e.source.to_string() < e.target.to_string()) // deduplicate symmetric pairs
            .map(|e| (e.source.to_string(), e.target.to_string(), e.raw_weight))
            .collect();
        cooccurrence_pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        // The top co-occurrence pairs should reveal thematic clusters.
        // memory + time, memory + identity, water + memory, etc.
        assert!(
            !cooccurrence_pairs.is_empty(),
            "should have co-occurrence pairs"
        );

        // --- Cross-cluster discovery ---

        // TRAVERSAL 1: From Heraclitus → through concepts → to writer's fragments
        // Heraclitus has tags: water, identity, time, transformation
        // These bridge the Water, Identity, and Memory clusters.
        let heraclitus_chain = NodeId::from_string("chain:carrel-llm:heraclitus-fragments");
        let heraclitus_mark_edges: Vec<_> = ctx
            .edges()
            .filter(|e| e.source == heraclitus_chain && e.relationship == "contains")
            .collect();
        assert_eq!(heraclitus_mark_edges.len(), 1);

        let heraclitus_mark = &heraclitus_mark_edges[0].target;
        let heraclitus_concepts: std::collections::HashSet<String> = ctx
            .edges()
            .filter(|e| e.source == *heraclitus_mark && e.relationship == "references")
            .map(|e| e.target.to_string())
            .collect();

        assert!(heraclitus_concepts.contains("concept:water"));
        assert!(heraclitus_concepts.contains("concept:identity"));
        assert!(heraclitus_concepts.contains("concept:transformation"));
        assert!(heraclitus_concepts.contains("concept:time"));

        // From concept:transformation, how many fragments from different consumers?
        let transformation_id = NodeId::from_string("concept:transformation");
        let transformation_fragments: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == transformation_id && e.relationship == "tagged_with")
            .collect();
        // Trellis fragments with "transformation" + Carrel sources with "transformation"
        assert!(
            transformation_fragments.len() >= 5,
            "concept:transformation should connect fragments from both consumers"
        );

        // TRAVERSAL 2: From the protagonist's return (Ch1) through concepts
        // to discover the Heraclitus connection
        //
        // draft-ch1-harbor tags: return, water, identity, transformation
        // Heraclitus tags: water, identity, time, transformation
        // Shared concepts: water, identity, transformation
        let ch1_refs: std::collections::HashSet<String> = ctx
            .edges()
            .filter(|e| {
                e.source == NodeId::from_string("draft-ch1-harbor")
                    && e.relationship == "references"
            })
            .map(|e| e.target.to_string())
            .collect();

        let shared_with_heraclitus: Vec<_> = ch1_refs
            .intersection(&heraclitus_concepts)
            .collect();
        assert!(
            shared_with_heraclitus.len() >= 3,
            "draft-ch1-harbor and Heraclitus share at least 3 concepts (water, identity, transformation)"
        );

        // TRAVERSAL 3: concept:memory as the central hub
        // Memory should be the most connected concept — it appears across
        // nearly every cluster and consumer.
        let memory_id = NodeId::from_string("concept:memory");
        let memory_tagged_with = ctx
            .edges()
            .filter(|e| e.target == memory_id && e.relationship == "tagged_with")
            .count();
        let memory_references = ctx
            .edges()
            .filter(|e| e.target == memory_id && e.relationship == "references")
            .count();
        let memory_cooccurrences = ctx
            .edges()
            .filter(|e| {
                e.relationship == "may_be_related"
                    && (e.source == memory_id || e.target == memory_id)
            })
            .count();

        // Memory should be heavily connected across all relationship types
        assert!(memory_tagged_with >= 15, "memory tagged in many fragments");
        assert!(memory_references >= 15, "memory referenced by many marks");
        assert!(memory_cooccurrences >= 10, "memory co-occurs with many other concepts");

        // TRAVERSAL 4: depth-2 from concept:architecture should reach
        // concept:memory (via co-occurrence or shared fragments), creating
        // the "house as memory" thematic thread
        let arch_id = NodeId::from_string("concept:architecture");
        let arch_traversal = TraverseQuery::from(arch_id.clone())
            .depth(2)
            .direction(Direction::Both)
            .execute(&ctx);

        let arch_reached: std::collections::HashSet<String> = arch_traversal
            .levels
            .iter()
            .flatten()
            .map(|n| n.id.to_string())
            .collect();

        // Architecture should reach memory through shared fragments and co-occurrence
        assert!(
            arch_reached.contains("concept:memory"),
            "architecture reaches memory (the house-as-memory theme)"
        );

        // TRAVERSAL 5: From Borges' Library of Babel → through labyrinth →
        // to the writer's myth fragments
        let borges_chain = NodeId::from_string("chain:carrel-llm:borges-library-of-babel");
        let borges_marks: Vec<_> = ctx
            .edges()
            .filter(|e| e.source == borges_chain && e.relationship == "contains")
            .collect();
        assert_eq!(borges_marks.len(), 1);

        let borges_concepts: std::collections::HashSet<String> = ctx
            .edges()
            .filter(|e| e.source == borges_marks[0].target.clone() && e.relationship == "references")
            .map(|e| e.target.to_string())
            .collect();
        assert!(borges_concepts.contains("concept:labyrinth"));
        assert!(borges_concepts.contains("concept:naming"));

        // From concept:labyrinth, reach the myth cluster
        let lab_id = NodeId::from_string("concept:labyrinth");
        let labyrinth_fragments = ctx
            .edges()
            .filter(|e| e.target == lab_id && e.relationship == "tagged_with")
            .count();
        assert!(
            labyrinth_fragments >= 3,
            "labyrinth connects Borges to writer's myth fragments"
        );

        // ================================================================
        // Narrative thread analysis: what story outline does the graph suggest?
        // ================================================================

        // Collect the top N co-occurrence pairs as potential narrative threads
        let top_pairs: Vec<_> = cooccurrence_pairs.iter().take(20).collect();

        // Identify hub concepts (appear in most co-occurrence pairs)
        let mut concept_frequency: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (src, tgt, _) in &cooccurrence_pairs {
            *concept_frequency.entry(src.clone()).or_default() += 1;
            *concept_frequency.entry(tgt.clone()).or_default() += 1;
        }
        let mut hub_concepts: Vec<_> = concept_frequency.into_iter().collect();
        hub_concepts.sort_by(|a, b| b.1.cmp(&a.1));

        // ================================================================
        // Summary output
        // ================================================================

        eprintln!("\n=== Creative Writing at Scale: Graph Topology ===\n");
        eprintln!("Nodes: {} total", total_nodes);
        eprintln!("  Structure:  {} (82 trellis + 15 carrel-llm)", structure_count);
        eprintln!("  Semantic:   {} concepts", semantic_nodes.len());
        eprintln!("  Provenance: {} ({} chains + {} marks)", provenance_count, chains_count, marks_count);
        eprintln!("\nEdges: {} total", total_edges);
        eprintln!("  tagged_with:    {}", tagged_with_count);
        eprintln!("  contains:       {}", contains_count);
        eprintln!("  references:     {}", references_count);
        eprintln!("  links_to:       {}", links_to_count);
        eprintln!("  may_be_related: {} ({} unique pairs)",
            may_be_related.len(), cooccurrence_pairs.len());

        eprintln!("\n--- Concept vocabulary ({} concepts) ---", concept_ids.len());
        for id in &concept_ids {
            eprintln!("  {}", id);
        }

        eprintln!("\n--- Top 20 co-occurrence pairs (narrative threads) ---");
        for (src, tgt, weight) in &top_pairs {
            eprintln!("  {:.1}  {} ↔ {}", weight,
                src.strip_prefix("concept:").unwrap_or(src),
                tgt.strip_prefix("concept:").unwrap_or(tgt));
        }

        eprintln!("\n--- Hub concepts (most connected) ---");
        for (concept, freq) in hub_concepts.iter().take(10) {
            eprintln!("  {} — {} co-occurrence connections",
                concept.strip_prefix("concept:").unwrap_or(concept), freq);
        }

        eprintln!("\n--- Cross-consumer discovery ---");
        eprintln!("  Heraclitus → concept:transformation → {} trellis+carrel fragments",
            transformation_fragments.len());
        eprintln!("  concept:memory hub: {} tagged_with + {} references + {} co-occurrences",
            memory_tagged_with, memory_references, memory_cooccurrences);
        eprintln!("  Borges → concept:labyrinth → {} myth/journey fragments",
            labyrinth_fragments);

        // ================================================================
        // Persistence
        // ================================================================

        drop(engine);
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();

        assert_eq!(
            ctx2.nodes().filter(|n| n.dimension == dimension::STRUCTURE).count(),
            97,
            "97 fragments survive restart"
        );
        assert_eq!(
            ctx2.nodes().filter(|n| n.dimension == dimension::SEMANTIC).count(),
            semantic_nodes.len(),
            "concepts survive restart"
        );
    }

    // ================================================================
    // Embedding Enrichment Integration (ADR-026)
    // ================================================================

    use crate::adapter::embedding::{Embedder, EmbeddingError, EmbeddingSimilarityEnrichment};

    /// Mock embedder for integration testing — returns predetermined vectors.
    ///
    /// Uses the same test vectors as the unit tests in `embedding.rs`:
    /// travel=[0.9,0.3,0.1], journey=[0.85,0.35,0.15],
    /// voyage=[0.88,0.32,0.12], democracy=[0.1,0.2,0.95].
    struct MockEmbedder {
        vectors: std::collections::HashMap<String, Vec<f32>>,
    }

    impl MockEmbedder {
        fn with_test_vectors() -> Self {
            let mut vectors = std::collections::HashMap::new();
            vectors.insert("travel".to_string(), vec![0.9, 0.3, 0.1]);
            vectors.insert("journey".to_string(), vec![0.85, 0.35, 0.15]);
            vectors.insert("voyage".to_string(), vec![0.88, 0.32, 0.12]);
            vectors.insert("democracy".to_string(), vec![0.1, 0.2, 0.95]);
            Self { vectors }
        }
    }

    impl Embedder for MockEmbedder {
        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            let mut results = Vec::new();
            for text in texts {
                let vec = self
                    .vectors
                    .get(*text)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; 3]);
                results.push(vec);
            }
            Ok(results)
        }
    }

    // === Scenario: Embedding enrichment fires alongside existing enrichments in pipeline ===
    #[tokio::test]
    async fn embedding_enrichment_fires_alongside_existing_enrichments() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("embedding-integration");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "embedding-integration"))
            .unwrap();

        // Pipeline with FragmentAdapter + 3 enrichments:
        // TagConceptBridger, CoOccurrenceEnrichment, EmbeddingSimilarityEnrichment
        let fragment_adapter = Arc::new(FragmentAdapter::new("test-fragment"));
        let embedding_enrichment = Arc::new(EmbeddingSimilarityEnrichment::new(
            "mock-model",
            0.7,
            "similar_to",
            Box::new(MockEmbedder::with_test_vectors()),
        ));

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            fragment_adapter,
            vec![
                Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
                Arc::new(CoOccurrenceEnrichment::new()) as Arc<dyn Enrichment>,
                embedding_enrichment as Arc<dyn Enrichment>,
            ],
        );

        // Fragment 1: tags=["travel", "voyage"]
        // Creates concept:travel and concept:voyage → embedding enrichment caches both,
        // finds travel↔voyage similarity (cosine ~0.99), emits similar_to edges
        pipeline
            .ingest(
                "embedding-integration",
                "fragment",
                Box::new(FragmentInput::new(
                    "Sailing voyage across the Mediterranean",
                    vec!["travel".to_string(), "voyage".to_string()],
                )),
            )
            .await
            .unwrap();

        // Fragment 2: tags=["journey", "democracy"]
        // Creates concept:journey and concept:democracy
        // journey is similar to cached travel/voyage; democracy is not
        pipeline
            .ingest(
                "embedding-integration",
                "fragment",
                Box::new(FragmentInput::new(
                    "Journey through democratic institutions",
                    vec!["journey".to_string(), "democracy".to_string()],
                )),
            )
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();

        // --- Assert: similar_to edges exist for the travel cluster ---
        let similar_to_edges: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "similar_to")
            .collect();

        // travel↔voyage (symmetric pair) = 2 edges from fragment 1
        // journey↔travel + journey↔voyage (symmetric pairs) = 4 edges from fragment 2
        // Total: ≥4 similar_to edges (at least the 4 from cross-fragment similarity)
        assert!(
            similar_to_edges.len() >= 4,
            "expected ≥4 similar_to edges, got {}",
            similar_to_edges.len()
        );

        // --- Assert: democracy has NO similar_to edges ---
        let democracy_id = NodeId::from_string("concept:democracy");
        let democracy_similar: Vec<_> = similar_to_edges
            .iter()
            .filter(|e| e.source == democracy_id || e.target == democracy_id)
            .collect();
        assert!(
            democracy_similar.is_empty(),
            "democracy should have no similar_to edges, got {}",
            democracy_similar.len()
        );

        // --- Assert: similar_to edges are in semantic dimension with positive weight ---
        for edge in &similar_to_edges {
            assert_eq!(
                edge.source_dimension, dimension::SEMANTIC,
                "similar_to edge source should be in semantic dimension"
            );
            assert_eq!(
                edge.target_dimension, dimension::SEMANTIC,
                "similar_to edge target should be in semantic dimension"
            );
            assert!(
                edge.raw_weight > 0.0,
                "similar_to edge should have positive weight, got {}",
                edge.raw_weight
            );
        }

        // --- Assert: CoOccurrenceEnrichment also fired (may_be_related edges exist) ---
        let may_be_related: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "may_be_related")
            .collect();
        assert!(
            !may_be_related.is_empty(),
            "CoOccurrenceEnrichment should have produced may_be_related edges"
        );

        // --- Assert: TagConceptBridger also fired (references edges exist) ---
        let references: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "references")
            .collect();
        assert!(
            !references.is_empty(),
            "TagConceptBridger should have produced references edges"
        );

        // --- Assert: provenance traversal works ---
        // concept:journey → incoming references → mark → incoming contains → chain
        let journey_id = NodeId::from_string("concept:journey");
        let journey_refs: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == journey_id && e.relationship == "references")
            .collect();
        assert!(
            !journey_refs.is_empty(),
            "concept:journey should have incoming references edges from marks"
        );

        // Follow from mark to chain via contains
        let mark_id = &journey_refs[0].source;
        let contains_edges: Vec<_> = ctx
            .edges()
            .filter(|e| e.target == *mark_id && e.relationship == "contains")
            .collect();
        assert!(
            !contains_edges.is_empty(),
            "mark should have incoming contains edge from chain"
        );

        // The chain node should exist
        let chain_id = &contains_edges[0].source;
        assert!(
            ctx.get_node(chain_id).is_some(),
            "chain node should exist at end of provenance traversal"
        );
    }

    // === Scenario: Different embedding models produce separate contribution slots ===
    #[tokio::test]
    async fn multi_model_contribution_slots_on_same_edge() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("multi-model");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "multi-model"))
            .unwrap();

        // Model A: uses the standard test vectors
        let enrichment_a = Arc::new(EmbeddingSimilarityEnrichment::new(
            "model-a",
            0.7,
            "similar_to",
            Box::new(MockEmbedder::with_test_vectors()),
        ));

        // Model B: uses slightly different vectors (still similar travel/journey)
        let mut vectors_b = std::collections::HashMap::new();
        vectors_b.insert("travel".to_string(), vec![0.85, 0.35, 0.15]);
        vectors_b.insert("journey".to_string(), vec![0.9, 0.3, 0.1]);
        vectors_b.insert("democracy".to_string(), vec![0.1, 0.2, 0.95]);
        let enrichment_b = Arc::new(EmbeddingSimilarityEnrichment::new(
            "model-b",
            0.7,
            "similar_to",
            Box::new(MockEmbedder { vectors: vectors_b }),
        ));

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            Arc::new(FragmentAdapter::new("multi-model")),
            vec![
                Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
                enrichment_a as Arc<dyn Enrichment>,
                enrichment_b as Arc<dyn Enrichment>,
            ],
        );

        // Ingest: travel and journey appear together → both models fire
        pipeline
            .ingest(
                "multi-model",
                "fragment",
                Box::new(FragmentInput::new(
                    "A journey through travel",
                    vec!["travel".to_string(), "journey".to_string()],
                )),
            )
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();

        // Find a similar_to edge (either direction)
        let similar_to: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "similar_to")
            .collect();
        assert!(
            !similar_to.is_empty(),
            "should have similar_to edges from both models"
        );

        // Check contribution slots on one of the edges
        let edge = &similar_to[0];
        assert!(
            edge.contributions.contains_key("embedding:model-a"),
            "edge should have contribution from model-a, got: {:?}",
            edge.contributions.keys().collect::<Vec<_>>()
        );
        assert!(
            edge.contributions.contains_key("embedding:model-b"),
            "edge should have contribution from model-b, got: {:?}",
            edge.contributions.keys().collect::<Vec<_>>()
        );

        // Both contribution values should be positive
        let val_a = edge.contributions["embedding:model-a"];
        let val_b = edge.contributions["embedding:model-b"];
        assert!(val_a > 0.0, "model-a contribution should be positive");
        assert!(val_b > 0.0, "model-b contribution should be positive");
    }

    // === Scenario: Embedding enrichment triggers discovery gap detection ===
    #[tokio::test]
    async fn embedding_triggers_discovery_gap_detection() {
        use crate::adapter::discovery_gap::DiscoveryGapEnrichment;

        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("embed-gap");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "embed-gap"))
            .unwrap();

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            Arc::new(FragmentAdapter::new("embed-gap")),
            vec![
                Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
                Arc::new(EmbeddingSimilarityEnrichment::new(
                    "mock-model",
                    0.7,
                    "similar_to",
                    Box::new(MockEmbedder::with_test_vectors()),
                )) as Arc<dyn Enrichment>,
                Arc::new(DiscoveryGapEnrichment::new("similar_to", "discovery_gap"))
                    as Arc<dyn Enrichment>,
            ],
        );

        // Fragment 1: travel only — caches embedding
        pipeline
            .ingest(
                "embed-gap",
                "fragment",
                Box::new(FragmentInput::new(
                    "Planning a trip",
                    vec!["travel".to_string()],
                )),
            )
            .await
            .unwrap();

        // Fragment 2: journey only — structurally disconnected from travel
        // (different fragments, no shared tags → no co-occurrence)
        // Embedding enrichment finds journey↔travel similar, emits similar_to
        // DiscoveryGapEnrichment detects: similar but no structural evidence → gap
        pipeline
            .ingest(
                "embed-gap",
                "fragment",
                Box::new(FragmentInput::new(
                    "A long journey",
                    vec!["journey".to_string()],
                )),
            )
            .await
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();

        // similar_to edges should exist
        let similar_to: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "similar_to")
            .collect();
        assert!(
            !similar_to.is_empty(),
            "embedding enrichment should have produced similar_to edges"
        );

        // discovery_gap edges should exist between travel and journey
        let discovery_gaps: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "discovery_gap")
            .collect();
        assert!(
            !discovery_gaps.is_empty(),
            "DiscoveryGapEnrichment should have fired after similar_to edges were added"
        );

        // Verify the gap is between travel and journey
        let travel = NodeId::from_string("concept:travel");
        let journey = NodeId::from_string("concept:journey");
        let gap_involves_travel_journey = discovery_gaps.iter().any(|e| {
            (e.source == travel && e.target == journey)
                || (e.source == journey && e.target == travel)
        });
        assert!(
            gap_involves_travel_journey,
            "discovery_gap should connect travel and journey"
        );
    }

    // === Scenario: Discovery gap cleanup after embedding retraction ===
    #[tokio::test]
    async fn discovery_gap_cleanup_after_retraction() {
        use crate::adapter::discovery_gap::DiscoveryGapEnrichment;

        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("retract-gap");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "retract-gap"))
            .unwrap();

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_integration(
            Arc::new(FragmentAdapter::new("retract-gap")),
            vec![
                Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
                Arc::new(EmbeddingSimilarityEnrichment::new(
                    "mock-model",
                    0.7,
                    "similar_to",
                    Box::new(MockEmbedder::with_test_vectors()),
                )) as Arc<dyn Enrichment>,
                Arc::new(DiscoveryGapEnrichment::new("similar_to", "discovery_gap"))
                    as Arc<dyn Enrichment>,
            ],
        );

        // Ingest two structurally disconnected concepts
        pipeline
            .ingest(
                "retract-gap",
                "fragment",
                Box::new(FragmentInput::new(
                    "Planning a trip",
                    vec!["travel".to_string()],
                )),
            )
            .await
            .unwrap();
        pipeline
            .ingest(
                "retract-gap",
                "fragment",
                Box::new(FragmentInput::new(
                    "A long journey",
                    vec!["journey".to_string()],
                )),
            )
            .await
            .unwrap();

        // Verify pre-conditions: similar_to and discovery_gap edges exist
        {
            let ctx = engine.get_context(&ctx_id).unwrap();
            assert!(
                ctx.edges().any(|e| e.relationship == "similar_to"),
                "pre-condition: similar_to edges should exist"
            );
            assert!(
                ctx.edges().any(|e| e.relationship == "discovery_gap"),
                "pre-condition: discovery_gap edges should exist"
            );
        }

        // Retract the embedding model's contributions
        engine
            .retract_contributions(&ctx_id, "embedding:mock-model")
            .unwrap();

        // After retraction: similar_to edges should be pruned (sole contributor removed)
        let ctx = engine.get_context(&ctx_id).unwrap();
        let similar_to_after: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "similar_to")
            .collect();
        assert!(
            similar_to_after.is_empty(),
            "similar_to edges should be pruned after retracting their only contributor"
        );

        // discovery_gap edges should also be pruned (their sole contributor was the gap enrichment,
        // but with the trigger edges gone, retraction of the gap enrichment's contributions
        // removes them too)
        // Note: the gap enrichment contributed these edges, so retract its contributions as well
        engine
            .retract_contributions(
                &ctx_id,
                "discovery_gap:similar_to:discovery_gap",
            )
            .unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();
        let gaps_after: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "discovery_gap")
            .collect();
        assert!(
            gaps_after.is_empty(),
            "discovery_gap edges should be pruned after retracting gap enrichment contributions"
        );
    }

    // === Scenario: Model replacement workflow ===
    #[tokio::test]
    async fn model_replacement_retract_v1_ingest_v2() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = ContextId::from("model-replace");
        engine
            .upsert_context(Context::with_id(ctx_id.clone(), "model-replace"))
            .unwrap();

        // Phase 1: Ingest with model-v1
        let enrichment_v1 = Arc::new(EmbeddingSimilarityEnrichment::new(
            "model-v1",
            0.7,
            "similar_to",
            Box::new(MockEmbedder::with_test_vectors()),
        ));

        let mut pipeline_v1 = IngestPipeline::new(engine.clone());
        pipeline_v1.register_integration(
            Arc::new(FragmentAdapter::new("model-replace")),
            vec![
                Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
                enrichment_v1 as Arc<dyn Enrichment>,
            ],
        );

        pipeline_v1
            .ingest(
                "model-replace",
                "fragment",
                Box::new(FragmentInput::new(
                    "Sailing voyage",
                    vec!["travel".to_string(), "voyage".to_string()],
                )),
            )
            .await
            .unwrap();

        // Verify v1 edges exist
        {
            let ctx = engine.get_context(&ctx_id).unwrap();
            let v1_edges: Vec<_> = ctx
                .edges()
                .filter(|e| {
                    e.relationship == "similar_to"
                        && e.contributions.contains_key("embedding:model-v1")
                })
                .collect();
            assert!(!v1_edges.is_empty(), "v1 should have produced similar_to edges");
        }

        // Phase 2: Retract model-v1
        engine
            .retract_contributions(&ctx_id, "embedding:model-v1")
            .unwrap();

        // Verify v1 edges are gone
        {
            let ctx = engine.get_context(&ctx_id).unwrap();
            let similar_after: Vec<_> = ctx
                .edges()
                .filter(|e| e.relationship == "similar_to")
                .collect();
            assert!(
                similar_after.is_empty(),
                "similar_to edges should be pruned after v1 retraction"
            );
        }

        // Phase 3: Re-ingest with model-v2 (different vectors, slightly different similarities)
        let mut vectors_v2 = std::collections::HashMap::new();
        vectors_v2.insert("travel".to_string(), vec![0.85, 0.4, 0.05]);
        vectors_v2.insert("voyage".to_string(), vec![0.82, 0.38, 0.08]);
        let enrichment_v2 = Arc::new(EmbeddingSimilarityEnrichment::new(
            "model-v2",
            0.7,
            "similar_to",
            Box::new(MockEmbedder { vectors: vectors_v2 }),
        ));

        let mut pipeline_v2 = IngestPipeline::new(engine.clone());
        pipeline_v2.register_integration(
            Arc::new(FragmentAdapter::new("model-replace")),
            vec![
                Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
                enrichment_v2 as Arc<dyn Enrichment>,
            ],
        );

        // Re-ingest the same content — nodes already exist, embedding enrichment
        // fires on the NodesAdded events, caches with v2 vectors
        pipeline_v2
            .ingest(
                "model-replace",
                "fragment",
                Box::new(FragmentInput::new(
                    "Sailing voyage",
                    vec!["travel".to_string(), "voyage".to_string()],
                )),
            )
            .await
            .unwrap();

        // Verify: only v2 contributions remain
        let ctx = engine.get_context(&ctx_id).unwrap();
        let similar_to: Vec<_> = ctx
            .edges()
            .filter(|e| e.relationship == "similar_to")
            .collect();
        assert!(
            !similar_to.is_empty(),
            "v2 should have produced similar_to edges"
        );

        for edge in &similar_to {
            assert!(
                !edge.contributions.contains_key("embedding:model-v1"),
                "no v1 slots should remain"
            );
            assert!(
                edge.contributions.contains_key("embedding:model-v2"),
                "edge should have v2 contribution slot"
            );
        }
    }

    // ================================================================
    // Integration Debt: ADR-003/005 — Normalization Pipeline End-to-End
    // ================================================================

    // === Scenario: Normalized weight is correct after full persist/reload pipeline ===
    #[tokio::test]
    async fn normalization_pipeline_end_to_end_through_persistence() {
        use crate::query::{OutgoingDivisive, NormalizationStrategy};

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        let ctx_id = ContextId::from("norm-pipeline-test");
        engine.upsert_context(Context::with_id(ctx_id.clone(), "norm-pipeline-test")).unwrap();

        // Pre-populate nodes
        {
            let mut ctx = engine.get_context(&ctx_id).unwrap();
            ctx.add_node(node("A"));
            ctx.add_node(node("B"));
            ctx.add_node(node("C"));
            engine.upsert_context(ctx).unwrap();
        }

        // Emit two edges from a real adapter with different raw weights
        let sink = make_engine_sink(&engine, &ctx_id, "adapter-1");
        let mut e1 = edge("A", "B");
        e1.raw_weight = 5.0;
        let mut e2 = edge("A", "C");
        e2.raw_weight = 10.0;
        sink.emit(Emission::new().with_edge(e1).with_edge(e2)).await.unwrap();

        // Capture in-memory query-time normalized weights
        let ctx = engine.get_context(&ctx_id).unwrap();
        let strategy = OutgoingDivisive;
        let in_memory = strategy.normalize(&NodeId::from_string("A"), &ctx);
        let mem_ab = in_memory.iter().find(|ne| ne.edge.target == NodeId::from_string("B")).unwrap().normalized_weight;
        let mem_ac = in_memory.iter().find(|ne| ne.edge.target == NodeId::from_string("C")).unwrap().normalized_weight;

        // Restart from storage
        drop(engine);
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();

        // Query-time normalization on reloaded data — no manual recompute_raw_weights() call
        let reloaded = strategy.normalize(&NodeId::from_string("A"), &ctx2);
        let reload_ab = reloaded.iter().find(|ne| ne.edge.target == NodeId::from_string("B")).unwrap().normalized_weight;
        let reload_ac = reloaded.iter().find(|ne| ne.edge.target == NodeId::from_string("C")).unwrap().normalized_weight;

        assert!(
            (mem_ab - reload_ab).abs() < 1e-6,
            "A→B normalized weight should match after reload: {} vs {}", mem_ab, reload_ab
        );
        assert!(
            (mem_ac - reload_ac).abs() < 1e-6,
            "A→C normalized weight should match after reload: {} vs {}", mem_ac, reload_ac
        );

        // Verify the values are actually what divisive normalization should produce
        // raw_weight values are scale-normalized by emit_inner, so we check the ratio
        let sum = reload_ab + reload_ac;
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "divisive normalized weights should sum to 1.0, got {}", sum
        );
    }

    // ================================================================
    // Integration Debt: ADR-009/015 — Tag Bridging with # Prefix
    // ================================================================

    // === Scenario: annotate() with #-prefixed tags bridges correctly ===
    #[tokio::test]
    async fn annotate_with_hash_prefixed_tags_bridges_to_concepts() {
        use crate::adapter::ingest::IngestPipeline;
        use crate::adapter::provenance_adapter::ProvenanceAdapter;
        use crate::adapter::tag_bridger::TagConceptBridger;
        use crate::api::PlexusApi;

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));
        engine.upsert_context(Context::new("research")).unwrap();

        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(Arc::new(FragmentAdapter::new("annotate")));
        pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            vec![Arc::new(TagConceptBridger::new())],
        );
        let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

        // Call annotate with #-prefixed tags — exercises the # stripping in api.rs:77
        api.annotate(
            "research", "notes", "src/main.rs", 1, "travel observations",
            None, None, Some(vec!["#travel".into(), "#avignon".into()]),
        ).await.unwrap();

        // Resolve context by name (api.resolve is private, so use engine directly)
        let ctx_id = engine.list_contexts().into_iter()
            .find(|id| engine.get_context(id).map(|c| c.name == "research").unwrap_or(false))
            .expect("research context should exist");
        let ctx = engine.get_context(&ctx_id).unwrap();

        // FragmentAdapter should create concepts WITHOUT the # prefix
        assert!(
            ctx.get_node(&NodeId::from("concept:travel")).is_some(),
            "concept:travel should exist (# stripped before FragmentAdapter)"
        );
        assert!(
            ctx.get_node(&NodeId::from("concept:avignon")).is_some(),
            "concept:avignon should exist (# stripped before FragmentAdapter)"
        );
        // Concepts with # should NOT exist
        assert!(
            ctx.get_node(&NodeId::from("concept:#travel")).is_none(),
            "concept:#travel should NOT exist"
        );

        // annotate() creates two marks: one from FragmentAdapter (provenance for the
        // fragment) and one from ProvenanceAdapter (the user's annotation mark).
        // Both should exist; TagConceptBridger should bridge tags from both to concepts.
        let marks: Vec<_> = ctx.nodes()
            .filter(|n| n.node_type == "mark")
            .collect();
        assert_eq!(marks.len(), 2, "should have 2 mark nodes (fragment + provenance)");

        // The provenance mark (from ProvenanceAdapter) has the #-prefixed tags.
        // TagConceptBridger strips # and bridges to concepts created by FragmentAdapter.
        // Verify references edges exist from marks to concept nodes.
        let all_refs: Vec<_> = ctx.edges()
            .filter(|e| e.relationship == "references")
            .collect();
        // Both marks should bridge to concepts (FragmentAdapter mark has lowercased tags,
        // ProvenanceAdapter mark has #-prefixed tags — both should bridge)
        let ref_targets: std::collections::HashSet<String> = all_refs.iter()
            .map(|e| e.target.to_string()).collect();
        assert!(ref_targets.contains("concept:travel"), "should reference concept:travel");
        assert!(ref_targets.contains("concept:avignon"), "should reference concept:avignon");

        // Verify persistence round-trip
        drop(engine);
        let engine2 = PlexusEngine::with_store(store);
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&ctx_id).unwrap();

        let refs2: Vec<_> = ctx2.edges()
            .filter(|e| e.relationship == "references")
            .collect();
        assert!(refs2.len() >= 2, "references edges should survive persistence, got {}", refs2.len());
        let targets2: std::collections::HashSet<String> = refs2.iter()
            .map(|e| e.target.to_string()).collect();
        assert!(targets2.contains("concept:travel"), "concept:travel ref should survive persistence");
        assert!(targets2.contains("concept:avignon"), "concept:avignon ref should survive persistence");
    }
}
