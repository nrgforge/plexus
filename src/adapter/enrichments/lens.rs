//! LensEnrichment — consumer-scoped domain vocabulary translation (ADR-033).
//!
//! Translates cross-domain graph relationships into a consumer's vocabulary
//! by creating new edges with namespaced relationship types. Implements the
//! Enrichment trait — same interface as all other core enrichments, same
//! enrichment loop participation (Invariant 57).
//!
//! Lens output is public — visible to all consumers (Invariant 56).

use crate::adapter::adapters::declarative::LensSpec;
use crate::adapter::enrichment::Enrichment;
use crate::adapter::types::{AnnotatedEdge, Emission};
use crate::graph::events::GraphEvent;
use crate::graph::{dimension, Context, Edge};

/// Consumer-scoped enrichment that translates cross-domain relationships
/// into one consumer's domain vocabulary.
///
/// Constructed by `DeclarativeAdapter::lens()` from the adapter spec's
/// `lens:` section. Not constructed directly by consumers.
pub struct LensEnrichment {
    spec: LensSpec,
    id: String,
}

impl LensEnrichment {
    /// Create a new LensEnrichment from a lens specification.
    pub fn new(spec: LensSpec) -> Self {
        let id = format!("lens:{}", spec.consumer);
        Self { spec, id }
    }
}

impl Enrichment for LensEnrichment {
    fn id(&self) -> &str {
        &self.id
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        use crate::graph::NodeId;
        use std::collections::HashMap;

        // Only react to EdgesAdded events (ADR-033 algorithm)
        if !events.iter().any(|e| matches!(e, GraphEvent::EdgesAdded { .. })) {
            return None;
        }

        // Accumulate translations per (source, target, to_relationship) to support
        // many-to-one rules — multiple source relationships merge into one edge
        // with per-source contribution keys (ADR-033).
        type EdgeKey = (NodeId, NodeId, String);
        let mut pending: HashMap<EdgeKey, HashMap<String, f32>> = HashMap::new();

        for rule in &self.spec.translations {
            let min_weight = rule.min_weight.unwrap_or(0.0);
            let to_relationship = format!(
                "lens:{}:{}",
                self.spec.consumer, rule.to
            );

            for edge in &context.edges {
                if !rule.from.contains(&edge.relationship) {
                    continue;
                }

                if edge.combined_weight < min_weight {
                    continue;
                }

                // Idempotency guard: skip if translated edge already exists in context
                let already_exists = context.edges.iter().any(|e| {
                    e.source == edge.source
                        && e.target == edge.target
                        && e.relationship == to_relationship
                });
                if already_exists {
                    continue;
                }

                let key = (
                    edge.source.clone(),
                    edge.target.clone(),
                    to_relationship.clone(),
                );
                let contribution_key = format!(
                    "{}:{}",
                    to_relationship, edge.relationship
                );

                pending
                    .entry(key)
                    .or_default()
                    .insert(contribution_key, edge.combined_weight);
            }
        }

        if pending.is_empty() {
            return None;
        }

        let mut emission = Emission::new();

        for ((source, target, relationship), contributions) in pending {
            let mut translated_edge = Edge::new_in_dimension(
                source,
                target,
                &relationship,
                dimension::SEMANTIC,
            );
            // Combined weight is the max across contributing sources
            translated_edge.combined_weight = contributions
                .values()
                .copied()
                .fold(0.0_f32, f32::max);
            translated_edge.contributions = contributions;

            emission = emission.with_edge(AnnotatedEdge::new(translated_edge));
        }

        Some(emission)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::adapters::declarative::{LensSpec, TranslationRule};
    use crate::graph::{dimension, ContentType, Context, Edge, EdgeId, Node, NodeId};

    fn concept_node(label: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(&format!("concept:{}", label));
        n.dimension = dimension::SEMANTIC.to_string();
        n
    }

    fn edges_added_event() -> GraphEvent {
        GraphEvent::EdgesAdded {
            edge_ids: vec![EdgeId::new()],
            adapter_id: "test".into(),
            context_id: "test".into(),
        }
    }

    fn basic_lens_spec() -> LensSpec {
        LensSpec {
            consumer: "trellis".into(),
            translations: vec![TranslationRule {
                from: vec!["may_be_related".into()],
                to: "thematic_connection".into(),
                min_weight: None,
                involving: None,
            }],
        }
    }

    fn context_with_may_be_related(weight: f32) -> Context {
        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("a"));
        ctx.add_node(concept_node("b"));

        let mut edge = Edge::new_in_dimension(
            NodeId::from_string("concept:a"),
            NodeId::from_string("concept:b"),
            "may_be_related",
            dimension::SEMANTIC,
        );
        edge.combined_weight = weight;
        ctx.add_edge(edge);
        ctx
    }

    #[test]
    fn creates_translated_edge() {
        let ctx = context_with_may_be_related(0.6);
        let lens = LensEnrichment::new(basic_lens_spec());

        let emission = lens.enrich(&[edges_added_event()], &ctx)
            .expect("should emit");

        assert_eq!(emission.edges.len(), 1);
        let edge = &emission.edges[0].edge;
        assert_eq!(edge.source, NodeId::from_string("concept:a"));
        assert_eq!(edge.target, NodeId::from_string("concept:b"));
        assert_eq!(edge.relationship, "lens:trellis:thematic_connection");
    }

    #[test]
    fn contribution_key_follows_namespace_convention() {
        let ctx = context_with_may_be_related(0.6);
        let lens = LensEnrichment::new(basic_lens_spec());

        let emission = lens.enrich(&[edges_added_event()], &ctx).unwrap();
        let edge = &emission.edges[0].edge;

        assert!(edge.contributions.contains_key("lens:trellis:thematic_connection:may_be_related"));
        assert_eq!(
            edge.contributions["lens:trellis:thematic_connection:may_be_related"],
            0.6
        );
    }

    #[test]
    fn stable_id() {
        let lens = LensEnrichment::new(basic_lens_spec());
        assert_eq!(lens.id(), "lens:trellis");
    }

    #[test]
    fn quiescent_on_non_edge_events() {
        let ctx = context_with_may_be_related(0.6);
        let lens = LensEnrichment::new(basic_lens_spec());

        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![NodeId::from_string("n1")],
            adapter_id: "test".into(),
            context_id: "test".into(),
        }];

        assert!(lens.enrich(&events, &ctx).is_none());
    }
}
