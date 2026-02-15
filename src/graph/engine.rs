//! PlexusEngine: The main entry point for the knowledge graph

use super::context::{Context, ContextId, ContextMetadata, Source};
use super::edge::Edge;
use super::node::NodeId;
use crate::query::{FindQuery, PathQuery, QueryResult, PathResult, TraversalResult, TraverseQuery};
use crate::storage::{GraphStore, StorageError};
use chrono::Utc;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
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

    #[error("{0}")]
    Other(String),
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
    /// Last observed data_version for cache coherence (ADR-017 §2)
    last_data_version: AtomicU64,
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
            last_data_version: AtomicU64::new(0),
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
            last_data_version: AtomicU64::new(0),
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

        // Record initial data_version for cache coherence (ADR-017 §2)
        if let Ok(v) = store.data_version() {
            self.last_data_version.store(v, std::sync::atomic::Ordering::Release);
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

    /// Execute a closure with mutable access to a context (ADR-006).
    ///
    /// Keeps DashMap internals private. After the closure completes,
    /// the context is automatically persisted to storage (if configured).
    /// This is the integration point for EngineSink's emission protocol.
    pub fn with_context_mut<R>(
        &self,
        id: &ContextId,
        f: impl FnOnce(&mut Context) -> R,
    ) -> PlexusResult<R> {
        let mut context = self.contexts.get_mut(id)
            .ok_or_else(|| PlexusError::ContextNotFound(id.clone()))?;

        let result = f(&mut context);

        // Persist-per-emission: save after mutation completes
        if let Some(ref store) = self.store {
            store.save_context(&context)?;
        }

        Ok(result)
    }

    /// Check `data_version` and reload all contexts if the database
    /// has been modified by another engine (ADR-017 §2).
    ///
    /// Returns `true` if a reload occurred, `false` if the cache was fresh.
    pub fn reload_if_changed(&self) -> PlexusResult<bool> {
        let Some(ref store) = self.store else {
            return Ok(false);
        };

        let current = store.data_version()?;
        let last = self.last_data_version.load(std::sync::atomic::Ordering::Acquire);

        if current == last {
            return Ok(false);
        }

        // Reload all contexts from storage
        let context_ids = store.list_contexts()?;
        for id in &context_ids {
            if let Some(context) = store.load_context(id)? {
                self.contexts.insert(id.clone(), context);
            }
        }

        // Remove contexts that no longer exist in storage
        let stored_ids: HashSet<ContextId> = context_ids.into_iter().collect();
        self.contexts.retain(|id, _| stored_ids.contains(id));

        self.last_data_version.store(current, std::sync::atomic::Ordering::Release);
        Ok(true)
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

    // === Context Metadata Operations ===

    /// Rename a context
    pub fn rename_context(&self, id: &ContextId, new_name: &str) -> PlexusResult<()> {
        let mut context = self.contexts.get_mut(id)
            .ok_or_else(|| PlexusError::ContextNotFound(id.clone()))?;

        context.name = new_name.to_string();
        context.metadata.updated_at = Some(Utc::now());

        if let Some(ref store) = self.store {
            store.save_context_metadata(&context)?;
        }

        Ok(())
    }

    /// Delete a context (alias for remove_context that returns bool)
    pub fn delete_context(&self, id: &ContextId) -> PlexusResult<bool> {
        Ok(self.remove_context(id)?.is_some())
    }

    /// Get a context's metadata
    pub fn get_context_metadata(&self, id: &ContextId) -> Option<ContextMetadata> {
        self.contexts.get(id).map(|r| r.metadata.clone())
    }

    /// Update a context's metadata
    pub fn update_context_metadata(&self, id: &ContextId, metadata: ContextMetadata) -> PlexusResult<()> {
        let mut context = self.contexts.get_mut(id)
            .ok_or_else(|| PlexusError::ContextNotFound(id.clone()))?;

        context.metadata = metadata;
        context.metadata.updated_at = Some(Utc::now());

        if let Some(ref store) = self.store {
            store.save_context_metadata(&context)?;
        }

        Ok(())
    }

    // === Query Operations ===

    /// Find nodes in a context matching the query criteria
    pub fn find_nodes(&self, context_id: &ContextId, query: FindQuery) -> PlexusResult<QueryResult> {
        let context = self.contexts.get(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;
        Ok(query.execute(&context))
    }

    /// Traverse the graph from a starting node
    pub fn traverse(&self, context_id: &ContextId, query: TraverseQuery) -> PlexusResult<TraversalResult> {
        let context = self.contexts.get(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;
        Ok(query.execute(&context))
    }

    /// Find a path between two nodes
    pub fn find_path(&self, context_id: &ContextId, query: PathQuery) -> PlexusResult<PathResult> {
        let context = self.contexts.get(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;
        Ok(query.execute(&context))
    }

    // === Source Management ===

    /// Add a source to a context
    pub fn add_source(&self, context_id: &ContextId, source: Source) -> PlexusResult<()> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        if !context.metadata.sources.contains(&source) {
            context.metadata.sources.push(source);
            context.metadata.updated_at = Some(Utc::now());

            if let Some(ref store) = self.store {
                store.save_context_metadata(&context)?;
            }
        }

        Ok(())
    }

    /// Remove a source from a context. Returns true if the source was found and removed.
    pub fn remove_source(&self, context_id: &ContextId, source: &Source) -> PlexusResult<bool> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        let before = context.metadata.sources.len();
        context.metadata.sources.retain(|s| s != source);
        let removed = context.metadata.sources.len() < before;

        if removed {
            context.metadata.updated_at = Some(Utc::now());
            if let Some(ref store) = self.store {
                store.save_context_metadata(&context)?;
            }
        }

        Ok(removed)
    }

    /// List all sources in a context
    pub fn list_sources(&self, context_id: &ContextId) -> PlexusResult<Vec<Source>> {
        let context = self.contexts.get(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;
        Ok(context.metadata.sources.clone())
    }

    // === Mutation Helpers ===

    /// Add a node to a context
    pub fn add_node(&self, context_id: &ContextId, node: super::node::Node) -> PlexusResult<NodeId> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        let id = context.add_node(node);

        // Persist if storage configured
        if let Some(ref store) = self.store {
            store.save_context(&context)?;
        }

        Ok(id)
    }

    /// Add an edge to a context
    pub fn add_edge(&self, context_id: &ContextId, edge: Edge) -> PlexusResult<()> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        context.add_edge(edge);

        // Persist if storage configured
        if let Some(ref store) = self.store {
            store.save_context(&context)?;
        }

        Ok(())
    }

    /// Apply a batch mutation to a context (single persist at end)
    ///
    /// This is more efficient than calling add_node/add_edge individually
    /// when you have multiple changes to make, as it only persists once.
    pub fn apply_mutation(
        &self,
        context_id: &ContextId,
        nodes: Vec<super::node::Node>,
        edges: Vec<Edge>,
    ) -> PlexusResult<(usize, usize)> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        // Apply all node changes in memory
        let node_count = nodes.len();
        for node in nodes {
            context.add_node(node);
        }

        // Apply all edge changes in memory
        let edge_count = edges.len();
        for edge in edges {
            context.add_edge(edge);
        }

        // Persist once at the end
        if let Some(ref store) = self.store {
            store.save_context(&context)?;
        }

        Ok((node_count, edge_count))
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

    // === Query Tests ===

    #[test]
    fn test_find_nodes_via_engine() {
        use crate::graph::{ContentType, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        ctx.add_node(Node::new("function", ContentType::Code));
        ctx.add_node(Node::new("function", ContentType::Code));
        ctx.add_node(Node::new("class", ContentType::Code));

        let ctx_id = engine.upsert_context(ctx).unwrap();

        let result = engine.find_nodes(&ctx_id, FindQuery::new().with_node_type("function")).unwrap();
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn test_traverse_via_engine() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));
        ctx.add_edge(Edge::new(id_a.clone(), id_b.clone(), "calls"));

        let ctx_id = engine.upsert_context(ctx).unwrap();

        let result = engine.traverse(&ctx_id, TraverseQuery::from(id_a).depth(1)).unwrap();
        assert!(result.levels.len() >= 1);
    }

    #[test]
    fn test_find_path_via_engine() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));
        let id_c = ctx.add_node(Node::new("node", ContentType::Code));
        ctx.add_edge(Edge::new(id_a.clone(), id_b.clone(), "calls"));
        ctx.add_edge(Edge::new(id_b.clone(), id_c.clone(), "calls"));

        let ctx_id = engine.upsert_context(ctx).unwrap();

        let result = engine.find_path(&ctx_id, PathQuery::between(id_a, id_c)).unwrap();
        assert!(result.found);
        assert_eq!(result.length, 2);
    }

    // === Mutation Helper Tests ===

    #[test]
    fn test_add_node_to_context() {
        use crate::graph::{ContentType, Node};

        let engine = PlexusEngine::new();
        let ctx = Context::new("test");
        let ctx_id = engine.upsert_context(ctx).unwrap();

        let node = Node::new("function", ContentType::Code);
        let node_id = engine.add_node(&ctx_id, node).unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&node_id).is_some());
    }

    #[test]
    fn test_add_edge_to_context() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");
        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));
        let ctx_id = engine.upsert_context(ctx).unwrap();

        let edge = Edge::new(id_a, id_b, "calls");
        engine.add_edge(&ctx_id, edge).unwrap();

        let ctx = engine.get_context(&ctx_id).unwrap();
        assert_eq!(ctx.edges.len(), 1);
    }

    // === Cache Coherence Tests (ADR-017 §2) ===

    #[test]
    fn test_reload_if_changed_detects_external_write() {
        use crate::graph::{ContentType, Node};

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("coherence.db");

        let store_a: Arc<dyn crate::storage::GraphStore> = Arc::new(SqliteStore::open(&db_path).unwrap());
        let store_b: Arc<dyn crate::storage::GraphStore> = Arc::new(SqliteStore::open(&db_path).unwrap());

        let engine_a = PlexusEngine::with_store(store_a);
        let engine_b = PlexusEngine::with_store(store_b);

        // Engine A creates context with 3 nodes
        let mut ctx = Context::new("shared");
        let ctx_id = ctx.id.clone();
        ctx.add_node(Node::new("concept", ContentType::Document));
        ctx.add_node(Node::new("concept", ContentType::Document));
        ctx.add_node(Node::new("concept", ContentType::Document));
        engine_a.upsert_context(ctx).unwrap();
        engine_a.load_all().unwrap(); // set initial data_version

        // Engine B loads, sees 3 nodes
        engine_b.load_all().unwrap();
        assert_eq!(engine_b.get_context(&ctx_id).unwrap().node_count(), 3);

        // Engine A adds 2 more nodes externally
        engine_a.with_context_mut(&ctx_id, |ctx| {
            ctx.add_node(Node::new("concept", ContentType::Document));
            ctx.add_node(Node::new("concept", ContentType::Document));
        }).unwrap();

        // Engine B detects change and reloads
        let reloaded = engine_b.reload_if_changed().unwrap();
        assert!(reloaded, "should detect external changes");
        assert_eq!(engine_b.get_context(&ctx_id).unwrap().node_count(), 5,
            "must see 5 nodes after reload");
    }

    #[test]
    fn test_reload_if_changed_noop_when_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("stable.db");

        let store: Arc<dyn crate::storage::GraphStore> = Arc::new(SqliteStore::open(&db_path).unwrap());
        let engine = PlexusEngine::with_store(store);

        engine.upsert_context(Context::new("test")).unwrap();
        engine.load_all().unwrap();

        let reloaded = engine.reload_if_changed().unwrap();
        assert!(!reloaded, "should not reload when no external writes occurred");
    }
}
