//! Query types and result structures

use crate::graph::{Edge, Node, NodeId};
use super::filter::RankBy;

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

    /// Rank nodes within each depth level by the given dimension.
    ///
    /// Ranking reorders nodes within each level but does not change which
    /// nodes appear or which level they belong to. Ordering is descending.
    pub fn rank_by(&mut self, rank: RankBy, edges: &[Edge]) {
        use std::collections::HashMap;

        // Build a lookup: node_id → best score from incident edges in result
        let mut scores: HashMap<&NodeId, f64> = HashMap::new();

        for edge in edges {
            let score = match rank {
                RankBy::RawWeight => edge.combined_weight as f64,
                RankBy::Corroboration => edge.contributions.len() as f64,
            };
            // Use the max score across all incident edges for each node
            for node_id in [&edge.source, &edge.target] {
                let entry = scores.entry(node_id).or_insert(0.0);
                if score > *entry {
                    *entry = score;
                }
            }
        }

        for level in &mut self.levels {
            level.sort_by(|a, b| {
                let sa = scores.get(&a.id).copied().unwrap_or(0.0);
                let sb = scores.get(&b.id).copied().unwrap_or(0.0);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
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
