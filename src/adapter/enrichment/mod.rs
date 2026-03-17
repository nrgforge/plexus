//! Enrichment contract and loop — reactive graph intelligence after each emission.
//!
//! Enrichments react to graph events and produce additional mutations.
//! The loop runs until quiescence (all enrichments return None) or the
//! safety valve (max rounds) is reached.

mod traits;
pub(crate) mod enrichment_loop;

pub use traits::{Enrichment, EnrichmentRegistry};
pub(crate) use enrichment_loop::run_enrichment_loop;
