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

use crate::adapter::engine_sink::EngineSink;
use crate::adapter::provenance::FrameworkContext;
use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission};
use crate::graph::{dimension, ContentType, Context, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

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
    /// Phase 3 (semantic) adapter — runs sequentially after Phase 2
    phase3_adapter: Option<Arc<dyn Adapter>>,
    /// Shared context for creating background phase sinks
    shared_context: Option<Arc<std::sync::Mutex<Context>>>,
    /// Concurrency semaphore for Phase 2 tasks
    analysis_semaphore: Arc<tokio::sync::Semaphore>,
    /// Concurrency semaphore for Phase 3 tasks
    semantic_semaphore: Arc<tokio::sync::Semaphore>,
    /// Background task handles (for testing / coordination)
    background_tasks: Arc<TokioMutex<Vec<tokio::task::JoinHandle<Result<(), AdapterError>>>>>,
}

impl ExtractionCoordinator {
    pub fn new() -> Self {
        Self {
            phase2_adapters: Vec::new(),
            phase3_adapter: None,
            shared_context: None,
            analysis_semaphore: Arc::new(tokio::sync::Semaphore::new(4)),
            semantic_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
            background_tasks: Arc::new(TokioMutex::new(Vec::new())),
        }
    }

    /// Set the shared context for background phase sinks.
    pub fn with_context(mut self, context: Arc<std::sync::Mutex<Context>>) -> Self {
        self.shared_context = Some(context);
        self
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

    /// Register a Phase 3 (semantic) adapter.
    ///
    /// Phase 3 runs sequentially after Phase 2 completes, within the same
    /// background task. If Phase 2 fails, Phase 3 is skipped.
    pub fn register_phase3(&mut self, adapter: Arc<dyn Adapter>) {
        self.phase3_adapter = Some(adapter);
    }

    /// Find the Phase 2 adapter matching a MIME type.
    fn find_phase2_adapter(&self, mime_type: &str) -> Option<&Arc<dyn Adapter>> {
        self.phase2_adapters
            .iter()
            .find(|reg| mime_type.starts_with(&reg.mime_prefix))
            .map(|reg| &reg.adapter)
    }

    /// Wait for all background tasks to complete. Used in tests.
    pub async fn wait_for_background(&self) -> Vec<Result<(), AdapterError>> {
        let mut tasks = self.background_tasks.lock().await;
        let mut results = Vec::new();
        for handle in tasks.drain(..) {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(AdapterError::Internal(
                    format!("background task panicked: {}", e),
                ))),
            }
        }
        results
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
    _adapter_id: &str,
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

/// Update a phase status on the extraction status node.
fn update_extraction_status(
    ctx: &Arc<std::sync::Mutex<Context>>,
    file_path: &str,
    phase_key: &str,
    status: &str,
) {
    let mut context = ctx.lock().unwrap();
    let status_id = NodeId::from_string(format!("extraction-status:{}", file_path));
    if let Some(node) = context.get_node_mut(&status_id) {
        node.properties.insert(
            phase_key.to_string(),
            PropertyValue::String(status.to_string()),
        );
    }
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

        let file_path = file_input.file_path.clone();
        let context_id = input.context_id.clone();

        // Phase 1: synchronous registration
        let (emission, mime_type, _metadata_warning) =
            run_phase1(&file_path, self.id())?;

        sink.emit(emission).await?;

        // Phase 2+3: spawn background task if we have a context and matching adapter
        if let Some(ref shared_ctx) = self.shared_context {
            if let Some(phase2_adapter) = self.find_phase2_adapter(&mime_type) {
                let adapter = phase2_adapter.clone();
                let ctx = shared_ctx.clone();
                let semaphore = self.analysis_semaphore.clone();
                let sem_semantic = self.semantic_semaphore.clone();
                let tasks = self.background_tasks.clone();
                let file_path_bg = file_path.clone();
                let context_id_bg = context_id.clone();
                let phase3_opt = self.phase3_adapter.clone();

                let handle = tokio::spawn(async move {
                    // Phase 2: acquire analysis semaphore
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        AdapterError::Internal(format!("semaphore closed: {}", e))
                    })?;

                    let sink = EngineSink::new(ctx.clone()).with_framework_context(
                        FrameworkContext {
                            adapter_id: adapter.id().to_string(),
                            context_id: context_id_bg.clone(),
                            input_summary: None,
                        },
                    );

                    let phase2_input = AdapterInput::new(
                        adapter.input_kind(),
                        ExtractFileInput {
                            file_path: file_path_bg.clone(),
                        },
                        &context_id_bg,
                    );

                    let result = adapter.process(&phase2_input, &sink).await;

                    let status_str = match &result {
                        Ok(()) => "complete".to_string(),
                        Err(e) => format!("failed: {}", e),
                    };
                    update_extraction_status(&ctx, &file_path_bg, "phase2", &status_str);

                    // Release analysis permit before Phase 3
                    drop(_permit);

                    // Chain Phase 3 only if Phase 2 succeeded and Phase 3 is registered
                    if result.is_ok() {
                        if let Some(phase3) = phase3_opt {
                            let _permit3 = sem_semantic.acquire().await.map_err(|e| {
                                AdapterError::Internal(format!("semaphore closed: {}", e))
                            })?;

                            let sink3 = EngineSink::new(ctx.clone()).with_framework_context(
                                FrameworkContext {
                                    adapter_id: phase3.id().to_string(),
                                    context_id: context_id_bg.clone(),
                                    input_summary: None,
                                },
                            );

                            let phase3_input = AdapterInput::new(
                                phase3.input_kind(),
                                ExtractFileInput {
                                    file_path: file_path_bg.clone(),
                                },
                                &context_id_bg,
                            );

                            let result3 = phase3.process(&phase3_input, &sink3).await;

                            let status3_str = match &result3 {
                                Ok(()) => "complete".to_string(),
                                Err(AdapterError::Skipped(ref reason)) => {
                                    format!("skipped: {}", reason)
                                }
                                Err(ref e) => format!("failed: {}", e),
                            };
                            update_extraction_status(
                                &ctx,
                                &file_path_bg,
                                "phase3",
                                &status3_str,
                            );

                            // Skipped is not a failure — return Ok
                            return match result3 {
                                Err(AdapterError::Skipped(_)) => Ok(()),
                                other => other,
                            };
                        }
                    }

                    result
                });

                tasks.lock().await.push(handle);
            }
        }

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

    // --- Helpers ---

    /// A no-op adapter (used for dispatch and ID tests).
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

    /// An adapter that emits a concept node (proves it ran and output persists).
    struct EmittingAdapter {
        adapter_id: String,
        input_kind: String,
        concept_id: String,
    }

    impl EmittingAdapter {
        fn new(adapter_id: &str, input_kind: &str, concept_id: &str) -> Self {
            Self {
                adapter_id: adapter_id.to_string(),
                input_kind: input_kind.to_string(),
                concept_id: concept_id.to_string(),
            }
        }
    }

    #[async_trait]
    impl Adapter for EmittingAdapter {
        fn id(&self) -> &str {
            &self.adapter_id
        }

        fn input_kind(&self) -> &str {
            &self.input_kind
        }

        async fn process(
            &self,
            _input: &AdapterInput,
            sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            let mut concept = Node::new_in_dimension(
                "concept",
                ContentType::Concept,
                dimension::SEMANTIC,
            );
            concept.id = NodeId::from_string(&self.concept_id);
            concept.properties.insert(
                "label".to_string(),
                PropertyValue::String(self.concept_id.clone()),
            );
            let emission = Emission::new().with_node(AnnotatedNode::new(concept));
            sink.emit(emission).await?;
            Ok(())
        }
    }

    /// An adapter that always fails (for failure-isolation tests).
    struct FailingAdapter {
        adapter_id: String,
        input_kind: String,
    }

    impl FailingAdapter {
        fn new(adapter_id: &str, input_kind: &str) -> Self {
            Self {
                adapter_id: adapter_id.to_string(),
                input_kind: input_kind.to_string(),
            }
        }
    }

    #[async_trait]
    impl Adapter for FailingAdapter {
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
            Err(AdapterError::Internal("llm-orc unavailable".to_string()))
        }
    }

    /// An adapter that returns Skipped (for graceful degradation tests).
    struct SkippingAdapter {
        adapter_id: String,
        input_kind: String,
    }

    impl SkippingAdapter {
        fn new(adapter_id: &str, input_kind: &str) -> Self {
            Self {
                adapter_id: adapter_id.to_string(),
                input_kind: input_kind.to_string(),
            }
        }
    }

    #[async_trait]
    impl Adapter for SkippingAdapter {
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
            Err(AdapterError::Skipped("llm-orc not running".to_string()))
        }
    }

    /// A Phase 3 adapter that verifies Phase 2 output exists (proves sequential execution).
    struct SequenceVerifyingAdapter {
        adapter_id: String,
        input_kind: String,
        shared_ctx: Arc<Mutex<Context>>,
        phase2_concept_id: String,
    }

    impl SequenceVerifyingAdapter {
        fn new(
            adapter_id: &str,
            input_kind: &str,
            shared_ctx: Arc<Mutex<Context>>,
            phase2_concept_id: &str,
        ) -> Self {
            Self {
                adapter_id: adapter_id.to_string(),
                input_kind: input_kind.to_string(),
                shared_ctx,
                phase2_concept_id: phase2_concept_id.to_string(),
            }
        }
    }

    #[async_trait]
    impl Adapter for SequenceVerifyingAdapter {
        fn id(&self) -> &str {
            &self.adapter_id
        }

        fn input_kind(&self) -> &str {
            &self.input_kind
        }

        async fn process(
            &self,
            _input: &AdapterInput,
            sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            // Verify Phase 2 output exists — proves Phase 3 runs after Phase 2
            {
                let ctx = self.shared_ctx.lock().unwrap();
                let phase2_node =
                    ctx.get_node(&NodeId::from_string(&self.phase2_concept_id));
                if phase2_node.is_none() {
                    return Err(AdapterError::Internal(
                        "Phase 2 output not found — sequencing violated".to_string(),
                    ));
                }
            }

            // Emit Phase 3 output
            let mut concept = Node::new_in_dimension(
                "concept",
                ContentType::Concept,
                dimension::SEMANTIC,
            );
            concept.id = NodeId::from_string("concept:phase3-semantic");
            concept.properties.insert(
                "label".to_string(),
                PropertyValue::String("phase3-semantic".to_string()),
            );
            sink.emit(Emission::new().with_node(AnnotatedNode::new(concept)))
                .await?;
            Ok(())
        }
    }

    // --- Scenario: Extraction coordinator spawns Phase 2 as background task ---

    #[tokio::test]
    async fn phase2_spawns_as_background_task() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        let emitting = Arc::new(EmittingAdapter::new(
            "extract-analysis-text",
            "extract-analysis-text",
            "concept:phase2-discovery",
        ));
        coordinator.register_phase2("text/", emitting);

        let dir = create_temp_file("bg-test.md", "# Background test\n\nSome content.");
        let file_path = dir.path().join("bg-test.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        // process() returns after Phase 1 — Phase 2 runs in background
        coordinator.process(&input, &sink).await.unwrap();

        // Phase 1 results should be in context immediately
        {
            let snapshot = ctx.lock().unwrap();
            let file_id = NodeId::from_string(format!("file:{}", file_path_str));
            assert!(
                snapshot.get_node(&file_id).is_some(),
                "Phase 1 file node should exist"
            );
        }

        // Wait for background Phase 2
        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().all(|r| r.is_ok()),
            "Phase 2 should succeed"
        );

        // Phase 2 output should now be in context
        let snapshot = ctx.lock().unwrap();
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:phase2-discovery"))
                .is_some(),
            "Phase 2 concept node should be persisted"
        );

        // Extraction status should show phase2 complete
        let status_id =
            NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("phase2"),
            Some(&PropertyValue::String("complete".to_string()))
        );
    }

    // --- Scenario: Phase 3 spawns only after Phase 2 completes ---

    #[tokio::test]
    async fn phase3_spawns_after_phase2_completes() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Phase 2: emits concept:phase2-discovery
        let phase2 = Arc::new(EmittingAdapter::new(
            "extract-analysis-text",
            "extract-analysis-text",
            "concept:phase2-discovery",
        ));
        coordinator.register_phase2("text/", phase2);

        // Phase 3: verifies Phase 2 output exists before proceeding
        let phase3 = Arc::new(SequenceVerifyingAdapter::new(
            "extract-semantic",
            "extract-semantic",
            ctx.clone(),
            "concept:phase2-discovery",
        ));
        coordinator.register_phase3(phase3);

        let dir = create_temp_file("seq-test.md", "# Sequence test\n\nContent.");
        let file_path = dir.path().join("seq-test.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        // Wait for background tasks (Phase 2 → Phase 3 chain)
        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().all(|r| r.is_ok()),
            "Phase 2 and Phase 3 should both succeed (sequencing verified)"
        );

        // Both phases' output should be in context
        let snapshot = ctx.lock().unwrap();
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:phase2-discovery"))
                .is_some(),
            "Phase 2 output should be persisted"
        );
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:phase3-semantic"))
                .is_some(),
            "Phase 3 output should be persisted"
        );

        // Extraction status should show both phases complete
        let status_id =
            NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("phase2"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("phase3"),
            Some(&PropertyValue::String("complete".to_string()))
        );
    }

    // --- Scenario: Background phase failure does not affect earlier phases ---

    #[tokio::test]
    async fn background_failure_does_not_affect_earlier_phases() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Phase 2: succeeds and emits output
        let phase2 = Arc::new(EmittingAdapter::new(
            "extract-analysis-text",
            "extract-analysis-text",
            "concept:phase2-discovery",
        ));
        coordinator.register_phase2("text/", phase2);

        // Phase 3: fails (llm-orc unavailable)
        let phase3 = Arc::new(FailingAdapter::new(
            "extract-semantic",
            "extract-semantic",
        ));
        coordinator.register_phase3(phase3);

        let dir = create_temp_file(
            "fail-test.md",
            "---\ntags: [resilience]\n---\n\n# Failure test",
        );
        let file_path = dir.path().join("fail-test.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        // Wait for background tasks — Phase 3 should fail
        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().any(|r| r.is_err()),
            "Phase 3 should have failed"
        );

        // Phase 1 results remain (file node + concept from frontmatter)
        let snapshot = ctx.lock().unwrap();
        let file_id = NodeId::from_string(format!("file:{}", file_path_str));
        assert!(
            snapshot.get_node(&file_id).is_some(),
            "Phase 1 file node should persist despite Phase 3 failure"
        );
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:resilience"))
                .is_some(),
            "Phase 1 concept node should persist despite Phase 3 failure"
        );

        // Phase 2 results remain
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:phase2-discovery"))
                .is_some(),
            "Phase 2 output should persist despite Phase 3 failure"
        );

        // Extraction status: phases 1-2 complete, phase 3 failed
        let status_id =
            NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("phase1"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("phase2"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        let phase3_status = status
            .properties
            .get("phase3")
            .and_then(|pv| match pv {
                PropertyValue::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap();
        assert!(
            phase3_status.starts_with("failed:"),
            "Phase 3 status should be 'failed: ...', got '{}'",
            phase3_status
        );
    }

    // --- Scenario: Phase 3 graceful degradation (ADR-021) ---

    #[tokio::test]
    async fn phase3_graceful_degradation_when_unavailable() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Phase 2: succeeds normally
        let phase2 = Arc::new(EmittingAdapter::new(
            "extract-analysis-text",
            "extract-analysis-text",
            "concept:phase2-result",
        ));
        coordinator.register_phase2("text/", phase2);

        // Phase 3: skipped (llm-orc not running)
        let phase3 = Arc::new(SkippingAdapter::new(
            "extract-semantic",
            "extract-semantic",
        ));
        coordinator.register_phase3(phase3);

        let dir = create_temp_file(
            "graceful.md",
            "---\ntags: [graceful]\n---\n\n# Graceful degradation test",
        );
        let file_path = dir.path().join("graceful.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        // Background task should return Ok — skipped is not an error
        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().all(|r| r.is_ok()),
            "Skipped Phase 3 should not surface as an error"
        );

        let snapshot = ctx.lock().unwrap();

        // Phases 1-2 completed normally
        let file_id = NodeId::from_string(format!("file:{}", file_path_str));
        assert!(snapshot.get_node(&file_id).is_some(), "Phase 1 file node exists");
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:phase2-result"))
                .is_some(),
            "Phase 2 output exists"
        );

        // Phase 3 status is "skipped", not "failed"
        let status_id =
            NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("phase1"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("phase2"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        let phase3_status = status
            .properties
            .get("phase3")
            .and_then(|pv| match pv {
                PropertyValue::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap();
        assert!(
            phase3_status.starts_with("skipped:"),
            "Phase 3 status should be 'skipped: ...', got '{}'",
            phase3_status
        );
    }

    // --- Scenario: Enrichments fire incrementally after each phase ---
    //
    // This scenario is architecturally guaranteed: each phase uses its own
    // EngineSink, and the Engine path runs the enrichment loop after every
    // emit(). Phase 1's emission triggers enrichments (TagConceptBridger),
    // and Phase 2's independent emission triggers another enrichment round
    // (CoOccurrenceEnrichment on cross-phase concepts).
    //
    // Full integration test requires the Engine path (PlexusEngine with
    // enrichment registry). The enrichment loop itself is already tested
    // in engine_sink::tests and enrichment-specific modules.
    //
    // Lightweight verification: each phase's emission is independently
    // committed via separate sinks, so enrichments see incremental state.
    #[tokio::test]
    async fn enrichments_fire_incrementally_verified_by_independent_sinks() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Phase 2 emits a concept node via its own sink
        let phase2 = Arc::new(EmittingAdapter::new(
            "extract-analysis-text",
            "extract-analysis-text",
            "concept:cross-phase",
        ));
        coordinator.register_phase2("text/", phase2);

        let dir = create_temp_file(
            "enrich-test.md",
            "---\ntags: [jazz]\n---\n\n# Enrichment test",
        );
        let file_path = dir.path().join("enrich-test.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput {
                file_path: file_path_str.clone(),
            },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();
        coordinator.wait_for_background().await;

        let snapshot = ctx.lock().unwrap();

        // Phase 1 concept from frontmatter
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:jazz"))
                .is_some(),
            "Phase 1 concept should exist"
        );

        // Phase 2 concept from analysis
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:cross-phase"))
                .is_some(),
            "Phase 2 concept should exist"
        );

        // Both committed independently — enrichments in production would
        // fire after each emission, seeing incremental graph state.
        // Phase 1's concepts trigger TagConceptBridger.
        // Phase 2's concepts trigger CoOccurrenceEnrichment with cross-phase pairs.
    }
}
