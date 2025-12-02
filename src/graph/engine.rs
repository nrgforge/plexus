//! PlexusEngine: The main entry point for the knowledge graph

use super::context::{Context, ContextId};
use super::edge::Edge;
use super::node::NodeId;
use crate::query::{FindQuery, PathQuery, QueryResult, PathResult, TraversalResult, TraverseQuery};
use crate::storage::{GraphStore, StorageError};
use chrono::Utc;
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

    // === Edge Reinforcement ===

    /// Reinforce an edge, increasing its strength
    ///
    /// This implements Hebbian learning: "neurons that fire together wire together"
    /// Each reinforcement increases the edge strength up to a maximum of 1.0.
    pub fn reinforce_edge(
        &self,
        context_id: &ContextId,
        edge_id: &str,
        amount: f32,
    ) -> PlexusResult<Option<f32>> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        // Find and reinforce the edge
        let edge = context.edges.iter_mut()
            .find(|e| e.id.as_str() == edge_id);

        let Some(edge) = edge else {
            return Ok(None);
        };

        // Increase strength (capped at 1.0)
        edge.strength = (edge.strength + amount).min(1.0);
        edge.last_reinforced = Utc::now();

        let new_strength = edge.strength;

        // Persist if storage configured
        if let Some(ref store) = self.store {
            store.save_context(&context)?;
        }

        Ok(Some(new_strength))
    }

    /// Apply decay to all edges in a context
    ///
    /// Edges that haven't been reinforced will gradually lose strength.
    /// This prevents the graph from becoming too interconnected over time.
    pub fn decay_edges(&self, context_id: &ContextId, decay_factor: f32) -> PlexusResult<usize> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        let mut decayed_count = 0;

        for edge in &mut context.edges {
            let old_strength = edge.strength;
            edge.strength = (edge.strength * (1.0 - decay_factor)).max(0.0);
            if edge.strength < old_strength {
                decayed_count += 1;
            }
        }

        // Persist if storage configured
        if self.store.is_some() && decayed_count > 0 {
            if let Some(ref store) = self.store {
                store.save_context(&context)?;
            }
        }

        Ok(decayed_count)
    }

    /// Prune weak edges below a threshold
    ///
    /// Removes edges with strength below the threshold.
    /// Returns the number of edges removed.
    pub fn prune_weak_edges(&self, context_id: &ContextId, threshold: f32) -> PlexusResult<usize> {
        let mut context = self.contexts.get_mut(context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(context_id.clone()))?;

        let original_count = context.edges.len();
        context.edges.retain(|e| e.strength >= threshold);
        let removed = original_count - context.edges.len();

        // Persist if storage configured
        if self.store.is_some() && removed > 0 {
            if let Some(ref store) = self.store {
                store.save_context(&context)?;
            }
        }

        Ok(removed)
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

    // === Edge Reinforcement Tests ===

    #[test]
    fn test_reinforce_edge() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        // Create edge with low initial strength
        let mut edge = Edge::new(id_a, id_b, "calls");
        edge.strength = 0.5;
        ctx.add_edge(edge);

        let ctx_id = engine.upsert_context(ctx).unwrap();

        // Get initial strength and edge ID from context
        let ctx_snapshot = engine.get_context(&ctx_id).unwrap();
        let initial_strength = ctx_snapshot.edges[0].strength;
        let edge_id = ctx_snapshot.edges[0].id.as_str().to_string();
        assert_eq!(initial_strength, 0.5);

        // Reinforce the edge
        let new_strength = engine.reinforce_edge(&ctx_id, &edge_id, 0.1).unwrap();
        assert!(new_strength.is_some(), "Edge should be found");
        assert_eq!(new_strength.unwrap(), 0.6);

        // Verify the context was actually updated
        let updated_ctx = engine.get_context(&ctx_id).unwrap();
        assert_eq!(updated_ctx.edges[0].strength, 0.6);
    }

    #[test]
    fn test_reinforce_edge_caps_at_one() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));
        let edge = Edge::new(id_a, id_b, "calls");
        let edge_id = edge.id.as_str().to_string();
        ctx.add_edge(edge);

        let ctx_id = engine.upsert_context(ctx).unwrap();

        // Reinforce heavily
        let new_strength = engine.reinforce_edge(&ctx_id, &edge_id, 10.0).unwrap();
        assert_eq!(new_strength, Some(1.0));
    }

    #[test]
    fn test_decay_edges() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));
        ctx.add_edge(Edge::new(id_a, id_b, "calls"));

        let ctx_id = engine.upsert_context(ctx).unwrap();

        // Get initial strength
        let initial_strength = engine.get_context(&ctx_id).unwrap().edges[0].strength;

        // Decay edges by 10%
        let decayed = engine.decay_edges(&ctx_id, 0.1).unwrap();
        assert_eq!(decayed, 1);

        // Check strength decreased
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.edges[0].strength < initial_strength);
    }

    #[test]
    fn test_prune_weak_edges() {
        use crate::graph::{ContentType, Edge, Node};

        let engine = PlexusEngine::new();
        let mut ctx = Context::new("test");

        let id_a = ctx.add_node(Node::new("node", ContentType::Code));
        let id_b = ctx.add_node(Node::new("node", ContentType::Code));

        let mut weak_edge = Edge::new(id_a.clone(), id_b.clone(), "weak");
        weak_edge.strength = 0.1;
        ctx.add_edge(weak_edge);

        let strong_edge = Edge::new(id_a, id_b, "strong");
        ctx.add_edge(strong_edge);

        let ctx_id = engine.upsert_context(ctx).unwrap();

        // Prune edges below 0.4 strength
        let pruned = engine.prune_weak_edges(&ctx_id, 0.4).unwrap();
        assert_eq!(pruned, 1);

        let ctx = engine.get_context(&ctx_id).unwrap();
        assert_eq!(ctx.edges.len(), 1);
        assert_eq!(ctx.edges[0].relationship, "strong");
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
}
