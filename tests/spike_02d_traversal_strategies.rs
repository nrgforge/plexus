//! Spike Investigation 02d: Graph Traversal Strategies
//!
//! Compares different approaches to traversing the graph for LLM analysis:
//! - PageRank-based seed selection
//! - Random seed selection
//! - Random walks with restart
//! - BFS from random seeds
//! - Stratified random (by directory)
//!
//! Key metrics:
//! - Coverage rate: How many nodes reached after N steps?
//! - Diversity: Are we visiting different parts of the graph?
//! - Efficiency: Unique nodes / total steps

mod common;

use common::{build_structure_graph, pagerank, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use rand::prelude::*;
use rand::SeedableRng;
use std::collections::{HashMap, HashSet, VecDeque};

/// Simulate a traversal strategy and measure coverage over time
struct TraversalSimulation {
    /// Nodes visited in order
    visit_order: Vec<NodeId>,
    /// Coverage at each step (cumulative unique nodes / total nodes)
    coverage_curve: Vec<f64>,
    /// Total nodes in graph
    total_nodes: usize,
}

impl TraversalSimulation {
    fn new(total_nodes: usize) -> Self {
        Self {
            visit_order: Vec::new(),
            coverage_curve: Vec::new(),
            total_nodes,
        }
    }

    fn record_visit(&mut self, node: NodeId) {
        self.visit_order.push(node);
        let unique: HashSet<_> = self.visit_order.iter().collect();
        self.coverage_curve.push(unique.len() as f64 / self.total_nodes as f64);
    }

    fn final_coverage(&self) -> f64 {
        self.coverage_curve.last().copied().unwrap_or(0.0)
    }

    fn unique_visited(&self) -> usize {
        self.visit_order.iter().collect::<HashSet<_>>().len()
    }

    fn efficiency(&self) -> f64 {
        if self.visit_order.is_empty() {
            0.0
        } else {
            self.unique_visited() as f64 / self.visit_order.len() as f64
        }
    }

    fn steps_to_coverage(&self, target: f64) -> Option<usize> {
        self.coverage_curve.iter()
            .position(|&c| c >= target)
            .map(|i| i + 1)
    }
}

/// Build adjacency list for traversal (all edges, bidirectional consideration)
fn build_adjacency(context: &Context) -> HashMap<NodeId, Vec<NodeId>> {
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    // Initialize all nodes
    for node_id in context.nodes.keys() {
        adj.insert(node_id.clone(), Vec::new());
    }

    // Add edges (following edge direction)
    for edge in &context.edges {
        if context.nodes.contains_key(&edge.source) && context.nodes.contains_key(&edge.target) {
            adj.get_mut(&edge.source).unwrap().push(edge.target.clone());
        }
    }

    adj
}

/// Strategy 1: PageRank-based BFS
fn traverse_pagerank_bfs(
    context: &Context,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    num_seeds: usize,
    max_steps: usize,
) -> TraversalSimulation {
    let pr = pagerank(context, 0.85, 100, 1e-6);

    // Get top-k by PageRank
    let mut scores: Vec<_> = pr.scores.iter().collect();
    scores.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());

    let seeds: Vec<_> = scores.iter()
        .take(num_seeds)
        .map(|(id, _)| (*id).clone())
        .collect();

    bfs_from_seeds(context, adj, &seeds, max_steps)
}

/// Strategy 2: Random seeds with BFS
fn traverse_random_bfs(
    context: &Context,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    num_seeds: usize,
    max_steps: usize,
    rng: &mut impl Rng,
) -> TraversalSimulation {
    let node_ids: Vec<_> = context.nodes.keys().cloned().collect();
    let seeds: Vec<_> = node_ids.choose_multiple(rng, num_seeds).cloned().collect();

    bfs_from_seeds(context, adj, &seeds, max_steps)
}

/// BFS from given seeds
fn bfs_from_seeds(
    context: &Context,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    seeds: &[NodeId],
    max_steps: usize,
) -> TraversalSimulation {
    let mut sim = TraversalSimulation::new(context.nodes.len());
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut queue: VecDeque<NodeId> = VecDeque::new();

    // Add seeds to queue
    for seed in seeds {
        if !visited.contains(seed) {
            queue.push_back(seed.clone());
            visited.insert(seed.clone());
        }
    }

    let mut steps = 0;
    while let Some(node) = queue.pop_front() {
        if steps >= max_steps {
            break;
        }

        sim.record_visit(node.clone());
        steps += 1;

        // Add unvisited neighbors
        if let Some(neighbors) = adj.get(&node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor.clone());
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    sim
}

/// Strategy 3: Random walk with restart
fn traverse_random_walk(
    context: &Context,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    num_walkers: usize,
    steps_per_walker: usize,
    restart_prob: f64,
    rng: &mut impl Rng,
) -> TraversalSimulation {
    let mut sim = TraversalSimulation::new(context.nodes.len());
    let node_ids: Vec<_> = context.nodes.keys().cloned().collect();

    for _ in 0..num_walkers {
        // Pick random starting node
        let start = node_ids.choose(rng).unwrap().clone();
        let mut current = start.clone();

        for _ in 0..steps_per_walker {
            sim.record_visit(current.clone());

            // Restart with probability restart_prob
            if rng.gen::<f64>() < restart_prob {
                current = node_ids.choose(rng).unwrap().clone();
                continue;
            }

            // Otherwise, walk to random neighbor
            if let Some(neighbors) = adj.get(&current) {
                if !neighbors.is_empty() {
                    current = neighbors.choose(rng).unwrap().clone();
                } else {
                    // Dead end - restart
                    current = node_ids.choose(rng).unwrap().clone();
                }
            }
        }
    }

    sim
}

/// Strategy 4: Stratified random by directory
fn traverse_stratified_bfs(
    context: &Context,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    seeds_per_stratum: usize,
    max_steps: usize,
    rng: &mut impl Rng,
) -> TraversalSimulation {
    // Group document nodes by directory
    let mut by_directory: HashMap<String, Vec<NodeId>> = HashMap::new();

    for (node_id, node) in &context.nodes {
        if node.node_type == "document" {
            let source = node.properties.get("source")
                .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
                .unwrap_or("");

            let dir = source.rsplit_once('/')
                .map(|(d, _)| d.to_string())
                .unwrap_or_else(|| "root".to_string());

            by_directory.entry(dir).or_default().push(node_id.clone());
        }
    }

    // Sample from each directory
    let mut seeds: Vec<NodeId> = Vec::new();
    for (_dir, nodes) in &by_directory {
        let sample: Vec<_> = nodes.choose_multiple(rng, seeds_per_stratum.min(nodes.len()))
            .cloned()
            .collect();
        seeds.extend(sample);
    }

    bfs_from_seeds(context, adj, &seeds, max_steps)
}

/// Strategy 5: Greedy coverage (pick nodes that maximize new coverage)
fn traverse_greedy_coverage(
    context: &Context,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    max_steps: usize,
) -> TraversalSimulation {
    let mut sim = TraversalSimulation::new(context.nodes.len());
    let mut covered: HashSet<NodeId> = HashSet::new();

    for _ in 0..max_steps {
        // Find node that would cover the most new nodes (itself + uncovered neighbors)
        let mut best_node: Option<NodeId> = None;
        let mut best_gain = 0usize;

        for node_id in context.nodes.keys() {
            if covered.contains(node_id) {
                continue;
            }

            let mut gain = 1; // The node itself
            if let Some(neighbors) = adj.get(node_id) {
                for neighbor in neighbors {
                    if !covered.contains(neighbor) {
                        gain += 1;
                    }
                }
            }

            if gain > best_gain {
                best_gain = gain;
                best_node = Some(node_id.clone());
            }
        }

        if let Some(node) = best_node {
            sim.record_visit(node.clone());
            covered.insert(node.clone());

            // Also mark neighbors as "reachable" but not visited
            if let Some(neighbors) = adj.get(&node) {
                for neighbor in neighbors {
                    covered.insert(neighbor.clone());
                }
            }
        } else {
            break; // All covered
        }
    }

    sim
}

#[tokio::test]
#[ignore]
async fn test_traversal_strategies() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");

    println!("\n{}", "=".repeat(80));
    println!("=== Spike 02d: Traversal Strategy Comparison ===");
    println!("{}\n", "=".repeat(80));

    let adj = build_adjacency(&graph.context);
    let total_nodes = graph.context.nodes.len();
    let max_steps = total_nodes; // Allow visiting all nodes

    println!("Graph: {} nodes, {} edges\n", total_nodes, graph.context.edges.len());

    // Use fixed seed for reproducibility
    let mut rng = StdRng::seed_from_u64(42);

    // Run each strategy
    let strategies: Vec<(&str, TraversalSimulation)> = vec![
        ("PageRank BFS (5 seeds)", traverse_pagerank_bfs(&graph.context, &adj, 5, max_steps)),
        ("PageRank BFS (10 seeds)", traverse_pagerank_bfs(&graph.context, &adj, 10, max_steps)),
        ("Random BFS (5 seeds)", traverse_random_bfs(&graph.context, &adj, 5, max_steps, &mut rng)),
        ("Random BFS (10 seeds)", traverse_random_bfs(&graph.context, &adj, 10, max_steps, &mut rng)),
        ("Random Walk (5×100, p=0.1)", traverse_random_walk(&graph.context, &adj, 5, 100, 0.1, &mut rng)),
        ("Random Walk (10×50, p=0.15)", traverse_random_walk(&graph.context, &adj, 10, 50, 0.15, &mut rng)),
        ("Stratified BFS (1/dir)", traverse_stratified_bfs(&graph.context, &adj, 1, max_steps, &mut rng)),
        ("Stratified BFS (2/dir)", traverse_stratified_bfs(&graph.context, &adj, 2, max_steps, &mut rng)),
        ("Greedy Coverage", traverse_greedy_coverage(&graph.context, &adj, max_steps)),
    ];

    // Summary table
    println!("{:<30} {:>10} {:>10} {:>12} {:>12} {:>12}",
        "Strategy", "Steps", "Unique", "Efficiency", "Coverage", "Steps@80%");
    println!("{}", "-".repeat(90));

    for (name, sim) in &strategies {
        let steps_80 = sim.steps_to_coverage(0.80)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        println!("{:<30} {:>10} {:>10} {:>12.1}% {:>12.1}% {:>12}",
            name,
            sim.visit_order.len(),
            sim.unique_visited(),
            sim.efficiency() * 100.0,
            sim.final_coverage() * 100.0,
            steps_80);
    }

    // Coverage curves at key points
    println!("\n=== Coverage Progression ===\n");
    println!("{:<30} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Strategy", "@10 steps", "@25 steps", "@50 steps", "@100 steps", "@200 steps");
    println!("{}", "-".repeat(85));

    for (name, sim) in &strategies {
        let at_10 = sim.coverage_curve.get(9).copied().unwrap_or(0.0);
        let at_25 = sim.coverage_curve.get(24).copied().unwrap_or(0.0);
        let at_50 = sim.coverage_curve.get(49).copied().unwrap_or(0.0);
        let at_100 = sim.coverage_curve.get(99).copied().unwrap_or(0.0);
        let at_200 = sim.coverage_curve.get(199).copied().unwrap_or(0.0);

        println!("{:<30} {:>10.1}% {:>10.1}% {:>10.1}% {:>10.1}% {:>10.1}%",
            name,
            at_10 * 100.0,
            at_25 * 100.0,
            at_50 * 100.0,
            at_100 * 100.0,
            at_200 * 100.0);
    }

    // Node type coverage analysis
    println!("\n=== Node Type Coverage (first 50 visits) ===\n");

    for (name, sim) in strategies.iter().take(4) {
        let first_50: Vec<_> = sim.visit_order.iter().take(50).collect();
        let mut type_counts: HashMap<&str, usize> = HashMap::new();

        for node_id in &first_50 {
            if let Some(node) = graph.context.nodes.get(*node_id) {
                *type_counts.entry(&node.node_type).or_default() += 1;
            }
        }

        println!("{}: {:?}", name, type_counts);
    }

    // Document-only analysis
    println!("\n=== Document Coverage ===\n");

    let doc_ids: HashSet<_> = graph.context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id)
        .collect();

    let total_docs = doc_ids.len();

    println!("{:<30} {:>15} {:>15}",
        "Strategy", "Docs Visited", "Doc Coverage");
    println!("{}", "-".repeat(65));

    for (name, sim) in &strategies {
        let docs_visited: HashSet<_> = sim.visit_order.iter()
            .filter(|id| doc_ids.contains(id))
            .collect();

        println!("{:<30} {:>15} {:>15.1}%",
            name,
            docs_visited.len(),
            (docs_visited.len() as f64 / total_docs as f64) * 100.0);
    }

    println!("\n=== Key Insights ===\n");
    println!("• Efficiency = unique nodes / total steps (higher = less redundant visiting)");
    println!("• Steps@80% = how many steps to reach 80% coverage (lower = faster)");
    println!("• Random walks trade efficiency for exploration diversity");
    println!("• BFS is systematic but may cluster in one region");
    println!("• Stratified sampling guarantees directory diversity");
}

#[tokio::test]
#[ignore]
async fn test_random_walk_variants() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");

    println!("\n{}", "=".repeat(80));
    println!("=== Random Walk Parameter Exploration ===");
    println!("{}\n", "=".repeat(80));

    let adj = build_adjacency(&graph.context);
    let total_nodes = graph.context.nodes.len();

    // Test different restart probabilities
    println!("Varying restart probability (10 walkers × 50 steps each):\n");
    println!("{:<20} {:>12} {:>12} {:>12}",
        "Restart Prob", "Unique", "Coverage", "Efficiency");
    println!("{}", "-".repeat(60));

    for restart_prob in [0.0, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5] {
        let mut rng = StdRng::seed_from_u64(42);
        let sim = traverse_random_walk(&graph.context, &adj, 10, 50, restart_prob, &mut rng);

        println!("{:<20} {:>12} {:>12.1}% {:>12.1}%",
            format!("p = {:.2}", restart_prob),
            sim.unique_visited(),
            sim.final_coverage() * 100.0,
            sim.efficiency() * 100.0);
    }

    // Test different walker configurations (same total budget)
    println!("\nVarying walker configuration (500 total steps):\n");
    println!("{:<25} {:>12} {:>12} {:>12}",
        "Config", "Unique", "Coverage", "Efficiency");
    println!("{}", "-".repeat(55));

    let configs = [
        (1, 500, "1 walker × 500 steps"),
        (5, 100, "5 walkers × 100 steps"),
        (10, 50, "10 walkers × 50 steps"),
        (25, 20, "25 walkers × 20 steps"),
        (50, 10, "50 walkers × 10 steps"),
        (100, 5, "100 walkers × 5 steps"),
    ];

    for (walkers, steps, label) in configs {
        let mut rng = StdRng::seed_from_u64(42);
        let sim = traverse_random_walk(&graph.context, &adj, walkers, steps, 0.1, &mut rng);

        println!("{:<25} {:>12} {:>12.1}% {:>12.1}%",
            label,
            sim.unique_visited(),
            sim.final_coverage() * 100.0,
            sim.efficiency() * 100.0);
    }
}
