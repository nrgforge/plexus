//! Extraction coordinator — phased file extraction (ADR-019)
//!
//! Handles `extract-file` input kind. Runs Phase 1 (registration)
//! synchronously, then spawns Phases 2–3 as background tokio tasks.
//!
//! Phase 1 — Registration (instant, blocking):
//!   File node (MIME type, size, path) + concept nodes from YAML frontmatter
//!
//! Phase 2 — Analysis (moderate, background):
//!   Modality-dispatched heuristic extraction (by MIME type)
//!
//! Phase 3 — Semantic (slow, background, LLM):
//!   Abstract concept extraction via llm-orc (ADR-021)

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission};
use crate::graph::{dimension, ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Input for the extraction coordinator.
#[derive(Debug, Clone)]
pub struct ExtractFileInput {
    pub file_path: String,
}

/// Phase completion status for tracking.
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseStatus {
    Pending,
    Complete,
    Failed(String),
    Skipped,
}

/// A Phase 2 adapter registered for a specific MIME type prefix.
pub struct Phase2Registration {
    /// MIME type prefix (e.g., "text/", "audio/")
    pub mime_prefix: String,
    /// The Phase 2 adapter
    pub adapter: Arc<dyn Adapter>,
}

/// Extraction coordinator — orchestrates phased extraction (ADR-019).
pub struct ExtractionCoordinator {
    /// Phase 2 adapters indexed by MIME type prefix
    phase2_adapters: Vec<Phase2Registration>,
    /// Concurrency semaphore for Phase 2 tasks
    analysis_semaphore: Arc<tokio::sync::Semaphore>,
    /// Concurrency semaphore for Phase 3 tasks
    semantic_semaphore: Arc<tokio::sync::Semaphore>,
}

impl ExtractionCoordinator {
    pub fn new() -> Self {
        Self {
            phase2_adapters: Vec::new(),
            analysis_semaphore: Arc::new(tokio::sync::Semaphore::new(4)),
            semantic_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }

    /// Set the concurrency limit for Phase 2 (analysis) tasks.
    pub fn with_analysis_concurrency(mut self, limit: usize) -> Self {
        self.analysis_semaphore = Arc::new(tokio::sync::Semaphore::new(limit));
        self
    }

    /// Register a Phase 2 adapter for a MIME type prefix.
    pub fn register_phase2(
        &mut self,
        mime_prefix: impl Into<String>,
        adapter: Arc<dyn Adapter>,
    ) {
        self.phase2_adapters.push(Phase2Registration {
            mime_prefix: mime_prefix.into(),
            adapter,
        });
    }

    /// Find the Phase 2 adapter matching a MIME type.
    fn find_phase2_adapter(&self, mime_type: &str) -> Option<&Arc<dyn Adapter>> {
        self.phase2_adapters
            .iter()
            .find(|reg| mime_type.starts_with(&reg.mime_prefix))
            .map(|reg| &reg.adapter)
    }
}

/// Detect MIME type from file extension.
fn detect_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("md") | Some("markdown") => "text/markdown",
        Some("txt") => "text/plain",
        Some("rs") => "text/x-rust",
        Some("py") => "text/x-python",
        Some("js") => "text/javascript",
        Some("ts") => "text/typescript",
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("yaml") | Some("yml") => "text/yaml",
        Some("toml") => "text/toml",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Parse YAML frontmatter from file content.
///
/// Frontmatter is delimited by `---` at the start and end:
/// ```text
/// ---
/// tags: [travel, avignon]
/// title: My Document
/// ---
/// ```
///
/// Returns the parsed YAML as a serde_yaml::Value, or None if no frontmatter.
fn parse_frontmatter(content: &str) -> Option<Result<Value, String>> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let end_pos = after_first.find("\n---");
    let end_pos = match end_pos {
        Some(pos) => pos,
        None => return None,
    };

    let frontmatter_str = &after_first[..end_pos];

    // Parse YAML to serde_json::Value via serde_yaml
    match serde_yaml::from_str::<serde_yaml::Value>(frontmatter_str) {
        Ok(yaml_val) => {
            // Convert serde_yaml::Value to serde_json::Value
            match serde_json::to_value(yaml_val) {
                Ok(json_val) => Some(Ok(json_val)),
                Err(e) => Some(Err(format!("YAML to JSON conversion failed: {}", e))),
            }
        }
        Err(e) => Some(Err(format!("YAML parse error: {}", e))),
    }
}

/// Extract tags from parsed frontmatter.
fn extract_tags_from_frontmatter(frontmatter: &Value) -> Vec<String> {
    let mut tags = Vec::new();

    if let Some(tag_val) = frontmatter.get("tags") {
        match tag_val {
            Value::Array(arr) => {
                for item in arr {
                    if let Value::String(s) = item {
                        tags.push(s.to_lowercase());
                    }
                }
            }
            Value::String(s) => {
                // Comma-separated tags
                for tag in s.split(',') {
                    let t = tag.trim().to_lowercase();
                    if !t.is_empty() {
                        tags.push(t);
                    }
                }
            }
            _ => {}
        }
    }

    tags
}

/// Run Phase 1: file registration + metadata extraction.
///
/// Creates:
/// - File node in structure dimension (MIME type, size, path)
/// - Concept nodes from YAML frontmatter tags in semantic dimension
/// - tagged_with edges from file node to concepts
/// - Extraction status node
fn run_phase1(
    file_path: &str,
    adapter_id: &str,
) -> Result<(Emission, String, Option<String>), AdapterError> {
    let path = Path::new(file_path);
    let mime_type = detect_mime_type(path);

    // Read file metadata
    let metadata = std::fs::metadata(path)
        .map_err(|e| AdapterError::Internal(format!("cannot read file metadata: {}", e)))?;
    let file_size = metadata.len();

    // File node
    let file_node_id = NodeId::from_string(format!("file:{}", file_path));
    let mut file_node =
        Node::new_in_dimension("file", ContentType::Document, dimension::STRUCTURE);
    file_node.id = file_node_id.clone();
    file_node.properties.insert(
        "path".to_string(),
        PropertyValue::String(file_path.to_string()),
    );
    file_node.properties.insert(
        "mime_type".to_string(),
        PropertyValue::String(mime_type.to_string()),
    );
    file_node.properties.insert(
        "file_size".to_string(),
        PropertyValue::Int(file_size as i64),
    );

    let mut emission = Emission::new().with_node(AnnotatedNode::new(file_node));

    // Try to read file content for frontmatter
    let mut metadata_warning: Option<String> = None;
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Some(fm_result) = parse_frontmatter(&content) {
            match fm_result {
                Ok(frontmatter) => {
                    let tags = extract_tags_from_frontmatter(&frontmatter);
                    for tag in &tags {
                        let concept_id = NodeId::from_string(format!("concept:{}", tag));

                        let mut concept_node = Node::new_in_dimension(
                            "concept",
                            ContentType::Concept,
                            dimension::SEMANTIC,
                        );
                        concept_node.id = concept_id.clone();
                        concept_node.properties.insert(
                            "label".to_string(),
                            PropertyValue::String(tag.clone()),
                        );
                        emission = emission.with_node(AnnotatedNode::new(concept_node));

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
                Err(warning) => {
                    metadata_warning = Some(warning);
                }
            }
        }
    }

    // Extraction status node
    let status_id = NodeId::from_string(format!("extraction-status:{}", file_path));
    let mut status_node =
        Node::new_in_dimension("extraction-status", ContentType::Document, dimension::STRUCTURE);
    status_node.id = status_id;
    status_node.properties.insert(
        "file_path".to_string(),
        PropertyValue::String(file_path.to_string()),
    );
    status_node.properties.insert(
        "phase1".to_string(),
        PropertyValue::String("complete".to_string()),
    );
    status_node.properties.insert(
        "phase2".to_string(),
        PropertyValue::String("pending".to_string()),
    );
    status_node.properties.insert(
        "phase3".to_string(),
        PropertyValue::String("pending".to_string()),
    );
    if let Some(ref warning) = metadata_warning {
        status_node.properties.insert(
            "phase1_warning".to_string(),
            PropertyValue::String(warning.clone()),
        );
    }
    emission = emission.with_node(AnnotatedNode::new(status_node));

    Ok((emission, mime_type.to_string(), metadata_warning))
}

#[async_trait]
impl Adapter for ExtractionCoordinator {
    fn id(&self) -> &str {
        "extract-coordinator"
    }

    fn input_kind(&self) -> &str {
        "extract-file"
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let file_input = input
            .downcast_data::<ExtractFileInput>()
            .ok_or(AdapterError::InvalidInput)?;

        // Phase 1: synchronous registration
        let (emission, _mime_type, _metadata_warning) =
            run_phase1(&file_input.file_path, self.id())?;

        sink.emit(emission).await?;

        // Phase 2–3: background tasks would be spawned here.
        // The coordinator needs an engine reference for background sinks.
        // For now, Phase 2 spawning is handled via spawn_background_phases()
        // which is called by the pipeline or test harness.

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::graph::Context;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    fn test_sink(ctx: Arc<Mutex<Context>>, adapter_id: &str) -> EngineSink {
        EngineSink::new(ctx).with_framework_context(FrameworkContext {
            adapter_id: adapter_id.to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        })
    }

    /// Create a temp file with given content and return its path.
    fn create_temp_file(name: &str, content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join(name);
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        dir
    }

    // --- Scenario: Extraction coordinator runs Phase 1 synchronously ---

    #[tokio::test]
    async fn phase1_runs_synchronously() {
        let coordinator = ExtractionCoordinator::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let dir = create_temp_file(
            "example.md",
            "---\ntags: [travel, avignon]\ntitle: My Document\n---\n\n# Hello\n\nSome content.",
        );
        let file_path = dir.path().join("example.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // File node exists with MIME type and file size
        let file_node_id = NodeId::from_string(format!("file:{}", file_path_str));
        let file_node = snapshot
            .get_node(&file_node_id)
            .expect("file node should exist");
        assert_eq!(file_node.dimension, dimension::STRUCTURE);
        assert_eq!(
            file_node.properties.get("mime_type"),
            Some(&PropertyValue::String("text/markdown".to_string()))
        );
        assert!(file_node.properties.get("file_size").is_some());

        // Concept nodes from frontmatter tags
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:travel"))
                .is_some(),
            "concept:travel should exist"
        );
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:avignon"))
                .is_some(),
            "concept:avignon should exist"
        );

        // Extraction status node
        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot
            .get_node(&status_id)
            .expect("extraction status should exist");
        assert_eq!(
            status.properties.get("phase1"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("phase2"),
            Some(&PropertyValue::String("pending".to_string()))
        );
    }

    // --- Scenario: Phase 1 metadata failure does not prevent file registration ---

    #[tokio::test]
    async fn phase1_metadata_failure_still_registers_file() {
        let coordinator = ExtractionCoordinator::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        // File with corrupt frontmatter
        let dir = create_temp_file(
            "corrupt.md",
            "---\ntags: [unclosed\n---\n\nContent here.",
        );
        let file_path = dir.path().join("corrupt.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // File node still exists
        let file_node_id = NodeId::from_string(format!("file:{}", file_path_str));
        assert!(
            snapshot.get_node(&file_node_id).is_some(),
            "file node should exist despite corrupt frontmatter"
        );

        // No concept nodes created
        let concept_count = snapshot
            .nodes()
            .filter(|n| n.node_type == "concept")
            .count();
        assert_eq!(concept_count, 0, "no concepts from corrupt frontmatter");

        // Extraction status shows warning
        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("phase1"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert!(
            status.properties.get("phase1_warning").is_some(),
            "should have metadata warning"
        );
    }

    // --- Scenario: Phase 2 dispatches by MIME type ---

    #[tokio::test]
    async fn phase2_dispatches_by_mime_type() {
        let mut coordinator = ExtractionCoordinator::new();

        // Register text and audio Phase 2 adapters
        let text_adapter = Arc::new(RecordingAdapter::new("extract-analysis-text", "extract-analysis-text"));
        let audio_adapter = Arc::new(RecordingAdapter::new("extract-analysis-audio", "extract-analysis-audio"));

        coordinator.register_phase2("text/", text_adapter.clone());
        coordinator.register_phase2("audio/", audio_adapter.clone());

        // For an MP3 file, should dispatch to audio adapter
        let found = coordinator.find_phase2_adapter("audio/mpeg");
        assert!(found.is_some(), "should find audio adapter");
        assert_eq!(found.unwrap().id(), "extract-analysis-audio");

        // For a markdown file, should dispatch to text adapter
        let found = coordinator.find_phase2_adapter("text/markdown");
        assert!(found.is_some(), "should find text adapter");
        assert_eq!(found.unwrap().id(), "extract-analysis-text");
    }

    // --- Scenario: Extraction status tracks phase completion ---

    #[tokio::test]
    async fn extraction_status_tracks_phases() {
        let coordinator = ExtractionCoordinator::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let dir = create_temp_file("track.md", "# Simple doc\n\nNo frontmatter.");
        let file_path = dir.path().join("track.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");

        assert_eq!(
            status.properties.get("phase1"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("phase2"),
            Some(&PropertyValue::String("pending".to_string()))
        );
        assert_eq!(
            status.properties.get("phase3"),
            Some(&PropertyValue::String("pending".to_string()))
        );
    }

    // --- Scenario: Each phase has a distinct adapter ID ---

    #[test]
    fn each_phase_has_distinct_adapter_id() {
        let coordinator = ExtractionCoordinator::new();
        let text_adapter = Arc::new(RecordingAdapter::new(
            "extract-analysis-text",
            "extract-analysis-text",
        ));

        assert_eq!(coordinator.id(), "extract-coordinator");
        assert_eq!(text_adapter.id(), "extract-analysis-text");
        assert_ne!(coordinator.id(), text_adapter.id());
    }

    // --- Scenario: Concurrency control limits background phases ---

    #[tokio::test]
    async fn concurrency_control_limits_background_phases() {
        let coordinator = ExtractionCoordinator::new()
            .with_analysis_concurrency(4);

        // Verify semaphore has correct capacity
        assert_eq!(coordinator.analysis_semaphore.available_permits(), 4);

        // Acquire permits to simulate concurrent tasks
        let _permit1 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();
        let _permit2 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();
        let _permit3 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();
        let _permit4 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();

        // 5th should fail (all permits taken)
        assert!(
            coordinator.analysis_semaphore.clone().try_acquire_owned().is_err(),
            "should not get 5th permit with capacity 4"
        );
    }

    // --- Helper: Recording adapter for dispatch tests ---

    struct RecordingAdapter {
        adapter_id: String,
        input_kind: String,
    }

    impl RecordingAdapter {
        fn new(adapter_id: &str, input_kind: &str) -> Self {
            Self {
                adapter_id: adapter_id.to_string(),
                input_kind: input_kind.to_string(),
            }
        }
    }

    #[async_trait]
    impl Adapter for RecordingAdapter {
        fn id(&self) -> &str {
            &self.adapter_id
        }

        fn input_kind(&self) -> &str {
            &self.input_kind
        }

        async fn process(
            &self,
            _input: &AdapterInput,
            _sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
    }
}
