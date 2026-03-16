//! Storage trait definitions

use crate::graph::{Context, ContextId};
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
}

/// Extension trait for opening stores from paths
pub trait OpenStore: GraphStore + Sized {
    /// Open or create a store at the given path
    fn open(path: impl AsRef<Path>) -> StorageResult<Self>;

    /// Create an in-memory store (useful for testing)
    fn open_in_memory() -> StorageResult<Self>;
}
