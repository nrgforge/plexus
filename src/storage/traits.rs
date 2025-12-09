//! Storage trait definitions

use crate::graph::{Context, ContextId, Edge, Node, NodeId};
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
}

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

/// Filter criteria for querying nodes
#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    /// Filter by node type (e.g., "function", "class")
    pub node_type: Option<String>,
    /// Filter by content type (e.g., "code", "agent")
    pub content_type: Option<String>,
    /// Filter by dimension (e.g., "structure", "semantic", "relational")
    pub dimension: Option<String>,
    /// Maximum number of results
    pub limit: Option<usize>,
}

impl NodeFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = Some(node_type.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_dimension(mut self, dimension: impl Into<String>) -> Self {
        self.dimension = Some(dimension.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Filter criteria for querying edges
#[derive(Debug, Clone, Default)]
pub struct EdgeFilter {
    /// Filter by relationship type
    pub relationship: Option<String>,
    /// Filter by minimum strength
    pub min_strength: Option<f32>,
    /// Filter by source dimension
    pub source_dimension: Option<String>,
    /// Filter by target dimension
    pub target_dimension: Option<String>,
    /// If true, only return cross-dimensional edges
    pub cross_dimensional_only: Option<bool>,
    /// Maximum number of results
    pub limit: Option<usize>,
}

/// A subgraph extracted from the full graph
#[derive(Debug, Clone)]
pub struct Subgraph {
    /// Nodes in the subgraph
    pub nodes: Vec<Node>,
    /// Edges in the subgraph (only those where both endpoints are in nodes)
    pub edges: Vec<Edge>,
}

/// Trait for graph storage backends
///
/// Implementations must be thread-safe (Send + Sync) to support
/// concurrent access from multiple threads.
pub trait GraphStore: Send + Sync {
    // === Context Operations ===

    /// Create or update a context
    fn save_context(&self, context: &Context) -> StorageResult<()>;

    /// Load a context by ID
    fn load_context(&self, id: &ContextId) -> StorageResult<Option<Context>>;

    /// Delete a context and all its nodes/edges
    fn delete_context(&self, id: &ContextId) -> StorageResult<bool>;

    /// List all context IDs
    fn list_contexts(&self) -> StorageResult<Vec<ContextId>>;

    // === Node Operations ===

    /// Save a node (insert or update)
    fn save_node(&self, context_id: &ContextId, node: &Node) -> StorageResult<()>;

    /// Load a node by ID
    fn load_node(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<Option<Node>>;

    /// Delete a node and its edges
    fn delete_node(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<bool>;

    /// Find nodes matching filter criteria
    fn find_nodes(&self, context_id: &ContextId, filter: &NodeFilter) -> StorageResult<Vec<Node>>;

    // === Edge Operations ===

    /// Save an edge (insert or update)
    fn save_edge(&self, context_id: &ContextId, edge: &Edge) -> StorageResult<()>;

    /// Get edges originating from a node
    fn get_edges_from(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<Vec<Edge>>;

    /// Get edges targeting a node
    fn get_edges_to(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<Vec<Edge>>;

    /// Delete an edge
    fn delete_edge(&self, context_id: &ContextId, edge_id: &str) -> StorageResult<bool>;

    // === Subgraph Operations ===

    /// Load a subgraph starting from seed nodes, traversing up to max_depth hops
    fn load_subgraph(
        &self,
        context_id: &ContextId,
        seeds: &[NodeId],
        max_depth: usize,
    ) -> StorageResult<Subgraph>;
}

/// Extension trait for opening stores from paths
pub trait OpenStore: GraphStore + Sized {
    /// Open or create a store at the given path
    fn open(path: impl AsRef<Path>) -> StorageResult<Self>;

    /// Create an in-memory store (useful for testing)
    fn open_in_memory() -> StorageResult<Self>;
}
