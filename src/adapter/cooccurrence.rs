//! CoOccurrenceEnrichment — concept co-occurrence detection (ADR-010)
//!
//! Detects concepts that share fragments (via `tagged_with` edges) and emits
//! `may_be_related` symmetric edge pairs. Co-occurrence score is `count / max_count`.
//!
//! Idempotent: checks for existing edges before emitting, so the enrichment
//! loop reaches quiescence.

use crate::adapter::enrichment::Enrichment;
use crate::adapter::events::GraphEvent;
use crate::adapter::types::{AnnotatedEdge, Emission};
use crate::graph::{dimension, ContentType, Context, Edge, NodeId};
use std::collections::{HashMap, HashSet};

/// Enrichment that detects concept co-occurrence via shared fragments.
///
/// Scans the context for concepts sharing fragments (via `tagged_with` edges)
/// and emits symmetric `may_be_related` edge pairs. Idempotent: checks for
/// existing edges before emitting, so the enrichment loop reaches quiescence.
pub struct CoOccurrenceEnrichment;

impl CoOccurrenceEnrichment {
    pub fn new() -> Self {
        Self
    }
}

impl Enrichment for CoOccurrenceEnrichment {
    fn id(&self) -> &str {
        "co-occurrence"
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        // Only run when structural changes could affect co-occurrence
        if !has_structural_events(events) {
            return None;
        }

        let pairs = detect_cooccurrence_pairs(context);
        if pairs.is_empty() {
            return None;
        }

        let max_count = *pairs.values().max().unwrap_or(&1) as f32;
        let mut emission = Emission::new();

        for ((a, b), count) in &pairs {
            let score = *count as f32 / max_count;

            // Idempotent: skip edges that already exist
            if !may_be_related_exists(context, a, b) {
                let mut edge = Edge::new_in_dimension(
                    a.clone(),
                    b.clone(),
                    "may_be_related",
                    dimension::SEMANTIC,
                );
                edge.raw_weight = score;
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }

            if !may_be_related_exists(context, b, a) {
                let mut edge = Edge::new_in_dimension(
                    b.clone(),
                    a.clone(),
                    "may_be_related",
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

/// Build a reverse index (fragment → concepts) and count shared fragments
/// for each concept pair. Returns canonical pairs with counts.
fn detect_cooccurrence_pairs(context: &Context) -> HashMap<(NodeId, NodeId), usize> {
    let mut fragment_to_concepts: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for edge in context.edges() {
        if edge.relationship != "tagged_with" {
            continue;
        }
        if let Some(target_node) = context.get_node(&edge.target) {
            if target_node.content_type == ContentType::Concept {
                fragment_to_concepts
                    .entry(edge.source.clone())
                    .or_default()
                    .insert(edge.target.clone());
            }
        }
    }

    let mut pair_counts: HashMap<(NodeId, NodeId), usize> = HashMap::new();

    for concepts in fragment_to_concepts.values() {
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

/// Check if a `may_be_related` edge from source to target already exists.
fn may_be_related_exists(context: &Context, source: &NodeId, target: &NodeId) -> bool {
    context.edges().any(|e| {
        e.source == *source && e.target == *target && e.relationship == "may_be_related"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::fragment::{FragmentAdapter, FragmentInput};
    use crate::adapter::provenance::FrameworkContext;
    use crate::adapter::traits::{Adapter, AdapterInput};
    use crate::graph::EdgeId;
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
}
