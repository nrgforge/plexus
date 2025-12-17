//! Spike Investigation 03: Link↔Semantic Correlation
//!
//! **Critical Question**: Do documents that link to each other share semantic concepts?
//!
//! This is the hinge point of the entire spike. If linked documents don't share
//! concepts, then graph structure is meaningless for propagation—we'd just be
//! spreading noise.
//!
//! Test approach:
//! 1. Find document pairs connected by wikilinks (A → link_node → B)
//! 2. Extract concepts from both documents using mock analyzer
//! 3. Compute Jaccard similarity of concept sets
//! 4. Compare to random baseline (unlinked pairs)
//!
//! **Criteria**:
//! - GO: Linked pairs have ≥30% higher similarity than random pairs
//! - PIVOT: 10-30% higher similarity
//! - NO-GO: <10% difference (links don't predict semantic similarity)

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use std::collections::{HashMap, HashSet};
use rand::seq::SliceRandom;
use rand::SeedableRng;

/// Find document pairs that are linked via wikilinks
/// Returns Vec<(source_doc_id, target_doc_id)>
fn find_linked_document_pairs(context: &Context) -> Vec<(NodeId, NodeId)> {
    let mut pairs = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Build a lookup map from filename to document node ID
    // This handles wikilinks like [[Desktop Launchers]] -> Linux/Gnome/Desktop Launchers.md
    let mut filename_to_doc: HashMap<String, NodeId> = HashMap::new();
    for (id, node) in context.nodes.iter() {
        if node.node_type != "document" { continue; }

        let source = node.properties.get("source")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");

        // Extract just the filename without path
        let filename = std::path::Path::new(source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !filename.is_empty() {
            filename_to_doc.insert(filename, id.clone());
        }
    }

    // Get all link nodes
    let link_nodes: Vec<_> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "link")
        .collect();

    for (link_id, _link_node) in &link_nodes {
        // Find parent document (doc --contains--> link)
        let parent_doc_id = context.edges.iter()
            .find(|e| e.target == **link_id && e.relationship == "contains")
            .and_then(|e| {
                let node = context.nodes.get(&e.source)?;
                if node.node_type == "document" { Some(e.source.clone()) } else { None }
            });

        // Find target from links_to edge
        let links_to_target = context.edges.iter()
            .find(|e| e.source == **link_id && e.relationship == "links_to")
            .map(|e| e.target.clone());

        if let (Some(src_id), Some(target_ref)) = (parent_doc_id, links_to_target) {
            // The target_ref might be like "Desktop Launchers.md:document" (unresolved)
            // Try to resolve it to an actual document by filename
            let target_filename = target_ref.as_str()
                .trim_end_matches(":document")
                .trim_end_matches(".md")
                .to_lowercase();

            // Check if target already exists as a node
            let target_doc_id = if context.nodes.get(&target_ref)
                .map(|n| n.node_type == "document")
                .unwrap_or(false)
            {
                Some(target_ref)
            } else {
                // Try to resolve by filename
                filename_to_doc.get(&target_filename).cloned()
            };

            if let Some(tgt_id) = target_doc_id {
                // Avoid self-links and deduplicate
                if src_id != tgt_id {
                    let key = format!("{}:{}", src_id.as_str(), tgt_id.as_str());
                    if seen.insert(key) {
                        pairs.push((src_id, tgt_id));
                    }
                }
            }
        }
    }

    pairs
}

/// Extract concepts from a document (using mock analysis logic)
/// Returns set of concept names (lowercase for comparison)
fn extract_concepts_from_doc_with_corpus(context: &Context, corpus: &TestCorpus, doc_id: &NodeId) -> HashSet<String> {
    let mut concepts = HashSet::new();

    // Get document source path
    let doc = match context.nodes.get(doc_id) {
        Some(d) if d.node_type == "document" => d,
        _ => return concepts,
    };

    let source_path = doc.properties.get("source")
        .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
        .unwrap_or("");

    // Get content from corpus
    let content = corpus.items.iter()
        .find(|item| item.id.as_str() == source_path)
        .map(|item| item.content.as_str())
        .unwrap_or("");

    // Mock concept extraction (same logic as MockSemanticAnalyzer)
    for line in content.lines() {
        let trimmed = line.trim();

        // H1-H3 headers
        if trimmed.starts_with("# ") {
            let name = trimmed.trim_start_matches("# ").trim().to_lowercase();
            if !name.is_empty() {
                concepts.insert(name);
            }
        } else if trimmed.starts_with("## ") {
            let name = trimmed.trim_start_matches("## ").trim().to_lowercase();
            if !name.is_empty() {
                concepts.insert(name);
            }
        } else if trimmed.starts_with("### ") {
            let name = trimmed.trim_start_matches("### ").trim().to_lowercase();
            if !name.is_empty() {
                concepts.insert(name);
            }
        }
    }

    // Code block languages
    let code_block_pattern = regex_lite::Regex::new(r"```(\w+)").unwrap();
    for cap in code_block_pattern.captures_iter(content) {
        if let Some(lang) = cap.get(1) {
            let lang_name = lang.as_str().to_lowercase();
            if !["text", "plaintext", "output", "console"].contains(&lang_name.as_str()) {
                concepts.insert(lang_name);
            }
        }
    }

    // Also extract wikilink targets as implicit concepts
    // [[Typescript]] in a doc suggests the doc is about Typescript
    let wikilink_pattern = regex_lite::Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    for cap in wikilink_pattern.captures_iter(content) {
        if let Some(target) = cap.get(1) {
            // Handle aliased links like [[Page|Display Text]]
            let target_name = target.as_str().split('|').next().unwrap_or("");
            if !target_name.is_empty() {
                concepts.insert(format!("ref:{}", target_name.to_lowercase()));
            }
        }
    }

    concepts
}

/// Compute Jaccard similarity between two sets
fn jaccard_similarity(set_a: &HashSet<String>, set_b: &HashSet<String>) -> f64 {
    if set_a.is_empty() && set_b.is_empty() {
        return 0.0;
    }

    let intersection = set_a.intersection(set_b).count();
    let union = set_a.union(set_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Generate random document pairs (not necessarily linked)
fn generate_random_pairs(
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
    let max_attempts = count * 100;

    while pairs.len() < count && attempts < max_attempts {
        attempts += 1;

        let a = doc_ids.choose(rng).cloned();
        let b = doc_ids.choose(rng).cloned();

        if let (Some(a), Some(b)) = (a, b) {
            if a != b {
                let key1 = format!("{}:{}", a.as_str(), b.as_str());
                let key2 = format!("{}:{}", b.as_str(), a.as_str());
                if !exclude.contains(&key1) && !exclude.contains(&key2) {
                    pairs.push((a, b));
                }
            }
        }
    }

    pairs
}

/// Debug test to understand link structure
#[tokio::test]
async fn test_debug_link_structure() {
    println!("\n{}", "=".repeat(70));
    println!("=== Debug: Link Structure Analysis ===");
    println!("{}\n", "=".repeat(70));

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;

    // Check document node properties
    println!("Sample document properties:");
    for (id, node) in context.nodes.iter().filter(|(_, n)| n.node_type == "document").take(3) {
        println!("\n  {}", id.as_str());
        for (k, v) in &node.properties {
            let v_str = match v {
                PropertyValue::String(s) => {
                    let preview = if s.len() > 50 { format!("{}...", &s[..50]) } else { s.clone() };
                    format!("String({})", preview)
                },
                PropertyValue::Int(i) => format!("Int({})", i),
                PropertyValue::Float(f) => format!("Float({})", f),
                PropertyValue::Bool(b) => format!("Bool({})", b),
                _ => "Other".to_string(),
            };
            println!("    {}: {}", k, v_str);
        }
    }

    // Count node types
    let mut node_type_counts: HashMap<String, usize> = HashMap::new();
    for (_, node) in context.nodes.iter() {
        *node_type_counts.entry(node.node_type.clone()).or_default() += 1;
    }
    println!("Node types:");
    for (t, c) in &node_type_counts {
        println!("  {}: {}", t, c);
    }

    // Count edge types
    let mut edge_type_counts: HashMap<String, usize> = HashMap::new();
    for edge in &context.edges {
        *edge_type_counts.entry(edge.relationship.clone()).or_default() += 1;
    }
    println!("\nEdge types:");
    for (t, c) in &edge_type_counts {
        println!("  {}: {}", t, c);
    }

    // Get link nodes
    let link_nodes: Vec<_> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "link")
        .collect();
    println!("\nLink nodes: {}", link_nodes.len());

    // Sample link nodes and their connections
    println!("\nSample link node connections (first 5):");
    for (link_id, link_node) in link_nodes.iter().take(5) {
        let target = link_node.properties.get("target")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("?");
        println!("\n  Link: {} -> target: {}", link_id.as_str(), target);

        // Find edges from/to this link
        let edges_from: Vec<_> = context.edges.iter()
            .filter(|e| e.source == **link_id)
            .collect();
        let edges_to: Vec<_> = context.edges.iter()
            .filter(|e| e.target == **link_id)
            .collect();

        println!("    Edges FROM link ({}):", edges_from.len());
        for e in &edges_from {
            let tgt_type = context.nodes.get(&e.target)
                .map(|n| n.node_type.as_str())
                .unwrap_or("MISSING");
            println!("      --{}-> {} ({})", e.relationship, e.target.as_str(), tgt_type);
        }

        println!("    Edges TO link ({}):", edges_to.len());
        for e in &edges_to {
            let src_type = context.nodes.get(&e.source)
                .map(|n| n.node_type.as_str())
                .unwrap_or("MISSING");
            println!("      {} ({}) --{}->", e.source.as_str(), src_type, e.relationship);
        }
    }

    // Count links_to edges and their targets
    let links_to_edges: Vec<_> = context.edges.iter()
        .filter(|e| e.relationship == "links_to")
        .collect();
    println!("\n\nlinks_to edges: {}", links_to_edges.len());

    let mut target_types: HashMap<String, usize> = HashMap::new();
    for e in &links_to_edges {
        let tgt_type = context.nodes.get(&e.target)
            .map(|n| n.node_type.clone())
            .unwrap_or_else(|| "MISSING".to_string());
        *target_types.entry(tgt_type).or_default() += 1;
    }
    println!("links_to target types:");
    for (t, c) in &target_types {
        println!("  {}: {}", t, c);
    }

    // Find links that point to documents
    let links_to_docs: Vec<_> = links_to_edges.iter()
        .filter(|e| {
            context.nodes.get(&e.target)
                .map(|n| n.node_type == "document")
                .unwrap_or(false)
        })
        .collect();
    println!("\nlinks_to edges pointing to documents: {}", links_to_docs.len());

    // For links pointing to docs, trace back to source doc
    println!("\nSample document-to-document paths via links (first 10):");
    let mut doc_pairs_found = 0;
    for e in links_to_docs.iter().take(20) {
        let link_id = &e.source;
        let target_doc_id = &e.target;

        // Find the document that contains this link
        let source_doc = context.edges.iter()
            .find(|ce| ce.target == *link_id && ce.relationship == "contains")
            .and_then(|ce| context.nodes.get(&ce.source))
            .filter(|n| n.node_type == "document");

        if let Some(src_doc) = source_doc {
            let src_path = src_doc.properties.get("source")
                .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
                .unwrap_or("?");
            let tgt_doc = context.nodes.get(target_doc_id).unwrap();
            let tgt_path = tgt_doc.properties.get("source")
                .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
                .unwrap_or("?");

            println!("  {} -> {}", src_path, tgt_path);
            doc_pairs_found += 1;
        }
    }
    println!("\nTotal doc-to-doc paths found: {}", doc_pairs_found);
}

#[tokio::test]
async fn test_link_semantic_correlation() {
    println!("\n{}", "=".repeat(70));
    println!("=== Spike 03: Link↔Semantic Correlation ===");
    println!("{}\n", "=".repeat(70));

    // Load corpus
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;

    // Find linked pairs
    let linked_pairs = find_linked_document_pairs(context);
    println!("Found {} linked document pairs", linked_pairs.len());

    if linked_pairs.is_empty() {
        println!("\nNO-GO: No linked document pairs found!");
        panic!("Cannot test link↔semantic correlation without linked pairs");
    }

    // Convert to set for exclusion
    let linked_set: HashSet<_> = linked_pairs.iter()
        .flat_map(|(a, b)| vec![
            format!("{}:{}", a.as_str(), b.as_str()),
            format!("{}:{}", b.as_str(), a.as_str())
        ])
        .collect();

    // Generate random pairs for baseline
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let random_pairs = generate_random_pairs(context, linked_pairs.len(), &linked_set, &mut rng);
    println!("Generated {} random (unlinked) pairs for baseline", random_pairs.len());

    // Compute concept overlap for linked pairs
    println!("\n--- Linked Pairs Analysis ---\n");
    let mut linked_similarities: Vec<f64> = Vec::new();
    let mut linked_details: Vec<(String, String, f64, usize)> = Vec::new();

    for (src_id, tgt_id) in &linked_pairs {
        let src_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, src_id);
        let tgt_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, tgt_id);
        let sim = jaccard_similarity(&src_concepts, &tgt_concepts);
        let shared: HashSet<_> = src_concepts.intersection(&tgt_concepts).cloned().collect();

        linked_similarities.push(sim);

        // Get document names for display
        let src_name = context.nodes.get(src_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| src_id.as_str().to_string());
        let tgt_name = context.nodes.get(tgt_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| tgt_id.as_str().to_string());

        linked_details.push((src_name, tgt_name, sim, shared.len()));
    }

    // Sort by similarity for display
    linked_details.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    println!("Top 10 linked pairs by similarity:");
    println!("{:<40} {:<40} {:>8} {:>6}", "Source", "Target", "Jaccard", "Shared");
    println!("{}", "-".repeat(100));
    for (src, tgt, sim, shared) in linked_details.iter().take(10) {
        let src_short = if src.len() > 38 { format!("...{}", &src[src.len()-35..]) } else { src.clone() };
        let tgt_short = if tgt.len() > 38 { format!("...{}", &tgt[tgt.len()-35..]) } else { tgt.clone() };
        println!("{:<40} {:<40} {:>8.3} {:>6}", src_short, tgt_short, sim, shared);
    }

    // Debug: show concepts for first few pairs
    println!("\n--- Concept Debug (first 5 pairs) ---\n");
    for (src_id, tgt_id) in linked_pairs.iter().take(5) {
        let src_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, src_id);
        let tgt_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, tgt_id);

        let src_name = context.nodes.get(src_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| src_id.as_str().to_string());
        let tgt_name = context.nodes.get(tgt_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| tgt_id.as_str().to_string());

        println!("  {} ({} concepts):", src_name, src_concepts.len());
        for c in src_concepts.iter().take(5) {
            println!("    - {}", c);
        }
        println!("  {} ({} concepts):", tgt_name, tgt_concepts.len());
        for c in tgt_concepts.iter().take(5) {
            println!("    - {}", c);
        }
        println!();
    }

    // Compute concept overlap for random pairs
    println!("\n--- Random Pairs Analysis ---\n");
    let mut random_similarities: Vec<f64> = Vec::new();

    for (src_id, tgt_id) in &random_pairs {
        let src_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, src_id);
        let tgt_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, tgt_id);
        let sim = jaccard_similarity(&src_concepts, &tgt_concepts);
        random_similarities.push(sim);
    }

    // Statistics
    let linked_mean: f64 = linked_similarities.iter().sum::<f64>() / linked_similarities.len() as f64;
    let random_mean: f64 = if random_similarities.is_empty() {
        0.0
    } else {
        random_similarities.iter().sum::<f64>() / random_similarities.len() as f64
    };

    let linked_nonzero = linked_similarities.iter().filter(|&&s| s > 0.0).count();
    let random_nonzero = random_similarities.iter().filter(|&&s| s > 0.0).count();

    println!("\n{}", "=".repeat(70));
    println!("=== Results Summary ===");
    println!("{}\n", "=".repeat(70));

    println!("{:<25} {:>15} {:>15}", "", "Linked Pairs", "Random Pairs");
    println!("{}", "-".repeat(55));
    println!("{:<25} {:>15} {:>15}", "Count", linked_pairs.len(), random_pairs.len());
    println!("{:<25} {:>15.4} {:>15.4}", "Mean Jaccard", linked_mean, random_mean);
    println!("{:<25} {:>15} {:>15}", "Non-zero similarity", linked_nonzero, random_nonzero);
    println!("{:<25} {:>14.1}% {:>14.1}%",
        "% with overlap",
        100.0 * linked_nonzero as f64 / linked_pairs.len() as f64,
        if random_pairs.is_empty() { 0.0 } else { 100.0 * random_nonzero as f64 / random_pairs.len() as f64 }
    );

    // Calculate improvement ratio
    let improvement = if random_mean > 0.0 {
        (linked_mean - random_mean) / random_mean * 100.0
    } else if linked_mean > 0.0 {
        100.0 // Any positive is infinite improvement over zero
    } else {
        0.0
    };

    println!("\n{:<25} {:>15.1}%", "Improvement over random", improvement);

    // Verdict
    println!("\n{}", "=".repeat(70));
    let verdict = if improvement >= 30.0 {
        "GO"
    } else if improvement >= 10.0 {
        "PIVOT"
    } else {
        "NO-GO"
    };

    println!("VERDICT: {} (improvement = {:.1}%, threshold: GO ≥30%, PIVOT ≥10%)",
        verdict, improvement);
    println!("{}", "=".repeat(70));

    // Additional analysis: What concepts are most commonly shared?
    println!("\n=== Shared Concept Analysis ===\n");
    let mut shared_concept_counts: HashMap<String, usize> = HashMap::new();

    for (src_id, tgt_id) in &linked_pairs {
        let src_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, src_id);
        let tgt_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, tgt_id);
        for concept in src_concepts.intersection(&tgt_concepts) {
            *shared_concept_counts.entry(concept.clone()).or_default() += 1;
        }
    }

    let mut sorted_concepts: Vec<_> = shared_concept_counts.iter().collect();
    sorted_concepts.sort_by(|a, b| b.1.cmp(a.1));

    println!("Most commonly shared concepts between linked documents:");
    for (concept, count) in sorted_concepts.iter().take(15) {
        println!("  {:>3}x  {}", count, concept);
    }

    // Soft assertion - test passes but logs verdict
    assert!(
        linked_pairs.len() > 0,
        "Need linked pairs for analysis"
    );
}

/// Analyze whether specific link types correlate with semantic similarity
#[tokio::test]
async fn test_link_type_breakdown() {
    println!("\n{}", "=".repeat(70));
    println!("=== Link Type Semantic Correlation ===");
    println!("{}\n", "=".repeat(70));

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;

    // Categorize linked pairs by inferred relationship type
    let linked_pairs = find_linked_document_pairs(context);

    let mut same_dir_pairs: Vec<(NodeId, NodeId, f64)> = Vec::new();
    let mut cross_dir_pairs: Vec<(NodeId, NodeId, f64)> = Vec::new();

    for (src_id, tgt_id) in &linked_pairs {
        let src_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, src_id);
        let tgt_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, tgt_id);
        let sim = jaccard_similarity(&src_concepts, &tgt_concepts);

        // Check if same directory
        let src_path = context.nodes.get(src_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_default();
        let tgt_path = context.nodes.get(tgt_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_default();

        let src_dir = std::path::Path::new(&src_path).parent().map(|p| p.to_string_lossy().to_string());
        let tgt_dir = std::path::Path::new(&tgt_path).parent().map(|p| p.to_string_lossy().to_string());

        if src_dir == tgt_dir {
            same_dir_pairs.push((src_id.clone(), tgt_id.clone(), sim));
        } else {
            cross_dir_pairs.push((src_id.clone(), tgt_id.clone(), sim));
        }
    }

    let same_dir_mean = if same_dir_pairs.is_empty() {
        0.0
    } else {
        same_dir_pairs.iter().map(|p| p.2).sum::<f64>() / same_dir_pairs.len() as f64
    };
    let cross_dir_mean = if cross_dir_pairs.is_empty() {
        0.0
    } else {
        cross_dir_pairs.iter().map(|p| p.2).sum::<f64>() / cross_dir_pairs.len() as f64
    };

    println!("{:<25} {:>15} {:>15}", "", "Same Dir", "Cross Dir");
    println!("{}", "-".repeat(55));
    println!("{:<25} {:>15} {:>15}", "Pair count", same_dir_pairs.len(), cross_dir_pairs.len());
    println!("{:<25} {:>15.4} {:>15.4}", "Mean Jaccard", same_dir_mean, cross_dir_mean);

    let same_nonzero = same_dir_pairs.iter().filter(|p| p.2 > 0.0).count();
    let cross_nonzero = cross_dir_pairs.iter().filter(|p| p.2 > 0.0).count();

    println!("{:<25} {:>14.1}% {:>14.1}%",
        "% with overlap",
        if same_dir_pairs.is_empty() { 0.0 } else { 100.0 * same_nonzero as f64 / same_dir_pairs.len() as f64 },
        if cross_dir_pairs.is_empty() { 0.0 } else { 100.0 * cross_nonzero as f64 / cross_dir_pairs.len() as f64 }
    );

    println!("\n=== Insight ===");
    if same_dir_mean > cross_dir_mean * 1.2 {
        println!("Same-directory links have higher semantic correlation.");
        println!("Consider: Weight intra-directory edges higher for propagation.");
    } else if cross_dir_mean > same_dir_mean * 1.2 {
        println!("Cross-directory links have higher semantic correlation.");
        println!("Consider: Cross-references may be more intentional/meaningful.");
    } else {
        println!("Similar correlation for same-dir and cross-dir links.");
        println!("Consider: Edge weighting can be uniform.");
    }
}

/// Test sibling document correlation (same directory, no explicit link)
#[tokio::test]
async fn test_sibling_semantic_correlation() {
    println!("\n{}", "=".repeat(70));
    println!("=== Sibling Document Semantic Correlation (Investigation 9 Preview) ===");
    println!("{}\n", "=".repeat(70));

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;

    // Get linked pairs to exclude
    let linked_pairs = find_linked_document_pairs(context);
    let linked_set: HashSet<_> = linked_pairs.iter()
        .flat_map(|(a, b)| vec![
            format!("{}:{}", a.as_str(), b.as_str()),
            format!("{}:{}", b.as_str(), a.as_str())
        ])
        .collect();

    // Group documents by directory
    let mut docs_by_dir: HashMap<String, Vec<NodeId>> = HashMap::new();
    for (id, node) in context.nodes.iter() {
        if node.node_type != "document" { continue; }

        let source = node.properties.get("source")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_default();

        let dir = std::path::Path::new(&source)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        docs_by_dir.entry(dir).or_default().push(id.clone());
    }

    // Generate sibling pairs (same dir, not linked)
    let mut sibling_pairs: Vec<(NodeId, NodeId)> = Vec::new();
    for docs in docs_by_dir.values() {
        if docs.len() < 2 { continue; }
        for i in 0..docs.len() {
            for j in (i+1)..docs.len() {
                let key1 = format!("{}:{}", docs[i].as_str(), docs[j].as_str());
                let key2 = format!("{}:{}", docs[j].as_str(), docs[i].as_str());
                if !linked_set.contains(&key1) && !linked_set.contains(&key2) {
                    sibling_pairs.push((docs[i].clone(), docs[j].clone()));
                }
            }
        }
    }

    println!("Found {} sibling pairs (same directory, not explicitly linked)", sibling_pairs.len());

    if sibling_pairs.is_empty() {
        println!("No sibling pairs found for analysis.");
        return;
    }

    // Compute similarities
    let mut sibling_sims: Vec<f64> = Vec::new();
    for (src_id, tgt_id) in &sibling_pairs {
        let src_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, src_id);
        let tgt_concepts = extract_concepts_from_doc_with_corpus(context, &corpus, tgt_id);
        sibling_sims.push(jaccard_similarity(&src_concepts, &tgt_concepts));
    }

    // Compare to linked pairs and random
    let linked_sims: Vec<f64> = linked_pairs.iter()
        .map(|(src, tgt)| {
            jaccard_similarity(
                &extract_concepts_from_doc_with_corpus(context, &corpus, src),
                &extract_concepts_from_doc_with_corpus(context, &corpus, tgt),
            )
        })
        .collect();

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let random_pairs = generate_random_pairs(context, sibling_pairs.len(), &linked_set, &mut rng);
    let random_sims: Vec<f64> = random_pairs.iter()
        .map(|(src, tgt)| {
            jaccard_similarity(
                &extract_concepts_from_doc_with_corpus(context, &corpus, src),
                &extract_concepts_from_doc_with_corpus(context, &corpus, tgt),
            )
        })
        .collect();

    let sibling_mean = sibling_sims.iter().sum::<f64>() / sibling_sims.len() as f64;
    let linked_mean = if linked_sims.is_empty() { 0.0 } else { linked_sims.iter().sum::<f64>() / linked_sims.len() as f64 };
    let random_mean = if random_sims.is_empty() { 0.0 } else { random_sims.iter().sum::<f64>() / random_sims.len() as f64 };

    println!("\n{:<20} {:>12} {:>12} {:>12}", "", "Siblings", "Linked", "Random");
    println!("{}", "-".repeat(60));
    println!("{:<20} {:>12} {:>12} {:>12}", "Count", sibling_pairs.len(), linked_pairs.len(), random_pairs.len());
    println!("{:<20} {:>12.4} {:>12.4} {:>12.4}", "Mean Jaccard", sibling_mean, linked_mean, random_mean);

    let sibling_nonzero = sibling_sims.iter().filter(|&&s| s > 0.0).count();
    let linked_nonzero = linked_sims.iter().filter(|&&s| s > 0.0).count();
    let random_nonzero = random_sims.iter().filter(|&&s| s > 0.0).count();

    println!("{:<20} {:>11.1}% {:>11.1}% {:>11.1}%",
        "% with overlap",
        100.0 * sibling_nonzero as f64 / sibling_pairs.len() as f64,
        if linked_pairs.is_empty() { 0.0 } else { 100.0 * linked_nonzero as f64 / linked_pairs.len() as f64 },
        if random_pairs.is_empty() { 0.0 } else { 100.0 * random_nonzero as f64 / random_pairs.len() as f64 }
    );

    println!("\n=== Implications for Sibling Edges ===");
    if sibling_mean > random_mean * 1.1 {
        println!("Siblings have higher semantic correlation than random pairs.");
        println!("The sibling edges added in Investigation 1 are justified.");
    } else {
        println!("Sibling correlation is similar to random baseline.");
        println!("Sibling edges may add noise rather than signal.");
    }
}
