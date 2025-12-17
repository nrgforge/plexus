//! Spike Investigation 03b: Multi-Corpus Link↔Semantic Correlation
//!
//! Tests whether the link↔semantic findings from pkm-webdev generalize
//! to other corpus types:
//! - arch-wiki: Large, semi-structured wiki
//! - pkm-datascience: Another PKM vault
//! - shakespeare: Flat literary corpus (no directory structure)

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use std::collections::{HashMap, HashSet};
use rand::seq::SliceRandom;
use rand::SeedableRng;

/// Corpus analysis results
#[derive(Debug)]
struct CorpusAnalysis {
    name: String,
    doc_count: usize,
    dir_count: usize,
    linked_pairs: usize,
    sibling_pairs: usize,
    linked_mean_jaccard: f64,
    sibling_mean_jaccard: f64,
    random_mean_jaccard: f64,
    linked_overlap_pct: f64,
    sibling_overlap_pct: f64,
    random_overlap_pct: f64,
}

/// Find document pairs linked via wikilinks (resolving by filename)
fn find_linked_pairs(context: &Context) -> Vec<(NodeId, NodeId)> {
    let mut pairs = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Build filename lookup
    let mut filename_to_doc: HashMap<String, NodeId> = HashMap::new();
    for (id, node) in context.nodes.iter() {
        if node.node_type != "document" { continue; }
        let source = node.properties.get("source")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");
        let filename = std::path::Path::new(source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !filename.is_empty() {
            filename_to_doc.insert(filename, id.clone());
        }
    }

    // Find link nodes and trace paths
    for (link_id, node) in context.nodes.iter() {
        if node.node_type != "link" { continue; }

        let parent_doc = context.edges.iter()
            .find(|e| e.target == *link_id && e.relationship == "contains")
            .and_then(|e| {
                let n = context.nodes.get(&e.source)?;
                if n.node_type == "document" { Some(e.source.clone()) } else { None }
            });

        let target_ref = context.edges.iter()
            .find(|e| e.source == *link_id && e.relationship == "links_to")
            .map(|e| e.target.clone());

        if let (Some(src), Some(tgt_ref)) = (parent_doc, target_ref) {
            let tgt_filename = tgt_ref.as_str()
                .trim_end_matches(":document")
                .trim_end_matches(".md")
                .to_lowercase();

            let tgt = if context.nodes.get(&tgt_ref).map(|n| n.node_type == "document").unwrap_or(false) {
                Some(tgt_ref)
            } else {
                filename_to_doc.get(&tgt_filename).cloned()
            };

            if let Some(tgt_id) = tgt {
                if src != tgt_id {
                    let key = format!("{}:{}", src.as_str(), tgt_id.as_str());
                    if seen.insert(key) {
                        pairs.push((src, tgt_id));
                    }
                }
            }
        }
    }
    pairs
}

/// Find sibling pairs (same directory, not linked)
fn find_sibling_pairs(context: &Context, linked_set: &HashSet<String>) -> Vec<(NodeId, NodeId)> {
    let mut docs_by_dir: HashMap<String, Vec<NodeId>> = HashMap::new();

    for (id, node) in context.nodes.iter() {
        if node.node_type != "document" { continue; }
        let source = node.properties.get("source")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");
        let dir = std::path::Path::new(source)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        docs_by_dir.entry(dir).or_default().push(id.clone());
    }

    let mut pairs = Vec::new();
    for docs in docs_by_dir.values() {
        if docs.len() < 2 { continue; }
        for i in 0..docs.len() {
            for j in (i+1)..docs.len() {
                let key1 = format!("{}:{}", docs[i].as_str(), docs[j].as_str());
                let key2 = format!("{}:{}", docs[j].as_str(), docs[i].as_str());
                if !linked_set.contains(&key1) && !linked_set.contains(&key2) {
                    pairs.push((docs[i].clone(), docs[j].clone()));
                }
            }
        }
    }
    pairs
}

/// Generate random pairs
fn random_pairs(
    context: &Context,
    count: usize,
    exclude: &HashSet<String>,
    rng: &mut impl rand::Rng,
) -> Vec<(NodeId, NodeId)> {
    let doc_ids: Vec<_> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id.clone())
        .collect();

    let mut pairs = Vec::new();
    let mut attempts = 0;
    while pairs.len() < count && attempts < count * 100 {
        attempts += 1;
        if let (Some(a), Some(b)) = (doc_ids.choose(rng).cloned(), doc_ids.choose(rng).cloned()) {
            if a != b {
                let k1 = format!("{}:{}", a.as_str(), b.as_str());
                let k2 = format!("{}:{}", b.as_str(), a.as_str());
                if !exclude.contains(&k1) && !exclude.contains(&k2) {
                    pairs.push((a, b));
                }
            }
        }
    }
    pairs
}

/// Extract concepts from document content
fn extract_concepts(corpus: &TestCorpus, source_path: &str) -> HashSet<String> {
    let mut concepts = HashSet::new();

    let content = corpus.items.iter()
        .find(|item| item.id.as_str() == source_path)
        .map(|item| item.content.as_str())
        .unwrap_or("");

    // Headers
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            concepts.insert(trimmed[2..].trim().to_lowercase());
        } else if trimmed.starts_with("## ") {
            concepts.insert(trimmed[3..].trim().to_lowercase());
        } else if trimmed.starts_with("### ") {
            concepts.insert(trimmed[4..].trim().to_lowercase());
        }
    }

    // Code blocks
    let code_re = regex_lite::Regex::new(r"```(\w+)").unwrap();
    for cap in code_re.captures_iter(content) {
        if let Some(lang) = cap.get(1) {
            let l = lang.as_str().to_lowercase();
            if !["text", "plaintext", "output", "console"].contains(&l.as_str()) {
                concepts.insert(l);
            }
        }
    }

    // Wikilinks as refs
    let wiki_re = regex_lite::Regex::new(r"\[\[([^\]|]+)").unwrap();
    for cap in wiki_re.captures_iter(content) {
        if let Some(target) = cap.get(1) {
            concepts.insert(format!("ref:{}", target.as_str().to_lowercase()));
        }
    }

    concepts
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() { return 0.0; }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

/// Compute similarity stats for a set of pairs
fn compute_stats(
    pairs: &[(NodeId, NodeId)],
    context: &Context,
    corpus: &TestCorpus,
) -> (f64, f64) {  // (mean_jaccard, overlap_pct)
    if pairs.is_empty() {
        return (0.0, 0.0);
    }

    let mut sims: Vec<f64> = Vec::new();
    for (src, tgt) in pairs {
        let src_path = context.nodes.get(src)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");
        let tgt_path = context.nodes.get(tgt)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");

        let src_concepts = extract_concepts(corpus, src_path);
        let tgt_concepts = extract_concepts(corpus, tgt_path);
        sims.push(jaccard(&src_concepts, &tgt_concepts));
    }

    let mean = sims.iter().sum::<f64>() / sims.len() as f64;
    let overlap_pct = 100.0 * sims.iter().filter(|&&s| s > 0.0).count() as f64 / sims.len() as f64;
    (mean, overlap_pct)
}

/// Analyze a single corpus
async fn analyze_corpus(name: &str) -> Option<CorpusAnalysis> {
    let corpus = match TestCorpus::load(name) {
        Ok(c) => c,
        Err(e) => {
            println!("  Skipping {} - {}", name, e);
            return None;
        }
    };

    let graph = match build_structure_graph(&corpus).await {
        Ok(g) => g,
        Err(e) => {
            println!("  Skipping {} - build failed: {:?}", name, e);
            return None;
        }
    };

    let context = &graph.context;

    // Count docs and directories
    let doc_count = context.nodes.iter().filter(|(_, n)| n.node_type == "document").count();
    let dir_count = context.nodes.iter().filter(|(_, n)| n.node_type == "directory").count();

    // Find pairs
    let linked = find_linked_pairs(context);
    let linked_set: HashSet<_> = linked.iter()
        .flat_map(|(a, b)| vec![
            format!("{}:{}", a.as_str(), b.as_str()),
            format!("{}:{}", b.as_str(), a.as_str())
        ])
        .collect();

    let siblings = find_sibling_pairs(context, &linked_set);

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let sample_size = linked.len().max(siblings.len()).max(20).min(100);
    let random = random_pairs(context, sample_size, &linked_set, &mut rng);

    // Compute stats
    let (linked_mean, linked_overlap) = compute_stats(&linked, context, &corpus);
    let (sibling_mean, sibling_overlap) = compute_stats(&siblings, context, &corpus);
    let (random_mean, random_overlap) = compute_stats(&random, context, &corpus);

    Some(CorpusAnalysis {
        name: name.to_string(),
        doc_count,
        dir_count,
        linked_pairs: linked.len(),
        sibling_pairs: siblings.len(),
        linked_mean_jaccard: linked_mean,
        sibling_mean_jaccard: sibling_mean,
        random_mean_jaccard: random_mean,
        linked_overlap_pct: linked_overlap,
        sibling_overlap_pct: sibling_overlap,
        random_overlap_pct: random_overlap,
    })
}

#[tokio::test]
async fn test_multi_corpus_comparison() {
    println!("\n{}", "=".repeat(80));
    println!("=== Investigation 3b: Multi-Corpus Link↔Semantic Comparison ===");
    println!("{}\n", "=".repeat(80));

    // Skip arch-wiki for now - too large (60k+ nodes)
    let corpora = ["pkm-webdev", "pkm-datascience", "shakespeare"];
    let mut results: Vec<CorpusAnalysis> = Vec::new();

    for name in &corpora {
        println!("Analyzing {}...", name);
        if let Some(analysis) = analyze_corpus(name).await {
            results.push(analysis);
        }
    }

    // Print corpus structure
    println!("\n{}", "=".repeat(80));
    println!("=== Corpus Structure ===");
    println!("{}", "=".repeat(80));
    println!("\n{:<20} {:>8} {:>8} {:>12} {:>12}",
        "Corpus", "Docs", "Dirs", "Linked", "Siblings");
    println!("{}", "-".repeat(70));
    for r in &results {
        println!("{:<20} {:>8} {:>8} {:>12} {:>12}",
            r.name, r.doc_count, r.dir_count, r.linked_pairs, r.sibling_pairs);
    }

    // Print similarity comparison
    println!("\n{}", "=".repeat(80));
    println!("=== Mean Jaccard Similarity ===");
    println!("{}", "=".repeat(80));
    println!("\n{:<20} {:>12} {:>12} {:>12} {:>15}",
        "Corpus", "Linked", "Siblings", "Random", "Best Signal");
    println!("{}", "-".repeat(75));
    for r in &results {
        let best = if r.sibling_mean_jaccard > r.linked_mean_jaccard && r.sibling_mean_jaccard > r.random_mean_jaccard {
            "Siblings"
        } else if r.linked_mean_jaccard > r.random_mean_jaccard {
            "Links"
        } else {
            "None"
        };
        println!("{:<20} {:>12.4} {:>12.4} {:>12.4} {:>15}",
            r.name, r.linked_mean_jaccard, r.sibling_mean_jaccard, r.random_mean_jaccard, best);
    }

    // Print overlap percentages
    println!("\n{}", "=".repeat(80));
    println!("=== % Pairs With Any Overlap ===");
    println!("{}", "=".repeat(80));
    println!("\n{:<20} {:>12} {:>12} {:>12}",
        "Corpus", "Linked", "Siblings", "Random");
    println!("{}", "-".repeat(60));
    for r in &results {
        println!("{:<20} {:>11.1}% {:>11.1}% {:>11.1}%",
            r.name, r.linked_overlap_pct, r.sibling_overlap_pct, r.random_overlap_pct);
    }

    // Analysis
    println!("\n{}", "=".repeat(80));
    println!("=== Analysis ===");
    println!("{}", "=".repeat(80));

    for r in &results {
        println!("\n{}:", r.name);

        // Directory structure assessment
        let dir_ratio = if r.doc_count > 0 { r.dir_count as f64 / r.doc_count as f64 } else { 0.0 };
        if dir_ratio > 0.3 {
            println!("  Structure: Well-organized ({} dirs / {} docs = {:.1}%)",
                r.dir_count, r.doc_count, dir_ratio * 100.0);
        } else if dir_ratio > 0.1 {
            println!("  Structure: Moderately organized ({:.1}% dir ratio)", dir_ratio * 100.0);
        } else {
            println!("  Structure: Flat ({:.1}% dir ratio)", dir_ratio * 100.0);
        }

        // Signal assessment
        let link_improvement = if r.random_mean_jaccard > 0.0 {
            (r.linked_mean_jaccard - r.random_mean_jaccard) / r.random_mean_jaccard * 100.0
        } else if r.linked_mean_jaccard > 0.0 { 100.0 } else { 0.0 };

        let sibling_improvement = if r.random_mean_jaccard > 0.0 {
            (r.sibling_mean_jaccard - r.random_mean_jaccard) / r.random_mean_jaccard * 100.0
        } else if r.sibling_mean_jaccard > 0.0 { 100.0 } else { 0.0 };

        println!("  Link signal: {:.1}% vs random ({})",
            link_improvement,
            if link_improvement > 30.0 { "GO" } else if link_improvement > 10.0 { "PIVOT" } else { "NO-GO" });
        println!("  Sibling signal: {:.1}% vs random ({})",
            sibling_improvement,
            if sibling_improvement > 30.0 { "GO" } else if sibling_improvement > 10.0 { "PIVOT" } else { "NO-GO" });

        // Recommendation
        if r.sibling_mean_jaccard > r.linked_mean_jaccard * 2.0 && r.sibling_pairs > 10 {
            println!("  → Recommendation: Weight SIBLINGS heavily for this corpus");
        } else if r.linked_mean_jaccard > r.sibling_mean_jaccard * 2.0 && r.linked_pairs > 10 {
            println!("  → Recommendation: Weight LINKS heavily for this corpus");
        } else if r.linked_pairs == 0 && r.sibling_pairs == 0 {
            println!("  → Recommendation: Need CONTENT-BASED similarity (no structural signal)");
        } else {
            println!("  → Recommendation: Use balanced weighting");
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("=== Key Takeaway ===");
    println!("{}", "=".repeat(80));
    println!("\nThe optimal edge weighting strategy depends on corpus structure:");
    println!("  • Well-organized PKM vaults: Trust directory structure (siblings)");
    println!("  • Flat/literary corpora: Rely on content analysis or explicit links");
    println!("  • Mixed corpora: Use adaptive weighting based on dir/doc ratio");
}
