//! Spike Investigation 07: Link Density Variance
//!
//! Analyzes the distribution of links across documents and sections
//! to validate the assumption that link density varies significantly.
//!
//! ## Hypothesis
//! H7 (Link Density): Link density varies significantly between sections,
//! making section-level analysis more informative than document-level.
//!
//! ## Go/No-Go Criteria
//! - **GO**: Coefficient of variation (CV) of section link density ≥0.5
//! - **PIVOT**: CV 0.25-0.5 → Moderate variance, section-level may still help
//! - **NO-GO**: CV <0.25 → Links uniformly distributed, document-level sufficient

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::PropertyValue;
use std::collections::HashMap;

/// Target coefficient of variation for GO criteria
const TARGET_CV: f64 = 0.5;

/// Pivot threshold
const PIVOT_THRESHOLD: f64 = 0.25;

/// Calculate coefficient of variation (std dev / mean)
fn coefficient_of_variation(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean == 0.0 {
        return Some(0.0);
    }

    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    let std_dev = variance.sqrt();

    Some(std_dev / mean)
}

/// Statistics for link density analysis
#[derive(Debug)]
struct LinkDensityStats {
    /// Number of sections analyzed
    section_count: usize,
    /// Number of sections with at least one link
    sections_with_links: usize,
    /// Total links found
    total_links: usize,
    /// Link counts per section
    link_counts: Vec<f64>,
    /// Coefficient of variation
    cv: Option<f64>,
    /// Mean links per section
    mean: f64,
    /// Max links in any section
    max: f64,
    /// Min links in any section
    min: f64,
}

impl LinkDensityStats {
    fn from_link_counts(counts: Vec<usize>) -> Self {
        let link_counts: Vec<f64> = counts.iter().map(|&c| c as f64).collect();
        let section_count = link_counts.len();
        let sections_with_links = counts.iter().filter(|&&c| c > 0).count();
        let total_links: usize = counts.iter().sum();

        let mean = if section_count > 0 {
            total_links as f64 / section_count as f64
        } else {
            0.0
        };

        let max = link_counts.iter().cloned().fold(0.0, f64::max);
        let min = link_counts.iter().cloned().fold(f64::MAX, f64::min);
        let min = if min == f64::MAX { 0.0 } else { min };

        let cv = coefficient_of_variation(&link_counts);

        Self {
            section_count,
            sections_with_links,
            total_links,
            link_counts,
            cv,
            mean,
            max,
            min,
        }
    }

    fn print_summary(&self, corpus_name: &str) {
        println!("\n=== Spike 07: Link Density Analysis ===");
        println!("Corpus: {}", corpus_name);
        println!("Sections analyzed: {}", self.section_count);
        println!(
            "Sections with links: {} ({:.1}%)",
            self.sections_with_links,
            if self.section_count > 0 {
                self.sections_with_links as f64 / self.section_count as f64 * 100.0
            } else {
                0.0
            }
        );
        println!("Total links: {}", self.total_links);
        println!("Mean links/section: {:.2}", self.mean);
        println!("Min: {}, Max: {}", self.min, self.max);

        if let Some(cv) = self.cv {
            println!("Coefficient of Variation: {:.3}", cv);

            if cv >= TARGET_CV {
                println!(
                    "Result: GO ✓ (CV ≥{:.2} indicates high variance)",
                    TARGET_CV
                );
                println!("  → Section-level analysis is justified");
            } else if cv >= PIVOT_THRESHOLD {
                println!(
                    "Result: PIVOT ⚠ (CV {:.2}-{:.2})",
                    PIVOT_THRESHOLD, TARGET_CV
                );
                println!("  → Moderate variance, section-level may still help");
            } else {
                println!("Result: NO-GO ✗ (CV <{:.2})", PIVOT_THRESHOLD);
                println!("  → Links uniformly distributed, document-level may suffice");
            }
        } else {
            println!("Result: N/A (no data)");
        }
    }

    fn print_histogram(&self) {
        if self.link_counts.is_empty() {
            return;
        }

        // Create histogram buckets
        let max_count = self.max as usize;
        let mut histogram: HashMap<usize, usize> = HashMap::new();

        for &count in &self.link_counts {
            *histogram.entry(count as usize).or_insert(0) += 1;
        }

        println!("\nLink Count Distribution:");
        let max_bucket = 10.min(max_count);
        for i in 0..=max_bucket {
            let count = histogram.get(&i).copied().unwrap_or(0);
            let bar_len = (count as f64 / self.section_count as f64 * 50.0) as usize;
            let bar = "#".repeat(bar_len);
            println!("  {:>3} links: {:>4} sections {}", i, count, bar);
        }

        if max_count > 10 {
            let overflow: usize = (11..=max_count)
                .map(|i| histogram.get(&i).copied().unwrap_or(0))
                .sum();
            println!("  >10 links: {:>4} sections", overflow);
        }
    }
}

#[tokio::test]
async fn test_link_density_pkm_webdev() {
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load pkm-webdev corpus");
    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    // Get all section nodes and count their outgoing link edges
    let sections = graph.nodes_by_type("section");
    let link_edges = graph.edges_by_relationship("links_to");

    // Count links per section
    let mut link_counts: Vec<usize> = Vec::new();

    for section in &sections {
        let outgoing_links = link_edges
            .iter()
            .filter(|e| e.source == section.id)
            .count();
        link_counts.push(outgoing_links);
    }

    // If no sections, fall back to document-level analysis
    if link_counts.is_empty() {
        println!("No section nodes found, analyzing document nodes...");

        let documents = graph.nodes_by_type("document");
        for doc in &documents {
            let outgoing_links = link_edges.iter().filter(|e| e.source == doc.id).count();
            link_counts.push(outgoing_links);
        }
    }

    let stats = LinkDensityStats::from_link_counts(link_counts);
    stats.print_summary("pkm-webdev");
    stats.print_histogram();
}

#[tokio::test]
async fn test_link_density_arch_wiki() {
    let corpus = TestCorpus::load("arch-wiki").expect("Failed to load arch-wiki corpus");
    let graph = build_structure_graph(&corpus)
        .await
        .expect("Failed to build graph");

    let sections = graph.nodes_by_type("section");
    let link_edges = graph.edges_by_relationship("links_to");

    let mut link_counts: Vec<usize> = Vec::new();

    for section in &sections {
        let outgoing_links = link_edges
            .iter()
            .filter(|e| e.source == section.id)
            .count();
        link_counts.push(outgoing_links);
    }

    if link_counts.is_empty() {
        let documents = graph.nodes_by_type("document");
        for doc in &documents {
            let outgoing_links = link_edges.iter().filter(|e| e.source == doc.id).count();
            link_counts.push(outgoing_links);
        }
    }

    let stats = LinkDensityStats::from_link_counts(link_counts);
    stats.print_summary("arch-wiki");
    stats.print_histogram();
}

#[tokio::test]
async fn test_link_density_comparison() {
    let corpora = ["pkm-webdev", "arch-wiki", "pkm-datascience", "shakespeare"];

    println!("\n=== Spike 07: Link Density Comparison ===");
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Corpus", "Sections", "W/Links", "Mean", "CV", "Result"
    );
    println!("{}", "-".repeat(70));

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

        let sections = graph.nodes_by_type("section");
        let documents = graph.nodes_by_type("document");
        let link_edges = graph.edges_by_relationship("links_to");

        // Prefer sections, fall back to documents
        let nodes_to_analyze = if !sections.is_empty() {
            sections
        } else {
            documents
        };

        let mut link_counts: Vec<usize> = Vec::new();
        for node in &nodes_to_analyze {
            let outgoing_links = link_edges.iter().filter(|e| e.source == node.id).count();
            link_counts.push(outgoing_links);
        }

        let stats = LinkDensityStats::from_link_counts(link_counts);

        let result = match stats.cv {
            Some(cv) if cv >= TARGET_CV => "GO ✓",
            Some(cv) if cv >= PIVOT_THRESHOLD => "PIVOT ⚠",
            Some(_) => "NO-GO ✗",
            None => "N/A",
        };

        println!(
            "{:<20} {:>10} {:>10} {:>10.2} {:>10.3} {:>10}",
            corpus_name,
            stats.section_count,
            stats.sections_with_links,
            stats.mean,
            stats.cv.unwrap_or(0.0),
            result
        );
    }
}
