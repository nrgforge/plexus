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
        kinds.contains(&"trellis.fragment"),
        "adapter should be registered for input_kind 'trellis.fragment', got: {:?}",
        kinds
    );

    // Assert: enrichment registry includes declared enrichments AND lens
    let enrichment_ids: Vec<&str> = pipeline
        .enrichment_registry()
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
