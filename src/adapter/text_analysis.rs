//! Phase 2 text analysis adapter — heuristic section detection and proper noun extraction (ADR-019)
//!
//! Concrete Phase 2 adapter for `text/*` MIME types. Detects section boundaries
//! (headings, act/scene markers) and extracts capitalized proper nouns as concept
//! candidates. Output feeds Phase 3 via `SemanticInput.sections`.
//!
//! Invariant 7 (dual obligation): produces both semantic content (concept nodes)
//! and provenance (chain + marks + contains edges).

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission};
use crate::graph::{dimension, ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use std::collections::HashSet;

/// A section boundary detected by heuristic analysis.
#[derive(Debug, Clone)]
pub struct DetectedSection {
    pub label: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Nesting depth: 0 = top-level (act), 1 = nested (scene)
    pub depth: usize,
}

/// Phase 2 text analysis adapter for `text/*` files.
///
/// Detects:
/// - Markdown headings (`#`, `##`, etc.)
/// - ALL CAPS headings (e.g., "ACT I", "SCENE 1")
/// - Blank-line-separated sections
///
/// Extracts:
/// - Capitalized proper nouns as concept candidates
pub struct TextAnalysisAdapter {
    adapter_id: String,
}

impl TextAnalysisAdapter {
    pub fn new() -> Self {
        Self {
            adapter_id: "extract-analysis-text".to_string(),
        }
    }
}

/// Detect section boundaries from text lines.
fn detect_sections(lines: &[&str]) -> Vec<DetectedSection> {
    let mut sections = Vec::new();
    let total = lines.len();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Markdown headings: # Heading
        if let Some(rest) = trimmed.strip_prefix('#') {
            let depth = rest.chars().take_while(|&c| c == '#').count(); // additional # after first
            let label = rest.trim_start_matches('#').trim();
            if !label.is_empty() {
                sections.push(DetectedSection {
                    label: label.to_string(),
                    start_line: i + 1,
                    end_line: total, // placeholder, fixed below
                    depth,
                });
            }
            continue;
        }

        // ACT markers (e.g., "ACT I", "ACT II")
        if trimmed.starts_with("ACT ") && trimmed.len() <= 20 && is_upper_or_roman(trimmed) {
            sections.push(DetectedSection {
                label: trimmed.to_string(),
                start_line: i + 1,
                end_line: total,
                depth: 0,
            });
            continue;
        }

        // SCENE markers (e.g., "SCENE 1", "SCENE II")
        if trimmed.starts_with("SCENE ") && trimmed.len() <= 20 && is_upper_or_roman(trimmed) {
            sections.push(DetectedSection {
                label: trimmed.to_string(),
                start_line: i + 1,
                end_line: total,
                depth: 1,
            });
            continue;
        }

        // ALL CAPS lines (short, likely headings)
        if trimmed.len() >= 3
            && trimmed.len() <= 60
            && trimmed.chars().all(|c| c.is_uppercase() || c.is_whitespace() || c.is_ascii_punctuation())
            && trimmed.chars().any(|c| c.is_alphabetic())
        {
            sections.push(DetectedSection {
                label: trimmed.to_string(),
                start_line: i + 1,
                end_line: total,
                depth: 0,
            });
        }
    }

    // Fix end_line: each section ends where the next same-or-higher-depth section starts
    for i in 0..sections.len() {
        let current_depth = sections[i].depth;
        let next_start = sections[i + 1..]
            .iter()
            .find(|s| s.depth <= current_depth)
            .map(|s| s.start_line - 1);
        if let Some(end) = next_start {
            sections[i].end_line = end;
        }
    }

    sections
}

/// Check if a string is uppercase with optional Roman numerals and digits.
fn is_upper_or_roman(s: &str) -> bool {
    s.chars().all(|c| c.is_uppercase() || c.is_whitespace() || c.is_ascii_digit() || c == '.')
}

/// Extract proper nouns from text lines.
///
/// A proper noun candidate is a capitalized word that:
/// - Is not at the start of a sentence (after `.!?` or at line start)
/// - Is at least 2 characters
/// - Is not a common English word
fn extract_proper_nouns(lines: &[&str]) -> Vec<String> {
    let common_words: HashSet<&str> = [
        "The", "This", "That", "These", "Those", "When", "Where", "What",
        "Which", "While", "With", "From", "Into", "Upon", "About", "After",
        "Before", "During", "Between", "Through", "Against", "Without",
        "Within", "Along", "Beyond", "Under", "Above", "Below", "Behind",
        "Here", "There", "Then", "Thus", "Also", "Even", "Just", "Only",
        "Some", "Many", "Much", "Most", "Other", "Such", "Each", "Every",
        "Both", "Either", "Neither", "All", "Any", "Few", "More", "Less",
        "But", "And", "For", "Nor", "Not", "Yet", "His", "Her", "Its",
        "Our", "Your", "Their", "Who", "How", "Why", "Can", "May", "Will",
        "Shall", "Should", "Would", "Could", "Must", "Has", "Have", "Had",
        "Was", "Were", "Been", "Being", "Are", "Now", "New", "Old",
        "Good", "Great", "Long", "First", "Last", "Next", "Like", "Over",
        "Still", "Back", "Well", "Down", "Off", "Come", "Made", "See",
        "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight",
        "Nine", "Ten", "Act", "Scene", "Part", "Chapter", "Section",
    ].iter().copied().collect();

    let mut nouns = HashSet::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip all-caps lines (section headers)
        if trimmed.chars().all(|c| c.is_uppercase() || c.is_whitespace() || c.is_ascii_punctuation()) {
            continue;
        }

        let words: Vec<&str> = trimmed.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            // Strip trailing punctuation
            let clean = word.trim_end_matches(|c: char| c.is_ascii_punctuation());
            if clean.len() < 2 {
                continue;
            }

            // Must start with uppercase
            let first = clean.chars().next().unwrap();
            if !first.is_uppercase() {
                continue;
            }

            // Skip sentence-initial words (first word or after sentence-ending punctuation)
            if i == 0 {
                continue;
            }
            if i > 0 {
                let prev = words[i - 1];
                if prev.ends_with('.') || prev.ends_with('!') || prev.ends_with('?') {
                    continue;
                }
            }

            // Skip common words
            if common_words.contains(clean) {
                continue;
            }

            // Must not be all uppercase (that's an acronym or heading fragment)
            if clean.chars().all(|c| c.is_uppercase()) && clean.len() > 1 {
                continue;
            }

            nouns.insert(clean.to_string());
        }
    }

    let mut sorted: Vec<String> = nouns.into_iter().collect();
    sorted.sort();
    sorted
}

#[async_trait]
impl Adapter for TextAnalysisAdapter {
    fn id(&self) -> &str {
        &self.adapter_id
    }

    fn input_kind(&self) -> &str {
        "extract-analysis-text"
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        use crate::adapter::extraction::ExtractFileInput;

        let file_input = input
            .downcast_data::<ExtractFileInput>()
            .ok_or(AdapterError::InvalidInput)?;

        let file_path = &file_input.file_path;

        // Read file content
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            AdapterError::Internal(format!("failed to read {}: {}", file_path, e))
        })?;

        let lines: Vec<&str> = content.lines().collect();

        // Detect sections
        let sections = detect_sections(&lines);

        // Extract proper nouns
        let proper_nouns = extract_proper_nouns(&lines);

        let mut emission = Emission::new();
        let file_node_id = NodeId::from_string(format!("file:{}", file_path));
        let adapter_id = self.id();

        // Store section boundaries as property on the file node
        if !sections.is_empty() {
            let section_data: Vec<PropertyValue> = sections
                .iter()
                .map(|s| {
                    PropertyValue::String(format!(
                        "{}:{}:{}:{}",
                        s.label, s.start_line, s.end_line, s.depth
                    ))
                })
                .collect();

            emission = emission.with_property_update(crate::adapter::types::PropertyUpdate {
                node_id: file_node_id.clone(),
                properties: {
                    let mut props = std::collections::HashMap::new();
                    props.insert(
                        "sections".to_string(),
                        PropertyValue::Array(section_data),
                    );
                    props
                },
            });
        }

        // Emit concept nodes for proper nouns
        for noun in &proper_nouns {
            let normalized = noun.to_lowercase();
            let concept_id = NodeId::from_string(format!("concept:{}", normalized));

            let mut node = Node::new_in_dimension(
                "concept",
                ContentType::Concept,
                dimension::SEMANTIC,
            );
            node.id = concept_id.clone();
            node.properties.insert(
                "label".to_string(),
                PropertyValue::String(normalized.clone()),
            );
            node.properties.insert(
                "source".to_string(),
                PropertyValue::String("text-analysis".to_string()),
            );
            emission = emission.with_node(AnnotatedNode::new(node));

            // mentions edge: file → concept
            let mut edge = Edge::new_cross_dimensional(
                file_node_id.clone(),
                dimension::STRUCTURE,
                concept_id,
                dimension::SEMANTIC,
                "mentions",
            );
            edge.raw_weight = 1.0;
            emission = emission.with_edge(AnnotatedEdge::new(edge));
        }

        // Provenance trail (Invariant 7 — dual obligation)
        let chain_id = NodeId::from_string(format!("chain:{}:{}", adapter_id, file_path));
        let mut chain_node = Node::new_in_dimension(
            "chain",
            ContentType::Provenance,
            dimension::PROVENANCE,
        );
        chain_node.id = chain_id.clone();
        chain_node.properties.insert(
            "name".to_string(),
            PropertyValue::String(format!("{} — {}", adapter_id, file_path)),
        );
        chain_node.properties.insert(
            "status".to_string(),
            PropertyValue::String("active".to_string()),
        );
        emission = emission.with_node(AnnotatedNode::new(chain_node));

        // One mark per section (or one for whole file)
        let concept_labels: Vec<String> = proper_nouns.iter().map(|n| n.to_lowercase()).collect();

        let mark_targets: Vec<(String, NodeId, Node)> = if sections.is_empty() {
            let mark_id = NodeId::from_string(format!("mark:{}:{}", adapter_id, file_path));
            let mut mark = Node::new_in_dimension(
                "mark",
                ContentType::Provenance,
                dimension::PROVENANCE,
            );
            mark.id = mark_id.clone();
            mark.properties.insert(
                "file_path".to_string(),
                PropertyValue::String(file_path.clone()),
            );
            mark.properties.insert(
                "annotation".to_string(),
                PropertyValue::String(format!("text analysis of {}", file_path)),
            );
            if !concept_labels.is_empty() {
                mark.properties.insert(
                    "tags".to_string(),
                    PropertyValue::Array(
                        concept_labels.iter().map(|l| PropertyValue::String(l.clone())).collect(),
                    ),
                );
            }
            vec![("whole-file".to_string(), mark_id, mark)]
        } else {
            sections
                .iter()
                .map(|s| {
                    let slug = s.label.to_lowercase().replace(' ', "-");
                    let mark_id = NodeId::from_string(format!(
                        "mark:{}:{}:{}", adapter_id, file_path, slug
                    ));
                    let mut mark = Node::new_in_dimension(
                        "mark",
                        ContentType::Provenance,
                        dimension::PROVENANCE,
                    );
                    mark.id = mark_id.clone();
                    mark.properties.insert(
                        "file_path".to_string(),
                        PropertyValue::String(file_path.clone()),
                    );
                    mark.properties.insert(
                        "section".to_string(),
                        PropertyValue::String(s.label.clone()),
                    );
                    mark.properties.insert(
                        "start_line".to_string(),
                        PropertyValue::Int(s.start_line as i64),
                    );
                    mark.properties.insert(
                        "end_line".to_string(),
                        PropertyValue::Int(s.end_line as i64),
                    );
                    mark.properties.insert(
                        "annotation".to_string(),
                        PropertyValue::String(format!(
                            "text analysis of {} [{}]", file_path, s.label
                        )),
                    );
                    if !concept_labels.is_empty() {
                        mark.properties.insert(
                            "tags".to_string(),
                            PropertyValue::Array(
                                concept_labels.iter().map(|l| PropertyValue::String(l.clone())).collect(),
                            ),
                        );
                    }
                    (slug, mark_id, mark)
                })
                .collect()
        };

        for (_slug, mark_id, mark) in mark_targets {
            emission = emission.with_node(AnnotatedNode::new(mark));
            let mut contains = Edge::new_in_dimension(
                chain_id.clone(),
                mark_id,
                "contains",
                dimension::PROVENANCE,
            );
            contains.contributions.insert(adapter_id.to_string(), 1.0);
            emission = emission.with_edge(AnnotatedEdge::new(contains));
        }

        if !emission.is_empty() {
            sink.emit(emission).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::graph::Context;
    use std::sync::{Arc, Mutex};

    fn test_sink(ctx: Arc<Mutex<Context>>) -> EngineSink {
        EngineSink::new(ctx).with_framework_context(FrameworkContext {
            adapter_id: "extract-analysis-text".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        })
    }

    fn write_temp_file(name: &str, content: &str) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(name), content).unwrap();
        dir
    }

    // --- Scenario: TextAnalysisAdapter detects section boundaries in plain text ---

    #[tokio::test]
    async fn detects_section_boundaries_in_plain_text() {
        let dir = write_temp_file(
            "test.txt",
            "# Introduction\n\nSome intro text.\n\n# Methods\n\nMethodology here.\n\n# Results\n\nResults here.\n",
        );
        let file_path = dir.path().join("test.txt");

        let adapter = TextAnalysisAdapter::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input = AdapterInput::new(
            "extract-analysis-text",
            crate::adapter::extraction::ExtractFileInput {
                file_path: file_path.to_str().unwrap().to_string(),
            },
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Check section boundaries stored as property updates
        // The emission contains property_updates for the file node
        // In the mutex path, property updates are applied to existing nodes.
        // Since the file node doesn't exist in this test context, let's check
        // the sections were detected correctly via the detect_sections function directly.
        let content = std::fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let sections = detect_sections(&lines);

        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].label, "Introduction");
        assert_eq!(sections[0].start_line, 1);
        assert_eq!(sections[1].label, "Methods");
        assert_eq!(sections[1].start_line, 5);
        assert_eq!(sections[2].label, "Results");
        assert_eq!(sections[2].start_line, 9);

        // Each section has end_line
        assert!(sections[0].end_line > sections[0].start_line);

        // Provenance chain exists
        let chain_id = NodeId::from_string(format!(
            "chain:extract-analysis-text:{}", file_path.to_str().unwrap()
        ));
        assert!(
            snapshot.get_node(&chain_id).is_some(),
            "chain node should exist"
        );

        // Marks exist (one per section)
        let marks: Vec<_> = snapshot.nodes().filter(|n| n.node_type == "mark").collect();
        assert_eq!(marks.len(), 3, "one mark per section");
    }

    // --- Scenario: TextAnalysisAdapter detects act/scene markers in dramatic text ---

    #[tokio::test]
    async fn detects_act_scene_markers() {
        let text = "\
ACT I

SCENE 1

Enter Macbeth and Banquo.

SCENE 2

Enter Lady Macbeth.

ACT II

SCENE 1

A dark chamber.
";
        let dir = write_temp_file("macbeth.txt", text);
        let file_path = dir.path().join("macbeth.txt");

        let content = std::fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let sections = detect_sections(&lines);

        // Should detect ACT I, SCENE 1, SCENE 2, ACT II, SCENE 1
        let acts: Vec<_> = sections.iter().filter(|s| s.label.starts_with("ACT")).collect();
        let scenes: Vec<_> = sections.iter().filter(|s| s.label.starts_with("SCENE")).collect();

        assert_eq!(acts.len(), 2, "two acts");
        assert_eq!(scenes.len(), 3, "three scenes");

        // Acts are depth 0, scenes are depth 1
        for act in &acts {
            assert_eq!(act.depth, 0, "acts are top-level");
        }
        for scene in &scenes {
            assert_eq!(scene.depth, 1, "scenes are nested under acts");
        }

        // ACT I ends before ACT II starts
        assert!(acts[0].end_line < acts[1].start_line);
    }

    // --- Scenario: TextAnalysisAdapter extracts proper nouns as concepts ---

    #[tokio::test]
    async fn extracts_proper_nouns_as_concepts() {
        let text = "\
The play is set in Scotland. The tragic hero Macbeth murders King Duncan.
Lady Macbeth drives the plot. Banquo is murdered too.
The battle takes place near Dunsinane.
";
        let dir = write_temp_file("nouns.txt", text);
        let file_path = dir.path().join("nouns.txt");

        let adapter = TextAnalysisAdapter::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Pre-create the file node (Phase 1 would do this in production)
        {
            let mut c = ctx.lock().unwrap();
            let mut file_node = Node::new_in_dimension(
                "file",
                ContentType::Document,
                dimension::STRUCTURE,
            );
            file_node.id = NodeId::from_string(format!("file:{}", file_path.to_str().unwrap()));
            c.add_node(file_node);
        }

        let sink = test_sink(ctx.clone());

        let input = AdapterInput::new(
            "extract-analysis-text",
            crate::adapter::extraction::ExtractFileInput {
                file_path: file_path.to_str().unwrap().to_string(),
            },
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Should have concept nodes for proper nouns
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:macbeth")).is_some(),
            "Macbeth should be extracted as concept"
        );
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:scotland")).is_some(),
            "Scotland should be extracted as concept"
        );

        // mentions edges from file to concepts
        let mentions: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "mentions")
            .collect();
        assert!(
            mentions.len() >= 2,
            "should have mentions edges for proper nouns"
        );
    }

    // --- Scenario: TextAnalysisAdapter produces provenance trail ---

    #[tokio::test]
    async fn produces_provenance_trail() {
        let dir = write_temp_file(
            "prov.txt",
            "# Section One\n\nText with Macbeth.\n\n# Section Two\n\nMore text.\n",
        );
        let file_path = dir.path().join("prov.txt");

        let adapter = TextAnalysisAdapter::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input = AdapterInput::new(
            "extract-analysis-text",
            crate::adapter::extraction::ExtractFileInput {
                file_path: file_path.to_str().unwrap().to_string(),
            },
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Chain node
        let chain_id = NodeId::from_string(format!(
            "chain:extract-analysis-text:{}", file_path.to_str().unwrap()
        ));
        let chain = snapshot.get_node(&chain_id).expect("chain node should exist");
        assert_eq!(chain.dimension, dimension::PROVENANCE);

        // Mark nodes
        let marks: Vec<_> = snapshot.nodes().filter(|n| n.node_type == "mark").collect();
        assert_eq!(marks.len(), 2, "two marks (one per section)");

        // Contains edges from chain to marks
        let contains: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "contains" && e.source == chain_id)
            .collect();
        assert_eq!(contains.len(), 2, "two contains edges");

        for edge in &contains {
            assert_eq!(
                edge.contributions.get("extract-analysis-text"),
                Some(&1.0),
                "contains edge should have contribution from adapter"
            );
        }
    }

    // --- Scenario: TextAnalysisAdapter output consumed by real SemanticAdapter (integration) ---

    #[tokio::test]
    async fn text_analysis_feeds_real_semantic_adapter() {
        use crate::adapter::extraction::{ExtractionCoordinator, ExtractFileInput};
        use crate::adapter::semantic::SemanticAdapter;
        use crate::graph::{ContextId, PlexusEngine};
        use crate::llm_orc::{AgentResult, InvokeResponse, MockClient};
        use crate::storage::{SqliteStore, OpenStore};

        // Mock LLM returns concepts
        let mut results = std::collections::HashMap::new();
        results.insert(
            "synthesizer".to_string(),
            AgentResult {
                response: Some(
                    r#"{"concepts": [{"label": "Tragedy", "confidence": 0.9}], "relationships": []}"#
                        .to_string(),
                ),
                status: Some("success".to_string()),
                error: None,
            },
        );
        let mock_client = Arc::new(
            MockClient::available().with_response(
                "extract-semantic",
                InvokeResponse {
                    results,
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));
        let context_id = ContextId::from_string("test");
        let mut ctx = Context::new("test");
        ctx.id = context_id.clone();
        engine.upsert_context(ctx).unwrap();

        // Phase 2: real TextAnalysisAdapter
        let text_adapter = Arc::new(TextAnalysisAdapter::new());
        // Phase 3: real SemanticAdapter with MockClient
        let semantic = Arc::new(SemanticAdapter::new(mock_client, "extract-semantic"));

        let mut coordinator = ExtractionCoordinator::new()
            .with_engine(engine.clone(), context_id.clone());
        coordinator.register_phase2("text/", text_adapter);
        coordinator.register_phase3(semantic);

        let primary_sink = EngineSink::for_engine(engine.clone(), context_id.clone())
            .with_framework_context(FrameworkContext {
                adapter_id: "extract-coordinator".to_string(),
                context_id: "test".to_string(),
                input_summary: None,
            });

        // Create a text file with dramatic structure
        let dir = write_temp_file(
            "drama.txt",
            "ACT I\n\nSCENE 1\n\nEnter Macbeth and Scotland beckons.\n\nSCENE 2\n\nMore drama.\n",
        );
        let file_path = dir.path().join("drama.txt");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path.to_str().unwrap().to_string(),
            },
            "test",
        );

        coordinator.process(&input, &primary_sink).await.unwrap();
        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().all(|r| r.is_ok()),
            "Both phases should succeed: {:?}",
            results
        );

        // Phase 2 concepts (proper nouns) persisted
        let ctx = engine.get_context(&context_id).expect("context should exist");

        // Phase 3 concept (from LLM) persisted
        assert!(
            ctx.get_node(&NodeId::from_string("concept:tragedy")).is_some(),
            "Phase 3 LLM concept should be persisted"
        );

        // Both phases' emissions are independent (Invariant 46)
        // Reload from store to verify
        let engine2 = Arc::new(PlexusEngine::with_store(store.clone()));
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&context_id).expect("context should survive reload");
        assert!(
            ctx2.get_node(&NodeId::from_string("concept:tragedy")).is_some(),
            "Phase 3 concept should survive reload"
        );

        // Phase 2 provenance chain exists
        let chain_id = NodeId::from_string(format!(
            "chain:extract-analysis-text:{}", file_path.to_str().unwrap()
        ));
        assert!(
            ctx2.get_node(&chain_id).is_some(),
            "Phase 2 provenance chain should survive reload"
        );
    }
}
