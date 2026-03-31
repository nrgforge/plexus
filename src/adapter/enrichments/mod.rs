//! Core enrichment implementations — reactive graph intelligence algorithms.
//!
//! Four core enrichments define what kind of knowledge graph engine Plexus is:
//! CoOccurrenceEnrichment, DiscoveryGapEnrichment,
//! TemporalProximityEnrichment, EmbeddingSimilarityEnrichment.

pub mod cooccurrence;
pub mod discovery_gap;
pub mod embedding;
pub mod lens;
pub mod temporal_proximity;
