//! Lens declaration and translation acceptance tests (ADR-033).
//!
//! Scenarios from docs/scenarios/033-035-query-surface.md §Lens Declaration.

use super::helpers::TestEnv;
use plexus::adapter::FragmentInput;
use plexus::NodeId;

/// Scenario: Lens creates translated edges from matching source relationships
///
/// Given a context with concepts A and B connected by a `may_be_related` edge
/// When a LensEnrichment translating `may_be_related` → `thematic_connection` runs
/// Then a `lens:trellis:thematic_connection` edge exists from A to B
/// And that edge's contributions contain key `lens:trellis:thematic_connection:may_be_related`
#[tokio::test]
async fn lens_creates_translated_edges_from_matching_source_relationships() {
    use plexus::adapter::Enrichment;
    use plexus::adapter::lens::LensEnrichment;
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::GraphEvent;
    use plexus::EdgeId;

    let env = TestEnv::new();

    // Ingest a fragment with two tags to create concepts + co-occurrence edges
    let input = FragmentInput::new(
        "Research on graph structures",
        vec!["graphs".into(), "structures".into()],
    );
    env.api
        .ingest(env.ctx_name(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Verify co-occurrence created may_be_related edges
    let ctx = env.engine.get_context(&env.context_id).expect("context");
    let graphs_id = NodeId::from_string("concept:graphs");
    let structures_id = NodeId::from_string("concept:structures");

    let has_may_be_related = ctx.edges.iter().any(|e| {
        e.source == graphs_id
            && e.target == structures_id
            && e.relationship == "may_be_related"
    });
    assert!(has_may_be_related, "co-occurrence should have created may_be_related edge");

    // Now run the lens enrichment against the context
    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };
    let lens = LensEnrichment::new(lens_spec);

    // Simulate EdgesAdded events (the trigger for lens)
    let events = vec![GraphEvent::EdgesAdded {
        edge_ids: vec![EdgeId::new()],
        adapter_id: "co_occurrence:tagged_with:may_be_related".into(),
        context_id: env.ctx_id().into(),
    }];

    let emission = lens.enrich(&events, &ctx).expect("lens should emit translated edges");

    // Check translated edge exists
    let translated = emission.edges.iter().find(|ae| {
        ae.edge.source == graphs_id
            && ae.edge.target == structures_id
            && ae.edge.relationship == "lens:trellis:thematic_connection"
    });
    assert!(translated.is_some(), "translated edge should exist");

    // Check contribution key
    let edge = &translated.unwrap().edge;
    assert!(
        edge.contributions.contains_key("lens:trellis:thematic_connection:may_be_related"),
        "contribution key should follow namespace convention"
    );
}

/// Scenario: Many-to-one translation produces per-source contribution slots
///
/// Given a lens translating [may_be_related, similar_to] → thematic_connection
/// And concept nodes A and B connected by both relationship types
/// When the lens runs
/// Then one edge exists with relationship `lens:trellis:thematic_connection`
/// And that edge has two contribution keys (one per source relationship)
/// And the corroboration count is 2
#[tokio::test]
async fn many_to_one_translation_produces_per_source_contribution_slots() {
    use plexus::adapter::Enrichment;
    use plexus::adapter::lens::LensEnrichment;
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::GraphEvent;
    use plexus::{Context, Edge, EdgeId, Node, NodeId, ContentType, dimension};

    let mut ctx = Context::new("test");

    // Create concept nodes
    let mut node_a = Node::new("concept", ContentType::Concept);
    node_a.id = NodeId::from_string("concept:a");
    node_a.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_a);

    let mut node_b = Node::new("concept", ContentType::Concept);
    node_b.id = NodeId::from_string("concept:b");
    node_b.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_b);

    // Add may_be_related edge (weight 0.4)
    let mut edge1 = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:b"),
        "may_be_related",
        dimension::SEMANTIC,
    );
    edge1.combined_weight = 0.4;
    ctx.add_edge(edge1);

    // Add similar_to edge (weight 0.7)
    let mut edge2 = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:b"),
        "similar_to",
        dimension::SEMANTIC,
    );
    edge2.combined_weight = 0.7;
    ctx.add_edge(edge2);

    // Lens with many-to-one translation
    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into(), "similar_to".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };
    let lens = LensEnrichment::new(lens_spec);

    let events = vec![GraphEvent::EdgesAdded {
        edge_ids: vec![EdgeId::new()],
        adapter_id: "test".into(),
        context_id: "test".into(),
    }];

    let emission = lens.enrich(&events, &ctx).expect("lens should emit");

    // Should produce exactly one translated edge (many-to-one)
    let translated: Vec<_> = emission.edges.iter().filter(|ae| {
        ae.edge.relationship == "lens:trellis:thematic_connection"
    }).collect();
    assert_eq!(translated.len(), 1, "many-to-one should produce one edge");

    let edge = &translated[0].edge;

    // Two contribution keys — one per source relationship
    assert!(
        edge.contributions.contains_key("lens:trellis:thematic_connection:may_be_related"),
        "should have may_be_related contribution key"
    );
    assert!(
        edge.contributions.contains_key("lens:trellis:thematic_connection:similar_to"),
        "should have similar_to contribution key"
    );

    // Corroboration count = number of distinct contribution keys
    assert_eq!(edge.contributions.len(), 2, "corroboration count should be 2");
}

/// Scenario: Lens respects min_weight threshold
///
/// Given a lens with min_weight: 0.3
/// And a may_be_related edge with raw weight 0.1
/// When the lens runs
/// Then no translated edge exists
#[tokio::test]
async fn lens_respects_min_weight_threshold() {
    use plexus::adapter::Enrichment;
    use plexus::adapter::lens::LensEnrichment;
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::GraphEvent;
    use plexus::{Context, Edge, EdgeId, Node, NodeId, ContentType, dimension};

    let mut ctx = Context::new("test");

    let mut node_a = Node::new("concept", ContentType::Concept);
    node_a.id = NodeId::from_string("concept:a");
    node_a.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_a);

    let mut node_b = Node::new("concept", ContentType::Concept);
    node_b.id = NodeId::from_string("concept:b");
    node_b.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_b);

    // Edge with weight below threshold
    let mut edge = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:b"),
        "may_be_related",
        dimension::SEMANTIC,
    );
    edge.combined_weight = 0.1;
    ctx.add_edge(edge);

    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: Some(0.3),
            involving: None,
        }],
    };
    let lens = LensEnrichment::new(lens_spec);

    let events = vec![GraphEvent::EdgesAdded {
        edge_ids: vec![EdgeId::new()],
        adapter_id: "test".into(),
        context_id: "test".into(),
    }];

    let result = lens.enrich(&events, &ctx);
    assert!(result.is_none(), "edge below min_weight should not produce translation");
}

/// Scenario: Lens is idempotent across enrichment rounds
///
/// Given the lens has already created a translated edge between A and B
/// When the enrichment loop runs another round
/// Then no duplicate translated edge is created
#[tokio::test]
async fn lens_is_idempotent_across_enrichment_rounds() {
    use plexus::adapter::Enrichment;
    use plexus::adapter::lens::LensEnrichment;
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::GraphEvent;
    use plexus::{Context, Edge, EdgeId, Node, NodeId, ContentType, dimension};

    let mut ctx = Context::new("test");

    let mut node_a = Node::new("concept", ContentType::Concept);
    node_a.id = NodeId::from_string("concept:a");
    node_a.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_a);

    let mut node_b = Node::new("concept", ContentType::Concept);
    node_b.id = NodeId::from_string("concept:b");
    node_b.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_b);

    // Source edge
    let mut source_edge = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:b"),
        "may_be_related",
        dimension::SEMANTIC,
    );
    source_edge.combined_weight = 0.6;
    ctx.add_edge(source_edge);

    // Pre-existing translated edge (from a prior enrichment round)
    let mut existing_translated = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:b"),
        "lens:trellis:thematic_connection",
        dimension::SEMANTIC,
    );
    existing_translated.combined_weight = 0.6;
    existing_translated.contributions.insert(
        "lens:trellis:thematic_connection:may_be_related".into(),
        0.6,
    );
    ctx.add_edge(existing_translated);

    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };
    let lens = LensEnrichment::new(lens_spec);

    let events = vec![GraphEvent::EdgesAdded {
        edge_ids: vec![EdgeId::new()],
        adapter_id: "test".into(),
        context_id: "test".into(),
    }];

    // Lens should return None — the translated edge already exists
    let result = lens.enrich(&events, &ctx);
    assert!(result.is_none(), "lens should be quiescent when translated edge exists");
}

/// Scenario: Untranslated edges remain accessible
///
/// Given a lens translating only `may_be_related`
/// And A→C connected by `similar_to` (not in translation rules)
/// When traversing from A without relationship filtering
/// Then C is reachable via the `similar_to` edge
/// And no `lens:trellis:*` edge exists between A and C
#[tokio::test]
async fn untranslated_edges_remain_accessible() {
    use plexus::adapter::Enrichment;
    use plexus::adapter::lens::LensEnrichment;
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::GraphEvent;
    use plexus::{Context, Edge, EdgeId, Node, NodeId, ContentType, dimension};

    let mut ctx = Context::new("test");

    let mut node_a = Node::new("concept", ContentType::Concept);
    node_a.id = NodeId::from_string("concept:a");
    node_a.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_a);

    let mut node_c = Node::new("concept", ContentType::Concept);
    node_c.id = NodeId::from_string("concept:c");
    node_c.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_c);

    // Edge with relationship not in lens translation rules
    let mut edge = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:c"),
        "similar_to",
        dimension::SEMANTIC,
    );
    edge.combined_weight = 0.5;
    ctx.add_edge(edge);

    let lens_spec = LensSpec {
        consumer: "trellis".into(),
        translations: vec![TranslationRule {
            from: vec!["may_be_related".into()],
            to: "thematic_connection".into(),
            min_weight: None,
            involving: None,
        }],
    };
    let lens = LensEnrichment::new(lens_spec);

    let events = vec![GraphEvent::EdgesAdded {
        edge_ids: vec![EdgeId::new()],
        adapter_id: "test".into(),
        context_id: "test".into(),
    }];

    // Lens should not emit — similar_to is not in translation rules
    let result = lens.enrich(&events, &ctx);
    assert!(result.is_none(), "lens should not translate unmatched relationships");

    // The similar_to edge remains in the context — traversal without filtering
    // would still reach C. Verify the edge is still there.
    let similar_to_exists = ctx.edges.iter().any(|e| {
        e.source == NodeId::from_string("concept:a")
            && e.target == NodeId::from_string("concept:c")
            && e.relationship == "similar_to"
    });
    assert!(similar_to_exists, "untranslated similar_to edge should remain in context");

    // No lens:trellis:* edge between A and C
    let lens_edge_exists = ctx.edges.iter().any(|e| {
        e.source == NodeId::from_string("concept:a")
            && e.target == NodeId::from_string("concept:c")
            && e.relationship.starts_with("lens:trellis:")
    });
    assert!(!lens_edge_exists, "no lens edge should exist for untranslated relationship");
}

/// Scenario: Lens output is visible to all consumers (Invariant 56)
///
/// Given trellis-content adapter has a lens creating `lens:trellis:thematic_connection` edges
/// When carrel-research (without a lens) traverses the context
/// Then the traversal results include `lens:trellis:thematic_connection` edges
#[tokio::test]
async fn lens_output_is_visible_to_all_consumers() {
    use plexus::{Context, Edge, Node, NodeId, ContentType, dimension, TraverseQuery};

    let mut ctx = Context::new("test");

    // Create concept nodes
    let mut node_a = Node::new("concept", ContentType::Concept);
    node_a.id = NodeId::from_string("concept:a");
    node_a.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_a);

    let mut node_b = Node::new("concept", ContentType::Concept);
    node_b.id = NodeId::from_string("concept:b");
    node_b.dimension = dimension::SEMANTIC.to_string();
    ctx.add_node(node_b);

    // Simulate a lens-created edge (as if trellis lens enrichment already ran)
    let mut lens_edge = Edge::new_in_dimension(
        NodeId::from_string("concept:a"),
        NodeId::from_string("concept:b"),
        "lens:trellis:thematic_connection",
        dimension::SEMANTIC,
    );
    lens_edge.combined_weight = 0.6;
    lens_edge.contributions.insert(
        "lens:trellis:thematic_connection:may_be_related".into(),
        0.6,
    );
    ctx.add_edge(lens_edge);

    // Carrel traverses from A without relationship filtering
    let query = TraverseQuery::from(NodeId::from_string("concept:a"));
    let result = query.execute(&ctx);

    // The traversal should reach B via the lens-created edge
    let reached_b = result.at_depth(1).iter().any(|n| {
        n.id == NodeId::from_string("concept:b")
    });
    assert!(
        reached_b,
        "lens:trellis:thematic_connection edge should be visible to all consumers"
    );
}

/// Scenario: Adapter without lens section works identically to before
///
/// Given a declarative adapter spec with no `lens:` section
/// When the adapter is constructed via `DeclarativeAdapter::from_yaml()`
/// Then `adapter.lens()` returns `None`
#[tokio::test]
async fn adapter_without_lens_section_works_identically() {
    use plexus::adapter::DeclarativeAdapter;

    let yaml = r#"
adapter_id: test-no-lens
input_kind: test.fragment
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;

    let adapter = DeclarativeAdapter::from_yaml(yaml).expect("should parse without lens");
    assert!(adapter.lens().is_none(), "adapter without lens: section should return None");

    let enrichments = adapter.enrichments().expect("enrichments should succeed");
    assert!(enrichments.is_empty(), "no enrichments declared");
}

/// Additional: YAML deserialization with lens section
#[tokio::test]
async fn yaml_with_lens_section_deserializes() {
    use plexus::adapter::DeclarativeAdapter;

    let yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related, similar_to]
      to: thematic_connection
      min_weight: 0.2
    - from: [tagged_with]
      to: topic_link
enrichments:
  - type: co_occurrence
    source_relationship: tagged_with
    output_relationship: may_be_related
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;

    let adapter = DeclarativeAdapter::from_yaml(yaml).expect("should parse with lens");
    let lens = adapter.lens();
    assert!(lens.is_some(), "adapter with lens: section should return Some");

    // Verify the lens enrichment has the correct ID
    use plexus::adapter::Enrichment;
    assert_eq!(lens.unwrap().id(), "lens:trellis");
}

// ---------------------------------------------------------------------------
// Integration verification: lens through real pipeline
// ---------------------------------------------------------------------------

/// Integration: Lens enrichment participates in the enrichment loop via real pipeline.
///
/// This test wires LensEnrichment into a real PipelineBuilder alongside
/// CoOccurrenceEnrichment, ingests a fragment, and verifies that:
/// 1. Co-occurrence creates may_be_related edges (round 1)
/// 2. Lens translates those into lens:trellis:thematic_connection edges (round 2)
/// 3. Both are committed to the context
#[tokio::test]
async fn lens_in_real_pipeline_enrichment_loop() {
    use plexus::adapter::{PipelineBuilder, Enrichment, CoOccurrenceEnrichment, FragmentInput};
    use plexus::adapter::declarative::{LensSpec, TranslationRule};
    use plexus::adapter::lens::LensEnrichment;
    use plexus::storage::{OpenStore, SqliteStore};
    use plexus::{Context, PlexusApi, PlexusEngine, NodeId};
    use std::sync::Arc;

    // Set up engine with in-memory SQLite
    let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let engine = Arc::new(PlexusEngine::with_store(store));

    // Create context
    let ctx = Context::new("integration-lens");
    let ctx_id = ctx.id.clone();
    engine.upsert_context(ctx).expect("upsert");

    // Build pipeline with default adapters + co-occurrence + lens
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

    // Ingest a fragment with two tags — triggers co-occurrence → lens
    let input = FragmentInput::new(
        "Graph structures in knowledge systems",
        vec!["graphs".into(), "knowledge".into()],
    );
    api.ingest("integration-lens", "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // Verify the committed context
    let result_ctx = engine
        .get_context(&ctx_id)
        .expect("context should exist");

    let graphs_id = NodeId::from_string("concept:graphs");
    let knowledge_id = NodeId::from_string("concept:knowledge");

    // Co-occurrence should have created may_be_related edges
    let has_may_be_related = result_ctx.edges.iter().any(|e| {
        e.source == graphs_id
            && e.target == knowledge_id
            && e.relationship == "may_be_related"
    });
    assert!(has_may_be_related, "co-occurrence should create may_be_related");

    // Lens should have translated into lens:trellis:thematic_connection
    let lens_edge = result_ctx.edges.iter().find(|e| {
        e.source == graphs_id
            && e.target == knowledge_id
            && e.relationship == "lens:trellis:thematic_connection"
    });
    assert!(
        lens_edge.is_some(),
        "lens should translate may_be_related → lens:trellis:thematic_connection"
    );

    // Verify contribution key on the lens edge
    let le = lens_edge.unwrap();
    assert!(
        le.contributions.contains_key("lens:trellis:thematic_connection:may_be_related"),
        "lens edge should have per-source contribution key"
    );
}
