//! Acceptance test suite for Plexus contracts.
//!
//! Organized by contract area (ingest, extraction, enrichment, provenance,
//! contribution, persistence, degradation, query). Each module tests stable
//! domain invariants using the public API via `plexus::*`.

#[path = "acceptance/helpers.rs"]
mod helpers;
#[path = "acceptance/ingest.rs"]
mod ingest;
#[path = "acceptance/extraction.rs"]
mod extraction;
#[path = "acceptance/enrichment.rs"]
mod enrichment;
#[path = "acceptance/provenance.rs"]
mod provenance;
#[path = "acceptance/contribution.rs"]
mod contribution;
#[path = "acceptance/persistence.rs"]
mod persistence;
#[path = "acceptance/degradation.rs"]
mod degradation;
#[path = "acceptance/query.rs"]
mod query;
#[path = "acceptance/integration.rs"]
mod integration;
#[path = "acceptance/cursor.rs"]
mod cursor;
#[path = "acceptance/lens.rs"]
mod lens;
#[path = "acceptance/filter.rs"]
mod filter;
#[path = "acceptance/spec.rs"]
mod spec;
#[path = "acceptance/mcp_harness.rs"]
mod mcp_harness;
#[path = "acceptance/mcp_e2e.rs"]
mod mcp_e2e;
#[path = "acceptance/llm_orc_wiring.rs"]
mod llm_orc_wiring;
