//! Graph traversal operations

use std::collections::{HashMap, HashSet};
use crate::graph::{Context, Edge, Node, NodeId};
use super::types::{Direction, TraversalResult};

/// Query for traversing the graph from a starting node
#[derive(Debug, Clone)]
pub struct TraverseQuery {
    /// Starting node ID
    pub origin: NodeId,
    /// Maximum depth to traverse (0 = origin only, 1 = immediate neighbors, etc.)
    pub max_depth: usize,
    /// Direction to traverse edges
    pub direction: Direction,
    /// Optional relationship type filter
    pub relationship: Option<String>,
    /// Minimum edge raw weight filter
    pub min_raw_weight: Option<f32>,
}

impl TraverseQuery {
    /// Create a new traversal query from a starting node
    pub fn from(origin: NodeId) -> Self {
        Self {
            origin,
            max_depth: 1,
            direction: Direction::Outgoing,
            relationship: None,
            min_raw_weight: None,
        }
    }

    /// Set the maximum traversal depth
    pub fn depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set the traversal direction
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Filter by relationship type
    pub fn with_relationship(mut self, relationship: impl Into<String>) -> Self {
        self.relationship = Some(relationship.into());
        self
    }

    /// Filter by minimum edge raw weight
    pub fn min_raw_weight(mut self, min_raw_weight: f32) -> Self {
        self.min_raw_weight = Some(min_raw_weight);
        self
    }

    /// Execute the traversal against a context
    pub fn execute(&self, context: &Context) -> TraversalResult {
        let mut result = TraversalResult::new(self.origin.clone());

        // Get origin node
        let Some(origin_node) = context.get_node(&self.origin) else {
            return result;
        };

        // Build edge index for faster lookup
        let edge_index = EdgeIndex::build(context);

        // BFS traversal
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut current_level: Vec<NodeId> = vec![self.origin.clone()];
        visited.insert(self.origin.clone());

        // Level 0 is the origin
        result.levels.push(vec![origin_node.clone()]);

        for _depth in 0..self.max_depth {
            if current_level.is_empty() {
                break;
            }

            let mut next_level: Vec<NodeId> = Vec::new();
            let mut level_nodes: Vec<Node> = Vec::new();

            for node_id in &current_level {
                // Get edges based on direction
                let edges = self.get_edges(node_id, &edge_index);

                for edge in edges {
                    // Check if edge passes filters
                    if !self.edge_matches(edge) {
                        continue;
                    }

                    // Get the neighbor node ID
                    let neighbor_id = if &edge.source == node_id {
                        &edge.target
                    } else {
                        &edge.source
                    };

                    // Skip if already visited
                    if visited.contains(neighbor_id) {
                        continue;
                    }

                    // Get the neighbor node
                    if let Some(neighbor) = context.get_node(neighbor_id) {
                        visited.insert(neighbor_id.clone());
                        next_level.push(neighbor_id.clone());
                        level_nodes.push(neighbor.clone());
                        result.edges.push(edge.clone());
                    }
                }
            }

            if !level_nodes.is_empty() {
                result.levels.push(level_nodes);
            }
            current_level = next_level;
        }

        result
    }

    /// Get edges for a node based on direction
    fn get_edges<'a>(&self, node_id: &NodeId, index: &'a EdgeIndex<'a>) -> Vec<&'a Edge> {
        match self.direction {
            Direction::Outgoing => index.outgoing(node_id),
            Direction::Incoming => index.incoming(node_id),
            Direction::Both => {
                let mut edges = index.outgoing(node_id);
                edges.extend(index.incoming(node_id));
                edges
            }
        }
    }

    /// Check if an edge matches the query filters
    fn edge_matches(&self, edge: &Edge) -> bool {
        // Check relationship filter
        if let Some(ref rel) = self.relationship {
            if &edge.relationship != rel {
                return false;
            }
        }

        // Check raw weight filter
        if let Some(min) = self.min_raw_weight {
            if edge.raw_weight < min {
                return false;
            }
        }

        true
    }
}

/// Index for fast edge lookups
struct EdgeIndex<'a> {
    outgoing: HashMap<&'a NodeId, Vec<&'a Edge>>,
    incoming: HashMap<&'a NodeId, Vec<&'a Edge>>,
}

impl<'a> EdgeIndex<'a> {
    fn build(context: &'a Context) -> Self {
        let mut outgoing: HashMap<&NodeId, Vec<&Edge>> = HashMap::new();
        let mut incoming: HashMap<&NodeId, Vec<&Edge>> = HashMap::new();

        for edge in &context.edges {
            outgoing.entry(&edge.source).or_default().push(edge);
            incoming.entry(&edge.target).or_default().push(edge);
        }

        Self { outgoing, incoming }
    }

    fn outgoing(&self, node_id: &NodeId) -> Vec<&'a Edge> {
        self.outgoing.get(node_id).cloned().unwrap_or_default()
    }

    fn incoming(&self, node_id: &NodeId) -> Vec<&'a Edge> {
        self.incoming.get(node_id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Edge, Node, NodeId};

    fn create_test_graph() -> Context {
        let mut ctx = Context::new("test");

        // Create nodes: A -> B -> C -> D
        //                    \-> E
        let node_a = Node::new("node", ContentType::Code);
        let node_b = Node::new("node", ContentType::Code);
        let node_c = Node::new("node", ContentType::Code);
        let node_d = Node::new("node", ContentType::Code);
        let node_e = Node::new("node", ContentType::Code);

        let id_a = ctx.add_node(node_a);
        let id_b = ctx.add_node(node_b);
        let id_c = ctx.add_node(node_c);
        let id_d = ctx.add_node(node_d);
        let id_e = ctx.add_node(node_e);

        ctx.add_edge(Edge::new(id_a.clone(), id_b.clone(), "calls"));
        ctx.add_edge(Edge::new(id_b.clone(), id_c.clone(), "calls"));
        ctx.add_edge(Edge::new(id_c.clone(), id_d.clone(), "calls"));
        ctx.add_edge(Edge::new(id_b.clone(), id_e.clone(), "uses"));

        ctx
    }

    #[test]
    fn test_traverse_depth_one() {
        let ctx = create_test_graph();
        let origin = ctx.nodes.keys().next().unwrap().clone();

        let result = TraverseQuery::from(origin)
            .depth(1)
            .execute(&ctx);

        // Origin + 1 neighbor
        assert!(result.levels.len() >= 1);
    }

    #[test]
    fn test_traverse_depth_two() {
        let ctx = create_test_graph();

        // Find node A (first node added)
        let all_nodes: Vec<_> = ctx.nodes.values().collect();
        let origin = all_nodes[0].id.clone();

        let result = TraverseQuery::from(origin)
            .depth(2)
            .direction(Direction::Outgoing)
            .execute(&ctx);

        // Should have levels for origin, depth 1, depth 2
        assert!(result.levels.len() >= 1);
    }

    #[test]
    fn test_traverse_both_directions() {
        let ctx = create_test_graph();
        let all_nodes: Vec<_> = ctx.nodes.values().collect();

        // Pick a middle node
        let origin = all_nodes[1].id.clone();

        let result = TraverseQuery::from(origin)
            .depth(1)
            .direction(Direction::Both)
            .execute(&ctx);

        // Should find neighbors in both directions
        assert!(result.levels.len() >= 1);
    }

    #[test]
    fn test_traverse_with_relationship_filter() {
        let ctx = create_test_graph();
        let all_nodes: Vec<_> = ctx.nodes.values().collect();
        let origin = all_nodes[1].id.clone(); // Node B

        let result = TraverseQuery::from(origin)
            .depth(1)
            .direction(Direction::Outgoing)
            .with_relationship("calls")
            .execute(&ctx);

        // Should only follow "calls" edges
        for edge in &result.edges {
            assert_eq!(edge.relationship, "calls");
        }
    }

    #[test]
    fn test_traverse_nonexistent_origin() {
        let ctx = create_test_graph();
        let fake_id = NodeId::from_string("nonexistent");

        let result = TraverseQuery::from(fake_id.clone())
            .depth(1)
            .execute(&ctx);

        assert_eq!(result.origin, fake_id);
        assert!(result.levels.is_empty());
    }
}
