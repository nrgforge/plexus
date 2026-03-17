//! Semantic adapter layer
//!
//! Implements ADR-001: adapters transform domain-specific input into graph
//! mutations via sink-based progressive emission.
//!
//! Submodule structure (system design v1.0):
//! - sink/        — emission contract (AdapterSink, EngineSink, provenance)
//! - enrichment/  — enrichment contract and loop
//! - pipeline/    — ingest pipeline, input routing
//! - adapters/    — domain adapter implementations
//! - enrichments/ — core enrichment implementations
//! - types, traits, cancel — shared types at module root

mod cancel;
mod enrichment;
#[cfg(test)]
mod integration_tests;
mod pipeline;
mod sink;
mod traits;
mod types;

mod adapters;
mod enrichments;

// Re-exports: public API unchanged
pub use cancel::CancellationToken;
pub use sink::{EngineSink, FrameworkContext, ProvenanceEntry};
pub(crate) use enrichment::run_enrichment_loop;
pub use enrichment::{Enrichment, EnrichmentRegistry};
pub use crate::graph::events::GraphEvent;
pub use pipeline::{classify_input, ClassifyError, IngestPipeline, PipelineBuilder};
pub use traits::{Adapter, AdapterInput};
pub use sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
pub use types::{
    Annotation, AnnotatedEdge, AnnotatedNode, EdgeRemoval, Emission, OutboundEvent,
    PropertyUpdate, Removal, chain_node, concept_node, file_node, mark_node,
};

// Adapter submodule re-exports (preserve crate::adapter::<name>::* paths)
pub use adapters::content;
pub use adapters::declarative;
pub use adapters::extraction;
pub use adapters::graph_analysis;
pub use adapters::provenance_adapter;
pub use adapters::semantic;

// Flat adapter type re-exports
pub use content::{ContentAdapter, FragmentInput, normalize_chain_name};
pub use declarative::DeclarativeAdapter;
pub use extraction::ExtractionCoordinator;
pub use graph_analysis::{GraphAnalysisAdapter, run_analysis, export_graph_for_analysis};
pub use provenance_adapter::{ProvenanceAdapter, ProvenanceInput};

// Enrichment submodule re-exports (preserve crate::adapter::<name>::* paths)
pub use enrichments::cooccurrence;
pub use enrichments::discovery_gap;
pub use enrichments::embedding;
pub use enrichments::temporal_proximity;

// Flat enrichment type re-exports
pub use cooccurrence::CoOccurrenceEnrichment;
pub use discovery_gap::DiscoveryGapEnrichment;
pub use embedding::{Embedder, EmbeddingError, EmbeddingSimilarityEnrichment, InMemoryVectorStore, VectorStore};
#[cfg(feature = "embeddings")]
pub use embedding::FastEmbedEmbedder;
pub use temporal_proximity::TemporalProximityEnrichment;
