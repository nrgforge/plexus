//! Integration tests for cancellation, progressive emission, and end-to-end adapter scenarios

#[cfg(test)]
mod tests {
    use crate::adapter::cancel::CancellationToken;
    use crate::adapter::cooccurrence::CoOccurrenceAdapter;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::events::GraphEvent;
    use crate::adapter::fragment::{FragmentAdapter, FragmentInput};
    use crate::adapter::proposal_sink::ProposalSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::adapter::sink::{AdapterError, AdapterSink};
    use crate::adapter::traits::{Adapter, AdapterInput};
    use crate::adapter::types::Emission;
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
    // End-to-End: FragmentAdapter + CoOccurrenceAdapter
    // ================================================================

    // === Scenario: Three fragments produce tagged_with and may_be_related edges ===
    #[tokio::test]
    async fn three_fragments_end_to_end() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Step 1: FragmentAdapter processes three fragments
        let fragment_adapter = FragmentAdapter::new("manual-fragment");
        let fragment_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "manual-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });

        let fragments = vec![
            FragmentInput::new("F1", vec!["travel".into(), "avignon".into(), "walking".into()]),
            FragmentInput::new("F2", vec!["travel".into(), "avignon".into(), "paris".into()]),
            FragmentInput::new("F3", vec!["walking".into(), "nature".into()]),
        ];

        for fragment in fragments {
            let input = AdapterInput::new("fragment", fragment, "test");
            fragment_adapter.process(&input, &fragment_sink).await.unwrap();
        }

        // Verify fragment structure before co-occurrence
        {
            let c = ctx.lock().unwrap();
            // 3 fragment nodes + 5 concept nodes (travel, avignon, walking, paris, nature)
            assert_eq!(c.node_count(), 8, "expected 3 fragments + 5 concepts = 8 nodes");

            // 8 tagged_with edges (F1: 3 tags, F2: 3 tags, F3: 2 tags)
            let tagged_with: Vec<_> = c.edges.iter()
                .filter(|e| e.relationship == "tagged_with")
                .collect();
            assert_eq!(tagged_with.len(), 8, "expected 3+3+2=8 tagged_with edges");

            // All tagged_with edges from manual-fragment
            for edge in &tagged_with {
                assert_eq!(edge.contributions.get("manual-fragment"), Some(&1.0));
            }
        }

        // Step 2: CoOccurrenceAdapter processes snapshot
        let snapshot = ctx.lock().unwrap().clone();
        let cooccurrence = CoOccurrenceAdapter::new("co-occurrence");
        let co_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "co-occurrence".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let proposal_sink = ProposalSink::new(co_sink, 1.0);

        let co_input = AdapterInput::new("graph_state", snapshot, "test");
        cooccurrence.process(&co_input, &proposal_sink).await.unwrap();

        // Verify end-to-end graph
        let c = ctx.lock().unwrap();

        // may_be_related edges exist
        let may_be_related: Vec<_> = c.edges.iter()
            .filter(|e| e.relationship == "may_be_related")
            .collect();
        assert!(may_be_related.len() > 0, "should have may_be_related edges");

        // All may_be_related are symmetric pairs (even count)
        assert_eq!(may_be_related.len() % 2, 0, "may_be_related should come in symmetric pairs");

        // All may_be_related edges from co-occurrence
        for edge in &may_be_related {
            assert!(edge.contributions.contains_key("co-occurrence"),
                "may_be_related edge should have co-occurrence contribution");
        }

        // travel ↔ avignon should have highest score (2 shared fragments)
        let travel_id = NodeId::from_string("concept:travel");
        let avignon_id = NodeId::from_string("concept:avignon");
        let ta = may_be_related.iter().find(|e| {
            e.source == travel_id && e.target == avignon_id
        }).expect("travel→avignon should exist");

        let ta_score = *ta.contributions.get("co-occurrence").unwrap();

        // travel ↔ paris has 1 shared fragment (F2 only), so lower score
        let paris_id = NodeId::from_string("concept:paris");
        let tp = may_be_related.iter().find(|e| {
            e.source == travel_id && e.target == paris_id
        }).expect("travel→paris should exist");

        let tp_score = *tp.contributions.get("co-occurrence").unwrap();
        assert!(ta_score > tp_score,
            "travel↔avignon ({}) should score higher than travel↔paris ({})", ta_score, tp_score);

        // tagged_with edges still have manual-fragment contributions
        let tagged_with: Vec<_> = c.edges.iter()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_with.len(), 8);
        for edge in &tagged_with {
            assert_eq!(edge.contributions.get("manual-fragment"), Some(&1.0));
        }
    }

    // === Scenario: Re-running CoOccurrenceAdapter with unchanged graph is idempotent ===
    #[tokio::test]
    async fn cooccurrence_rerun_is_idempotent() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Setup: process one fragment
        let fragment_adapter = FragmentAdapter::new("manual-fragment");
        let fragment_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "manual-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });

        let input = AdapterInput::new(
            "fragment",
            FragmentInput::new("F1", vec!["travel".into(), "avignon".into()]),
            "test",
        );
        fragment_adapter.process(&input, &fragment_sink).await.unwrap();

        // First co-occurrence run
        let snapshot1 = ctx.lock().unwrap().clone();
        let cooccurrence = CoOccurrenceAdapter::new("co-occurrence");
        let co_sink1 = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "co-occurrence".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let proposal_sink1 = ProposalSink::new(co_sink1, 1.0);
        let co_input1 = AdapterInput::new("graph_state", snapshot1, "test");
        cooccurrence.process(&co_input1, &proposal_sink1).await.unwrap();

        // Capture state after first run
        let edges_after_first: Vec<_> = {
            let c = ctx.lock().unwrap();
            c.edges.iter()
                .filter(|e| e.relationship == "may_be_related")
                .map(|e| (e.source.clone(), e.target.clone(), *e.contributions.get("co-occurrence").unwrap()))
                .collect()
        };

        // Second co-occurrence run (same graph, no new fragments)
        let snapshot2 = ctx.lock().unwrap().clone();
        let co_sink2 = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "co-occurrence".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let proposal_sink2 = ProposalSink::new(co_sink2, 1.0);
        let co_input2 = AdapterInput::new("graph_state", snapshot2, "test");
        cooccurrence.process(&co_input2, &proposal_sink2).await.unwrap();

        // Verify: same contributions, no duplication
        let c = ctx.lock().unwrap();
        let edges_after_second: Vec<_> = c.edges.iter()
            .filter(|e| e.relationship == "may_be_related")
            .map(|e| (e.source.clone(), e.target.clone(), *e.contributions.get("co-occurrence").unwrap()))
            .collect();

        // Same number of may_be_related edges
        assert_eq!(edges_after_first.len(), edges_after_second.len(),
            "re-run should not create duplicate may_be_related edges");

        // Same contribution values (latest-value-replace with same values)
        for (first, second) in edges_after_first.iter().zip(edges_after_second.iter()) {
            assert_eq!(first.0, second.0, "edge source should match");
            assert_eq!(first.1, second.1, "edge target should match");
            assert!((first.2 - second.2).abs() < 1e-6,
                "contribution should be unchanged: {} vs {}", first.2, second.2);
        }
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
}
