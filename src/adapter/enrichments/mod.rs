//! Core enrichment implementations — reactive graph intelligence algorithms.
//!
//! Five core enrichments define what kind of knowledge graph engine Plexus is:
//! CoOccurrenceEnrichment, TagConceptBridger, DiscoveryGapEnrichment,
//! TemporalProximityEnrichment, EmbeddingSimilarityEnrichment.

pub mod cooccurrence;
pub mod discovery_gap;
pub mod embedding;
pub mod tag_bridger;
pub mod temporal_proximity;
