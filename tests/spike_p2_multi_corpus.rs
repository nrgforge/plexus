//! Spike Experiment P2: Multi-Corpus Extraction Validation
//!
//! **Research Question**: Does the plexus-semantic extraction ensemble generalize
//! across different corpus types, or does it show content-specific failure modes?
//!
//! **Corpora**:
//! - pkm-webdev: Technical PKM (baseline from P1, cached)
//! - pkm-datascience: Different technical domain
//! - shakespeare: Literary/flat corpus (known challenge)
//!
//! **Success Criteria**:
//! - Consistent extraction quality across tech corpora OR
//! - Clear, documented failure modes for different content types
//!
//! Run with: `cargo test --test spike_p2_multi_corpus -- --nocapture`

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::PropertyValue;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// ============================================================================
// Types
// ============================================================================

/// Concept extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractedConcept {
    name: String,
    #[serde(rename = "type")]
    concept_type: Option<String>,
    confidence: f64,
}

/// Relationship extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractedRelationship {
    source: String,
    target: String,
    relationship: String,
    confidence: f64,
}

/// Full extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionResult {
    concepts: Vec<ExtractedConcept>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationship>,
}

/// Grounding check result for a single concept
#[derive(Debug, Clone)]
struct GroundingCheck {
    concept: String,
    grounded: bool,
    evidence: Option<String>,
}

/// Document-level extraction metrics
#[derive(Debug, Clone)]
struct DocExtractionMetrics {
    doc_path: String,
    concept_count: usize,
    relationship_count: usize,
    grounded_count: usize,
    grounding_pct: f64,
    avg_confidence: f64,
    extraction_time_ms: u64,
    error: Option<String>,
}

/// Corpus-level aggregated metrics
#[derive(Debug, Clone)]
struct CorpusMetrics {
    corpus_name: String,
    doc_count: usize,
    docs_extracted: usize,
    docs_failed: usize,
    total_concepts: usize,
    total_relationships: usize,
    avg_concepts_per_doc: f64,
    avg_grounding_pct: f64,
    avg_extraction_time_ms: f64,
    concept_type_distribution: HashMap<String, usize>,
    failure_modes: Vec<String>,
}

// ============================================================================
// LLM Integration
// ============================================================================

/// Get the llm-orc config directory
fn llm_orc_config_dir() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Could not find repo root");
    repo_root.join(".llm-orc").to_string_lossy().to_string()
}

/// Check if Ollama is available
fn check_ollama_available() -> bool {
    std::process::Command::new("ollama")
        .arg("list")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extract concepts from document using plexus-semantic ensemble
async fn extract_concepts_llm(content: &str) -> Result<(ExtractionResult, u64), String> {
    let config_dir = llm_orc_config_dir();
    let start = std::time::Instant::now();

    let mut child = Command::new("llm-orc")
        .args([
            "invoke",
            "plexus-semantic",
            "--config-dir",
            &config_dir,
            "--output-format",
            "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start llm-orc: {}", e))?;

    let stdin = child.stdin.as_mut().ok_or("No stdin")?;
    stdin
        .write_all(content.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait for output: {}", e))?;

    let elapsed_ms = start.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("llm-orc failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_extraction_result(&stdout)?;
    Ok((result, elapsed_ms))
}

/// Parse extraction result from LLM output
fn parse_extraction_result(text: &str) -> Result<ExtractionResult, String> {
    // llm-orc returns artifact JSON with response nested inside
    if let Ok(artifact) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(response_str) = artifact
            .get("results")
            .and_then(|r| r.get("semantic-analyzer"))
            .and_then(|p| p.get("response"))
            .and_then(|r| r.as_str())
        {
            return serde_json::from_str(response_str)
                .map_err(|e| format!("Failed to parse inner response: {} in '{}'", e,
                    &response_str[..response_str.len().min(200)]));
        }
    }

    // Fallback: try to find raw JSON
    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON: {} in '{}'", e, &json_str[..json_str.len().min(200)]))
    } else {
        Err(format!(
            "No JSON found in output: {}",
            &text[..text.len().min(200)]
        ))
    }
}

// ============================================================================
// Grounding Checks
// ============================================================================

/// Check if a concept is grounded in the document content
fn check_concept_grounding(concept: &str, content: &str) -> GroundingCheck {
    let content_lower = content.to_lowercase();
    let concept_lower = concept.to_lowercase();

    // Direct mention check
    if content_lower.contains(&concept_lower) {
        return GroundingCheck {
            concept: concept.to_string(),
            grounded: true,
            evidence: Some(format!("Direct mention: '{}'", concept)),
        };
    }

    // Check for partial matches (multi-word concepts)
    let words: Vec<&str> = concept_lower.split_whitespace().collect();
    if words.len() > 1 {
        let all_words_present = words.iter().all(|w| content_lower.contains(w));
        if all_words_present {
            return GroundingCheck {
                concept: concept.to_string(),
                grounded: true,
                evidence: Some(format!("All words present: {:?}", words)),
            };
        }
    }

    // Check for semantic variants (simple heuristics)
    let variants = generate_variants(&concept_lower);
    for variant in &variants {
        if content_lower.contains(variant) {
            return GroundingCheck {
                concept: concept.to_string(),
                grounded: true,
                evidence: Some(format!("Variant match: '{}'", variant)),
            };
        }
    }

    // Not grounded - likely hallucinated or inferred
    GroundingCheck {
        concept: concept.to_string(),
        grounded: false,
        evidence: None,
    }
}

/// Generate simple variants of a concept for grounding check
fn generate_variants(concept: &str) -> Vec<String> {
    let mut variants = Vec::new();

    // Plural/singular
    if concept.ends_with('s') {
        variants.push(concept.trim_end_matches('s').to_string());
    } else {
        variants.push(format!("{}s", concept));
    }

    // Common tech abbreviations
    if concept == "javascript" {
        variants.push("js".to_string());
    } else if concept == "typescript" {
        variants.push("ts".to_string());
    } else if concept == "python" {
        variants.push("py".to_string());
    }

    // Hyphenation variants
    if concept.contains(' ') {
        variants.push(concept.replace(' ', "-"));
        variants.push(concept.replace(' ', "_"));
    }

    variants
}

// ============================================================================
// Cache Management
// ============================================================================

/// Cache file path for a corpus
fn extraction_cache_path(corpus_name: &str) -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .join("tests")
        .join("artifacts")
        .join(format!("p2_extraction_cache_{}.json", corpus_name))
}

/// Load cached extractions if available
fn load_extraction_cache(corpus_name: &str) -> Option<HashMap<String, ExtractionResult>> {
    let path = extraction_cache_path(corpus_name);
    if path.exists() {
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Save extractions to cache
fn save_extraction_cache(corpus_name: &str, cache: &HashMap<String, ExtractionResult>) {
    let path = extraction_cache_path(corpus_name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(&path, content);
    }
}

// ============================================================================
// Corpus Analysis
// ============================================================================

/// Analyze a single corpus
async fn analyze_corpus(corpus_name: &str, sample_size: usize) -> Result<CorpusMetrics, String> {
    println!("\n--- Analyzing {} ---", corpus_name);

    let corpus = TestCorpus::load(corpus_name)
        .map_err(|e| format!("Failed to load corpus: {}", e))?;

    let graph = build_structure_graph(&corpus)
        .await
        .map_err(|e| format!("Failed to build graph: {:?}", e))?;

    let context = &graph.context;

    // Get all documents
    let doc_nodes: Vec<_> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .collect();

    let doc_count = doc_nodes.len();
    println!("  Corpus has {} documents", doc_count);

    // Load or create cache
    let mut cache = load_extraction_cache(corpus_name).unwrap_or_default();
    let cache_hits = cache.len();
    if cache_hits > 0 {
        println!("  Cache loaded: {} documents already extracted", cache_hits);
    }

    // Sample documents for extraction
    let mut rng: StdRng = rand::SeedableRng::seed_from_u64(42);
    let mut doc_paths: Vec<_> = doc_nodes.iter()
        .filter_map(|(id, node)| {
            node.properties.get("source")
                .and_then(|v| if let PropertyValue::String(s) = v { Some((id.clone(), s.clone())) } else { None })
        })
        .collect();

    use rand::seq::SliceRandom;
    doc_paths.shuffle(&mut rng);
    let docs_to_analyze: Vec<_> = doc_paths.into_iter().take(sample_size).collect();

    println!("  Analyzing {} documents (sample)", docs_to_analyze.len());

    // Extract and analyze
    let mut doc_metrics: Vec<DocExtractionMetrics> = Vec::new();
    let mut failure_modes: Vec<String> = Vec::new();
    let mut concept_types: HashMap<String, usize> = HashMap::new();

    for (i, (_doc_id, source_path)) in docs_to_analyze.iter().enumerate() {
        let short_name = source_path.split('/').last().unwrap_or(source_path);
        print!("  [{}/{}] {}: ", i + 1, docs_to_analyze.len(), short_name);

        // Get content
        let content = corpus.items.iter()
            .find(|item| item.id.as_str() == source_path)
            .map(|item| item.content.as_str())
            .unwrap_or("");

        // Skip very short documents
        if content.len() < 50 {
            println!("SKIPPED (too short)");
            continue;
        }

        // Check cache first
        let (result, extraction_time) = if let Some(cached) = cache.get(source_path) {
            (cached.clone(), 0)
        } else {
            match extract_concepts_llm(content).await {
                Ok((r, t)) => {
                    cache.insert(source_path.clone(), r.clone());
                    (r, t)
                }
                Err(e) => {
                    println!("FAILED: {}", e);
                    failure_modes.push(format!("{}: {}", short_name, e));
                    doc_metrics.push(DocExtractionMetrics {
                        doc_path: source_path.clone(),
                        concept_count: 0,
                        relationship_count: 0,
                        grounded_count: 0,
                        grounding_pct: 0.0,
                        avg_confidence: 0.0,
                        extraction_time_ms: 0,
                        error: Some(e),
                    });
                    continue;
                }
            }
        };

        // Check grounding for each concept
        let mut grounded_count = 0;
        for concept in &result.concepts {
            let check = check_concept_grounding(&concept.name, content);
            if check.grounded {
                grounded_count += 1;
            }

            // Track concept types
            let ctype = concept.concept_type.clone().unwrap_or_else(|| "unknown".to_string());
            *concept_types.entry(ctype).or_insert(0) += 1;
        }

        let grounding_pct = if result.concepts.is_empty() {
            100.0 // No concepts = nothing to ground
        } else {
            100.0 * grounded_count as f64 / result.concepts.len() as f64
        };

        let avg_confidence = if result.concepts.is_empty() {
            0.0
        } else {
            result.concepts.iter().map(|c| c.confidence).sum::<f64>() / result.concepts.len() as f64
        };

        println!("{} concepts, {:.0}% grounded, {:.2} avg conf",
            result.concepts.len(), grounding_pct, avg_confidence);

        doc_metrics.push(DocExtractionMetrics {
            doc_path: source_path.clone(),
            concept_count: result.concepts.len(),
            relationship_count: result.relationships.len(),
            grounded_count,
            grounding_pct,
            avg_confidence,
            extraction_time_ms: extraction_time,
            error: None,
        });
    }

    // Save updated cache
    save_extraction_cache(corpus_name, &cache);

    // Aggregate metrics
    let successful: Vec<_> = doc_metrics.iter().filter(|m| m.error.is_none()).collect();
    let docs_extracted = successful.len();
    let docs_failed = doc_metrics.len() - docs_extracted;

    let total_concepts: usize = successful.iter().map(|m| m.concept_count).sum();
    let total_relationships: usize = successful.iter().map(|m| m.relationship_count).sum();

    let avg_concepts_per_doc = if docs_extracted > 0 {
        total_concepts as f64 / docs_extracted as f64
    } else {
        0.0
    };

    let avg_grounding_pct = if docs_extracted > 0 {
        successful.iter().map(|m| m.grounding_pct).sum::<f64>() / docs_extracted as f64
    } else {
        0.0
    };

    let avg_extraction_time = if docs_extracted > 0 {
        successful.iter().map(|m| m.extraction_time_ms as f64).sum::<f64>() / docs_extracted as f64
    } else {
        0.0
    };

    Ok(CorpusMetrics {
        corpus_name: corpus_name.to_string(),
        doc_count,
        docs_extracted,
        docs_failed,
        total_concepts,
        total_relationships,
        avg_concepts_per_doc,
        avg_grounding_pct,
        avg_extraction_time_ms: avg_extraction_time,
        concept_type_distribution: concept_types,
        failure_modes,
    })
}

// ============================================================================
// Main Test
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_p2_multi_corpus_extraction() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment P2: Multi-Corpus Extraction Validation ===");
    println!("{}\n", "=".repeat(80));

    // Check Ollama availability
    if !check_ollama_available() {
        println!("⚠️  SKIPPED: Ollama not available");
        println!("   To run this test, ensure Ollama is running with llama3 model.");
        return;
    }

    // Analyze each corpus
    let corpora = [
        ("pkm-webdev", 20),      // Baseline from P1
        ("pkm-datascience", 20), // Different tech domain
        ("shakespeare", 15),     // Literary (flat corpus)
    ];

    let mut results: Vec<CorpusMetrics> = Vec::new();

    for (corpus_name, sample_size) in &corpora {
        match analyze_corpus(corpus_name, *sample_size).await {
            Ok(metrics) => results.push(metrics),
            Err(e) => println!("  SKIPPED {}: {}", corpus_name, e),
        }
    }

    // Print comparison table
    println!("\n{}", "=".repeat(80));
    println!("=== Corpus Comparison ===");
    println!("{}\n", "=".repeat(80));

    println!("{:<20} {:>8} {:>10} {:>12} {:>12} {:>10}",
        "Corpus", "Docs", "Extracted", "Concepts", "Grounding%", "Avg Time");
    println!("{}", "-".repeat(80));

    for r in &results {
        println!("{:<20} {:>8} {:>10} {:>12} {:>11.1}% {:>9.0}ms",
            r.corpus_name, r.doc_count, r.docs_extracted,
            r.total_concepts, r.avg_grounding_pct, r.avg_extraction_time_ms);
    }

    // Print concept type distribution
    println!("\n{}", "=".repeat(80));
    println!("=== Concept Type Distribution ===");
    println!("{}\n", "=".repeat(80));

    for r in &results {
        println!("{}:", r.corpus_name);
        let mut types: Vec<_> = r.concept_type_distribution.iter().collect();
        types.sort_by(|a, b| b.1.cmp(a.1));
        for (ctype, count) in types.iter().take(5) {
            println!("  {}: {}", ctype, count);
        }
    }

    // Print failure modes
    println!("\n{}", "=".repeat(80));
    println!("=== Failure Modes ===");
    println!("{}\n", "=".repeat(80));

    for r in &results {
        if r.failure_modes.is_empty() {
            println!("{}: No failures", r.corpus_name);
        } else {
            println!("{}:", r.corpus_name);
            for mode in &r.failure_modes {
                println!("  - {}", mode);
            }
        }
    }

    // Analysis and verdict
    println!("\n{}", "=".repeat(80));
    println!("=== Analysis ===");
    println!("{}\n", "=".repeat(80));

    let baseline = results.iter().find(|r| r.corpus_name == "pkm-webdev");

    for r in &results {
        println!("\n{}:", r.corpus_name);

        // Compare to baseline
        if let Some(base) = baseline {
            if r.corpus_name != "pkm-webdev" {
                let grounding_diff = r.avg_grounding_pct - base.avg_grounding_pct;
                let concepts_diff = r.avg_concepts_per_doc - base.avg_concepts_per_doc;

                println!("  vs baseline: grounding {:+.1}%, concepts/doc {:+.1}",
                    grounding_diff, concepts_diff);
            }
        }

        // Quality assessment
        if r.avg_grounding_pct >= 90.0 {
            println!("  Quality: EXCELLENT (≥90% grounded)");
        } else if r.avg_grounding_pct >= 80.0 {
            println!("  Quality: GOOD (≥80% grounded)");
        } else if r.avg_grounding_pct >= 60.0 {
            println!("  Quality: ACCEPTABLE (≥60% grounded, some hallucination)");
        } else {
            println!("  Quality: POOR (<60% grounded, significant hallucination)");
        }

        // Identify content-specific issues
        if r.avg_concepts_per_doc < 3.0 {
            println!("  Issue: Low concept density - may need prompt tuning");
        }
        if r.docs_failed > r.docs_extracted / 4 {
            println!("  Issue: High failure rate ({}/{}) - check content format",
                r.docs_failed, r.docs_extracted + r.docs_failed);
        }
    }

    // Final verdict
    println!("\n{}", "=".repeat(80));
    println!("=== Verdict ===");
    println!("{}\n", "=".repeat(80));

    let tech_corpora: Vec<_> = results.iter()
        .filter(|r| r.corpus_name.starts_with("pkm"))
        .collect();
    let literary_corpora: Vec<_> = results.iter()
        .filter(|r| r.corpus_name == "shakespeare")
        .collect();

    // Check tech corpus consistency
    if tech_corpora.len() >= 2 {
        let grounding_variance: f64 = tech_corpora.iter()
            .map(|r| r.avg_grounding_pct)
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[0] - w[1]).abs())
            .sum::<f64>() / (tech_corpora.len() - 1) as f64;

        if grounding_variance < 10.0 {
            println!("✓ Tech corpora: CONSISTENT ({:.1}% variance in grounding)", grounding_variance);
        } else {
            println!("⚠ Tech corpora: INCONSISTENT ({:.1}% variance)", grounding_variance);
        }
    }

    // Check literary corpus behavior
    if let Some(lit) = literary_corpora.first() {
        if lit.avg_grounding_pct >= 60.0 {
            println!("✓ Literary corpus: ACCEPTABLE ({:.1}% grounding)", lit.avg_grounding_pct);
        } else {
            println!("⚠ Literary corpus: NEEDS ADAPTATION ({:.1}% grounding)", lit.avg_grounding_pct);
            println!("  → May need content-type-specific prompts for literary analysis");
        }
    }

    // Overall
    let overall_grounding = results.iter()
        .map(|r| r.avg_grounding_pct)
        .sum::<f64>() / results.len() as f64;

    if overall_grounding >= 80.0 {
        println!("\nOVERALL: GO ✓ - Extraction generalizes well ({:.1}% avg grounding)", overall_grounding);
    } else if overall_grounding >= 60.0 {
        println!("\nOVERALL: CONDITIONAL GO - Extraction works with caveats ({:.1}% avg grounding)", overall_grounding);
    } else {
        println!("\nOVERALL: NEEDS WORK - Extraction needs improvement ({:.1}% avg grounding)", overall_grounding);
    }

    println!("{}", "=".repeat(80));
}

/// Quick test to verify extraction works on a single document
#[tokio::test]
#[ignore]
async fn test_extraction_single_doc() {
    if !check_ollama_available() {
        println!("SKIPPED: Ollama not available");
        return;
    }

    let content = r#"
# Introduction to Promises

Promises are a fundamental concept in JavaScript for handling asynchronous operations.

## Creating a Promise

```javascript
const promise = new Promise((resolve, reject) => {
    // async operation
});
```

## Chaining Promises

Promises can be chained using `.then()` and `.catch()`:

```javascript
fetch('/api/data')
    .then(response => response.json())
    .then(data => console.log(data))
    .catch(error => console.error(error));
```

## Async/Await

Modern JavaScript provides async/await syntax for cleaner promise handling.
"#;

    println!("Testing extraction on sample document...");

    match extract_concepts_llm(content).await {
        Ok((result, time)) => {
            println!("Extracted {} concepts in {}ms:", result.concepts.len(), time);
            for concept in &result.concepts {
                let grounding = check_concept_grounding(&concept.name, content);
                println!("  - {} ({:.2}) [{}]",
                    concept.name, concept.confidence,
                    if grounding.grounded { "grounded" } else { "ungrounded" });
            }

            let grounded = result.concepts.iter()
                .filter(|c| check_concept_grounding(&c.name, content).grounded)
                .count();
            println!("\nGrounding: {}/{} ({:.0}%)",
                grounded, result.concepts.len(),
                100.0 * grounded as f64 / result.concepts.len() as f64);
        }
        Err(e) => {
            panic!("Extraction failed: {}", e);
        }
    }
}
