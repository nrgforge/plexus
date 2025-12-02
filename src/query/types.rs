//! Query types and result structures

use crate::graph::{Edge, Node, NodeId};

/// Direction for edge traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Follow outgoing edges (source -> target)
    #[default]
    Outgoing,
    /// Follow incoming edges (target <- source)
    Incoming,
    /// Follow edges in both directions
    Both,
}

/// Result of a find query
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Nodes matching the query
    pub nodes: Vec<Node>,
    /// Total count (may differ from nodes.len() if limit applied)
    pub total_count: usize,
}

impl QueryResult {
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            total_count: 0,
        }
    }

    pub fn from_nodes(nodes: Vec<Node>) -> Self {
        let total_count = nodes.len();
        Self { nodes, total_count }
    }
}

/// Result of a traversal query
#[derive(Debug, Clone)]
pub struct TraversalResult {
    /// Starting node
    pub origin: NodeId,
    /// Nodes discovered at each depth level
    /// Level 0 = origin, Level 1 = immediate neighbors, etc.
    pub levels: Vec<Vec<Node>>,
    /// Edges traversed
    pub edges: Vec<Edge>,
}

impl TraversalResult {
    pub fn new(origin: NodeId) -> Self {
        Self {
            origin,
            levels: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Get all nodes across all levels (excluding origin)
    pub fn all_nodes(&self) -> Vec<&Node> {
        self.levels.iter().skip(1).flatten().collect()
    }

    /// Get nodes at a specific depth
    pub fn at_depth(&self, depth: usize) -> &[Node] {
        self.levels.get(depth).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get the maximum depth reached
    pub fn max_depth(&self) -> usize {
        self.levels.len().saturating_sub(1)
    }
}

/// Result of a path query
#[derive(Debug, Clone)]
pub struct PathResult {
    /// Whether a path was found
    pub found: bool,
    /// Nodes in the path from source to target (inclusive)
    pub path: Vec<Node>,
    /// Edges in the path
    pub edges: Vec<Edge>,
    /// Path length (number of hops)
    pub length: usize,
}

impl PathResult {
    pub fn not_found() -> Self {
        Self {
            found: false,
            path: Vec::new(),
            edges: Vec::new(),
            length: 0,
        }
    }

    pub fn found(path: Vec<Node>, edges: Vec<Edge>) -> Self {
        let length = edges.len();
        Self {
            found: true,
            path,
            edges,
            length,
        }
    }
}
