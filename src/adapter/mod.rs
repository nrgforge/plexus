//! Semantic adapter layer
//!
//! Implements ADR-001: adapters transform domain-specific input into graph
//! mutations via sink-based progressive emission.

mod cancel;
pub mod cooccurrence;
pub mod declarative;
mod engine_sink;
mod enrichment;
mod events;
pub mod extraction;
pub mod fragment;
pub mod graph_analysis;
mod ingest;
#[cfg(test)]
mod integration_tests;
mod provenance;
pub mod provenance_adapter;
mod router;
pub mod semantic;
mod sink;
mod tag_bridger;
mod traits;
mod types;

pub use cancel::CancellationToken;
pub use engine_sink::EngineSink;
pub use enrichment::{Enrichment, EnrichmentRegistry};
pub use events::GraphEvent;
pub use provenance::{FrameworkContext, ProvenanceEntry};
pub use router::{InputRouter, RouteResult};
pub use traits::{Adapter, AdapterInput};
pub use sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
pub use types::{
    Annotation, AnnotatedEdge, AnnotatedNode, EdgeRemoval, Emission, OutboundEvent,
    PropertyUpdate, Removal,
};
pub use cooccurrence::CoOccurrenceEnrichment;
pub use declarative::DeclarativeAdapter;
pub use extraction::ExtractionCoordinator;
pub use graph_analysis::GraphAnalysisAdapter;
pub use fragment::{FragmentAdapter, FragmentInput};
pub use ingest::IngestPipeline;
pub use provenance_adapter::{ProvenanceAdapter, ProvenanceInput};
pub use tag_bridger::TagConceptBridger;
