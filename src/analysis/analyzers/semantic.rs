//! Semantic analyzer using llm-orc MCP integration
//!
//! Extracts concepts and semantic relationships from content using LLM analysis.
//! Populates the `semantic` dimension of the knowledge graph.

use crate::analysis::{
    AnalysisCapability, AnalysisError, AnalysisResult, AnalysisScope, ContentAnalyzer,
};
use crate::graph::{ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Configuration for the semantic analyzer
#[derive(Debug, Clone)]
pub struct SemanticAnalyzerConfig {
    /// Name of the llm-orc ensemble to use
    pub ensemble_name: String,
    /// Path to llm-orc executable
    pub llm_orc_path: String,
    /// Project path for llm-orc context
    pub project_path: Option<String>,
    /// Timeout for LLM requests in seconds
    pub timeout_seconds: u64,
    /// Maximum content size to analyze (skip larger files)
    pub max_content_size: usize,
}

impl Default for SemanticAnalyzerConfig {
    fn default() -> Self {
        Self {
            ensemble_name: "plexus-semantic".to_string(),
            llm_orc_path: "llm-orc".to_string(),
            project_path: None,
            timeout_seconds: 60,
            max_content_size: 50_000, // 50KB default
        }
    }
}

/// MCP JSON-RPC request
#[derive(Debug, Serialize)]
struct McpRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: serde_json::Value,
}

/// MCP JSON-RPC response
#[derive(Debug, Deserialize)]
struct McpResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<serde_json::Value>,
    error: Option<McpError>,
}

#[derive(Debug, Deserialize)]
struct McpError {
    code: i64,
    message: String,
}

/// Concept extracted from content
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractedConcept {
    pub name: String,
    #[serde(rename = "type")]
    pub concept_type: String,
    pub confidence: f64,
}

/// Relationship between concepts
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractedRelationship {
    pub source: String,
    pub target: String,
    pub relationship: String,
    pub confidence: f64,
}

/// Result from semantic analysis
#[derive(Debug, Clone, Deserialize)]
pub struct SemanticExtractionResult {
    pub concepts: Vec<ExtractedConcept>,
    pub relationships: Vec<ExtractedRelationship>,
}

/// MCP client for communicating with llm-orc
struct McpClient {
    process: Child,
    request_id: u64,
}

impl McpClient {
    /// Start the MCP server process
    async fn start(llm_orc_path: &str, project_path: Option<&str>) -> Result<Self, AnalysisError> {
        let mut cmd = Command::new(llm_orc_path);
        cmd.args(["mcp", "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(path) = project_path {
            cmd.current_dir(path);
        }

        let process = cmd.spawn().map_err(|e| {
            AnalysisError::LlmUnavailable(format!("Failed to start llm-orc MCP server: {}", e))
        })?;

        Ok(Self {
            process,
            request_id: 0,
        })
    }

    /// Send a request and receive response
    async fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout_duration: Duration,
    ) -> Result<serde_json::Value, AnalysisError> {
        self.request_id += 1;
        let request = McpRequest {
            jsonrpc: "2.0",
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let stdin = self
            .process
            .stdin
            .as_mut()
            .ok_or_else(|| AnalysisError::LlmUnavailable("No stdin available".to_string()))?;

        let request_json = serde_json::to_string(&request).map_err(|e| {
            AnalysisError::LlmUnavailable(format!("Failed to serialize request: {}", e))
        })?;

        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| AnalysisError::LlmUnavailable(format!("Failed to write request: {}", e)))?;
        stdin.write_all(b"\n").await.map_err(|e| {
            AnalysisError::LlmUnavailable(format!("Failed to write newline: {}", e))
        })?;
        stdin.flush().await.map_err(|e| {
            AnalysisError::LlmUnavailable(format!("Failed to flush stdin: {}", e))
        })?;

        // Read response with timeout
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or_else(|| AnalysisError::LlmUnavailable("No stdout available".to_string()))?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        let read_result = timeout(timeout_duration, reader.read_line(&mut line)).await;

        match read_result {
            Ok(Ok(_)) => {
                let response: McpResponse = serde_json::from_str(&line).map_err(|e| {
                    AnalysisError::LlmUnavailable(format!("Failed to parse response: {}", e))
                })?;

                if let Some(error) = response.error {
                    return Err(AnalysisError::LlmUnavailable(format!(
                        "MCP error {}: {}",
                        error.code, error.message
                    )));
                }

                response
                    .result
                    .ok_or_else(|| AnalysisError::LlmUnavailable("Empty response".to_string()))
            }
            Ok(Err(e)) => Err(AnalysisError::LlmUnavailable(format!(
                "Failed to read response: {}",
                e
            ))),
            Err(_) => Err(AnalysisError::LlmUnavailable("Request timed out".to_string())),
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Kill the process when dropped
        let _ = self.process.start_kill();
    }
}

/// Analyzer that extracts semantic concepts and relationships using LLM
///
/// Target dimension: `semantic`
///
/// Creates nodes for:
/// - `concept` with name, type, confidence
///
/// Creates edges for:
/// - Various semantic relationships (implements, describes, depends_on, etc.)
pub struct SemanticAnalyzer {
    config: SemanticAnalyzerConfig,
    available: Arc<AtomicBool>,
    client: Arc<Mutex<Option<McpClient>>>,
    priority: u32,
}

impl SemanticAnalyzer {
    pub fn new(config: SemanticAnalyzerConfig) -> Self {
        Self {
            config,
            available: Arc::new(AtomicBool::new(true)), // Assume available until proven otherwise
            client: Arc::new(Mutex::new(None)),
            priority: 100, // Run after programmatic analyzers
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Check if llm-orc is available
    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }

    /// Mark llm-orc as unavailable (after failure)
    fn mark_unavailable(&self) {
        self.available.store(false, Ordering::Relaxed);
    }

    /// Ensure MCP client is connected
    async fn ensure_client(&self) -> Result<(), AnalysisError> {
        let mut client_guard = self.client.lock().await;
        if client_guard.is_none() {
            let client = McpClient::start(
                &self.config.llm_orc_path,
                self.config.project_path.as_deref(),
            )
            .await?;
            *client_guard = Some(client);
        }
        Ok(())
    }

    /// Invoke the semantic analysis ensemble
    async fn invoke_ensemble(&self, content: &str) -> Result<SemanticExtractionResult, AnalysisError> {
        self.ensure_client().await?;

        let mut client_guard = self.client.lock().await;
        let client = client_guard
            .as_mut()
            .ok_or_else(|| AnalysisError::LlmUnavailable("No client available".to_string()))?;

        let params = serde_json::json!({
            "name": "invoke",
            "arguments": {
                "ensemble_name": self.config.ensemble_name,
                "input_data": content
            }
        });

        let timeout_duration = Duration::from_secs(self.config.timeout_seconds);
        let result = client.call("tools/call", params, timeout_duration).await?;

        // Parse the result
        self.parse_llm_response(result)
    }

    /// Parse LLM response into semantic extraction result
    fn parse_llm_response(
        &self,
        response: serde_json::Value,
    ) -> Result<SemanticExtractionResult, AnalysisError> {
        // The response structure depends on llm-orc's output format
        // Try to extract the semantic-analyzer agent's response
        let result_str = response
            .get("result")
            .and_then(|r| r.as_str())
            .or_else(|| response.as_str())
            .ok_or_else(|| {
                AnalysisError::LlmUnavailable("Invalid response format".to_string())
            })?;

        // Parse the JSON from the result string
        // The result may be wrapped in the llm-orc response structure
        let parsed: serde_json::Value = serde_json::from_str(result_str).map_err(|e| {
            AnalysisError::LlmUnavailable(format!("Failed to parse LLM response JSON: {}", e))
        })?;

        // Try to find the semantic-analyzer result
        let extraction = if let Some(results) = parsed.get("results") {
            // Multi-agent response format
            if let Some(analyzer_result) = results.get("semantic-analyzer") {
                let response_str = analyzer_result
                    .get("response")
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                self.parse_extraction_json(response_str)?
            } else {
                // Try first agent result
                self.parse_extraction_json(result_str)?
            }
        } else if parsed.get("concepts").is_some() {
            // Direct extraction result
            serde_json::from_value(parsed).map_err(|e| {
                AnalysisError::LlmUnavailable(format!("Failed to parse extraction result: {}", e))
            })?
        } else {
            return Err(AnalysisError::LlmUnavailable(
                "Could not find extraction result in response".to_string(),
            ));
        };

        Ok(extraction)
    }

    /// Parse extraction JSON, handling potential extra text
    fn parse_extraction_json(&self, text: &str) -> Result<SemanticExtractionResult, AnalysisError> {
        // Try to find JSON in the text (LLMs sometimes add extra text)
        let json_start = text.find('{');
        let json_end = text.rfind('}');

        if let (Some(start), Some(end)) = (json_start, json_end) {
            let json_str = &text[start..=end];
            serde_json::from_str(json_str).map_err(|e| {
                AnalysisError::LlmUnavailable(format!("Failed to parse extraction JSON: {}", e))
            })
        } else {
            Err(AnalysisError::LlmUnavailable(
                "No JSON found in LLM response".to_string(),
            ))
        }
    }

    /// Convert extraction result to graph nodes and edges
    fn extraction_to_graph(
        &self,
        extraction: &SemanticExtractionResult,
        source_id: &str,
        result: &mut AnalysisResult,
    ) {
        let mut concept_node_ids: HashMap<String, NodeId> = HashMap::new();

        // Create concept nodes
        for concept in &extraction.concepts {
            let node_id = NodeId::from_string(format!(
                "concept:{}",
                concept.name.to_lowercase().replace(' ', "-")
            ));

            let mut node = Node::new_in_dimension("concept", ContentType::Concept, "semantic");
            node.id = node_id.clone();
            node.properties
                .insert("name".into(), PropertyValue::String(concept.name.clone()));
            node.properties.insert(
                "concept_type".into(),
                PropertyValue::String(concept.concept_type.clone()),
            );
            node.properties.insert(
                "confidence".into(),
                PropertyValue::Float(concept.confidence),
            );
            node.properties.insert(
                "_source_content".into(),
                PropertyValue::String(source_id.to_string()),
            );

            result.nodes.push(node);
            concept_node_ids.insert(concept.name.to_lowercase(), node_id);
        }

        // Create relationship edges
        for rel in &extraction.relationships {
            let source_key = rel.source.to_lowercase();
            let target_key = rel.target.to_lowercase();

            if let (Some(source_node_id), Some(target_node_id)) =
                (concept_node_ids.get(&source_key), concept_node_ids.get(&target_key))
            {
                // Clean up relationship type (handle "implements|describes" format)
                let relationship = rel
                    .relationship
                    .split('|')
                    .next()
                    .unwrap_or(&rel.relationship)
                    .to_string();

                let mut edge = Edge::new_in_dimension(
                    source_node_id.clone(),
                    target_node_id.clone(),
                    &relationship,
                    "semantic",
                );
                edge.raw_weight = rel.confidence as f32;
                edge.properties.insert(
                    "_source_content".into(),
                    PropertyValue::String(source_id.to_string()),
                );

                result.edges.push(edge);
            }
        }

        // Create cross-dimensional edge from source document to concepts
        let doc_node_id = NodeId::from_string(format!("{}:document", source_id));
        for concept_node_id in concept_node_ids.values() {
            let edge = Edge::new_cross_dimensional(
                doc_node_id.clone(),
                "structure".to_string(),
                concept_node_id.clone(),
                "semantic".to_string(),
                "discusses",
            );
            result.edges.push(edge);
        }
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new(SemanticAnalyzerConfig::default())
    }
}

#[async_trait]
impl ContentAnalyzer for SemanticAnalyzer {
    fn id(&self) -> &str {
        "semantic-analyzer"
    }

    fn name(&self) -> &str {
        "Semantic Analyzer (LLM)"
    }

    fn dimensions(&self) -> Vec<&str> {
        vec!["semantic"]
    }

    fn capabilities(&self) -> Vec<AnalysisCapability> {
        vec![AnalysisCapability::Semantics, AnalysisCapability::Entities]
    }

    fn handles(&self) -> Vec<ContentType> {
        vec![ContentType::Document, ContentType::Code]
    }

    fn requires_llm(&self) -> bool {
        true
    }

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn analyze(&self, scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
        let mut result = AnalysisResult::new();

        // Check if LLM analysis is enabled
        if !scope.config.enable_llm_analysis {
            result.add_warning("LLM analysis disabled in config".to_string());
            return Ok(result);
        }

        // Check if llm-orc is available
        if !self.is_available() {
            result.add_warning("llm-orc is not available, skipping semantic analysis".to_string());
            return Ok(result);
        }

        for item in scope.items_to_analyze() {
            // Skip non-document content
            if !matches!(item.content_type, ContentType::Document | ContentType::Code) {
                continue;
            }

            // Skip large files
            if item.content.len() > self.config.max_content_size {
                result.add_warning(format!(
                    "Skipping {} - content too large for LLM analysis ({} bytes)",
                    item.id.as_str(),
                    item.content.len()
                ));
                continue;
            }

            // Skip empty content
            if item.content.trim().is_empty() {
                continue;
            }

            // Invoke LLM analysis
            match self.invoke_ensemble(&item.content).await {
                Ok(extraction) => {
                    self.extraction_to_graph(&extraction, item.id.as_str(), &mut result);
                }
                Err(e) => {
                    // Log warning but don't fail the whole analysis
                    result.add_warning(format!(
                        "Semantic analysis failed for {}: {}",
                        item.id.as_str(),
                        e
                    ));

                    // If this is a connection error, mark llm-orc as unavailable
                    if matches!(e, AnalysisError::LlmUnavailable(_)) {
                        self.mark_unavailable();
                    }
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extraction_json() {
        let analyzer = SemanticAnalyzer::default();

        let json = r#"{"concepts": [{"name": "rust", "type": "technology", "confidence": 0.9}], "relationships": []}"#;
        let result = analyzer.parse_extraction_json(json).unwrap();

        assert_eq!(result.concepts.len(), 1);
        assert_eq!(result.concepts[0].name, "rust");
        assert_eq!(result.concepts[0].concept_type, "technology");
    }

    #[test]
    fn test_parse_extraction_json_with_extra_text() {
        let analyzer = SemanticAnalyzer::default();

        let json = r#"Here is the analysis:
{"concepts": [{"name": "test", "type": "concept", "confidence": 0.8}], "relationships": []}
That's the result."#;

        let result = analyzer.parse_extraction_json(json).unwrap();
        assert_eq!(result.concepts.len(), 1);
        assert_eq!(result.concepts[0].name, "test");
    }

    #[test]
    fn test_extraction_to_graph() {
        let analyzer = SemanticAnalyzer::default();
        let mut result = AnalysisResult::new();

        let extraction = SemanticExtractionResult {
            concepts: vec![
                ExtractedConcept {
                    name: "rust".to_string(),
                    concept_type: "technology".to_string(),
                    confidence: 0.9,
                },
                ExtractedConcept {
                    name: "plexus".to_string(),
                    concept_type: "entity".to_string(),
                    confidence: 0.85,
                },
            ],
            relationships: vec![ExtractedRelationship {
                source: "plexus".to_string(),
                target: "rust".to_string(),
                relationship: "uses".to_string(),
                confidence: 0.8,
            }],
        };

        analyzer.extraction_to_graph(&extraction, "test.md", &mut result);

        // Should have 2 concept nodes
        let concept_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "concept")
            .collect();
        assert_eq!(concept_nodes.len(), 2);

        // Should have 1 semantic edge + 2 cross-dimensional edges
        assert!(result.edges.len() >= 1);

        let uses_edges: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "uses")
            .collect();
        assert_eq!(uses_edges.len(), 1);
    }

    #[test]
    fn test_config_defaults() {
        let config = SemanticAnalyzerConfig::default();
        assert_eq!(config.ensemble_name, "plexus-semantic");
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.max_content_size, 50_000);
    }
}
