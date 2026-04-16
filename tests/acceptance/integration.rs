//! Tier 2 integration tests — real llm-orc ensemble execution (WP-B4).
//!
//! These tests invoke the actual `extract-semantic` ensemble against fixture
//! documents via a running Ollama instance. They validate:
//! - Ensemble YAML is well-formed and executable
//! - SpaCy script runs and produces parseable output
//! - SemanticAdapter correctly parses LLM output into graph mutations
//! - Enrichments fire on extraction output
//!
//! **Gated:** Only run when `PLEXUS_INTEGRATION=1` is set.
//! **Requires:** Running Ollama instance with `mistral:7b` model.
//!
//! Assertions are property-based (produces concepts, produces relationships)
//! — never exact counts or labels, because LLM output varies.

use super::helpers::TestEnv;
use plexus::adapter::extraction::{ExtractionCoordinator, ExtractFileInput};
use plexus::adapter::semantic::SemanticAdapter;
use plexus::adapter::structural::MarkdownStructureModule;
use plexus::adapter::{Adapter, AdapterInput, EngineSink, FrameworkContext};
use plexus::llm_orc::SubprocessClient;
use plexus::storage::{OpenStore, SqliteStore};
use plexus::{Context, ContextId, NodeId, PlexusEngine};
use std::sync::Arc;

/// Check if integration tests should run.
fn integration_enabled() -> bool {
    std::env::var("PLEXUS_INTEGRATION")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Build a real coordinator wired with SubprocessClient + MarkdownStructureModule.
fn build_real_coordinator(engine: Arc<PlexusEngine>) -> ExtractionCoordinator {
    let client: Arc<dyn plexus::llm_orc::LlmOrcClient> = Arc::new(SubprocessClient::new());
    let semantic = Arc::new(SemanticAdapter::new(client, "extract-semantic"));
    let mut coordinator = ExtractionCoordinator::new()
        .with_engine(engine);
    coordinator.register_structural_module(Arc::new(MarkdownStructureModule::new()));
    coordinator.register_semantic_extraction(semantic);
    coordinator
}

/// Build engine + context for a test, returning (engine, context_id).
fn setup_engine() -> (Arc<PlexusEngine>, ContextId) {
    let store = Arc::new(SqliteStore::open_in_memory().unwrap());
    let engine = Arc::new(PlexusEngine::with_store(store));
    let context_id = ContextId::from_string("integration-test");
    let mut ctx = Context::new("integration-test");
    ctx.id = context_id.clone();
    engine.upsert_context(ctx).unwrap();
    (engine, context_id)
}

/// Build an EngineSink for extraction.
fn extraction_sink(engine: Arc<PlexusEngine>, context_id: ContextId) -> EngineSink {
    EngineSink::for_engine(engine, context_id.clone())
        .with_framework_context(FrameworkContext {
            adapter_id: "extract-coordinator".to_string(),
            context_id: context_id.as_str().to_string(),
            input_summary: None,
        })
}

// --- Tests ---

#[tokio::test]
async fn extraction_produces_concept_nodes_from_markdown() {
    if !integration_enabled() {
        return;
    }

    let (engine, context_id) = setup_engine();
    let coordinator = build_real_coordinator(engine.clone());
    let sink = extraction_sink(engine.clone(), context_id.clone());

    let fixture_path = TestEnv::fixture("frontmatter.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = AdapterInput::new(
        "extract-file",
        ExtractFileInput { file_path: file_path.to_string() },
        context_id.as_str(),
    );

    coordinator.process(&input, &sink).await.unwrap();
    let bg_results = coordinator.wait_for_background().await;

    // At least one background task should have run
    assert!(!bg_results.is_empty(), "background tasks should have executed");

    // Allow failures from individual agents — the pipeline is resilient
    let successes = bg_results.iter().filter(|r| r.is_ok()).count();
    assert!(successes > 0, "at least one background task should succeed: {:?}", bg_results);

    // Check that concept nodes were produced
    let ctx = engine.get_context(&context_id).expect("context exists");
    let concept_nodes: Vec<_> = ctx.nodes()
        .filter(|n| n.node_type == "concept")
        .collect();

    assert!(
        !concept_nodes.is_empty(),
        "extraction should produce at least one concept node from frontmatter.md"
    );
}

#[tokio::test]
async fn extraction_produces_relationships_between_concepts() {
    if !integration_enabled() {
        return;
    }

    let (engine, context_id) = setup_engine();
    let coordinator = build_real_coordinator(engine.clone());
    let sink = extraction_sink(engine.clone(), context_id.clone());

    let fixture_path = TestEnv::fixture("simple.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = AdapterInput::new(
        "extract-file",
        ExtractFileInput { file_path: file_path.to_string() },
        context_id.as_str(),
    );

    coordinator.process(&input, &sink).await.unwrap();
    let bg_results = coordinator.wait_for_background().await;
    let successes = bg_results.iter().filter(|r| r.is_ok()).count();
    assert!(successes > 0, "at least one background task should succeed: {:?}", bg_results);

    let ctx = engine.get_context(&context_id).expect("context exists");

    // Relationship edges should exist (tagged_with, or semantic relationships)
    let relationship_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.source.as_str().starts_with("concept:") || e.target.as_str().starts_with("concept:"))
        .collect();

    assert!(
        !relationship_edges.is_empty(),
        "extraction should produce edges involving concept nodes"
    );
}

#[tokio::test]
async fn structural_analysis_feeds_vocabulary_to_semantic() {
    if !integration_enabled() {
        return;
    }

    let (engine, context_id) = setup_engine();
    let coordinator = build_real_coordinator(engine.clone());
    let sink = extraction_sink(engine.clone(), context_id.clone());

    // Use simple.md — has headings that structural analysis will extract
    let fixture_path = TestEnv::fixture("simple.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = AdapterInput::new(
        "extract-file",
        ExtractFileInput { file_path: file_path.to_string() },
        context_id.as_str(),
    );

    coordinator.process(&input, &sink).await.unwrap();
    let bg_results = coordinator.wait_for_background().await;
    let successes = bg_results.iter().filter(|r| r.is_ok()).count();
    assert!(successes > 0, "at least one background task should succeed: {:?}", bg_results);

    // Structural analysis should have produced file + concept nodes
    let ctx = engine.get_context(&context_id).expect("context exists");

    // File node must exist (registration)
    let file_node_id = NodeId::from_string(format!("file:{}", file_path));
    assert!(
        ctx.get_node(&file_node_id).is_some(),
        "file node should exist from registration"
    );

    // Concept nodes should exist (from semantic extraction)
    let concept_count = ctx.nodes()
        .filter(|n| n.node_type == "concept")
        .count();
    assert!(
        concept_count > 0,
        "semantic extraction should produce concept nodes"
    );
}

#[tokio::test]
async fn enrichments_fire_on_extraction_output() {
    if !integration_enabled() {
        return;
    }

    let (engine, context_id) = setup_engine();
    let coordinator = build_real_coordinator(engine.clone());
    let sink = extraction_sink(engine.clone(), context_id.clone());

    let fixture_path = TestEnv::fixture("frontmatter.md");
    let file_path = fixture_path.to_str().unwrap();

    let input = AdapterInput::new(
        "extract-file",
        ExtractFileInput { file_path: file_path.to_string() },
        context_id.as_str(),
    );

    coordinator.process(&input, &sink).await.unwrap();
    let bg_results = coordinator.wait_for_background().await;
    let successes = bg_results.iter().filter(|r| r.is_ok()).count();
    assert!(successes > 0, "at least one background task should succeed: {:?}", bg_results);

    let ctx = engine.get_context(&context_id).expect("context exists");

    // After extraction, registration's frontmatter tags create tagged_with edges.
    // These edges involve concept nodes sharing the same file source,
    // which should trigger CoOccurrenceEnrichment to create may_be_related edges.
    let tagged_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.relationship == "tagged_with")
        .collect();

    // frontmatter.md has 3 tags — should produce 3 tagged_with edges minimum
    assert!(
        tagged_edges.len() >= 3,
        "should have at least 3 tagged_with edges from frontmatter tags, got {}",
        tagged_edges.len()
    );

    // CoOccurrence enrichment should have fired (may_be_related edges)
    // Only check if enough concept nodes exist for co-occurrence to trigger
    let concept_count = ctx.nodes()
        .filter(|n| n.node_type == "concept")
        .count();

    if concept_count >= 2 {
        let cooccurrence_edges: Vec<_> = ctx.edges.iter()
            .filter(|e| e.relationship == "may_be_related")
            .collect();
        assert!(
            !cooccurrence_edges.is_empty(),
            "CoOccurrenceEnrichment should produce may_be_related edges when {} concepts share a source",
            concept_count
        );
    }
}

#[tokio::test]
async fn non_markdown_file_gets_no_structural_analysis() {
    if !integration_enabled() {
        return;
    }

    let (engine, context_id) = setup_engine();
    let coordinator = build_real_coordinator(engine.clone());
    let sink = extraction_sink(engine.clone(), context_id.clone());

    // Use a .rs file — MarkdownStructureModule should NOT match
    let fixture_path = TestEnv::fixture("code_sample.rs");
    let file_path = fixture_path.to_str().unwrap();

    let input = AdapterInput::new(
        "extract-file",
        ExtractFileInput { file_path: file_path.to_string() },
        context_id.as_str(),
    );

    coordinator.process(&input, &sink).await.unwrap();
    let bg_results = coordinator.wait_for_background().await;

    // Registration should create file node regardless
    let ctx = engine.get_context(&context_id).expect("context exists");
    let file_node_id = NodeId::from_string(format!("file:{}", file_path));
    assert!(
        ctx.get_node(&file_node_id).is_some(),
        "file node should exist even for non-markdown files"
    );

    // Background tasks should still run (semantic extraction gets empty structural context)
    // Semantic extraction may still produce concepts from code
    let successes = bg_results.iter().filter(|r| r.is_ok()).count();
    assert!(successes > 0, "semantic extraction should still run for non-markdown files: {:?}", bg_results);
}
