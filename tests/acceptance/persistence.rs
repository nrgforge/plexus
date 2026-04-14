//! Persistence contract acceptance tests.
//!
//! Scenarios:
//! - Nodes and edges survive engine reload from SQLite store
//! - Edge weights and contributions survive reload

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::{NodeId, PlexusEngine};
use std::sync::Arc;

#[tokio::test]
async fn nodes_and_edges_survive_engine_reload() {
    let env = TestEnv::new();

    let input = FragmentInput::new(
        "Rust is a systems programming language",
        vec!["rust".into(), "systems".into()],
    );

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Verify the nodes exist before reload
    let ctx_before = env.engine.get_context(&env.context_id).expect("context exists");
    assert!(
        ctx_before.get_node(&NodeId::from_string("concept:rust")).is_some(),
        "concept:rust node should exist before reload"
    );
    assert!(
        ctx_before.get_node(&NodeId::from_string("concept:systems")).is_some(),
        "concept:systems node should exist before reload"
    );

    // Create a second engine over the same store — simulating a process restart
    let engine2 = Arc::new(PlexusEngine::with_store(env.store.clone()));
    engine2.load_all().expect("load_all should succeed");

    let ctx2 = engine2
        .get_context(&env.context_id)
        .expect("context should be present after reload");

    assert!(
        ctx2.get_node(&NodeId::from_string("concept:rust")).is_some(),
        "concept:rust node should survive engine reload"
    );
    assert!(
        ctx2.get_node(&NodeId::from_string("concept:systems")).is_some(),
        "concept:systems node should survive engine reload"
    );

    // tagged_with edges from the fragment node to each concept should also survive
    let tagged_edges: Vec<_> = ctx2
        .edges
        .iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    assert!(
        tagged_edges.len() >= 2,
        "at least 2 tagged_with edges should survive reload (one per tag), got {}",
        tagged_edges.len()
    );

    let has_rust_edge = tagged_edges
        .iter()
        .any(|e| e.target == NodeId::from_string("concept:rust"));
    let has_systems_edge = tagged_edges
        .iter()
        .any(|e| e.target == NodeId::from_string("concept:systems"));

    assert!(has_rust_edge, "tagged_with → concept:rust edge should survive reload");
    assert!(has_systems_edge, "tagged_with → concept:systems edge should survive reload");
}

#[tokio::test]
async fn edge_weights_survive_reload() {
    let env = TestEnv::new();

    let input = FragmentInput::new(
        "Persistence and durability are key database properties",
        vec!["persistence".into(), "durability".into()],
    );

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Capture the tagged_with edge weights from the live engine
    let ctx_before = env.engine.get_context(&env.context_id).expect("context exists");
    let edges_before: Vec<_> = ctx_before
        .edges
        .iter()
        .filter(|e| e.relationship == "tagged_with")
        .cloned()
        .collect();

    assert!(
        !edges_before.is_empty(),
        "tagged_with edges should exist before reload"
    );

    // Create a second engine over the same store — simulating a process restart
    let engine2 = Arc::new(PlexusEngine::with_store(env.store.clone()));
    engine2.load_all().expect("load_all should succeed");

    let ctx2 = engine2
        .get_context(&env.context_id)
        .expect("context should be present after reload");

    let edges_after: Vec<_> = ctx2
        .edges
        .iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    assert_eq!(
        edges_before.len(),
        edges_after.len(),
        "same number of tagged_with edges should survive reload"
    );

    // Each reloaded edge should have a matching combined_weight and contribution
    // from the "content" adapter (adapter ID assigned by PipelineBuilder::with_default_adapters)
    for edge in &edges_after {
        assert!(
            (edge.combined_weight - 1.0_f32).abs() < 1e-6,
            "tagged_with edge combined_weight should be 1.0 after reload, got {}",
            edge.combined_weight
        );

        assert_eq!(
            edge.contributions.get("content"),
            Some(&1.0_f32),
            "tagged_with edge should carry 'content' adapter contribution of 1.0 after reload"
        );
    }
}
