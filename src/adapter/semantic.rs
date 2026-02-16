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
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission};
use crate::graph::{dimension, ContentType, Context, Edge, Node, NodeId, PropertyValue};
use crate::llm_orc::{LlmOrcClient, LlmOrcError};
use async_trait::async_trait;
use std::sync::Arc;

/// Input for the semantic adapter.
///
/// Extends the basic file path with optional section boundaries from Phase 2.
/// When sections are present, llm-orc can chunk the document along structural
/// boundaries for parallel fan-out (ADR-021 Scenario 3).
#[derive(Debug, Clone)]
pub struct SemanticInput {
    pub file_path: String,
    /// Section boundaries identified by Phase 2 (heuristic analysis).
    /// Each entry is (label, start_line, end_line). Empty = process whole file.
    pub sections: Vec<SectionBoundary>,
}

/// A structural boundary identified by Phase 2 analysis.
#[derive(Debug, Clone)]
pub struct SectionBoundary {
    pub label: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl SemanticInput {
    /// Create input for a single file with no section boundaries.
    pub fn for_file(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            sections: Vec::new(),
        }
    }

    /// Create input with section boundaries from Phase 2.
    pub fn with_sections(
        file_path: impl Into<String>,
        sections: Vec<SectionBoundary>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            sections,
        }
    }
}

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
    /// Reads Phase 2 output from the shared context and serializes
    /// relevant information (file path, extracted terms, sections) as text.
    /// When sections are present, includes them so llm-orc can chunk along
    /// structural boundaries (ADR-021 Scenario 3).
    fn build_input(
        &self,
        input: &SemanticInput,
        context: Option<&Context>,
    ) -> String {
        let mut parts = vec![format!("file: {}", input.file_path)];

        // Include section boundaries from Phase 2
        if !input.sections.is_empty() {
            let section_strs: Vec<String> = input
                .sections
                .iter()
                .map(|s| format!("{}:{}-{}", s.label, s.start_line, s.end_line))
                .collect();
            parts.push(format!("sections: {}", section_strs.join(", ")));
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
                parts.push(format!("existing concepts: {}", concepts.join(", ")));
            }

            // Include file metadata if available
            let file_node_id = NodeId::from_string(format!("file:{}", input.file_path));
            if let Some(file_node) = ctx.get_node(&file_node_id) {
                if let Some(PropertyValue::String(mime)) =
                    file_node.properties.get("mime_type")
                {
                    parts.push(format!("mime_type: {}", mime));
                }
            }
        }

        parts.join("\n")
    }

    /// Parse llm-orc response into an emission of concept nodes and edges.
    ///
    /// Expects the final agent's response to be a JSON object with:
    /// ```json
    /// {
    ///   "concepts": [
    ///     { "label": "machine learning", "confidence": 0.9 },
    ///     { "label": "neural networks", "confidence": 0.85 }
    ///   ],
    ///   "relationships": [
    ///     { "source": "neural networks", "target": "machine learning",
    ///       "relationship": "is_a", "weight": 0.8 }
    ///   ]
    /// }
    /// ```
    fn parse_response(
        &self,
        response_text: &str,
        file_path: &str,
    ) -> Result<Emission, AdapterError> {
        let parsed: serde_json::Value = serde_json::from_str(response_text)
            .map_err(|e| AdapterError::Internal(format!("failed to parse llm-orc response: {}", e)))?;

        let mut emission = Emission::new();
        let file_node_id = NodeId::from_string(format!("file:{}", file_path));

        // Parse concepts
        if let Some(concepts) = parsed.get("concepts").and_then(|v| v.as_array()) {
            for concept in concepts {
                let label = concept
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                if label.is_empty() {
                    continue;
                }

                let normalized = label.to_lowercase();
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

                if let Some(confidence) = concept.get("confidence").and_then(|v| v.as_f64()) {
                    node.properties.insert(
                        "confidence".to_string(),
                        PropertyValue::Float(confidence),
                    );
                }

                emission = emission.with_node(AnnotatedNode::new(node));

                // tagged_with edge: file → concept
                let mut edge = Edge::new_cross_dimensional(
                    file_node_id.clone(),
                    dimension::STRUCTURE,
                    concept_id,
                    dimension::SEMANTIC,
                    "tagged_with",
                );
                edge.raw_weight = 1.0;
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }
        }

        // Parse relationships between concepts
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
                edge.raw_weight = weight as f32;
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }
        }

        Ok(emission)
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

        // Find the final agent's response (last in the pipeline)
        // Convention: the last agent produces the structured output
        let response_text = response
            .results
            .values()
            .filter_map(|r| r.response.as_deref())
            .last()
            .ok_or_else(|| {
                AdapterError::Internal("no agent responses in llm-orc result".to_string())
            })?;

        // Parse response into emission
        let emission = self.parse_response(response_text, file_path)?;

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
            let mut file_node = Node::new_in_dimension(
                "file",
                ContentType::Document,
                dimension::STRUCTURE,
            );
            file_node.id = NodeId::from_string("file:/docs/example.md");
            c.add_node(file_node);
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

        assert!(payload.contains("file: /docs/hamlet.txt"));
        assert!(payload.contains("sections:"));
        assert!(payload.contains("Act I:1-500"));
        assert!(payload.contains("Act II:501-1000"));
        assert!(payload.contains("Act III:1001-1500"));
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
            let mut file_node = Node::new_in_dimension(
                "file",
                ContentType::Document,
                dimension::STRUCTURE,
            );
            file_node.id = NodeId::from_string("file:/docs/hamlet.txt");
            c.add_node(file_node);
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
}
