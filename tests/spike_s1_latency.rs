//! Spike Experiment S1: Latency Distribution Profiling
//!
//! **Research Question**: What is the actual latency distribution for LLM extraction?
//! Are the claimed targets (<5s single doc, <30s large doc) realistic?
//!
//! **Method**:
//! - Sample 100 documents across size ranges (small/medium/large)
//! - Measure end-to-end extraction time for each
//! - Calculate p50, p95, p99 percentiles
//! - Correlate latency with document characteristics
//!
//! **Success Criteria**: p95 < 10s
//!
//! Run with: `cargo test --test spike_s1_latency -- --nocapture`

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

/// Extraction result (we only care about success/failure for latency)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionResult {
    concepts: Vec<serde_json::Value>,
    #[serde(default)]
    relationships: Vec<serde_json::Value>,
}

/// Latency measurement for a single document
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

/// Percentile statistics
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

/// Size category analysis
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
// LLM Integration
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

/// Extract concepts and measure latency (no caching - we want real timings)
async fn extract_with_timing(content: &str) -> (Result<ExtractionResult, String>, u64) {
    let config_dir = llm_orc_config_dir();
    let start = Instant::now();

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

    // Sort by size category
    let order = ["tiny (<0.5KB)", "small (0.5-2KB)", "medium (2-5KB)", "large (5-10KB)", "xlarge (>10KB)"];
    stats.sort_by_key(|s| order.iter().position(|&o| o == s.category).unwrap_or(99));

    stats
}

// ============================================================================
// Main Test
// ============================================================================

#[tokio::test]
async fn test_s1_latency_profiling() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment S1: Latency Distribution Profiling ===");
    println!("{}\n", "=".repeat(80));

    if !check_ollama_available() {
        println!("⚠️  SKIPPED: Ollama not available");
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

    // Stratified sampling: ensure coverage across size categories
    let mut by_size: HashMap<&str, Vec<(&str, &str, usize)>> = HashMap::new();
    for doc in &all_docs {
        let cat = categorize_size(doc.2);
        by_size.entry(cat).or_default().push(*doc);
    }

    println!("Document size distribution:");
    for (cat, docs) in &by_size {
        println!("  {}: {} documents", cat, docs.len());
    }

    // Sample up to 100 docs, stratified by size
    let target_total = 100;
    let mut sampled: Vec<(&str, &str, usize)> = Vec::new();

    use rand::seq::SliceRandom;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    let mut rng: StdRng = SeedableRng::seed_from_u64(42);

    // First, ensure at least some from each category
    let categories = ["tiny (<0.5KB)", "small (0.5-2KB)", "medium (2-5KB)", "large (5-10KB)", "xlarge (>10KB)"];
    for cat in &categories {
        if let Some(docs) = by_size.get(cat) {
            let mut docs_vec = docs.clone();
            docs_vec.shuffle(&mut rng);
            // Take up to 20 per category, or proportionally
            let take = (docs_vec.len()).min(20);
            sampled.extend(docs_vec.into_iter().take(take));
        }
    }

    // Shuffle and trim to target
    sampled.shuffle(&mut rng);
    sampled.truncate(target_total);

    println!("\nSampled {} documents for latency profiling", sampled.len());
    println!("(Note: No caching - measuring real LLM latency)\n");

    // Measure latency for each document
    let mut measurements: Vec<LatencyMeasurement> = Vec::new();
    let total_start = Instant::now();

    for (i, (corpus, doc_path, size)) in sampled.iter().enumerate() {
        let short_name = doc_path.split('/').last().unwrap_or(doc_path);
        let category = categorize_size(*size);

        // Get content
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
        println!("\n⚠️  No successful extractions - cannot calculate statistics");
        return;
    }

    let stats = calculate_percentiles(&latencies);
    let category_stats = calculate_category_stats(&measurements);

    // Print results
    println!("\n{}", "=".repeat(80));
    println!("=== Overall Latency Distribution ===");
    println!("{}\n", "=".repeat(80));

    println!("Successful extractions: {}/{}", successful.len(), measurements.len());
    println!("Failure rate: {:.1}%", 100.0 * (measurements.len() - successful.len()) as f64 / measurements.len() as f64);
    println!();

    println!("Latency Percentiles:");
    println!("  Min:  {:>6}ms", stats.min);
    println!("  p50:  {:>6}ms (median)", stats.p50);
    println!("  p75:  {:>6}ms", stats.p75);
    println!("  p90:  {:>6}ms", stats.p90);
    println!("  p95:  {:>6}ms  ← TARGET: <10,000ms", stats.p95);
    println!("  p99:  {:>6}ms", stats.p99);
    println!("  Max:  {:>6}ms", stats.max);
    println!();
    println!("  Mean: {:>6.0}ms (σ={:.0}ms)", stats.mean, stats.std_dev);

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

    // Correlation analysis
    println!("\n{}", "=".repeat(80));
    println!("=== Size-Latency Correlation ===");
    println!("{}\n", "=".repeat(80));

    let sizes: Vec<f64> = successful.iter().map(|m| m.doc_size_bytes as f64).collect();
    let lats: Vec<f64> = successful.iter().map(|m| m.latency_ms as f64).collect();

    if sizes.len() > 2 {
        let n = sizes.len() as f64;
        let sum_x: f64 = sizes.iter().sum();
        let sum_y: f64 = lats.iter().sum();
        let sum_xy: f64 = sizes.iter().zip(lats.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = sizes.iter().map(|x| x * x).sum();
        let sum_y2: f64 = lats.iter().map(|y| y * y).sum();

        let correlation = (n * sum_xy - sum_x * sum_y) /
            ((n * sum_x2 - sum_x.powi(2)).sqrt() * (n * sum_y2 - sum_y.powi(2)).sqrt());

        println!("Pearson correlation (size vs latency): {:.3}", correlation);

        if correlation > 0.7 {
            println!("  → Strong positive correlation: larger docs take longer");
        } else if correlation > 0.3 {
            println!("  → Moderate positive correlation");
        } else if correlation > -0.3 {
            println!("  → Weak/no correlation: latency mostly independent of size");
        } else {
            println!("  → Negative correlation (unexpected)");
        }

        // Linear regression for prediction
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        let intercept = (sum_y - slope * sum_x) / n;

        println!("\nLinear model: latency_ms = {:.2} + {:.4} × size_bytes", intercept, slope);
        println!("  Predicted latency for 1KB doc:  {:.0}ms", intercept + slope * 1000.0);
        println!("  Predicted latency for 5KB doc:  {:.0}ms", intercept + slope * 5000.0);
        println!("  Predicted latency for 10KB doc: {:.0}ms", intercept + slope * 10000.0);
    }

    // Verdict
    println!("\n{}", "=".repeat(80));
    println!("=== Verdict ===");
    println!("{}\n", "=".repeat(80));

    let p95_target = 10000; // 10 seconds

    if stats.p95 <= p95_target {
        println!("✓ PASS: p95 latency {}ms ≤ {}ms target", stats.p95, p95_target);
    } else {
        println!("✗ FAIL: p95 latency {}ms > {}ms target", stats.p95, p95_target);
    }

    // Additional insights
    if stats.p50 < 5000 {
        println!("✓ Median latency under 5s - good interactive experience");
    }

    if stats.max > 30000 {
        println!("⚠ Maximum latency {}ms > 30s - some docs may timeout", stats.max);
    }

    let throughput = successful.len() as f64 / total_elapsed.as_secs_f64() * 60.0;
    println!("\nThroughput: {:.1} docs/minute (sequential)", throughput);

    println!("{}", "=".repeat(80));

    // Save raw data for further analysis
    save_measurements(&measurements);
}

fn save_measurements(measurements: &[LatencyMeasurement]) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir)
        .join("tests")
        .join("artifacts")
        .join("s1_latency_measurements.json");

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(content) = serde_json::to_string_pretty(measurements) {
        let _ = std::fs::write(&path, content);
        println!("\nRaw data saved to: tests/artifacts/s1_latency_measurements.json");
    }
}

/// Quick sanity check
#[tokio::test]
async fn test_s1_single_extraction_timing() {
    if !check_ollama_available() {
        println!("SKIPPED: Ollama not available");
        return;
    }

    let content = "# Quick Test\n\nThis is a simple document about JavaScript and React.";

    println!("Timing single extraction...");
    let (result, latency) = extract_with_timing(content).await;

    match result {
        Ok(r) => println!("Success: {}ms, {} concepts", latency, r.concepts.len()),
        Err(e) => println!("Failed: {}ms, error: {}", latency, e),
    }
}
