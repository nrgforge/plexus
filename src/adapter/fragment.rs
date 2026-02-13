//! FragmentAdapter — external adapter for tagged writing fragments (ADR-004)
//!
//! Maps a fragment (text + tags) to graph structure:
//! - A fragment node (Document, structure dimension) — deterministic ID via content hash
//! - A concept node per tag (Concept, semantic dimension) — deterministic ID via tag label
//! - A tagged_with edge per tag (fragment → concept, contribution 1.0)
//! - A chain node (Provenance, provenance dimension) — per adapter+source, idempotent
//! - A mark node (Provenance, provenance dimension) — source evidence for the fragment
//! - A contains edge (chain → mark, within provenance)
//!
//! All node IDs are deterministic. Re-ingesting the same fragment produces the same
//! nodes, triggering upsert rather than creating duplicates.

use crate::adapter::events::GraphEvent;
use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{Emission, OutboundEvent};
use crate::graph::{dimension, ContentType, Context, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use uuid::Uuid;

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
        // Deterministic ID: content-hash of adapter_id + text + sorted tags.
        // Re-ingesting the same fragment produces the same node → upsert, not duplicate.
        let mut sorted_tags: Vec<String> = fragment.tags.iter().map(|t| t.to_lowercase()).collect();
        sorted_tags.sort();
        let hash_input = format!(
            "{}:{}:{}",
            self.adapter_id,
            fragment.text,
            sorted_tags.join(",")
        );
        // UUID v5 namespace for Plexus fragments (stable, arbitrary)
        const FRAGMENT_NS: Uuid = Uuid::from_bytes([
            0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1,
            0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
        ]);
        let fragment_id = NodeId::from_string(
            format!("fragment:{}", Uuid::new_v5(&FRAGMENT_NS, hash_input.as_bytes()))
        );
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

        // Build provenance chain + mark (source evidence)
        //
        // The chain groups all marks from this adapter for this source.
        // Deterministic ID ensures idempotent upsert across ingest calls.
        // The mark records where this fragment's knowledge came from,
        // with tags that TagConceptBridger will bridge to concept nodes.
        let source = fragment.source.as_deref().unwrap_or("default");
        let chain_id = NodeId::from_string(format!("chain:{}:{}", self.adapter_id, source));
        let mut chain_node = Node::new_in_dimension(
            "chain",
            ContentType::Provenance,
            dimension::PROVENANCE,
        );
        chain_node.id = chain_id.clone();
        chain_node.properties.insert(
            "name".to_string(),
            PropertyValue::String(format!("{} — {}", self.adapter_id, source)),
        );
        chain_node.properties.insert(
            "status".to_string(),
            PropertyValue::String("active".to_string()),
        );

        let mark_id = NodeId::from_string(format!("mark:{}:{}", self.adapter_id, fragment_id));
        let mut mark_node = Node::new_in_dimension(
            "mark",
            ContentType::Provenance,
            dimension::PROVENANCE,
        );
        mark_node.id = mark_id.clone();
        mark_node.properties.insert(
            "chain_id".to_string(),
            PropertyValue::String(chain_id.to_string()),
        );
        mark_node.properties.insert(
            "annotation".to_string(),
            PropertyValue::String(fragment.text.clone()),
        );
        mark_node.properties.insert(
            "file".to_string(),
            PropertyValue::String(source.to_string()),
        );
        mark_node.properties.insert(
            "line".to_string(),
            PropertyValue::Int(1),
        );
        if !fragment.tags.is_empty() {
            let tag_vals: Vec<PropertyValue> = fragment
                .tags
                .iter()
                .map(|t| PropertyValue::String(t.to_lowercase()))
                .collect();
            mark_node.properties.insert(
                "tags".to_string(),
                PropertyValue::Array(tag_vals),
            );
        }

        let contains_edge = Edge::new_in_dimension(
            chain_id,
            mark_id,
            "contains",
            dimension::PROVENANCE,
        );

        emission = emission
            .with_node(chain_node)
            .with_node(mark_node)
            .with_edge(contains_edge);

        sink.emit(emission).await?;
        Ok(())
    }

    fn transform_events(&self, events: &[GraphEvent], _context: &Context) -> Vec<OutboundEvent> {
        let mut outbound = Vec::new();
        for event in events {
            if let GraphEvent::NodesAdded { node_ids, adapter_id, .. } = event {
                if adapter_id != &self.adapter_id {
                    continue;
                }
                // Fragment indexed
                let fragments: Vec<String> = node_ids
                    .iter()
                    .filter(|id| id.to_string().starts_with("fragment:"))
                    .map(|id| id.to_string())
                    .collect();
                for frag_id in fragments {
                    outbound.push(OutboundEvent::new("fragment_indexed", frag_id));
                }
                // Concepts detected
                let concepts: Vec<String> = node_ids
                    .iter()
                    .filter(|id| id.to_string().starts_with("concept:"))
                    .map(|id| id.to_string().strip_prefix("concept:").unwrap().to_string())
                    .collect();
                if !concepts.is_empty() {
                    outbound.push(OutboundEvent::new(
                        "concepts_detected",
                        concepts.join(", "),
                    ));
                }
            }
        }
        outbound
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
        // 5 nodes: 1 fragment + 2 concepts + 1 chain + 1 mark
        assert_eq!(ctx.node_count(), 5);

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

        // 2 tagged_with edges + 1 contains edge = 3 total
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

        // Provenance: chain + mark + contains edge
        let chain = ctx.get_node(&NodeId::from_string("chain:manual-fragment:default"));
        assert!(chain.is_some());
        let chain = chain.unwrap();
        assert_eq!(chain.dimension, dimension::PROVENANCE);

        let contains_edges: Vec<_> = ctx
            .edges
            .iter()
            .filter(|e| e.relationship == "contains")
            .collect();
        assert_eq!(contains_edges.len(), 1);
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
        // 8 nodes: 2 fragments + 3 concepts + 1 chain (shared source "default") + 2 marks
        assert_eq!(ctx.node_count(), 8);

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
        // 3 nodes: 1 fragment + 1 chain + 1 mark (no concepts — no tags)
        assert_eq!(ctx.node_count(), 3);
        // 1 edge: contains (chain → mark), no tagged_with edges
        assert_eq!(ctx.edge_count(), 1);

        let fragment_nodes: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.content_type == ContentType::Document)
            .collect();
        assert_eq!(fragment_nodes.len(), 1);

        // Mark exists but has no tags property
        let marks: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.node_type == "mark")
            .collect();
        assert_eq!(marks.len(), 1);
        assert!(!marks[0].properties.contains_key("tags"));
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
        // 5 nodes: 1 fragment + 2 concepts + 1 chain + 1 mark
        assert_eq!(ctx.node_count(), 5);
        // 3 edges: 2 tagged_with + 1 contains
        assert_eq!(ctx.edge_count(), 3);
    }

    // === Scenario: Re-ingesting the same fragment produces the same node (upsert, not duplicate) ===
    #[tokio::test]
    async fn idempotent_reingest() {
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

        // Ingest the same fragment twice
        adapter.process(&input, &sink).await.unwrap();
        adapter.process(&input, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // Still 5 nodes — upserted, not duplicated
        // 1 fragment + 2 concepts + 1 chain + 1 mark
        assert_eq!(ctx.node_count(), 5);
        // Still 3 edges — 2 tagged_with + 1 contains
        assert_eq!(ctx.edge_count(), 3);
    }

    // === Scenario: Different text with same tags produces different fragment nodes ===
    #[tokio::test]
    async fn different_text_different_fragment_id() {
        let adapter = FragmentAdapter::new("manual-fragment");
        let (sink, ctx) = make_sink("manual-fragment");

        let input1 = AdapterInput::new(
            "fragment",
            FragmentInput::new("Fragment A", vec!["travel".to_string()]),
            "test",
        );
        let input2 = AdapterInput::new(
            "fragment",
            FragmentInput::new("Fragment B", vec!["travel".to_string()]),
            "test",
        );

        adapter.process(&input1, &sink).await.unwrap();
        adapter.process(&input2, &sink).await.unwrap();

        let ctx = ctx.lock().unwrap();
        // 2 distinct fragment nodes (different text → different hash)
        let fragments: Vec<_> = ctx
            .nodes
            .values()
            .filter(|n| n.node_type == "fragment")
            .collect();
        assert_eq!(fragments.len(), 2);
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
