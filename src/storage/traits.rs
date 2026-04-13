//! Storage trait definitions

use crate::graph::{Context, ContextId};
use crate::query::{CursorFilter, PersistedEvent};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during storage operations
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Context not found: {0}")]
    ContextNotFound(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Date parsing error: {0}")]
    DateParse(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

/// Trait for graph storage backends
///
/// Implementations must be thread-safe (Send + Sync) to support
/// concurrent access from multiple threads.
pub trait GraphStore: Send + Sync {
    // === Context Operations ===

    /// Create or update a context
    fn save_context(&self, context: &Context) -> StorageResult<()>;

    /// Update only the context row (name, description, metadata) without touching nodes/edges
    fn save_context_metadata(&self, context: &Context) -> StorageResult<()>;

    /// Load a context by ID
    fn load_context(&self, id: &ContextId) -> StorageResult<Option<Context>>;

    /// Delete a context and all its nodes/edges
    fn delete_context(&self, id: &ContextId) -> StorageResult<bool>;

    /// List all context IDs
    fn list_contexts(&self) -> StorageResult<Vec<ContextId>>;

    // === Coherence ===

    /// Return the database version counter for cache coherence (ADR-017 §2).
    ///
    /// For SQLite, this is `PRAGMA data_version` — a connection-local counter
    /// that increases when another connection modifies the database.
    /// Non-SQLite backends should return a monotonic version counter.
    /// Returns 0 by default (no coherence tracking).
    fn data_version(&self) -> StorageResult<u64> {
        Ok(0)
    }

    // === Event Cursor Operations (ADR-035) ===

    /// Persist a graph event to the event log.
    ///
    /// Returns the assigned sequence number. Default no-op returns 0 —
    /// backends that don't support event persistence silently skip it.
    fn persist_event(
        &self,
        context_id: &str,
        event_type: &str,
        node_ids: &[String],
        edge_ids: &[String],
        adapter_id: &str,
    ) -> StorageResult<u64> {
        let _ = (context_id, event_type, node_ids, edge_ids, adapter_id);
        Ok(0)
    }

    /// Query events after the given cursor (sequence number).
    ///
    /// Returns events with sequence > cursor, ordered by sequence ascending.
    /// Default no-op returns empty vec.
    fn query_events_since(
        &self,
        context_id: &str,
        cursor: u64,
        filter: Option<&CursorFilter>,
    ) -> StorageResult<Vec<PersistedEvent>> {
        let _ = (context_id, cursor, filter);
        Ok(Vec::new())
    }

    /// Return the latest sequence number for a context, or 0 if no events exist.
    fn latest_sequence(&self, context_id: &str) -> StorageResult<u64> {
        let _ = context_id;
        Ok(0)
    }

    // === Spec Persistence Operations (ADR-037) ===

    /// Persist a loaded spec to the specs table.
    ///
    /// Upserts by composite key `(context_id, adapter_id)`. Default no-op.
    fn persist_spec(&self, spec: &PersistedSpec) -> StorageResult<()> {
        let _ = spec;
        Ok(())
    }

    /// Query all persisted specs for a context.
    ///
    /// Returns specs ordered by `loaded_at` ascending. Default no-op returns empty vec.
    fn query_specs_for_context(&self, context_id: &str) -> StorageResult<Vec<PersistedSpec>> {
        let _ = context_id;
        Ok(Vec::new())
    }

    /// Delete a persisted spec by composite key `(context_id, adapter_id)`.
    ///
    /// Returns true if a row was deleted. Default no-op returns false.
    fn delete_spec(&self, context_id: &str, adapter_id: &str) -> StorageResult<bool> {
        let _ = (context_id, adapter_id);
        Ok(false)
    }
}

/// A persisted consumer spec row from the `specs` table (ADR-037 §2).
///
/// Struct rather than tuple to allow non-breaking schema evolution —
/// additional fields can be added without breaking callers.
#[derive(Debug, Clone)]
pub struct PersistedSpec {
    pub context_id: String,
    pub adapter_id: String,
    pub spec_yaml: String,
    pub loaded_at: String,
}

/// Extension trait for opening stores from paths
pub trait OpenStore: GraphStore + Sized {
    /// Open or create a store at the given path
    fn open(path: impl AsRef<Path>) -> StorageResult<Self>;

    /// Create an in-memory store (useful for testing)
    fn open_in_memory() -> StorageResult<Self>;
}
