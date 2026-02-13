//! Provenance tracking: chains, marks, and links modeled as graph nodes/edges.

pub mod api;
pub mod types;

pub use api::ProvenanceApi;
pub use types::{ChainStatus, ChainView, MarkView};
