//! Graph metrics and algorithms for spike tests
//!
//! Implements PageRank, HITS, and other network science metrics
//! for validating the spike investigations.

use plexus::{Context, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};

/// PageRank result for a context
#[derive(Debug, Clone)]
pub struct PageRankResult {
    /// Node scores (NodeId -> score)
    pub scores: HashMap<NodeId, f64>,
    /// Iterations to convergence
    pub iterations: usize,
    /// Final convergence delta
    pub delta: f64,
}

impl PageRankResult {
    /// Get top-k nodes by PageRank score
    pub fn top_k(&self, k: usize) -> Vec<(NodeId, f64)> {
        let mut sorted: Vec<_> = self.scores.iter().map(|(id, s)| (id.clone(), *s)).collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(k);
        sorted
    }

    /// Get percentile rank of a node (0.0 = lowest, 1.0 = highest)
    pub fn percentile(&self, node_id: &NodeId) -> Option<f64> {
        let score = self.scores.get(node_id)?;
        let lower_count = self.scores.values().filter(|s| *s < score).count();
        Some(lower_count as f64 / self.scores.len() as f64)
    }

    /// Check if node is in top percentage (e.g., top 25%)
    pub fn is_in_top_percent(&self, node_id: &NodeId, percent: f64) -> bool {
        self.percentile(node_id)
            .map(|p| p >= (1.0 - percent))
            .unwrap_or(false)
    }
}

/// Compute PageRank for a context
///
/// # Arguments
/// * `context` - The graph context to analyze
/// * `damping` - Damping factor (typically 0.85)
/// * `max_iterations` - Maximum iterations
/// * `tolerance` - Convergence tolerance
pub fn pagerank(
    context: &Context,
    damping: f64,
    max_iterations: usize,
    tolerance: f64,
) -> PageRankResult {
    let n = context.nodes.len();
    if n == 0 {
        return PageRankResult {
            scores: HashMap::new(),
            iterations: 0,
            delta: 0.0,
        };
    }

    // Build adjacency lists
    let node_ids: Vec<NodeId> = context.nodes.keys().cloned().collect();
    let node_index: HashMap<&NodeId, usize> =
        node_ids.iter().enumerate().map(|(i, id)| (id, i)).collect();

    // Outgoing edges for each node
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); n];
    // Incoming edges for each node
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); n];

    for edge in &context.edges {
        if let (Some(&src_idx), Some(&dst_idx)) =
            (node_index.get(&edge.source), node_index.get(&edge.target))
        {
            outgoing[src_idx].push(dst_idx);
            incoming[dst_idx].push(src_idx);
        }
    }

    // Initialize scores uniformly
    let initial_score = 1.0 / n as f64;
    let mut scores: Vec<f64> = vec![initial_score; n];
    let mut new_scores: Vec<f64> = vec![0.0; n];

    let base = (1.0 - damping) / n as f64;
    let mut iterations = 0;
    let mut delta = f64::MAX;

    while iterations < max_iterations && delta > tolerance {
        // Handle dangling nodes (no outgoing edges)
        let dangling_sum: f64 = scores
            .iter()
            .enumerate()
            .filter(|(i, _)| outgoing[*i].is_empty())
            .map(|(_, s)| s)
            .sum();

        for i in 0..n {
            let mut sum = 0.0;
            for &j in &incoming[i] {
                let out_degree = outgoing[j].len() as f64;
                if out_degree > 0.0 {
                    sum += scores[j] / out_degree;
                }
            }
            // Add dangling contribution
            sum += dangling_sum / n as f64;
            new_scores[i] = base + damping * sum;
        }

        // Calculate delta and normalize
        delta = scores
            .iter()
            .zip(new_scores.iter())
            .map(|(old, new)| (old - new).abs())
            .sum();

        std::mem::swap(&mut scores, &mut new_scores);
        iterations += 1;
    }

    let result_scores: HashMap<NodeId, f64> = node_ids
        .into_iter()
        .zip(scores.into_iter())
        .collect();

    PageRankResult {
        scores: result_scores,
        iterations,
        delta,
    }
}

/// HITS (Hyperlink-Induced Topic Search) result
#[derive(Debug, Clone)]
pub struct HitsResult {
    /// Hub scores (nodes that point to many authorities)
    pub hub_scores: HashMap<NodeId, f64>,
    /// Authority scores (nodes pointed to by many hubs)
    pub authority_scores: HashMap<NodeId, f64>,
    /// Iterations to convergence
    pub iterations: usize,
}

impl HitsResult {
    /// Get top-k hubs
    pub fn top_hubs(&self, k: usize) -> Vec<(NodeId, f64)> {
        let mut sorted: Vec<_> = self
            .hub_scores
            .iter()
            .map(|(id, s)| (id.clone(), *s))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(k);
        sorted
    }

    /// Get top-k authorities
    pub fn top_authorities(&self, k: usize) -> Vec<(NodeId, f64)> {
        let mut sorted: Vec<_> = self
            .authority_scores
            .iter()
            .map(|(id, s)| (id.clone(), *s))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(k);
        sorted
    }
}

/// Compute HITS scores for a context
pub fn hits(context: &Context, max_iterations: usize) -> HitsResult {
    let n = context.nodes.len();
    if n == 0 {
        return HitsResult {
            hub_scores: HashMap::new(),
            authority_scores: HashMap::new(),
            iterations: 0,
        };
    }

    let node_ids: Vec<NodeId> = context.nodes.keys().cloned().collect();
    let node_index: HashMap<&NodeId, usize> =
        node_ids.iter().enumerate().map(|(i, id)| (id, i)).collect();

    // Build adjacency
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); n];

    for edge in &context.edges {
        if let (Some(&src_idx), Some(&dst_idx)) =
            (node_index.get(&edge.source), node_index.get(&edge.target))
        {
            outgoing[src_idx].push(dst_idx);
            incoming[dst_idx].push(src_idx);
        }
    }

    // Initialize
    let mut hub: Vec<f64> = vec![1.0; n];
    let mut auth: Vec<f64> = vec![1.0; n];

    for iter in 0..max_iterations {
        // Update authority scores
        let mut new_auth: Vec<f64> = vec![0.0; n];
        for i in 0..n {
            for &j in &incoming[i] {
                new_auth[i] += hub[j];
            }
        }

        // Update hub scores
        let mut new_hub: Vec<f64> = vec![0.0; n];
        for i in 0..n {
            for &j in &outgoing[i] {
                new_hub[i] += new_auth[j];
            }
        }

        // Normalize
        let auth_norm: f64 = new_auth.iter().map(|x| x * x).sum::<f64>().sqrt();
        let hub_norm: f64 = new_hub.iter().map(|x| x * x).sum::<f64>().sqrt();

        if auth_norm > 0.0 {
            new_auth.iter_mut().for_each(|x| *x /= auth_norm);
        }
        if hub_norm > 0.0 {
            new_hub.iter_mut().for_each(|x| *x /= hub_norm);
        }

        hub = new_hub;
        auth = new_auth;

        // Check convergence (simplified)
        if iter > 0 {
            let _ = iter; // Continue for all iterations in simple version
        }
    }

    HitsResult {
        hub_scores: node_ids.iter().cloned().zip(hub.into_iter()).collect(),
        authority_scores: node_ids.into_iter().zip(auth.into_iter()).collect(),
        iterations: max_iterations,
    }
}

/// Check if a node is reachable from another via BFS
pub fn is_reachable(context: &Context, from: &NodeId, to: &NodeId, max_depth: usize) -> bool {
    if from == to {
        return true;
    }

    // Build adjacency for outgoing edges
    let mut outgoing: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    for edge in &context.edges {
        outgoing
            .entry(&edge.source)
            .or_default()
            .push(&edge.target);
    }

    let mut visited: HashSet<&NodeId> = HashSet::new();
    let mut queue: VecDeque<(&NodeId, usize)> = VecDeque::new();

    queue.push_back((from, 0));
    visited.insert(from);

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        if let Some(neighbors) = outgoing.get(current) {
            for neighbor in neighbors {
                if *neighbor == to {
                    return true;
                }
                if !visited.contains(neighbor) {
                    visited.insert(neighbor);
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }
    }

    false
}

/// Count reachable nodes from a set of seed nodes
///
/// Only counts nodes that actually exist in context.nodes (ignores phantom targets)
pub fn reachable_count(context: &Context, seeds: &[NodeId], max_depth: usize) -> usize {
    // Build adjacency list using only edges where BOTH source and target exist
    let mut outgoing: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    for edge in &context.edges {
        // Only include edges where both endpoints exist in the graph
        if let (Some((src_id, _)), Some((tgt_id, _))) = (
            context.nodes.get_key_value(&edge.source),
            context.nodes.get_key_value(&edge.target),
        ) {
            outgoing.entry(src_id).or_default().push(tgt_id);
        }
    }

    let mut visited: HashSet<&NodeId> = HashSet::new();
    let mut queue: VecDeque<(&NodeId, usize)> = VecDeque::new();

    for seed in seeds {
        if let Some((node_id, _node)) = context.nodes.get_key_value(seed) {
            visited.insert(node_id);
            queue.push_back((node_id, 0));
        }
    }

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        if let Some(neighbors) = outgoing.get(current) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor);
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }
    }

    visited.len()
}

/// Calculate graph density (edges / max possible edges)
pub fn graph_density(context: &Context) -> f64 {
    let n = context.nodes.len();
    if n < 2 {
        return 0.0;
    }

    let max_edges = n * (n - 1); // Directed graph
    context.edges.len() as f64 / max_edges as f64
}

/// Calculate average degree (in + out / 2 for directed)
pub fn average_degree(context: &Context) -> f64 {
    let n = context.nodes.len();
    if n == 0 {
        return 0.0;
    }

    // Each edge contributes 1 to out-degree and 1 to in-degree
    // Average = 2 * edges / nodes
    2.0 * context.edges.len() as f64 / n as f64
}

/// Find connected components (treating graph as undirected)
pub fn connected_components(context: &Context) -> Vec<HashSet<NodeId>> {
    let mut adj: HashMap<&NodeId, HashSet<&NodeId>> = HashMap::new();

    // Build undirected adjacency
    for node_id in context.nodes.keys() {
        adj.entry(node_id).or_default();
    }
    for edge in &context.edges {
        adj.entry(&edge.source).or_default().insert(&edge.target);
        adj.entry(&edge.target).or_default().insert(&edge.source);
    }

    let mut visited: HashSet<&NodeId> = HashSet::new();
    let mut components: Vec<HashSet<NodeId>> = Vec::new();

    for node_id in context.nodes.keys() {
        if visited.contains(node_id) {
            continue;
        }

        let mut component: HashSet<NodeId> = HashSet::new();
        let mut stack: Vec<&NodeId> = vec![node_id];

        while let Some(current) = stack.pop() {
            if visited.contains(current) {
                continue;
            }
            visited.insert(current);
            component.insert(current.clone());

            if let Some(neighbors) = adj.get(current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }

        components.push(component);
    }

    components
}

#[cfg(test)]
mod tests {
    use super::*;
    use plexus::{ContentType, Edge, Node};

    fn create_test_context() -> (Context, NodeId, NodeId, NodeId) {
        let mut ctx = Context::new("test");

        // Create a simple graph: A -> B -> C
        let a = ctx.add_node(Node::new("document", ContentType::Document));
        let b = ctx.add_node(Node::new("document", ContentType::Document));
        let c = ctx.add_node(Node::new("document", ContentType::Document));

        ctx.add_edge(Edge::new(a.clone(), b.clone(), "links_to"));
        ctx.add_edge(Edge::new(b.clone(), c.clone(), "links_to"));

        (ctx, a, b, c)
    }

    #[test]
    fn test_pagerank() {
        let (ctx, _, _, _) = create_test_context();
        let result = pagerank(&ctx, 0.85, 100, 1e-6);

        assert_eq!(result.scores.len(), 3);
        // All scores should be positive
        assert!(result.scores.values().all(|s| *s > 0.0));
    }

    #[test]
    fn test_hits() {
        let (ctx, _, _, _) = create_test_context();
        let result = hits(&ctx, 20);

        assert_eq!(result.hub_scores.len(), 3);
        assert_eq!(result.authority_scores.len(), 3);
    }

    #[test]
    fn test_reachability() {
        let (ctx, a, _b, c) = create_test_context();

        // A can reach C through B
        assert!(is_reachable(&ctx, &a, &c, 3));
        // C cannot reach A (directed graph)
        assert!(!is_reachable(&ctx, &c, &a, 3));
    }

    #[test]
    fn test_graph_density() {
        let (ctx, _, _, _) = create_test_context();
        let density = graph_density(&ctx);

        // 3 nodes, 2 edges, max = 6
        assert!((density - 2.0 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_connected_components() {
        let (ctx, _, _, _) = create_test_context();
        let components = connected_components(&ctx);

        // All nodes are connected
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 3);
    }
}
