//! Emission contract — how adapters push mutations into the engine.
//!
//! Implements the sink side of ADR-001: adapters call `emit()` with an Emission
//! and receive validation feedback. EngineSink is the production implementation.

mod contract;
pub(crate) mod engine_sink;
pub(crate) mod provenance;

pub use contract::{AdapterError, AdapterSink, EmitResult, Rejection, RejectionReason};
pub use engine_sink::EngineSink;
pub use provenance::{FrameworkContext, ProvenanceEntry};
