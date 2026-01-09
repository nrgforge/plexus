//! Spike Experiment P1: Propagation Parameter Sweep
//!
//! **Research Question**: What propagation parameters (decay, hops, threshold)
//! optimize concept spreading in structured corpora?
//!
//! The system design assumes decay=0.7, hops=3, threshold=0.5 but these were
//! invented without validation. This experiment tests:
//! - decay: 0.5, 0.6, 0.7, 0.8, 0.9
//! - hops: 1, 2, 3, 4
//! - threshold: 0.5, 0.6, 0.7, 0.8
//!
//! **Metric**: Propagation usefulness = % of propagated concepts that also appear
//! in the target document's own extracted concepts (precision against ground truth).
//!
//! **Success**: Find parameters achieving >85% usefulness.

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use std::collections::{HashMap, HashSet, VecDeque};

/// A concept with confidence score
#[derive(Clone, Debug)]
struct Concept {
    name: String,
    confidence: f64,
}

/// Extract concepts from a document using mock logic (headings + code blocks)
fn extract_concepts(context: &Context, corpus: &TestCorpus, doc_id: &NodeId) -> Vec<Concept> {
    let mut concepts = Vec::new();

    let doc = match context.nodes.get(doc_id) {
        Some(d) if d.node_type == "document" => d,
        _ => return concepts,
    };

    let source_path = doc.properties.get("source")
        .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
        .unwrap_or("");

    let content = corpus.items.iter()
        .find(|item| item.id.as_str() == source_path)
        .map(|item| item.content.as_str())
        .unwrap_or("");

    // Extract from headers (high confidence)
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            let name = trimmed.trim_start_matches("# ").trim().to_lowercase();
            if !name.is_empty() {
                concepts.push(Concept { name, confidence: 1.0 });
            }
        } else if trimmed.starts_with("## ") {
            let name = trimmed.trim_start_matches("## ").trim().to_lowercase();
            if !name.is_empty() {
                concepts.push(Concept { name, confidence: 0.9 });
            }
        } else if trimmed.starts_with("### ") {
            let name = trimmed.trim_start_matches("### ").trim().to_lowercase();
            if !name.is_empty() {
                concepts.push(Concept { name, confidence: 0.8 });
            }
        }
    }

    // Extract code block languages (medium confidence)
    let code_pattern = regex_lite::Regex::new(r"```(\w+)").unwrap();
    for cap in code_pattern.captures_iter(content) {
        if let Some(lang) = cap.get(1) {
            let name = lang.as_str().to_lowercase();
            if !["text", "plaintext", "output", "console"].contains(&name.as_str()) {
                concepts.push(Concept { name, confidence: 0.7 });
            }
        }
    }

    concepts
}

/// Build adjacency list for BFS traversal
fn build_adjacency(context: &Context) -> HashMap<NodeId, Vec<(NodeId, f64)>> {
    let mut adj: HashMap<NodeId, Vec<(NodeId, f64)>> = HashMap::new();

    // Initialize all document nodes
    for (node_id, node) in &context.nodes {
        if node.node_type == "document" {
            adj.insert(node_id.clone(), Vec::new());
        }
    }

    // Add edges with weights
    for edge in &context.edges {
        let src = &edge.source;
        let tgt = &edge.target;

        // Only consider document-to-document edges
        let src_is_doc = context.nodes.get(src).map(|n| n.node_type == "document").unwrap_or(false);
        let tgt_is_doc = context.nodes.get(tgt).map(|n| n.node_type == "document").unwrap_or(false);

        if !src_is_doc || !tgt_is_doc {
            continue;
        }

        // Weight by edge type (sibling edges weighted higher per findings)
        let weight = match edge.relationship.as_str() {
            "sibling" => 1.0,        // Strongest signal (9x)
            "links_to" => 0.5,       // Medium
            "linked_from" => 0.3,    // Weaker
            _ => 0.2,
        };

        adj.entry(src.clone()).or_default().push((tgt.clone(), weight));
    }

    adj
}

/// Propagate concepts from a source document with given parameters
/// Returns: map of target_doc_id -> Vec<(concept_name, propagated_confidence)>
fn propagate_concepts(
    context: &Context,
    adj: &HashMap<NodeId, Vec<(NodeId, f64)>>,
    source_id: &NodeId,
    source_concepts: &[Concept],
    decay: f64,
    max_hops: usize,
    threshold: f64,
) -> HashMap<NodeId, Vec<(String, f64)>> {
    let mut result: HashMap<NodeId, Vec<(String, f64)>> = HashMap::new();

    // BFS with distance tracking
    let mut queue: VecDeque<(NodeId, usize, f64)> = VecDeque::new(); // (node, hops, cumulative_weight)
    let mut visited: HashSet<NodeId> = HashSet::new();

    queue.push_back((source_id.clone(), 0, 1.0));
    visited.insert(source_id.clone());

    while let Some((current, hops, path_weight)) = queue.pop_front() {
        if hops >= max_hops {
            continue;
        }

        if let Some(neighbors) = adj.get(&current) {
            for (neighbor, edge_weight) in neighbors {
                if visited.contains(neighbor) {
                    continue;
                }

                visited.insert(neighbor.clone());

                // Calculate propagated confidence for each concept
                let new_weight = path_weight * decay * edge_weight;

                let propagated: Vec<(String, f64)> = source_concepts.iter()
                    .map(|c| {
                        let conf = c.confidence * new_weight;
                        (c.name.clone(), conf)
                    })
                    .filter(|(_, conf)| *conf >= threshold)
                    .collect();

                if !propagated.is_empty() {
                    result.insert(neighbor.clone(), propagated);
                }

                queue.push_back((neighbor.clone(), hops + 1, new_weight));
            }
        }
    }

    result
}

/// Measure propagation usefulness:
/// For each propagated concept at target, check if target also has that concept
/// Returns (precision, recall, f1)
fn measure_usefulness(
    propagated: &[(String, f64)],
    target_concepts: &[Concept],
) -> (f64, f64, f64) {
    let propagated_set: HashSet<&str> = propagated.iter().map(|(n, _)| n.as_str()).collect();
    let target_set: HashSet<&str> = target_concepts.iter().map(|c| c.name.as_str()).collect();

    if propagated_set.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let true_positives = propagated_set.intersection(&target_set).count();

    let precision = true_positives as f64 / propagated_set.len() as f64;
    let recall = if target_set.is_empty() { 0.0 } else { true_positives as f64 / target_set.len() as f64 };
    let f1 = if precision + recall > 0.0 { 2.0 * precision * recall / (precision + recall) } else { 0.0 };

    (precision, recall, f1)
}

#[derive(Debug, Clone)]
struct ParamResult {
    decay: f64,
    hops: usize,
    threshold: f64,
    avg_precision: f64,
    avg_recall: f64,
    avg_f1: f64,
    propagation_count: usize,
    useful_propagations: usize,
}

#[tokio::test]
async fn test_propagation_parameter_sweep() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment P1: Propagation Parameter Sweep ===");
    println!("{}\n", "=".repeat(80));

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;
    let adj = build_adjacency(context);

    // Get all document nodes
    let doc_ids: Vec<NodeId> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id.clone())
        .collect();

    println!("Documents: {}", doc_ids.len());
    println!("Edges in adjacency: {}", adj.values().map(|v| v.len()).sum::<usize>());

    // Extract concepts for all documents (ground truth)
    let mut doc_concepts: HashMap<NodeId, Vec<Concept>> = HashMap::new();
    for doc_id in &doc_ids {
        let concepts = extract_concepts(context, &corpus, doc_id);
        doc_concepts.insert(doc_id.clone(), concepts);
    }

    // Parameter grid
    let decays = [0.5, 0.6, 0.7, 0.8, 0.9];
    let hops_list = [1, 2, 3, 4];
    let thresholds = [0.3, 0.4, 0.5, 0.6, 0.7];

    let mut results: Vec<ParamResult> = Vec::new();

    println!("\nRunning parameter sweep...\n");

    for &decay in &decays {
        for &hops in &hops_list {
            for &threshold in &thresholds {
                let mut total_precision = 0.0;
                let mut total_recall = 0.0;
                let mut total_f1 = 0.0;
                let mut propagation_count = 0usize;
                let mut useful_count = 0usize;

                // Test propagation from each document that has concepts
                for source_id in &doc_ids {
                    let source_concepts = match doc_concepts.get(source_id) {
                        Some(c) if !c.is_empty() => c,
                        _ => continue,
                    };

                    let propagated = propagate_concepts(
                        context, &adj, source_id, source_concepts,
                        decay, hops, threshold
                    );

                    for (target_id, prop_concepts) in &propagated {
                        let target_concepts = doc_concepts.get(target_id).cloned().unwrap_or_default();
                        let (precision, recall, f1) = measure_usefulness(prop_concepts, &target_concepts);

                        total_precision += precision;
                        total_recall += recall;
                        total_f1 += f1;
                        propagation_count += 1;

                        if precision > 0.5 {
                            useful_count += 1;
                        }
                    }
                }

                if propagation_count > 0 {
                    results.push(ParamResult {
                        decay,
                        hops,
                        threshold,
                        avg_precision: total_precision / propagation_count as f64,
                        avg_recall: total_recall / propagation_count as f64,
                        avg_f1: total_f1 / propagation_count as f64,
                        propagation_count,
                        useful_propagations: useful_count,
                    });
                }
            }
        }
    }

    // Sort by F1 score
    results.sort_by(|a, b| b.avg_f1.partial_cmp(&a.avg_f1).unwrap());

    // Print top 20 results
    println!("{}", "=".repeat(100));
    println!("{:<8} {:>6} {:>10} {:>12} {:>10} {:>10} {:>12} {:>10}",
        "Decay", "Hops", "Threshold", "Precision", "Recall", "F1", "PropCount", "Useful%");
    println!("{}", "-".repeat(100));

    for r in results.iter().take(20) {
        let useful_pct = if r.propagation_count > 0 {
            100.0 * r.useful_propagations as f64 / r.propagation_count as f64
        } else { 0.0 };

        println!("{:<8.2} {:>6} {:>10.2} {:>12.3} {:>10.3} {:>10.3} {:>12} {:>9.1}%",
            r.decay, r.hops, r.threshold,
            r.avg_precision, r.avg_recall, r.avg_f1,
            r.propagation_count, useful_pct);
    }

    // Find best by different metrics
    println!("\n{}", "=".repeat(80));
    println!("=== Best Parameters by Metric ===\n");

    if let Some(best_precision) = results.iter().max_by(|a, b| a.avg_precision.partial_cmp(&b.avg_precision).unwrap()) {
        println!("Best Precision: decay={:.1}, hops={}, threshold={:.1} -> {:.3}",
            best_precision.decay, best_precision.hops, best_precision.threshold, best_precision.avg_precision);
    }

    if let Some(best_recall) = results.iter().max_by(|a, b| a.avg_recall.partial_cmp(&b.avg_recall).unwrap()) {
        println!("Best Recall:    decay={:.1}, hops={}, threshold={:.1} -> {:.3}",
            best_recall.decay, best_recall.hops, best_recall.threshold, best_recall.avg_recall);
    }

    if let Some(best_f1) = results.iter().max_by(|a, b| a.avg_f1.partial_cmp(&b.avg_f1).unwrap()) {
        println!("Best F1:        decay={:.1}, hops={}, threshold={:.1} -> {:.3}",
            best_f1.decay, best_f1.hops, best_f1.threshold, best_f1.avg_f1);
    }

    // Check against assumed parameters
    println!("\n{}", "=".repeat(80));
    println!("=== Comparison with Assumed Parameters (decay=0.7, hops=3, threshold=0.5) ===\n");

    if let Some(assumed) = results.iter().find(|r|
        (r.decay - 0.7).abs() < 0.01 && r.hops == 3 && (r.threshold - 0.5).abs() < 0.01
    ) {
        println!("Assumed params:  precision={:.3}, recall={:.3}, F1={:.3}",
            assumed.avg_precision, assumed.avg_recall, assumed.avg_f1);
    } else {
        println!("Assumed params not in results (may have 0 propagations)");
    }

    if let Some(best) = results.first() {
        println!("Best params:     precision={:.3}, recall={:.3}, F1={:.3}",
            best.avg_precision, best.avg_recall, best.avg_f1);
        println!("\nRecommended: decay={:.1}, hops={}, threshold={:.1}",
            best.decay, best.hops, best.threshold);
    }

    // Verdict
    println!("\n{}", "=".repeat(80));
    let best_precision = results.iter().map(|r| r.avg_precision).fold(0.0, f64::max);
    if best_precision >= 0.85 {
        println!("VERDICT: GO - Found parameters with >85% precision");
    } else if best_precision >= 0.70 {
        println!("VERDICT: PIVOT - Best precision {:.1}% < 85% target", best_precision * 100.0);
    } else {
        println!("VERDICT: NEEDS INVESTIGATION - Best precision {:.1}%", best_precision * 100.0);
    }
    println!("{}", "=".repeat(80));
}

/// Diagnostic: Understand concept extraction and overlap
#[tokio::test]
async fn test_concept_diagnostic() {
    println!("\n{}", "=".repeat(80));
    println!("=== DIAGNOSTIC: Concept Extraction Analysis ===");
    println!("{}\n", "=".repeat(80));

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;
    let adj = build_adjacency(context);

    let doc_ids: Vec<NodeId> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id.clone())
        .collect();

    // Extract concepts for all documents
    let mut doc_concepts: HashMap<NodeId, Vec<Concept>> = HashMap::new();
    let mut all_concepts: HashSet<String> = HashSet::new();

    for doc_id in &doc_ids {
        let concepts = extract_concepts(context, &corpus, doc_id);
        for c in &concepts {
            all_concepts.insert(c.name.clone());
        }
        doc_concepts.insert(doc_id.clone(), concepts);
    }

    println!("Total unique concepts across corpus: {}", all_concepts.len());
    println!("Documents with concepts: {}", doc_concepts.values().filter(|c| !c.is_empty()).count());

    // Show concept distribution
    let mut concept_freq: HashMap<&str, usize> = HashMap::new();
    for concepts in doc_concepts.values() {
        for c in concepts {
            *concept_freq.entry(c.name.as_str()).or_insert(0) += 1;
        }
    }

    let mut freq_vec: Vec<_> = concept_freq.iter().collect();
    freq_vec.sort_by(|a, b| b.1.cmp(a.1));

    println!("\nTop 20 most frequent concepts:");
    println!("{:<30} {:>10}", "Concept", "Documents");
    println!("{}", "-".repeat(42));
    for (concept, freq) in freq_vec.iter().take(20) {
        println!("{:<30} {:>10}", concept, freq);
    }

    // Analyze sibling overlap specifically
    println!("\n{}", "=".repeat(80));
    println!("=== Sibling Concept Overlap Analysis ===\n");

    let mut sibling_pairs = 0;
    let mut sibling_overlap_sum = 0.0;
    let mut link_pairs = 0;
    let mut link_overlap_sum = 0.0;

    for edge in &context.edges {
        let src_concepts = doc_concepts.get(&edge.source);
        let tgt_concepts = doc_concepts.get(&edge.target);

        if let (Some(src), Some(tgt)) = (src_concepts, tgt_concepts) {
            if src.is_empty() || tgt.is_empty() {
                continue;
            }

            let src_set: HashSet<&str> = src.iter().map(|c| c.name.as_str()).collect();
            let tgt_set: HashSet<&str> = tgt.iter().map(|c| c.name.as_str()).collect();
            let overlap = src_set.intersection(&tgt_set).count();
            let jaccard = if src_set.len() + tgt_set.len() > 0 {
                overlap as f64 / (src_set.len() + tgt_set.len() - overlap) as f64
            } else { 0.0 };

            match edge.relationship.as_str() {
                "sibling" => {
                    sibling_pairs += 1;
                    sibling_overlap_sum += jaccard;
                }
                "links_to" => {
                    link_pairs += 1;
                    link_overlap_sum += jaccard;
                }
                _ => {}
            }
        }
    }

    println!("Sibling pairs: {}, avg Jaccard overlap: {:.3}",
        sibling_pairs,
        if sibling_pairs > 0 { sibling_overlap_sum / sibling_pairs as f64 } else { 0.0 });
    println!("Link pairs: {}, avg Jaccard overlap: {:.3}",
        link_pairs,
        if link_pairs > 0 { link_overlap_sum / link_pairs as f64 } else { 0.0 });

    // Show sample propagation
    println!("\n{}", "=".repeat(80));
    println!("=== Sample Propagation (first doc with concepts) ===\n");

    for source_id in &doc_ids {
        let source_concepts = match doc_concepts.get(source_id) {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        let source_name = context.nodes.get(source_id)
            .and_then(|n| n.properties.get("source"))
            .map(|v| format!("{:?}", v))
            .unwrap_or_default();

        println!("Source: {}", source_name);
        println!("Concepts: {:?}", source_concepts.iter().map(|c| &c.name).collect::<Vec<_>>());

        if let Some(neighbors) = adj.get(source_id) {
            println!("\nNeighbors ({}):", neighbors.len());
            for (neighbor_id, weight) in neighbors.iter().take(5) {
                let neighbor_name = context.nodes.get(neighbor_id)
                    .and_then(|n| n.properties.get("source"))
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_default();
                let neighbor_concepts = doc_concepts.get(neighbor_id);

                println!("  - {} (weight: {:.2})", neighbor_name, weight);
                if let Some(nc) = neighbor_concepts {
                    println!("    Concepts: {:?}", nc.iter().map(|c| &c.name).collect::<Vec<_>>());

                    // Calculate overlap
                    let src_set: HashSet<&str> = source_concepts.iter().map(|c| c.name.as_str()).collect();
                    let tgt_set: HashSet<&str> = nc.iter().map(|c| c.name.as_str()).collect();
                    let overlap: Vec<_> = src_set.intersection(&tgt_set).collect();
                    println!("    Overlap: {:?}", overlap);
                }
            }
        }

        break; // Just show first example
    }

    println!("\n{}", "=".repeat(80));
    println!("INSIGHT: Mock extraction uses headers + code languages.");
    println!("         These rarely overlap between documents (each doc has unique title).");
    println!("         Real semantic concepts (topics, technologies) would overlap more.");
    println!("{}", "=".repeat(80));
}

/// Test individual parameter effects
#[tokio::test]
async fn test_decay_effect() {
    println!("\n=== Decay Effect Analysis ===\n");

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;
    let adj = build_adjacency(context);

    let doc_ids: Vec<NodeId> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id.clone())
        .collect();

    let mut doc_concepts: HashMap<NodeId, Vec<Concept>> = HashMap::new();
    for doc_id in &doc_ids {
        doc_concepts.insert(doc_id.clone(), extract_concepts(context, &corpus, doc_id));
    }

    // Fix hops=2, threshold=0.5, vary decay
    let hops = 2;
    let threshold = 0.5;

    println!("{:<10} {:>12} {:>12} {:>12}", "Decay", "Precision", "Recall", "Propagations");
    println!("{}", "-".repeat(50));

    for decay in [0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0] {
        let mut total_precision = 0.0;
        let mut total_recall = 0.0;
        let mut count = 0usize;

        for source_id in &doc_ids {
            let source_concepts = match doc_concepts.get(source_id) {
                Some(c) if !c.is_empty() => c,
                _ => continue,
            };

            let propagated = propagate_concepts(context, &adj, source_id, source_concepts, decay, hops, threshold);

            for (target_id, prop_concepts) in &propagated {
                let target_concepts = doc_concepts.get(target_id).cloned().unwrap_or_default();
                let (precision, recall, _) = measure_usefulness(prop_concepts, &target_concepts);
                total_precision += precision;
                total_recall += recall;
                count += 1;
            }
        }

        if count > 0 {
            println!("{:<10.2} {:>12.3} {:>12.3} {:>12}",
                decay,
                total_precision / count as f64,
                total_recall / count as f64,
                count);
        }
    }
}
