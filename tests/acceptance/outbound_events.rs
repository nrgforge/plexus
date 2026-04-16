//! Outbound event coverage — symmetric `transform_events` across
//! built-in adapters.
//!
//! ContentAdapter and ProvenanceAdapter override `Adapter::transform_events`
//! to produce consumer-meaningful OutboundEvents from their emit results
//! (see content.rs:318 and provenance_adapter.rs:244). DeclarativeAdapter
//! and ExtractionCoordinator used the trait default (empty Vec) —
//! symmetric with the pattern here.
//!
//! These tests pin the new behavior so regressions surface early.

use plexus::adapter::PipelineBuilder;
use plexus::adapter::extraction::ExtractFileInput;
use plexus::{Context, ContextId, NodeId, OpenStore, PlexusApi, PlexusEngine, SqliteStore};
use std::sync::Arc;
use tempfile::TempDir;

fn setup_engine(name: &str) -> (Arc<PlexusEngine>, ContextId) {
    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));
    let context_id = ContextId::from_string(name);
    let mut ctx = Context::new(name);
    ctx.id = context_id.clone();
    engine.upsert_context(ctx).expect("upsert");
    (engine, context_id)
}

// ── DeclarativeAdapter ─────────────────────────────────────────────────

/// Given a declarative spec whose emit creates multiple node types,
/// when the adapter ingests via its declared input_kind,
/// then the returned outbound events include one `{type}_created` event
/// per created node — derived mechanically from the spec's own type
/// labels (consumer owns the vocabulary).
#[tokio::test]
async fn declarative_adapter_emits_outbound_events_per_node_type() {
    let (engine, _) = setup_engine("outbound-decl");

    let pipeline = PipelineBuilder::new(engine.clone())
        .with_default_adapters()
        .with_default_enrichments()
        .build();
    let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

    let spec_yaml = r#"
adapter_id: test-outbound
input_kind: test.outbound
emit:
  - create_node:
      id: "fragment:{input.id}"
      type: fragment
      dimension: structure
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
  - create_edge:
      source: "fragment:{input.id}"
      target: "concept:{input.tag}"
      relationship: tagged_with
"#;
    api.load_spec("outbound-decl", spec_yaml).await.expect("load_spec");

    let events = api
        .ingest(
            "outbound-decl",
            "test.outbound",
            Box::new(serde_json::json!({"id": "A", "tag": "travel"})),
        )
        .await
        .expect("ingest");

    let kinds: Vec<String> = events.iter().map(|e| e.kind.clone()).collect();

    assert!(
        kinds.iter().any(|k| k == "fragment_created"),
        "expected fragment_created event — got kinds: {:?}",
        kinds
    );
    assert!(
        kinds.iter().any(|k| k == "concept_created"),
        "expected concept_created event — got kinds: {:?}",
        kinds
    );
    assert!(
        kinds.iter().any(|k| k == "tagged_with_linked"),
        "expected tagged_with_linked event — got kinds: {:?}",
        kinds
    );
}

// ── ExtractionCoordinator ──────────────────────────────────────────────

/// Given an ingest through `extract-file` input_kind,
/// when ExtractionCoordinator runs registration synchronously,
/// then the returned outbound events include `file_registered` and
/// (when frontmatter is present) `concept_created` events.
///
/// Structural and semantic phases run in background — their events
/// are not visible in the ingest caller's response (documented
/// limitation, tracked as follow-up in cycle-status.md).
#[tokio::test]
async fn extraction_coordinator_emits_outbound_events_for_registration() {
    let (engine, _) = setup_engine("outbound-extract");

    let pipeline = PipelineBuilder::new(engine.clone())
        .with_default_adapters()
        .with_default_structural_modules()
        .with_default_enrichments()
        .build();
    let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

    let tmp = TempDir::new().expect("tempdir");
    let file_path = tmp.path().join("frontmatter.md");
    std::fs::write(
        &file_path,
        "---\ntags: [alpha, beta]\n---\n\n# Test\n\nContent.",
    )
    .expect("write fixture");

    let events = api
        .ingest(
            "outbound-extract",
            "extract-file",
            Box::new(ExtractFileInput {
                file_path: file_path.to_str().unwrap().to_string(),
            }),
        )
        .await
        .expect("extract-file ingest");

    let kinds: Vec<String> = events.iter().map(|e| e.kind.clone()).collect();
    assert!(
        kinds.iter().any(|k| k == "file_registered"),
        "expected file_registered event from registration — got kinds: {:?}",
        kinds
    );
    assert!(
        kinds.iter().any(|k| k == "concept_created"),
        "expected concept_created event(s) from frontmatter tags — got kinds: {:?}",
        kinds
    );

    // Registration writes a file: node deterministically
    let ctx = engine.get_context(&ContextId::from_string("outbound-extract")).unwrap();
    let file_node_id = NodeId::from_string(format!("file:{}", file_path.to_str().unwrap()));
    assert!(
        ctx.get_node(&file_node_id).is_some(),
        "registration should create the file node"
    );
}
