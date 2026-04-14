//! Acceptance tests for consumer spec loading lifecycle (ADR-037).
//!
//! Covers: spec validation, complete spec wiring, lens enrichment execution,
//! spec persistence, unloading, vocabulary layer discovery, and end-to-end
//! consumer workflows.

#[allow(unused_imports)]
use crate::helpers::TestEnv;

// ---------------------------------------------------------------------------
// Feature: Complete Spec Wiring (ADR-037 §§1,4)
// ---------------------------------------------------------------------------

/// Scenario: register_specs_from_dir wires enrichments and lens
///
/// Given a directory containing a valid spec YAML file with adapter ID "trellis",
///   enrichments, and a lens section
/// When `register_specs_from_dir` is called with that directory
/// Then the adapter is registered for ingest routing
/// And the spec's declared enrichments are registered in the enrichment registry
/// And the lens enrichment is registered in the enrichment registry
#[tokio::test]
async fn register_specs_from_dir_wires_enrichments_and_lens() {
    use plexus::adapter::{Enrichment, IngestPipeline};
    use plexus::storage::{OpenStore, SqliteStore};
    use plexus::PlexusEngine;
    use std::sync::Arc;
    use tempfile::TempDir;

    // Set up a minimal engine
    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    // Write a spec YAML with adapter, enrichment declaration, and lens
    let tmp = TempDir::new().expect("tempdir");
    let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
enrichments:
  - type: co_occurrence
    source_relationship: tagged_with
    output_relationship: may_be_related
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    std::fs::write(tmp.path().join("trellis.yaml"), spec_yaml).expect("write spec");

    // Create a pipeline and load specs from the directory
    let mut pipeline = IngestPipeline::new(engine);
    let loaded = pipeline.register_specs_from_dir(tmp.path(), None);

    assert_eq!(loaded, 1, "one spec should be loaded");

    // Assert: adapter is registered for ingest routing
    let kinds = pipeline.registered_input_kinds();
    assert!(
        kinds.iter().any(|k| k == "trellis.fragment"),
        "adapter should be registered for input_kind 'trellis.fragment', got: {:?}",
        kinds
    );

    // Assert: enrichment registry includes declared enrichments AND lens
    let registry = pipeline.enrichment_registry();
    let enrichment_ids: Vec<&str> = registry
        .enrichments()
        .iter()
        .map(|e| e.id())
        .collect();

    assert!(
        enrichment_ids.iter().any(|id| id.contains("co_occurrence")),
        "co_occurrence enrichment should be registered, got: {:?}",
        enrichment_ids
    );
    assert!(
        enrichment_ids.iter().any(|id| *id == "lens:trellis"),
        "lens enrichment should be registered with id 'lens:trellis', got: {:?}",
        enrichment_ids
    );
}

// ---------------------------------------------------------------------------
// Feature: Builder Rehydration (ADR-037 §2)
// ---------------------------------------------------------------------------

/// PipelineBuilder::with_persisted_specs rehydrates lens enrichments from
/// persisted spec data. The original adapter is NOT re-registered; only
/// the lens enrichment is extracted and registered.
#[tokio::test]
async fn builder_with_persisted_specs_rehydrates_lens_only() {
    use plexus::adapter::{Enrichment, PipelineBuilder};
    use plexus::storage::{OpenStore, PersistedSpec, SqliteStore};
    use plexus::PlexusEngine;
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;

    let persisted = vec![PersistedSpec {
        context_id: "ctx-1".into(),
        adapter_id: "trellis-content".into(),
        spec_yaml: spec_yaml.into(),
        loaded_at: "2026-04-12T00:00:00Z".into(),
    }];

    let pipeline = PipelineBuilder::new(engine)
        .with_default_adapters()
        .with_default_enrichments()
        .with_persisted_specs(persisted)
        .build();

    // The lens enrichment should be registered
    let registry = pipeline.enrichment_registry();
    let enrichment_ids: Vec<&str> = registry
        .enrichments()
        .iter()
        .map(|e| e.id())
        .collect();

    assert!(
        enrichment_ids.iter().any(|id| *id == "lens:trellis"),
        "persisted lens should be rehydrated, got: {:?}",
        enrichment_ids
    );

    // The adapter should NOT be registered (enrichment-only rehydration)
    let kinds = pipeline.registered_input_kinds();
    assert!(
        !kinds.iter().any(|k| k == "trellis.fragment"),
        "adapter should NOT be re-registered during rehydration, got: {:?}",
        kinds
    );
}

/// Malformed persisted spec is logged and skipped — pipeline construction
/// continues with remaining specs.
#[tokio::test]
async fn builder_with_persisted_specs_skips_malformed() {
    use plexus::adapter::{Enrichment, PipelineBuilder};
    use plexus::storage::{OpenStore, PersistedSpec, SqliteStore};
    use plexus::PlexusEngine;
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let good_yaml = r#"
adapter_id: carrel
input_kind: carrel.citation
lens:
  consumer: carrel
  translations:
    - from: [may_be_related]
      to: citation_link
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;

    let persisted = vec![
        PersistedSpec {
            context_id: "ctx-1".into(),
            adapter_id: "bad-spec".into(),
            spec_yaml: "this is not valid yaml: [[[".into(),
            loaded_at: "2026-04-12T00:00:00Z".into(),
        },
        PersistedSpec {
            context_id: "ctx-1".into(),
            adapter_id: "carrel".into(),
            spec_yaml: good_yaml.into(),
            loaded_at: "2026-04-12T00:00:00Z".into(),
        },
    ];

    let pipeline = PipelineBuilder::new(engine)
        .with_persisted_specs(persisted)
        .build();

    // The good spec's lens should be registered despite the bad spec
    let registry = pipeline.enrichment_registry();
    let enrichment_ids: Vec<&str> = registry
        .enrichments()
        .iter()
        .map(|e| e.id())
        .collect();

    assert!(
        enrichment_ids.iter().any(|id| *id == "lens:carrel"),
        "good spec's lens should be rehydrated despite bad spec, got: {:?}",
        enrichment_ids
    );
}

// ---------------------------------------------------------------------------
// Feature: Spec Validation (ADR-037 §1, Invariant 60)
// ---------------------------------------------------------------------------

/// Scenario: invalid spec YAML fails before any graph work
///
/// Given a context "test" exists
/// And a malformed YAML string (not valid YAML)
/// When `load_spec` is called with context "test" and the malformed YAML
/// Then the result is an error indicating validation failure
/// And no adapter is registered on the pipeline
/// And no enrichments are registered
/// And no edges are added to the graph
#[tokio::test]
async fn load_spec_invalid_yaml_leaves_no_mutations() {
    use plexus::adapter::PipelineBuilder;
    use plexus::storage::{GraphStore, OpenStore, SqliteStore};
    use plexus::{Context, PlexusApi, PlexusEngine};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store.clone()));

    let ctx = Context::new("test");
    engine.upsert_context(ctx).expect("upsert");

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_default_enrichments()
            .build(),
    );
    let api = PlexusApi::new(engine.clone(), pipeline.clone());

    // Snapshot state before
    let enrichment_count_before = pipeline.enrichment_registry().enrichments().len();
    let kinds_before = pipeline.registered_input_kinds();

    // Attempt to load malformed YAML
    let result = api.load_spec("test", "this is not valid yaml: [[[").await;
    assert!(result.is_err(), "malformed YAML should fail");

    // Assert: no mutations occurred
    let enrichment_count_after = pipeline.enrichment_registry().enrichments().len();
    let kinds_after = pipeline.registered_input_kinds();

    assert_eq!(enrichment_count_before, enrichment_count_after, "no enrichments registered");
    assert_eq!(kinds_before, kinds_after, "no adapters registered");

    // Assert: no specs row persisted
    let specs = store.query_specs_for_context("test").unwrap();
    assert!(specs.is_empty(), "no spec row should be persisted");
}

/// Scenario: valid spec loads successfully
///
/// Given a context "test" exists
/// And a valid declarative adapter spec YAML with adapter ID "trellis",
///   input kind "content", and a lens section
/// When `load_spec` is called with context "test" and the spec YAML
/// Then the result contains `adapter_id: "trellis"`
/// And the result contains the lens namespace
/// And the adapter is available for ingest routing on context "test"
#[tokio::test]
async fn load_spec_valid_spec_succeeds() {
    use plexus::adapter::PipelineBuilder;
    use plexus::storage::{GraphStore, OpenStore, SqliteStore};
    use plexus::{Context, PlexusApi, PlexusEngine};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store.clone()));

    let ctx = Context::new("test");
    let ctx_id = ctx.id.clone();
    engine.upsert_context(ctx).expect("upsert");

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_default_enrichments()
            .build(),
    );
    let api = PlexusApi::new(engine.clone(), pipeline.clone());

    let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;

    let result = api.load_spec("test", spec_yaml).await;
    assert!(result.is_ok(), "valid spec should load: {:?}", result.err());

    let load_result = result.unwrap();
    assert_eq!(load_result.adapter_id, "trellis-content");
    assert_eq!(load_result.lens_namespace.as_deref(), Some("lens:trellis"));

    // Adapter should be registered for routing
    let kinds = pipeline.registered_input_kinds();
    assert!(
        kinds.iter().any(|k| k == "trellis.fragment"),
        "adapter should be registered for routing, got: {:?}",
        kinds
    );

    // Spec should be persisted keyed by UUID (rename-safe, ADR-037 §2)
    let specs = store.query_specs_for_context(ctx_id.as_str()).unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].adapter_id, "trellis-content");
    assert_eq!(specs[0].context_id, ctx_id.as_str());
}

// ---------------------------------------------------------------------------
// Feature: Lens Enrichment Execution (ADR-037 §§1,3)
// ---------------------------------------------------------------------------

/// Scenario: lens runs immediately on existing graph content
///
/// Given a context "test" with existing concept nodes and `may_be_related` edges
/// When `load_spec` is called with a spec containing a lens that translates
///   `may_be_related` to `lens:trellis:thematic_connection`
/// Then the result reports the number of vocabulary edges created
/// And the graph contains `lens:trellis:thematic_connection` edges
#[tokio::test]
async fn load_spec_lens_runs_on_existing_content() {
    use plexus::adapter::{PipelineBuilder, FragmentInput};
    use plexus::storage::{OpenStore, SqliteStore};
    use plexus::{Context, NodeId, PlexusApi, PlexusEngine};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let ctx = Context::new("test");
    let ctx_id = ctx.id.clone();
    engine.upsert_context(ctx).expect("upsert");

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_default_enrichments()
            .build(),
    );
    let api = PlexusApi::new(engine.clone(), pipeline.clone());

    // Ingest content that produces may_be_related edges via co-occurrence
    let input = FragmentInput::new(
        "Graph structures in knowledge systems",
        vec!["graphs".into(), "knowledge".into()],
    );
    api.ingest("test", "content", Box::new(input)).await.expect("ingest");

    // Verify may_be_related edges exist before lens
    let ctx_before = engine.get_context(&ctx_id).unwrap();
    let has_may_be_related = ctx_before.edges.iter().any(|e| e.relationship == "may_be_related");
    assert!(has_may_be_related, "co-occurrence should have created may_be_related edges");

    // Load spec with lens
    let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    let result = api.load_spec("test", spec_yaml).await.expect("load_spec");

    // Lens should have created vocabulary edges
    assert!(result.vocabulary_edges_created > 0, "lens should create vocabulary edges");

    // Verify the graph contains translated edges
    let ctx_after = engine.get_context(&ctx_id).unwrap();
    let lens_edges: Vec<_> = ctx_after.edges.iter()
        .filter(|e| e.relationship.starts_with("lens:trellis:thematic_connection"))
        .collect();
    assert!(
        !lens_edges.is_empty(),
        "graph should contain lens:trellis:thematic_connection edges"
    );
}

// ---------------------------------------------------------------------------
// Feature: Spec Unloading (ADR-037 §6)
// ---------------------------------------------------------------------------

/// Scenario: unload_spec preserves vocabulary edges
///
/// Given a context "test" with a loaded spec for "trellis" and existing vocabulary edges
/// When `unload_spec` is called
/// Then the vocabulary edges remain in the graph
/// And the adapter is deregistered
/// And the spec is removed from the specs table
#[tokio::test]
async fn unload_spec_preserves_vocabulary_edges() {
    use plexus::adapter::{PipelineBuilder, FragmentInput};
    use plexus::storage::{GraphStore, OpenStore, SqliteStore};
    use plexus::{Context, PlexusApi, PlexusEngine};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store.clone()));

    let ctx = Context::new("test");
    let ctx_id = ctx.id.clone();
    engine.upsert_context(ctx).expect("upsert");

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_default_enrichments()
            .build(),
    );
    let api = PlexusApi::new(engine.clone(), pipeline.clone());

    // Ingest content, then load spec with lens
    let input = FragmentInput::new(
        "Graph structures in knowledge systems",
        vec!["graphs".into(), "knowledge".into()],
    );
    api.ingest("test", "content", Box::new(input)).await.expect("ingest");

    let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    api.load_spec("test", spec_yaml).await.expect("load_spec");

    // Count vocabulary edges before unload
    let ctx_before = engine.get_context(&ctx_id).unwrap();
    let lens_edge_count_before = ctx_before.edges.iter()
        .filter(|e| e.relationship.starts_with("lens:trellis:"))
        .count();
    assert!(lens_edge_count_before > 0, "should have vocabulary edges before unload");

    // Unload the spec
    api.unload_spec("test", "trellis-content").expect("unload_spec");

    // Vocabulary edges should still be in the graph (Invariant 62)
    let ctx_after = engine.get_context(&ctx_id).unwrap();
    let lens_edge_count_after = ctx_after.edges.iter()
        .filter(|e| e.relationship.starts_with("lens:trellis:"))
        .count();
    assert_eq!(
        lens_edge_count_before, lens_edge_count_after,
        "vocabulary edges should survive unload (Invariant 62)"
    );

    // Adapter should be deregistered
    let kinds = pipeline.registered_input_kinds();
    assert!(
        !kinds.iter().any(|k| k == "trellis.fragment"),
        "adapter should be deregistered after unload"
    );

    // Spec should be removed from storage
    let specs = store.query_specs_for_context("test").unwrap();
    assert!(specs.is_empty(), "spec row should be deleted after unload");
}

// ---------------------------------------------------------------------------
// Feature: Integration — End-to-End Consumer Workflow
// ---------------------------------------------------------------------------

/// Scenario: second consumer adds vocabulary layer to existing context
///
/// This is the e2e acceptance criterion from product discovery:
/// load two specs with different lenses onto the same context, ingest
/// content through both, and verify both vocabulary layers are queryable.
#[tokio::test]
async fn two_consumers_two_lenses_on_same_context() {
    use plexus::adapter::{PipelineBuilder, FragmentInput};
    use plexus::storage::{OpenStore, SqliteStore};
    use plexus::{Context, FindQuery, PlexusApi, PlexusEngine, QueryFilter};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let ctx = Context::new("shared");
    let ctx_id = ctx.id.clone();
    engine.upsert_context(ctx).expect("upsert");

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_default_enrichments()
            .build(),
    );
    let api = PlexusApi::new(engine.clone(), pipeline.clone());

    // Consumer 1: Trellis — translates may_be_related → thematic_connection
    let trellis_spec = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    api.load_spec("shared", trellis_spec).await.expect("load trellis spec");

    // Consumer 2: Carrel — translates may_be_related → citation_link
    let carrel_spec = r#"
adapter_id: carrel-content
input_kind: carrel.citation
lens:
  consumer: carrel
  translations:
    - from: [may_be_related]
      to: citation_link
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    api.load_spec("shared", carrel_spec).await.expect("load carrel spec");

    // Ingest content via the default content adapter (produces co-occurrence → may_be_related)
    let input = FragmentInput::new(
        "Graph structures in knowledge systems",
        vec!["graphs".into(), "knowledge".into()],
    );
    api.ingest("shared", "content", Box::new(input)).await.expect("ingest");

    // Verify: both vocabulary layers exist in the graph
    let ctx = engine.get_context(&ctx_id).unwrap();

    let trellis_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.relationship.starts_with("lens:trellis:"))
        .collect();
    let carrel_edges: Vec<_> = ctx.edges.iter()
        .filter(|e| e.relationship.starts_with("lens:carrel:"))
        .collect();

    assert!(
        !trellis_edges.is_empty(),
        "trellis vocabulary layer should exist"
    );
    assert!(
        !carrel_edges.is_empty(),
        "carrel vocabulary layer should exist"
    );

    // Verify: queryable via relationship_prefix filter
    let trellis_results = api.find_nodes("shared", FindQuery {
        filter: Some(QueryFilter {
            relationship_prefix: Some("lens:trellis".into()),
            ..Default::default()
        }),
        ..Default::default()
    }).unwrap();

    let carrel_results = api.find_nodes("shared", FindQuery {
        filter: Some(QueryFilter {
            relationship_prefix: Some("lens:carrel".into()),
            ..Default::default()
        }),
        ..Default::default()
    }).unwrap();

    assert!(
        !trellis_results.nodes.is_empty(),
        "find_nodes with lens:trellis prefix should return results"
    );
    assert!(
        !carrel_results.nodes.is_empty(),
        "find_nodes with lens:carrel prefix should return results"
    );
}

// ---------------------------------------------------------------------------
// Feature: Startup Rehydration via Host + Builder (ADR-037 §2, WP-D)
// ---------------------------------------------------------------------------

/// Scenario: persisted specs re-register on startup, and lens fires after restart.
///
/// This is the canonical test for Invariant 62 effect (b): a persisted spec's
/// lens enrichment transiently runs on behalf of the context when ANY library
/// instance is constructed against that context, regardless of which consumer
/// originally loaded the spec.
///
/// Process:
///   1. Consumer 1 opens an on-disk SQLite store, constructs a pipeline,
///      and loads a spec with a lens. Vocabulary edges are written.
///   2. Consumer 1 drops the api, pipeline, and engine.
///   3. Consumer 2 opens the same SQLite store (fresh process), constructs a
///      default pipeline — which gathers persisted specs and rehydrates the
///      lens enrichment at build time.
///   4. Consumer 2 ingests new content via the default content adapter. The
///      rehydrated lens fires during the enrichment loop, producing new
///      vocabulary edges for the new content.
#[tokio::test]
async fn persisted_spec_rehydrates_across_restart() {
    use plexus::adapter::{FragmentInput, PipelineBuilder};
    use plexus::storage::{OpenStore, SqliteStore};
    use plexus::{Context, PlexusApi, PlexusEngine};
    use std::sync::Arc;
    use tempfile::TempDir;

    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("plexus.db");

    // ── Consumer 1 lifetime: load spec, then drop everything ─────────────
    {
        let store = Arc::new(SqliteStore::open(&db_path).expect("sqlite open"));
        let engine = Arc::new(PlexusEngine::with_store(store));

        engine.upsert_context(Context::new("shared")).expect("upsert");

        let pipeline = Arc::new(
            PipelineBuilder::new(engine.clone())
                .with_default_adapters()
                .with_default_enrichments()
                .build(),
        );
        let api = PlexusApi::new(engine.clone(), pipeline);

        let trellis_spec = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
        api.load_spec("shared", trellis_spec)
            .await
            .expect("load_spec should succeed on consumer 1");
    }

    // ── Consumer 2 lifetime: reopen store, rehydrate via default_pipeline,
    //    ingest via a different adapter, assert lens fires ────────────────
    let store = Arc::new(SqliteStore::open(&db_path).expect("sqlite reopen"));
    let engine = Arc::new(PlexusEngine::with_store(store));
    engine.load_all().expect("load_all");

    // Host-level pipeline construction: default_pipeline gathers persisted
    // specs and rehydrates their lens enrichments. This is what host code
    // (run_mcp_server, CLI commands) is expected to invoke.
    let pipeline = Arc::new(PipelineBuilder::default_pipeline(engine.clone(), None));
    let api = PlexusApi::new(engine.clone(), pipeline);

    // Ingest via the default content adapter (NOT the trellis adapter —
    // consumer 2 is a different consumer that doesn't know about trellis).
    // co_occurrence produces may_be_related; rehydrated trellis lens should
    // translate those to lens:trellis:thematic_connection.
    let input = FragmentInput::new(
        "Graph structures and knowledge systems reinforce each other",
        vec!["graphs".into(), "knowledge".into()],
    );
    api.ingest("shared", "content", Box::new(input))
        .await
        .expect("ingest on consumer 2");

    // Assert: the persisted trellis lens fired on consumer 2's new content
    let ctx_id = engine
        .resolve_by_name("shared")
        .expect("shared context should resolve post-restart");
    let ctx_after = engine
        .get_context(&ctx_id)
        .expect("context should be loaded");
    let trellis_edges: Vec<_> = ctx_after
        .edges
        .iter()
        .filter(|e| e.relationship.starts_with("lens:trellis:thematic_connection"))
        .collect();
    assert!(
        !trellis_edges.is_empty(),
        "persisted trellis lens should have fired on consumer 2's new content — \
         found edges: {:?}",
        ctx_after
            .edges
            .iter()
            .map(|e| e.relationship.as_str())
            .collect::<Vec<_>>()
    );
}
