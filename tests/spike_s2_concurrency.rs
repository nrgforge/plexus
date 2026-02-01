//! Spike Experiment S2: Concurrency Testing
//!
//! **Research Question**: How does Ollama/llama3 7B handle concurrent extraction
//! requests on typical laptop hardware? What's the safe parallelism limit?
//!
//! **Method**:
//! - Fix a batch of 20 documents
//! - Run extraction at concurrency levels: 1, 2, 4, 6, 8
//! - Measure: total time, individual latencies, error rate
//! - Find throughput plateau and error threshold
//!
//! **Success Criteria**: Identify safe max_concurrent value
//!
//! Run with: `cargo test --test spike_s2_concurrency -- --nocapture`

mod common;

use common::TestCorpus;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionResult {
    concepts: Vec<serde_json::Value>,
    #[serde(default)]
    relationships: Vec<serde_json::Value>,
}

/// Result of a single extraction attempt
#[derive(Debug, Clone)]
struct ExtractionAttempt {
    doc_id: String,
    latency_ms: u64,
    success: bool,
    concept_count: usize,
    error: Option<String>,
}

/// Results for a concurrency level
#[derive(Debug, Clone)]
struct ConcurrencyResult {
    concurrency: usize,
    total_docs: usize,
    successful: usize,
    failed: usize,
    total_time_ms: u64,
    throughput_docs_per_min: f64,
    mean_latency_ms: f64,
    p50_latency_ms: u64,
    p95_latency_ms: u64,
    error_rate_pct: f64,
    errors: Vec<String>,
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

/// Extract with timing - no semaphore, raw extraction
async fn extract_single(doc_id: &str, content: &str) -> ExtractionAttempt {
    let config_dir = llm_orc_config_dir();
    let start = Instant::now();

    let child_result = Command::new("llm-orc")
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
        .spawn();

    let mut child = match child_result {
        Ok(c) => c,
        Err(e) => {
            return ExtractionAttempt {
                doc_id: doc_id.to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
                success: false,
                concept_count: 0,
                error: Some(format!("Spawn error: {}", e)),
            };
        }
    };

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(content.as_bytes()).await;
    }
    drop(child.stdin.take());

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            return ExtractionAttempt {
                doc_id: doc_id.to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
                success: false,
                concept_count: 0,
                error: Some(format!("Wait error: {}", e)),
            };
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return ExtractionAttempt {
            doc_id: doc_id.to_string(),
            latency_ms,
            success: false,
            concept_count: 0,
            error: Some(format!("Exit error: {}", stderr.chars().take(100).collect::<String>())),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    match parse_result(&stdout) {
        Ok(r) => ExtractionAttempt {
            doc_id: doc_id.to_string(),
            latency_ms,
            success: true,
            concept_count: r.concepts.len(),
            error: None,
        },
        Err(e) => ExtractionAttempt {
            doc_id: doc_id.to_string(),
            latency_ms,
            success: false,
            concept_count: 0,
            error: Some(e),
        },
    }
}

fn parse_result(text: &str) -> Result<ExtractionResult, String> {
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
        serde_json::from_str(json_str).map_err(|e| format!("Parse error: {}", e))
    } else {
        Err("No JSON found".to_string())
    }
}

// ============================================================================
// Concurrency Testing
// ============================================================================

/// Run extraction batch at specified concurrency level
async fn run_batch_at_concurrency(
    docs: &[(String, String)], // (id, content)
    concurrency: usize,
) -> ConcurrencyResult {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let completed = Arc::new(AtomicUsize::new(0));
    let total = docs.len();

    let start = Instant::now();

    // Use JoinSet for concurrent execution
    let mut join_set: JoinSet<ExtractionAttempt> = JoinSet::new();

    for (id, content) in docs.iter() {
        let sem = semaphore.clone();
        let comp = completed.clone();
        let doc_id = id.clone();
        let doc_content = content.clone();
        let total_docs = total;

        join_set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let result = extract_single(&doc_id, &doc_content).await;
            let done = comp.fetch_add(1, Ordering::SeqCst) + 1;
            print!("\r  Progress: {}/{} ", done, total_docs);
            std::io::Write::flush(&mut std::io::stdout()).ok();
            result
        });
    }

    // Collect all results
    let mut results: Vec<ExtractionAttempt> = Vec::new();
    while let Some(res) = join_set.join_next().await {
        if let Ok(attempt) = res {
            results.push(attempt);
        }
    }

    println!(); // newline after progress

    let total_time_ms = start.elapsed().as_millis() as u64;

    // Calculate statistics
    let successful: Vec<_> = results.iter().filter(|r| r.success).collect();
    let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();

    let latencies: Vec<u64> = successful.iter().map(|r| r.latency_ms).collect();

    let mean_latency = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
    };

    let (p50, p95) = if latencies.is_empty() {
        (0, 0)
    } else {
        let mut sorted = latencies.clone();
        sorted.sort();
        let n = sorted.len();
        let p50_idx = (0.5 * (n - 1) as f64).round() as usize;
        let p95_idx = (0.95 * (n - 1) as f64).round() as usize;
        (sorted[p50_idx.min(n - 1)], sorted[p95_idx.min(n - 1)])
    };

    let throughput = if total_time_ms > 0 {
        (results.len() as f64 / total_time_ms as f64) * 60000.0 // docs per minute
    } else {
        0.0
    };

    let error_rate = 100.0 * failed.len() as f64 / results.len() as f64;

    let errors: Vec<String> = failed
        .iter()
        .filter_map(|r| r.error.clone())
        .take(5) // Just first 5 unique errors
        .collect();

    ConcurrencyResult {
        concurrency,
        total_docs: results.len(),
        successful: successful.len(),
        failed: failed.len(),
        total_time_ms,
        throughput_docs_per_min: throughput,
        mean_latency_ms: mean_latency,
        p50_latency_ms: p50,
        p95_latency_ms: p95,
        error_rate_pct: error_rate,
        errors,
    }
}

// ============================================================================
// Main Test
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_s2_concurrency() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment S2: Concurrency Testing ===");
    println!("=== Research Question: 7B model performance on laptop hardware ===");
    println!("{}\n", "=".repeat(80));

    if !check_ollama_available() {
        println!("⚠️  SKIPPED: Ollama not available");
        return;
    }

    // Load corpus and select test batch
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");

    // Select 20 documents of varying sizes for the test batch
    let mut docs: Vec<(String, String)> = corpus
        .items
        .iter()
        .filter(|item| item.content.len() >= 100 && item.content.len() <= 5000)
        .take(20)
        .map(|item| (item.id.as_str().to_string(), item.content.clone()))
        .collect();

    if docs.len() < 20 {
        // Pad with more docs if needed
        for item in corpus.items.iter().filter(|i| i.content.len() >= 50) {
            if docs.len() >= 20 {
                break;
            }
            let id_str = item.id.as_str().to_string();
            if !docs.iter().any(|(id, _)| id == &id_str) {
                docs.push((id_str, item.content.clone()));
            }
        }
    }

    println!("Test batch: {} documents", docs.len());
    println!("Size range: {}B - {}B",
        docs.iter().map(|(_, c)| c.len()).min().unwrap_or(0),
        docs.iter().map(|(_, c)| c.len()).max().unwrap_or(0));

    // Test concurrency levels
    let concurrency_levels = [1, 2, 4, 6, 8];
    let mut results: Vec<ConcurrencyResult> = Vec::new();

    for &conc in &concurrency_levels {
        println!("\n--- Testing concurrency = {} ---", conc);

        // Small delay between runs to let Ollama settle
        if conc > 1 {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        let result = run_batch_at_concurrency(&docs, conc).await;

        println!("  Total time: {:.1}s", result.total_time_ms as f64 / 1000.0);
        println!("  Throughput: {:.1} docs/min", result.throughput_docs_per_min);
        println!("  Success rate: {}/{} ({:.1}%)",
            result.successful, result.total_docs, 100.0 - result.error_rate_pct);
        println!("  Mean latency: {:.0}ms", result.mean_latency_ms);

        results.push(result);
    }

    // Print comparison table
    println!("\n{}", "=".repeat(80));
    println!("=== Concurrency Comparison ===");
    println!("{}\n", "=".repeat(80));

    println!("{:>6} {:>10} {:>12} {:>12} {:>12} {:>10}",
        "Conc", "Time (s)", "Throughput", "Mean (ms)", "p95 (ms)", "Errors");
    println!("{}", "-".repeat(70));

    for r in &results {
        println!("{:>6} {:>10.1} {:>10.1}/m {:>12.0} {:>12} {:>9.1}%",
            r.concurrency,
            r.total_time_ms as f64 / 1000.0,
            r.throughput_docs_per_min,
            r.mean_latency_ms,
            r.p95_latency_ms,
            r.error_rate_pct);
    }

    // Calculate speedup relative to sequential
    println!("\n{}", "=".repeat(80));
    println!("=== Speedup Analysis ===");
    println!("{}\n", "=".repeat(80));

    let baseline = results.first().map(|r| r.total_time_ms).unwrap_or(1);

    println!("{:>6} {:>10} {:>12} {:>15}",
        "Conc", "Speedup", "Efficiency", "Marginal Gain");
    println!("{}", "-".repeat(50));

    let mut prev_time = baseline;
    for r in &results {
        let speedup = baseline as f64 / r.total_time_ms as f64;
        let efficiency = speedup / r.concurrency as f64 * 100.0;
        let marginal = if r.concurrency == 1 {
            "-".to_string()
        } else {
            format!("{:.1}%", (prev_time as f64 / r.total_time_ms as f64 - 1.0) * 100.0)
        };

        println!("{:>6} {:>10.2}x {:>11.0}% {:>15}",
            r.concurrency, speedup, efficiency, marginal);

        prev_time = r.total_time_ms;
    }

    // Find optimal concurrency
    println!("\n{}", "=".repeat(80));
    println!("=== Recommendation ===");
    println!("{}\n", "=".repeat(80));

    // Find best throughput with acceptable error rate (<30%)
    let acceptable_error_threshold = 30.0;
    let best = results.iter()
        .filter(|r| r.error_rate_pct < acceptable_error_threshold)
        .max_by(|a, b| a.throughput_docs_per_min.partial_cmp(&b.throughput_docs_per_min).unwrap());

    if let Some(best) = best {
        println!("Recommended max_concurrent: {}", best.concurrency);
        println!("  - Throughput: {:.1} docs/min", best.throughput_docs_per_min);
        println!("  - Error rate: {:.1}%", best.error_rate_pct);
        println!("  - Speedup vs sequential: {:.2}x", baseline as f64 / best.total_time_ms as f64);
    }

    // Check for diminishing returns
    let throughputs: Vec<f64> = results.iter().map(|r| r.throughput_docs_per_min).collect();
    if throughputs.len() >= 3 {
        let gain_2_to_4 = throughputs.get(2).unwrap_or(&0.0) / throughputs.get(1).unwrap_or(&1.0);
        let gain_4_to_8 = throughputs.get(4).unwrap_or(&0.0) / throughputs.get(2).unwrap_or(&1.0);

        if gain_4_to_8 < 1.2 {
            println!("\n⚠️  Diminishing returns beyond 4 workers ({:.0}% gain 4→8)",
                (gain_4_to_8 - 1.0) * 100.0);
        }
    }

    // Check for error spikes
    let error_spike = results.iter()
        .find(|r| r.error_rate_pct > acceptable_error_threshold);

    if let Some(spike) = error_spike {
        println!("\n⚠️  Error rate spikes at concurrency {} ({:.1}%)",
            spike.concurrency, spike.error_rate_pct);
        println!("   → Ollama may be hitting resource limits");
    }

    // Hardware context
    println!("\n--- Hardware Context ---");
    println!("Model: llama3 7B (via Ollama)");
    println!("Platform: Laptop (exact specs not measured)");
    println!("Note: Results are hardware-specific. GPU memory and");
    println!("      CPU cores significantly affect concurrency limits.");

    println!("\n{}", "=".repeat(80));
}

/// Quick test with just 2 concurrency levels
#[tokio::test]
#[ignore]
async fn test_s2_quick() {
    if !check_ollama_available() {
        println!("SKIPPED: Ollama not available");
        return;
    }

    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");

    let docs: Vec<(String, String)> = corpus
        .items
        .iter()
        .filter(|item| item.content.len() >= 100)
        .take(5)
        .map(|item| (item.id.as_str().to_string(), item.content.clone()))
        .collect();

    println!("Quick test with {} docs...", docs.len());

    for conc in [1, 2] {
        println!("\nConcurrency {}: ", conc);
        let result = run_batch_at_concurrency(&docs, conc).await;
        println!("  {:.1}s total, {:.1} docs/min, {:.1}% errors",
            result.total_time_ms as f64 / 1000.0,
            result.throughput_docs_per_min,
            result.error_rate_pct);
    }
}
