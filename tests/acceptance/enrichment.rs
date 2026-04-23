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
        .ingest(env.ctx_name(), "content", Box::new(input))
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

// ADR-039: TemporalProximityEnrichment fires on ContentAdapter output.
//
// The integration test for the `created_at` property contract. Two fragments
// ingested through the real PlexusApi → IngestPipeline → ContentAdapter path,
// in the same context, within the 24-hour default threshold. The contract is
// satisfied only when the producer (ContentAdapter) and consumer
// (TemporalProximityEnrichment) agree on the property surface (node.properties)
// and the on-wire format (ISO-8601 UTC string).
#[tokio::test]
async fn temporal_proximity_fires_on_content_adapter_output_within_window() {
    let env = TestEnv::new();

    env.api
        .ingest(
            env.ctx_name(),
            "content",
            Box::new(FragmentInput::new(
                "first fragment",
                vec!["alpha".into()],
            )),
        )
        .await
        .expect("first ingest should succeed");

    env.api
        .ingest(
            env.ctx_name(),
            "content",
            Box::new(FragmentInput::new(
                "second fragment",
                vec!["beta".into()],
            )),
        )
        .await
        .expect("second ingest should succeed");

    let ctx = env
        .engine
        .get_context(&env.context_id)
        .expect("context exists");

    // Identify the two fragment nodes. Their IDs are UUID-v5-hashed from
    // the fragment text, so we match by node_type rather than by ID.
    let fragment_ids: Vec<_> = ctx
        .nodes
        .values()
        .filter(|n| n.node_type == "fragment")
        .map(|n| n.id.clone())
        .collect();
    assert_eq!(
        fragment_ids.len(),
        2,
        "two fragments should have been ingested"
    );

    let has_forward = ctx.edges.iter().any(|e| {
        e.source == fragment_ids[0]
            && e.target == fragment_ids[1]
            && e.relationship == "temporal_proximity"
    });
    let has_reverse = ctx.edges.iter().any(|e| {
        e.source == fragment_ids[1]
            && e.target == fragment_ids[0]
            && e.relationship == "temporal_proximity"
    });

    assert!(
        has_forward && has_reverse,
        "TemporalProximityEnrichment should emit a symmetric temporal_proximity \
         edge pair between two fragments ingested within the 24-hour window \
         (ADR-039 producer/consumer contract)"
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
        env.api.ingest(env.ctx_name(), "content", Box::new(input)),
    )
    .await
    .expect("ingest should complete within 5 seconds — enrichment loop must quiesce");

    assert!(result.is_ok(), "ingest should succeed");
}
