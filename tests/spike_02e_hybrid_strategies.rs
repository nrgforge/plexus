//! Spike Investigation 02e: Hybrid Traversal Strategies
//!
//! Tests whether combining strategies offers advantages over pure stratified sampling:
//! - Stratified + PageRank weighting (pick highest PR doc per directory)
//! - Stratified + Content filter (pick docs with >100 words per directory)
//! - Stratified + Betweenness (pick highest betweenness per directory)
//! - PageRank seeds + Random walk expansion
//! - Adaptive: start stratified, then use graph structure to fill gaps
//!
//! Key question: Does hybridization improve over pure stratified sampling?

mod common;

use common::{build_structure_graph, pagerank, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use rand::prelude::*;
use rand::SeedableRng;
use std::collections::{HashMap, HashSet, VecDeque};

/// Document metadata for ranking within strata
#[derive(Clone)]
struct DocInfo {
    id: NodeId,
    source: String,
    directory: String,
    word_count: usize,
    pagerank: f64,
    betweenness: f64,
}

/// Build adjacency list
fn build_adjacency(context: &Context) -> HashMap<NodeId, Vec<NodeId>> {
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for node_id in context.nodes.keys() {
        adj.insert(node_id.clone(), Vec::new());
    }
    for edge in &context.edges {
        if context.nodes.contains_key(&edge.source) && context.nodes.contains_key(&edge.target) {
            adj.get_mut(&edge.source).unwrap().push(edge.target.clone());
        }
    }
    adj
}

/// Calculate betweenness centrality (simplified)
fn betweenness_centrality(context: &Context) -> HashMap<NodeId, f64> {
    let mut centrality: HashMap<NodeId, f64> = HashMap::new();
    for node_id in context.nodes.keys() {
        centrality.insert(node_id.clone(), 0.0);
    }

    let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    for edge in &context.edges {
        if context.nodes.contains_key(&edge.source) && context.nodes.contains_key(&edge.target) {
            adj.entry(&edge.source).or_default().push(&edge.target);
        }
    }

    let node_ids: Vec<_> = context.nodes.keys().collect();

    for s in &node_ids {
        let mut stack: Vec<&NodeId> = Vec::new();
        let mut pred: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
        let mut sigma: HashMap<&NodeId, f64> = HashMap::new();
        let mut dist: HashMap<&NodeId, i32> = HashMap::new();

        for v in &node_ids {
            pred.insert(*v, Vec::new());
            sigma.insert(*v, 0.0);
            dist.insert(*v, -1);
        }

        sigma.insert(*s, 1.0);
        dist.insert(*s, 0);

        let mut queue: VecDeque<&NodeId> = VecDeque::new();
        queue.push_back(*s);

        while let Some(v) = queue.pop_front() {
            stack.push(v);
            let v_dist = *dist.get(v).unwrap();

            if let Some(neighbors) = adj.get(v) {
                for w in neighbors {
                    if *dist.get(*w).unwrap() < 0 {
                        queue.push_back(*w);
                        dist.insert(*w, v_dist + 1);
                    }
                    if *dist.get(*w).unwrap() == v_dist + 1 {
                        let sigma_v = *sigma.get(v).unwrap();
                        *sigma.get_mut(*w).unwrap() += sigma_v;
                        pred.get_mut(*w).unwrap().push(v);
                    }
                }
            }
        }

        let mut delta: HashMap<&NodeId, f64> = HashMap::new();
        for v in &node_ids {
            delta.insert(*v, 0.0);
        }

        while let Some(w) = stack.pop() {
            let sigma_w = *sigma.get(w).unwrap();
            let delta_w = *delta.get(w).unwrap();

            for v in pred.get(w).unwrap() {
                let sigma_v = *sigma.get(*v).unwrap();
                let contribution = (sigma_v / sigma_w) * (1.0 + delta_w);
                *delta.get_mut(*v).unwrap() += contribution;
            }

            if *w != **s {
                *centrality.get_mut(&(*w).clone()).unwrap() += delta_w;
            }
        }
    }

    let n = node_ids.len() as f64;
    if n > 2.0 {
        let norm = 1.0 / ((n - 1.0) * (n - 2.0));
        for val in centrality.values_mut() {
            *val *= norm;
        }
    }

    centrality
}

/// Extract mock concepts from content (for quality scoring)
fn extract_concepts(content: &str) -> Vec<String> {
    let mut concepts = Vec::new();

    // Headings
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim();
            if !heading.is_empty() && heading.len() > 2 {
                concepts.push(heading.to_string());
            }
        }
    }

    // Code blocks
    for line in content.lines() {
        if line.trim().starts_with("```") {
            let lang = line.trim().trim_start_matches('`').trim();
            if !lang.is_empty() && lang.len() < 20 {
                concepts.push(format!("code:{}", lang));
            }
        }
    }

    concepts
}

/// Calculate semantic quality score for a document
fn quality_score(content: &str) -> f64 {
    let word_count = content.split_whitespace().count();
    let concepts = extract_concepts(content);

    let heading_count = concepts.iter()
        .filter(|c| !c.starts_with("code:"))
        .count();
    let code_count = concepts.iter()
        .filter(|c| c.starts_with("code:"))
        .count();

    heading_count as f64 * 2.0 +
    code_count as f64 * 3.0 +
    (word_count as f64 / 50.0).min(5.0)
}

/// BFS from seeds, return visited nodes in order
fn bfs_traverse(
    adj: &HashMap<NodeId, Vec<NodeId>>,
    seeds: &[NodeId],
    max_steps: usize,
) -> Vec<NodeId> {
    let mut visited_order = Vec::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut queue: VecDeque<NodeId> = VecDeque::new();

    for seed in seeds {
        if !visited.contains(seed) {
            queue.push_back(seed.clone());
            visited.insert(seed.clone());
        }
    }

    while let Some(node) = queue.pop_front() {
        if visited_order.len() >= max_steps {
            break;
        }

        visited_order.push(node.clone());

        if let Some(neighbors) = adj.get(&node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor.clone());
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    visited_order
}

#[tokio::test]
async fn test_hybrid_strategies() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");

    println!("\n{}", "=".repeat(80));
    println!("=== Spike 02e: Hybrid Strategy Comparison ===");
    println!("{}\n", "=".repeat(80));

    let adj = build_adjacency(&graph.context);
    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);
    let bc = betweenness_centrality(&graph.context);

    // Build document info
    let mut docs: Vec<DocInfo> = Vec::new();

    for (node_id, node) in &graph.context.nodes {
        if node.node_type != "document" {
            continue;
        }

        let source = node.properties.get("source")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_default();

        let directory = source.rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_else(|| "root".to_string());

        let word_count = corpus.items.iter()
            .find(|item| item.id.as_str() == source)
            .map(|item| item.content.split_whitespace().count())
            .unwrap_or(0);

        let pagerank = *pr.scores.get(node_id).unwrap_or(&0.0);
        let betweenness = *bc.get(node_id).unwrap_or(&0.0);

        docs.push(DocInfo {
            id: node_id.clone(),
            source,
            directory,
            word_count,
            pagerank,
            betweenness,
        });
    }

    // Group by directory
    let mut by_dir: HashMap<String, Vec<DocInfo>> = HashMap::new();
    for doc in &docs {
        by_dir.entry(doc.directory.clone()).or_default().push(doc.clone());
    }

    let num_dirs = by_dir.len();
    println!("Documents: {}, Directories: {}\n", docs.len(), num_dirs);

    // === Strategy 1: Pure Stratified (random per dir) ===
    let mut rng = StdRng::seed_from_u64(42);
    let seeds_random: Vec<NodeId> = by_dir.values()
        .filter_map(|dir_docs| dir_docs.choose(&mut rng).map(|d| d.id.clone()))
        .collect();

    // === Strategy 2: Stratified + PageRank (highest PR per dir) ===
    let seeds_pr: Vec<NodeId> = by_dir.values()
        .filter_map(|dir_docs| {
            dir_docs.iter()
                .max_by(|a, b| a.pagerank.partial_cmp(&b.pagerank).unwrap())
                .map(|d| d.id.clone())
        })
        .collect();

    // === Strategy 3: Stratified + Content (highest word count per dir, min 100) ===
    let seeds_content: Vec<NodeId> = by_dir.values()
        .filter_map(|dir_docs| {
            let filtered: Vec<_> = dir_docs.iter().filter(|d| d.word_count > 100).collect();
            if filtered.is_empty() {
                // Fall back to highest word count
                dir_docs.iter().max_by_key(|d| d.word_count).map(|d| d.id.clone())
            } else {
                filtered.iter().max_by_key(|d| d.word_count).map(|d| d.id.clone())
            }
        })
        .collect();

    // === Strategy 4: Stratified + Betweenness (highest BC per dir) ===
    let seeds_bc: Vec<NodeId> = by_dir.values()
        .filter_map(|dir_docs| {
            dir_docs.iter()
                .max_by(|a, b| a.betweenness.partial_cmp(&b.betweenness).unwrap())
                .map(|d| d.id.clone())
        })
        .collect();

    // === Strategy 5: Stratified + Composite (PR × word_count) ===
    let seeds_composite: Vec<NodeId> = by_dir.values()
        .filter_map(|dir_docs| {
            dir_docs.iter()
                .max_by(|a, b| {
                    let score_a = a.pagerank * (a.word_count as f64).ln().max(1.0);
                    let score_b = b.pagerank * (b.word_count as f64).ln().max(1.0);
                    score_a.partial_cmp(&score_b).unwrap()
                })
                .map(|d| d.id.clone())
        })
        .collect();

    // Evaluate each strategy
    let strategies: Vec<(&str, Vec<NodeId>)> = vec![
        ("Pure Stratified (random)", seeds_random),
        ("Stratified + PageRank", seeds_pr),
        ("Stratified + Content (>100w)", seeds_content),
        ("Stratified + Betweenness", seeds_bc),
        ("Stratified + Composite", seeds_composite),
    ];

    // Calculate quality metrics for each strategy
    println!("{:<35} {:>8} {:>12} {:>12} {:>12}",
        "Strategy", "Seeds", "Avg Words", "Avg Quality", "Total Quality");
    println!("{}", "-".repeat(85));

    for (name, seeds) in &strategies {
        let mut total_words = 0usize;
        let mut total_quality = 0.0f64;

        for seed_id in seeds {
            if let Some(doc) = docs.iter().find(|d| &d.id == seed_id) {
                total_words += doc.word_count;

                // Get content for quality score
                if let Some(item) = corpus.items.iter().find(|i| i.id.as_str() == doc.source) {
                    total_quality += quality_score(&item.content);
                }
            }
        }

        let avg_words = total_words as f64 / seeds.len() as f64;
        let avg_quality = total_quality / seeds.len() as f64;

        println!("{:<35} {:>8} {:>12.1} {:>12.1} {:>12.1}",
            name, seeds.len(), avg_words, avg_quality, total_quality);
    }

    // Coverage comparison
    println!("\n=== Coverage After BFS Expansion ===\n");

    let max_steps = graph.context.nodes.len();
    let doc_ids: HashSet<_> = docs.iter().map(|d| &d.id).collect();

    println!("{:<35} {:>12} {:>12} {:>15}",
        "Strategy", "Nodes Reached", "Docs Reached", "Doc Coverage");
    println!("{}", "-".repeat(80));

    for (name, seeds) in &strategies {
        let visited = bfs_traverse(&adj, seeds, max_steps);
        let docs_visited: HashSet<_> = visited.iter().filter(|id| doc_ids.contains(id)).collect();

        println!("{:<35} {:>12} {:>12} {:>15.1}%",
            name,
            visited.len(),
            docs_visited.len(),
            (docs_visited.len() as f64 / docs.len() as f64) * 100.0);
    }

    // Detailed seed comparison
    println!("\n=== Seed Document Details ===\n");

    for (name, seeds) in strategies.iter().take(3) {
        println!("--- {} ---", name);
        let mut seed_details: Vec<_> = seeds.iter()
            .filter_map(|id| docs.iter().find(|d| &d.id == id))
            .collect();
        seed_details.sort_by(|a, b| b.word_count.cmp(&a.word_count));

        for doc in seed_details.iter().take(8) {
            let content = corpus.items.iter()
                .find(|i| i.id.as_str() == doc.source)
                .map(|i| i.content.as_str())
                .unwrap_or("");
            let q = quality_score(content);
            println!("  {} ({} words, quality {:.1})",
                truncate(&doc.source, 40), doc.word_count, q);
        }
        println!();
    }

    // Concept extraction comparison
    println!("=== Unique Concepts Extracted ===\n");

    for (name, seeds) in &strategies {
        let mut all_concepts: HashSet<String> = HashSet::new();

        for seed_id in seeds {
            if let Some(doc) = docs.iter().find(|d| &d.id == seed_id) {
                if let Some(item) = corpus.items.iter().find(|i| i.id.as_str() == doc.source) {
                    for concept in extract_concepts(&item.content) {
                        all_concepts.insert(concept);
                    }
                }
            }
        }

        println!("{:<35} {:>5} concepts", name, all_concepts.len());
    }

    println!("\n=== Key Insights ===\n");
    println!("• All stratified approaches achieve same coverage (by design)");
    println!("• Hybrid weighting affects QUALITY of seeds, not coverage");
    println!("• Content filter ensures we hit document-rich nodes");
    println!("• PageRank weighting may select index pages within directories");
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - (max_len - 3)..])
    }
}

#[tokio::test]
async fn test_prioritized_traversal() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");

    println!("\n{}", "=".repeat(80));
    println!("=== Prioritized Traversal: Quality vs Coverage Trade-off ===");
    println!("{}\n", "=".repeat(80));

    // The question: If we have limited LLM budget, which documents should we analyze first?
    // We want to maximize semantic value early in the traversal.

    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);

    let mut docs: Vec<(NodeId, String, usize, f64)> = Vec::new();

    for (node_id, node) in &graph.context.nodes {
        if node.node_type != "document" {
            continue;
        }

        let source = node.properties.get("source")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_default();

        let word_count = corpus.items.iter()
            .find(|item| item.id.as_str() == source)
            .map(|item| item.content.split_whitespace().count())
            .unwrap_or(0);

        let content = corpus.items.iter()
            .find(|item| item.id.as_str() == source)
            .map(|item| item.content.as_str())
            .unwrap_or("");

        let quality = quality_score(content);

        docs.push((node_id.clone(), source, word_count, quality));
    }

    // Sort by different criteria and compare cumulative quality
    println!("Cumulative quality at N documents analyzed:\n");
    println!("{:<25} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Ordering", "N=5", "N=10", "N=15", "N=20", "N=all");
    println!("{}", "-".repeat(75));

    // Random order
    let mut rng = StdRng::seed_from_u64(42);
    let mut random_order = docs.clone();
    random_order.shuffle(&mut rng);
    print_cumulative_quality("Random", &random_order);

    // PageRank order
    let mut pr_order = docs.clone();
    pr_order.sort_by(|a, b| {
        let pr_a = pr.scores.get(&a.0).unwrap_or(&0.0);
        let pr_b = pr.scores.get(&b.0).unwrap_or(&0.0);
        pr_b.partial_cmp(pr_a).unwrap()
    });
    print_cumulative_quality("By PageRank", &pr_order);

    // Word count order
    let mut wc_order = docs.clone();
    wc_order.sort_by(|a, b| b.2.cmp(&a.2));
    print_cumulative_quality("By Word Count", &wc_order);

    // Quality order (oracle - best possible)
    let mut quality_order = docs.clone();
    quality_order.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
    print_cumulative_quality("By Quality (oracle)", &quality_order);

    // PR × log(word_count) - practical hybrid
    let mut hybrid_order = docs.clone();
    hybrid_order.sort_by(|a, b| {
        let pr_a = pr.scores.get(&a.0).unwrap_or(&0.0);
        let pr_b = pr.scores.get(&b.0).unwrap_or(&0.0);
        let score_a = pr_a * (a.2 as f64 + 1.0).ln();
        let score_b = pr_b * (b.2 as f64 + 1.0).ln();
        score_b.partial_cmp(&score_a).unwrap()
    });
    print_cumulative_quality("PR × ln(words)", &hybrid_order);

    // Word count filtered, then by PageRank
    let mut filtered_pr = docs.clone();
    filtered_pr.sort_by(|a, b| {
        // First: docs with >100 words come first
        let a_filtered = a.2 > 100;
        let b_filtered = b.2 > 100;
        match (a_filtered, b_filtered) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // Then sort by PageRank
                let pr_a = pr.scores.get(&a.0).unwrap_or(&0.0);
                let pr_b = pr.scores.get(&b.0).unwrap_or(&0.0);
                pr_b.partial_cmp(pr_a).unwrap()
            }
        }
    });
    print_cumulative_quality("Filtered(>100) + PR", &filtered_pr);

    println!("\n• Oracle shows best achievable with perfect foreknowledge");
    println!("• Practical strategies should approach oracle curve");
    println!("• Higher early cumulative quality = better prioritization");
}

fn print_cumulative_quality(name: &str, docs: &[(NodeId, String, usize, f64)]) {
    let at_5: f64 = docs.iter().take(5).map(|d| d.3).sum();
    let at_10: f64 = docs.iter().take(10).map(|d| d.3).sum();
    let at_15: f64 = docs.iter().take(15).map(|d| d.3).sum();
    let at_20: f64 = docs.iter().take(20).map(|d| d.3).sum();
    let at_all: f64 = docs.iter().map(|d| d.3).sum();

    println!("{:<25} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
        name, at_5, at_10, at_15, at_20, at_all);
}
