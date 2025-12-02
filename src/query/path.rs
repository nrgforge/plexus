//! Path finding algorithms

use std::collections::{HashMap, HashSet, VecDeque};
use crate::graph::{Context, Edge, Node, NodeId};
use super::types::{Direction, PathResult};

/// Query for finding paths between nodes
#[derive(Debug, Clone)]
pub struct PathQuery {
    /// Source node ID
    pub source: NodeId,
    /// Target node ID
    pub target: NodeId,
    /// Maximum path length to search
    pub max_length: usize,
    /// Direction to traverse edges
    pub direction: Direction,
    /// Optional relationship type filter
    pub relationship: Option<String>,
}

impl PathQuery {
    /// Create a new path query between two nodes
    pub fn between(source: NodeId, target: NodeId) -> Self {
        Self {
            source,
            target,
            max_length: 10, // Default max
            direction: Direction::Outgoing,
            relationship: None,
        }
    }

    /// Set maximum path length
    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Set traversal direction
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Filter by relationship type
    pub fn with_relationship(mut self, relationship: impl Into<String>) -> Self {
        self.relationship = Some(relationship.into());
        self
    }

    /// Execute the path query (BFS for shortest path)
    pub fn execute(&self, context: &Context) -> PathResult {
        // Quick checks
        if self.source == self.target {
            if let Some(node) = context.get_node(&self.source) {
                return PathResult::found(vec![node.clone()], vec![]);
            }
            return PathResult::not_found();
        }

        if context.get_node(&self.source).is_none() || context.get_node(&self.target).is_none() {
            return PathResult::not_found();
        }

        // Build edge index
        let edge_index = EdgeIndex::build(context, &self.relationship);

        // BFS to find shortest path
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        let mut predecessors: HashMap<NodeId, (NodeId, Edge)> = HashMap::new();

        visited.insert(self.source.clone());
        queue.push_back(self.source.clone());

        let mut found = false;
        let mut depth = 0;

        while !queue.is_empty() && depth < self.max_length {
            let level_size = queue.len();

            for _ in 0..level_size {
                let current = queue.pop_front().unwrap();

                // Get neighbors based on direction
                let edges = self.get_edges(&current, &edge_index);

                for edge in edges {
                    let neighbor = if edge.source == current {
                        &edge.target
                    } else {
                        &edge.source
                    };

                    if visited.contains(neighbor) {
                        continue;
                    }

                    visited.insert(neighbor.clone());
                    predecessors.insert(neighbor.clone(), (current.clone(), edge.clone()));
                    queue.push_back(neighbor.clone());

                    if neighbor == &self.target {
                        found = true;
                        break;
                    }
                }

                if found {
                    break;
                }
            }

            if found {
                break;
            }

            depth += 1;
        }

        if !found {
            return PathResult::not_found();
        }

        // Reconstruct path
        self.reconstruct_path(context, &predecessors)
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

    /// Reconstruct the path from predecessors map
    fn reconstruct_path(
        &self,
        context: &Context,
        predecessors: &HashMap<NodeId, (NodeId, Edge)>,
    ) -> PathResult {
        let mut path_nodes: Vec<Node> = Vec::new();
        let mut path_edges: Vec<Edge> = Vec::new();

        // Walk backwards from target to source
        let mut current = self.target.clone();

        while let Some((pred, edge)) = predecessors.get(&current) {
            if let Some(node) = context.get_node(&current) {
                path_nodes.push(node.clone());
            }
            path_edges.push(edge.clone());
            current = pred.clone();
        }

        // Add source node
        if let Some(source_node) = context.get_node(&self.source) {
            path_nodes.push(source_node.clone());
        }

        // Reverse to get source -> target order
        path_nodes.reverse();
        path_edges.reverse();

        PathResult::found(path_nodes, path_edges)
    }
}

/// Index for fast edge lookups with optional relationship filter
struct EdgeIndex<'a> {
    outgoing: HashMap<NodeId, Vec<&'a Edge>>,
    incoming: HashMap<NodeId, Vec<&'a Edge>>,
}

impl<'a> EdgeIndex<'a> {
    fn build(context: &'a Context, relationship_filter: &Option<String>) -> Self {
        let mut outgoing: HashMap<NodeId, Vec<&Edge>> = HashMap::new();
        let mut incoming: HashMap<NodeId, Vec<&Edge>> = HashMap::new();

        for edge in &context.edges {
            // Apply relationship filter
            if let Some(ref rel) = relationship_filter {
                if &edge.relationship != rel {
                    continue;
                }
            }

            outgoing.entry(edge.source.clone()).or_default().push(edge);
            incoming.entry(edge.target.clone()).or_default().push(edge);
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
    use crate::graph::{ContentType, Edge, Node};

    fn create_test_graph() -> (Context, Vec<NodeId>) {
        use crate::graph::PropertyValue;

        let mut ctx = Context::new("test");

        // Create a graph: A -> B -> C -> D
        //                      \-> E -> F
        let mut ids = Vec::new();

        for name in ["A", "B", "C", "D", "E", "F"] {
            let node = Node::new("node", ContentType::Code)
                .with_property("name", PropertyValue::String(name.into()));
            ids.push(ctx.add_node(node));
        }

        // A -> B
        ctx.add_edge(Edge::new(ids[0].clone(), ids[1].clone(), "calls"));
        // B -> C
        ctx.add_edge(Edge::new(ids[1].clone(), ids[2].clone(), "calls"));
        // C -> D
        ctx.add_edge(Edge::new(ids[2].clone(), ids[3].clone(), "calls"));
        // B -> E
        ctx.add_edge(Edge::new(ids[1].clone(), ids[4].clone(), "uses"));
        // E -> F
        ctx.add_edge(Edge::new(ids[4].clone(), ids[5].clone(), "uses"));

        (ctx, ids)
    }

    #[test]
    fn test_path_same_node() {
        let (ctx, ids) = create_test_graph();
        let result = PathQuery::between(ids[0].clone(), ids[0].clone())
            .execute(&ctx);

        assert!(result.found);
        assert_eq!(result.length, 0);
        assert_eq!(result.path.len(), 1);
    }

    #[test]
    fn test_path_direct_neighbor() {
        let (ctx, ids) = create_test_graph();
        let result = PathQuery::between(ids[0].clone(), ids[1].clone())
            .execute(&ctx);

        assert!(result.found);
        assert_eq!(result.length, 1);
        assert_eq!(result.path.len(), 2);
    }

    #[test]
    fn test_path_two_hops() {
        let (ctx, ids) = create_test_graph();
        let result = PathQuery::between(ids[0].clone(), ids[2].clone())
            .execute(&ctx);

        assert!(result.found);
        assert_eq!(result.length, 2);
        assert_eq!(result.path.len(), 3); // A, B, C
    }

    #[test]
    fn test_path_three_hops() {
        let (ctx, ids) = create_test_graph();
        let result = PathQuery::between(ids[0].clone(), ids[3].clone())
            .execute(&ctx);

        assert!(result.found);
        assert_eq!(result.length, 3);
        assert_eq!(result.path.len(), 4); // A, B, C, D
    }

    #[test]
    fn test_path_not_found() {
        let (ctx, ids) = create_test_graph();
        // D -> A path doesn't exist (edges are directional)
        let result = PathQuery::between(ids[3].clone(), ids[0].clone())
            .direction(Direction::Outgoing)
            .execute(&ctx);

        assert!(!result.found);
    }

    #[test]
    fn test_path_with_max_length() {
        let (ctx, ids) = create_test_graph();
        // A to D requires 3 hops, but we limit to 2
        let result = PathQuery::between(ids[0].clone(), ids[3].clone())
            .max_length(2)
            .execute(&ctx);

        assert!(!result.found);
    }

    #[test]
    fn test_path_with_relationship_filter() {
        let (ctx, ids) = create_test_graph();
        // A to E goes through B, but B->E is "uses" not "calls"
        let result = PathQuery::between(ids[0].clone(), ids[4].clone())
            .with_relationship("calls")
            .execute(&ctx);

        // Should not find path since B->E edge is "uses"
        assert!(!result.found);
    }

    #[test]
    fn test_path_bidirectional() {
        let (ctx, ids) = create_test_graph();
        // D -> A with bidirectional traversal
        let result = PathQuery::between(ids[3].clone(), ids[0].clone())
            .direction(Direction::Both)
            .execute(&ctx);

        assert!(result.found);
        assert_eq!(result.length, 3); // D -> C -> B -> A
    }

    #[test]
    fn test_path_nonexistent_source() {
        let (ctx, ids) = create_test_graph();
        let fake_id = NodeId::from_string("nonexistent");

        let result = PathQuery::between(fake_id, ids[0].clone())
            .execute(&ctx);

        assert!(!result.found);
    }
}
