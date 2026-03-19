//! Phase 3 semantic adapter — LLM-based concept extraction (ADR-021)
//!
//! Delegates to llm-orc for deep semantic analysis. Runs as Phase 3
//! in the extraction coordinator, after Phase 2 (heuristic analysis).
//!
//! When llm-orc is unavailable, returns `AdapterError::Skipped` for
//! graceful degradation (Invariant 47).
//!
//! The adapter:
//! 1. Checks llm-orc availability
//! 2. Serializes Phase 2 output (from context) as input
//! 3. Invokes the extraction ensemble
//! 4. Deserializes the response into concept nodes and edges

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission, concept_node};
use crate::graph::{dimension, ContentType, Context, Edge, Node, NodeId, PropertyValue};
use crate::llm_orc::{LlmOrcClient, LlmOrcError};
use async_trait::async_trait;
use std::sync::Arc;

pub use super::structural::SectionBoundary;

/// Input for the semantic adapter.
///
/// Extends the basic file path with optional section boundaries and vocabulary
/// from structural analysis. When sections are present, llm-orc can chunk the
/// document along structural boundaries (ADR-021 Scenario 3). Vocabulary terms
/// are passed as a glossary hint for entity-primed extraction (ADR-031).
#[derive(Debug, Clone)]
pub struct SemanticInput {
    pub file_path: String,
    /// Section boundaries identified by structural analysis.
    /// Each entry is (label, start_line, end_line). Empty = process whole file.
    pub sections: Vec<SectionBoundary>,
    /// Vocabulary terms from structural analysis — entity names, key terms,
    /// link targets. Passed to llm-orc as a glossary hint (not a constraint).
    /// Empty when no structural modules matched or produced vocabulary.
    pub vocabulary: Vec<String>,
}

impl SemanticInput {
    /// Create input for a single file with no structural context.
    pub fn for_file(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            sections: Vec::new(),
            vocabulary: Vec::new(),
        }
    }

    /// Create input with section boundaries from structural analysis.
    pub fn with_sections(
        file_path: impl Into<String>,
        sections: Vec<SectionBoundary>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            sections,
            vocabulary: Vec::new(),
        }
    }

    /// Create input with full structural context — sections and vocabulary
    /// from structural analysis modules (ADR-031).
    pub fn with_structural_context(
        file_path: impl Into<String>,
        sections: Vec<SectionBoundary>,
        vocabulary: Vec<String>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            sections,
            vocabulary,
        }
    }
}

/// Extract a JSON object from LLM response text.
///
/// LLMs sometimes wrap JSON in markdown code fences or add explanation text.
/// This function tries, in order:
/// 1. Direct parse (response is pure JSON)
/// 2. Extract from ```json ... ``` or ``` ... ``` fenced block
/// 3. Find the first `{` to last `}` span and parse that
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();

    // Try 1: Direct parse
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if v.is_object() {
            return Some(v);
        }
    }

    // Try 2: Extract from fenced code block
    let fenced = if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        after.find("```").map(|end| &after[..end])
    } else if let Some(start) = trimmed.find("```\n") {
        let after = &trimmed[start + 4..];
        after.find("```").map(|end| &after[..end])
    } else {
        None
    };

    if let Some(block) = fenced {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(block.trim()) {
            if v.is_object() {
                return Some(v);
            }
        }
    }

    // Try 3: Find first { to last }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]) {
                if v.is_object() {
                    return Some(v);
                }
            }
        }
    }

    None
}

/// Build a concept node from a label string.
///
/// Shared construction: normalizes the label, creates a deterministic NodeId,
/// and sets the label property. Callers add their own extra properties
/// (confidence, concept_type, source, etc.) after this returns.
/// Phase 3 semantic adapter — delegates to llm-orc for concept extraction.
pub struct SemanticAdapter {
    /// The llm-orc client (mock or real)
    client: Arc<dyn LlmOrcClient>,
    /// The ensemble to invoke for semantic extraction
    ensemble_name: String,
}

impl SemanticAdapter {
    pub fn new(client: Arc<dyn LlmOrcClient>, ensemble_name: impl Into<String>) -> Self {
        Self {
            client,
            ensemble_name: ensemble_name.into(),
        }
    }

    /// Build the input payload for llm-orc from the extraction context.
    ///
    /// Produces structured JSON conforming to `docs/schemas/phase2-output.schema.json`.
    /// Reads Phase 2 output from the shared context and serializes
    /// relevant information (file path, extracted terms, sections).
    /// When sections are present, includes them so llm-orc can chunk along
    /// structural boundaries (ADR-021 Scenario 3).
    fn build_input(
        &self,
        input: &SemanticInput,
        context: Option<&Context>,
    ) -> String {
        let mut payload = serde_json::json!({
            "file_path": input.file_path,
        });

        // Include section boundaries from structural analysis
        if !input.sections.is_empty() {
            let sections: Vec<serde_json::Value> = input
                .sections
                .iter()
                .map(|s| serde_json::json!({
                    "label": s.label,
                    "start_line": s.start_line,
                    "end_line": s.end_line,
                }))
                .collect();
            payload["sections"] = serde_json::Value::Array(sections);
        }

        // Include vocabulary from structural analysis (ADR-031)
        if !input.vocabulary.is_empty() {
            payload["vocabulary"] = serde_json::json!(input.vocabulary);
        }

        if let Some(ctx) = context {
            // Collect existing concepts (from Phase 1 frontmatter + Phase 2 analysis)
            let concepts: Vec<String> = ctx
                .nodes()
                .filter(|n| n.dimension == dimension::SEMANTIC && n.node_type == "concept")
                .filter_map(|n| {
                    n.properties.get("label").and_then(|pv| match pv {
                        PropertyValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                })
                .collect();

            if !concepts.is_empty() {
                payload["existing_concepts"] = serde_json::json!(concepts);
            }

            // Include file metadata if available
            let file_node_id = NodeId::from_string(format!("file:{}", input.file_path));
            if let Some(file_node) = ctx.get_node(&file_node_id) {
                if let Some(PropertyValue::String(mime)) =
                    file_node.properties.get("mime_type")
                {
                    payload["mime_type"] = serde_json::Value::String(mime.clone());
                }
            }
        }

        serde_json::to_string(&payload).expect("Phase 2 output serialization should not fail")
    }

}

#[async_trait]
impl Adapter for SemanticAdapter {
    fn id(&self) -> &str {
        "extract-semantic"
    }

    fn input_kind(&self) -> &str {
        "extract-semantic"
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let semantic_input = input
            .downcast_data::<SemanticInput>()
            .ok_or(AdapterError::InvalidInput)?;

        let file_path = &semantic_input.file_path;

        // Check availability — graceful degradation (Invariant 47)
        if !self.client.is_available().await {
            return Err(AdapterError::Skipped(
                "llm-orc not running".to_string(),
            ));
        }

        // Build input from Phase 2 context, including section boundaries
        let input_text = self.build_input(semantic_input, None);

        // Invoke llm-orc ensemble
        let response = self
            .client
            .invoke(&self.ensemble_name, &input_text)
            .await
            .map_err(|e| match e {
                LlmOrcError::Unavailable(msg) => AdapterError::Skipped(msg),
                other => AdapterError::Internal(other.to_string()),
            })?;

        if response.is_failed() {
            return Err(AdapterError::Internal(
                "llm-orc ensemble execution failed".to_string(),
            ));
        }

        // Multi-agent parsing: iterate all agent results, merge into single emission.
        // Each agent's edges carry per-agent contribution keys (Invariant 45).
        let mut emission = Emission::new();
        for (agent_name, agent_result) in &response.results {
            if let Some(ref text) = agent_result.response {
                if let Some(parsed) = extract_json(text) {
                    let contribution_key = format!("extract-phase3:{}", agent_name);
                    let agent_emission =
                        self.parse_agent_response(&parsed, file_path, &contribution_key);
                    emission = emission.merge(agent_emission);
                }
            }
        }

        // Add provenance trail (Invariant 7 — dual obligation)
        if !emission.is_empty() {
            self.add_provenance(&mut emission, semantic_input);
            sink.emit(emission).await?;
        }

        Ok(())
    }
}

/// Build a tagged_with edge from a file node (structure) to a concept node (semantic).
///
/// Sets combined_weight = 1.0 and inserts a contribution under `contribution_key`.
/// Used by all Phase 3 parsers (standard, SpaCy, themes).
fn tagged_with_edge(
    file_node_id: &NodeId,
    concept_id: NodeId,
    contribution_key: &str,
) -> AnnotatedEdge {
    let mut edge = Edge::new_cross_dimensional(
        file_node_id.clone(),
        dimension::STRUCTURE,
        concept_id,
        dimension::SEMANTIC,
        "tagged_with",
    );
    edge.combined_weight = 1.0;
    edge.contributions
        .insert(contribution_key.to_string(), 1.0);
    AnnotatedEdge::new(edge)
}

impl SemanticAdapter {
    /// Parse a single agent's JSON response into an emission with per-agent contribution keys.
    ///
    /// Dispatches to specialized parsers based on response shape:
    /// - `"entities"` key → SpaCy script output (parse_spacy_response)
    /// - `"themes"` key → theme extraction (parse_themes)
    /// - `"concepts"` / `"relationships"` → standard LLM extraction (inline)
    fn parse_agent_response(
        &self,
        parsed: &serde_json::Value,
        file_path: &str,
        contribution_key: &str,
    ) -> Emission {
        let file_node_id = NodeId::from_string(format!("file:{}", file_path));

        // Dispatch by response shape
        // SpaCy script wraps output in {"success": ..., "data": {"entities": ...}}
        let has_entities = parsed.get("entities").is_some()
            || parsed
                .get("data")
                .and_then(|d| d.get("entities"))
                .is_some();
        if has_entities {
            return self.parse_spacy_response(parsed, &file_node_id, contribution_key);
        }

        if parsed.get("themes").is_some()
            && parsed.get("concepts").is_none()
            && parsed.get("relationships").is_none()
        {
            return self.parse_themes(parsed, &file_node_id, contribution_key);
        }

        // Standard: concepts + relationships
        let mut emission = Emission::new();

        if let Some(concepts) = parsed.get("concepts").and_then(|v| v.as_array()) {
            for concept in concepts {
                let label = concept
                    .get("label")
                    .or_else(|| concept.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if label.is_empty() {
                    continue;
                }

                let (concept_id, mut node) = concept_node(label);
                if let Some(concept_type) = concept.get("type").and_then(|v| v.as_str()) {
                    node.properties.insert(
                        "concept_type".to_string(),
                        PropertyValue::String(concept_type.to_string()),
                    );
                }
                if let Some(confidence) = concept.get("confidence").and_then(|v| v.as_f64()) {
                    node.properties.insert(
                        "confidence".to_string(),
                        PropertyValue::Float(confidence),
                    );
                }
                emission = emission.with_node(AnnotatedNode::new(node));
                emission = emission.with_edge(tagged_with_edge(&file_node_id, concept_id, contribution_key));
            }
        }

        if let Some(rels) = parsed.get("relationships").and_then(|v| v.as_array()) {
            for rel in rels {
                let source = rel.get("source").and_then(|v| v.as_str()).unwrap_or_default();
                let target = rel.get("target").and_then(|v| v.as_str()).unwrap_or_default();
                let relationship = rel
                    .get("relationship")
                    .and_then(|v| v.as_str())
                    .unwrap_or("related_to");
                let weight = rel
                    .get("weight")
                    .or_else(|| rel.get("confidence"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);

                if source.is_empty() || target.is_empty() {
                    continue;
                }

                let source_id =
                    NodeId::from_string(format!("concept:{}", source.to_lowercase()));
                let target_id =
                    NodeId::from_string(format!("concept:{}", target.to_lowercase()));

                let mut edge = Edge::new(source_id, target_id, relationship);
                edge.source_dimension = dimension::SEMANTIC.to_string();
                edge.target_dimension = dimension::SEMANTIC.to_string();
                edge.combined_weight = weight as f32;
                edge.contributions
                    .insert(contribution_key.to_string(), weight as f32);
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }
        }

        emission
    }

    /// Parse SpaCy script output: entities, relationships (SVO), and co-occurrences.
    fn parse_spacy_response(
        &self,
        parsed: &serde_json::Value,
        file_node_id: &NodeId,
        contribution_key: &str,
    ) -> Emission {
        let mut emission = Emission::new();

        // Use "data" wrapper if present (script agent protocol)
        let data = parsed.get("data").unwrap_or(parsed);

        // entities → concept nodes
        if let Some(entities) = data.get("entities").and_then(|v| v.as_array()) {
            for entity in entities {
                let label = entity.get("label").and_then(|v| v.as_str()).unwrap_or_default();
                if label.is_empty() {
                    continue;
                }
                let (concept_id, mut node) = concept_node(label);
                if let Some(etype) = entity.get("type").and_then(|v| v.as_str()) {
                    node.properties.insert(
                        "concept_type".to_string(),
                        PropertyValue::String(etype.to_string()),
                    );
                }
                node.properties.insert(
                    "source".to_string(),
                    PropertyValue::String("spacy".to_string()),
                );
                emission = emission.with_node(AnnotatedNode::new(node));
                emission = emission.with_edge(tagged_with_edge(file_node_id, concept_id, contribution_key));
            }
        }

        // relationships (SVO triples) → typed directed edges
        if let Some(rels) = data.get("relationships").and_then(|v| v.as_array()) {
            for rel in rels {
                let source = rel.get("source").and_then(|v| v.as_str()).unwrap_or_default();
                let target = rel.get("target").and_then(|v| v.as_str()).unwrap_or_default();
                let relationship = rel
                    .get("relationship")
                    .and_then(|v| v.as_str())
                    .unwrap_or("related_to");

                if source.is_empty() || target.is_empty() {
                    continue;
                }

                let source_id =
                    NodeId::from_string(format!("concept:{}", source.to_lowercase()));
                let target_id =
                    NodeId::from_string(format!("concept:{}", target.to_lowercase()));

                let mut edge = Edge::new(source_id, target_id, relationship);
                edge.source_dimension = dimension::SEMANTIC.to_string();
                edge.target_dimension = dimension::SEMANTIC.to_string();
                edge.combined_weight = 1.0;
                edge.contributions
                    .insert(contribution_key.to_string(), 1.0);
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }
        }

        // cooccurrences → symmetric may_be_related edge pairs
        if let Some(coocs) = data.get("cooccurrences").and_then(|v| v.as_array()) {
            for cooc in coocs {
                let a = cooc.get("entity_a").and_then(|v| v.as_str()).unwrap_or_default();
                let b = cooc.get("entity_b").and_then(|v| v.as_str()).unwrap_or_default();

                if a.is_empty() || b.is_empty() {
                    continue;
                }

                let a_id = NodeId::from_string(format!("concept:{}", a.to_lowercase()));
                let b_id = NodeId::from_string(format!("concept:{}", b.to_lowercase()));

                // A → B
                let mut edge_ab = Edge::new(a_id.clone(), b_id.clone(), "may_be_related");
                edge_ab.source_dimension = dimension::SEMANTIC.to_string();
                edge_ab.target_dimension = dimension::SEMANTIC.to_string();
                edge_ab.combined_weight = 1.0;
                edge_ab
                    .contributions
                    .insert(contribution_key.to_string(), 1.0);
                emission = emission.with_edge(AnnotatedEdge::new(edge_ab));

                // B → A
                let mut edge_ba = Edge::new(b_id, a_id, "may_be_related");
                edge_ba.source_dimension = dimension::SEMANTIC.to_string();
                edge_ba.target_dimension = dimension::SEMANTIC.to_string();
                edge_ba.combined_weight = 1.0;
                edge_ba
                    .contributions
                    .insert(contribution_key.to_string(), 1.0);
                emission = emission.with_edge(AnnotatedEdge::new(edge_ba));
            }
        }

        emission
    }

    /// Parse theme extraction response into concept nodes.
    fn parse_themes(
        &self,
        parsed: &serde_json::Value,
        file_node_id: &NodeId,
        contribution_key: &str,
    ) -> Emission {
        let mut emission = Emission::new();

        if let Some(themes) = parsed.get("themes").and_then(|v| v.as_array()) {
            for theme in themes {
                let description = theme
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if description.is_empty() {
                    continue;
                }

                let (concept_id, mut node) = concept_node(description);
                node.properties.insert(
                    "source".to_string(),
                    PropertyValue::String("theme".to_string()),
                );
                if let Some(theme_type) = theme.get("type").and_then(|v| v.as_str()) {
                    node.properties.insert(
                        "concept_type".to_string(),
                        PropertyValue::String(theme_type.to_string()),
                    );
                }
                if let Some(evidence) = theme.get("supporting_evidence").and_then(|v| v.as_str()) {
                    node.properties.insert(
                        "evidence".to_string(),
                        PropertyValue::String(evidence.to_string()),
                    );
                }
                emission = emission.with_node(AnnotatedNode::new(node));

                emission = emission.with_edge(tagged_with_edge(file_node_id, concept_id, contribution_key));
            }
        }

        emission
    }

    /// Add provenance trail to emission: chain + marks + contains edges.
    ///
    /// Invariant 7 (dual obligation): every adapter emits both semantic content
    /// AND provenance. The chain scopes all marks to this adapter run. Marks
    /// record where concepts were found, with concept labels as tags.
    fn add_provenance(&self, emission: &mut Emission, input: &SemanticInput) {
        let file_path = &input.file_path;
        let adapter_id = "extract-semantic";

        // Chain node — one per adapter run per file
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
        *emission = std::mem::take(emission).with_node(AnnotatedNode::new(chain_node));

        // Collect concept labels from emission for tags
        let concept_labels: Vec<String> = emission
            .nodes
            .iter()
            .filter(|n| n.node.node_type == "concept")
            .filter_map(|n| match n.node.properties.get("label") {
                Some(PropertyValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        // Mark nodes — one per section (or one for whole file)
        let marks: Vec<(String, NodeId, Node)> = if input.sections.is_empty() {
            // No sections: one mark for the whole file
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
                PropertyValue::String(format!("semantic extraction of {}", file_path)),
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
            // One mark per section
            input
                .sections
                .iter()
                .map(|section| {
                    let slug = section.label.to_lowercase().replace(' ', "-");
                    let mark_id = NodeId::from_string(format!(
                        "mark:{}:{}:{}",
                        adapter_id, file_path, slug
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
                        PropertyValue::String(section.label.clone()),
                    );
                    mark.properties.insert(
                        "start_line".to_string(),
                        PropertyValue::Int(section.start_line as i64),
                    );
                    mark.properties.insert(
                        "end_line".to_string(),
                        PropertyValue::Int(section.end_line as i64),
                    );
                    mark.properties.insert(
                        "annotation".to_string(),
                        PropertyValue::String(format!(
                            "semantic extraction of {} [{}]",
                            file_path, section.label
                        )),
                    );
                    if !concept_labels.is_empty() {
                        mark.properties.insert(
                            "tags".to_string(),
                            PropertyValue::Array(
                                concept_labels
                                    .iter()
                                    .map(|l| PropertyValue::String(l.clone()))
                                    .collect(),
                            ),
                        );
                    }
                    (slug, mark_id, mark)
                })
                .collect()
        };

        // Add marks and contains edges to emission
        for (_slug, mark_id, mark) in marks {
            *emission = std::mem::take(emission).with_node(AnnotatedNode::new(mark));

            let mut contains_edge = Edge::new_in_dimension(
                chain_id.clone(),
                mark_id,
                "contains",
                dimension::PROVENANCE,
            );
            contains_edge
                .contributions
                .insert(adapter_id.to_string(), 1.0);
            *emission = std::mem::take(emission).with_edge(AnnotatedEdge::new(contains_edge));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::EngineSink;
    use crate::adapter::FrameworkContext;
    use crate::llm_orc::MockClient;
    use std::sync::{Mutex};

    fn test_sink(ctx: Arc<Mutex<Context>>) -> EngineSink {
        EngineSink::new(ctx).with_framework_context(FrameworkContext {
            adapter_id: "extract-semantic".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        })
    }

    // --- Scenario: Phase 3 delegates to llm-orc (ADR-021) ---

    #[tokio::test]
    async fn phase3_delegates_to_llm_orc() {
        // Mock llm-orc returns structured concepts
        let llm_response = r#"{
            "concepts": [
                { "label": "machine learning", "confidence": 0.92 },
                { "label": "neural networks", "confidence": 0.87 }
            ],
            "relationships": [
                {
                    "source": "neural networks",
                    "target": "machine learning",
                    "relationship": "is_a",
                    "weight": 0.8
                }
            ]
        }"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "semantic-extraction",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "concept-extractor".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(llm_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "semantic-extraction");
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Pre-populate with a file node (from Phase 1)
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("/docs/example.md"));
        }

        let sink = test_sink(ctx.clone());

        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("/docs/example.md"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Concept nodes created
        let ml = snapshot
            .get_node(&NodeId::from_string("concept:machine learning"))
            .expect("machine learning concept should exist");
        assert_eq!(ml.dimension, dimension::SEMANTIC);
        assert_eq!(
            ml.properties.get("confidence"),
            Some(&PropertyValue::Float(0.92))
        );

        let nn = snapshot
            .get_node(&NodeId::from_string("concept:neural networks"))
            .expect("neural networks concept should exist");
        assert_eq!(
            nn.properties.get("confidence"),
            Some(&PropertyValue::Float(0.87))
        );

        // tagged_with edges from file to concepts
        let tagged_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_edges.len(), 2, "two tagged_with edges");

        // Relationship between concepts
        let is_a_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "is_a")
            .collect();
        assert_eq!(is_a_edges.len(), 1, "one is_a relationship");
        assert_eq!(
            is_a_edges[0].source,
            NodeId::from_string("concept:neural networks")
        );
        assert_eq!(
            is_a_edges[0].target,
            NodeId::from_string("concept:machine learning")
        );
    }

    // --- Scenario: Phase 3 graceful degradation via SemanticAdapter ---

    #[tokio::test]
    async fn phase3_skips_when_llm_orc_unavailable() {
        let mock_client = Arc::new(MockClient::unavailable());
        let adapter = SemanticAdapter::new(mock_client, "semantic-extraction");

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("/docs/example.md"),
            "test",
        );

        let result = adapter.process(&input, &sink).await;

        // Should return Skipped, not a hard failure
        assert!(matches!(result, Err(AdapterError::Skipped(_))));
    }

    // --- Scenario: Empty response produces no emission ---

    #[tokio::test]
    async fn empty_llm_response_produces_no_emission() {
        let mock_client = Arc::new(
            MockClient::available().with_response(
                "semantic-extraction",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "extractor".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(
                                    r#"{ "concepts": [], "relationships": [] }"#
                                        .to_string(),
                                ),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "semantic-extraction");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("/docs/empty.md"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();
        assert_eq!(snapshot.node_count(), 0, "no nodes from empty response");
    }

    // --- Adapter ID is stable ---

    #[test]
    fn semantic_adapter_has_stable_id() {
        let client = Arc::new(MockClient::unavailable());
        let adapter = SemanticAdapter::new(client, "any-ensemble");
        assert_eq!(adapter.id(), "extract-semantic");
        assert_eq!(adapter.input_kind(), "extract-semantic");
    }

    // --- Scenario: Long document chunking via Phase 2 boundaries (ADR-021) ---

    #[test]
    fn build_input_includes_section_boundaries() {
        let client = Arc::new(MockClient::unavailable());
        let adapter = SemanticAdapter::new(client, "semantic-extraction");

        let input = SemanticInput::with_sections(
            "/docs/hamlet.txt",
            vec![
                SectionBoundary {
                    label: "Act I".to_string(),
                    start_line: 1,
                    end_line: 500,
                },
                SectionBoundary {
                    label: "Act II".to_string(),
                    start_line: 501,
                    end_line: 1000,
                },
                SectionBoundary {
                    label: "Act III".to_string(),
                    start_line: 1001,
                    end_line: 1500,
                },
            ],
        );

        let payload = adapter.build_input(&input, None);
        let parsed: serde_json::Value = serde_json::from_str(&payload)
            .expect("build_input should produce valid JSON");

        assert_eq!(parsed["file_path"], "/docs/hamlet.txt");

        let sections = parsed["sections"].as_array().expect("sections should be array");
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0]["label"], "Act I");
        assert_eq!(sections[0]["start_line"], 1);
        assert_eq!(sections[0]["end_line"], 500);
        assert_eq!(sections[1]["label"], "Act II");
        assert_eq!(sections[1]["start_line"], 501);
        assert_eq!(sections[1]["end_line"], 1000);
        assert_eq!(sections[2]["label"], "Act III");
        assert_eq!(sections[2]["start_line"], 1001);
        assert_eq!(sections[2]["end_line"], 1500);
    }

    // --- Scenario: Vocabulary from structural analysis included in build_input (ADR-031) ---

    #[test]
    fn build_input_includes_vocabulary() {
        let client = Arc::new(MockClient::unavailable());
        let adapter = SemanticAdapter::new(client, "semantic-extraction");

        let input = SemanticInput::with_structural_context(
            "/docs/readme.md",
            vec![SectionBoundary {
                label: "Introduction".to_string(),
                start_line: 1,
                end_line: 50,
            }],
            vec!["Plexus".to_string(), "knowledge graph".to_string()],
        );

        let payload = adapter.build_input(&input, None);
        let parsed: serde_json::Value = serde_json::from_str(&payload)
            .expect("build_input should produce valid JSON");

        assert_eq!(parsed["file_path"], "/docs/readme.md");

        let vocab = parsed["vocabulary"].as_array().expect("vocabulary should be array");
        assert_eq!(vocab.len(), 2);
        assert_eq!(vocab[0], "Plexus");
        assert_eq!(vocab[1], "knowledge graph");

        // Sections also present
        assert!(parsed["sections"].is_array());
    }

    #[test]
    fn build_input_omits_empty_vocabulary() {
        let client = Arc::new(MockClient::unavailable());
        let adapter = SemanticAdapter::new(client, "semantic-extraction");

        // for_file produces empty vocabulary
        let input = SemanticInput::for_file("/docs/test.md");
        let payload = adapter.build_input(&input, None);
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();

        // Empty vocabulary should not appear in JSON
        assert!(parsed.get("vocabulary").is_none());
    }

    #[test]
    fn with_structural_context_carries_all_fields() {
        let input = SemanticInput::with_structural_context(
            "/docs/test.md",
            vec![SectionBoundary {
                label: "Header".to_string(),
                start_line: 1,
                end_line: 10,
            }],
            vec!["term1".to_string(), "term2".to_string()],
        );

        assert_eq!(input.file_path, "/docs/test.md");
        assert_eq!(input.sections.len(), 1);
        assert_eq!(input.vocabulary.len(), 2);
    }

    #[tokio::test]
    async fn chunked_document_produces_per_section_concepts() {
        // Mock llm-orc returns concepts that span multiple sections
        let llm_response = r#"{
            "concepts": [
                { "label": "revenge", "confidence": 0.95 },
                { "label": "madness", "confidence": 0.88 },
                { "label": "mortality", "confidence": 0.91 }
            ],
            "relationships": [
                {
                    "source": "madness",
                    "target": "revenge",
                    "relationship": "consequence_of",
                    "weight": 0.7
                }
            ]
        }"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "semantic-extraction",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "concept-extractor".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(llm_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "semantic-extraction");
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Pre-populate with file node (from Phase 1)
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("/docs/hamlet.txt"));
        }

        let sink = test_sink(ctx.clone());

        // Input with section boundaries (simulating Phase 2 output)
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::with_sections(
                "/docs/hamlet.txt",
                vec![
                    SectionBoundary {
                        label: "Act I".to_string(),
                        start_line: 1,
                        end_line: 500,
                    },
                    SectionBoundary {
                        label: "Act II".to_string(),
                        start_line: 501,
                        end_line: 1000,
                    },
                ],
            ),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Concepts extracted (llm-orc handled the fan-out internally)
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:revenge"))
                .is_some(),
            "revenge concept extracted"
        );
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:madness"))
                .is_some(),
            "madness concept extracted"
        );
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:mortality"))
                .is_some(),
            "mortality concept extracted"
        );

        // Relationship between concepts
        let rel_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "consequence_of")
            .collect();
        assert_eq!(rel_edges.len(), 1);

        // tagged_with edges from file to concepts
        let tagged_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_edges.len(), 3, "three tagged_with edges");
    }

    // --- Scenario: process() accepts LLM fallback field names ---

    #[tokio::test]
    async fn process_accepts_name_and_confidence_fallbacks() {
        // LLM returns "name" instead of "label", "confidence" instead of "weight"
        let llm_response = r#"{
            "concepts": [
                { "name": "rust", "confidence": 0.95 },
                { "name": "async", "confidence": 0.80 }
            ],
            "relationships": [
                {
                    "source": "rust",
                    "target": "async",
                    "relationship": "uses",
                    "confidence": 0.75
                }
            ]
        }"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "semantic-extraction",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "extractor".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(llm_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "semantic-extraction");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("/docs/test.rs"));
        }

        let sink = test_sink(ctx.clone());
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("/docs/test.rs"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Concepts created from "name" fallback
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:rust")).is_some(),
            "rust concept from 'name' fallback"
        );
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:async")).is_some(),
            "async concept from 'name' fallback"
        );

        // Relationship with "confidence" → weight fallback
        let uses_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "uses")
            .collect();
        assert_eq!(uses_edges.len(), 1);
        assert_eq!(uses_edges[0].source, NodeId::from_string("concept:rust"));
        assert_eq!(uses_edges[0].target, NodeId::from_string("concept:async"));
    }

    // --- Scenario: Multi-agent pipeline parses ALL agent results ---

    #[tokio::test]
    async fn multi_agent_results_all_parsed() {
        // All agents' concepts should appear — multi-run union (Essay 25).
        let agent_a_response = r#"{
            "concepts": [
                { "label": "merged concept", "confidence": 0.95 }
            ],
            "relationships": []
        }"#;

        let agent_b_response = r#"{
            "concepts": [
                { "label": "partial only", "confidence": 0.5 }
            ],
            "relationships": []
        }"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "semantic-extraction",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "entity-primed-1".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(agent_a_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m.insert(
                            "entity-primed-2".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(agent_b_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "semantic-extraction");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("/docs/fanout.md"));
        }

        let sink = test_sink(ctx.clone());
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("/docs/fanout.md"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Both agents' concepts should be present (multi-run union)
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:merged concept")).is_some(),
            "should extract from agent A"
        );
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:partial only")).is_some(),
            "should also extract from agent B (multi-run union)"
        );
    }

    // ================================================================
    // Essay 22 Item 4: SemanticAdapter Provenance Trail Scenarios
    // ================================================================

    /// Build a mock client + adapter that returns the given concepts.
    fn provenance_test_adapter(concepts_json: &str) -> SemanticAdapter {
        let mock_client = Arc::new(
            MockClient::available().with_response(
                "semantic-extraction",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "synthesizer".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(concepts_json.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );
        SemanticAdapter::new(mock_client, "semantic-extraction")
    }

    // --- Scenario: SemanticAdapter produces chain node ---

    #[tokio::test]
    async fn semantic_adapter_produces_chain_node() {
        let adapter = provenance_test_adapter(
            r#"{"concepts": [{"label": "testing", "confidence": 0.9}], "relationships": []}"#,
        );

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("test.txt"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();
        let chain = snapshot
            .get_node(&NodeId::from_string("chain:extract-semantic:test.txt"))
            .expect("chain node should exist");
        assert_eq!(chain.dimension, dimension::PROVENANCE);
        assert_eq!(chain.content_type, ContentType::Provenance);
    }

    // --- Scenario: SemanticAdapter produces mark per extracted passage ---

    #[tokio::test]
    async fn semantic_adapter_produces_mark_per_passage() {
        let adapter = provenance_test_adapter(
            r#"{"concepts": [{"label": "revenge", "confidence": 0.9}, {"label": "madness", "confidence": 0.85}], "relationships": []}"#,
        );

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        // Two sections → two marks
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::with_sections(
                "/docs/hamlet.txt",
                vec![
                    SectionBoundary { label: "Act I".to_string(), start_line: 1, end_line: 500 },
                    SectionBoundary { label: "Act II".to_string(), start_line: 501, end_line: 1000 },
                ],
            ),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Two mark nodes (one per section)
        let marks: Vec<_> = snapshot
            .nodes()
            .filter(|n| n.node_type == "mark")
            .collect();
        assert_eq!(marks.len(), 2, "should have two mark nodes (one per section)");

        // Each mark has concept labels as tags
        for mark in &marks {
            let tags = match mark.properties.get("tags") {
                Some(PropertyValue::Array(arr)) => arr,
                _ => panic!("mark should have tags array"),
            };
            assert!(tags.len() >= 2, "each mark should carry concept labels as tags");
            assert_eq!(mark.dimension, dimension::PROVENANCE);

            // Each mark has file_path and passage location
            assert!(mark.properties.contains_key("file_path"));
            assert!(mark.properties.contains_key("start_line"));
            assert!(mark.properties.contains_key("end_line"));
        }
    }

    // --- Scenario: SemanticAdapter produces contains edges ---

    #[tokio::test]
    async fn semantic_adapter_produces_contains_edges() {
        let adapter = provenance_test_adapter(
            r#"{"concepts": [{"label": "testing", "confidence": 0.9}], "relationships": []}"#,
        );

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        // Two sections → two marks → two contains edges
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::with_sections(
                "/docs/test.txt",
                vec![
                    SectionBoundary { label: "Section A".to_string(), start_line: 1, end_line: 50 },
                    SectionBoundary { label: "Section B".to_string(), start_line: 51, end_line: 100 },
                ],
            ),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();
        let chain_id = NodeId::from_string("chain:extract-semantic:/docs/test.txt");

        let contains_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "contains" && e.source == chain_id)
            .collect();
        assert_eq!(contains_edges.len(), 2, "two contains edges (chain → mark)");

        // Each contains edge has contribution from extract-semantic
        for edge in &contains_edges {
            assert_eq!(
                edge.contributions.get("extract-semantic"),
                Some(&1.0),
                "contains edge should have contribution of 1.0 from extract-semantic"
            );
        }
    }

    // --- Scenario: Per-agent contribution keys tracked ---

    #[tokio::test]
    async fn per_agent_contributions_tracked() {
        let response_a = r#"{"concepts": [{"label": "alpha"}], "relationships": []}"#;
        let response_b = r#"{"concepts": [{"label": "beta"}], "relationships": []}"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "extract-semantic",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "entity-primed-1".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(response_a.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m.insert(
                            "relationship-1".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(response_b.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "extract-semantic");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("test.md"));
        }

        let sink = test_sink(ctx.clone());
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("test.md"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // tagged_with edges should carry per-agent contribution keys
        let tagged: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "tagged_with")
            .collect();

        let has_primed = tagged.iter().any(|e| {
            e.contributions
                .contains_key("extract-phase3:entity-primed-1")
        });
        let has_rel = tagged.iter().any(|e| {
            e.contributions
                .contains_key("extract-phase3:relationship-1")
        });

        assert!(has_primed, "should have contribution from entity-primed-1");
        assert!(has_rel, "should have contribution from relationship-1");
    }

    // --- Scenario: SpaCy response creates entities and relationships ---

    #[tokio::test]
    async fn spacy_response_creates_entities_and_relationships() {
        let spacy_response = r#"{
            "success": true,
            "data": {
                "entities": [
                    {"label": "Plexus", "type": "component"},
                    {"label": "knowledge graph", "type": "concept"}
                ],
                "relationships": [
                    {"source": "Plexus", "target": "knowledge graph", "relationship": "implement", "evidence": "Plexus implements a knowledge graph"}
                ],
                "cooccurrences": [
                    {"entity_a": "Plexus", "entity_b": "knowledge graph"}
                ]
            }
        }"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "extract-semantic",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "spacy-extract".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(spacy_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "extract-semantic");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("test.md"));
        }

        let sink = test_sink(ctx.clone());
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("test.md"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Concept nodes from SpaCy entities
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:plexus"))
                .is_some(),
            "SpaCy entity should become concept node"
        );
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:knowledge graph"))
                .is_some(),
            "SpaCy entity should become concept node"
        );

        // Typed relationship from SVO triple
        let implement_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "implement")
            .collect();
        assert_eq!(implement_edges.len(), 1, "one typed relationship from SVO");

        // Co-occurrence edges (symmetric pair)
        let cooc_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "may_be_related")
            .collect();
        assert_eq!(cooc_edges.len(), 2, "co-occurrence produces symmetric pair");

        // Contribution keys from SpaCy agent
        for edge in &implement_edges {
            assert!(
                edge.contributions
                    .contains_key("extract-phase3:spacy-extract"),
                "SpaCy edges should carry spacy-extract contribution"
            );
        }
    }

    // --- Scenario: Theme response creates concept nodes ---

    #[tokio::test]
    async fn theme_response_creates_concept_nodes() {
        let theme_response = r#"{
            "themes": [
                {
                    "description": "cognitive offloading",
                    "type": "concept",
                    "supporting_evidence": "The paper argues that..."
                },
                {
                    "description": "material disengagement",
                    "type": "tension",
                    "supporting_evidence": "When practitioners..."
                }
            ]
        }"#;

        let mock_client = Arc::new(
            MockClient::available().with_response(
                "extract-semantic",
                crate::llm_orc::InvokeResponse {
                    results: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "theme".to_string(),
                            crate::llm_orc::AgentResult {
                                response: Some(theme_response.to_string()),
                                status: Some("success".to_string()),
                                error: None,
                            },
                        );
                        m
                    },
                    status: "completed".to_string(),
                    metadata: serde_json::Value::Null,
                },
            ),
        );

        let adapter = SemanticAdapter::new(mock_client, "extract-semantic");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(crate::adapter::file_node("test.md"));
        }

        let sink = test_sink(ctx.clone());
        let input = AdapterInput::new(
            "extract-semantic",
            SemanticInput::for_file("test.md"),
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Theme concept nodes
        let cog = snapshot
            .get_node(&NodeId::from_string("concept:cognitive offloading"))
            .expect("theme should become concept node");
        assert_eq!(
            cog.properties.get("source"),
            Some(&PropertyValue::String("theme".to_string())),
            "theme concepts have source=theme"
        );
        assert_eq!(
            cog.properties.get("concept_type"),
            Some(&PropertyValue::String("concept".to_string())),
        );

        let mat = snapshot
            .get_node(&NodeId::from_string("concept:material disengagement"))
            .expect("second theme should become concept node");
        assert_eq!(
            mat.properties.get("concept_type"),
            Some(&PropertyValue::String("tension".to_string())),
        );

        // tagged_with edges from file to theme concepts
        let tagged: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged.len(), 2, "two tagged_with edges for themes");

        // Contribution key includes agent name
        for edge in &tagged {
            assert!(
                edge.contributions.contains_key("extract-phase3:theme"),
                "theme edges should carry theme contribution"
            );
        }
    }
}
