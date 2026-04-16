//! Extraction contract acceptance tests.
//!
//! Scenarios:
//! - Registration creates file node with MIME type and size
//! - Registration creates extraction status node tracking phase completion
//! - Registration extracts frontmatter tags from markdown files
//! - Semantic extraction with mock ensemble produces concept nodes and relationships

use super::helpers::TestEnv;
use plexus::adapter::extraction::{ExtractionCoordinator, ExtractFileInput};
use plexus::adapter::semantic::SemanticAdapter;
use plexus::adapter::structural::{StructuralModule, StructuralOutput};
use plexus::adapter::{Adapter, AdapterInput, EngineSink, FrameworkContext};
use plexus::llm_orc::{AgentResult, InvokeResponse, MockClient};
use plexus::storage::{OpenStore, SqliteStore};
use plexus::{Context, ContextId, NodeId, PlexusEngine, PropertyValue};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Minimal structural module that succeeds without emitting anything.
/// Required because semantic extraction chains after structural analysis.
struct PassthroughModule;

#[async_trait]
impl StructuralModule for PassthroughModule {
    fn id(&self) -> &str { "passthrough-module" }
    fn mime_affinity(&self) -> &str { "text/" }
    async fn analyze(&self, _file_path: &str, _content: &str) -> StructuralOutput {
        StructuralOutput::default()
    }
}

// --- Registration tests (use standard pipeline via TestEnv) ---

#[tokio::test]
async fn registration_creates_file_node_with_mime_type_and_size() {
    let env = TestEnv::new();
    let fixture_path = TestEnv::fixture("simple.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = ExtractFileInput {
        file_path: file_path.to_string(),
    };

    env.api
        .ingest(env.ctx_name(), "extract-file", Box::new(input))
        .await
        .expect("file extraction should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    let file_node_id = NodeId::from_string(format!("file:{}", file_path));
    let file_node = ctx.get_node(&file_node_id).expect("file node should exist");

    assert_eq!(
        file_node.properties.get("mime_type"),
        Some(&PropertyValue::String("text/markdown".to_string())),
        "file node should have mime_type=text/markdown"
    );

    assert!(
        matches!(file_node.properties.get("file_size"), Some(PropertyValue::Int(size)) if *size > 0),
        "file node should have a positive file_size"
    );
}

#[tokio::test]
async fn registration_creates_extraction_status_node() {
    let env = TestEnv::new();
    let fixture_path = TestEnv::fixture("plain.txt");
    let file_path = fixture_path.to_str().unwrap();

    let input = ExtractFileInput {
        file_path: file_path.to_string(),
    };

    env.api
        .ingest(env.ctx_name(), "extract-file", Box::new(input))
        .await
        .expect("file extraction should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    let status_id = NodeId::from_string(format!("extraction-status:{}", file_path));
    let status_node = ctx.get_node(&status_id).expect("extraction status node should exist");

    assert_eq!(
        status_node.properties.get("registration"),
        Some(&PropertyValue::String("complete".to_string())),
        "registration should be marked complete"
    );
}

#[tokio::test]
async fn registration_extracts_frontmatter_tags() {
    let env = TestEnv::new();
    let fixture_path = TestEnv::fixture("frontmatter.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = ExtractFileInput {
        file_path: file_path.to_string(),
    };

    env.api
        .ingest(env.ctx_name(), "extract-file", Box::new(input))
        .await
        .expect("file extraction should succeed");

    let ctx = env.engine.get_context(&env.context_id).expect("context exists");

    // Frontmatter tags produce concept nodes
    assert!(
        ctx.get_node(&NodeId::from_string("concept:rust")).is_some(),
        "concept:rust should be extracted from frontmatter"
    );
    assert!(
        ctx.get_node(&NodeId::from_string("concept:knowledge-graph")).is_some(),
        "concept:knowledge-graph should be extracted from frontmatter"
    );
    assert!(
        ctx.get_node(&NodeId::from_string("concept:testing")).is_some(),
        "concept:testing should be extracted from frontmatter"
    );

    // tagged_with edges from file to concepts
    let file_node_id = NodeId::from_string(format!("file:{}", file_path));
    let tagged_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.source == file_node_id && e.relationship == "tagged_with")
        .collect();

    assert_eq!(
        tagged_edges.len(),
        3,
        "should have 3 tagged_with edges (rust, knowledge-graph, testing)"
    );
}

// --- Semantic extraction test (custom wiring with mock ensemble) ---

#[tokio::test]
async fn semantic_extraction_with_mock_ensemble_produces_concepts() {
    // Build a mock ensemble response with concepts
    let mut results = HashMap::new();
    results.insert(
        "synthesizer".to_string(),
        AgentResult {
            response: Some(
                r#"{"concepts": [{"label": "Testing", "confidence": 0.9}]}"#.to_string(),
            ),
            status: Some("success".to_string()),
            error: None,
        },
    );
    let mock_response = InvokeResponse {
        results,
        status: "completed".to_string(),
        metadata: serde_json::Value::Null,
    };
    let mock_client = Arc::new(
        MockClient::available().with_response("extract-semantic", mock_response),
    );

    // Build engine with store
    let store = Arc::new(SqliteStore::open_in_memory().unwrap());
    let engine = Arc::new(PlexusEngine::with_store(store.clone()));
    let context_id = ContextId::from_string("semantic-extraction-test");
    let mut ctx = Context::new("semantic-extraction-test");
    ctx.id = context_id.clone();
    engine.upsert_context(ctx).unwrap();

    // Wire coordinator with structural passthrough + semantic extraction
    let semantic = Arc::new(SemanticAdapter::new(mock_client, "extract-semantic"));
    let mut coordinator = ExtractionCoordinator::new()
        .with_engine(engine.clone());
    coordinator.register_structural_module(Arc::new(PassthroughModule));
    coordinator.register_semantic_extraction(semantic);

    let fixture_path = TestEnv::fixture("simple.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = AdapterInput::new(
        "extract-file",
        ExtractFileInput {
            file_path: file_path.to_string(),
        },
        context_id.as_str(),
    );

    let primary_sink = EngineSink::for_engine(engine.clone(), context_id.clone())
        .with_framework_context(FrameworkContext {
            adapter_id: "extract-coordinator".to_string(),
            context_id: context_id.as_str().to_string(),
            input_summary: None,
        });

    coordinator.process(&input, &primary_sink).await.unwrap();
    let bg_results = coordinator.wait_for_background().await;

    assert!(
        bg_results.iter().all(|r| r.is_ok()),
        "semantic extraction should succeed with mock client: {:?}",
        bg_results
    );

    // Concept from ensemble response should be persisted
    let ctx = engine.get_context(&context_id).expect("context exists");
    assert!(
        ctx.get_node(&NodeId::from_string("concept:testing")).is_some(),
        "concept:testing should be produced by semantic extraction mock ensemble"
    );
}
