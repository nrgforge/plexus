//! Spike Investigation 02c: Semantic Quality Deep Dive
//!
//! Analyzes the actual semantic content of top documents by each strategy.
//! Questions:
//! - What concepts would we extract from each strategy's picks?
//! - Why is HITS zeroing out on the directed graph?
//! - Which strategy finds the most semantically valuable seeds?

mod common;

use common::{build_structure_graph, pagerank, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use std::collections::{HashMap, HashSet, VecDeque};

const TOP_K: usize = 8;

/// Analyze the directed graph structure to understand HITS behavior
fn analyze_directed_graph(context: &Context) {
    println!("\n=== Directed Graph Analysis ===\n");

    // Count edges by type
    let mut edge_counts: HashMap<&str, usize> = HashMap::new();
    for edge in &context.edges {
        *edge_counts.entry(&edge.relationship).or_default() += 1;
    }

    println!("All edge types:");
    for (rel, count) in &edge_counts {
        println!("  {}: {}", rel, count);
    }

    // Directed edges only (original, not synthetic)
    let directed_rels = ["links_to", "references", "contains", "follows"];
    let directed_edges: Vec<_> = context.edges.iter()
        .filter(|e| directed_rels.contains(&e.relationship.as_str()))
        .collect();

    println!("\nDirected edges only: {}", directed_edges.len());

    // Filter to document-to-document links only
    let doc_to_doc: Vec<_> = directed_edges.iter()
        .filter(|e| {
            let src_is_doc = context.nodes.get(&e.source)
                .map(|n| n.node_type == "document")
                .unwrap_or(false);
            let tgt_is_doc = context.nodes.get(&e.target)
                .map(|n| n.node_type == "document")
                .unwrap_or(false);
            src_is_doc && tgt_is_doc && e.relationship == "links_to"
        })
        .collect();

    println!("Document-to-document links_to edges: {}", doc_to_doc.len());

    // Build adjacency for document nodes only
    let doc_ids: Vec<_> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id)
        .collect();

    let mut out_degree: HashMap<&NodeId, usize> = HashMap::new();
    let mut in_degree: HashMap<&NodeId, usize> = HashMap::new();

    for id in &doc_ids {
        out_degree.insert(*id, 0);
        in_degree.insert(*id, 0);
    }

    for edge in &doc_to_doc {
        if let Some(count) = out_degree.get_mut(&edge.source) {
            *count += 1;
        }
        if let Some(count) = in_degree.get_mut(&edge.target) {
            *count += 1;
        }
    }

    // Distribution analysis
    let docs_with_outlinks: usize = out_degree.values().filter(|&&d| d > 0).count();
    let docs_with_inlinks: usize = in_degree.values().filter(|&&d| d > 0).count();

    println!("\nDocument link distribution:");
    println!("  Documents with outgoing links: {}/{}", docs_with_outlinks, doc_ids.len());
    println!("  Documents with incoming links: {}/{}", docs_with_inlinks, doc_ids.len());

    // Top outlinkers (hubs)
    let mut out_sorted: Vec<_> = out_degree.iter().collect();
    out_sorted.sort_by(|a, b| b.1.cmp(a.1));

    println!("\nTop 5 by outgoing links (potential hubs):");
    for (id, count) in out_sorted.iter().take(5) {
        let source = context.nodes.get(*id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or(id.as_str());
        println!("  {} -> {} outlinks", source, count);
    }

    // Top inlinked (authorities)
    let mut in_sorted: Vec<_> = in_degree.iter().collect();
    in_sorted.sort_by(|a, b| b.1.cmp(a.1));

    println!("\nTop 5 by incoming links (potential authorities):");
    for (id, count) in in_sorted.iter().take(5) {
        let source = context.nodes.get(*id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or(id.as_str());
        println!("  {} <- {} inlinks", source, count);
    }

    // Check connectivity in directed doc graph
    println!("\n--- Directed Document Graph Connectivity ---");

    // Build directed adjacency
    let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    for edge in &doc_to_doc {
        adj.entry(&edge.source).or_default().push(&edge.target);
    }

    // Find strongly connected components (simplified - just check if docs are reachable)
    let mut reachable_from_any: HashSet<&NodeId> = HashSet::new();
    for start in &doc_ids {
        if out_degree.get(start).copied().unwrap_or(0) > 0 {
            // BFS from this node
            let mut visited: HashSet<&NodeId> = HashSet::new();
            let mut queue: VecDeque<&NodeId> = VecDeque::new();
            queue.push_back(*start);
            visited.insert(*start);

            while let Some(node) = queue.pop_front() {
                if let Some(neighbors) = adj.get(node) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.insert(*neighbor);
                            queue.push_back(*neighbor);
                        }
                    }
                }
            }
            reachable_from_any.extend(visited);
        }
    }

    println!("Documents reachable via directed links: {}/{}", reachable_from_any.len(), doc_ids.len());

    // Isolated documents (no in or out links)
    let isolated: Vec<_> = doc_ids.iter()
        .filter(|id| {
            out_degree.get(*id).copied().unwrap_or(0) == 0 &&
            in_degree.get(*id).copied().unwrap_or(0) == 0
        })
        .collect();

    println!("Isolated documents (no doc-to-doc links): {}", isolated.len());
}

/// Extract mock concepts from document content (simulates what LLM would find)
fn extract_mock_concepts(content: &str) -> Vec<String> {
    let mut concepts = Vec::new();

    // Extract from headings
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim();
            if !heading.is_empty() && heading.len() > 2 {
                concepts.push(heading.to_string());
            }
        }
    }

    // Extract code language indicators
    for line in content.lines() {
        if line.trim().starts_with("```") {
            let lang = line.trim().trim_start_matches('`').trim();
            if !lang.is_empty() && lang.len() < 20 {
                concepts.push(format!("code:{}", lang));
            }
        }
    }

    // Extract wikilinks as concept references
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek() == Some(&'[') {
            chars.next();
            let mut link = String::new();
            while let Some(c) = chars.next() {
                if c == ']' && chars.peek() == Some(&']') {
                    chars.next();
                    break;
                }
                if c != '|' {
                    link.push(c);
                } else {
                    break;
                }
            }
            if !link.is_empty() {
                concepts.push(format!("ref:{}", link.trim()));
            }
        }
    }

    // Extract emphasized terms (bold) - likely important concepts
    // Simple manual parsing for **bold** text
    let mut in_bold = false;
    let mut bold_text = String::new();
    let mut prev_char = ' ';

    for c in content.chars() {
        if c == '*' && prev_char == '*' && !in_bold {
            in_bold = true;
            bold_text.clear();
        } else if c == '*' && prev_char == '*' && in_bold {
            in_bold = false;
            let t = bold_text.trim();
            if t.len() > 2 && t.len() < 50 && !t.contains('\n') {
                // Remove the trailing * from bold_text
                let clean = t.trim_end_matches('*').trim();
                if !clean.is_empty() {
                    concepts.push(format!("term:{}", clean));
                }
            }
        } else if in_bold && c != '*' {
            bold_text.push(c);
        }
        prev_char = c;
    }

    concepts
}

/// Analyze semantic value of a document
fn analyze_document_semantics(corpus: &TestCorpus, source: &str) -> DocumentSemantics {
    let content = corpus.items.iter()
        .find(|item| item.id.as_str() == source)
        .map(|item| item.content.as_str())
        .unwrap_or("");

    let word_count = content.split_whitespace().count();
    let concepts = extract_mock_concepts(content);

    // Count different concept types
    let heading_count = concepts.iter()
        .filter(|c| !c.starts_with("code:") && !c.starts_with("ref:") && !c.starts_with("term:"))
        .count();
    let code_count = concepts.iter().filter(|c| c.starts_with("code:")).count();
    let ref_count = concepts.iter().filter(|c| c.starts_with("ref:")).count();
    let term_count = concepts.iter().filter(|c| c.starts_with("term:")).count();

    // Calculate semantic density (concepts per 100 words)
    let density = if word_count > 0 {
        (concepts.len() as f64 / word_count as f64) * 100.0
    } else {
        0.0
    };

    // Heuristic quality score
    let quality_score =
        heading_count as f64 * 2.0 +  // Headings indicate structure
        code_count as f64 * 3.0 +      // Code indicates practical content
        term_count as f64 * 2.0 +      // Bold terms indicate key concepts
        (word_count as f64 / 50.0).min(5.0);    // Some credit for length, capped

    DocumentSemantics {
        source: source.to_string(),
        word_count,
        concepts,
        heading_count,
        code_count,
        ref_count,
        term_count,
        density,
        quality_score,
    }
}

struct DocumentSemantics {
    source: String,
    word_count: usize,
    concepts: Vec<String>,
    heading_count: usize,
    code_count: usize,
    ref_count: usize,
    term_count: usize,
    density: f64,
    quality_score: f64,
}

#[tokio::test]
async fn test_semantic_quality_analysis() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");

    println!("\n{}", "=".repeat(80));
    println!("=== Spike 02c: Semantic Quality Deep Dive ===");
    println!("{}\n", "=".repeat(80));

    // First, analyze the directed graph structure
    analyze_directed_graph(&graph.context);

    // Get document nodes
    let doc_nodes: Vec<_> = graph.context.nodes.iter()
        .filter(|(_, node)| node.node_type == "document")
        .collect();

    // Calculate PageRank
    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);

    // Get word counts
    let get_source = |id: &NodeId| -> String {
        graph.context.nodes.get(id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| id.to_string())
    };

    let get_word_count = |source: &str| -> usize {
        corpus.items.iter()
            .find(|item| item.id.as_str() == source)
            .map(|item| item.content.split_whitespace().count())
            .unwrap_or(0)
    };

    // Strategy 1: Pure PageRank
    let mut pr_docs: Vec<_> = doc_nodes.iter()
        .map(|(id, _)| {
            let source = get_source(id);
            let score = *pr.scores.get(*id).unwrap_or(&0.0);
            ((*id).clone(), source, score)
        })
        .collect();
    pr_docs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // Strategy 2: PageRank filtered by word count
    let mut pr_filtered: Vec<_> = pr_docs.iter()
        .filter(|(_, source, _)| get_word_count(source) > 100)
        .cloned()
        .collect();
    pr_filtered.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    println!("\n{}", "=".repeat(80));
    println!("=== SEMANTIC ANALYSIS: PageRank Top 8 ===");
    println!("{}\n", "=".repeat(80));

    let mut pr_total_quality = 0.0;
    for (i, (_, source, _)) in pr_docs.iter().take(TOP_K).enumerate() {
        let sem = analyze_document_semantics(&corpus, source);
        pr_total_quality += sem.quality_score;

        println!("{}. {} ({} words)", i + 1, truncate(source, 50), sem.word_count);
        println!("   Headings: {}, Code blocks: {}, Refs: {}, Terms: {}",
            sem.heading_count, sem.code_count, sem.ref_count, sem.term_count);
        println!("   Density: {:.1} concepts/100 words, Quality: {:.1}", sem.density, sem.quality_score);

        if !sem.concepts.is_empty() {
            let preview: Vec<_> = sem.concepts.iter().take(5).map(|s| s.as_str()).collect();
            println!("   Sample concepts: {:?}", preview);
        }
        println!();
    }
    println!("PageRank Total Quality Score: {:.1}", pr_total_quality);

    println!("\n{}", "=".repeat(80));
    println!("=== SEMANTIC ANALYSIS: PageRank + Filter Top 8 ===");
    println!("{}\n", "=".repeat(80));

    let mut filtered_total_quality = 0.0;
    for (i, (_, source, _)) in pr_filtered.iter().take(TOP_K).enumerate() {
        let sem = analyze_document_semantics(&corpus, source);
        filtered_total_quality += sem.quality_score;

        println!("{}. {} ({} words)", i + 1, truncate(source, 50), sem.word_count);
        println!("   Headings: {}, Code blocks: {}, Refs: {}, Terms: {}",
            sem.heading_count, sem.code_count, sem.ref_count, sem.term_count);
        println!("   Density: {:.1} concepts/100 words, Quality: {:.1}", sem.density, sem.quality_score);

        if !sem.concepts.is_empty() {
            let preview: Vec<_> = sem.concepts.iter().take(5).map(|s| s.as_str()).collect();
            println!("   Sample concepts: {:?}", preview);
        }
        println!();
    }
    println!("Filtered Total Quality Score: {:.1}", filtered_total_quality);

    // Compare by looking at what concepts would be extracted
    println!("\n{}", "=".repeat(80));
    println!("=== CONCEPT EXTRACTION COMPARISON ===");
    println!("{}\n", "=".repeat(80));

    println!("PageRank seeds would extract concepts like:");
    let mut pr_all_concepts: Vec<String> = Vec::new();
    for (_, source, _) in pr_docs.iter().take(TOP_K) {
        let sem = analyze_document_semantics(&corpus, source);
        pr_all_concepts.extend(sem.concepts);
    }
    // Dedupe and show
    let pr_unique: HashSet<_> = pr_all_concepts.iter().collect();
    let mut pr_sorted: Vec<_> = pr_unique.into_iter().collect();
    pr_sorted.sort();
    for concept in pr_sorted.iter().take(20) {
        println!("  - {}", concept);
    }
    println!("  ... ({} unique concepts total)", pr_sorted.len());

    println!("\nFiltered PageRank seeds would extract concepts like:");
    let mut filtered_all_concepts: Vec<String> = Vec::new();
    for (_, source, _) in pr_filtered.iter().take(TOP_K) {
        let sem = analyze_document_semantics(&corpus, source);
        filtered_all_concepts.extend(sem.concepts);
    }
    let filtered_unique: HashSet<_> = filtered_all_concepts.iter().collect();
    let mut filtered_sorted: Vec<_> = filtered_unique.into_iter().collect();
    filtered_sorted.sort();
    for concept in filtered_sorted.iter().take(20) {
        println!("  - {}", concept);
    }
    println!("  ... ({} unique concepts total)", filtered_sorted.len());

    // Overlap analysis
    let pr_set: HashSet<_> = pr_sorted.iter().cloned().collect();
    let filtered_set: HashSet<_> = filtered_sorted.iter().cloned().collect();
    let overlap: HashSet<_> = pr_set.intersection(&filtered_set).collect();
    let pr_only: HashSet<_> = pr_set.difference(&filtered_set).collect();
    let filtered_only: HashSet<_> = filtered_set.difference(&pr_set).collect();

    println!("\n--- Concept Overlap ---");
    println!("Shared concepts: {}", overlap.len());
    println!("PageRank-only concepts: {}", pr_only.len());
    println!("Filtered-only concepts: {}", filtered_only.len());

    if !filtered_only.is_empty() {
        println!("\nConcepts gained by filtering (sample):");
        for concept in filtered_only.iter().take(10) {
            println!("  + {}", concept);
        }
    }

    if !pr_only.is_empty() {
        println!("\nConcepts lost by filtering (sample):");
        for concept in pr_only.iter().take(10) {
            println!("  - {}", concept);
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("=== SUMMARY ===");
    println!("{}\n", "=".repeat(80));

    println!("Quality Score Comparison:");
    println!("  PageRank:          {:.1}", pr_total_quality);
    println!("  PageRank+Filter:   {:.1}", filtered_total_quality);
    println!("  Improvement:       {:.1}%", (filtered_total_quality - pr_total_quality) / pr_total_quality * 100.0);

    println!("\nUnique Concepts Extracted:");
    println!("  PageRank:          {}", pr_sorted.len());
    println!("  PageRank+Filter:   {}", filtered_sorted.len());

    let quality_per_concept_pr = pr_total_quality / pr_sorted.len() as f64;
    let quality_per_concept_filtered = filtered_total_quality / filtered_sorted.len() as f64;

    println!("\nQuality per Concept:");
    println!("  PageRank:          {:.2}", quality_per_concept_pr);
    println!("  PageRank+Filter:   {:.2}", quality_per_concept_filtered);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - (max_len - 3)..])
    }
}
