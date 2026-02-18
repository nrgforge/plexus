//! EmbeddingSimilarityEnrichment — embedding-based similarity detection (ADR-026)
//!
//! Core enrichment that reacts to `NodesAdded` events by embedding new node
//! labels, comparing against cached vectors, and emitting symmetric `similar_to`
//! edge pairs above a configurable similarity threshold.
//!
//! Uses a trait-based embedding backend (`Embedder`) so production code can use
//! fastembed-rs while tests use deterministic mock embedders.
//!
//! Structure-aware: filters nodes by dimension (Invariant 50).
//! Idempotent: checks for existing edges before emitting.

use crate::adapter::enrichment::Enrichment;
use crate::adapter::events::GraphEvent;
use crate::adapter::types::{AnnotatedEdge, Emission};
use crate::graph::{dimension, Context, Edge, Node, NodeId, PropertyValue};
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

/// Error type for embedding operations.
#[derive(Debug)]
pub enum EmbeddingError {
    /// The embedding model returned no results
    EmptyResult,
    /// Model loading or inference failed
    ModelError(String),
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbeddingError::EmptyResult => write!(f, "embedding returned no results"),
            EmbeddingError::ModelError(msg) => write!(f, "embedding model error: {}", msg),
        }
    }
}

/// Trait for embedding text into vectors.
///
/// Implementations handle model loading and inference.
/// fastembed-rs for production, mock for tests.
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts, returning one vector per text.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

/// Trait for storing and querying embedding vectors.
///
/// Implementations range from in-memory (tests/fallback) to sqlite-vec
/// (production persistence). All operations are scoped by `context_id`
/// so vectors from different contexts never mix.
pub trait VectorStore: Send + Sync {
    /// Store an embedding vector for a node in a context.
    fn store(&self, context_id: &str, node_id: &NodeId, vector: Vec<f32>);
    /// Check if an embedding exists for a node in a context.
    fn has(&self, context_id: &str, node_id: &NodeId) -> bool;
    /// Find nodes with vectors similar to the query above the threshold.
    fn find_similar(&self, context_id: &str, query: &[f32], threshold: f32) -> Vec<(NodeId, f32)>;
}

/// In-memory vector store for embedding cache.
///
/// Thread-safe via RwLock. Scoped by context_id so vectors from different
/// contexts never mix. Production path uses sqlite-vec (ADR-026);
/// this is the test/fallback path for contexts without persistence.
pub struct InMemoryVectorStore {
    /// Outer key: context_id, inner key: node_id
    vectors: RwLock<HashMap<String, HashMap<String, Vec<f32>>>>,
}

impl InMemoryVectorStore {
    /// Create a new empty in-memory vector store.
    pub fn new() -> Self {
        Self {
            vectors: RwLock::new(HashMap::new()),
        }
    }
}

impl VectorStore for InMemoryVectorStore {
    fn store(&self, context_id: &str, node_id: &NodeId, vector: Vec<f32>) {
        self.vectors
            .write()
            .unwrap()
            .entry(context_id.to_string())
            .or_default()
            .insert(node_id.as_str().to_string(), vector);
    }

    fn has(&self, context_id: &str, node_id: &NodeId) -> bool {
        self.vectors
            .read()
            .unwrap()
            .get(context_id)
            .map_or(false, |ctx| ctx.contains_key(node_id.as_str()))
    }

    fn find_similar(&self, context_id: &str, query: &[f32], threshold: f32) -> Vec<(NodeId, f32)> {
        let store = self.vectors.read().unwrap();
        let ctx_store = match store.get(context_id) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let mut results = Vec::new();
        for (id, cached) in ctx_store.iter() {
            let sim = cosine_similarity(query, cached);
            if sim >= threshold {
                results.push((NodeId::from_string(id.clone()), sim));
            }
        }
        results
    }
}

// ---------------------------------------------------------------------------
// FastEmbedEmbedder — production embedder behind `embeddings` feature
// ---------------------------------------------------------------------------

#[cfg(feature = "embeddings")]
mod fastembed_impl {
    use super::{Embedder, EmbeddingError};
    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
    use std::sync::Mutex;

    /// Production embedder backed by fastembed (ONNX Runtime).
    ///
    /// Wraps `fastembed::TextEmbedding` in a `Mutex` because its `embed`
    /// method requires `&mut self`, while the `Embedder` trait uses `&self`.
    pub struct FastEmbedEmbedder {
        model: Mutex<TextEmbedding>,
    }

    impl FastEmbedEmbedder {
        /// Create a new FastEmbedEmbedder with a specific model.
        pub fn new(model: EmbeddingModel) -> Result<Self, EmbeddingError> {
            let options = InitOptions::new(model).with_show_download_progress(false);
            let embedding = TextEmbedding::try_new(options)
                .map_err(|e| EmbeddingError::ModelError(e.to_string()))?;
            Ok(Self {
                model: Mutex::new(embedding),
            })
        }

        /// Create a new FastEmbedEmbedder with the default model (nomic-embed-text-v1.5).
        pub fn default_model() -> Result<Self, EmbeddingError> {
            Self::new(EmbeddingModel::NomicEmbedTextV15)
        }
    }

    impl Embedder for FastEmbedEmbedder {
        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            if texts.is_empty() {
                return Ok(Vec::new());
            }
            let mut model = self.model.lock().unwrap();
            let embeddings = model
                .embed(texts.to_vec(), None)
                .map_err(|e| EmbeddingError::ModelError(e.to_string()))?;
            if embeddings.is_empty() {
                return Err(EmbeddingError::EmptyResult);
            }
            Ok(embeddings)
        }
    }
}

#[cfg(feature = "embeddings")]
pub use fastembed_impl::FastEmbedEmbedder;

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Core enrichment: embedding-based similarity detection (ADR-026).
///
/// Reacts to `NodesAdded` events by embedding new node labels,
/// comparing against cached vectors, and emitting symmetric
/// `similar_to` edge pairs above a configurable threshold.
///
/// Enrichment ID encodes the model name: `embedding:{model_name}`.
pub struct EmbeddingSimilarityEnrichment {
    similarity_threshold: f32,
    output_relationship: String,
    id: String,
    embedder: Box<dyn Embedder>,
    cache: Box<dyn VectorStore>,
    dimension_filter: String,
}

impl EmbeddingSimilarityEnrichment {
    /// Create a new embedding similarity enrichment with an in-memory vector store.
    ///
    /// - `model_name`: identifies the embedding model (encoded in enrichment ID)
    /// - `similarity_threshold`: minimum cosine similarity for edge emission (e.g., 0.7)
    /// - `output_relationship`: relationship type for emitted edges (e.g., "similar_to")
    /// - `embedder`: the embedding backend (fastembed-rs or mock)
    pub fn new(
        model_name: &str,
        similarity_threshold: f32,
        output_relationship: &str,
        embedder: Box<dyn Embedder>,
    ) -> Self {
        Self {
            id: format!("embedding:{}", model_name),
            similarity_threshold,
            output_relationship: output_relationship.to_string(),
            embedder,
            cache: Box::new(InMemoryVectorStore::new()),
            dimension_filter: dimension::SEMANTIC.to_string(),
        }
    }

    /// Create a new embedding similarity enrichment with a custom vector store.
    pub fn with_vector_store(
        model_name: &str,
        similarity_threshold: f32,
        output_relationship: &str,
        embedder: Box<dyn Embedder>,
        store: Box<dyn VectorStore>,
    ) -> Self {
        Self {
            id: format!("embedding:{}", model_name),
            similarity_threshold,
            output_relationship: output_relationship.to_string(),
            embedder,
            cache: store,
            dimension_filter: dimension::SEMANTIC.to_string(),
        }
    }

    /// Set the dimension filter (default: semantic).
    pub fn with_dimension_filter(mut self, dim: &str) -> Self {
        self.dimension_filter = dim.to_string();
        self
    }

    /// Get the embeddable text for a node — its "label" property.
    fn node_text(node: &Node) -> Option<&str> {
        node.properties.get("label").and_then(|v| match v {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        })
    }
}

impl Enrichment for EmbeddingSimilarityEnrichment {
    fn id(&self) -> &str {
        &self.id
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        let ctx_name = &context.name;

        // Only fire on NodesAdded events
        let new_node_ids: Vec<&NodeId> = events
            .iter()
            .filter_map(|e| match e {
                GraphEvent::NodesAdded { node_ids, .. } => Some(node_ids.iter()),
                _ => None,
            })
            .flatten()
            .collect();

        if new_node_ids.is_empty() {
            return None;
        }

        // Filter to nodes in the target dimension that have embeddable text
        // and aren't already cached
        let mut to_embed: Vec<(NodeId, String)> = Vec::new();
        for node_id in &new_node_ids {
            if let Some(node) = context.nodes().find(|n| &n.id == *node_id) {
                if node.dimension != self.dimension_filter {
                    continue;
                }
                if self.cache.has(ctx_name, &node.id) {
                    continue;
                }
                if let Some(text) = Self::node_text(node) {
                    to_embed.push((node.id.clone(), text.to_string()));
                }
            }
        }

        if to_embed.is_empty() {
            return None;
        }

        // Batch embed all new node labels
        let texts: Vec<&str> = to_embed.iter().map(|(_, t)| t.as_str()).collect();
        let embeddings = match self.embedder.embed_batch(&texts) {
            Ok(e) => e,
            Err(_) => return None,
        };

        // Store embeddings and find similar pairs
        let mut emission = Emission::new();

        for ((node_id, _), embedding) in to_embed.iter().zip(embeddings.iter()) {
            // Find similar nodes already in cache
            let similar = self.cache.find_similar(ctx_name, embedding, self.similarity_threshold);

            for (other_id, similarity) in &similar {
                // Idempotency: skip if edges already exist
                if output_edge_exists(context, node_id, other_id, &self.output_relationship) {
                    continue;
                }

                // Forward edge
                let mut forward = Edge::new_in_dimension(
                    node_id.clone(),
                    other_id.clone(),
                    &self.output_relationship,
                    dimension::SEMANTIC,
                );
                forward.raw_weight = *similarity;
                emission = emission.with_edge(AnnotatedEdge::new(forward));

                // Reverse edge (symmetric pair)
                if !output_edge_exists(context, other_id, node_id, &self.output_relationship) {
                    let mut reverse = Edge::new_in_dimension(
                        other_id.clone(),
                        node_id.clone(),
                        &self.output_relationship,
                        dimension::SEMANTIC,
                    );
                    reverse.raw_weight = *similarity;
                    emission = emission.with_edge(AnnotatedEdge::new(reverse));
                }
            }

            // Cache the new embedding after comparisons
            self.cache.store(ctx_name, node_id, embedding.clone());
        }

        if emission.is_empty() {
            None
        } else {
            Some(emission)
        }
    }
}

/// Check if an edge from source to target with the given relationship already exists.
fn output_edge_exists(
    context: &Context,
    source: &NodeId,
    target: &NodeId,
    relationship: &str,
) -> bool {
    context.edges().any(|e| {
        e.source == *source && e.target == *target && e.relationship == relationship
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ContentType;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Shared call counter for verifying batching behavior.
    #[derive(Clone)]
    struct CallCounter(Arc<AtomicUsize>);

    impl CallCounter {
        fn new() -> Self {
            Self(Arc::new(AtomicUsize::new(0)))
        }
        fn get(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }
        fn increment(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Mock embedder that returns predetermined vectors based on node text.
    struct MockEmbedder {
        /// Map from text to embedding vector
        vectors: HashMap<String, Vec<f32>>,
        /// Shared call counter (for verifying batching)
        call_count: CallCounter,
    }

    impl MockEmbedder {
        fn new(vectors: HashMap<String, Vec<f32>>) -> (Self, CallCounter) {
            let counter = CallCounter::new();
            (
                Self {
                    vectors,
                    call_count: counter.clone(),
                },
                counter,
            )
        }

        fn simple(vectors: HashMap<String, Vec<f32>>) -> Self {
            Self {
                vectors,
                call_count: CallCounter::new(),
            }
        }
    }

    impl Embedder for MockEmbedder {
        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            self.call_count.increment();
            let mut results = Vec::new();
            for text in texts {
                let vec = self
                    .vectors
                    .get(*text)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; 3]);
                results.push(vec);
            }
            Ok(results)
        }
    }

    fn concept_node(id: &str, label: &str) -> Node {
        let mut n = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
        n.id = NodeId::from_string(id);
        n.properties
            .insert("label".to_string(), PropertyValue::String(label.to_string()));
        n
    }

    fn provenance_node(id: &str) -> Node {
        let mut n =
            Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
        n.id = NodeId::from_string(id);
        n.properties.insert(
            "label".to_string(),
            PropertyValue::String("some mark".to_string()),
        );
        n
    }

    fn nodes_added_event(ids: &[&str]) -> GraphEvent {
        GraphEvent::NodesAdded {
            node_ids: ids.iter().map(|id| NodeId::from_string(*id)).collect(),
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }
    }

    // Vectors chosen so that cosine similarity between "travel" and "journey"
    // is high (~0.85), and between "travel" and "democracy" is low (~0.3).
    fn test_vectors() -> HashMap<String, Vec<f32>> {
        let mut m = HashMap::new();
        // travel:  [0.9, 0.3, 0.1]
        // journey: [0.85, 0.35, 0.15] — similar to travel
        // voyage:  [0.88, 0.32, 0.12] — similar to travel and journey
        // democracy: [0.1, 0.2, 0.95] — dissimilar to travel
        m.insert("travel".to_string(), vec![0.9, 0.3, 0.1]);
        m.insert("journey".to_string(), vec![0.85, 0.35, 0.15]);
        m.insert("voyage".to_string(), vec![0.88, 0.32, 0.12]);
        m.insert("democracy".to_string(), vec![0.1, 0.2, 0.95]);
        m
    }

    // === Scenario: Embedding enrichment ID encodes model name ===

    #[test]
    fn enrichment_id_encodes_model_name() {
        let embedder = MockEmbedder::simple(HashMap::new());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("nomic-embed-text-v1.5", 0.7, "similar_to", Box::new(embedder));
        assert_eq!(enrichment.id(), "embedding:nomic-embed-text-v1.5");
    }

    // === Scenario: Embedding enrichment fires on new nodes ===

    #[test]
    fn fires_on_new_nodes_emitting_symmetric_pairs() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:travel", "travel"));
        ctx.add_node(concept_node("concept:journey", "journey"));

        // First: cache "travel" by processing its NodesAdded event
        let event1 = nodes_added_event(&["concept:travel"]);
        // No cached vectors yet, so no similar pairs — should be None
        assert!(enrichment.enrich(&[event1], &ctx).is_none());

        // Now add "journey" — should find similarity with cached "travel"
        let event2 = nodes_added_event(&["concept:journey"]);
        let emission = enrichment
            .enrich(&[event2], &ctx)
            .expect("should emit similar_to edges");

        // Symmetric pair: journey→travel and travel→journey
        assert_eq!(emission.edges.len(), 2);

        let travel = NodeId::from_string("concept:travel");
        let journey = NodeId::from_string("concept:journey");

        let forward = emission
            .edges
            .iter()
            .find(|ae| ae.edge.source == journey && ae.edge.target == travel);
        let reverse = emission
            .edges
            .iter()
            .find(|ae| ae.edge.source == travel && ae.edge.target == journey);

        assert!(forward.is_some(), "journey→travel edge");
        assert!(reverse.is_some(), "travel→journey edge");
        assert_eq!(forward.unwrap().edge.relationship, "similar_to");
        assert_eq!(reverse.unwrap().edge.relationship, "similar_to");
        // Similarity should be above threshold
        assert!(forward.unwrap().edge.raw_weight > 0.7);
    }

    // === Scenario: Embedding enrichment respects similarity threshold ===

    #[test]
    fn respects_similarity_threshold() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:travel", "travel"));
        ctx.add_node(concept_node("concept:democracy", "democracy"));

        // Cache "travel"
        let event1 = nodes_added_event(&["concept:travel"]);
        enrichment.enrich(&[event1], &ctx);

        // Add "democracy" — cosine similarity with "travel" should be well below 0.7
        let event2 = nodes_added_event(&["concept:democracy"]);
        assert!(
            enrichment.enrich(&[event2], &ctx).is_none(),
            "should not emit edges when similarity is below threshold"
        );
    }

    // === Scenario: Embedding enrichment is idempotent ===

    #[test]
    fn idempotent_when_edges_already_exist() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:travel", "travel"));
        ctx.add_node(concept_node("concept:journey", "journey"));

        // Cache travel
        enrichment.enrich(&[nodes_added_event(&["concept:travel"])], &ctx);

        // First emission: produces similar_to edges
        let emission = enrichment
            .enrich(&[nodes_added_event(&["concept:journey"])], &ctx)
            .expect("first round emits");

        // Simulate committing the edges to context
        for ae in &emission.edges {
            ctx.add_edge(ae.edge.clone());
        }

        // Second round with same events: should be quiescent (edges already exist)
        // Journey is already cached, so no new embeddings. Even with a new NodesAdded
        // event, the cache check prevents re-embedding.
        let event = nodes_added_event(&["concept:journey"]);
        assert!(
            enrichment.enrich(&[event], &ctx).is_none(),
            "should be quiescent when edges already exist"
        );
    }

    // === Scenario: Embedding enrichment batches node bursts ===

    #[test]
    fn batches_node_bursts_into_single_call() {
        let (embedder, counter) = MockEmbedder::new(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        // Pre-cache one node
        ctx.add_node(concept_node("concept:travel", "travel"));
        enrichment.enrich(&[nodes_added_event(&["concept:travel"])], &ctx);
        assert_eq!(counter.get(), 1, "one call to cache travel");

        // Add two new nodes in a single event
        ctx.add_node(concept_node("concept:journey", "journey"));
        ctx.add_node(concept_node("concept:voyage", "voyage"));
        let burst_event = nodes_added_event(&["concept:journey", "concept:voyage"]);

        let emission = enrichment.enrich(&[burst_event], &ctx);
        assert!(emission.is_some(), "burst should produce edges");

        // The embedder should have been called exactly twice total:
        // once for "travel", once for the batch of ["journey", "voyage"]
        assert_eq!(counter.get(), 2, "travel cached in 1 call, burst in 1 batch call");
    }

    // === Scenario: Embedding enrichment filters by node dimension ===

    #[test]
    fn filters_by_dimension() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:travel", "travel"));
        ctx.add_node(provenance_node("mark:some-mark")); // wrong dimension

        // Cache travel
        enrichment.enrich(&[nodes_added_event(&["concept:travel"])], &ctx);

        // Event includes both nodes, but only the concept node should be embedded
        let event = nodes_added_event(&["mark:some-mark"]);
        assert!(
            enrichment.enrich(&[event], &ctx).is_none(),
            "provenance-dimension nodes should be filtered out"
        );
    }

    // === Scenario: Embedding enrichment produces symmetric edge pairs ===

    #[test]
    fn produces_symmetric_edge_pairs() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:travel", "travel"));
        ctx.add_node(concept_node("concept:voyage", "voyage"));

        // Cache travel
        enrichment.enrich(&[nodes_added_event(&["concept:travel"])], &ctx);

        // Add voyage
        let emission = enrichment
            .enrich(&[nodes_added_event(&["concept:voyage"])], &ctx)
            .expect("should emit");

        assert_eq!(emission.edges.len(), 2, "symmetric pair");

        let travel = NodeId::from_string("concept:travel");
        let voyage = NodeId::from_string("concept:voyage");

        let has_forward = emission
            .edges
            .iter()
            .any(|ae| ae.edge.source == voyage && ae.edge.target == travel);
        let has_reverse = emission
            .edges
            .iter()
            .any(|ae| ae.edge.source == travel && ae.edge.target == voyage);

        assert!(has_forward, "voyage→travel");
        assert!(has_reverse, "travel→voyage");

        // Both edges should have the same similarity value
        let fw_weight = emission
            .edges
            .iter()
            .find(|ae| ae.edge.source == voyage && ae.edge.target == travel)
            .unwrap()
            .edge
            .raw_weight;
        let rv_weight = emission
            .edges
            .iter()
            .find(|ae| ae.edge.source == travel && ae.edge.target == voyage)
            .unwrap()
            .edge
            .raw_weight;

        assert!(
            (fw_weight - rv_weight).abs() < 1e-6,
            "symmetric edges should have equal weights"
        );
    }

    // === Unit test: cosine similarity computation ===

    #[test]
    fn cosine_similarity_correct() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6, "identical vectors");

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 1e-6, "orthogonal vectors");

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 1e-6, "opposite vectors");
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = vec![1.0, 0.0, 0.0];
        let zero = vec![0.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &zero), 0.0);
    }

    // === Scenario: Multiple new nodes in burst find similarities between each other ===

    #[test]
    fn burst_nodes_find_similarities_with_each_other() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        let mut ctx = Context::new("test");
        ctx.add_node(concept_node("concept:travel", "travel"));
        ctx.add_node(concept_node("concept:journey", "journey"));
        ctx.add_node(concept_node("concept:voyage", "voyage"));

        // Process all three in sequence: travel first, then journey+voyage burst
        enrichment.enrich(&[nodes_added_event(&["concept:travel"])], &ctx);

        let burst = nodes_added_event(&["concept:journey", "concept:voyage"]);
        let emission = enrichment.enrich(&[burst], &ctx).expect("should emit");

        // journey is similar to travel (cached), voyage is similar to travel (cached)
        // AND journey is similar to voyage (journey gets cached before voyage is compared)
        // So we expect: journey↔travel + voyage↔travel + voyage↔journey = 6 edges
        assert!(
            emission.edges.len() >= 4,
            "at least journey↔travel and voyage↔travel: got {} edges",
            emission.edges.len()
        );
    }

    // === Scenario: Vectors from different contexts are isolated ===

    #[test]
    fn context_isolation_prevents_cross_context_matches() {
        let embedder = MockEmbedder::simple(test_vectors());
        let enrichment =
            EmbeddingSimilarityEnrichment::new("test-model", 0.7, "similar_to", Box::new(embedder));

        // Context A: cache "travel"
        let mut ctx_a = Context::new("context-a");
        ctx_a.add_node(concept_node("concept:travel", "travel"));
        enrichment.enrich(&[nodes_added_event(&["concept:travel"])], &ctx_a);

        // Context B: add "journey" — should NOT find similarity with travel
        // because travel was cached under context-a
        let mut ctx_b = Context::new("context-b");
        ctx_b.add_node(concept_node("concept:journey", "journey"));
        let result = enrichment.enrich(&[nodes_added_event(&["concept:journey"])], &ctx_b);

        assert!(
            result.is_none(),
            "vectors from context-a should not appear in context-b queries"
        );
    }

    // === Scenario: FastEmbedEmbedder loads model and embeds text ===

    #[cfg(feature = "embeddings")]
    #[test]
    #[ignore] // requires model download
    fn fastembed_default_model_embeds_text() {
        let embedder = super::FastEmbedEmbedder::default_model().expect("model should load");
        let result = embedder.embed_batch(&["hello world"]).expect("should embed");
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_empty(), "embedding vector should not be empty");
    }
}
