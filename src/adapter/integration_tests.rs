//! Integration tests for cancellation, progressive emission, and end-to-end adapter scenarios

#[cfg(test)]
mod tests {
    use crate::adapter::cancel::CancellationToken;
    use crate::adapter::cooccurrence::CoOccurrenceAdapter;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::enrichment::{Enrichment, EnrichmentRegistry};
    use crate::adapter::events::GraphEvent;
    use crate::adapter::fragment::{FragmentAdapter, FragmentInput};
    use crate::adapter::proposal_sink::ProposalSink;
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

    // ================================================================
    // End-to-End: Provence Travel Research
    // ================================================================

    // === Scenario: Full workflow from ingestion through marking to query ===
    #[tokio::test]
    async fn end_to_end_provence_travel_research() {
        use crate::provenance::api::ProvenanceApi;
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

        // Step 2: Create provenance chain and add mark with tags
        let api = ProvenanceApi::new(&engine, ctx_id.clone());
        let chain_id = api.create_chain("reading-notes", None).unwrap();
        let mark_id = api.add_mark(
            &chain_id, "notes.md", 10, "walking through Avignon",
            None, None, Some(vec!["#travel".into(), "#avignon".into()]),
        ).unwrap();

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
}
