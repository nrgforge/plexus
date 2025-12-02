//! PlexusEngine: The main entry point for the knowledge graph

use super::context::{Context, ContextId};
use crate::storage::{GraphStore, StorageError};
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur in Plexus operations
#[derive(Debug, Error)]
pub enum PlexusError {
    #[error("Context not found: {0}")]
    ContextNotFound(ContextId),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
}

/// Result type for Plexus operations
pub type PlexusResult<T> = Result<T, PlexusError>;

/// The main Plexus engine
///
/// Manages contexts and provides operations for querying and modifying
/// the knowledge graph. Optionally backed by persistent storage.
pub struct PlexusEngine {
    /// All contexts managed by this engine (in-memory cache)
    contexts: DashMap<ContextId, Context>,
    /// Optional persistent storage backend
    store: Option<Arc<dyn GraphStore>>,
}

impl std::fmt::Debug for PlexusEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlexusEngine")
            .field("contexts", &self.contexts)
            .field("has_store", &self.store.is_some())
            .finish()
    }
}

impl Default for PlexusEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PlexusEngine {
    /// Create a new in-memory PlexusEngine (no persistence)
    pub fn new() -> Self {
        Self {
            contexts: DashMap::new(),
            store: None,
        }
    }

    /// Create a PlexusEngine with persistent storage
    ///
    /// The engine will automatically persist changes and can load
    /// existing data from storage.
    pub fn with_store(store: Arc<dyn GraphStore>) -> Self {
        Self {
            contexts: DashMap::new(),
            store: Some(store),
        }
    }

    /// Load all contexts from storage into memory
    ///
    /// Call this on startup to hydrate the in-memory cache from
    /// persistent storage. Returns the number of contexts loaded.
    pub fn load_all(&self) -> PlexusResult<usize> {
        let Some(ref store) = self.store else {
            return Ok(0);
        };

        let context_ids = store.list_contexts()?;
        let mut loaded = 0;

        for id in context_ids {
            if let Some(context) = store.load_context(&id)? {
                self.contexts.insert(id, context);
                loaded += 1;
            }
        }

        Ok(loaded)
    }

    /// Create or update a context
    ///
    /// If a context with the same ID already exists, it will be replaced.
    /// Automatically persists to storage if configured.
    pub fn upsert_context(&self, context: Context) -> PlexusResult<ContextId> {
        let id = context.id.clone();

        // Persist to storage first (if configured)
        if let Some(ref store) = self.store {
            store.save_context(&context)?;
        }

        // Update in-memory cache
        self.contexts.insert(id.clone(), context);
        Ok(id)
    }

    /// Get a context by ID
    ///
    /// Returns from in-memory cache. Use `load_all()` on startup
    /// to populate cache from storage.
    pub fn get_context(&self, id: &ContextId) -> Option<Context> {
        self.contexts.get(id).map(|r| r.clone())
    }

    /// Remove a context
    ///
    /// Removes from both in-memory cache and persistent storage.
    pub fn remove_context(&self, id: &ContextId) -> PlexusResult<Option<Context>> {
        // Remove from storage first (if configured)
        if let Some(ref store) = self.store {
            store.delete_context(id)?;
        }

        // Remove from in-memory cache
        Ok(self.contexts.remove(id).map(|(_, ctx)| ctx))
    }

    /// List all context IDs
    pub fn list_contexts(&self) -> Vec<ContextId> {
        self.contexts.iter().map(|r| r.key().clone()).collect()
    }

    /// Get the number of contexts
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// Check if a context exists
    pub fn has_context(&self, id: &ContextId) -> bool {
        self.contexts.contains_key(id)
    }

    /// Check if engine has persistent storage configured
    pub fn has_store(&self) -> bool {
        self.store.is_some()
    }

    /// Persist a specific context to storage
    ///
    /// Useful when modifying a context's contents (nodes/edges)
    /// after retrieving it with `get_context`.
    pub fn persist_context(&self, context: &Context) -> PlexusResult<()> {
        if let Some(ref store) = self.store {
            store.save_context(context)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{OpenStore, SqliteStore};

    #[test]
    fn test_create_engine() {
        let engine = PlexusEngine::new();
        assert_eq!(engine.context_count(), 0);
        assert!(!engine.has_store());
    }

    #[test]
    fn test_upsert_context() {
        let engine = PlexusEngine::new();
        let context = Context::new("test-context");
        let id = context.id.clone();

        let returned_id = engine.upsert_context(context).unwrap();
        assert_eq!(id, returned_id);
        assert_eq!(engine.context_count(), 1);
        assert!(engine.has_context(&id));
    }

    #[test]
    fn test_get_context() {
        let engine = PlexusEngine::new();
        let context = Context::new("test-context");
        let id = context.id.clone();

        engine.upsert_context(context).unwrap();

        let retrieved = engine.get_context(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-context");
    }

    #[test]
    fn test_remove_context() {
        let engine = PlexusEngine::new();
        let context = Context::new("test-context");
        let id = context.id.clone();

        engine.upsert_context(context).unwrap();
        assert_eq!(engine.context_count(), 1);

        let removed = engine.remove_context(&id).unwrap();
        assert!(removed.is_some());
        assert_eq!(engine.context_count(), 0);
    }

    // === Storage Integration Tests ===

    #[test]
    fn test_engine_with_store() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = PlexusEngine::with_store(store);

        assert!(engine.has_store());
        assert_eq!(engine.context_count(), 0);
    }

    #[test]
    fn test_upsert_persists_to_store() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = PlexusEngine::with_store(store.clone());

        let context = Context::new("persisted-context");
        let id = context.id.clone();

        engine.upsert_context(context).unwrap();

        // Verify it was persisted to storage
        let from_store = store.load_context(&id).unwrap();
        assert!(from_store.is_some());
        assert_eq!(from_store.unwrap().name, "persisted-context");
    }

    #[test]
    fn test_remove_deletes_from_store() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = PlexusEngine::with_store(store.clone());

        let context = Context::new("to-delete");
        let id = context.id.clone();

        engine.upsert_context(context).unwrap();
        assert!(store.load_context(&id).unwrap().is_some());

        engine.remove_context(&id).unwrap();

        // Verify it was deleted from storage
        assert!(store.load_context(&id).unwrap().is_none());
    }

    #[test]
    fn test_load_all_hydrates_from_store() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());

        // First engine saves contexts to store
        {
            let engine = PlexusEngine::with_store(store.clone());
            engine.upsert_context(Context::new("context-1")).unwrap();
            engine.upsert_context(Context::new("context-2")).unwrap();
        }

        // Second engine loads from store
        let engine = PlexusEngine::with_store(store);
        assert_eq!(engine.context_count(), 0); // Not loaded yet

        let loaded = engine.load_all().unwrap();
        assert_eq!(loaded, 2);
        assert_eq!(engine.context_count(), 2);
    }

    #[test]
    fn test_load_all_without_store_returns_zero() {
        let engine = PlexusEngine::new();
        let loaded = engine.load_all().unwrap();
        assert_eq!(loaded, 0);
    }

    #[test]
    fn test_persist_context_updates_store() {
        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = PlexusEngine::with_store(store.clone());

        let mut context = Context::new("mutable-context");
        let id = context.id.clone();

        engine.upsert_context(context.clone()).unwrap();

        // Modify the context
        context.description = Some("Updated description".to_string());

        // Persist the updated version
        engine.persist_context(&context).unwrap();

        // Verify update was persisted
        let from_store = store.load_context(&id).unwrap().unwrap();
        assert_eq!(from_store.description, Some("Updated description".to_string()));
    }
}
