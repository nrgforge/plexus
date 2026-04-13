//! Unified ingest pipeline — routing, adapter dispatch, enrichment loop
//! orchestration, and outbound event transformation.
//!
//! All graph writes go through `IngestPipeline::ingest()` (Invariant 34).

mod builder;
mod ingest;
mod router;

pub use builder::{gather_persisted_specs, PipelineBuilder};
pub use ingest::IngestPipeline;
pub use router::{classify_input, ClassifyError};
