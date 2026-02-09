//! FragmentAdapter — external adapter for tagged writing fragments (ADR-004)
//!
//! Maps a fragment (text + tags) to graph structure:
//! - A fragment node (Document, structure dimension)
//! - A concept node per tag (Concept, semantic dimension)
//! - A tagged_with edge per tag (fragment → concept, contribution 1.0)

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::Emission;
use crate::graph::{dimension, ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;

/// Input data for the FragmentAdapter.
///
/// A fragment carries text and tags — applied manually by a human
/// or extracted by an LLM. Tags are expected to be single words
/// or short normalized phrases.
#[derive(Debug, Clone)]
pub struct FragmentInput {
    /// The text content of the fragment
    pub text: String,
    /// Tags applied to the fragment (each produces a concept node)
    pub tags: Vec<String>,
    /// Optional source identifier (e.g., "journal", "sms", "email")
    pub source: Option<String>,
    /// Optional date string
    pub date: Option<String>,
}

impl FragmentInput {
    pub fn new(text: impl Into<String>, tags: Vec<String>) -> Self {
        Self {
            text: text.into(),
            tags,
            source: None,
            date: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }
}

/// External adapter that maps tagged fragments to graph structure.
///
/// One adapter type serves multiple evidence sources via configurable
/// adapter ID. See ADR-004 Decision 1.
pub struct FragmentAdapter {
    adapter_id: String,
}

impl FragmentAdapter {
    pub fn new(adapter_id: impl Into<String>) -> Self {
        Self {
            adapter_id: adapter_id.into(),
        }
    }
}

#[async_trait]
impl Adapter for FragmentAdapter {
    fn id(&self) -> &str {
        &self.adapter_id
    }

    fn input_kind(&self) -> &str {
        "fragment"
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let fragment = input
            .downcast_data::<FragmentInput>()
            .ok_or(AdapterError::InvalidInput)?;

        // Build the fragment node (Document, structure dimension)
        let fragment_id = NodeId::new();
        let mut fragment_node =
            Node::new_in_dimension("fragment", ContentType::Document, dimension::STRUCTURE);
        fragment_node.id = fragment_id.clone();
        fragment_node.properties.insert(
            "text".to_string(),
            PropertyValue::String(fragment.text.clone()),
        );
        if let Some(ref source) = fragment.source {
            fragment_node.properties.insert(
                "source".to_string(),
                PropertyValue::String(source.clone()),
            );
        }
        if let Some(ref date) = fragment.date {
            fragment_node.properties.insert(
                "date".to_string(),
                PropertyValue::String(date.clone()),
            );
        }

        let mut emission = Emission::new().with_node(fragment_node);

        // Build concept nodes and tagged_with edges
        for tag in &fragment.tags {
            let concept_id = NodeId::from_string(format!("concept:{}", tag.to_lowercase()));

            let mut concept_node =
                Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
            concept_node.id = concept_id.clone();
            concept_node.properties.insert(
                "label".to_string(),
                PropertyValue::String(tag.to_lowercase()),
            );

            // tagged_with edge: fragment → concept, cross-dimensional
            let edge = Edge::new_cross_dimensional(
                fragment_id.clone(),
                dimension::STRUCTURE,
                concept_id,
                dimension::SEMANTIC,
                "tagged_with",
            );
            // Set raw_weight to 1.0 — engine extracts this as the contribution value
            let mut edge = edge;
            edge.raw_weight = 1.0;

            emission = emission.with_node(concept_node).with_edge(edge);
        }

        sink.emit(emission).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::graph::{dimension, Context};
    use std::sync::{Arc, Mutex};

    fn make_sink(adapter_id: &str) -> (EngineSink, Arc<Mutex<Context>>) {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let fw = FrameworkContext {
            adapter_id: adapter_id.to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        };
        let sink = EngineSink::new(ctx.clone()).with_framework_context(fw);
        (sink, ctx)
    }

    // === Scenario: Single fragment with tags produces fragment node, concept nodes, and edges ===
    #[tokio::test]
    async fn single_fragment_with_tags() {
        let adapter = FragmentAdapter::new("manual-fragment");
        let (sink, ctx) = make_sink("manual-fragment");

        let input = AdapterInput::new(
            "fragment",
            FragmentInput::new(
                "Walked through Avignon",
                vec!["travel".to_string(), "avignon".to_string()],
            ),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // 3 nodes: 1 fragment + 2 concepts
        assert_eq!(ctx.node_count(), 3);

        // Fragment node
        let fragment_nodes: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.content_type == ContentType::Document && n.dimension == dimension::STRUCTURE)
            .collect();
        assert_eq!(fragment_nodes.len(), 1);

        // Concept nodes
        let travel = ctx.get_node(&NodeId::from_string("concept:travel"));
        assert!(travel.is_some());
        let travel = travel.unwrap();
        assert_eq!(travel.content_type, ContentType::Concept);
        assert_eq!(travel.dimension, dimension::SEMANTIC);

        let avignon = ctx.get_node(&NodeId::from_string("concept:avignon"));
        assert!(avignon.is_some());
        let avignon = avignon.unwrap();
        assert_eq!(avignon.content_type, ContentType::Concept);
        assert_eq!(avignon.dimension, dimension::SEMANTIC);

        // 2 tagged_with edges
        let tagged_with_edges: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_with_edges.len(), 2);

        // Each edge has contribution from manual-fragment
        for edge in &tagged_with_edges {
            assert_eq!(edge.contributions.get("manual-fragment"), Some(&1.0));
        }
    }

    // === Scenario: Two fragments sharing a tag converge on the same concept node ===
    #[tokio::test]
    async fn two_fragments_sharing_tag_converge() {
        let adapter = FragmentAdapter::new("manual-fragment");
        let (sink, ctx) = make_sink("manual-fragment");

        let input1 = AdapterInput::new(
            "fragment",
            FragmentInput::new("Fragment 1", vec!["travel".to_string(), "avignon".to_string()]),
            "test",
        );
        let input2 = AdapterInput::new(
            "fragment",
            FragmentInput::new("Fragment 2", vec!["travel".to_string(), "paris".to_string()]),
            "test",
        );

        adapter.process(&input1, &sink).await.unwrap();
        adapter.process(&input2, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // 5 nodes: 2 fragments + 3 concepts (travel upserted, not duplicated)
        assert_eq!(ctx.node_count(), 5);

        // 3 concept nodes
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("concept:avignon")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("concept:paris")).is_some());

        // 4 tagged_with edges
        let tagged_with_edges: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_with_edges.len(), 4);
    }

    // === Scenario: Tag case normalization ensures convergence ===
    #[tokio::test]
    async fn tag_case_normalization() {
        let adapter = FragmentAdapter::new("manual-fragment");
        let (sink, ctx) = make_sink("manual-fragment");

        let input1 = AdapterInput::new(
            "fragment",
            FragmentInput::new("F1", vec!["Travel".to_string()]),
            "test",
        );
        let input2 = AdapterInput::new(
            "fragment",
            FragmentInput::new("F2", vec!["travel".to_string()]),
            "test",
        );

        adapter.process(&input1, &sink).await.unwrap();
        adapter.process(&input2, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // Both produce concept:travel — exactly 1 concept node
        let concept_nodes: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.content_type == ContentType::Concept)
            .collect();
        assert_eq!(concept_nodes.len(), 1);
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());
    }

    // === Scenario: Fragment with no tags produces only the fragment node ===
    #[tokio::test]
    async fn fragment_with_no_tags() {
        let adapter = FragmentAdapter::new("manual-fragment");
        let (sink, ctx) = make_sink("manual-fragment");

        let input = AdapterInput::new(
            "fragment",
            FragmentInput::new("A thought", vec![]),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.node_count(), 1);
        assert_eq!(ctx.edge_count(), 0);

        let fragment_nodes: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.content_type == ContentType::Document)
            .collect();
        assert_eq!(fragment_nodes.len(), 1);
    }

    // === Scenario: Fragment adapter emits all items in a single emission ===
    #[tokio::test]
    async fn single_emission_per_fragment() {
        // We verify this indirectly: if edges reference nodes in the same emission
        // and commit without rejection, it was a single emission.
        let adapter = FragmentAdapter::new("manual-fragment");
        let (sink, ctx) = make_sink("manual-fragment");

        let input = AdapterInput::new(
            "fragment",
            FragmentInput::new(
                "Test",
                vec!["travel".to_string(), "avignon".to_string()],
            ),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // All committed — no rejections means single emission with nodes + edges together
        assert_eq!(ctx.node_count(), 3);
        assert_eq!(ctx.edge_count(), 2);
    }

    // ================================================================
    // Configurable Adapter Identity (Scenario Group 2)
    // ================================================================

    // === Scenario: Two adapter instances produce separate contribution slots ===
    #[tokio::test]
    async fn two_instances_separate_contributions() {
        let manual = FragmentAdapter::new("manual-fragment");
        let llm = FragmentAdapter::new("llm-fragment");

        // Both adapters share the same graph
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let manual_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "manual-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let llm_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "llm-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });

        let input1 = AdapterInput::new(
            "fragment",
            FragmentInput::new("F1", vec!["travel".to_string()]),
            "test",
        );
        let input2 = AdapterInput::new(
            "fragment",
            FragmentInput::new("F2", vec!["travel".to_string()]),
            "test",
        );

        manual.process(&input1, &manual_sink).await.unwrap();
        llm.process(&input2, &llm_sink).await.unwrap();

        let ctx = ctx.lock().unwrap();

        // Find edges from each fragment to concept:travel
        let travel_id = NodeId::from_string("concept:travel");
        let tagged_edges: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.target == travel_id && e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_edges.len(), 2);

        // Each edge has its own adapter's contribution, not the other's
        for edge in &tagged_edges {
            // Each edge should have exactly one contribution
            assert_eq!(edge.contributions.len(), 1);
        }

        // F1→travel has manual-fragment contribution
        let manual_edge = tagged_edges
            .iter()
            .find(|e| e.contributions.contains_key("manual-fragment"))
            .expect("should have manual-fragment edge");
        assert_eq!(manual_edge.contributions.get("manual-fragment"), Some(&1.0));

        // F2→travel has llm-fragment contribution
        let llm_edge = tagged_edges
            .iter()
            .find(|e| e.contributions.contains_key("llm-fragment"))
            .expect("should have llm-fragment edge");
        assert_eq!(llm_edge.contributions.get("llm-fragment"), Some(&1.0));
    }

    // === Scenario: Same concept from different sources shows evidence diversity ===
    #[tokio::test]
    async fn same_concept_different_sources_evidence_diversity() {
        let manual = FragmentAdapter::new("manual-fragment");
        let llm = FragmentAdapter::new("llm-fragment");

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let manual_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "manual-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });
        let llm_sink = EngineSink::new(ctx.clone()).with_framework_context(FrameworkContext {
            adapter_id: "llm-fragment".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        });

        let input1 = AdapterInput::new(
            "fragment",
            FragmentInput::new("F1", vec!["travel".to_string()]),
            "test",
        );
        let input2 = AdapterInput::new(
            "fragment",
            FragmentInput::new("F2", vec!["travel".to_string()]),
            "test",
        );

        manual.process(&input1, &manual_sink).await.unwrap();
        llm.process(&input2, &llm_sink).await.unwrap();

        let ctx = ctx.lock().unwrap();

        // concept:travel exists exactly once (upserted by both adapters)
        let concept_nodes: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.content_type == ContentType::Concept)
            .collect();
        assert_eq!(concept_nodes.len(), 1);
        assert!(ctx.get_node(&NodeId::from_string("concept:travel")).is_some());

        // Two distinct adapter IDs contributed edges to concept:travel
        let travel_id = NodeId::from_string("concept:travel");
        let adapter_ids: std::collections::HashSet<_> = ctx
            .edges
            .iter()
            .filter(|e| e.target == travel_id && e.relationship == "tagged_with")
            .flat_map(|e| e.contributions.keys().cloned())
            .collect();
        assert_eq!(adapter_ids.len(), 2);
        assert!(adapter_ids.contains("manual-fragment"));
        assert!(adapter_ids.contains("llm-fragment"));
    }
}
