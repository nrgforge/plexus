//! Shared test utilities for acceptance tests.

use plexus::adapter::{IngestPipeline, PipelineBuilder};
use plexus::llm_orc::{LlmOrcClient, MockClient};
use plexus::storage::{OpenStore, SqliteStore};
use plexus::{Context, ContextId, PlexusApi, PlexusEngine};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Test environment wiring PlexusEngine + IngestPipeline + PlexusApi.
///
/// Uses in-memory SQLite and a mock llm-orc client by default.
pub struct TestEnv {
    pub engine: Arc<PlexusEngine>,
    pub store: Arc<SqliteStore>,
    pub pipeline: Arc<IngestPipeline>,
    pub api: PlexusApi,
    pub context_id: ContextId,
    pub context_name: String,
}

impl TestEnv {
    /// Default env with `MockClient::unavailable()` — Phase 3 skips gracefully.
    pub fn new() -> Self {
        let client = Arc::new(MockClient::unavailable());
        Self::with_mock_client(client)
    }

    /// Env with a custom mock llm-orc client.
    pub fn with_mock_client(client: Arc<dyn LlmOrcClient>) -> Self {
        let store = Arc::new(SqliteStore::open_in_memory().expect("in-memory SQLite"));
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));

        // Create a test context
        let context_name = "acceptance-test".to_string();
        let ctx = Context::new(&context_name);
        let context_id = ctx.id.clone();
        engine.upsert_context(ctx).expect("upsert context");

        // Build pipeline with default adapters and enrichments
        let pipeline = Arc::new(
            PipelineBuilder::new(engine.clone())
                .with_default_adapters()
                .with_default_enrichments()
                .build(),
        );

        let api = PlexusApi::new(engine.clone(), pipeline.clone());

        Self {
            engine,
            store,
            pipeline,
            api,
            context_id,
            context_name,
        }
    }

    /// Path to a fixture file.
    pub fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    /// Read a fixture file as a string.
    pub fn fixture_content(name: &str) -> String {
        std::fs::read_to_string(Self::fixture(name))
            .unwrap_or_else(|e| panic!("failed to read fixture '{}': {}", name, e))
    }

    /// The context ID as a string (for API calls).
    pub fn ctx_id(&self) -> &str {
        self.context_id.as_str()
    }
}
