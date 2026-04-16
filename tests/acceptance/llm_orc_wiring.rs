//! Acceptance tests for llm-orc client wiring through the pipeline builder
//! and load_spec.
//!
//! Findings closed:
//! - A: `PipelineBuilder` exposes `with_llm_client` to wire SemanticAdapter
//!   onto the ExtractionCoordinator (so `extract-file` ingest invokes
//!   semantic extraction by default through `default_pipeline`).
//! - B: `PlexusApi::load_spec` propagates the pipeline's llm_client to
//!   declarative adapters with `ensemble:` field.

use plexus::adapter::PipelineBuilder;
use plexus::adapter::extraction::ExtractFileInput;
use plexus::llm_orc::{AgentResult, InvokeResponse, LlmOrcClient, MockClient};
use plexus::{Context, ContextId, NodeId, OpenStore, PlexusApi, PlexusEngine, SqliteStore};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const POLL_TIMEOUT: Duration = Duration::from_secs(2);

// ── Helpers ────────────────────────────────────────────────────────────

fn mock_llm_client(ensemble_name: &str, synthesizer_json: &str) -> Arc<dyn LlmOrcClient> {
    let mut results = HashMap::new();
    results.insert(
        "synthesizer".to_string(),
        AgentResult {
            response: Some(synthesizer_json.to_string()),
            status: Some("success".to_string()),
            error: None,
        },
    );
    let response = InvokeResponse {
        results,
        status: "completed".to_string(),
        metadata: serde_json::Value::Null,
    };
    Arc::new(MockClient::available().with_response(ensemble_name, response))
}

fn setup_engine(name: &str) -> (Arc<PlexusEngine>, ContextId) {
    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));
    let context_id = ContextId::from_string(name);
    let mut ctx = Context::new(name);
    ctx.id = context_id.clone();
    engine.upsert_context(ctx).expect("upsert");
    (engine, context_id)
}

async fn poll_for_node(engine: &Arc<PlexusEngine>, context_id: &ContextId, target: &NodeId) -> bool {
    let deadline = std::time::Instant::now() + POLL_TIMEOUT;
    while std::time::Instant::now() < deadline {
        let ctx = engine.get_context(context_id).expect("context");
        if ctx.get_node(target).is_some() {
            return true;
        }
        drop(ctx);
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    false
}

fn write_fixture(dir: &TempDir, name: &str, contents: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, contents).expect("write fixture");
    path.to_str().expect("path utf-8").to_string()
}

// ── Finding A: PipelineBuilder::with_llm_client wires semantic extraction ──

/// Given a pipeline built with `with_llm_client(mock)`,
/// when extract-file ingest runs on a markdown fixture,
/// then the mock ensemble's response produces concept nodes in the graph.
///
/// This proves `with_llm_client` correctly wires `SemanticAdapter` onto
/// the `ExtractionCoordinator`, which `default_pipeline` then leverages
/// to provide built-in semantic extraction for MCP consumers.
#[tokio::test]
async fn pipeline_with_llm_client_wires_semantic_extraction() {
    let mock = mock_llm_client(
        "extract-semantic",
        r#"{"concepts": [{"label": "Wired", "confidence": 0.9}]}"#,
    );

    let (engine, context_id) = setup_engine("wiring-a");

    let pipeline = PipelineBuilder::new(engine.clone())
        .with_default_adapters()
        .with_default_structural_modules()
        .with_default_enrichments()
        .with_llm_client(mock)
        .build();

    let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

    let tmp = TempDir::new().expect("tempdir");
    let file_path = write_fixture(&tmp, "wiring.md", "# Wiring test\n\nProse content.\n");

    api.ingest("wiring-a", "extract-file", Box::new(ExtractFileInput { file_path }))
        .await
        .expect("extract-file ingest should succeed");

    let target = NodeId::from_string("concept:wired");
    let appeared = poll_for_node(&engine, &context_id, &target).await;
    assert!(
        appeared,
        "concept:wired should be produced by mock semantic extraction within {:?} \
         — `with_llm_client` may not be wiring SemanticAdapter onto the coordinator",
        POLL_TIMEOUT
    );
}

// ── Finding B: load_spec propagates llm_client to declarative adapters ──

/// Given a pipeline built with `with_llm_client(mock)`,
/// when a declarative spec with an `ensemble:` field is loaded via load_spec
/// and then ingested through the spec's input_kind,
/// then the mock ensemble is invoked and the spec's emit primitives produce
/// nodes whose IDs reference fields from the ensemble response.
///
/// This proves `load_spec` propagates the pipeline's llm_client to the
/// DeclarativeAdapter — without this propagation, the adapter would return
/// `AdapterError::Skipped("ensemble declared but no LlmOrcClient configured")`
/// and no theme node would be produced.
#[tokio::test]
async fn load_spec_propagates_llm_client_to_declarative_adapter() {
    let mock = mock_llm_client(
        "trellis-themes",
        r#"{"theme": "wandering"}"#,
    );

    let (engine, context_id) = setup_engine("wiring-b");

    let pipeline = PipelineBuilder::new(engine.clone())
        .with_default_adapters()
        .with_default_enrichments()
        .with_llm_client(mock)
        .build();

    let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

    let spec_yaml = r#"
adapter_id: trellis-themes
input_kind: trellis.theme-input
ensemble: trellis-themes
emit:
  - create_node:
      id: "theme:{ensemble.theme}"
      type: theme
      dimension: semantic
"#;

    api.load_spec("wiring-b", spec_yaml)
        .await
        .expect("load_spec should succeed");

    api.ingest(
        "wiring-b",
        "trellis.theme-input",
        Box::new(serde_json::json!({"text": "test input"})),
    )
    .await
    .expect("ingest with consumer's input_kind should succeed");

    let ctx = engine.get_context(&context_id).expect("context");
    let target = NodeId::from_string("theme:wandering");
    assert!(
        ctx.get_node(&target).is_some(),
        "theme:wandering should be produced by mock ensemble + emit primitive — \
         load_spec is not propagating the pipeline's llm_client to the DeclarativeAdapter. \
         Existing nodes: {:?}",
        ctx.nodes().map(|n| n.id.to_string()).collect::<Vec<_>>()
    );
}
