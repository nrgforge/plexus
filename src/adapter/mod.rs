//! Semantic adapter layer
//!
//! Implements ADR-001: adapters transform domain-specific input into graph
//! mutations via sink-based progressive emission.

mod cancel;
pub mod cooccurrence;
mod engine_sink;
mod events;
pub mod fragment;
#[cfg(test)]
mod integration_tests;
mod proposal_sink;
mod provenance;
mod router;
mod sink;
mod traits;
mod types;

pub use cancel::CancellationToken;
pub use engine_sink::EngineSink;
pub use events::GraphEvent;
pub use proposal_sink::ProposalSink;
pub use provenance::{FrameworkContext, ProvenanceEntry};
pub use router::{InputRouter, RouteResult};
pub use traits::{Adapter, AdapterInput};
pub use sink::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
pub use types::{
    Annotation, AnnotatedEdge, AnnotatedNode, Emission, Removal,
};
pub use cooccurrence::CoOccurrenceAdapter;
pub use fragment::{FragmentAdapter, FragmentInput};
