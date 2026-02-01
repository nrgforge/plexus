//! Spike Investigation 02b: Alternative Seed Selection Strategies
//!
//! Compares different approaches to identifying semantically-rich documents:
//! - PageRank (baseline)
//! - PageRank with word count filter
//! - Betweenness Centrality
//! - HITS on original directed edges only (G')
//! - Composite scoring
//!
//! Goal: Find the strategy that best identifies content-rich documents for seeding.

mod common;

use common::{build_structure_graph, pagerank, hits, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use std::collections::{HashMap, HashSet, VecDeque};

const TOP_K: usize = 8;

/// Calculate betweenness centrality for all nodes
/// Uses Brandes' algorithm (simplified for unweighted graphs)
fn betweenness_centrality(context: &Context) -> HashMap<NodeId, f64> {
    let mut centrality: HashMap<NodeId, f64> = HashMap::new();

    // Initialize all nodes with 0
    for node_id in context.nodes.keys() {
        centrality.insert(node_id.clone(), 0.0);
    }

    // Build adjacency list
    let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    for edge in &context.edges {
        if context.nodes.contains_key(&edge.source) && context.nodes.contains_key(&edge.target) {
            adj.entry(&edge.source).or_default().push(&edge.target);
        }
    }

    let node_ids: Vec<_> = context.nodes.keys().collect();

    // Brandes' algorithm
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
                    // First visit?
                    if *dist.get(*w).unwrap() < 0 {
                        queue.push_back(*w);
                        dist.insert(*w, v_dist + 1);
                    }
                    // Shortest path to w via v?
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

    // Normalize
    let n = node_ids.len() as f64;
    if n > 2.0 {
        let norm = 1.0 / ((n - 1.0) * (n - 2.0));
        for val in centrality.values_mut() {
            *val *= norm;
        }
    }

    centrality
}

/// Run HITS on only original directed edges (excluding synthetic reverse edges)
fn hits_directed_only(context: &Context, iterations: usize) -> common::HitsResult {
    // Create a filtered context with only original edges
    let directed_edges: Vec<_> = context.edges.iter()
        .filter(|e| {
            // Keep only original link edges, not reverse edges
            e.relationship == "links_to"
            || e.relationship == "references"
            || e.relationship == "contains"
            || e.relationship == "follows"
        })
        .collect();

    // Build adjacency for HITS
    let mut hub_scores: HashMap<NodeId, f64> = HashMap::new();
    let mut auth_scores: HashMap<NodeId, f64> = HashMap::new();

    // Initialize
    for node_id in context.nodes.keys() {
        hub_scores.insert(node_id.clone(), 1.0);
        auth_scores.insert(node_id.clone(), 1.0);
    }

    // Build outgoing and incoming adjacency from directed edges only
    let mut outgoing: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    let mut incoming: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();

    for edge in &directed_edges {
        if context.nodes.contains_key(&edge.source) && context.nodes.contains_key(&edge.target) {
            outgoing.entry(&edge.source).or_default().push(&edge.target);
            incoming.entry(&edge.target).or_default().push(&edge.source);
        }
    }

    // HITS iterations
    for _ in 0..iterations {
        // Update authority scores
        let mut new_auth: HashMap<NodeId, f64> = HashMap::new();
        for node_id in context.nodes.keys() {
            let score: f64 = incoming.get(node_id)
                .map(|sources| sources.iter().map(|s| hub_scores.get(*s).unwrap_or(&0.0)).sum())
                .unwrap_or(0.0);
            new_auth.insert(node_id.clone(), score);
        }

        // Normalize authority
        let auth_norm: f64 = new_auth.values().map(|x| x * x).sum::<f64>().sqrt();
        if auth_norm > 0.0 {
            for val in new_auth.values_mut() {
                *val /= auth_norm;
            }
        }
        auth_scores = new_auth;

        // Update hub scores
        let mut new_hub: HashMap<NodeId, f64> = HashMap::new();
        for node_id in context.nodes.keys() {
            let score: f64 = outgoing.get(node_id)
                .map(|targets| targets.iter().map(|t| auth_scores.get(*t).unwrap_or(&0.0)).sum())
                .unwrap_or(0.0);
            new_hub.insert(node_id.clone(), score);
        }

        // Normalize hub
        let hub_norm: f64 = new_hub.values().map(|x| x * x).sum::<f64>().sqrt();
        if hub_norm > 0.0 {
            for val in new_hub.values_mut() {
                *val /= hub_norm;
            }
        }
        hub_scores = new_hub;
    }

    common::HitsResult {
        hub_scores,
        authority_scores: auth_scores,
        iterations,
    }
}

/// Get word count for a document from corpus
fn get_word_count(corpus: &TestCorpus, source: &str) -> usize {
    corpus.items.iter()
        .find(|item| item.id.as_str() == source)
        .map(|item| item.content.split_whitespace().count())
        .unwrap_or(0)
}

/// Get document source path from node
fn get_source(node: &plexus::Node, node_id: &NodeId) -> String {
    match node.properties.get("source") {
        Some(PropertyValue::String(s)) => s.clone(),
        _ => node_id.to_string(),
    }
}

#[tokio::test]
#[ignore]
async fn test_seed_strategy_comparison() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");

    println!("\n{}", "=".repeat(70));
    println!("=== Spike 02b: Seed Selection Strategy Comparison ===");
    println!("{}\n", "=".repeat(70));

    // Get document nodes only
    let doc_nodes: Vec<_> = graph.context.nodes.iter()
        .filter(|(_, node)| node.node_type == "document")
        .collect();

    println!("Documents: {}", doc_nodes.len());

    // Calculate all metrics
    println!("\nCalculating metrics...");

    // 1. PageRank
    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);

    // 2. Betweenness Centrality
    let bc = betweenness_centrality(&graph.context);

    // 3. HITS on directed edges only
    let hits_directed = hits_directed_only(&graph.context, 100);

    // 4. HITS on full graph (for comparison)
    let hits_full = hits(&graph.context, 100);

    // Build word counts
    let word_counts: HashMap<String, usize> = doc_nodes.iter()
        .map(|(id, node)| {
            let source = get_source(node, id);
            let wc = get_word_count(&corpus, &source);
            (id.to_string(), wc)
        })
        .collect();

    // Score each document by each strategy
    let mut doc_scores: Vec<DocScores> = doc_nodes.iter()
        .map(|(id, node)| {
            let source = get_source(node, id);
            let word_count = *word_counts.get(&id.to_string()).unwrap_or(&0);

            DocScores {
                id: (*id).clone(),
                source,
                word_count,
                pagerank: *pr.scores.get(*id).unwrap_or(&0.0),
                betweenness: *bc.get(*id).unwrap_or(&0.0),
                hits_auth_directed: *hits_directed.authority_scores.get(*id).unwrap_or(&0.0),
                hits_hub_directed: *hits_directed.hub_scores.get(*id).unwrap_or(&0.0),
                hits_auth_full: *hits_full.authority_scores.get(*id).unwrap_or(&0.0),
                hits_hub_full: *hits_full.hub_scores.get(*id).unwrap_or(&0.0),
            }
        })
        .collect();

    // === Strategy 1: PageRank ===
    println!("\n--- Strategy 1: PageRank ---\n");
    doc_scores.sort_by(|a, b| b.pagerank.partial_cmp(&a.pagerank).unwrap());
    print_top_k(&doc_scores, TOP_K, "PageRank");
    let pr_quality = assess_quality(&doc_scores[..TOP_K]);

    // === Strategy 2: PageRank with word count filter ===
    println!("\n--- Strategy 2: PageRank (words > 100) ---\n");
    let mut filtered: Vec<_> = doc_scores.iter().filter(|d| d.word_count > 100).cloned().collect();
    filtered.sort_by(|a, b| b.pagerank.partial_cmp(&a.pagerank).unwrap());
    print_top_k(&filtered, TOP_K, "PageRank (filtered)");
    let pr_filtered_quality = assess_quality(&filtered[..TOP_K.min(filtered.len())]);

    // === Strategy 3: Betweenness Centrality ===
    println!("\n--- Strategy 3: Betweenness Centrality ---\n");
    doc_scores.sort_by(|a, b| b.betweenness.partial_cmp(&a.betweenness).unwrap());
    print_top_k(&doc_scores, TOP_K, "Betweenness");
    let bc_quality = assess_quality(&doc_scores[..TOP_K]);

    // === Strategy 4: HITS Authority (directed only) ===
    println!("\n--- Strategy 4: HITS Authority (G' directed) ---\n");
    doc_scores.sort_by(|a, b| b.hits_auth_directed.partial_cmp(&a.hits_auth_directed).unwrap());
    print_top_k(&doc_scores, TOP_K, "HITS-Auth(G')");
    let hits_dir_quality = assess_quality(&doc_scores[..TOP_K]);

    // === Strategy 5: HITS Authority vs Hub separation (directed) ===
    println!("\n--- Strategy 5: HITS Auth/Hub Separation (G' directed) ---\n");
    println!("{:<4} {:<35} {:>12} {:>12} {:>10}", "Rank", "Document", "Authority", "Hub", "Auth-Hub");
    println!("{}", "-".repeat(80));
    doc_scores.sort_by(|a, b| b.hits_auth_directed.partial_cmp(&a.hits_auth_directed).unwrap());
    for (i, doc) in doc_scores.iter().take(TOP_K).enumerate() {
        let diff = doc.hits_auth_directed - doc.hits_hub_directed;
        let short_source = truncate(&doc.source, 33);
        println!("{:<4} {:<35} {:>12.6} {:>12.6} {:>10.4}",
            i + 1, short_source, doc.hits_auth_directed, doc.hits_hub_directed, diff);
    }

    // === Strategy 6: Composite Score ===
    println!("\n--- Strategy 6: Composite (0.4·PR + 0.3·BC + 0.3·content) ---\n");

    // Normalize scores to [0,1]
    let pr_max = doc_scores.iter().map(|d| d.pagerank).fold(0.0f64, |a, b| a.max(b));
    let bc_max = doc_scores.iter().map(|d| d.betweenness).fold(0.0f64, |a, b| a.max(b));
    let wc_max = doc_scores.iter().map(|d| d.word_count as f64).fold(0.0f64, |a, b| a.max(b));

    let mut composite_scores: Vec<(DocScores, f64)> = doc_scores.iter()
        .map(|d| {
            let pr_norm = if pr_max > 0.0 { d.pagerank / pr_max } else { 0.0 };
            let bc_norm = if bc_max > 0.0 { d.betweenness / bc_max } else { 0.0 };
            let wc_norm = if wc_max > 0.0 { (d.word_count as f64) / wc_max } else { 0.0 };
            let composite = 0.4 * pr_norm + 0.3 * bc_norm + 0.3 * wc_norm;
            (d.clone(), composite)
        })
        .collect();
    composite_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("{:<4} {:<35} {:>8} {:>10} {:>10}", "Rank", "Document", "Words", "Composite", "Quality");
    println!("{}", "-".repeat(75));
    for (i, (doc, score)) in composite_scores.iter().take(TOP_K).enumerate() {
        let quality = classify_doc(doc);
        let short_source = truncate(&doc.source, 33);
        println!("{:<4} {:<35} {:>8} {:>10.4} {:>10}", i + 1, short_source, doc.word_count, score, quality);
    }
    let composite_docs: Vec<_> = composite_scores.iter().take(TOP_K).map(|(d, _)| d.clone()).collect();
    let composite_quality = assess_quality(&composite_docs);

    // === Summary ===
    println!("\n{}", "=".repeat(70));
    println!("=== SUMMARY ===");
    println!("{}\n", "=".repeat(70));

    println!("{:<40} {:>15} {:>10}", "Strategy", "Content-Rich+Mixed", "Verdict");
    println!("{}", "-".repeat(70));
    println!("{:<40} {:>15} {:>10}", "1. PageRank", format!("{}/8", pr_quality), verdict(pr_quality));
    println!("{:<40} {:>15} {:>10}", "2. PageRank (words > 100)", format!("{}/8", pr_filtered_quality), verdict(pr_filtered_quality));
    println!("{:<40} {:>15} {:>10}", "3. Betweenness Centrality", format!("{}/8", bc_quality), verdict(bc_quality));
    println!("{:<40} {:>15} {:>10}", "4. HITS Authority (G' directed)", format!("{}/8", hits_dir_quality), verdict(hits_dir_quality));
    println!("{:<40} {:>15} {:>10}", "6. Composite (PR+BC+content)", format!("{}/8", composite_quality), verdict(composite_quality));

    // === HITS Separation Analysis ===
    println!("\n--- HITS Auth/Hub Correlation ---\n");

    // Calculate correlation between auth and hub for full vs directed
    let full_corr = correlation(
        &doc_scores.iter().map(|d| d.hits_auth_full).collect::<Vec<_>>(),
        &doc_scores.iter().map(|d| d.hits_hub_full).collect::<Vec<_>>()
    );
    let dir_corr = correlation(
        &doc_scores.iter().map(|d| d.hits_auth_directed).collect::<Vec<_>>(),
        &doc_scores.iter().map(|d| d.hits_hub_directed).collect::<Vec<_>>()
    );

    println!("HITS on full graph (with reverse edges):    Auth/Hub correlation = {:.4}", full_corr);
    println!("HITS on G' (directed edges only):           Auth/Hub correlation = {:.4}", dir_corr);
    println!("\nLower correlation = better separation between hubs and authorities");
}

#[derive(Clone)]
struct DocScores {
    id: NodeId,
    source: String,
    word_count: usize,
    pagerank: f64,
    betweenness: f64,
    hits_auth_directed: f64,
    hits_hub_directed: f64,
    hits_auth_full: f64,
    hits_hub_full: f64,
}

fn print_top_k(docs: &[DocScores], k: usize, metric_name: &str) {
    println!("{:<4} {:<35} {:>8} {:>12} {:>10}", "Rank", "Document", "Words", metric_name, "Quality");
    println!("{}", "-".repeat(75));
    for (i, doc) in docs.iter().take(k).enumerate() {
        let quality = classify_doc(doc);
        let short_source = truncate(&doc.source, 33);
        let score = match metric_name {
            "PageRank" | "PageRank (filtered)" => doc.pagerank,
            "Betweenness" => doc.betweenness,
            "HITS-Auth(G')" => doc.hits_auth_directed,
            _ => 0.0,
        };
        println!("{:<4} {:<35} {:>8} {:>12.6} {:>10}", i + 1, short_source, doc.word_count, score, quality);
    }
}

fn classify_doc(doc: &DocScores) -> &'static str {
    let is_index_name = doc.source.to_lowercase().contains("index")
        || doc.source.to_lowercase().contains("readme")
        || doc.source.to_lowercase().contains("knowledge base");

    if is_index_name && doc.word_count < 100 {
        "Index/MOC"
    } else if doc.word_count >= 150 {
        "Content-Rich"
    } else if doc.word_count >= 50 {
        "Mixed"
    } else {
        "Index/MOC"
    }
}

fn assess_quality(docs: &[DocScores]) -> usize {
    docs.iter()
        .filter(|d| {
            let class = classify_doc(d);
            class == "Content-Rich" || class == "Mixed"
        })
        .count()
}

fn verdict(quality: usize) -> &'static str {
    if quality >= 6 { "GO ✓" }
    else if quality >= 4 { "PIVOT" }
    else { "NO-GO" }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - (max_len - 3)..])
    }
}

fn correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let mean_x: f64 = x.iter().sum::<f64>() / n;
    let mean_y: f64 = y.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x > 0.0 && var_y > 0.0 {
        cov / (var_x.sqrt() * var_y.sqrt())
    } else {
        0.0
    }
}
