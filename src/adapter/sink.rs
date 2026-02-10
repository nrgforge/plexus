//! AdapterSink trait and emission result types
//!
//! The sink is the interface through which adapters push graph mutations
//! into the engine. `emit()` is async — the adapter awaits validation feedback.

use super::events::GraphEvent;
use super::provenance::ProvenanceEntry;
use super::types::Emission;
use crate::graph::NodeId;
use async_trait::async_trait;
use thiserror::Error;

/// Why an individual item in an emission was rejected.
#[derive(Debug, Clone, PartialEq)]
pub enum RejectionReason {
    /// Edge references a node that doesn't exist in the graph or emission
    MissingEndpoint(NodeId),
    /// ProposalSink rejected: relationship type not allowed
    InvalidRelationshipType(String),
    /// ProposalSink rejected: node removal not allowed
    RemovalNotAllowed,
    /// Adapter-side error (e.g., downcast failure)
    Other(String),
}

impl std::fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEndpoint(id) => write!(f, "missing endpoint {}", id),
            Self::InvalidRelationshipType(rel) => write!(f, "invalid relationship type: {}", rel),
            Self::RemovalNotAllowed => write!(f, "removal not allowed"),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

/// A single rejected item from an emission.
#[derive(Debug, Clone)]
pub struct Rejection {
    /// Human-readable description of what was rejected
    pub description: String,
    /// Why it was rejected
    pub reason: RejectionReason,
}

impl Rejection {
    pub fn new(description: impl Into<String>, reason: RejectionReason) -> Self {
        Self {
            description: description.into(),
            reason,
        }
    }
}

/// The result of an `emit()` call.
///
/// Describes what was committed and what was rejected.
/// Partial success is the normal case — valid items commit even when
/// some items are rejected.
#[derive(Debug, Clone)]
pub struct EmitResult {
    /// Number of nodes committed
    pub nodes_committed: usize,
    /// Number of edges committed
    pub edges_committed: usize,
    /// Number of removals committed
    pub removals_committed: usize,
    /// Items that were rejected, with reasons
    pub rejections: Vec<Rejection>,
    /// Provenance entries constructed for committed items
    pub provenance: Vec<(NodeId, ProvenanceEntry)>,
    /// Graph events fired by this emission
    pub events: Vec<GraphEvent>,
}

impl EmitResult {
    pub fn empty() -> Self {
        Self {
            nodes_committed: 0,
            edges_committed: 0,
            removals_committed: 0,
            rejections: Vec::new(),
            provenance: Vec::new(),
            events: Vec::new(),
        }
    }

    /// True if no items were rejected
    pub fn is_fully_committed(&self) -> bool {
        self.rejections.is_empty()
    }

    /// True if nothing was committed and nothing was rejected (empty emission)
    pub fn is_noop(&self) -> bool {
        self.nodes_committed == 0
            && self.edges_committed == 0
            && self.removals_committed == 0
            && self.rejections.is_empty()
    }
}

/// Errors from adapter processing (not from individual item rejection).
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("invalid input: expected different data type")]
    InvalidInput,
    #[error("adapter cancelled")]
    Cancelled,
    #[error("context not found: {0}")]
    ContextNotFound(String),
    #[error("adapter error: {0}")]
    Internal(String),
}

/// The interface through which adapters push graph mutations into the engine.
///
/// Adapters call `emit()` with an Emission and await validation feedback.
/// Each emission is validated and committed atomically — valid items commit,
/// invalid items are rejected individually.
#[async_trait]
pub trait AdapterSink: Send + Sync {
    /// Push an emission into the engine.
    ///
    /// Returns a result describing what was committed and what was rejected.
    /// The adapter can inspect rejections and act on them or ignore them.
    async fn emit(&self, emission: Emission) -> Result<EmitResult, AdapterError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_result_empty_is_noop() {
        let result = EmitResult::empty();
        assert!(result.is_noop());
        assert!(result.is_fully_committed());
    }

    #[test]
    fn emit_result_with_commits_is_not_noop() {
        let result = EmitResult {
            nodes_committed: 2,
            edges_committed: 1,
            removals_committed: 0,
            rejections: Vec::new(),
            provenance: Vec::new(),
            events: Vec::new(),
        };
        assert!(!result.is_noop());
        assert!(result.is_fully_committed());
    }

    #[test]
    fn emit_result_with_rejections_is_not_fully_committed() {
        let result = EmitResult {
            nodes_committed: 1,
            edges_committed: 0,
            removals_committed: 0,
            rejections: vec![Rejection::new(
                "edge A→Z",
                RejectionReason::MissingEndpoint(NodeId::from_string("Z")),
            )],
            provenance: Vec::new(),
            events: Vec::new(),
        };
        assert!(!result.is_fully_committed());
        assert!(!result.is_noop());
    }
}
