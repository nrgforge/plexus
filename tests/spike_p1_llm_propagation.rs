//! Spike Experiment P1: Propagation Parameter Sweep with LLM Judgment
//!
//! **Research Question**: What propagation parameters (decay, hops, threshold)
//! optimize concept spreading, measured by semantic appropriateness?
//!
//! This test scales Investigation 6's methodology by using LLM to judge whether
//! propagations "make sense" rather than measuring exact concept overlap.
//!
//! **Key Insight**: Judgment is parameter-independent! We:
//! 1. Generate all propagation pairs at max hops
//! 2. Sample ~50 pairs and judge them ONCE with LLM
//! 3. Sweep parameters mathematically on the pre-judged pairs
//!
//! Run with: `cargo test --test spike_p1_llm_propagation -- --nocapture`

mod common;

use common::{build_structure_graph, TestCorpus};
use plexus::{Context, NodeId, PropertyValue};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// ============================================================================
// Types
// ============================================================================

/// Result of judging a propagation pair
#[derive(Debug, Clone, Deserialize, Serialize)]
struct JudgmentResult {
    appropriate: bool,
    confidence: f64,
    reasoning: String,
}

/// A propagation pair to be judged
#[derive(Debug, Clone)]
struct PropagationPair {
    source_doc: NodeId,
    source_name: String,
    target_doc: NodeId,
    target_name: String,
    concept: String,
    concept_confidence: f64,
    hop_distance: usize,
    edge_weight: f64,
    relationship: String,
}

/// Concept extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExtractedConcept {
    name: String,
    confidence: f64,
}

/// Results for a parameter combination
#[derive(Debug, Clone)]
struct ParamResult {
    decay: f64,
    threshold: f64,
    max_hops: usize,
    pairs_included: usize,
    appropriate_count: usize,
    appropriate_pct: f64,
    avg_confidence: f64,
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

/// Invoke the propagation-judge ensemble
async fn judge_propagation(
    source_desc: &str,
    target_desc: &str,
    concept: &str,
    relationship: &str,
) -> Result<JudgmentResult, String> {
    let config_dir = llm_orc_config_dir();

    let input = format!(
        "SOURCE_DOC: {}\nTARGET_DOC: {}\nCONCEPT: {}\nRELATIONSHIP: {}",
        source_desc, target_desc, concept, relationship
    );

    let mut child = Command::new("llm-orc")
        .args([
            "invoke",
            "plexus-propagation-judge",
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
    drop(child.stdin.take()); // Close stdin to signal EOF

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait for output: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("llm-orc failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_judgment(&stdout)
}

/// Parse judgment result from LLM output
fn parse_judgment(text: &str) -> Result<JudgmentResult, String> {
    // llm-orc returns full artifact JSON with response nested inside
    // Structure: { "results": { "propagation-judge": { "response": "{...json...}" } } }

    // First try to parse as artifact format
    if let Ok(artifact) = serde_json::from_str::<serde_json::Value>(text) {
        // Navigate to the nested response
        if let Some(response_str) = artifact
            .get("results")
            .and_then(|r| r.get("propagation-judge"))
            .and_then(|p| p.get("response"))
            .and_then(|r| r.as_str())
        {
            // Parse the inner JSON response
            return serde_json::from_str(response_str)
                .map_err(|e| format!("Failed to parse inner response: {} in '{}'", e, response_str));
        }
    }

    // Fallback: try to find raw JSON in output (for direct responses)
    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON: {} in '{}'", e, &json_str[..json_str.len().min(200)]))
    } else {
        Err(format!("No JSON found in output: {}", &text[..text.len().min(200)]))
    }
}

// ============================================================================
// Concept Extraction (Real LLM via plexus-semantic ensemble)
// ============================================================================

/// LLM extraction result from plexus-semantic ensemble
#[derive(Debug, Clone, Deserialize, Serialize)]
struct SemanticExtractionResult {
    concepts: Vec<SemanticConcept>,
    #[serde(default)]
    relationships: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SemanticConcept {
    name: String,
    #[serde(rename = "type")]
    concept_type: Option<String>,
    confidence: f64,
}

/// Cache file path for extracted concepts
fn extraction_cache_path() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .join("tests")
        .join("artifacts")
        .join("p1_extraction_cache.json")
}

/// Load cached extractions if available
fn load_extraction_cache() -> Option<HashMap<String, Vec<ExtractedConcept>>> {
    let path = extraction_cache_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Save extractions to cache
fn save_extraction_cache(cache: &HashMap<String, Vec<ExtractedConcept>>) {
    let path = extraction_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(&path, content);
    }
}

/// Extract concepts from document using plexus-semantic ensemble
async fn extract_concepts_llm(content: &str) -> Result<Vec<ExtractedConcept>, String> {
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

/// Parse extraction result from LLM output
fn parse_extraction_result(text: &str) -> Result<Vec<ExtractedConcept>, String> {
    // llm-orc returns artifact JSON with response nested inside
    if let Ok(artifact) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(response_str) = artifact
            .get("results")
            .and_then(|r| r.get("semantic-analyzer"))
            .and_then(|p| p.get("response"))
            .and_then(|r| r.as_str())
        {
            // Parse the inner JSON response
            if let Ok(result) = serde_json::from_str::<SemanticExtractionResult>(response_str) {
                return Ok(result
                    .concepts
                    .into_iter()
                    .map(|c| ExtractedConcept {
                        name: c.name.to_lowercase(),
                        confidence: c.confidence,
                    })
                    .collect());
            }
        }
    }

    // Fallback: try to find raw JSON
    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        if let Ok(result) = serde_json::from_str::<SemanticExtractionResult>(json_str) {
            return Ok(result
                .concepts
                .into_iter()
                .map(|c| ExtractedConcept {
                    name: c.name.to_lowercase(),
                    confidence: c.confidence,
                })
                .collect());
        }
    }

    Err(format!(
        "Failed to parse extraction result: {}",
        &text[..text.len().min(200)]
    ))
}

/// Extract concepts from document content using simple heuristics (fallback)
#[allow(dead_code)]
fn extract_concepts_mock(content: &str) -> Vec<ExtractedConcept> {
    let mut concepts = Vec::new();

    // Extract from headers (high confidence)
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            let name = trimmed.trim_start_matches("# ").trim().to_lowercase();
            if !name.is_empty() && name.len() < 50 {
                concepts.push(ExtractedConcept { name, confidence: 1.0 });
            }
        } else if trimmed.starts_with("## ") {
            let name = trimmed.trim_start_matches("## ").trim().to_lowercase();
            if !name.is_empty() && name.len() < 50 {
                concepts.push(ExtractedConcept { name, confidence: 0.9 });
            }
        }
    }

    // Extract technology keywords
    let tech_keywords = [
        "javascript", "typescript", "react", "node", "git", "docker",
        "python", "rust", "css", "html", "api", "database", "async",
        "promise", "function", "component", "hook", "state", "http",
    ];

    let content_lower = content.to_lowercase();
    for keyword in tech_keywords {
        if content_lower.contains(keyword) {
            // Avoid duplicates
            if !concepts.iter().any(|c| c.name.contains(keyword)) {
                concepts.push(ExtractedConcept {
                    name: keyword.to_string(),
                    confidence: 0.7,
                });
            }
        }
    }

    concepts
}

/// Get a brief description of a document for LLM judgment
fn get_doc_description(corpus: &TestCorpus, doc_path: &str) -> String {
    let item = corpus.items.iter().find(|i| i.id.as_str() == doc_path);
    if let Some(item) = item {
        // Use first line (usually the title) and path as description
        let first_line = item.content.lines().next().unwrap_or("").trim();
        let title = first_line.trim_start_matches('#').trim();
        format!("{} ({})", title, doc_path)
    } else {
        doc_path.to_string()
    }
}

// ============================================================================
// Graph Traversal
// ============================================================================

/// Build adjacency list for BFS traversal
fn build_adjacency(context: &Context) -> HashMap<NodeId, Vec<(NodeId, f64, String)>> {
    let mut adj: HashMap<NodeId, Vec<(NodeId, f64, String)>> = HashMap::new();

    for (node_id, node) in &context.nodes {
        if node.node_type == "document" {
            adj.insert(node_id.clone(), Vec::new());
        }
    }

    for edge in &context.edges {
        let src = &edge.source;
        let tgt = &edge.target;

        let src_is_doc = context.nodes.get(src).map(|n| n.node_type == "document").unwrap_or(false);
        let tgt_is_doc = context.nodes.get(tgt).map(|n| n.node_type == "document").unwrap_or(false);

        if !src_is_doc || !tgt_is_doc {
            continue;
        }

        // Weight by edge type (sibling edges weighted higher per spike findings)
        let weight = match edge.relationship.as_str() {
            "sibling" => 1.0,
            "links_to" => 0.5,
            "linked_from" => 0.3,
            _ => 0.2,
        };

        adj.entry(src.clone())
            .or_default()
            .push((tgt.clone(), weight, edge.relationship.clone()));
    }

    adj
}

/// Generate all propagation pairs up to max_hops
fn generate_propagation_pairs(
    context: &Context,
    corpus: &TestCorpus,
    adj: &HashMap<NodeId, Vec<(NodeId, f64, String)>>,
    doc_concepts: &HashMap<NodeId, Vec<ExtractedConcept>>,
    max_hops: usize,
) -> Vec<PropagationPair> {
    let mut pairs = Vec::new();

    for (source_id, concepts) in doc_concepts {
        if concepts.is_empty() {
            continue;
        }

        let source_path = context.nodes.get(source_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");

        // BFS to find all reachable documents within max_hops
        let mut queue: VecDeque<(NodeId, usize, f64, String)> = VecDeque::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        queue.push_back((source_id.clone(), 0, 1.0, "source".to_string()));
        visited.insert(source_id.clone());

        while let Some((current, hops, path_weight, last_rel)) = queue.pop_front() {
            if hops >= max_hops {
                continue;
            }

            if let Some(neighbors) = adj.get(&current) {
                for (neighbor, edge_weight, rel) in neighbors {
                    if visited.contains(neighbor) {
                        continue;
                    }
                    visited.insert(neighbor.clone());

                    let new_weight = path_weight * edge_weight;
                    let target_path = context.nodes.get(neighbor)
                        .and_then(|n| n.properties.get("source"))
                        .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
                        .unwrap_or("");

                    // Create pairs for each concept
                    for concept in concepts {
                        pairs.push(PropagationPair {
                            source_doc: source_id.clone(),
                            source_name: get_doc_description(corpus, source_path),
                            target_doc: neighbor.clone(),
                            target_name: get_doc_description(corpus, target_path),
                            concept: concept.name.clone(),
                            concept_confidence: concept.confidence,
                            hop_distance: hops + 1,
                            edge_weight: new_weight,
                            relationship: rel.clone(),
                        });
                    }

                    queue.push_back((neighbor.clone(), hops + 1, new_weight, rel.clone()));
                }
            }
        }
    }

    pairs
}

/// Sample pairs with stratification by hop distance
fn sample_pairs(pairs: &[PropagationPair], n: usize, seed: u64) -> Vec<&PropagationPair> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

    // Group by hop distance
    let mut by_hop: HashMap<usize, Vec<&PropagationPair>> = HashMap::new();
    for pair in pairs {
        by_hop.entry(pair.hop_distance).or_default().push(pair);
    }

    // Sample proportionally: 50% hop=1, 30% hop=2, 20% hop=3+
    let mut sampled = Vec::new();

    if let Some(hop1) = by_hop.get(&1) {
        let count = (n as f64 * 0.5).ceil() as usize;
        let mut hop1_vec: Vec<_> = hop1.iter().cloned().collect();
        hop1_vec.shuffle(&mut rng);
        sampled.extend(hop1_vec.into_iter().take(count));
    }

    if let Some(hop2) = by_hop.get(&2) {
        let count = (n as f64 * 0.3).ceil() as usize;
        let mut hop2_vec: Vec<_> = hop2.iter().cloned().collect();
        hop2_vec.shuffle(&mut rng);
        sampled.extend(hop2_vec.into_iter().take(count));
    }

    // Hop 3+
    let hop3plus: Vec<_> = by_hop.iter()
        .filter(|(h, _)| **h >= 3)
        .flat_map(|(_, v)| v.iter().cloned())
        .collect();
    if !hop3plus.is_empty() {
        let count = (n as f64 * 0.2).ceil() as usize;
        let mut hop3_vec = hop3plus;
        hop3_vec.shuffle(&mut rng);
        sampled.extend(hop3_vec.into_iter().take(count));
    }

    // If we didn't get enough, fill from remaining
    if sampled.len() < n {
        let sampled_set: HashSet<_> = sampled.iter().map(|p| (&p.source_doc, &p.target_doc, &p.concept)).collect();
        let remaining: Vec<_> = pairs.iter()
            .filter(|p| !sampled_set.contains(&(&p.source_doc, &p.target_doc, &p.concept)))
            .collect();
        let mut remaining = remaining;
        remaining.shuffle(&mut rng);
        sampled.extend(remaining.into_iter().take(n - sampled.len()));
    }

    sampled.truncate(n);
    sampled
}

/// Calculate propagated confidence for a pair given parameters
fn propagated_confidence(pair: &PropagationPair, decay: f64) -> f64 {
    pair.concept_confidence * decay.powi(pair.hop_distance as i32) * pair.edge_weight
}

// ============================================================================
// Main Test
// ============================================================================

#[tokio::test]
async fn test_p1_llm_propagation_sweep() {
    println!("\n{}", "=".repeat(80));
    println!("=== Experiment P1: Propagation Parameter Sweep ===");
    println!("=== (Real LLM Extraction + LLM Judgment) ===");
    println!("{}\n", "=".repeat(80));

    // Check Ollama availability
    if !check_ollama_available() {
        println!("⚠️  SKIPPED: Ollama not available");
        println!("   To run this test, ensure Ollama is running with llama3 model.");
        return;
    }

    // Load corpus and build graph
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    let graph = build_structure_graph(&corpus).await.expect("Failed to build graph");
    let context = &graph.context;
    let adj = build_adjacency(context);

    let doc_count = context.nodes.values().filter(|n| n.node_type == "document").count();
    println!("Corpus loaded: {} documents", doc_count);

    // Extract concepts for all documents (with caching)
    let doc_ids: Vec<NodeId> = context.nodes.iter()
        .filter(|(_, n)| n.node_type == "document")
        .map(|(id, _)| id.clone())
        .collect();

    // Load cache if available
    let mut extraction_cache = load_extraction_cache().unwrap_or_default();
    let cache_hits = extraction_cache.len();

    println!("\n--- Concept Extraction (Real LLM) ---");
    if cache_hits > 0 {
        println!("Cache loaded: {} documents already extracted", cache_hits);
    }

    let mut doc_concepts: HashMap<NodeId, Vec<ExtractedConcept>> = HashMap::new();
    let mut extracted_count = 0;
    let mut failed_count = 0;

    for (i, doc_id) in doc_ids.iter().enumerate() {
        let source_path = context.nodes.get(doc_id)
            .and_then(|n| n.properties.get("source"))
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or("");

        // Check cache first
        if let Some(cached) = extraction_cache.get(source_path) {
            doc_concepts.insert(doc_id.clone(), cached.clone());
            continue;
        }

        let content = corpus.items.iter()
            .find(|item| item.id.as_str() == source_path)
            .map(|item| item.content.as_str())
            .unwrap_or("");

        // Skip empty or very short documents
        if content.len() < 50 {
            doc_concepts.insert(doc_id.clone(), vec![]);
            continue;
        }

        print!("  [{}/{}] Extracting {}: ", i + 1, doc_count,
            source_path.split('/').last().unwrap_or(source_path));

        match extract_concepts_llm(content).await {
            Ok(concepts) => {
                println!("{} concepts", concepts.len());
                extraction_cache.insert(source_path.to_string(), concepts.clone());
                doc_concepts.insert(doc_id.clone(), concepts);
                extracted_count += 1;
            }
            Err(e) => {
                println!("FAILED: {}", e);
                doc_concepts.insert(doc_id.clone(), vec![]);
                failed_count += 1;
            }
        }
    }

    // Save updated cache
    save_extraction_cache(&extraction_cache);

    let total_concepts: usize = doc_concepts.values().map(|c| c.len()).sum();
    println!("\nExtraction complete:");
    println!("  - From cache: {}", cache_hits);
    println!("  - Newly extracted: {}", extracted_count);
    println!("  - Failed: {}", failed_count);
    println!("  - Total concepts: {} across {} documents", total_concepts, doc_concepts.len());

    // Generate all propagation pairs
    let max_hops = 4;
    let all_pairs = generate_propagation_pairs(context, &corpus, &adj, &doc_concepts, max_hops);
    println!("Propagation pairs generated: {}", all_pairs.len());

    // Sample pairs for LLM judgment
    let sample_size = 50;
    let sampled = sample_pairs(&all_pairs, sample_size, 42);
    println!("Sampled {} pairs for LLM judgment\n", sampled.len());

    // Judge sampled pairs
    println!("Judging pairs with LLM...");
    let mut judgments: HashMap<(String, String, String), JudgmentResult> = HashMap::new();

    for (i, pair) in sampled.iter().enumerate() {
        let key = (pair.source_name.clone(), pair.target_name.clone(), pair.concept.clone());

        print!("  [{}/{}] {} -> {} ({}): ", i + 1, sampled.len(),
            pair.source_name.split('/').last().unwrap_or(&pair.source_name),
            pair.target_name.split('/').last().unwrap_or(&pair.target_name),
            pair.concept);

        match judge_propagation(&pair.source_name, &pair.target_name, &pair.concept, &pair.relationship).await {
            Ok(result) => {
                println!("{} (conf: {:.2})", if result.appropriate { "✓" } else { "✗" }, result.confidence);
                judgments.insert(key, result);
            }
            Err(e) => {
                println!("ERROR: {}", e);
            }
        }
    }

    let judged_count = judgments.len();
    println!("\nJudged {}/{} pairs successfully", judged_count, sampled.len());

    // Parameter sweep
    println!("\n{}", "=".repeat(80));
    println!("=== Parameter Sweep Results ===\n");

    let decays = [0.5, 0.6, 0.7, 0.8, 0.9];
    let thresholds = [0.3, 0.4, 0.5, 0.6, 0.7];
    let hop_limits = [1, 2, 3, 4];

    let mut results: Vec<ParamResult> = Vec::new();

    for &decay in &decays {
        for &threshold in &thresholds {
            for &max_h in &hop_limits {
                let mut pairs_included = 0;
                let mut appropriate_count = 0;
                let mut total_confidence = 0.0;

                for pair in sampled.iter() {
                    // Check if pair is within hop limit
                    if pair.hop_distance > max_h {
                        continue;
                    }

                    // Check if propagated confidence meets threshold
                    let prop_conf = propagated_confidence(pair, decay);
                    if prop_conf < threshold {
                        continue;
                    }

                    // Look up judgment
                    let key = (pair.source_name.clone(), pair.target_name.clone(), pair.concept.clone());
                    if let Some(judgment) = judgments.get(&key) {
                        pairs_included += 1;
                        if judgment.appropriate && judgment.confidence >= 0.7 {
                            appropriate_count += 1;
                        }
                        total_confidence += judgment.confidence;
                    }
                }

                if pairs_included > 0 {
                    results.push(ParamResult {
                        decay,
                        threshold,
                        max_hops: max_h,
                        pairs_included,
                        appropriate_count,
                        appropriate_pct: 100.0 * appropriate_count as f64 / pairs_included as f64,
                        avg_confidence: total_confidence / pairs_included as f64,
                    });
                }
            }
        }
    }

    // Sort by appropriate %
    results.sort_by(|a, b| b.appropriate_pct.partial_cmp(&a.appropriate_pct).unwrap());

    // Print top results
    println!("{:<8} {:>10} {:>8} {:>12} {:>12} {:>12}",
        "Decay", "Threshold", "MaxHops", "Pairs", "Appropriate", "Avg Conf");
    println!("{}", "-".repeat(70));

    for r in results.iter().take(20) {
        println!("{:<8.2} {:>10.2} {:>8} {:>12} {:>11.1}% {:>12.2}",
            r.decay, r.threshold, r.max_hops, r.pairs_included, r.appropriate_pct, r.avg_confidence);
    }

    // Find best parameters
    println!("\n{}", "=".repeat(80));
    println!("=== Best Parameters ===\n");

    if let Some(best) = results.first() {
        println!("Best by appropriate %: decay={:.1}, threshold={:.1}, hops={} -> {:.1}%",
            best.decay, best.threshold, best.max_hops, best.appropriate_pct);
    }

    // Compare with assumed parameters
    if let Some(assumed) = results.iter().find(|r|
        (r.decay - 0.7).abs() < 0.01 && (r.threshold - 0.5).abs() < 0.01 && r.max_hops == 3
    ) {
        println!("Assumed (0.7, 0.5, 3): {:.1}% appropriate ({} pairs)",
            assumed.appropriate_pct, assumed.pairs_included);
    }

    // Summary statistics
    let overall_appropriate = judgments.values().filter(|j| j.appropriate).count();
    let overall_pct = 100.0 * overall_appropriate as f64 / judgments.len() as f64;
    println!("\nOverall (all judged pairs): {}/{} appropriate ({:.1}%)",
        overall_appropriate, judgments.len(), overall_pct);

    // Verdict
    println!("\n{}", "=".repeat(80));
    let best_pct = results.first().map(|r| r.appropriate_pct).unwrap_or(0.0);
    if best_pct >= 80.0 {
        println!("VERDICT: GO ✓ - Found parameters achieving ≥80% appropriate");
    } else if best_pct >= 67.0 {
        println!("VERDICT: MATCHES INVESTIGATION 6 - {:.1}% appropriate (target was 67%)", best_pct);
    } else {
        println!("VERDICT: NEEDS INVESTIGATION - Best {:.1}% < 67% target", best_pct);
    }
    println!("{}", "=".repeat(80));
}

/// Quick test to verify ensemble works
#[tokio::test]
async fn test_ensemble_connectivity() {
    if !check_ollama_available() {
        println!("SKIPPED: Ollama not available");
        return;
    }

    println!("Testing propagation-judge ensemble...");

    let result = judge_propagation(
        "Document about Git version control",
        "Document about branching strategies",
        "git",
        "sibling"
    ).await;

    match result {
        Ok(j) => {
            println!("Success! appropriate={}, confidence={:.2}, reasoning: {}",
                j.appropriate, j.confidence, j.reasoning);
            // We expect this to be appropriate
            assert!(j.confidence > 0.5, "Expected reasonable confidence");
        }
        Err(e) => {
            panic!("Ensemble failed: {}", e);
        }
    }
}
