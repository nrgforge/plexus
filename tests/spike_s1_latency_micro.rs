//! Spike Experiment S1-Micro: Latency Distribution Profiling (1B Model)
//!
//! **Research Question**: What is the latency distribution for a micro (1B) model
//! compared to the 7B baseline? How much faster is extraction?
//!
//! **Method**:
//! - Same methodology as S1 but using gemma3:1b (815MB vs 4.7GB)
//! - Sample 100 documents across size ranges (small/medium/large)
//! - Measure end-to-end extraction time for each
//! - Calculate p50, p95, p99 percentiles
//!
//! **Success Criteria**: Compare to S1 baseline (7B: p50=11.9s, p95=16.7s)
//!
//! Run with: `cargo test --test spike_s1_latency_micro -- --nocapture`

mod common;

use common::TestCorpus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionResult {
    concepts: Vec<serde_json::Value>,
    #[serde(default)]
    relationships: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct LatencyMeasurement {
    doc_path: String,
    doc_size_bytes: usize,
    doc_size_category: String,
    latency_ms: u64,
    concept_count: usize,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct PercentileStats {
    p50: u64,
    p75: u64,
    p90: u64,
    p95: u64,
    p99: u64,
    min: u64,
    max: u64,
    mean: f64,
    std_dev: f64,
}

#[derive(Debug, Clone)]
struct SizeCategoryStats {
    category: String,
    count: usize,
    success_count: usize,
    mean_latency_ms: f64,
    p95_latency_ms: u64,
    mean_size_bytes: f64,
}

// ============================================================================
// LLM Integration (Micro Model)
// ============================================================================

fn llm_orc_config_dir() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Could not find repo root");
    repo_root.join(".llm-orc").to_string_lossy().to_string()
}

fn check_ollama_available() -> bool {
    std::process::Command::new("ollama")
        .arg("list")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extract concepts using MICRO model (gemma3:1b)
async fn extract_with_timing(content: &str) -> (Result<ExtractionResult, String>, u64) {
    let config_dir = llm_orc_config_dir();
    let start = Instant::now();

    let mut child = Command::new("llm-orc")
        .args([
            "invoke",
            "plexus-semantic-micro",  // <-- MICRO MODEL
            "--config-dir",
            &config_dir,
            "--output-format",
            "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start llm-orc: {}", e))
        .unwrap();

    let stdin = child.stdin.as_mut().unwrap();
    let write_result = stdin.write_all(content.as_bytes()).await;
    drop(child.stdin.take());

    if write_result.is_err() {
        let elapsed = start.elapsed().as_millis() as u64;
        return (Err("Failed to write to stdin".to_string()), elapsed);
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            return (Err(format!("Process error: {}", e)), elapsed);
        }
    };

    let elapsed = start.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return (Err(format!("llm-orc failed: {}", stderr)), elapsed);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_extraction_result(&stdout);
    (result, elapsed)
}

fn parse_extraction_result(text: &str) -> Result<ExtractionResult, String> {
    if let Ok(artifact) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(response_str) = artifact
            .get("results")
            .and_then(|r| r.get("semantic-analyzer"))
            .and_then(|p| p.get("response"))
            .and_then(|r| r.as_str())
        {
            return serde_json::from_str(response_str)
                .map_err(|e| format!("Parse error: {}", e));
        }
    }

    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        serde_json::from_str(json_str)
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        Err("No JSON found".to_string())
    }
}

// ============================================================================
// Statistics
// ============================================================================

fn categorize_size(bytes: usize) -> &'static str {
    match bytes {
        0..=500 => "tiny (<0.5KB)",
        501..=2000 => "small (0.5-2KB)",
        2001..=5000 => "medium (2-5KB)",
        5001..=10000 => "large (5-10KB)",
        _ => "xlarge (>10KB)",
    }
}

fn calculate_percentiles(values: &[u64]) -> PercentileStats {
    if values.is_empty() {
        return PercentileStats {
            p50: 0, p75: 0, p90: 0, p95: 0, p99: 0,
            min: 0, max: 0, mean: 0.0, std_dev: 0.0,
        };
    }

    let mut sorted = values.to_vec();
    sorted.sort();

    let n = sorted.len();
    let percentile = |p: f64| -> u64 {
        let idx = ((p / 100.0) * (n - 1) as f64).round() as usize;
        sorted[idx.min(n - 1)]
    };

    let sum: u64 = sorted.iter().sum();
    let mean = sum as f64 / n as f64;

    let variance: f64 = sorted.iter()
        .map(|&x| (x as f64 - mean).powi(2))
        .sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();

    PercentileStats {
        p50: percentile(50.0),
        p75: percentile(75.0),
        p90: percentile(90.0),
        p95: percentile(95.0),
        p99: percentile(99.0),
        min: sorted[0],
        max: sorted[n - 1],
        mean,
        std_dev,
    }
}

fn calculate_category_stats(measurements: &[LatencyMeasurement]) -> Vec<SizeCategoryStats> {
    let mut by_category: HashMap<String, Vec<&LatencyMeasurement>> = HashMap::new();

    for m in measurements {
        by_category
            .entry(m.doc_size_category.clone())
            .or_default()
            .push(m);
    }

    let mut stats: Vec<SizeCategoryStats> = by_category
        .into_iter()
        .map(|(category, docs)| {
            let success_docs: Vec<_> = docs.iter().filter(|d| d.success).collect();
            let latencies: Vec<u64> = success_docs.iter().map(|d| d.latency_ms).collect();

            let mean_latency = if latencies.is_empty() {
                0.0
            } else {
                latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
            };

            let p95 = if latencies.is_empty() {
                0
            } else {
                let mut sorted = latencies.clone();
                sorted.sort();
                let idx = ((0.95 * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1);
                sorted[idx]
            };

            let mean_size = docs.iter().map(|d| d.doc_size_bytes as f64).sum::<f64>() / docs.len() as f64;

            SizeCategoryStats {
                category,
                count: docs.len(),
                success_count: success_docs.len(),
                mean_latency_ms: mean_latency,
                p95_latency_ms: p95,
                mean_size_bytes: mean_size,
            }
        })
        .collect();

    let order = ["tiny (<0.5KB)", "small (0.5-2KB)", "medium (2-5KB)", "large (5-10KB)", "xlarge (>10KB)"];
    stats.sort_by_key(|s| order.iter().position(|&o| o == s.category).unwrap_or(99));

    stats
}

// ============================================================================
// Main Test
// ============================================================================

#[tokio::test]
async fn test_s1_micro_latency_profiling() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment S1-Micro: Latency Distribution (gemma3:1b) ===");
    println!("=== Comparing to 7B baseline: p50=11.9s, p95=16.7s ===");
    println!("{}\n", "=".repeat(80));

    if !check_ollama_available() {
        println!("SKIPPED: Ollama not available");
        return;
    }

    // Load corpora
    let corpus_webdev = TestCorpus::load("pkm-webdev").expect("Failed to load pkm-webdev");
    let corpus_ds = TestCorpus::load("pkm-datascience").expect("Failed to load pkm-datascience");

    // Combine and sample documents
    let mut all_docs: Vec<(&str, &str, usize)> = Vec::new();

    for item in &corpus_webdev.items {
        all_docs.push(("pkm-webdev", item.id.as_str(), item.content.len()));
    }
    for item in &corpus_ds.items {
        all_docs.push(("pkm-datascience", item.id.as_str(), item.content.len()));
    }

    // Stratified sampling
    let mut by_size: HashMap<&str, Vec<(&str, &str, usize)>> = HashMap::new();
    for doc in &all_docs {
        let cat = categorize_size(doc.2);
        by_size.entry(cat).or_default().push(*doc);
    }

    println!("Document size distribution:");
    for (cat, docs) in &by_size {
        println!("  {}: {} documents", cat, docs.len());
    }

    let target_total = 100;
    let mut sampled: Vec<(&str, &str, usize)> = Vec::new();

    use rand::seq::SliceRandom;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let mut rng: StdRng = SeedableRng::seed_from_u64(42);

    let categories = ["tiny (<0.5KB)", "small (0.5-2KB)", "medium (2-5KB)", "large (5-10KB)", "xlarge (>10KB)"];
    for cat in &categories {
        if let Some(docs) = by_size.get(cat) {
            let mut docs_vec = docs.clone();
            docs_vec.shuffle(&mut rng);
            let take = (docs_vec.len()).min(20);
            sampled.extend(docs_vec.into_iter().take(take));
        }
    }

    sampled.shuffle(&mut rng);
    sampled.truncate(target_total);

    println!("\nSampled {} documents for latency profiling", sampled.len());
    println!("Model: gemma3:1b (815MB) - MICRO\n");

    // Measure latency
    let mut measurements: Vec<LatencyMeasurement> = Vec::new();
    let total_start = Instant::now();

    for (i, (corpus, doc_path, size)) in sampled.iter().enumerate() {
        let short_name = doc_path.split('/').last().unwrap_or(doc_path);
        let category = categorize_size(*size);

        let content = if *corpus == "pkm-webdev" {
            corpus_webdev.items.iter()
                .find(|item| item.id.as_str() == *doc_path)
                .map(|item| item.content.as_str())
        } else {
            corpus_ds.items.iter()
                .find(|item| item.id.as_str() == *doc_path)
                .map(|item| item.content.as_str())
        };

        let content = match content {
            Some(c) if c.len() >= 50 => c,
            _ => {
                println!("[{:3}/{}] {} - SKIPPED (too short)", i + 1, sampled.len(), short_name);
                continue;
            }
        };

        print!("[{:3}/{}] {} ({}, {}B): ", i + 1, sampled.len(), short_name, category, size);

        let (result, latency_ms) = extract_with_timing(content).await;

        let (success, concept_count, error) = match result {
            Ok(r) => (true, r.concepts.len(), None),
            Err(e) => (false, 0, Some(e)),
        };

        if success {
            println!("{}ms ({} concepts)", latency_ms, concept_count);
        } else {
            println!("{}ms FAILED: {}", latency_ms, error.as_ref().unwrap_or(&"Unknown".to_string()));
        }

        measurements.push(LatencyMeasurement {
            doc_path: doc_path.to_string(),
            doc_size_bytes: *size,
            doc_size_category: category.to_string(),
            latency_ms,
            concept_count,
            success,
            error,
        });
    }

    let total_elapsed = total_start.elapsed();
    println!("\nTotal profiling time: {:.1}s", total_elapsed.as_secs_f64());

    // Calculate statistics
    let successful: Vec<_> = measurements.iter().filter(|m| m.success).collect();
    let latencies: Vec<u64> = successful.iter().map(|m| m.latency_ms).collect();

    if latencies.is_empty() {
        println!("\nNo successful extractions - cannot calculate statistics");
        return;
    }

    let stats = calculate_percentiles(&latencies);
    let category_stats = calculate_category_stats(&measurements);

    // Print results
    println!("\n{}", "=".repeat(80));
    println!("=== Overall Latency Distribution (1B Model) ===");
    println!("{}\n", "=".repeat(80));

    println!("Successful extractions: {}/{}", successful.len(), measurements.len());
    println!("Failure rate: {:.1}%", 100.0 * (measurements.len() - successful.len()) as f64 / measurements.len() as f64);
    println!();

    println!("Latency Percentiles:");
    println!("  Min:  {:>6}ms", stats.min);
    println!("  p50:  {:>6}ms (7B baseline: 11900ms)", stats.p50);
    println!("  p75:  {:>6}ms", stats.p75);
    println!("  p90:  {:>6}ms", stats.p90);
    println!("  p95:  {:>6}ms (7B baseline: 16700ms)", stats.p95);
    println!("  p99:  {:>6}ms", stats.p99);
    println!("  Max:  {:>6}ms", stats.max);
    println!();
    println!("  Mean: {:>6.0}ms (std={:.0}ms)", stats.mean, stats.std_dev);

    // By size category
    println!("\n{}", "=".repeat(80));
    println!("=== Latency by Document Size ===");
    println!("{}\n", "=".repeat(80));

    println!("{:<20} {:>8} {:>10} {:>12} {:>12}",
        "Category", "Count", "Success", "Mean (ms)", "p95 (ms)");
    println!("{}", "-".repeat(65));

    for s in &category_stats {
        println!("{:<20} {:>8} {:>10} {:>12.0} {:>12}",
            s.category, s.count, s.success_count, s.mean_latency_ms, s.p95_latency_ms);
    }

    // Comparison with 7B baseline
    println!("\n{}", "=".repeat(80));
    println!("=== Comparison with 7B Baseline ===");
    println!("{}\n", "=".repeat(80));

    let baseline_p50 = 11900u64;
    let baseline_p95 = 16700u64;

    let speedup_p50 = baseline_p50 as f64 / stats.p50 as f64;
    let speedup_p95 = baseline_p95 as f64 / stats.p95 as f64;

    println!("{:<12} {:>12} {:>12} {:>12}",
        "Metric", "7B", "1B (Micro)", "Speedup");
    println!("{}", "-".repeat(50));
    println!("{:<12} {:>12}ms {:>12}ms {:>11.1}x", "p50", baseline_p50, stats.p50, speedup_p50);
    println!("{:<12} {:>12}ms {:>12}ms {:>11.1}x", "p95", baseline_p95, stats.p95, speedup_p95);

    let throughput = successful.len() as f64 / total_elapsed.as_secs_f64() * 60.0;
    let baseline_throughput = 3.8; // from S1 results

    println!("\nThroughput: {:.1} docs/min (7B baseline: {:.1})", throughput, baseline_throughput);
    println!("Throughput speedup: {:.1}x", throughput / baseline_throughput);

    println!("\n{}", "=".repeat(80));

    // Save raw data
    save_measurements(&measurements);
}

fn save_measurements(measurements: &[LatencyMeasurement]) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("tests")
        .join("artifacts")
        .join("s1_micro_latency_measurements.json");

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(content) = serde_json::to_string_pretty(measurements) {
        let _ = std::fs::write(&path, content);
        println!("\nRaw data saved to: tests/artifacts/s1_micro_latency_measurements.json");
    }
}

#[tokio::test]
async fn test_s1_micro_single_timing() {
    if !check_ollama_available() {
        println!("SKIPPED: Ollama not available");
        return;
    }

    let content = "# Quick Test\n\nThis is a simple document about JavaScript and React.";

    println!("Timing single extraction with gemma3:1b...");
    let (result, latency) = extract_with_timing(content).await;

    match result {
        Ok(r) => println!("Success: {}ms, {} concepts", latency, r.concepts.len()),
        Err(e) => println!("Failed: {}ms, error: {}", latency, e),
    }
}
