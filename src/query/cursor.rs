//! Event cursor types for pull-based change queries (ADR-035).
//!
//! Consumers query "what changed since sequence N" via `changes_since()`.
//! These types define the query parameters and results.

use serde::{Deserialize, Serialize};

/// A persisted graph event with a sequence number.
///
/// Represents a single entry in the event log. The `sequence` field is the
/// cursor — a monotonically increasing integer assigned by the database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedEvent {
    /// Monotonically increasing sequence number (the cursor value)
    pub sequence: u64,
    /// Context this event belongs to
    pub context_id: String,
    /// Event type: "NodesAdded", "EdgesAdded", "NodesRemoved", "EdgesRemoved",
    /// "WeightsChanged", "ContributionsRetracted"
    pub event_type: String,
    /// Affected node IDs (if applicable)
    pub node_ids: Vec<String>,
    /// Affected edge IDs (if applicable)
    pub edge_ids: Vec<String>,
    /// Which adapter or enrichment produced this event
    pub adapter_id: String,
    /// When the event was created
    pub created_at: String,
}

/// Filter for cursor queries.
#[derive(Debug, Clone, Default)]
pub struct CursorFilter {
    /// Only return events of these types
    pub event_types: Option<Vec<String>>,
    /// Only return events from this adapter/enrichment
    pub adapter_id: Option<String>,
    /// Maximum number of events to return
    pub limit: Option<usize>,
}

/// Result of a cursor query: events plus the latest sequence number.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeSet {
    /// Events matching the query, ordered by sequence
    pub events: Vec<PersistedEvent>,
    /// The highest sequence number in the result (or the cursor if no new events)
    pub latest_sequence: u64,
}

impl ChangeSet {
    /// Create an empty change set with the given cursor position.
    pub fn empty(cursor: u64) -> Self {
        Self {
            events: Vec::new(),
            latest_sequence: cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_event_constructible() {
        let event = PersistedEvent {
            sequence: 1,
            context_id: "test".to_string(),
            event_type: "NodesAdded".to_string(),
            node_ids: vec!["node-1".to_string()],
            edge_ids: vec![],
            adapter_id: "content-adapter".to_string(),
            created_at: "2026-03-28T00:00:00Z".to_string(),
        };
        assert_eq!(event.sequence, 1);
        assert_eq!(event.event_type, "NodesAdded");
    }

    #[test]
    fn cursor_filter_default_matches_all() {
        let filter = CursorFilter::default();
        assert!(filter.event_types.is_none());
        assert!(filter.adapter_id.is_none());
        assert!(filter.limit.is_none());
    }

    #[test]
    fn change_set_empty_preserves_cursor() {
        let cs = ChangeSet::empty(42);
        assert_eq!(cs.events.len(), 0);
        assert_eq!(cs.latest_sequence, 42);
    }
}
