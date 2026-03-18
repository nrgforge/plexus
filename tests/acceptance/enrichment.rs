//! Enrichment contract acceptance tests.
//!
//! Scenarios:
//! - Co-occurrence enrichment creates may_be_related edges between concepts sharing a source
//! - Enrichment loop quiesces (does not run forever)

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::NodeId;

#[tokio::test]
async fn cooccurrence_creates_may_be_related_edges() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Rust is a systems programming language",
        vec!["rust".into(), "programming".into()],
    );

    env.api
        .ingest(env.ctx_id(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    let rust_id = NodeId::from_string("concept:rust");
    let programming_id = NodeId::from_string("concept:programming");

    // Both concept nodes must exist before co-occurrence edges can be checked
    assert!(
        ctx.get_node(&rust_id).is_some(),
        "concept:rust node should exist"
    );
    assert!(
        ctx.get_node(&programming_id).is_some(),
        "concept:programming node should exist"
    );

    // CoOccurrenceEnrichment emits symmetric may_be_related edges between
    // concepts that share the same source fragment (via tagged_with edges).
    let has_rust_to_programming = ctx.edges.iter().any(|e| {
        e.source == rust_id
            && e.target == programming_id
            && e.relationship == "may_be_related"
    });
    let has_programming_to_rust = ctx.edges.iter().any(|e| {
        e.source == programming_id
            && e.target == rust_id
            && e.relationship == "may_be_related"
    });

    assert!(
        has_rust_to_programming,
        "concept:rust → concept:programming may_be_related edge should exist"
    );
    assert!(
        has_programming_to_rust,
        "concept:programming → concept:rust may_be_related edge should exist"
    );
}

#[tokio::test]
async fn enrichment_loop_quiesces() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Knowledge graphs connect ideas",
        vec!["knowledge-graph".into(), "ideas".into(), "connections".into()],
    );

    // The enrichment loop must terminate — if it does not quiesce the await
    // will hang and the test will time out. A successful return is the assertion.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        env.api.ingest(env.ctx_id(), "content", Box::new(input)),
    )
    .await
    .expect("ingest should complete within 5 seconds — enrichment loop must quiesce");

    assert!(result.is_ok(), "ingest should succeed");
}
