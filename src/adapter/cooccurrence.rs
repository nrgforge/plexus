//! CoOccurrenceAdapter — reflexive adapter for concept co-occurrence detection (ADR-004)
//!
//! Detects concepts that share fragments (via `tagged_with` edges) and proposes
//! `may_be_related` symmetric edge pairs via ProposalSink. Co-occurrence score
//! is `count / max_count`.

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::Emission;
use crate::graph::{dimension, ContentType, Context, Edge, NodeId};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

/// Reflexive adapter that detects co-occurrence between concepts.
///
/// Receives a graph state snapshot (cloned Context) as its opaque payload.
/// See ADR-004 Decision 3.
pub struct CoOccurrenceAdapter {
    adapter_id: String,
}

impl CoOccurrenceAdapter {
    pub fn new(adapter_id: impl Into<String>) -> Self {
        Self {
            adapter_id: adapter_id.into(),
        }
    }
}

#[async_trait]
impl Adapter for CoOccurrenceAdapter {
    fn id(&self) -> &str {
        &self.adapter_id
    }

    fn input_kind(&self) -> &str {
        "graph_state"
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let snapshot = input
            .downcast_data::<Context>()
            .ok_or(AdapterError::InvalidInput)?;

        // Step 1: Build reverse index — fragment → set of concept NodeIds
        // A fragment is the source of a tagged_with edge; the concept is the target.
        let mut fragment_to_concepts: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

        for edge in &snapshot.edges {
            if edge.relationship != "tagged_with" {
                continue;
            }
            // Verify the target is a concept node
            if let Some(target_node) = snapshot.get_node(&edge.target) {
                if target_node.content_type == ContentType::Concept {
                    fragment_to_concepts
                        .entry(edge.source.clone())
                        .or_default()
                        .insert(edge.target.clone());
                }
            }
        }

        // Step 2: Count shared fragments for each concept pair
        let mut pair_counts: HashMap<(NodeId, NodeId), usize> = HashMap::new();

        for concepts in fragment_to_concepts.values() {
            let concepts_vec: Vec<_> = concepts.iter().collect();
            for i in 0..concepts_vec.len() {
                for j in (i + 1)..concepts_vec.len() {
                    // Canonical ordering to avoid double-counting
                    let (a, b) = if concepts_vec[i].as_str() <= concepts_vec[j].as_str() {
                        (concepts_vec[i].clone(), concepts_vec[j].clone())
                    } else {
                        (concepts_vec[j].clone(), concepts_vec[i].clone())
                    };
                    *pair_counts.entry((a, b)).or_insert(0) += 1;
                }
            }
        }

        if pair_counts.is_empty() {
            return Ok(());
        }

        // Step 3: Normalize scores — count / max_count
        let max_count = *pair_counts.values().max().unwrap_or(&1) as f32;

        // Step 4: Emit symmetric may_be_related edge pairs
        let mut emission = Emission::new();

        for ((a, b), count) in &pair_counts {
            let score = *count as f32 / max_count;

            // A → B
            let mut edge_ab = Edge::new_in_dimension(
                a.clone(),
                b.clone(),
                "may_be_related",
                dimension::SEMANTIC,
            );
            edge_ab.raw_weight = score;

            // B → A
            let mut edge_ba = Edge::new_in_dimension(
                b.clone(),
                a.clone(),
                "may_be_related",
                dimension::SEMANTIC,
            );
            edge_ba.raw_weight = score;

            emission = emission.with_edge(edge_ab).with_edge(edge_ba);
        }

        sink.emit(emission).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::fragment::{FragmentAdapter, FragmentInput};
    use crate::adapter::proposal_sink::ProposalSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::adapter::traits::Adapter;
    use crate::graph::Context;
    use std::sync::{Arc, Mutex};

    /// Helper: create a graph with fragments and tags via the FragmentAdapter,
    /// return the context for snapshot.
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

    /// Helper: run the CoOccurrenceAdapter on a snapshot of the given context,
    /// using a ProposalSink with the given cap.
    async fn run_cooccurrence(
        ctx: &Arc<Mutex<Context>>,
        cap: f32,
    ) {
        let snapshot = ctx.lock().unwrap().clone();
        let adapter = CoOccurrenceAdapter::new("co-occurrence");
        let sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "co-occurrence".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let proposal_sink = ProposalSink::new(sink, cap);

        let input = AdapterInput::new("graph_state", snapshot, "test");
        adapter.process(&input, &proposal_sink).await.unwrap();
    }

    // === Scenario: Two concepts sharing one fragment get a co-occurrence proposal ===
    #[tokio::test]
    async fn two_concepts_one_shared_fragment() {
        let ctx = build_fragment_graph(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        run_cooccurrence(&ctx, 1.0).await;

        let ctx = ctx.lock().unwrap();
        let may_be_related: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "may_be_related")
            .collect();

        // Symmetric pair: travel→avignon and avignon→travel
        assert_eq!(may_be_related.len(), 2);

        // Score is 1.0 (1 shared / 1 max)
        for edge in &may_be_related {
            assert_eq!(edge.contributions.get("co-occurrence"), Some(&1.0));
        }
    }

    // === Scenario: Two concepts sharing multiple fragments score higher than single-shared ===
    #[tokio::test]
    async fn multi_shared_scores_higher() {
        let ctx = build_fragment_graph(vec![
            ("F1", vec!["travel", "avignon"]),
            ("F2", vec!["travel", "avignon", "paris"]),
        ]).await;

        run_cooccurrence(&ctx, 1.0).await;

        let ctx = ctx.lock().unwrap();
        let travel_id = NodeId::from_string("concept:travel");
        let avignon_id = NodeId::from_string("concept:avignon");
        let paris_id = NodeId::from_string("concept:paris");

        // travel ↔ avignon: 2 shared fragments / 2 max = 1.0
        let ta_edge = ctx.edges.iter().find(|e| {
            e.relationship == "may_be_related"
                && e.source == travel_id
                && e.target == avignon_id
        }).expect("travel→avignon should exist");
        assert_eq!(ta_edge.contributions.get("co-occurrence"), Some(&1.0));

        // travel ↔ paris: 1 shared fragment / 2 max = 0.5
        let tp_edge = ctx.edges.iter().find(|e| {
            e.relationship == "may_be_related"
                && e.source == travel_id
                && e.target == paris_id
        }).expect("travel→paris should exist");
        assert_eq!(tp_edge.contributions.get("co-occurrence"), Some(&0.5));
    }

    // === Scenario: Concepts with no shared fragments get no proposal ===
    #[tokio::test]
    async fn no_shared_fragments_no_proposal() {
        let ctx = build_fragment_graph(vec![
            ("F1", vec!["travel"]),
            ("F3", vec!["morning"]),
        ]).await;

        run_cooccurrence(&ctx, 1.0).await;

        let ctx = ctx.lock().unwrap();
        let may_be_related: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "may_be_related")
            .collect();

        assert_eq!(may_be_related.len(), 0);
    }

    // === Scenario: Co-occurrence proposals are symmetric edge pairs ===
    #[tokio::test]
    async fn proposals_are_symmetric() {
        let ctx = build_fragment_graph(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        run_cooccurrence(&ctx, 1.0).await;

        let ctx = ctx.lock().unwrap();
        let travel_id = NodeId::from_string("concept:travel");
        let avignon_id = NodeId::from_string("concept:avignon");

        let ta = ctx.edges.iter().find(|e| {
            e.relationship == "may_be_related"
                && e.source == travel_id
                && e.target == avignon_id
        });
        let at = ctx.edges.iter().find(|e| {
            e.relationship == "may_be_related"
                && e.source == avignon_id
                && e.target == travel_id
        });

        assert!(ta.is_some(), "travel→avignon should exist");
        assert!(at.is_some(), "avignon→travel should exist");

        // Both have the same contribution value
        let ta_score = ta.unwrap().contributions.get("co-occurrence");
        let at_score = at.unwrap().contributions.get("co-occurrence");
        assert_eq!(ta_score, at_score);
    }

    // === Scenario: ProposalSink clamps co-occurrence contribution to cap ===
    #[tokio::test]
    async fn proposal_sink_clamps_to_cap() {
        let ctx = build_fragment_graph(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        // Cap at 0.5 — score of 1.0 should be clamped
        run_cooccurrence(&ctx, 0.5).await;

        let ctx = ctx.lock().unwrap();
        let may_be_related: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "may_be_related")
            .collect();

        assert_eq!(may_be_related.len(), 2);
        for edge in &may_be_related {
            assert_eq!(edge.contributions.get("co-occurrence"), Some(&0.5));
        }
    }

    // === Scenario: Empty graph produces no proposals ===
    #[tokio::test]
    async fn empty_graph_no_proposals() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        run_cooccurrence(&ctx, 1.0).await;

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.edge_count(), 0);
    }

    // === Scenario: CoOccurrenceAdapter reads graph state snapshot, not live state ===
    #[tokio::test]
    async fn reads_snapshot_not_live_state() {
        let ctx = build_fragment_graph(vec![
            ("F1", vec!["travel", "avignon"]),
        ]).await;

        // Take snapshot before adding more fragments
        let snapshot = ctx.lock().unwrap().clone();

        // Add more fragments to the live graph after snapshot
        {
            let adapter = FragmentAdapter::new("manual-fragment");
            let sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
                adapter_id: "manual-fragment".to_string(),
                context_id: "test".to_string(),
                input_summary: None,
            });
            let input = AdapterInput::new(
                "fragment",
                FragmentInput::new("F2", vec!["travel".to_string(), "paris".to_string()]),
                "test",
            );
            adapter.process(&input, &sink).await.unwrap();
        }

        // Run co-occurrence on the OLD snapshot
        let cooccurrence = CoOccurrenceAdapter::new("co-occurrence");
        let sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "co-occurrence".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let proposal_sink = ProposalSink::new(sink, 1.0);

        let input = AdapterInput::new("graph_state", snapshot, "test");
        cooccurrence.process(&input, &proposal_sink).await.unwrap();

        let ctx = ctx.lock().unwrap();

        // Snapshot only had F1 with [travel, avignon]
        // So only travel↔avignon should be proposed, NOT travel↔paris
        let may_be_related: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "may_be_related")
            .collect();

        // Only the symmetric pair from the snapshot
        assert_eq!(may_be_related.len(), 2);

        let paris_id = NodeId::from_string("concept:paris");
        let paris_proposals: Vec<_> = may_be_related
            .iter()
            .filter(|e| e.source == paris_id || e.target == paris_id)
            .collect();
        assert_eq!(paris_proposals.len(), 0, "paris should not appear in proposals from old snapshot");
    }
}
