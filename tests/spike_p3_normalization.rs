//! Spike Experiment P3: Normalization Ablation Study
//!
//! **Research Question**: What level of concept normalization is safe vs destructive?
//!
//! **Normalization Levels**:
//! - None: Exact string matching only
//! - Case-only: Lowercase normalization (validated in Experiment C)
//! - +Singular: Add singularization (promises → promise)
//! - +Semantic: Add semantic equivalence (JS → javascript)
//!
//! **Success Criteria**: Identify the safest effective normalization that:
//! - Maximizes legitimate concept merging
//! - Minimizes false merges (distinct concepts collapsed)
//!
//! Run with: `cargo test --test spike_p3_normalization -- --nocapture`

mod common;

use common::TestCorpus;
use serde::{Deserialize, Serialize};
use rand::rngs::StdRng;
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// ============================================================================
// Types
// ============================================================================

/// Extracted concept
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractedConcept {
    name: String,
    #[serde(rename = "type")]
    concept_type: Option<String>,
    confidence: f64,
}

/// Extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractionResult {
    concepts: Vec<ExtractedConcept>,
    #[serde(default)]
    relationships: Vec<serde_json::Value>,
}

/// A merge decision for evaluation
#[derive(Debug, Clone)]
struct MergeCandidate {
    concept_a: String,
    concept_b: String,
    normalized_form: String,
    normalization_level: String,
    docs_a: Vec<String>,
    docs_b: Vec<String>,
}

/// Merge judgment
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeJudgment {
    should_merge: bool,
    confidence: f64,
    reasoning: String,
}

/// Results for a normalization level
#[derive(Debug, Clone)]
struct NormalizationResult {
    level: String,
    unique_concepts_before: usize,
    unique_concepts_after: usize,
    merges_performed: usize,
    true_positives: usize,  // Correct merges
    false_positives: usize, // Incorrect merges (distinct concepts collapsed)
    precision: f64,
    compression_ratio: f64,
}

// ============================================================================
// Normalization Functions
// ============================================================================

/// No normalization - exact string
fn normalize_none(s: &str) -> String {
    s.to_string()
}

/// Case-only normalization
fn normalize_case(s: &str) -> String {
    s.to_lowercase()
}

/// Case + singularization
fn normalize_singular(s: &str) -> String {
    let lowered = s.to_lowercase();
    singularize(&lowered)
}

/// Case + singularization + semantic equivalence
fn normalize_semantic(s: &str) -> String {
    let singular = normalize_singular(s);
    semantic_normalize(&singular)
}

/// Simple singularization (heuristic-based)
fn singularize(s: &str) -> String {
    // Handle common patterns
    if s.ends_with("ies") && s.len() > 3 {
        return format!("{}y", &s[..s.len() - 3]);
    }
    if s.ends_with("es") && (s.ends_with("shes") || s.ends_with("ches") || s.ends_with("xes") || s.ends_with("sses")) {
        return s[..s.len() - 2].to_string();
    }
    if s.ends_with('s') && !s.ends_with("ss") && s.len() > 1 {
        // Don't singularize words that end in 's' naturally
        let exceptions = ["class", "process", "access", "address", "progress", "success",
                         "analysis", "basis", "crisis", "thesis", "hypothesis"];
        if !exceptions.contains(&s) {
            return s[..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// Semantic normalization - map common equivalences
fn semantic_normalize(s: &str) -> String {
    // Technology abbreviations
    let mappings = [
        ("js", "javascript"),
        ("ts", "typescript"),
        ("py", "python"),
        ("rb", "ruby"),
        ("cpp", "c++"),
        ("csharp", "c#"),
        ("golang", "go"),
        ("node.js", "nodejs"),
        ("node js", "nodejs"),
        ("react.js", "react"),
        ("reactjs", "react"),
        ("vue.js", "vue"),
        ("vuejs", "vue"),
        ("angular.js", "angular"),
        ("angularjs", "angular"),
        ("next.js", "nextjs"),
        ("nuxt.js", "nuxtjs"),
        // Common synonyms
        ("async", "asynchronous"),
        ("sync", "synchronous"),
        ("db", "database"),
        ("api", "application programming interface"),
        ("ui", "user interface"),
        ("ux", "user experience"),
        ("cli", "command line interface"),
        ("gui", "graphical user interface"),
        ("url", "uniform resource locator"),
        ("http", "hypertext transfer protocol"),
        ("https", "hypertext transfer protocol secure"),
        ("html", "hypertext markup language"),
        ("css", "cascading style sheets"),
        ("xml", "extensible markup language"),
        ("json", "javascript object notation"),
        ("yaml", "yet another markup language"),
        ("sql", "structured query language"),
        ("nosql", "not only sql"),
        ("orm", "object relational mapping"),
        ("mvc", "model view controller"),
        ("mvvm", "model view viewmodel"),
        ("rest", "representational state transfer"),
        ("soap", "simple object access protocol"),
        ("grpc", "google remote procedure call"),
        ("oauth", "open authorization"),
        ("jwt", "json web token"),
        ("aws", "amazon web services"),
        ("gcp", "google cloud platform"),
        ("ci/cd", "continuous integration continuous deployment"),
        ("devops", "development operations"),
    ];

    for (short, long) in &mappings {
        if s == *short {
            return long.to_string();
        }
        if s == *long {
            return short.to_string(); // Normalize to short form
        }
    }

    s.to_string()
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

/// Judge whether two concepts should be merged
async fn judge_merge(
    concept_a: &str,
    concept_b: &str,
    context_a: &str,
    context_b: &str,
) -> Result<MergeJudgment, String> {
    let config_dir = llm_orc_config_dir();

    // Create a merge-judge prompt
    let input = format!(
        r#"Should these two concepts be merged as semantically equivalent?

CONCEPT A: "{}"
Context: {}

CONCEPT B: "{}"
Context: {}

Consider:
1. Do they refer to the same underlying concept/entity?
2. Would merging them lose important distinctions?
3. In a knowledge graph, should they be the same node?

Return ONLY valid JSON:
{{"should_merge": true/false, "confidence": 0.0-1.0, "reasoning": "brief explanation"}}"#,
        concept_a, context_a, concept_b, context_b
    );

    let mut child = Command::new("llm-orc")
        .args([
            "invoke",
            "plexus-propagation-judge", // Reuse the judge ensemble
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
        .write_all(input.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait for output: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("llm-orc failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_merge_judgment(&stdout)
}

fn parse_merge_judgment(text: &str) -> Result<MergeJudgment, String> {
    // Try artifact format first
    if let Ok(artifact) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(response_str) = artifact
            .get("results")
            .and_then(|r| r.get("propagation-judge"))
            .and_then(|p| p.get("response"))
            .and_then(|r| r.as_str())
        {
            // The judge might return appropriate/confidence/reasoning format
            // Try to parse as MergeJudgment or adapt
            if let Ok(judgment) = serde_json::from_str::<MergeJudgment>(response_str) {
                return Ok(judgment);
            }
            // Try to adapt from propagation-judge format
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(response_str) {
                let should_merge = v.get("appropriate").and_then(|a| a.as_bool()).unwrap_or(false);
                let confidence = v.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.5);
                let reasoning = v.get("reasoning").and_then(|r| r.as_str()).unwrap_or("").to_string();
                return Ok(MergeJudgment { should_merge, confidence, reasoning });
            }
        }
    }

    // Fallback: find JSON in output
    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        if let Ok(judgment) = serde_json::from_str::<MergeJudgment>(json_str) {
            return Ok(judgment);
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            let should_merge = v.get("appropriate").and_then(|a| a.as_bool())
                .or_else(|| v.get("should_merge").and_then(|a| a.as_bool()))
                .unwrap_or(false);
            let confidence = v.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.5);
            let reasoning = v.get("reasoning").and_then(|r| r.as_str()).unwrap_or("").to_string();
            return Ok(MergeJudgment { should_merge, confidence, reasoning });
        }
    }

    Err(format!("Failed to parse merge judgment: {}", &text[..text.len().min(200)]))
}

/// Extract concepts using plexus-semantic
async fn extract_concepts_llm(content: &str) -> Result<ExtractionResult, String> {
    let config_dir = llm_orc_config_dir();

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

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("llm-orc failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_extraction_result(&stdout)
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
                .map_err(|e| format!("Failed to parse: {}", e));
        }
    }

    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    } else {
        Err("No JSON found".to_string())
    }
}

// ============================================================================
// Cache Management
// ============================================================================

fn extraction_cache_path() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .join("tests")
        .join("artifacts")
        .join("p3_extraction_cache.json")
}

fn load_extraction_cache() -> Option<HashMap<String, ExtractionResult>> {
    let path = extraction_cache_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

fn save_extraction_cache(cache: &HashMap<String, ExtractionResult>) {
    let path = extraction_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(&path, content);
    }
}

// ============================================================================
// Analysis
// ============================================================================

/// Find merge candidates at a given normalization level
fn find_merge_candidates(
    concepts_by_doc: &HashMap<String, Vec<ExtractedConcept>>,
    normalizer: fn(&str) -> String,
    level_name: &str,
) -> Vec<MergeCandidate> {
    // Group concepts by normalized form
    let mut normalized_groups: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for (doc, concepts) in concepts_by_doc {
        for concept in concepts {
            let normalized = normalizer(&concept.name);
            normalized_groups
                .entry(normalized.clone())
                .or_default()
                .push((concept.name.clone(), doc.clone()));
        }
    }

    // Find groups with multiple distinct originals
    let mut candidates = Vec::new();

    for (normalized, members) in normalized_groups {
        let unique_originals: HashSet<_> = members.iter().map(|(name, _)| name.clone()).collect();
        if unique_originals.len() > 1 {
            let originals: Vec<_> = unique_originals.into_iter().collect();
            for i in 0..originals.len() {
                for j in (i + 1)..originals.len() {
                    let docs_a: Vec<_> = members.iter()
                        .filter(|(n, _)| *n == originals[i])
                        .map(|(_, d)| d.clone())
                        .collect();
                    let docs_b: Vec<_> = members.iter()
                        .filter(|(n, _)| *n == originals[j])
                        .map(|(_, d)| d.clone())
                        .collect();

                    candidates.push(MergeCandidate {
                        concept_a: originals[i].clone(),
                        concept_b: originals[j].clone(),
                        normalized_form: normalized.clone(),
                        normalization_level: level_name.to_string(),
                        docs_a,
                        docs_b,
                    });
                }
            }
        }
    }

    candidates
}

/// Evaluate normalization level
async fn evaluate_normalization(
    concepts_by_doc: &HashMap<String, Vec<ExtractedConcept>>,
    corpus: &TestCorpus,
    normalizer: fn(&str) -> String,
    level_name: &str,
    sample_size: usize,
) -> NormalizationResult {
    println!("\n--- Evaluating: {} ---", level_name);

    // Count unique concepts before normalization
    let all_concepts: HashSet<_> = concepts_by_doc.values()
        .flat_map(|concepts| concepts.iter().map(|c| c.name.clone()))
        .collect();
    let unique_before = all_concepts.len();

    // Count unique concepts after normalization
    let normalized_concepts: HashSet<_> = all_concepts.iter()
        .map(|c| normalizer(c))
        .collect();
    let unique_after = normalized_concepts.len();

    let merges = unique_before - unique_after;
    let compression = if unique_before > 0 {
        100.0 * (1.0 - unique_after as f64 / unique_before as f64)
    } else {
        0.0
    };

    println!("  Concepts: {} → {} ({} merges, {:.1}% compression)",
        unique_before, unique_after, merges, compression);

    // Find merge candidates
    let candidates = find_merge_candidates(concepts_by_doc, normalizer, level_name);
    println!("  Merge candidates to evaluate: {}", candidates.len());

    // Sample and evaluate merges with LLM
    let mut rng: StdRng = rand::SeedableRng::seed_from_u64(42);
    use rand::seq::SliceRandom;
    let mut sampled: Vec<_> = candidates.iter().collect();
    sampled.shuffle(&mut rng);
    sampled.truncate(sample_size);

    let mut true_positives = 0;
    let mut false_positives = 0;

    for (i, candidate) in sampled.iter().enumerate() {
        // Get context for each concept
        let context_a = candidate.docs_a.first()
            .and_then(|d| {
                corpus.items.iter().find(|item| item.id.as_str() == *d)
                    .map(|item| {
                        let first_lines: String = item.content.lines().take(3).collect::<Vec<_>>().join(" ");
                        format!("From '{}': {}", d.split('/').last().unwrap_or(d), &first_lines[..first_lines.len().min(100)])
                    })
            })
            .unwrap_or_else(|| "Unknown context".to_string());

        let context_b = candidate.docs_b.first()
            .and_then(|d| {
                corpus.items.iter().find(|item| item.id.as_str() == *d)
                    .map(|item| {
                        let first_lines: String = item.content.lines().take(3).collect::<Vec<_>>().join(" ");
                        format!("From '{}': {}", d.split('/').last().unwrap_or(d), &first_lines[..first_lines.len().min(100)])
                    })
            })
            .unwrap_or_else(|| "Unknown context".to_string());

        print!("  [{}/{}] '{}' ↔ '{}': ", i + 1, sampled.len(), candidate.concept_a, candidate.concept_b);

        match judge_merge(&candidate.concept_a, &candidate.concept_b, &context_a, &context_b).await {
            Ok(judgment) => {
                if judgment.should_merge && judgment.confidence >= 0.7 {
                    println!("✓ MERGE (conf: {:.2})", judgment.confidence);
                    true_positives += 1;
                } else {
                    println!("✗ DISTINCT (conf: {:.2}) - {}", judgment.confidence, judgment.reasoning);
                    false_positives += 1;
                }
            }
            Err(e) => {
                println!("ERROR: {}", e);
            }
        }
    }

    let precision = if true_positives + false_positives > 0 {
        100.0 * true_positives as f64 / (true_positives + false_positives) as f64
    } else {
        100.0 // No merges = perfect precision (vacuously true)
    };

    NormalizationResult {
        level: level_name.to_string(),
        unique_concepts_before: unique_before,
        unique_concepts_after: unique_after,
        merges_performed: merges,
        true_positives,
        false_positives,
        precision,
        compression_ratio: compression,
    }
}

// ============================================================================
// Main Test
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_p3_normalization_ablation() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment P3: Normalization Ablation Study ===");
    println!("{}\n", "=".repeat(80));

    if !check_ollama_available() {
        println!("⚠️  SKIPPED: Ollama not available");
        return;
    }

    // Load corpus
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    println!("Loaded {} files from pkm-webdev", corpus.file_count);

    // Extract concepts from sample of documents
    let sample_size = 25;
    let mut concepts_by_doc: HashMap<String, Vec<ExtractedConcept>> = HashMap::new();

    // Load cache
    let mut cache = load_extraction_cache().unwrap_or_default();
    let cache_hits = cache.len();
    if cache_hits > 0 {
        println!("Cache loaded: {} documents", cache_hits);
    }

    // Sample documents
    let mut rng: StdRng = rand::SeedableRng::seed_from_u64(42);
    use rand::seq::SliceRandom;
    let mut items: Vec<_> = corpus.items.iter().collect();
    items.shuffle(&mut rng);

    println!("\n--- Extracting concepts from {} documents ---", sample_size);

    for (i, item) in items.iter().take(sample_size).enumerate() {
        let doc_path = item.id.as_str();
        let short_name = doc_path.split('/').last().unwrap_or(doc_path);

        if item.content.len() < 50 {
            continue;
        }

        print!("  [{}/{}] {}: ", i + 1, sample_size, short_name);

        // Check cache
        if let Some(cached) = cache.get(doc_path) {
            println!("{} concepts (cached)", cached.concepts.len());
            concepts_by_doc.insert(doc_path.to_string(), cached.concepts.clone());
            continue;
        }

        match extract_concepts_llm(&item.content).await {
            Ok(result) => {
                println!("{} concepts", result.concepts.len());
                cache.insert(doc_path.to_string(), result.clone());
                concepts_by_doc.insert(doc_path.to_string(), result.concepts);
            }
            Err(e) => {
                println!("FAILED: {}", e);
            }
        }
    }

    // Save cache
    save_extraction_cache(&cache);

    let total_concepts: usize = concepts_by_doc.values().map(|c| c.len()).sum();
    println!("\nExtracted {} total concepts from {} documents",
        total_concepts, concepts_by_doc.len());

    // Evaluate each normalization level
    println!("\n{}", "=".repeat(80));
    println!("=== Normalization Level Evaluation ===");
    println!("{}\n", "=".repeat(80));

    let merge_sample_size = 10; // LLM calls per level

    let results = vec![
        evaluate_normalization(&concepts_by_doc, &corpus, normalize_none, "none", merge_sample_size).await,
        evaluate_normalization(&concepts_by_doc, &corpus, normalize_case, "case-only", merge_sample_size).await,
        evaluate_normalization(&concepts_by_doc, &corpus, normalize_singular, "+singular", merge_sample_size).await,
        evaluate_normalization(&concepts_by_doc, &corpus, normalize_semantic, "+semantic", merge_sample_size).await,
    ];

    // Print comparison table
    println!("\n{}", "=".repeat(80));
    println!("=== Comparison ===");
    println!("{}\n", "=".repeat(80));

    println!("{:<15} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Level", "Before", "After", "Merges", "Precision", "Compression");
    println!("{}", "-".repeat(75));

    for r in &results {
        println!("{:<15} {:>10} {:>10} {:>10} {:>9.1}% {:>11.1}%",
            r.level, r.unique_concepts_before, r.unique_concepts_after,
            r.merges_performed, r.precision, r.compression_ratio);
    }

    // Analyze trade-offs
    println!("\n{}", "=".repeat(80));
    println!("=== Trade-off Analysis ===");
    println!("{}\n", "=".repeat(80));

    for r in &results {
        println!("\n{}", r.level.to_uppercase());

        // Assess quality
        if r.precision >= 90.0 {
            println!("  Precision: EXCELLENT ({:.1}% - minimal false merges)", r.precision);
        } else if r.precision >= 75.0 {
            println!("  Precision: GOOD ({:.1}% - acceptable false merge rate)", r.precision);
        } else if r.precision >= 50.0 {
            println!("  Precision: MARGINAL ({:.1}% - significant false merges)", r.precision);
        } else {
            println!("  Precision: POOR ({:.1}% - majority false merges)", r.precision);
        }

        // Assess compression
        if r.compression_ratio < 5.0 {
            println!("  Compression: MINIMAL ({:.1}% - few merges)", r.compression_ratio);
        } else if r.compression_ratio < 15.0 {
            println!("  Compression: MODERATE ({:.1}%)", r.compression_ratio);
        } else {
            println!("  Compression: HIGH ({:.1}% - aggressive merging)", r.compression_ratio);
        }

        // Risk assessment
        let risk_score = (100.0 - r.precision) * r.compression_ratio / 100.0;
        if risk_score < 1.0 {
            println!("  Risk: LOW (risk score: {:.2})", risk_score);
        } else if risk_score < 5.0 {
            println!("  Risk: MEDIUM (risk score: {:.2})", risk_score);
        } else {
            println!("  Risk: HIGH (risk score: {:.2})", risk_score);
        }
    }

    // Recommendation
    println!("\n{}", "=".repeat(80));
    println!("=== Recommendation ===");
    println!("{}\n", "=".repeat(80));

    // Find optimal level (highest precision with meaningful compression)
    let optimal = results.iter()
        .filter(|r| r.precision >= 75.0 && r.compression_ratio > 0.0)
        .max_by(|a, b| {
            // Score = precision * log(1 + compression)
            let score_a = a.precision * (1.0 + a.compression_ratio).ln();
            let score_b = b.precision * (1.0 + b.compression_ratio).ln();
            score_a.partial_cmp(&score_b).unwrap()
        });

    if let Some(best) = optimal {
        println!("RECOMMENDED: {} normalization", best.level);
        println!("  - Achieves {:.1}% compression with {:.1}% precision", best.compression_ratio, best.precision);
        println!("  - {} merges out of {} concepts", best.merges_performed, best.unique_concepts_before);
    } else {
        // Fall back to case-only (validated in Experiment C)
        println!("RECOMMENDED: case-only normalization (conservative default)");
        println!("  - Validated in Experiment C as safe");
        println!("  - More aggressive normalization showed precision issues");
    }

    // Safety warnings
    let semantic_result = results.iter().find(|r| r.level == "+semantic");
    if let Some(sem) = semantic_result {
        if sem.precision < 75.0 {
            println!("\n⚠️  WARNING: Semantic normalization has low precision ({:.1}%)", sem.precision);
            println!("   → Do NOT use semantic equivalence without manual review");
        }
    }

    println!("\n{}", "=".repeat(80));
}

/// Test singularization function
#[test]
#[ignore]
fn test_singularization() {
    assert_eq!(singularize("promises"), "promise");
    assert_eq!(singularize("classes"), "class"); // Exception
    assert_eq!(singularize("processes"), "process"); // Exception
    assert_eq!(singularize("boxes"), "box");
    assert_eq!(singularize("queries"), "query");
    assert_eq!(singularize("analyses"), "analysis"); // Exception
    assert_eq!(singularize("hooks"), "hook");
    assert_eq!(singularize("components"), "component");
}

/// Test semantic normalization
#[test]
#[ignore]
fn test_semantic_normalization() {
    assert_eq!(semantic_normalize("js"), "javascript");
    assert_eq!(semantic_normalize("ts"), "typescript");
    assert_eq!(semantic_normalize("api"), "application programming interface");
    assert_eq!(semantic_normalize("reactjs"), "react");
}
