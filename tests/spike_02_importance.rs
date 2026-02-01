//! Spike Investigation 02: Importance Scoring Quality
//!
//! Validates that PageRank identifies semantically rich documents,
//! not just heavily-linked index pages.
//!
//! ## Hypothesis
//! H2 (Importance Scoring): PageRank identifies content-rich documents
//! suitable for concept extraction and propagation seeding.
//!
//! ## Go/No-Go Criteria
//! - **GO**: ≥6/8 top documents are Content-Rich or Mixed
//! - **PIVOT**: 4-5/8 Content-Rich → Consider HITS-authority or hybrid
//! - **NO-GO**: <4/8 Content-Rich → PageRank unsuitable for this corpus

mod common;

use common::{build_structure_graph, pagerank, TestCorpus};
use plexus::PropertyValue;
use std::collections::HashMap;

/// Number of top documents to analyze
const TOP_K: usize = 8;

#[tokio::test]
#[ignore]
async fn test_importance_pkm_webdev() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load pkm-webdev corpus");

    println!("\n=== Spike 02: Importance Scoring Quality ===");
    println!("Corpus: pkm-webdev");
    println!("Files: {}", corpus.file_count);

    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    println!("Nodes: {}", graph.node_count());
    println!("Edges: {}", graph.edge_count());

    // Calculate PageRank
    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);
    println!("PageRank converged in {} iterations", pr.iterations);

    // Get top K documents only (filter by node type)
    let doc_nodes: Vec<_> = graph
        .context
        .nodes
        .iter()
        .filter(|(_, node)| node.node_type == "document")
        .collect();

    println!("\nTotal documents: {}", doc_nodes.len());

    // Sort documents by PageRank score
    let mut doc_scores: Vec<_> = doc_nodes
        .iter()
        .filter_map(|(id, _)| pr.scores.get(*id).map(|score| (id, *score)))
        .collect();
    doc_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Count outgoing links per document
    let mut outgoing_links: HashMap<&str, usize> = HashMap::new();
    for edge in &graph.context.edges {
        if edge.relationship == "links_to" || edge.relationship == "references" {
            *outgoing_links.entry(edge.source.as_str()).or_default() += 1;
        }
    }

    // Count sections (headings) per document
    let mut section_counts: HashMap<&str, usize> = HashMap::new();
    for edge in &graph.context.edges {
        if edge.relationship == "contains" {
            if let Some(target_node) = graph.context.nodes.get(&edge.target) {
                if target_node.node_type == "heading" {
                    *section_counts.entry(edge.source.as_str()).or_default() += 1;
                }
            }
        }
    }

    println!("\n=== Top {} Documents by PageRank ===\n", TOP_K);
    println!(
        "{:<4} {:<40} {:>10} {:>8} {:>8} {:>10}",
        "Rank", "Document", "PR Score", "Links", "Sections", "Words"
    );
    println!("{}", "-".repeat(85));

    for (rank, (id, score)) in doc_scores.iter().take(TOP_K).enumerate() {
        let node = graph.context.nodes.get(*id).unwrap();

        // Get source path
        let source = match node.properties.get("source") {
            Some(PropertyValue::String(s)) => s.as_str(),
            _ => id.as_str(),
        };

        // Get word count from corpus
        let word_count = corpus
            .items
            .iter()
            .find(|item| item.id.as_str() == source)
            .map(|item| item.content.split_whitespace().count())
            .unwrap_or(0);

        let links = outgoing_links.get(id.as_str()).copied().unwrap_or(0);
        let sections = section_counts.get(id.as_str()).copied().unwrap_or(0);

        // Truncate source for display
        let display_source = if source.len() > 38 {
            format!("...{}", &source[source.len() - 35..])
        } else {
            source.to_string()
        };

        println!(
            "{:<4} {:<40} {:>10.6} {:>8} {:>8} {:>10}",
            rank + 1,
            display_source,
            score,
            links,
            sections,
            word_count
        );
    }

    // Print content previews for manual assessment
    println!("\n=== Content Previews ===\n");

    for (rank, (id, score)) in doc_scores.iter().take(TOP_K).enumerate() {
        let node = graph.context.nodes.get(*id).unwrap();

        let source = match node.properties.get("source") {
            Some(PropertyValue::String(s)) => s.as_str(),
            _ => id.as_str(),
        };

        // Find content from corpus
        if let Some(item) = corpus.items.iter().find(|item| item.id.as_str() == source) {
            println!("--- {} ({}) ---", rank + 1, source);

            // Show first 500 chars of content
            let preview: String = item
                .content
                .chars()
                .take(500)
                .collect::<String>()
                .replace('\n', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            println!("{}", preview);
            if item.content.len() > 500 {
                println!("...[truncated, {} total chars]", item.content.len());
            }
            println!();
        }
    }

    // Automated heuristics for classification
    println!("\n=== Automated Classification Heuristics ===\n");
    println!(
        "{:<4} {:<40} {:>12} {:>10}",
        "Rank", "Document", "Classification", "Confidence"
    );
    println!("{}", "-".repeat(70));

    let mut content_rich_count = 0;
    let mut mixed_count = 0;
    let mut index_count = 0;

    for (rank, (id, _)) in doc_scores.iter().take(TOP_K).enumerate() {
        let node = graph.context.nodes.get(*id).unwrap();

        let source = match node.properties.get("source") {
            Some(PropertyValue::String(s)) => s.as_str(),
            _ => id.as_str(),
        };

        let word_count = corpus
            .items
            .iter()
            .find(|item| item.id.as_str() == source)
            .map(|item| item.content.split_whitespace().count())
            .unwrap_or(0);

        let links = outgoing_links.get(id.as_str()).copied().unwrap_or(0);
        let sections = section_counts.get(id.as_str()).copied().unwrap_or(0);

        // Heuristic classification:
        // - Index/MOC: High links, low words per link, often has "index", "overview", "toc" in name
        // - Content-Rich: High words, moderate links, has sections
        // - Mixed: Somewhere in between

        let words_per_link = if links > 0 {
            word_count as f64 / links as f64
        } else {
            word_count as f64
        };

        let is_index_name = source.to_lowercase().contains("index")
            || source.to_lowercase().contains("readme")
            || source.to_lowercase().contains("overview")
            || source.to_lowercase().contains("toc")
            || source.to_lowercase().contains("contents");

        let (classification, confidence) = if is_index_name && links > 5 {
            index_count += 1;
            ("Index/MOC", "High")
        } else if word_count > 500 && sections >= 2 && words_per_link > 50.0 {
            content_rich_count += 1;
            ("Content-Rich", "High")
        } else if word_count > 300 && (sections >= 1 || words_per_link > 30.0) {
            content_rich_count += 1;
            ("Content-Rich", "Medium")
        } else if word_count > 200 || sections >= 1 {
            mixed_count += 1;
            ("Mixed", "Medium")
        } else if links > word_count / 50 {
            index_count += 1;
            ("Index/MOC", "Medium")
        } else {
            mixed_count += 1;
            ("Mixed", "Low")
        };

        let display_source = if source.len() > 38 {
            format!("...{}", &source[source.len() - 35..])
        } else {
            source.to_string()
        };

        println!(
            "{:<4} {:<40} {:>12} {:>10}",
            rank + 1,
            display_source,
            classification,
            confidence
        );
    }

    // Summary
    println!("\n=== Summary ===\n");
    println!("Content-Rich: {}", content_rich_count);
    println!("Mixed: {}", mixed_count);
    println!("Index/MOC: {}", index_count);

    let passing = content_rich_count + mixed_count;
    println!(
        "\nContent-Rich + Mixed: {}/{} ({:.0}%)",
        passing,
        TOP_K,
        (passing as f64 / TOP_K as f64) * 100.0
    );

    // Evaluate result
    if passing >= 6 {
        println!("\nResult: GO ✓ (≥6/8 Content-Rich or Mixed)");
        println!("  → PageRank is suitable for identifying seed documents");
    } else if passing >= 4 {
        println!("\nResult: PIVOT ⚠ (4-5/8 Content-Rich)");
        println!("  → Consider HITS-authority scoring or hybrid approach");
    } else {
        println!("\nResult: NO-GO ✗ (<4/8 Content-Rich)");
        println!("  → PageRank unsuitable; need different seed selection strategy");
    }

    println!("\n⚠️  Note: Automated heuristics provide guidance but manual review recommended");
    println!("    Check content previews above to verify classifications");
}

#[tokio::test]
#[ignore]
async fn test_importance_comparison_pagerank_vs_hits() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load pkm-webdev corpus");

    println!("\n=== PageRank vs HITS Comparison ===");

    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    // Get document nodes only
    let doc_ids: Vec<_> = graph
        .context
        .nodes
        .iter()
        .filter(|(_, node)| node.node_type == "document")
        .map(|(id, _)| id.clone())
        .collect();

    // Calculate PageRank
    let pr = pagerank(&graph.context, 0.85, 100, 1e-6);

    // Calculate HITS
    let hits_result = common::hits(&graph.context, 100);

    // Get top 8 by each metric
    let mut pr_docs: Vec<_> = doc_ids
        .iter()
        .filter_map(|id| pr.scores.get(id).map(|s| (id.clone(), *s)))
        .collect();
    pr_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut hits_auth_docs: Vec<_> = doc_ids
        .iter()
        .filter_map(|id| hits_result.authority_scores.get(id).map(|s| (id.clone(), *s)))
        .collect();
    hits_auth_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut hits_hub_docs: Vec<_> = doc_ids
        .iter()
        .filter_map(|id| hits_result.hub_scores.get(id).map(|s| (id.clone(), *s)))
        .collect();
    hits_hub_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("\n{:<4} {:<30} {:<30} {:<30}", "Rank", "PageRank", "HITS Authority", "HITS Hub");
    println!("{}", "-".repeat(100));

    for i in 0..TOP_K {
        let pr_doc = pr_docs.get(i).map(|(id, _)| {
            let node = graph.context.nodes.get(id).unwrap();
            match node.properties.get("source") {
                Some(PropertyValue::String(s)) => truncate_path(s, 28),
                _ => truncate_path(id.as_str(), 28),
            }
        }).unwrap_or_default();

        let auth_doc = hits_auth_docs.get(i).map(|(id, _)| {
            let node = graph.context.nodes.get(id).unwrap();
            match node.properties.get("source") {
                Some(PropertyValue::String(s)) => truncate_path(s, 28),
                _ => truncate_path(id.as_str(), 28),
            }
        }).unwrap_or_default();

        let hub_doc = hits_hub_docs.get(i).map(|(id, _)| {
            let node = graph.context.nodes.get(id).unwrap();
            match node.properties.get("source") {
                Some(PropertyValue::String(s)) => truncate_path(s, 28),
                _ => truncate_path(id.as_str(), 28),
            }
        }).unwrap_or_default();

        println!("{:<4} {:<30} {:<30} {:<30}", i + 1, pr_doc, auth_doc, hub_doc);
    }

    // Calculate overlap
    let pr_top: std::collections::HashSet<_> = pr_docs.iter().take(TOP_K).map(|(id, _)| id).collect();
    let auth_top: std::collections::HashSet<_> = hits_auth_docs.iter().take(TOP_K).map(|(id, _)| id).collect();
    let hub_top: std::collections::HashSet<_> = hits_hub_docs.iter().take(TOP_K).map(|(id, _)| id).collect();

    let pr_auth_overlap = pr_top.intersection(&auth_top).count();
    let pr_hub_overlap = pr_top.intersection(&hub_top).count();
    let auth_hub_overlap = auth_top.intersection(&hub_top).count();

    println!("\n=== Overlap Analysis ===");
    println!("PageRank ∩ HITS-Authority: {}/{}", pr_auth_overlap, TOP_K);
    println!("PageRank ∩ HITS-Hub: {}/{}", pr_hub_overlap, TOP_K);
    println!("HITS-Authority ∩ HITS-Hub: {}/{}", auth_hub_overlap, TOP_K);

    println!("\n=== Interpretation ===");
    println!("- High PR∩Auth overlap: PageRank finds authoritative content");
    println!("- High PR∩Hub overlap: PageRank finds index/navigation pages");
    println!("- Low Auth∩Hub overlap: Good separation of content vs navigation");
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    }
}
