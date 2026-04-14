//! Contribution contract acceptance tests.
//!
//! Scenarios:
//! - Edges carry per-adapter contribution keys after ingest
//! - Retracting an adapter's contributions removes its edges

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::NodeId;

#[tokio::test]
async fn edges_carry_per_adapter_contribution_keys() {
    let env = TestEnv::new();
    let input = FragmentInput::new("Rust ownership makes memory safety explicit", vec![
        "rust".into(),
        "memory-safety".into(),
    ]);

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    let tagged_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    assert!(
        !tagged_edges.is_empty(),
        "ingest with tags should produce tagged_with edges"
    );

    for edge in &tagged_edges {
        assert!(
            edge.contributions.contains_key("content"),
            "tagged_with edge {:?} → {:?} should carry a contribution key for 'content', got: {:?}",
            edge.source,
            edge.target,
            edge.contributions.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            edge.contributions.get("content"),
            Some(&1.0),
            "contribution weight for 'content' should be 1.0"
        );
    }

    // Verify the expected concept nodes exist
    assert!(
        ctx.get_node(&NodeId::from_string("concept:rust")).is_some(),
        "concept:rust should exist"
    );
    assert!(
        ctx.get_node(&NodeId::from_string("concept:memory-safety")).is_some(),
        "concept:memory-safety should exist"
    );
}

#[tokio::test]
async fn retract_contributions_removes_adapter_edges() {
    let env = TestEnv::new();
    let input = FragmentInput::new("Distributed systems require careful coordination", vec![
        "distributed-systems".into(),
        "coordination".into(),
    ]);

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Verify tagged_with edges exist before retraction
    let ctx_before = env.engine.get_context(&env.context_id).expect("context exists");
    let tagged_before: Vec<_> = ctx_before.edges.iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    assert!(
        !tagged_before.is_empty(),
        "tagged_with edges should exist before retraction"
    );

    // All of them should have a "content" contribution
    for edge in &tagged_before {
        assert!(
            edge.contributions.contains_key("content"),
            "edge should have 'content' contribution before retraction"
        );
    }

    // Retract — takes context_name, not context_id
    let affected = env.api
        .retract_contributions(&env.context_name, "content")
        .expect("retract_contributions should succeed");

    assert!(
        affected > 0,
        "retraction should report at least one affected edge, got {}",
        affected
    );

    // After retraction, tagged_with edges from the "content" adapter should be gone
    let ctx_after = env.engine.get_context(&env.context_id).expect("context exists");
    let tagged_after: Vec<_> = ctx_after.edges.iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    assert!(
        tagged_after.is_empty(),
        "tagged_with edges should be removed after retracting 'content' contributions, \
         found {} edge(s) remaining",
        tagged_after.len()
    );

    // No edge should carry the "content" contribution key
    for edge in &ctx_after.edges {
        assert!(
            !edge.contributions.contains_key("content"),
            "no edge should retain a 'content' contribution after retraction"
        );
    }
}
