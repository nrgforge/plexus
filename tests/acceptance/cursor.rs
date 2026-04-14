//! Acceptance tests for event cursor persistence (ADR-035).
//!
//! Scenarios from docs/scenarios/033-035-query-surface.md §Event Cursor Persistence.

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::query::CursorFilter;

fn fragment(text: &str, tags: Vec<&str>) -> Box<FragmentInput> {
    Box::new(FragmentInput::new(
        text,
        tags.into_iter().map(|t| t.to_string()).collect(),
    ))
}

/// Scenario: Events are persisted with sequence numbers after emission
#[tokio::test]
async fn events_persisted_with_sequence_numbers_after_emission() {
    let env = TestEnv::new();

    env.api
        .ingest(env.ctx_name(), "content", fragment("Test fragment", vec!["travel", "avignon"]))
        .await
        .unwrap();

    let changeset = env.api.changes_since(&env.context_name, 0, None).unwrap();
    assert!(
        !changeset.events.is_empty(),
        "events should be persisted after emission"
    );

    // Verify sequence numbers are monotonically increasing
    for window in changeset.events.windows(2) {
        assert!(
            window[1].sequence > window[0].sequence,
            "sequences must be monotonically increasing"
        );
    }

    // Verify context_id and adapter_id are populated
    for event in &changeset.events {
        assert!(!event.context_id.is_empty());
        assert!(!event.adapter_id.is_empty());
    }
}

/// Scenario: changes_since returns events after the given cursor
#[tokio::test]
async fn changes_since_returns_events_after_cursor() {
    let env = TestEnv::new();

    // First ingest
    env.api
        .ingest(env.ctx_name(), "content", fragment("first", vec!["alpha"]))
        .await
        .unwrap();

    let cs1 = env.api.changes_since(&env.context_name, 0, None).unwrap();
    let cursor = cs1.latest_sequence;
    assert!(cursor > 0);

    // Second ingest
    env.api
        .ingest(env.ctx_name(), "content", fragment("second", vec!["beta"]))
        .await
        .unwrap();

    let cs2 = env.api.changes_since(&env.context_name, cursor, None).unwrap();
    assert!(!cs2.events.is_empty(), "should have events from second ingest");
    assert!(cs2.latest_sequence > cursor, "latest_sequence should advance");

    for event in &cs2.events {
        assert!(event.sequence > cursor, "all events should be after cursor");
    }
}

/// Scenario: changes_since with cursor 0 returns all events
#[tokio::test]
async fn changes_since_cursor_zero_returns_all() {
    let env = TestEnv::new();

    env.api
        .ingest(env.ctx_name(), "content", fragment("fragment", vec!["gamma"]))
        .await
        .unwrap();

    let cs = env.api.changes_since(&env.context_name, 0, None).unwrap();
    assert!(!cs.events.is_empty(), "cursor 0 should return all events");
    assert!(cs.events[0].sequence >= 1);
}

/// Scenario: CursorFilter scopes by event_type
#[tokio::test]
async fn cursor_filter_scopes_by_event_type() {
    let env = TestEnv::new();

    env.api
        .ingest(env.ctx_name(), "content", fragment("fragment", vec!["delta", "epsilon"]))
        .await
        .unwrap();

    let all = env.api.changes_since(&env.context_name, 0, None).unwrap();
    let has_nodes = all.events.iter().any(|e| e.event_type == "NodesAdded");
    let has_edges = all.events.iter().any(|e| e.event_type == "EdgesAdded");
    assert!(has_nodes, "should have NodesAdded events");
    assert!(has_edges, "should have EdgesAdded events");

    let filter = CursorFilter {
        event_types: Some(vec!["EdgesAdded".to_string()]),
        ..Default::default()
    };
    let filtered = env.api.changes_since(&env.context_name, 0, Some(&filter)).unwrap();
    assert!(!filtered.events.is_empty());
    for event in &filtered.events {
        assert_eq!(event.event_type, "EdgesAdded");
    }
}

/// Scenario: CursorFilter scopes by adapter_id
#[tokio::test]
async fn cursor_filter_scopes_by_adapter_id() {
    let env = TestEnv::new();

    env.api
        .ingest(env.ctx_name(), "content", fragment("fragment", vec!["zeta", "eta"]))
        .await
        .unwrap();

    let filter = CursorFilter {
        adapter_id: Some("content-adapter".to_string()),
        ..Default::default()
    };
    let filtered = env.api.changes_since(&env.context_name, 0, Some(&filter)).unwrap();
    for event in &filtered.events {
        assert_eq!(event.adapter_id, "content-adapter");
    }
}

/// Scenario: changes_since with no new events returns empty result
#[tokio::test]
async fn changes_since_no_new_events_returns_empty() {
    let env = TestEnv::new();

    env.api
        .ingest(env.ctx_name(), "content", fragment("fragment", vec!["theta"]))
        .await
        .unwrap();

    let cs = env.api.changes_since(&env.context_name, 0, None).unwrap();
    let cursor = cs.latest_sequence;

    let cs2 = env.api.changes_since(&env.context_name, cursor, None).unwrap();
    assert_eq!(cs2.events.len(), 0);
    assert_eq!(cs2.latest_sequence, cursor);
}

/// Scenario: Event log survives persistence round-trip
#[tokio::test]
async fn event_log_survives_round_trip() {
    use plexus::adapter::PipelineBuilder;
    use plexus::storage::{OpenStore, SqliteStore};
    use plexus::{Context, PlexusApi, PlexusEngine};
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("roundtrip.db");
    let context_name = "roundtrip-test";

    // Phase 1: Write events
    {
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store));
        engine.upsert_context(Context::new(context_name)).unwrap();

        let pipeline = Arc::new(
            PipelineBuilder::new(engine.clone())
                .with_default_adapters()
                .with_default_enrichments()
                .build(),
        );
        let api = PlexusApi::new(engine.clone(), pipeline);

        api.ingest(context_name, "content", fragment("fragment", vec!["iota"]))
            .await
            .unwrap();

        let cs = api.changes_since(context_name, 0, None).unwrap();
        assert!(!cs.events.is_empty(), "events written in phase 1");
    }

    // Phase 2: Reopen and verify
    {
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store));
        engine.load_all().unwrap();

        let pipeline = Arc::new(
            PipelineBuilder::new(engine.clone())
                .with_default_adapters()
                .with_default_enrichments()
                .build(),
        );
        let api = PlexusApi::new(engine, pipeline);

        let cs = api.changes_since(context_name, 0, None).unwrap();
        assert!(!cs.events.is_empty(), "events must survive engine restart");
    }
}
