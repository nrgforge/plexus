//! Degradation contract acceptance tests.
//!
//! Scenarios:
//! - File extraction succeeds even when llm-orc is unavailable
//! - Text ingest succeeds without llm-orc

use super::helpers::TestEnv;
use plexus::adapter::extraction::ExtractFileInput;
use plexus::adapter::FragmentInput;
use plexus::NodeId;

/// Invariant 47: When llm-orc is unavailable, Phase 1 still completes and
/// the file node is registered. Phase 3 is skipped gracefully — no error.
#[tokio::test]
async fn file_extraction_succeeds_without_llm_orc() {
    // TestEnv::new() uses MockClient::unavailable() — Phase 3 skips by default.
    let env = TestEnv::new();
    let fixture_path = TestEnv::fixture("simple.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = ExtractFileInput {
        file_path: file_path.to_string(),
    };

    // Extraction must not error even though llm-orc is unavailable.
    env.api
        .ingest(env.ctx_id(), "extract-file", Box::new(input))
        .await
        .expect("file extraction should succeed without llm-orc");

    // Phase 1 completed: file node exists.
    let ctx = env.engine.get_context(&env.context_id).expect("context exists");
    let file_node_id = NodeId::from_string(format!("file:{}", file_path));
    assert!(
        ctx.get_node(&file_node_id).is_some(),
        "file node should be created by Phase 1 even when llm-orc is unavailable"
    );
}

/// Invariant 47: Text content ingested via ContentAdapter succeeds regardless
/// of llm-orc availability. Concept nodes from tags are created by Phase 1
/// (the ContentAdapter is not llm-orc-gated).
#[tokio::test]
async fn text_ingest_succeeds_without_llm_orc() {
    // TestEnv::new() uses MockClient::unavailable() — Phase 3 skips by default.
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Knowledge graphs represent structured information",
        vec!["knowledge-graph".into(), "structure".into()],
    );

    // Ingest must not error.
    let result = env
        .api
        .ingest(env.ctx_id(), "content", Box::new(input))
        .await;

    assert!(
        result.is_ok(),
        "text ingest should succeed without llm-orc: {:?}",
        result.err()
    );

    // Concept nodes from tags are created by ContentAdapter (no LLM required).
    let ctx = env.engine.get_context(&env.context_id).expect("context exists");
    assert!(
        ctx.get_node(&NodeId::from_string("concept:knowledge-graph")).is_some(),
        "concept:knowledge-graph should exist after ingest without llm-orc"
    );
    assert!(
        ctx.get_node(&NodeId::from_string("concept:structure")).is_some(),
        "concept:structure should exist after ingest without llm-orc"
    );
}
