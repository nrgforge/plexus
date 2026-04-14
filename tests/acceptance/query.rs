//! Query contract acceptance tests.
//!
//! Scenarios:
//! - FindQuery returns nodes matching type filter
//! - TraverseQuery follows edges from a starting node
//! - Evidence trail returns provenance for a concept

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::{FindQuery, NodeId, TraverseQuery, evidence_trail};

// ---------------------------------------------------------------------------
// Scenario 1: FindQuery returns nodes matching type filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn find_query_returns_nodes_matching_type() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Rust is a systems programming language",
        vec!["rust".into(), "programming".into()],
    );

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    // FindQuery executed directly against the context
    let result = FindQuery::new()
        .with_node_type("concept")
        .execute(&ctx);

    // ContentAdapter creates one concept node per tag
    assert!(
        result.nodes.len() >= 2,
        "should find at least 2 concept nodes (one per tag), found {}",
        result.nodes.len()
    );
    assert_eq!(
        result.total_count,
        result.nodes.len(),
        "total_count should match nodes returned when no limit is applied"
    );

    // Every returned node must have node_type "concept"
    for node in &result.nodes {
        assert_eq!(
            node.node_type, "concept",
            "FindQuery with_node_type(\"concept\") returned a non-concept node: {:?}",
            node.id
        );
    }

    // The specific concepts produced by the tags must be present
    let ids: Vec<String> = result.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(
        ids.contains(&"concept:rust".to_string()),
        "concept:rust should be in find results"
    );
    assert!(
        ids.contains(&"concept:programming".to_string()),
        "concept:programming should be in find results"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: TraverseQuery follows edges from a starting node
// ---------------------------------------------------------------------------

#[tokio::test]
async fn traverse_query_follows_edges() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Knowledge graphs connect ideas",
        vec!["knowledge-graph".into(), "ideas".into()],
    );

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    // concept:knowledge-graph has tagged_with edges pointing to it from the
    // fragment node (fragment → concept via tagged_with, outgoing from fragment).
    // Traversing from concept:knowledge-graph in the Incoming direction should
    // reach the fragment that tagged it.
    let start_id = NodeId::from_string("concept:knowledge-graph");
    assert!(
        ctx.get_node(&start_id).is_some(),
        "concept:knowledge-graph must exist before traversal"
    );

    // Filter to tagged_with edges only so the co-occurrence enrichment edges
    // (may_be_related between sibling concepts) do not interfere.
    let result = TraverseQuery::from(start_id.clone())
        .depth(1)
        .direction(plexus::Direction::Incoming)
        .with_relationship("tagged_with")
        .execute(&ctx);

    // Level 0 is always the origin node
    assert!(
        !result.levels.is_empty(),
        "traversal result should have at least the origin level"
    );
    assert_eq!(
        result.levels[0][0].id, start_id,
        "level 0 should contain the origin node"
    );

    // Level 1 holds immediate predecessors via tagged_with (the fragment node)
    assert!(
        result.levels.len() >= 2,
        "traversal at depth 1 should reach at least one tagged_with neighbor, found {} levels",
        result.levels.len()
    );

    // At least one tagged_with edge must have been traversed
    assert!(
        !result.edges.is_empty(),
        "traversal should have traversed at least one tagged_with edge"
    );

    // Every traversed edge is the relationship we filtered for
    for edge in &result.edges {
        assert_eq!(
            edge.relationship, "tagged_with",
            "with_relationship(\"tagged_with\") should only traverse tagged_with edges"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 3: Evidence trail returns provenance for a concept
// ---------------------------------------------------------------------------

#[tokio::test]
async fn evidence_trail_returns_provenance() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Systems design requires careful thought",
        vec!["systems-design".into(), "architecture".into()],
    );

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    let concept_id = NodeId::from_string("concept:systems-design");
    assert!(
        ctx.get_node(&concept_id).is_some(),
        "concept:systems-design must exist before calling evidence_trail"
    );

    // evidence_trail(concept_id, &context, filter)
    let trail = evidence_trail(concept_id.clone(), &ctx, None);

    assert_eq!(
        trail.concept, concept_id,
        "evidence trail should identify the queried concept"
    );

    // ContentAdapter creates a fragment tagged with the concept. The fragment is
    // connected to the concept via a tagged_with edge (fragment → concept).
    // evidence_trail Branch 2 traverses Incoming "tagged_with" from the concept,
    // which reaches the fragment node.
    assert!(
        !trail.fragments.is_empty(),
        "evidence trail should find at least one fragment tagged with concept:systems-design"
    );

    // ContentAdapter also creates a mark node and a chain node. The mark is connected
    // to the chain via a contains edge (chain → mark). evidence_trail Branch 1 looks
    // for marks that have a "references" edge pointing to the concept; ContentAdapter
    // does not emit references edges, so marks may be empty. What matters is that the
    // fragment provenance path (Branch 2) is populated.
    //
    // All edges in the trail must touch the queried concept.
    for edge in &trail.edges {
        assert!(
            edge.source == concept_id || edge.target == concept_id
                || trail.fragments.iter().any(|n| n.id == edge.source || n.id == edge.target)
                || trail.marks.iter().any(|n| n.id == edge.source || n.id == edge.target)
                || trail.chains.iter().any(|n| n.id == edge.source || n.id == edge.target),
            "every edge in the evidence trail should be reachable from the concept"
        );
    }
}
