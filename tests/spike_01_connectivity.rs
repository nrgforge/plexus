//! Spike Investigation 01: Graph Connectivity
//!
//! Validates that the structural graph is sufficiently connected
//! for information to propagate effectively.
//!
//! ## Hypothesis
//! H1 (Graph Connectivity): Document links create sufficient connectivity
//! for label propagation to converge.
//!
//! ## Go/No-Go Criteria
//! - **GO**: ≥80% of nodes reachable from top 10% PageRank seeds within 3 hops
//! - **PIVOT**: 50-80% reachability → May need synthetic edges or different seed strategy
//! - **NO-GO**: <50% reachability → Network too sparse for propagation approach

mod common;

use common::{build_structure_graph, connected_components, pagerank, reachable_count, TestCorpus};
use std::collections::HashMap;

/// Target reachability percentage for GO criteria
const TARGET_REACHABILITY: f64 = 0.80;

/// Pivot threshold - below this is concerning
const PIVOT_THRESHOLD: f64 = 0.50;

/// Max hops for reachability calculation
const MAX_HOPS: usize = 3;

/// Percentage of nodes to use as seeds (top PageRank)
const SEED_PERCENTAGE: f64 = 0.10;

#[tokio::test]
async fn test_connectivity_pkm_webdev() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load pkm-webdev corpus");

    println!("\n=== Spike 01: Graph Connectivity ===");
    println!("Corpus: pkm-webdev");
    println!("Files: {}", corpus.file_count);

    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    println!("Nodes: {}", graph.node_count());
    println!("Edges: {}", graph.edge_count());

    // Edge type breakdown
    let mut edge_types: HashMap<&str, usize> = HashMap::new();
    for edge in &graph.context.edges {
        *edge_types.entry(&edge.relationship).or_default() += 1;
    }
    println!("Edge types:");
    for (rel, count) in &edge_types {
        println!("  {}: {}", rel, count);
    }

    // Node type breakdown
    let mut node_types: HashMap<&str, usize> = HashMap::new();
    for node in graph.context.nodes.values() {
        *node_types.entry(&node.node_type).or_default() += 1;
    }
    println!("Node types:");
    for (nt, count) in &node_types {
        println!("  {}: {}", nt, count);
    }

    // Check document nodes for source property
    let docs_with_source: usize = graph.context.nodes.values()
        .filter(|n| n.node_type == "document")
        .filter(|n| n.properties.contains_key("source"))
        .count();
    let doc_count = graph.context.nodes.values()
        .filter(|n| n.node_type == "document")
        .count();
    println!("Documents with source: {}/{}", docs_with_source, doc_count);

    // Connected components analysis
    let components = connected_components(&graph.context);
    println!("Connected components: {}", components.len());
    if components.len() > 1 {
        let mut sizes: Vec<_> = components.iter().map(|c| c.len()).collect();
        sizes.sort_by(|a, b| b.cmp(a));
        println!("  Top 5 component sizes: {:?}", &sizes[..sizes.len().min(5)]);
    }

    // Calculate PageRank to find seed nodes
    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);
    println!("PageRank converged in {} iterations", pr.iterations);

    // Select top N% as seeds
    let seed_count = ((graph.node_count() as f64) * SEED_PERCENTAGE).ceil() as usize;
    let seed_count = seed_count.max(1); // At least 1 seed

    let top_nodes = pr.top_k(seed_count);
    let seeds: Vec<_> = top_nodes.iter().map(|(id, _)| id.clone()).collect();

    println!("Seeds (top {}%): {} nodes", SEED_PERCENTAGE * 100.0, seeds.len());

    // Check what types of nodes are seeds
    let mut seed_types: HashMap<&str, usize> = HashMap::new();
    for seed_id in &seeds {
        if let Some(node) = graph.context.nodes.get(seed_id) {
            *seed_types.entry(&node.node_type).or_default() += 1;
        }
    }
    println!("Seed node types: {:?}", seed_types);

    // Calculate reachability from seeds
    let reachable = reachable_count(&graph.context, &seeds, MAX_HOPS);
    let reachability = reachable as f64 / graph.node_count() as f64;

    println!(
        "Reachable within {} hops: {} / {} ({:.1}%)",
        MAX_HOPS,
        reachable,
        graph.node_count(),
        reachability * 100.0
    );

    // Evaluate result
    if reachability >= TARGET_REACHABILITY {
        println!("Result: GO ✓ (≥{:.0}% reachability)", TARGET_REACHABILITY * 100.0);
    } else if reachability >= PIVOT_THRESHOLD {
        println!(
            "Result: PIVOT ⚠ ({:.0}%-{:.0}% range)",
            PIVOT_THRESHOLD * 100.0,
            TARGET_REACHABILITY * 100.0
        );
        println!("  → Consider synthetic edges or different seed strategy");
    } else {
        println!("Result: NO-GO ✗ (<{:.0}% reachability)", PIVOT_THRESHOLD * 100.0);
        println!("  → Network too sparse for propagation approach");
    }

    // The test passes regardless - we're gathering data
    // In production, you might want to assert based on criteria:
    // assert!(reachability >= PIVOT_THRESHOLD, "Reachability too low for propagation");
}

#[tokio::test]
async fn test_connectivity_arch_wiki() {
    let corpus = TestCorpus::load("arch-wiki").expect("Failed to load arch-wiki corpus");

    println!("\n=== Spike 01: Graph Connectivity ===");
    println!("Corpus: arch-wiki");
    println!("Files: {}", corpus.file_count);

    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    println!("Nodes: {}", graph.node_count());
    println!("Edges: {}", graph.edge_count());

    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);
    let seed_count = ((graph.node_count() as f64) * SEED_PERCENTAGE).ceil() as usize;
    let seed_count = seed_count.max(1);

    let top_nodes = pr.top_k(seed_count);
    let seeds: Vec<_> = top_nodes.iter().map(|(id, _)| id.clone()).collect();

    let reachable = reachable_count(&graph.context, &seeds, MAX_HOPS);
    let reachability = reachable as f64 / graph.node_count() as f64;

    println!(
        "Reachability: {:.1}% ({}/{} from {} seeds)",
        reachability * 100.0,
        reachable,
        graph.node_count(),
        seeds.len()
    );

    // Report result
    if reachability >= TARGET_REACHABILITY {
        println!("Result: GO ✓");
    } else if reachability >= PIVOT_THRESHOLD {
        println!("Result: PIVOT ⚠");
    } else {
        println!("Result: NO-GO ✗");
    }
}

#[tokio::test]
async fn test_connectivity_all_corpora() {
    let corpora = ["pkm-webdev", "arch-wiki", "pkm-datascience", "shakespeare"];

    println!("\n=== Spike 01: Connectivity Summary ===");
    println!("{:<20} {:>8} {:>8} {:>8} {:>12}", "Corpus", "Nodes", "Edges", "Seeds", "Reachable");
    println!("{}", "-".repeat(60));

    for corpus_name in corpora {
        let corpus = match TestCorpus::load(corpus_name) {
            Ok(c) => c,
            Err(_) => {
                println!("{:<20} (not available)", corpus_name);
                continue;
            }
        };

        let graph = match build_structure_graph(&corpus).await {
            Ok(g) => g,
            Err(_) => {
                println!("{:<20} (build failed)", corpus_name);
                continue;
            }
        };

        if graph.node_count() == 0 {
            println!("{:<20} {:>8} {:>8} {:>8} {:>12}", corpus_name, 0, 0, 0, "N/A");
            continue;
        }

        let pr = pagerank(&graph.context, 0.85, 100, 1e-6);
        let seed_count = ((graph.node_count() as f64) * SEED_PERCENTAGE).ceil() as usize;
        let seed_count = seed_count.max(1);

        let top_nodes = pr.top_k(seed_count);
        let seeds: Vec<_> = top_nodes.iter().map(|(id, _)| id.clone()).collect();

        let reachable = reachable_count(&graph.context, &seeds, MAX_HOPS);
        let reachability = reachable as f64 / graph.node_count() as f64;

        let status = if reachability >= TARGET_REACHABILITY {
            "GO ✓"
        } else if reachability >= PIVOT_THRESHOLD {
            "PIVOT ⚠"
        } else {
            "NO-GO ✗"
        };

        println!(
            "{:<20} {:>8} {:>8} {:>8} {:>8.1}% {}",
            corpus_name,
            graph.node_count(),
            graph.edge_count(),
            seeds.len(),
            reachability * 100.0,
            status
        );
    }
}
