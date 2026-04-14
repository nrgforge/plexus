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
#[derive(Debug, Clone, serde::Serialize)]
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
#[derive(Debug, Clone, serde::Serialize)]
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
    ///
    /// The `context` parameter is required for `RankBy::NormalizedWeight` —
    /// per-node normalization strategies need the full neighborhood to compute
    /// each edge's normalized weight. `RawWeight` and `Corroboration` ignore
    /// it but accept it for signature uniformity.
    pub fn rank_by(&mut self, rank: RankBy, edges: &[Edge], context: &crate::graph::Context) {
        use crate::query::normalize::normalized_weights;
        use std::collections::HashMap;

        // Build a lookup: node_id → best score from incident edges in result
        let mut scores: HashMap<&NodeId, f64> = HashMap::new();

        // For NormalizedWeight, cache normalized-edge lookups per source node
        // (normalize() computes all outgoing edges from a source in one pass).
        let mut normalized_cache: HashMap<NodeId, HashMap<(NodeId, NodeId), f64>> =
            HashMap::new();

        for edge in edges {
            let score = match &rank {
                RankBy::RawWeight => edge.combined_weight as f64,
                RankBy::Corroboration => edge.contributions.len() as f64,
                RankBy::NormalizedWeight(strategy) => {
                    let per_source = normalized_cache
                        .entry(edge.source.clone())
                        .or_insert_with(|| {
                            normalized_weights(strategy.as_ref(), &edge.source, context)
                        });
                    per_source
                        .get(&(edge.source.clone(), edge.target.clone()))
                        .copied()
                        .unwrap_or(0.0)
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Context, ContentType, Edge, Node};
    use crate::query::normalize::OutgoingDivisive;

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    fn weighted_edge(source: &str, target: &str, w: f32) -> Edge {
        let mut e = Edge::new(
            NodeId::from_string(source),
            NodeId::from_string(target),
            "related_to",
        );
        e.combined_weight = w;
        e
    }

    // Two source nodes with different neighborhood densities:
    //
    // A → B (raw 3), A → C (raw 1)  → normalized A→B = 0.75, A→C = 0.25
    // X → Y (raw 2)                  → normalized X→Y = 1.0
    //
    // By RawWeight:        B(3) > Y(2) > C(1)
    // By NormalizedWeight: Y(1.0) > B(0.75) > C(0.25)
    //
    // This is the canonical motivating case for normalized ranking: X→Y is
    // weaker in raw terms but stronger relative to X's neighborhood.
    #[test]
    fn rank_by_normalized_weight_uses_outgoing_divisive() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_node(node("X"));
        ctx.add_node(node("Y"));
        ctx.add_edge(weighted_edge("A", "B", 3.0));
        ctx.add_edge(weighted_edge("A", "C", 1.0));
        ctx.add_edge(weighted_edge("X", "Y", 2.0));

        let mut result = TraversalResult::new(NodeId::from_string("origin"));
        result.levels = vec![
            vec![],
            vec![node("B"), node("C"), node("Y")],
        ];

        let edges: Vec<Edge> = ctx.edges.clone();

        result.rank_by(
            RankBy::NormalizedWeight(Box::new(OutgoingDivisive)),
            &edges,
            &ctx,
        );

        let level1_ids: Vec<String> =
            result.at_depth(1).iter().map(|n| n.id.to_string()).collect();

        assert_eq!(
            level1_ids,
            vec!["Y".to_string(), "B".to_string(), "C".to_string()],
            "normalized ranking should place Y (1.0) above B (0.75) above C (0.25)"
        );
    }

    // Verify the existing variants still work after the signature change.
    #[test]
    fn rank_by_raw_weight_ranks_by_combined_weight() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_edge(weighted_edge("A", "B", 5.0));
        ctx.add_edge(weighted_edge("A", "C", 1.0));

        let mut result = TraversalResult::new(NodeId::from_string("A"));
        result.levels = vec![vec![node("A")], vec![node("B"), node("C")]];

        let edges = ctx.edges.clone();
        result.rank_by(RankBy::RawWeight, &edges, &ctx);

        let level1_ids: Vec<String> =
            result.at_depth(1).iter().map(|n| n.id.to_string()).collect();
        assert_eq!(level1_ids, vec!["B".to_string(), "C".to_string()]);
    }
}

/// Result of a path query
#[derive(Debug, Clone, serde::Serialize)]
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
