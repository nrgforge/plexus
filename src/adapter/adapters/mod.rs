//! Domain adapter implementations — each transforms a specific input kind
//! into graph mutations.
//!
//! See ADR-001 (sink-based emission), ADR-022 (phased extraction),
//! ADR-028 (declarative adapter specs).

pub mod content;
pub mod declarative;
pub mod extraction;
pub mod graph_analysis;
pub mod provenance_adapter;
pub mod semantic;
pub mod structural;
