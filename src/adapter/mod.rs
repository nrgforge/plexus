//! Semantic adapter layer
//!
//! Implements ADR-001: adapters transform domain-specific input into graph
//! mutations via sink-based progressive emission.

mod cancel;
pub mod cooccurrence;
pub mod declarative;
pub mod discovery_gap;
pub mod embedding;
pub mod temporal_proximity;
mod enrichment;
mod enrichment_loop;
pub mod extraction;
pub mod content;
pub mod graph_analysis;
mod ingest;
#[cfg(test)]
mod integration_tests;
pub mod provenance_adapter;
mod router;
pub mod semantic;
mod sink;
mod tag_bridger;
mod traits;
mod types;

pub use cancel::CancellationToken;
pub use sink::{EngineSink, FrameworkContext, ProvenanceEntry};
pub(crate) use enrichment_loop::run_enrichment_loop;
pub use enrichment::{Enrichment, EnrichmentRegistry};
pub use crate::graph::events::GraphEvent;
pub use router::{classify_input, ClassifyError};
pub use traits::{Adapter, AdapterInput};
pub use sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
pub use types::{
    Annotation, AnnotatedEdge, AnnotatedNode, EdgeRemoval, Emission, OutboundEvent,
    PropertyUpdate, Removal, chain_node, concept_node, file_node, mark_node,
};
pub use cooccurrence::CoOccurrenceEnrichment;
pub use declarative::DeclarativeAdapter;
pub use discovery_gap::DiscoveryGapEnrichment;
pub use embedding::{Embedder, EmbeddingError, EmbeddingSimilarityEnrichment, InMemoryVectorStore, VectorStore};
#[cfg(feature = "embeddings")]
pub use embedding::FastEmbedEmbedder;
pub use temporal_proximity::TemporalProximityEnrichment;
pub use extraction::ExtractionCoordinator;
pub use graph_analysis::{GraphAnalysisAdapter, run_analysis, export_graph_for_analysis};
pub use content::{ContentAdapter, FragmentInput, normalize_chain_name};
pub use ingest::IngestPipeline;
pub use provenance_adapter::{ProvenanceAdapter, ProvenanceInput};
pub use tag_bridger::TagConceptBridger;
