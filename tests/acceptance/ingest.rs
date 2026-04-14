//! Ingest contract acceptance tests.
//!
//! Scenarios:
//! - Ingesting text content routes to ContentAdapter and produces concept nodes
//! - Ingesting with explicit input_kind routes directly to matching adapter
//! - Ingesting frontmatter-bearing markdown produces tagged_with edges
//! - Ingest returns outbound events describing graph mutations

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::NodeId;

#[tokio::test]
async fn ingest_text_content_produces_concept_nodes() {
    let env = TestEnv::new();
    let input = FragmentInput::new("Rust is a systems programming language", vec![
        "rust".into(),
        "programming".into(),
    ]);

    let events = env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Verify concept nodes were created
    let ctx = env.engine.get_context(&env.context_id).expect("context exists");
    assert!(
        ctx.get_node(&NodeId::from_string("concept:rust")).is_some(),
        "concept:rust node should exist"
    );
    assert!(
        ctx.get_node(&NodeId::from_string("concept:programming")).is_some(),
        "concept:programming node should exist"
    );

    // Ingest should return outbound events
    assert!(!events.is_empty(), "ingest should return outbound events");
}

#[tokio::test]
async fn ingest_with_explicit_input_kind_routes_to_adapter() {
    let env = TestEnv::new();
    let input = FragmentInput::new("test content", vec!["test".into()]);

    // Using explicit "content" input_kind
    let result = env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await;

    assert!(result.is_ok(), "explicit input_kind 'content' should route to ContentAdapter");
}

#[tokio::test]
async fn ingest_frontmatter_markdown_produces_tagged_edges() {
    let env = TestEnv::new();
    let content = TestEnv::fixture_content("frontmatter.md");

    // Parse frontmatter to extract tags
    let tags: Vec<String> = vec!["rust".into(), "knowledge-graph".into(), "testing".into()];
    let input = FragmentInput::new(&content, tags);

    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Verify tagged_with edges exist
    let ctx = env.engine.get_context(&env.context_id).expect("context exists");
    let tagged_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    assert!(
        tagged_edges.len() >= 3,
        "should have at least 3 tagged_with edges (one per tag), got {}",
        tagged_edges.len()
    );
}

#[tokio::test]
async fn ingest_returns_outbound_events() {
    let env = TestEnv::new();
    let input = FragmentInput::new("event test content", vec!["events".into()]);

    let events = env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Should have at least one outbound event describing the mutation
    assert!(
        !events.is_empty(),
        "ingest should produce outbound events"
    );
}

#[tokio::test]
async fn ingest_unknown_input_kind_returns_error() {
    let env = TestEnv::new();
    let data: Box<dyn std::any::Any + Send + Sync> = Box::new("bogus data".to_string());

    let result = env.api
        .ingest(env.ctx_name(), "nonexistent-kind", data)
        .await;

    assert!(result.is_err(), "unknown input_kind should produce an error");
}
