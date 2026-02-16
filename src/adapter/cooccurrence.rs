//! CoOccurrenceEnrichment — co-occurrence detection (ADR-010, ADR-022)
//!
//! Detects nodes that share source nodes (via a configured relationship)
//! and emits symmetric edge pairs with a configured output relationship.
//! Structure-aware: fires based on relationship, not node content type
//! (Invariant 50). Co-occurrence score is `count / max_count`.
//!
//! Default: `tagged_with` → `may_be_related` (backward compatible).
//! Parameterized: any source/output relationship pair (ADR-022).
//!
//! Idempotent: checks for existing edges before emitting, so the enrichment
//! loop reaches quiescence.

use crate::adapter::enrichment::Enrichment;
use crate::adapter::events::GraphEvent;
use crate::adapter::types::{AnnotatedEdge, Emission};
use crate::graph::{dimension, ContentType, Context, Edge, NodeId};
use std::collections::{HashMap, HashSet};

/// Enrichment that detects co-occurrence via shared source nodes.
///
/// Scans the context for nodes sharing source nodes (via a configured
/// relationship) and emits symmetric edge pairs with a configured output
/// relationship. Structure-aware: fires based on relationship, not node
/// content type (Invariant 50). Idempotent: checks for existing edges
/// before emitting, so the enrichment loop reaches quiescence.
///
/// Default configuration: `tagged_with` → `may_be_related` (backward compatible).
/// Parameterized instances use different relationships (ADR-022).
pub struct CoOccurrenceEnrichment {
    source_relationship: String,
    output_relationship: String,
    id: String,
}

impl CoOccurrenceEnrichment {
    pub fn new() -> Self {
        Self {
            source_relationship: "tagged_with".to_string(),
            output_relationship: "may_be_related".to_string(),
            id: "co_occurrence:tagged_with:may_be_related".to_string(),
        }
    }

    pub fn with_relationships(source_relationship: &str, output_relationship: &str) -> Self {
        Self {
            id: format!("co_occurrence:{}:{}", source_relationship, output_relationship),
            source_relationship: source_relationship.to_string(),
            output_relationship: output_relationship.to_string(),
        }
    }
}

impl Enrichment for CoOccurrenceEnrichment {
    fn id(&self) -> &str {
        &self.id
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        // Only run when structural changes could affect co-occurrence
        if !has_structural_events(events) {
            return None;
        }

        let pairs = detect_cooccurrence_pairs(context, &self.source_relationship);
        if pairs.is_empty() {
            return None;
        }

        let max_count = *pairs.values().max().unwrap_or(&1) as f32;
        let mut emission = Emission::new();

        for ((a, b), count) in &pairs {
            let score = *count as f32 / max_count;

            // Idempotent: skip edges that already exist
            if !output_edge_exists(context, a, b, &self.output_relationship) {
                let mut edge = Edge::new_in_dimension(
                    a.clone(),
                    b.clone(),
                    &self.output_relationship,
                    dimension::SEMANTIC,
                );
                edge.raw_weight = score;
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }

            if !output_edge_exists(context, b, a, &self.output_relationship) {
                let mut edge = Edge::new_in_dimension(
                    b.clone(),
                    a.clone(),
                    &self.output_relationship,
                    dimension::SEMANTIC,
                );
                edge.raw_weight = score;
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }
        }

        if emission.is_empty() {
            None
        } else {
            Some(emission)
        }
    }
}

/// Check if events include structural changes that could affect co-occurrence.
fn has_structural_events(events: &[GraphEvent]) -> bool {
    events.iter().any(|e| {
        matches!(
            e,
            GraphEvent::NodesAdded { .. } | GraphEvent::EdgesAdded { .. }
        )
    })
}

/// Build a reverse index (source → targets) and count shared sources
/// for each target pair. Returns canonical pairs with counts.
fn detect_cooccurrence_pairs(context: &Context, source_relationship: &str) -> HashMap<(NodeId, NodeId), usize> {
    let mut source_to_targets: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for edge in context.edges() {
        if edge.relationship != source_relationship {
            continue;
        }
        // Structure-aware: fire based on relationship, not node content type (Invariant 50)
        source_to_targets
            .entry(edge.source.clone())
            .or_default()
            .insert(edge.target.clone());
    }

    let mut pair_counts: HashMap<(NodeId, NodeId), usize> = HashMap::new();

    for concepts in source_to_targets.values() {
        let concepts_vec: Vec<_> = concepts.iter().collect();
        for i in 0..concepts_vec.len() {
            for j in (i + 1)..concepts_vec.len() {
                let (a, b) = if concepts_vec[i].as_str() <= concepts_vec[j].as_str() {
                    (concepts_vec[i].clone(), concepts_vec[j].clone())
                } else {
                    (concepts_vec[j].clone(), concepts_vec[i].clone())
                };
                *pair_counts.entry((a, b)).or_insert(0) += 1;
            }
        }
    }

    pair_counts
}

/// Check if an output edge from source to target already exists.
fn output_edge_exists(context: &Context, source: &NodeId, target: &NodeId, relationship: &str) -> bool {
    context.edges().any(|e| {
        e.source == *source && e.target == *target && e.relationship == relationship
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::fragment::{FragmentAdapter, FragmentInput};
    use crate::adapter::provenance::FrameworkContext;
    use crate::adapter::traits::{Adapter, AdapterInput};
    use crate::graph::{EdgeId, Node};
    use std::sync::{Arc, Mutex};

    /// Helper: create a graph with fragments and tags via the FragmentAdapter.
    async fn build_fragment_graph(
        fragments: Vec<(&str, Vec<&str>)>,
    ) -> Arc<Mutex<Context>> {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let adapter = FragmentAdapter::new("manual-fragment");
        let sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "manual-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });

        for (text, tags) in fragments {
            let input = AdapterInput::new(
                "fragment",
                FragmentInput::new(text, tags.into_iter().map(|s| s.to_string()).collect()),
                "test",
            );
            adapter.process(&input, &sink).await.unwrap();
        }

        ctx
    }

    /// Helper: build a Context snapshot from fragments.
    async fn build_context(
        fragments: Vec<(&str, Vec<&str>)>,
    ) -> Context {
        let ctx = build_fragment_graph(fragments).await;
        let snapshot = ctx.lock().unwrap().clone();
        snapshot
    }

    fn edges_added_event() -> GraphEvent {
        GraphEvent::EdgesAdded {
            edge_ids: vec![EdgeId::from_string("e1")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }
    }

    fn nodes_added_event(node_ids: Vec<&str>) -> GraphEvent {
        GraphEvent::NodesAdded {
            node_ids: node_ids.into_iter().map(NodeId::from_string).collect(),
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn detects_cooccurrence() {
        let ctx = build_context(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        let enrichment = CoOccurrenceEnrichment::new();
        let events = vec![edges_added_event()];

        let emission = enrichment.enrich(&events, &ctx).expect("should emit");

        // Symmetric pair
        assert_eq!(emission.edges.len(), 2);

        let travel_id = NodeId::from_string("concept:travel");
        let avignon_id = NodeId::from_string("concept:avignon");

        let has_ta = emission.edges.iter().any(|ae| {
            ae.edge.source == travel_id
                && ae.edge.target == avignon_id
                && ae.edge.relationship == "may_be_related"
        });
        let has_at = emission.edges.iter().any(|ae| {
            ae.edge.source == avignon_id
                && ae.edge.target == travel_id
                && ae.edge.relationship == "may_be_related"
        });

        assert!(has_ta, "travel→avignon");
        assert!(has_at, "avignon→travel");
    }

    #[tokio::test]
    async fn symmetric_pairs_have_equal_scores() {
        let ctx = build_context(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        let enrichment = CoOccurrenceEnrichment::new();
        let emission = enrichment.enrich(&[edges_added_event()], &ctx).unwrap();

        let scores: Vec<f32> = emission.edges.iter().map(|ae| ae.edge.raw_weight).collect();
        assert_eq!(scores[0], scores[1], "symmetric edges should have equal scores");
    }

    #[tokio::test]
    async fn normalized_scores() {
        let ctx = build_context(vec![
            ("F1", vec!["travel", "avignon"]),
            ("F2", vec!["travel", "avignon", "paris"]),
        ]).await;

        let enrichment = CoOccurrenceEnrichment::new();
        let emission = enrichment.enrich(&[edges_added_event()], &ctx).unwrap();

        let travel_id = NodeId::from_string("concept:travel");
        let avignon_id = NodeId::from_string("concept:avignon");
        let paris_id = NodeId::from_string("concept:paris");

        // travel↔avignon: 2 shared / 2 max = 1.0
        let ta_edge = emission.edges.iter().find(|ae| {
            ae.edge.source == travel_id && ae.edge.target == avignon_id
        }).expect("travel→avignon should exist");
        assert_eq!(ta_edge.edge.raw_weight, 1.0);

        // travel↔paris: 1 shared / 2 max = 0.5
        let tp_edge = emission.edges.iter().find(|ae| {
            ae.edge.source == travel_id && ae.edge.target == paris_id
        }).expect("travel→paris should exist");
        assert_eq!(tp_edge.edge.raw_weight, 0.5);
    }

    #[tokio::test]
    async fn no_shared_fragments_returns_none() {
        let ctx = build_context(vec![
            ("F1", vec!["travel"]),
            ("F2", vec!["morning"]),
        ]).await;

        let enrichment = CoOccurrenceEnrichment::new();
        assert!(enrichment.enrich(&[edges_added_event()], &ctx).is_none());
    }

    #[tokio::test]
    async fn idempotent_skips_existing_edges() {
        let mut ctx = build_context(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        let travel_id = NodeId::from_string("concept:travel");
        let avignon_id = NodeId::from_string("concept:avignon");

        let mut edge_ta = Edge::new_in_dimension(
            travel_id.clone(), avignon_id.clone(), "may_be_related", dimension::SEMANTIC,
        );
        edge_ta.raw_weight = 1.0;
        ctx.add_edge(edge_ta);

        let mut edge_at = Edge::new_in_dimension(
            avignon_id.clone(), travel_id.clone(), "may_be_related", dimension::SEMANTIC,
        );
        edge_at.raw_weight = 1.0;
        ctx.add_edge(edge_at);

        let enrichment = CoOccurrenceEnrichment::new();
        assert!(enrichment.enrich(&[edges_added_event()], &ctx).is_none());
    }

    #[tokio::test]
    async fn quiescent_on_non_structural_events() {
        let ctx = build_context(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        let enrichment = CoOccurrenceEnrichment::new();
        let events = vec![GraphEvent::NodesRemoved {
            node_ids: vec![NodeId::from_string("some-node")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        assert!(enrichment.enrich(&events, &ctx).is_none());
    }

    #[tokio::test]
    async fn empty_graph_returns_none() {
        let ctx = Context::new("test");
        let enrichment = CoOccurrenceEnrichment::new();
        assert!(enrichment.enrich(&[nodes_added_event(vec!["n1"])], &ctx).is_none());
    }

    // --- Scenario: Default CoOccurrenceEnrichment backward compatible ---

    #[tokio::test]
    async fn default_backward_compatible() {
        // CoOccurrenceEnrichment with no explicit configuration
        let enrichment = CoOccurrenceEnrichment::new();

        let ctx = build_context(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        let emission = enrichment.enrich(&[edges_added_event()], &ctx).expect("should emit");

        // Emits may_be_related (existing behavior unchanged)
        assert!(emission.edges.iter().all(|ae| ae.edge.relationship == "may_be_related"));
        assert_eq!(emission.edges.len(), 2);
    }

    // --- Scenario: CoOccurrenceEnrichment accepts relationship parameters ---

    /// Helper: build a context with source nodes connected to concept nodes
    /// via a custom relationship (e.g., "exhibits").
    fn build_custom_relationship_context(
        relationship: &str,
        sources_to_concepts: Vec<(&str, Vec<&str>)>,
    ) -> Context {
        let mut ctx = Context::new("test");

        for (source_id, concept_tags) in sources_to_concepts {
            // Create source node
            let mut source = Node::new("source", ContentType::Document);
            source.id = NodeId::from_string(source_id);
            ctx.add_node(source);

            for tag in concept_tags {
                // Create concept node
                let concept_id = format!("concept:{}", tag);
                if ctx.get_node(&NodeId::from_string(&concept_id)).is_none() {
                    let mut concept = Node::new("concept", ContentType::Concept);
                    concept.id = NodeId::from_string(&concept_id);
                    concept.dimension = dimension::SEMANTIC.to_string();
                    ctx.add_node(concept);
                }

                // Create edge with custom relationship
                let edge = Edge::new_in_dimension(
                    NodeId::from_string(source_id),
                    NodeId::from_string(&concept_id),
                    relationship,
                    dimension::SEMANTIC,
                );
                ctx.add_edge(edge);
            }
        }

        ctx
    }

    #[test]
    fn accepts_relationship_parameters() {
        let enrichment = CoOccurrenceEnrichment::with_relationships("exhibits", "co_exhibited");

        // Two source nodes share "exhibits" edges to the same concepts
        let ctx = build_custom_relationship_context("exhibits", vec![
            ("gesture-1", vec!["flow", "glide"]),
            ("gesture-2", vec!["flow", "glide"]),
        ]);

        let emission = enrichment.enrich(&[edges_added_event()], &ctx).expect("should emit");

        // Symmetric pair: flow↔glide
        assert_eq!(emission.edges.len(), 2);

        let flow_id = NodeId::from_string("concept:flow");
        let glide_id = NodeId::from_string("concept:glide");

        let has_fg = emission.edges.iter().any(|ae| {
            ae.edge.source == flow_id
                && ae.edge.target == glide_id
                && ae.edge.relationship == "co_exhibited"
        });
        let has_gf = emission.edges.iter().any(|ae| {
            ae.edge.source == glide_id
                && ae.edge.target == flow_id
                && ae.edge.relationship == "co_exhibited"
        });

        assert!(has_fg, "flow→glide with co_exhibited");
        assert!(has_gf, "glide→flow with co_exhibited");

        // Verify contribution is normalized score
        let score = emission.edges[0].edge.raw_weight;
        assert_eq!(score, 1.0, "2 shared / 2 max = 1.0");
    }

    // --- Scenario: Parameterized enrichment has unique stable ID ---

    #[test]
    fn parameterized_id_from_relationships() {
        let exhibits = CoOccurrenceEnrichment::with_relationships("exhibits", "co_exhibited");
        assert_eq!(exhibits.id(), "co_occurrence:exhibits:co_exhibited");

        let tagged = CoOccurrenceEnrichment::with_relationships("tagged_with", "may_be_related");
        assert_eq!(tagged.id(), "co_occurrence:tagged_with:may_be_related");

        // Default matches the tagged_with parameterization
        let default = CoOccurrenceEnrichment::new();
        assert_eq!(default.id(), "co_occurrence:tagged_with:may_be_related");

        // Different params → different IDs
        assert_ne!(exhibits.id(), tagged.id());
    }

    // --- Scenario: Structure-aware enrichment fires for any source node type ---

    #[test]
    fn structure_aware_fires_for_any_source_node_type() {
        let enrichment = CoOccurrenceEnrichment::new();

        let mut ctx = Context::new("test");

        // Fragment source node (ContentType::Document)
        let mut fragment = Node::new("fragment", ContentType::Document);
        fragment.id = NodeId::from_string("fragment-1");
        ctx.add_node(fragment);

        // Artifact source node (ContentType::Code — different type)
        let mut artifact = Node::new("artifact", ContentType::Code);
        artifact.id = NodeId::from_string("artifact-1");
        ctx.add_node(artifact);

        // Target nodes — deliberately use ContentType::Document (NOT Concept)
        // to verify the enrichment fires based on relationship structure,
        // not target node content type (Invariant 50).
        for tag in &["alpha", "bravo", "charlie"] {
            let mut target = Node::new("section", ContentType::Document);
            target.id = NodeId::from_string(&format!("section:{}", tag));
            target.dimension = dimension::SEMANTIC.to_string();
            ctx.add_node(target);
        }

        // Fragment tagged_with A and B
        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("fragment-1"),
            NodeId::from_string("section:alpha"),
            "tagged_with",
            dimension::SEMANTIC,
        ));
        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("fragment-1"),
            NodeId::from_string("section:bravo"),
            "tagged_with",
            dimension::SEMANTIC,
        ));

        // Artifact tagged_with B and C
        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("artifact-1"),
            NodeId::from_string("section:bravo"),
            "tagged_with",
            dimension::SEMANTIC,
        ));
        ctx.add_edge(Edge::new_in_dimension(
            NodeId::from_string("artifact-1"),
            NodeId::from_string("section:charlie"),
            "tagged_with",
            dimension::SEMANTIC,
        ));

        let emission = enrichment
            .enrich(&[edges_added_event()], &ctx)
            .expect("should emit co-occurrence edges");

        let alpha = NodeId::from_string("section:alpha");
        let bravo = NodeId::from_string("section:bravo");
        let charlie = NodeId::from_string("section:charlie");

        // A↔B co-occur via fragment-1
        let has_ab = emission.edges.iter().any(|ae| {
            ae.edge.source == alpha && ae.edge.target == bravo
                && ae.edge.relationship == "may_be_related"
        });
        let has_ba = emission.edges.iter().any(|ae| {
            ae.edge.source == bravo && ae.edge.target == alpha
                && ae.edge.relationship == "may_be_related"
        });

        // B↔C co-occur via artifact-1
        let has_bc = emission.edges.iter().any(|ae| {
            ae.edge.source == bravo && ae.edge.target == charlie
                && ae.edge.relationship == "may_be_related"
        });
        let has_cb = emission.edges.iter().any(|ae| {
            ae.edge.source == charlie && ae.edge.target == bravo
                && ae.edge.relationship == "may_be_related"
        });

        assert!(has_ab, "alpha→bravo via fragment source");
        assert!(has_ba, "bravo→alpha via fragment source");
        assert!(has_bc, "bravo→charlie via artifact source");
        assert!(has_cb, "charlie→bravo via artifact source");

        // A↔B via fragment, B↔C via artifact. No source has both A and C.
        // 2 pairs × 2 symmetric = 4 edges
        assert_eq!(emission.edges.len(), 4, "2 co-occurring pairs × 2 symmetric edges");
    }

    #[test]
    fn distinct_instances_not_deduplicated_in_registry() {
        use crate::adapter::enrichment::EnrichmentRegistry;
        use std::sync::Arc;

        let exhibits = Arc::new(
            CoOccurrenceEnrichment::with_relationships("exhibits", "co_exhibited"),
        ) as Arc<dyn Enrichment>;
        let tagged = Arc::new(
            CoOccurrenceEnrichment::with_relationships("tagged_with", "may_be_related"),
        ) as Arc<dyn Enrichment>;

        let registry = EnrichmentRegistry::new(vec![exhibits, tagged]);
        assert_eq!(registry.enrichments().len(), 2, "distinct params → distinct instances");
    }
}
