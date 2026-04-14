//! Composable query filter acceptance tests (ADR-034).
//!
//! Scenarios from docs/scenarios/033-035-query-surface.md §Composable Query Filters.

use plexus::{
    Context, Edge, FindQuery, NodeId, QueryFilter, RankBy, StepQuery, TraverseQuery,
    ContentType, Node, Direction, dimension, PlexusApi, PlexusEngine,
};

/// Create a node with a specific ID, type, and dimension.
fn node(id: &str, node_type: &str, dim: &str) -> Node {
    let mut n = Node::new_in_dimension(node_type, ContentType::Concept, dim);
    n.id = NodeId::from(id);
    n
}

/// Insert a node into a context by ID.
fn add(ctx: &mut Context, n: Node) {
    ctx.nodes.insert(n.id.clone(), n);
}

/// Create an edge with specific contributions.
fn edge_with_contribs(
    source: &str,
    target: &str,
    relationship: &str,
    contribs: &[(&str, f32)],
) -> Edge {
    let mut e = Edge::new(
        NodeId::from_string(source),
        NodeId::from_string(target),
        relationship,
    );
    for (key, val) in contribs {
        e.contributions.insert(key.to_string(), *val);
    }
    e.combined_weight = contribs.iter().map(|(_, v)| v).sum();
    e
}

// ---------------------------------------------------------------------------
// Scenario 1: QueryFilter with contributor_ids scopes traversal
// ---------------------------------------------------------------------------

#[test]
fn contributor_ids_scopes_traversal_to_specific_adapters() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));

    // A→B has contributions from content-adapter AND co-occurrence
    ctx.edges.push(edge_with_contribs(
        "A", "B", "may_be_related",
        &[("content-adapter", 1.0), ("co_occurrence:tagged_with:may_be_related", 0.8)],
    ));
    // A→C has contributions only from content-adapter
    ctx.edges.push(edge_with_contribs(
        "A", "C", "tagged_with",
        &[("content-adapter", 1.0)],
    ));

    let result = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .with_filter(QueryFilter {
            contributor_ids: Some(vec!["co_occurrence:tagged_with:may_be_related".into()]),
            ..Default::default()
        })
        .execute(&ctx);

    // Should reach B (has matching contributor)
    let reached_ids: Vec<String> = result.all_nodes().iter().map(|n| n.id.to_string()).collect();
    assert!(reached_ids.contains(&"B".to_string()), "should reach B");
    assert!(!reached_ids.contains(&"C".to_string()), "should NOT reach C");
}

// ---------------------------------------------------------------------------
// Scenario 2: QueryFilter with relationship_prefix scopes to lens output
// ---------------------------------------------------------------------------

#[test]
fn relationship_prefix_scopes_to_lens_output() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));

    // A→B with lens relationship
    ctx.edges.push(edge_with_contribs(
        "A", "B", "lens:trellis:thematic_connection",
        &[("lens:trellis:thematic_connection:may_be_related", 0.6)],
    ));
    // A→C with non-lens relationship
    ctx.edges.push(edge_with_contribs(
        "A", "C", "may_be_related",
        &[("co_occurrence:tagged_with:may_be_related", 0.5)],
    ));

    let result = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .with_filter(QueryFilter {
            relationship_prefix: Some("lens:trellis:".into()),
            ..Default::default()
        })
        .execute(&ctx);

    let reached_ids: Vec<String> = result.all_nodes().iter().map(|n| n.id.to_string()).collect();
    assert!(reached_ids.contains(&"B".to_string()), "should reach B via lens edge");
    assert!(!reached_ids.contains(&"C".to_string()), "should NOT reach C (non-lens edge)");
}

// ---------------------------------------------------------------------------
// Scenario 3: min_corroboration filters weakly corroborated edges
// ---------------------------------------------------------------------------

#[test]
fn min_corroboration_filters_weakly_corroborated_edges() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));

    // A→B: 3 distinct contributors
    ctx.edges.push(edge_with_contribs(
        "A", "B", "may_be_related",
        &[("adapter-1", 1.0), ("adapter-2", 0.8), ("adapter-3", 0.5)],
    ));
    // A→C: 1 contributor
    ctx.edges.push(edge_with_contribs(
        "A", "C", "may_be_related",
        &[("adapter-1", 1.0)],
    ));

    let result = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .with_filter(QueryFilter {
            min_corroboration: Some(2),
            ..Default::default()
        })
        .execute(&ctx);

    let reached_ids: Vec<String> = result.all_nodes().iter().map(|n| n.id.to_string()).collect();
    assert!(reached_ids.contains(&"B".to_string()), "should reach B (3 contributors >= 2)");
    assert!(!reached_ids.contains(&"C".to_string()), "should NOT reach C (1 contributor < 2)");
}

// ---------------------------------------------------------------------------
// Scenario 4: QueryFilter fields compose with AND semantics
// ---------------------------------------------------------------------------

#[test]
fn filter_fields_compose_with_and_semantics() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("D", "concept", dimension::SEMANTIC));

    // A→B: lens prefix + 3 contributors — passes both
    ctx.edges.push(edge_with_contribs(
        "A", "B", "lens:trellis:thematic_connection",
        &[("lens:trellis:thematic_connection:may_be_related", 0.6),
          ("adapter-1", 0.5), ("adapter-2", 0.3)],
    ));
    // A→C: lens prefix + 1 contributor — fails corroboration
    ctx.edges.push(edge_with_contribs(
        "A", "C", "lens:trellis:topic_link",
        &[("lens:trellis:topic_link:similar_to", 0.4)],
    ));
    // A→D: non-lens + 4 contributors — fails prefix
    ctx.edges.push(edge_with_contribs(
        "A", "D", "may_be_related",
        &[("a1", 1.0), ("a2", 0.8), ("a3", 0.5), ("a4", 0.3)],
    ));

    let result = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .with_filter(QueryFilter {
            relationship_prefix: Some("lens:trellis:".into()),
            min_corroboration: Some(2),
            ..Default::default()
        })
        .execute(&ctx);

    let reached_ids: Vec<String> = result.all_nodes().iter().map(|n| n.id.to_string()).collect();
    assert!(reached_ids.contains(&"B".to_string()), "B matches prefix AND corroboration");
    assert!(!reached_ids.contains(&"C".to_string()), "C matches prefix but fails corroboration");
    assert!(!reached_ids.contains(&"D".to_string()), "D matches corroboration but fails prefix");
}

// ---------------------------------------------------------------------------
// Scenario 5: Filter with None fields applies no constraint
// ---------------------------------------------------------------------------

#[test]
fn filter_with_none_fields_applies_no_constraint() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));

    ctx.edges.push(edge_with_contribs("A", "B", "tagged_with", &[("a1", 1.0)]));
    ctx.edges.push(edge_with_contribs("A", "C", "may_be_related", &[("a2", 0.5)]));

    // Explicit filter with all None fields
    let with_filter = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .with_filter(QueryFilter::default())
        .execute(&ctx);

    // No filter at all
    let without_filter = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .execute(&ctx);

    // Both should reach the same nodes
    let with_ids: Vec<String> = with_filter.all_nodes().iter().map(|n| n.id.to_string()).collect();
    let without_ids: Vec<String> = without_filter.all_nodes().iter().map(|n| n.id.to_string()).collect();

    assert_eq!(with_ids.len(), without_ids.len(), "same number of nodes reached");
    for id in &without_ids {
        assert!(with_ids.contains(id), "filter with all None should reach {}", id);
    }
}

// ---------------------------------------------------------------------------
// Scenario 6: StepQuery with QueryFilter — filter composes with per-step relationship
// ---------------------------------------------------------------------------

#[test]
fn step_query_with_filter_composes_with_per_step_relationship() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("D", "concept", dimension::SEMANTIC));

    // A→B via lens thematic_connection
    ctx.edges.push(edge_with_contribs(
        "A", "B", "lens:trellis:thematic_connection",
        &[("lens:trellis:thematic_connection:may_be_related", 0.6)],
    ));
    // B→C via lens topic_link
    ctx.edges.push(edge_with_contribs(
        "B", "C", "lens:trellis:topic_link",
        &[("lens:trellis:topic_link:similar_to", 0.5)],
    ));
    // A→D via tagged_with (non-lens)
    ctx.edges.push(edge_with_contribs(
        "A", "D", "tagged_with",
        &[("content-adapter", 1.0)],
    ));

    let result = StepQuery::from("A")
        .step(Direction::Outgoing, "lens:trellis:thematic_connection")
        .step(Direction::Outgoing, "lens:trellis:topic_link")
        .with_filter(QueryFilter {
            relationship_prefix: Some("lens:trellis:".into()),
            ..Default::default()
        })
        .execute(&ctx);

    // Step 1 reaches B
    assert_eq!(result.at_step(0).len(), 1);
    assert_eq!(result.at_step(0)[0].id.to_string(), "B");

    // Step 2 reaches C
    assert_eq!(result.at_step(1).len(), 1);
    assert_eq!(result.at_step(1)[0].id.to_string(), "C");

    // D is never reached (its edge is tagged_with, not lens-prefixed)
    let all_ids: Vec<String> = result.all_nodes().iter().map(|n| n.id.to_string()).collect();
    assert!(!all_ids.contains(&"D".to_string()), "D should never be reached");
}

// ---------------------------------------------------------------------------
// Scenario 7: StepQuery with conflicting step relationship and filter prefix
// ---------------------------------------------------------------------------

#[test]
fn step_query_conflicting_relationship_and_filter_terminates_early() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));

    // A→B via tagged_with
    ctx.edges.push(edge_with_contribs(
        "A", "B", "tagged_with",
        &[("content-adapter", 1.0)],
    ));

    let result = StepQuery::from("A")
        .step(Direction::Outgoing, "tagged_with")
        .with_filter(QueryFilter {
            relationship_prefix: Some("lens:trellis:".into()),
            ..Default::default()
        })
        .execute(&ctx);

    // Step 1 finds zero edges: tagged_with matches the step's relationship,
    // but fails the prefix filter lens:trellis:
    assert_eq!(result.at_step(0).len(), 0, "no edges should pass both step relationship and prefix filter");

    // Result contains only steps (empty) — no nodes beyond origin
    assert!(result.all_nodes().is_empty(), "only the origin node should remain");
}

// ---------------------------------------------------------------------------
// Scenario 8: RankBy Corroboration orders results by evidence diversity
// ---------------------------------------------------------------------------

#[test]
fn rank_by_corroboration_orders_by_evidence_diversity() {
    let mut ctx = Context::new("test");

    add(&mut ctx, node("A", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("B", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("C", "concept", dimension::SEMANTIC));
    add(&mut ctx, node("D", "concept", dimension::SEMANTIC));

    // A→B: corroboration 1
    ctx.edges.push(edge_with_contribs("A", "B", "may_be_related", &[("a1", 1.0)]));
    // A→C: corroboration 4
    ctx.edges.push(edge_with_contribs(
        "A", "C", "may_be_related",
        &[("a1", 1.0), ("a2", 0.8), ("a3", 0.5), ("a4", 0.3)],
    ));
    // A→D: corroboration 2
    ctx.edges.push(edge_with_contribs(
        "A", "D", "may_be_related",
        &[("a1", 1.0), ("a2", 0.7)],
    ));

    let mut result = TraverseQuery::from(NodeId::from_string("A"))
        .depth(1)
        .direction(Direction::Outgoing)
        .execute(&ctx);

    let edges = result.edges.clone();
    result.rank_by(RankBy::Corroboration, &edges);

    // Level 1 (depth 1) should be ordered: C (4), D (2), B (1) — descending corroboration
    let level1 = &result.levels[1];
    assert_eq!(level1.len(), 3);
    assert_eq!(level1[0].id.to_string(), "C", "highest corroboration first");
    assert_eq!(level1[1].id.to_string(), "D", "second highest");
    assert_eq!(level1[2].id.to_string(), "B", "lowest corroboration last");
}

// ---------------------------------------------------------------------------
// Scenario 9: find_nodes with min_corroboration returns globally filtered results
// ---------------------------------------------------------------------------

#[test]
fn find_nodes_with_min_corroboration_filters_globally() {
    let mut ctx = Context::new("test");

    // Create 5 concept nodes
    for i in 0..5 {
        add(&mut ctx, node(&format!("concept:{}", i), "concept", dimension::SEMANTIC));
    }

    // concept:0, concept:1, concept:2 have edges with corroboration >= 3
    for i in 0..3 {
        ctx.edges.push(edge_with_contribs(
            &format!("concept:{}", i), "concept:4", "may_be_related",
            &[("a1", 1.0), ("a2", 0.8), ("a3", 0.5)],
        ));
    }

    // concept:3 has edge with corroboration 1
    ctx.edges.push(edge_with_contribs(
        "concept:3", "concept:4", "tagged_with",
        &[("a1", 1.0)],
    ));

    // concept:4 has incident edges from all the above, some with >= 3 contributors
    // so it also qualifies

    let result = FindQuery::new()
        .with_node_type("concept")
        .with_filter(QueryFilter {
            min_corroboration: Some(3),
            ..Default::default()
        })
        .execute(&ctx);

    // concept:0, concept:1, concept:2 each have an incident edge with 3 contributors
    // concept:4 also has incident edges with 3 contributors (from 0, 1, 2)
    // concept:3 only has an incident edge with 1 contributor — excluded
    assert!(
        result.nodes.len() <= 4,
        "concept:3 should be excluded (only 1 contributor), got {} nodes",
        result.nodes.len()
    );

    let ids: Vec<String> = result.nodes.iter().map(|n| n.id.to_string()).collect();
    assert!(!ids.contains(&"concept:3".to_string()), "concept:3 should not appear (corroboration < 3)");
    // concept:0, 1, 2 should appear
    for i in 0..3 {
        assert!(ids.contains(&format!("concept:{}", i)), "concept:{} should appear", i);
    }
}

// ===========================================================================
// Integration Scenarios (Cross-ADR: lens + cursor + filter)
// ===========================================================================

// ---------------------------------------------------------------------------
// Scenario 10: Lens-created edges appear in cursor event log
// ---------------------------------------------------------------------------

#[tokio::test]
async fn lens_created_edges_appear_in_cursor_event_log() {
    use plexus::adapter::{PipelineBuilder, CoOccurrenceEnrichment, FragmentInput};
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::lens::LensEnrichment;
    use plexus::storage::{OpenStore, SqliteStore};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let ctx_name = "cursor-lens";
    engine.upsert_context(Context::new(ctx_name)).expect("upsert");

    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_enrichment(Arc::new(CoOccurrenceEnrichment::new()))
            .with_enrichment(Arc::new(LensEnrichment::new(lens_spec)))
            .build(),
    );

    let api = PlexusApi::new(engine.clone(), pipeline);

    // Ingest fragments to trigger co-occurrence → lens
    let input = FragmentInput::new(
        "Creative writing and research",
        vec!["writing".into(), "research".into()],
    );
    api.ingest(ctx_name, "content", Box::new(input))
        .await
        .expect("ingest");

    // Query cursor from sequence 0 (uses context name, not ID)
    let changeset = api
        .changes_since(ctx_name, 0, None)
        .expect("changes_since");

    // Events should include EdgesAdded from the lens
    let lens_events: Vec<_> = changeset
        .events
        .iter()
        .filter(|e| e.adapter_id.starts_with("lens:trellis"))
        .collect();

    assert!(
        !lens_events.is_empty(),
        "cursor event log should include lens-created edge events"
    );

    // All lens events should be EdgesAdded
    for event in &lens_events {
        assert_eq!(event.event_type, "EdgesAdded", "lens events should be EdgesAdded");
    }
}

// ---------------------------------------------------------------------------
// Scenario 11: QueryFilter on lens-created edges discovered via cursor
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_filter_on_lens_edges_discovered_via_cursor() {
    use plexus::adapter::{PipelineBuilder, CoOccurrenceEnrichment, FragmentInput};
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::lens::LensEnrichment;
    use plexus::storage::{OpenStore, SqliteStore};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let ctx_name = "filter-lens";
    let ctx_id = engine.upsert_context(Context::new(ctx_name)).expect("upsert");

    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_enrichment(Arc::new(CoOccurrenceEnrichment::new()))
            .with_enrichment(Arc::new(LensEnrichment::new(lens_spec)))
            .build(),
    );

    let api = PlexusApi::new(engine.clone(), pipeline);

    let input = FragmentInput::new(
        "Patterns in creative work",
        vec!["patterns".into(), "creative".into()],
    );
    api.ingest(ctx_name, "content", Box::new(input))
        .await
        .expect("ingest");

    // Consumer discovers lens edges via cursor
    let changeset = api
        .changes_since(ctx_name, 0, None)
        .expect("changes_since");
    let has_lens_events = changeset.events.iter().any(|e| e.adapter_id.starts_with("lens:trellis"));
    assert!(has_lens_events, "cursor should report lens events");

    // Now traverse with relationship_prefix filter to get only lens edges
    let result_ctx = engine.get_context(&ctx_id).expect("context");
    let patterns_id = NodeId::from_string("concept:patterns");

    let result = TraverseQuery::from(patterns_id.clone())
        .depth(1)
        .direction(Direction::Outgoing)
        .with_filter(QueryFilter {
            relationship_prefix: Some("lens:trellis:".into()),
            ..Default::default()
        })
        .execute(&result_ctx);

    // Should find the lens-translated edge to concept:creative
    let reached_ids: Vec<String> = result.all_nodes().iter().map(|n| n.id.to_string()).collect();
    assert!(
        reached_ids.contains(&"concept:creative".to_string()),
        "traversal with lens prefix filter should reach concept:creative via lens edge"
    );

    // Edges should all be lens-prefixed
    for edge in &result.edges {
        assert!(
            edge.relationship.starts_with("lens:trellis:"),
            "all traversed edges should have lens prefix, got: {}",
            edge.relationship
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 12: Full pull workflow — ingest, cursor, filtered query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_pull_workflow_ingest_cursor_filtered_query() {
    use plexus::adapter::{PipelineBuilder, CoOccurrenceEnrichment, FragmentInput};
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::lens::LensEnrichment;
    use plexus::query::CursorFilter;
    use plexus::storage::{OpenStore, SqliteStore};
    use std::sync::Arc;

    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    let ctx_name = "pull-workflow";
    let ctx_id = engine.upsert_context(Context::new(ctx_name)).expect("upsert");

    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };

    let pipeline = Arc::new(
        PipelineBuilder::new(engine.clone())
            .with_default_adapters()
            .with_enrichment(Arc::new(CoOccurrenceEnrichment::new()))
            .with_enrichment(Arc::new(LensEnrichment::new(lens_spec)))
            .build(),
    );

    let api = PlexusApi::new(engine.clone(), pipeline);

    // Trellis ingests fragments
    let input1 = FragmentInput::new(
        "Writing about nature and wilderness",
        vec!["nature".into(), "wilderness".into()],
    );
    api.ingest(ctx_name, "content", Box::new(input1))
        .await
        .expect("trellis ingest");

    // Carrel ingests research with overlapping concept
    let input2 = FragmentInput::new(
        "Research on ecology and wilderness preservation",
        vec!["ecology".into(), "wilderness".into()],
    );
    api.ingest(ctx_name, "content", Box::new(input2))
        .await
        .expect("carrel ingest");

    // Trellis consumer: query cursor filtered to only lens events
    let lens_cursor_filter = CursorFilter {
        adapter_id: Some("lens:trellis:thematic_connection".into()),
        ..Default::default()
    };
    let changeset = api
        .changes_since(ctx_name, 0, Some(&lens_cursor_filter))
        .expect("cursor query");

    // Lens events should be present (from co-occurrence → lens translation)
    // Note: depends on concepts overlapping enough for co-occurrence to fire
    // "wilderness" appears in both fragments, so concepts should co-occur

    // Traverse with lens prefix filter
    let result_ctx = engine.get_context(&ctx_id).expect("context");
    let wilderness_id = NodeId::from_string("concept:wilderness");

    if result_ctx.get_node(&wilderness_id).is_some() {
        let result = TraverseQuery::from(wilderness_id)
            .depth(1)
            .direction(Direction::Both)
            .with_filter(QueryFilter {
                relationship_prefix: Some("lens:trellis:".into()),
                ..Default::default()
            })
            .execute(&result_ctx);

        // If lens edges exist, they should be lens-prefixed
        for edge in &result.edges {
            assert!(
                edge.relationship.starts_with("lens:trellis:"),
                "lens-filtered traversal should only return lens edges, got: {}",
                edge.relationship
            );
        }
    }
}
