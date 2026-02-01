//! Spike Investigation 08: Document Hierarchy Structure
//!
//! Validates that documents have sufficient heading structure
//! to support section-level analysis.
//!
//! ## Hypothesis
//! H8 (Heading Structure): Most documents have heading-based structure
//! that can be exploited for section-level analysis.
//!
//! ## Go/No-Go Criteria
//! - **GO**: ≥80% of documents have at least one heading
//! - **PIVOT**: 50-80% → Mixed corpus, may need fallback strategy
//! - **NO-GO**: <50% → Corpus lacks structure, document-level only

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::PropertyValue;

/// Target percentage of documents with headings
const TARGET_WITH_HEADINGS: f64 = 0.80;

/// Pivot threshold
const PIVOT_THRESHOLD: f64 = 0.50;

/// Statistics for hierarchy analysis
#[derive(Debug)]
struct HierarchyStats {
    /// Total documents
    total_docs: usize,
    /// Documents with at least one heading
    docs_with_headings: usize,
    /// Total heading nodes
    total_headings: usize,
    /// Total section nodes
    total_sections: usize,
    /// Average headings per document (for docs with headings)
    avg_headings: f64,
    /// Distribution of heading levels
    heading_levels: [usize; 6], // H1-H6
    /// Documents with proper hierarchy (H1 before H2, etc.)
    well_structured: usize,
}

impl HierarchyStats {
    fn percentage_with_headings(&self) -> f64 {
        if self.total_docs == 0 {
            0.0
        } else {
            self.docs_with_headings as f64 / self.total_docs as f64
        }
    }

    fn print_summary(&self, corpus_name: &str) {
        println!("\n=== Spike 08: Document Hierarchy Analysis ===");
        println!("Corpus: {}", corpus_name);
        println!("Total documents: {}", self.total_docs);
        println!(
            "Documents with headings: {} ({:.1}%)",
            self.docs_with_headings,
            self.percentage_with_headings() * 100.0
        );
        println!("Total heading nodes: {}", self.total_headings);
        println!("Total section nodes: {}", self.total_sections);

        if self.docs_with_headings > 0 {
            println!(
                "Avg headings/doc (with headings): {:.1}",
                self.avg_headings
            );
        }

        println!("\nHeading Level Distribution:");
        for (i, count) in self.heading_levels.iter().enumerate() {
            if *count > 0 {
                let bar_len = (*count as f64 / self.total_headings.max(1) as f64 * 30.0) as usize;
                let bar = "█".repeat(bar_len);
                println!("  H{}: {:>4} {}", i + 1, count, bar);
            }
        }

        let pct = self.percentage_with_headings();
        if pct >= TARGET_WITH_HEADINGS {
            println!(
                "\nResult: GO ✓ (≥{:.0}% have headings)",
                TARGET_WITH_HEADINGS * 100.0
            );
            println!("  → Section-level analysis is well-supported");
        } else if pct >= PIVOT_THRESHOLD {
            println!(
                "\nResult: PIVOT ⚠ ({:.0}%-{:.0}%)",
                PIVOT_THRESHOLD * 100.0,
                TARGET_WITH_HEADINGS * 100.0
            );
            println!("  → Mixed corpus, consider fallback for flat documents");
        } else {
            println!(
                "\nResult: NO-GO ✗ (<{:.0}% have headings)",
                PIVOT_THRESHOLD * 100.0
            );
            println!("  → Corpus lacks structure, document-level analysis recommended");
        }
    }
}

/// Analyze hierarchy structure from graph
fn analyze_hierarchy(graph: &common::BuiltGraph) -> HierarchyStats {
    let documents = graph.nodes_by_type("document");
    let headings = graph.nodes_by_type("heading");
    let sections = graph.nodes_by_type("section");

    // Get edges that connect headings to documents
    let parent_edges = graph.edges_by_relationship("child_of");
    let contains_edges = graph.edges_by_relationship("contains");

    // Count documents with at least one heading
    let mut docs_with_headings = 0;
    let mut heading_counts: Vec<usize> = Vec::new();

    for doc in &documents {
        // Count headings that belong to this document
        let doc_headings: usize = headings
            .iter()
            .filter(|h| {
                // Check if heading is connected to document via parent edges
                parent_edges
                    .iter()
                    .any(|e| e.source == h.id && e.target == doc.id)
                    || contains_edges
                        .iter()
                        .any(|e| e.source == doc.id && e.target == h.id)
            })
            .count();

        if doc_headings > 0 {
            docs_with_headings += 1;
            heading_counts.push(doc_headings);
        }
    }

    // Analyze heading levels
    let mut heading_levels = [0usize; 6];
    for heading in &headings {
        if let Some(PropertyValue::Int(level)) = heading.properties.get("level") {
            let idx = (*level as usize).saturating_sub(1).min(5);
            heading_levels[idx] += 1;
        }
    }

    let avg_headings = if !heading_counts.is_empty() {
        heading_counts.iter().sum::<usize>() as f64 / heading_counts.len() as f64
    } else {
        0.0
    };

    HierarchyStats {
        total_docs: documents.len(),
        docs_with_headings,
        total_headings: headings.len(),
        total_sections: sections.len(),
        avg_headings,
        heading_levels,
        well_structured: 0, // TODO: Implement proper hierarchy check
    }
}

/// Analyze hierarchy directly from corpus content (fallback)
fn analyze_hierarchy_from_content(corpus: &TestCorpus) -> HierarchyStats {
    let mut total_docs = 0;
    let mut docs_with_headings = 0;
    let mut total_headings = 0;
    let mut heading_levels = [0usize; 6];
    let mut heading_counts: Vec<usize> = Vec::new();

    for item in &corpus.items {
        total_docs += 1;
        let mut doc_heading_count = 0;

        for line in item.content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                // Count leading #s
                let level = trimmed.chars().take_while(|c| *c == '#').count();
                if level >= 1 && level <= 6 {
                    // Verify it's actually a heading (space after #s)
                    if trimmed.len() > level && trimmed.chars().nth(level) == Some(' ') {
                        total_headings += 1;
                        doc_heading_count += 1;
                        heading_levels[level - 1] += 1;
                    }
                }
            }
        }

        if doc_heading_count > 0 {
            docs_with_headings += 1;
            heading_counts.push(doc_heading_count);
        }
    }

    let avg_headings = if !heading_counts.is_empty() {
        heading_counts.iter().sum::<usize>() as f64 / heading_counts.len() as f64
    } else {
        0.0
    };

    HierarchyStats {
        total_docs,
        docs_with_headings,
        total_headings,
        total_sections: 0, // Not available from content analysis
        avg_headings,
        heading_levels,
        well_structured: 0,
    }
}

#[tokio::test]
#[ignore]
async fn test_hierarchy_pkm_webdev() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load pkm-webdev corpus");
    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    let stats = analyze_hierarchy(&graph);

    // If graph analysis didn't find headings, fall back to content analysis
    if stats.total_headings == 0 && stats.total_docs > 0 {
        println!("Graph analysis found no headings, using content analysis fallback...");
        let content_stats = analyze_hierarchy_from_content(&corpus);
        content_stats.print_summary("pkm-webdev (content)");
    } else {
        stats.print_summary("pkm-webdev");
    }
}

#[tokio::test]
#[ignore]
async fn test_hierarchy_arch_wiki() {
    let corpus = TestCorpus::load("arch-wiki").expect("Failed to load arch-wiki corpus");
    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    let stats = analyze_hierarchy(&graph);

    if stats.total_headings == 0 && stats.total_docs > 0 {
        let content_stats = analyze_hierarchy_from_content(&corpus);
        content_stats.print_summary("arch-wiki (content)");
    } else {
        stats.print_summary("arch-wiki");
    }
}

#[tokio::test]
#[ignore]
async fn test_hierarchy_from_content_all() {
    // Direct content analysis for all corpora (doesn't depend on graph analyzers)
    let corpora = ["pkm-webdev", "arch-wiki", "pkm-datascience", "shakespeare"];

    println!("\n=== Spike 08: Hierarchy Summary (Content Analysis) ===");
    println!(
        "{:<20} {:>8} {:>12} {:>10} {:>12} {:>10}",
        "Corpus", "Docs", "W/Headings", "Headings", "Avg/Doc", "Result"
    );
    println!("{}", "-".repeat(75));

    for corpus_name in corpora {
        let corpus = match TestCorpus::load(corpus_name) {
            Ok(c) => c,
            Err(_) => {
                println!("{:<20} (not available)", corpus_name);
                continue;
            }
        };

        let stats = analyze_hierarchy_from_content(&corpus);
        let pct = stats.percentage_with_headings();

        let result = if pct >= TARGET_WITH_HEADINGS {
            "GO ✓"
        } else if pct >= PIVOT_THRESHOLD {
            "PIVOT ⚠"
        } else {
            "NO-GO ✗"
        };

        println!(
            "{:<20} {:>8} {:>10} ({:>4.0}%) {:>10} {:>12.1} {:>10}",
            corpus_name,
            stats.total_docs,
            stats.docs_with_headings,
            pct * 100.0,
            stats.total_headings,
            stats.avg_headings,
            result
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_heading_level_distribution() {
    let corpora = ["pkm-webdev", "arch-wiki"];

    for corpus_name in corpora {
        let corpus = match TestCorpus::load(corpus_name) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let stats = analyze_hierarchy_from_content(&corpus);

        println!("\n=== {} Heading Levels ===", corpus_name);
        for (i, count) in stats.heading_levels.iter().enumerate() {
            let pct = if stats.total_headings > 0 {
                *count as f64 / stats.total_headings as f64 * 100.0
            } else {
                0.0
            };
            println!("  H{}: {:>4} ({:>5.1}%)", i + 1, count, pct);
        }
    }
}
