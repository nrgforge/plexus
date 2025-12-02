//! PlexusEngine: The main entry point for the knowledge graph

use super::context::{Context, ContextId};
use dashmap::DashMap;
use thiserror::Error;

/// Errors that can occur in Plexus operations
#[derive(Debug, Error)]
#[allow(dead_code)] // Will be used as API expands
pub enum PlexusError {
    #[error("Context not found: {0}")]
    ContextNotFound(ContextId),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for Plexus operations
#[allow(dead_code)] // Will be used as API expands
pub type PlexusResult<T> = Result<T, PlexusError>;

/// The main Plexus engine
///
/// Manages contexts and provides operations for querying and modifying
/// the knowledge graph.
#[derive(Debug, Default)]
pub struct PlexusEngine {
    /// All contexts managed by this engine
    contexts: DashMap<ContextId, Context>,
}

impl PlexusEngine {
    /// Create a new PlexusEngine
    pub fn new() -> Self {
        Self {
            contexts: DashMap::new(),
        }
    }

    /// Create or update a context
    ///
    /// If a context with the same ID already exists, it will be replaced.
    /// Returns the context ID.
    pub fn upsert_context(&self, context: Context) -> ContextId {
        let id = context.id;
        self.contexts.insert(id, context);
        id
    }

    /// Get a context by ID
    pub fn get_context(&self, id: &ContextId) -> Option<Context> {
        self.contexts.get(id).map(|r| r.clone())
    }

    /// Remove a context
    pub fn remove_context(&self, id: &ContextId) -> Option<Context> {
        self.contexts.remove(id).map(|(_, ctx)| ctx)
    }

    /// List all context IDs
    pub fn list_contexts(&self) -> Vec<ContextId> {
        self.contexts.iter().map(|r| *r.key()).collect()
    }

    /// Get the number of contexts
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// Check if a context exists
    pub fn has_context(&self, id: &ContextId) -> bool {
        self.contexts.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_engine() {
        let engine = PlexusEngine::new();
        assert_eq!(engine.context_count(), 0);
    }

    #[test]
    fn test_upsert_context() {
        let engine = PlexusEngine::new();
        let context = Context::new("test-context");
        let id = context.id;

        let returned_id = engine.upsert_context(context);
        assert_eq!(id, returned_id);
        assert_eq!(engine.context_count(), 1);
        assert!(engine.has_context(&id));
    }

    #[test]
    fn test_get_context() {
        let engine = PlexusEngine::new();
        let context = Context::new("test-context");
        let id = context.id;

        engine.upsert_context(context);

        let retrieved = engine.get_context(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-context");
    }

    #[test]
    fn test_remove_context() {
        let engine = PlexusEngine::new();
        let context = Context::new("test-context");
        let id = context.id;

        engine.upsert_context(context);
        assert_eq!(engine.context_count(), 1);

        let removed = engine.remove_context(&id);
        assert!(removed.is_some());
        assert_eq!(engine.context_count(), 0);
    }
}
