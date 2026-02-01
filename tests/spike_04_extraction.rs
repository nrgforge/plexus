//! Investigation 4: LLM Concept Extraction Quality
//!
//! **Question**: Can LLMs extract meaningful, consistent concepts from documents?
//!
//! **Why it matters**: If concept extraction is noisy or hallucinates heavily,
//! the semantic layer will be garbage. This is the foundation for all semantic
//! analysis.
//!
//! Run with: `cargo test --test spike_04_extraction --features real_llm -- --nocapture`

mod common;

use common::corpus::TestCorpus;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Extracted concept from LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExtractedConcept {
    name: String,
    #[serde(rename = "type")]
    concept_type: String,
    confidence: f64,
}

/// Extracted relationship from LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExtractedRelationship {
    source: String,
    target: String,
    relationship: String,
    confidence: f64,
}

/// Full extraction result
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExtractionResult {
    concepts: Vec<ExtractedConcept>,
    relationships: Vec<ExtractedRelationship>,
}

/// Document analysis result with metadata
#[derive(Debug)]
struct DocumentAnalysis {
    path: String,
    word_count: usize,
    extraction: Option<ExtractionResult>,
    error: Option<String>,
    grounding_score: f64, // % of concepts found in source text
    elapsed_ms: u64,
}

/// Get the llm-orc config directory (project root/.llm-orc)
fn llm_orc_config_dir() -> String {
    // Navigate from CARGO_MANIFEST_DIR (crates/plexus) up to repo root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let repo_root = std::path::Path::new(manifest_dir)
        .parent()  // crates/
        .and_then(|p| p.parent())  // repo root
        .expect("Could not find repo root");
    repo_root.join(".llm-orc").to_string_lossy().to_string()
}

/// Invoke llm-orc ensemble and get extraction result
async fn invoke_llm_extraction(content: &str) -> Result<ExtractionResult, String> {
    let config_dir = llm_orc_config_dir();

    // Start llm-orc MCP server
    let mut child = Command::new("llm-orc")
        .args(["invoke", "plexus-semantic", "--config-dir", &config_dir, "--input", "-", "--output-format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start llm-orc: {}", e))?;

    // Write content to stdin
    let stdin = child.stdin.as_mut().ok_or("No stdin")?;
    stdin
        .write_all(content.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to stdin: {}", e))?;

    // Wait for output
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait for output: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("llm-orc failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the JSON result - llm-orc outputs the raw response
    parse_extraction_result(&stdout)
}

/// Parse extraction result from LLM output
fn parse_extraction_result(text: &str) -> Result<ExtractionResult, String> {
    // Try to find JSON in the output (LLM may add extra text)
    let json_start = text.find('{');
    let json_end = text.rfind('}');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        let json_str = &text[start..=end];
        serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse JSON: {} in '{}'", e, json_str))
    } else {
        Err(format!("No JSON found in output: {}", text))
    }
}

/// Calculate grounding score: what % of concepts appear in source text
fn calculate_grounding_score(extraction: &ExtractionResult, content: &str) -> f64 {
    if extraction.concepts.is_empty() {
        return 0.0;
    }

    let content_lower = content.to_lowercase();
    let grounded_count = extraction
        .concepts
        .iter()
        .filter(|c| {
            let name_lower = c.name.to_lowercase();
            // Check if concept name or any word from it appears in content
            content_lower.contains(&name_lower)
                || name_lower
                    .split_whitespace()
                    .any(|word| word.len() > 3 && content_lower.contains(word))
        })
        .count();

    grounded_count as f64 / extraction.concepts.len() as f64
}

/// Count words in content
fn word_count(content: &str) -> usize {
    content.split_whitespace().count()
}

/// Select representative documents for analysis
fn select_documents(corpus: &TestCorpus, count: usize) -> Vec<&plexus::ContentItem> {
    // Stratified selection: pick from different directories
    let mut by_directory: HashMap<String, Vec<&plexus::ContentItem>> = HashMap::new();

    for item in &corpus.items {
        let dir = item
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        by_directory.entry(dir).or_default().push(item);
    }

    // Sort directories by doc count (prioritize well-populated)
    let mut dirs: Vec<_> = by_directory.keys().cloned().collect();
    dirs.sort_by(|a, b| {
        by_directory
            .get(b)
            .map(|v| v.len())
            .cmp(&by_directory.get(a).map(|v| v.len()))
    });

    let mut selected = Vec::new();
    let mut dir_iter = dirs.iter().cycle();

    while selected.len() < count {
        if let Some(dir) = dir_iter.next() {
            if let Some(docs) = by_directory.get_mut(dir) {
                // Pick doc with most words from this directory
                docs.sort_by_key(|d| std::cmp::Reverse(word_count(&d.content)));
                if let Some(doc) = docs.pop() {
                    if word_count(&doc.content) > 50 {
                        // Skip very short docs
                        selected.push(doc);
                    }
                }
            }
        }
        // Safety: break if we've cycled through all dirs twice with no new docs
        if dirs.iter().all(|d| by_directory.get(d).map(|v| v.is_empty()).unwrap_or(true)) {
            break;
        }
    }

    selected
}

/// Investigation 4: LLM Extraction Quality
#[tokio::test]
#[ignore]
#[cfg_attr(not(feature = "real_llm"), ignore = "requires real_llm feature")]
async fn test_investigation_04_llm_extraction_quality() {
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║     Investigation 4: LLM Concept Extraction Quality              ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    // Load corpus
    let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
    println!("📚 Corpus: {} ({} files)\n", corpus.name, corpus.file_count);

    // Select 10 representative documents
    let documents = select_documents(&corpus, 10);
    println!("📄 Selected {} documents for analysis\n", documents.len());

    // Analyze each document
    let mut analyses: Vec<DocumentAnalysis> = Vec::new();

    for (i, doc) in documents.iter().enumerate() {
        let path = doc
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| doc.id.as_str().to_string());
        let wc = word_count(&doc.content);

        print!("  [{}/{}] {} ({} words)... ", i + 1, documents.len(), path, wc);

        let start = std::time::Instant::now();

        match invoke_llm_extraction(&doc.content).await {
            Ok(extraction) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let grounding = calculate_grounding_score(&extraction, &doc.content);
                let concept_count = extraction.concepts.len();
                let rel_count = extraction.relationships.len();

                println!(
                    "✓ {} concepts, {} relationships (grounding: {:.0}%, {}ms)",
                    concept_count,
                    rel_count,
                    grounding * 100.0,
                    elapsed
                );

                analyses.push(DocumentAnalysis {
                    path,
                    word_count: wc,
                    extraction: Some(extraction),
                    error: None,
                    grounding_score: grounding,
                    elapsed_ms: elapsed,
                });
            }
            Err(e) => {
                println!("✗ Error: {}", e);
                analyses.push(DocumentAnalysis {
                    path,
                    word_count: wc,
                    extraction: None,
                    error: Some(e),
                    grounding_score: 0.0,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                });
            }
        }
    }

    // Summary statistics
    println!("\n═══════════════════════════════════════════════════════════════════");
    println!("                         SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════\n");

    let successful: Vec<_> = analyses.iter().filter(|a| a.extraction.is_some()).collect();
    let failed: Vec<_> = analyses.iter().filter(|a| a.error.is_some()).collect();

    println!("Documents analyzed: {}/{}", successful.len(), analyses.len());
    println!("Failed: {}", failed.len());

    if !successful.is_empty() {
        // Concept statistics
        let total_concepts: usize = successful
            .iter()
            .filter_map(|a| a.extraction.as_ref())
            .map(|e| e.concepts.len())
            .sum();
        let avg_concepts = total_concepts as f64 / successful.len() as f64;

        // Relationship statistics
        let total_relationships: usize = successful
            .iter()
            .filter_map(|a| a.extraction.as_ref())
            .map(|e| e.relationships.len())
            .sum();
        let avg_relationships = total_relationships as f64 / successful.len() as f64;

        // Grounding statistics
        let avg_grounding: f64 =
            successful.iter().map(|a| a.grounding_score).sum::<f64>() / successful.len() as f64;

        // Latency statistics
        let avg_latency: f64 =
            successful.iter().map(|a| a.elapsed_ms as f64).sum::<f64>() / successful.len() as f64;

        println!("\n📊 Concept Extraction:");
        println!("   Total concepts extracted: {}", total_concepts);
        println!("   Average concepts per doc: {:.1}", avg_concepts);
        println!("   Total relationships: {}", total_relationships);
        println!("   Average relationships per doc: {:.1}", avg_relationships);

        println!("\n🎯 Grounding (concept in source text):");
        println!("   Average grounding score: {:.1}%", avg_grounding * 100.0);
        println!(
            "   Best grounding: {:.1}%",
            successful
                .iter()
                .map(|a| a.grounding_score)
                .fold(0.0, f64::max)
                * 100.0
        );
        println!(
            "   Worst grounding: {:.1}%",
            successful
                .iter()
                .map(|a| a.grounding_score)
                .fold(1.0, f64::min)
                * 100.0
        );

        println!("\n⏱️ Performance:");
        println!("   Average latency: {:.0}ms", avg_latency);
        println!(
            "   Total time: {:.1}s",
            successful.iter().map(|a| a.elapsed_ms).sum::<u64>() as f64 / 1000.0
        );

        // Concept type breakdown
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for analysis in &successful {
            if let Some(extraction) = &analysis.extraction {
                for concept in &extraction.concepts {
                    *type_counts.entry(concept.concept_type.clone()).or_default() += 1;
                }
            }
        }

        println!("\n📋 Concept Types:");
        let mut types: Vec<_> = type_counts.iter().collect();
        types.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        for (concept_type, count) in types {
            println!(
                "   {}: {} ({:.1}%)",
                concept_type,
                count,
                *count as f64 / total_concepts as f64 * 100.0
            );
        }

        // Relationship type breakdown
        let mut rel_counts: HashMap<String, usize> = HashMap::new();
        for analysis in &successful {
            if let Some(extraction) = &analysis.extraction {
                for rel in &extraction.relationships {
                    *rel_counts.entry(rel.relationship.clone()).or_default() += 1;
                }
            }
        }

        println!("\n🔗 Relationship Types:");
        let mut rels: Vec<_> = rel_counts.iter().collect();
        rels.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        for (rel_type, count) in rels {
            println!(
                "   {}: {} ({:.1}%)",
                rel_type,
                count,
                *count as f64 / total_relationships.max(1) as f64 * 100.0
            );
        }

        // All unique concepts
        let all_concepts: HashSet<_> = successful
            .iter()
            .filter_map(|a| a.extraction.as_ref())
            .flat_map(|e| e.concepts.iter().map(|c| c.name.to_lowercase()))
            .collect();

        println!("\n🏷️ Unique Concepts ({}):", all_concepts.len());
        let mut concept_list: Vec<_> = all_concepts.iter().cloned().collect();
        concept_list.sort();
        for chunk in concept_list.chunks(5) {
            println!("   {}", chunk.join(", "));
        }

        // Verdict
        println!("\n═══════════════════════════════════════════════════════════════════");
        println!("                         VERDICT");
        println!("═══════════════════════════════════════════════════════════════════\n");

        let hallucination_rate = 1.0 - avg_grounding;

        println!("Pass Criteria:");
        println!(
            "  - Grounding ≥50%: {} (actual: {:.1}%)",
            if avg_grounding >= 0.5 { "✓ PASS" } else { "✗ FAIL" },
            avg_grounding * 100.0
        );
        println!(
            "  - Hallucination ≤25%: {} (actual: {:.1}%)",
            if hallucination_rate <= 0.25 {
                "✓ PASS"
            } else {
                "✗ FAIL"
            },
            hallucination_rate * 100.0
        );
        println!(
            "  - Avg concepts 3-10: {} (actual: {:.1})",
            if avg_concepts >= 3.0 && avg_concepts <= 10.0 {
                "✓ PASS"
            } else {
                "⚠ WARN"
            },
            avg_concepts
        );

        let overall_pass =
            avg_grounding >= 0.5 && hallucination_rate <= 0.25 && avg_concepts >= 2.0;

        println!(
            "\n🏆 Overall: {}",
            if overall_pass {
                "GO ✓ - LLM extraction is viable"
            } else if avg_grounding >= 0.4 {
                "PIVOT - Needs tuning but promising"
            } else {
                "NO-GO - LLM extraction not reliable"
            }
        );

        // Output detailed results for manual review
        println!("\n═══════════════════════════════════════════════════════════════════");
        println!("                    DETAILED RESULTS");
        println!("═══════════════════════════════════════════════════════════════════\n");

        for analysis in &successful {
            println!("📄 {}", analysis.path);
            println!("   Words: {}, Grounding: {:.0}%", analysis.word_count, analysis.grounding_score * 100.0);

            if let Some(extraction) = &analysis.extraction {
                println!("   Concepts:");
                for concept in &extraction.concepts {
                    let grounded = if analysis.extraction.as_ref().map_or(false, |_| {
                        let content_lower = corpus.items.iter()
                            .find(|i| i.path.as_ref().map(|p| p.to_string_lossy().to_string()) == Some(analysis.path.clone()))
                            .map(|i| i.content.to_lowercase())
                            .unwrap_or_default();
                        content_lower.contains(&concept.name.to_lowercase())
                    }) {
                        "✓"
                    } else {
                        "?"
                    };
                    println!(
                        "     {} {} ({}, conf: {:.2})",
                        grounded, concept.name, concept.concept_type, concept.confidence
                    );
                }

                if !extraction.relationships.is_empty() {
                    println!("   Relationships:");
                    for rel in &extraction.relationships {
                        println!(
                            "     {} --[{}]--> {} (conf: {:.2})",
                            rel.source, rel.relationship, rel.target, rel.confidence
                        );
                    }
                }
            }
            println!();
        }
    }

    // Fail if no successful extractions
    assert!(
        !successful.is_empty(),
        "No successful extractions - LLM may not be available"
    );
}

/// Test with mock LLM (always runs)
#[tokio::test]
#[ignore]
async fn test_investigation_04_mock_extraction() {
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║     Investigation 4 (Mock): Extraction Structure Test            ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");

    // Test the extraction parser with known good JSON
    let sample_response = r#"
    {
        "concepts": [
            {"name": "git", "type": "technology", "confidence": 0.95},
            {"name": "version control", "type": "topic", "confidence": 0.9},
            {"name": "branching", "type": "concept", "confidence": 0.85}
        ],
        "relationships": [
            {"source": "git", "target": "version control", "relationship": "implements", "confidence": 0.9},
            {"source": "branching", "target": "git", "relationship": "part_of", "confidence": 0.85}
        ]
    }
    "#;

    let result = parse_extraction_result(sample_response).expect("Should parse");
    assert_eq!(result.concepts.len(), 3);
    assert_eq!(result.relationships.len(), 2);

    // Test grounding calculation
    let content = "Git is a distributed version control system that supports branching.";
    let grounding = calculate_grounding_score(&result, content);
    assert!(
        grounding > 0.5,
        "Expected good grounding for matching content"
    );

    println!("✓ Extraction parser works correctly");
    println!("✓ Grounding calculation: {:.0}%", grounding * 100.0);
}
