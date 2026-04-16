//! Extraction coordinator — phased file extraction (ADR-019, ADR-030, ADR-031)
//!
//! Handles `extract-file` input kind. Runs registration synchronously,
//! then spawns structural analysis + semantic extraction as background
//! tokio tasks.
//!
//! Registration (instant, blocking):
//!   File node (MIME type, size, path) + concept nodes from YAML frontmatter
//!
//! Structural analysis (moderate, background):
//!   MIME-dispatched fan-out to registered structural modules (ADR-030).
//!   Coordinator reads file once, dispatches to all matching modules,
//!   merges outputs (Invariant 53), emits module emissions.
//!
//! Semantic extraction (slow, background, LLM):
//!   Abstract concept extraction via llm-orc (ADR-021).
//!   Receives vocabulary + sections from structural analysis (ADR-031).

use crate::adapter::EngineSink;
use crate::adapter::FrameworkContext;
use crate::adapter::semantic::SemanticInput;
use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::structural::{StructuralModule, StructuralOutput};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission, concept_node};
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

impl ExtractFileInput {
    /// Parse from JSON wire format: `{"file_path": "..."}`.
    pub fn from_json(json: &serde_json::Value) -> Result<Self, AdapterError> {
        let file_path = json
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AdapterError::Internal("extract-file input requires 'file_path' field".into())
            })?
            .to_string();
        Ok(Self { file_path })
    }
}

/// Extraction coordinator — orchestrates phased extraction (ADR-019, ADR-030).
pub struct ExtractionCoordinator {
    /// Registered structural modules — dispatched by MIME affinity (ADR-030).
    /// Fan-out: all matching modules run for each file (Invariant 51).
    structural_modules: Vec<Arc<dyn StructuralModule>>,
    /// Semantic extraction adapter — runs after structural analysis
    semantic_adapter: Option<Arc<dyn Adapter>>,
    /// Shared context for creating background phase sinks (test path, no persistence)
    shared_context: Option<Arc<std::sync::Mutex<Context>>>,
    /// Engine for creating background phase sinks (production path, persist-per-emission)
    engine: Option<Arc<crate::graph::PlexusEngine>>,
    /// Context ID for engine-backed background sinks
    engine_context_id: Option<crate::graph::ContextId>,
    /// Concurrency semaphore for structural analysis tasks
    analysis_semaphore: Arc<tokio::sync::Semaphore>,
    /// Concurrency semaphore for semantic extraction tasks
    semantic_semaphore: Arc<tokio::sync::Semaphore>,
    /// Background task handles (for testing / coordination)
    background_tasks: Arc<TokioMutex<Vec<tokio::task::JoinHandle<Result<(), AdapterError>>>>>,
}

impl ExtractionCoordinator {
    pub fn new() -> Self {
        Self {
            structural_modules: Vec::new(),
            semantic_adapter: None,
            shared_context: None,
            engine: None,
            engine_context_id: None,
            analysis_semaphore: Arc::new(tokio::sync::Semaphore::new(4)),
            semantic_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
            background_tasks: Arc::new(TokioMutex::new(Vec::new())),
        }
    }

    /// Set the shared context for background phase sinks (test path, no persistence).
    pub fn with_context(mut self, context: Arc<std::sync::Mutex<Context>>) -> Self {
        self.shared_context = Some(context);
        self
    }

    /// Set the engine for background phase sinks (production path, persist-per-emission).
    ///
    /// Background phases use `EngineSink::for_engine()` which persists each emission
    /// through PlexusEngine (Invariant 30). Preferred over `with_context()`.
    pub fn with_engine(
        mut self,
        engine: Arc<crate::graph::PlexusEngine>,
        context_id: crate::graph::ContextId,
    ) -> Self {
        self.engine = Some(engine);
        self.engine_context_id = Some(context_id);
        self
    }

    /// Set the concurrency limit for structural analysis tasks.
    pub fn with_analysis_concurrency(mut self, limit: usize) -> Self {
        self.analysis_semaphore = Arc::new(tokio::sync::Semaphore::new(limit));
        self
    }

    /// Register a structural module (ADR-030).
    ///
    /// Modules are dispatched by MIME affinity — all modules whose
    /// `mime_affinity()` prefix matches the file's MIME type will run
    /// (fan-out, Invariant 51).
    pub fn register_structural_module(&mut self, module: Arc<dyn StructuralModule>) {
        self.structural_modules.push(module);
    }

    /// Register the semantic extraction adapter.
    ///
    /// Runs sequentially after structural analysis completes, within the same
    /// background task. Receives vocabulary + sections from structural analysis
    /// via `SemanticInput::with_structural_context()` (ADR-031).
    pub fn register_semantic_extraction(&mut self, adapter: Arc<dyn Adapter>) {
        self.semantic_adapter = Some(adapter);
    }

    /// Find all structural modules matching a MIME type (fan-out, Invariant 51).
    ///
    /// Returns all modules whose `mime_affinity()` is a prefix of the file's
    /// MIME type. A module with affinity `text/` matches `text/markdown`,
    /// `text/plain`, etc.
    fn matching_modules(&self, mime_type: &str) -> Vec<Arc<dyn StructuralModule>> {
        self.structural_modules
            .iter()
            .filter(|m| mime_type.starts_with(m.mime_affinity()))
            .cloned()
            .collect()
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

/// Merge two structural outputs (Invariant 53).
///
/// - Vocabulary: unioned case-insensitively
/// - Sections: concatenated, sorted by start_line
/// - Emissions: kept separate (per-module)
fn merge_structural_outputs(mut base: StructuralOutput, other: StructuralOutput) -> StructuralOutput {
    for term in other.vocabulary {
        let lower = term.to_lowercase();
        if !base.vocabulary.iter().any(|v| v.to_lowercase() == lower) {
            base.vocabulary.push(term);
        }
    }
    base.sections.extend(other.sections);
    base.sections.sort_by_key(|s| s.start_line);
    base.emissions.extend(other.emissions);
    base
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

/// Run registration: file registration + metadata extraction.
///
/// Creates:
/// - File node in structure dimension (MIME type, size, path)
/// - Concept nodes from YAML frontmatter tags in semantic dimension
/// - tagged_with edges from file node to concepts
/// - Extraction status node
fn run_registration(
    file_path: &str,
    _adapter_id: &str,
) -> Result<(Emission, String, Option<String>), AdapterError> {
    let path = Path::new(file_path);
    let mime_type = detect_mime_type(path);

    // Read file metadata
    let metadata = std::fs::metadata(path)
        .map_err(|e| AdapterError::Storage(format!("cannot read file metadata: {}", e)))?;
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
                        let (cid, node) = concept_node(tag);

                        emission = emission.with_node(AnnotatedNode::new(node));

                        // tagged_with edge: file → concept
                        let mut edge = Edge::new_cross_dimensional(
                            file_node_id.clone(),
                            dimension::STRUCTURE,
                            cid,
                            dimension::SEMANTIC,
                            "tagged_with",
                        );
                        edge.combined_weight = 1.0;
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
        "registration".to_string(),
        PropertyValue::String("complete".to_string()),
    );
    status_node.properties.insert(
        "structural_analysis".to_string(),
        PropertyValue::String("pending".to_string()),
    );
    status_node.properties.insert(
        "semantic_extraction".to_string(),
        PropertyValue::String("pending".to_string()),
    );
    if let Some(ref warning) = metadata_warning {
        status_node.properties.insert(
            "registration_warning".to_string(),
            PropertyValue::String(warning.clone()),
        );
    }
    emission = emission.with_node(AnnotatedNode::new(status_node));

    Ok((emission, mime_type.to_string(), metadata_warning))
}

/// Update a phase status on the extraction status node (Mutex path).
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

/// Update a phase status on the extraction status node (Engine path).
fn update_extraction_status_via_engine(
    engine: &crate::graph::PlexusEngine,
    context_id: &crate::graph::ContextId,
    file_path: &str,
    phase_key: &str,
    status: &str,
) {
    let _ = engine.with_context_mut(context_id, |ctx| {
        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path));
        if let Some(node) = ctx.get_node_mut(&status_id) {
            node.properties.insert(
                phase_key.to_string(),
                PropertyValue::String(status.to_string()),
            );
        }
    });
}

/// Create an EngineSink from either the engine path or the mutex path.
///
/// Used by structural analysis and semantic extraction to avoid duplicating sink construction.
fn create_sink(
    engine: &Option<Arc<crate::graph::PlexusEngine>>,
    context_id: &Option<crate::graph::ContextId>,
    mutex: &Option<Arc<std::sync::Mutex<Context>>>,
    adapter_id: &str,
    context_id_str: &str,
) -> EngineSink {
    let sink = if let (Some(ref eng), Some(ref ctx_id)) = (engine, context_id) {
        EngineSink::for_engine(eng.clone(), ctx_id.clone())
    } else {
        EngineSink::new(mutex.clone().unwrap())
    };
    sink.with_framework_context(FrameworkContext {
        adapter_id: adapter_id.to_string(),
        context_id: context_id_str.to_string(),
        input_summary: None,
    })
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
        let owned_input: ExtractFileInput;
        let file_input: &ExtractFileInput =
            if let Some(fi) = input.downcast_data::<ExtractFileInput>() {
                fi
            } else if let Some(json) = input.downcast_data::<serde_json::Value>() {
                owned_input = ExtractFileInput::from_json(json)?;
                &owned_input
            } else {
                return Err(AdapterError::InvalidInput);
            };

        let file_path = file_input.file_path.clone();
        let context_id = input.context_id.clone();

        // Registration: synchronous
        let (emission, mime_type, _metadata_warning) =
            run_registration(&file_path, self.id())?;

        sink.emit(emission).await?;

        // Structural analysis + semantic extraction: spawn background task
        // if we have a backend and there's work to do (modules or semantic extraction).
        let matching = self.matching_modules(&mime_type);
        let has_work = !matching.is_empty() || self.semantic_adapter.is_some();

        if has_work {
            let bg_engine = self.engine.clone();
            let bg_context_id = self.engine_context_id.clone();
            let bg_mutex = self.shared_context.clone();

            let has_backend = bg_engine.is_some() || bg_mutex.is_some();
            if !has_backend {
                tracing::warn!(
                    file_path = %file_path,
                    "skipping structural analysis + semantic extraction: no engine backend configured"
                );
            }
            if has_backend {
                let modules = matching;
                let semaphore = self.analysis_semaphore.clone();
                let sem_semantic = self.semantic_semaphore.clone();
                let tasks = self.background_tasks.clone();
                let file_path_bg = file_path.clone();
                let context_id_bg = context_id.clone();
                let semantic_opt = self.semantic_adapter.clone();

                let handle = tokio::spawn(async move {
                    // Structural analysis: acquire analysis semaphore
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        AdapterError::Internal(format!("semaphore closed: {}", e))
                    })?;

                    // Run structural modules: read file once, fan-out to all matching
                    let structural_output = if !modules.is_empty() {
                        let content = std::fs::read_to_string(&file_path_bg)
                            .unwrap_or_else(|e| {
                                tracing::warn!(
                                    file_path = %file_path_bg,
                                    error = %e,
                                    "cannot read file for structural analysis, using empty content"
                                );
                                String::new()
                            });

                        let mut merged = StructuralOutput::default();
                        for module in &modules {
                            let output = module.analyze(&file_path_bg, &content).await;
                            merged = merge_structural_outputs(merged, output);
                        }

                        // Emit module emissions with per-module adapter IDs
                        for module_emission in &merged.emissions {
                            let module_sink = create_sink(
                                &bg_engine, &bg_context_id, &bg_mutex,
                                &module_emission.module_id, &context_id_bg,
                            );
                            let emission = Emission {
                                nodes: module_emission.nodes.clone(),
                                edges: module_emission.edges.clone(),
                                removals: Vec::new(),
                                edge_removals: Vec::new(),
                                property_updates: Vec::new(),
                            };
                            if !emission.is_empty() {
                                module_sink.emit(emission).await?;
                            }
                        }

                        merged
                    } else {
                        // No matching modules — empty passthrough (Invariant 52)
                        StructuralOutput::default()
                    };

                    // Update structural analysis status
                    let analysis_status = "complete".to_string();
                    if let (Some(ref engine), Some(ref ctx_id)) = (&bg_engine, &bg_context_id) {
                        update_extraction_status_via_engine(engine, ctx_id, &file_path_bg, "structural_analysis", &analysis_status);
                    } else if let Some(ref ctx) = bg_mutex {
                        update_extraction_status(ctx, &file_path_bg, "structural_analysis", &analysis_status);
                    }

                    // Release analysis permit before semantic extraction
                    drop(_permit);

                    // Chain semantic extraction with structural context (ADR-031)
                    if let Some(semantic) = semantic_opt {
                        let _semantic_permit = sem_semantic.acquire().await.map_err(|e| {
                            AdapterError::Internal(format!("semaphore closed: {}", e))
                        })?;

                        let semantic_sink = create_sink(
                            &bg_engine, &bg_context_id, &bg_mutex,
                            semantic.id(), &context_id_bg,
                        );

                        // Hand off vocabulary + sections from structural analysis
                        let semantic_input_wrapped = AdapterInput::new(
                            semantic.input_kind(),
                            SemanticInput::with_structural_context(
                                file_path_bg.clone(),
                                structural_output.sections,
                                structural_output.vocabulary,
                            ),
                            &context_id_bg,
                        );

                        let semantic_result = semantic.process(&semantic_input_wrapped, &semantic_sink).await;

                        let semantic_status_str = match &semantic_result {
                            Ok(()) => "complete".to_string(),
                            Err(AdapterError::Skipped(ref reason)) => {
                                format!("skipped: {}", reason)
                            }
                            Err(ref e) => format!("failed: {}", e),
                        };
                        if let (Some(ref engine), Some(ref ctx_id)) = (&bg_engine, &bg_context_id) {
                            update_extraction_status_via_engine(engine, ctx_id, &file_path_bg, "semantic_extraction", &semantic_status_str);
                        } else if let Some(ref ctx) = bg_mutex {
                            update_extraction_status(ctx, &file_path_bg, "semantic_extraction", &semantic_status_str);
                        }

                        // Skipped is not a failure — return Ok
                        return match semantic_result {
                            Err(AdapterError::Skipped(_)) => Ok(()),
                            other => other,
                        };
                    }

                    Ok(())
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
    use crate::adapter::EngineSink;
    use crate::adapter::FrameworkContext;
    use crate::adapter::structural::{StructuralModule, StructuralOutput, ModuleEmission};
    use crate::adapter::structural::SectionBoundary;
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

    // --- Test structural module stubs ---

    /// A structural module that returns empty output (default behavior).
    struct PassthroughModule {
        id: &'static str,
        mime: &'static str,
    }

    #[async_trait]
    impl StructuralModule for PassthroughModule {
        fn id(&self) -> &str { self.id }
        fn mime_affinity(&self) -> &str { self.mime }
        async fn analyze(&self, _file_path: &str, _content: &str) -> StructuralOutput {
            StructuralOutput::default()
        }
    }

    /// A structural module that emits a concept node via ModuleEmission.
    struct EmittingModule {
        id: &'static str,
        mime: &'static str,
        concept_label: &'static str,
    }

    #[async_trait]
    impl StructuralModule for EmittingModule {
        fn id(&self) -> &str { self.id }
        fn mime_affinity(&self) -> &str { self.mime }
        async fn analyze(&self, _file_path: &str, _content: &str) -> StructuralOutput {
            let (_concept_id, node) = concept_node(self.concept_label);
            StructuralOutput {
                vocabulary: vec![self.concept_label.to_string()],
                sections: vec![],
                emissions: vec![ModuleEmission {
                    module_id: self.id.to_string(),
                    nodes: vec![AnnotatedNode::new(node)],
                    edges: vec![],
                }],
            }
        }
    }

    /// A structural module that returns vocabulary and sections.
    struct VocabularyModule {
        id: &'static str,
        mime: &'static str,
        terms: Vec<&'static str>,
        sections: Vec<SectionBoundary>,
    }

    #[async_trait]
    impl StructuralModule for VocabularyModule {
        fn id(&self) -> &str { self.id }
        fn mime_affinity(&self) -> &str { self.mime }
        async fn analyze(&self, _file_path: &str, _content: &str) -> StructuralOutput {
            StructuralOutput {
                vocabulary: self.terms.iter().map(|t| t.to_string()).collect(),
                sections: self.sections.clone(),
                emissions: vec![],
            }
        }
    }

    // --- Scenario: Extraction coordinator runs registration synchronously ---

    #[tokio::test]
    async fn registration_runs_synchronously() {
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
            status.properties.get("registration"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("structural_analysis"),
            Some(&PropertyValue::String("pending".to_string()))
        );
    }

    // --- Scenario: registration metadata failure does not prevent file registration ---

    #[tokio::test]
    async fn registration_metadata_failure_still_registers_file() {
        let coordinator = ExtractionCoordinator::new();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

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

        let file_node_id = NodeId::from_string(format!("file:{}", file_path_str));
        assert!(
            snapshot.get_node(&file_node_id).is_some(),
            "file node should exist despite corrupt frontmatter"
        );

        let concept_count = snapshot
            .nodes()
            .filter(|n| n.node_type == "concept")
            .count();
        assert_eq!(concept_count, 0, "no concepts from corrupt frontmatter");

        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("registration"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert!(
            status.properties.get("registration_warning").is_some(),
            "should have metadata warning"
        );
    }

    // --- Scenario: Structural modules dispatch by MIME affinity (fan-out, Invariant 51) ---

    #[tokio::test]
    async fn structural_modules_dispatch_by_mime_affinity() {
        let mut coordinator = ExtractionCoordinator::new();

        let text_module: Arc<dyn StructuralModule> = Arc::new(PassthroughModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
        });
        let markdown_module: Arc<dyn StructuralModule> = Arc::new(PassthroughModule {
            id: "extract-analysis-text-markdown",
            mime: "text/markdown",
        });
        let audio_module: Arc<dyn StructuralModule> = Arc::new(PassthroughModule {
            id: "extract-analysis-audio",
            mime: "audio/",
        });

        coordinator.register_structural_module(text_module);
        coordinator.register_structural_module(markdown_module);
        coordinator.register_structural_module(audio_module);

        // text/markdown matches both text/ and text/markdown (fan-out)
        let matching = coordinator.matching_modules("text/markdown");
        assert_eq!(matching.len(), 2, "text/markdown should match 2 modules");
        let ids: Vec<&str> = matching.iter().map(|m| m.id()).collect();
        assert!(ids.contains(&"extract-analysis-text-headings"));
        assert!(ids.contains(&"extract-analysis-text-markdown"));

        // audio/mpeg matches only audio/
        let matching = coordinator.matching_modules("audio/mpeg");
        assert_eq!(matching.len(), 1, "audio/mpeg should match 1 module");
        assert_eq!(matching[0].id(), "extract-analysis-audio");

        // application/pdf matches nothing
        let matching = coordinator.matching_modules("application/pdf");
        assert_eq!(matching.len(), 0, "application/pdf matches no modules");
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
            status.properties.get("registration"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        assert_eq!(
            status.properties.get("structural_analysis"),
            Some(&PropertyValue::String("pending".to_string()))
        );
        assert_eq!(
            status.properties.get("semantic_extraction"),
            Some(&PropertyValue::String("pending".to_string()))
        );
    }

    // --- Scenario: Each phase has a distinct adapter ID ---

    #[test]
    fn each_phase_has_distinct_adapter_id() {
        let coordinator = ExtractionCoordinator::new();
        assert_eq!(coordinator.id(), "extract-coordinator");
        // Structural modules have their own IDs distinct from coordinator
        let module = PassthroughModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
        };
        assert_ne!(coordinator.id(), module.id());
    }

    // --- Scenario: Concurrency control limits background phases ---

    #[tokio::test]
    async fn concurrency_control_limits_background_phases() {
        let coordinator = ExtractionCoordinator::new()
            .with_analysis_concurrency(4);

        assert_eq!(coordinator.analysis_semaphore.available_permits(), 4);

        let _permit1 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();
        let _permit2 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();
        let _permit3 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();
        let _permit4 = coordinator.analysis_semaphore.clone().try_acquire_owned().unwrap();

        assert!(
            coordinator.analysis_semaphore.clone().try_acquire_owned().is_err(),
            "should not get 5th permit with capacity 4"
        );
    }

    // --- Scenario: Structural analysis runs in background and emits module output ---

    #[tokio::test]
    async fn structural_analysis_runs_in_background() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        let emitting: Arc<dyn StructuralModule> = Arc::new(EmittingModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
            concept_label: "structural-discovery",
        });
        coordinator.register_structural_module(emitting);

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

        // process() returns after registration — structural analysis runs in background
        coordinator.process(&input, &sink).await.unwrap();

        // registration results should be in context immediately
        {
            let snapshot = ctx.lock().unwrap();
            let file_id = NodeId::from_string(format!("file:{}", file_path_str));
            assert!(
                snapshot.get_node(&file_id).is_some(),
                "registration file node should exist"
            );
        }

        // Wait for background structural analysis
        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().all(|r| r.is_ok()),
            "Structural analysis should succeed"
        );

        // Module emission should now be in context
        let snapshot = ctx.lock().unwrap();
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:structural-discovery"))
                .is_some(),
            "Structural module concept node should be persisted"
        );

        // Extraction status should show structural analysis complete
        let status_id =
            NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("structural_analysis"),
            Some(&PropertyValue::String("complete".to_string()))
        );
    }

    // --- Scenario: Semantic extraction runs after structural analysis with context (ADR-031) ---

    #[tokio::test]
    async fn semantic_extraction_receives_structural_context() {
        use crate::adapter::semantic::SemanticInput;

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Module that produces vocabulary + sections
        let vocab_module: Arc<dyn StructuralModule> = Arc::new(VocabularyModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
            terms: vec!["Plexus", "knowledge graph"],
            sections: vec![SectionBoundary {
                label: "Introduction".to_string(),
                start_line: 1,
                end_line: 10,
            }],
        });
        coordinator.register_structural_module(vocab_module);

        // semantic extraction that verifies it receives vocabulary + sections
        let vocab_received = Arc::new(Mutex::new(Vec::new()));
        let sections_received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let vr = vocab_received.clone();
        let sr = sections_received.clone();

        struct VocabVerifier {
            vocab: Arc<Mutex<Vec<String>>>,
            section_count: Arc<std::sync::atomic::AtomicUsize>,
        }

        #[async_trait]
        impl Adapter for VocabVerifier {
            fn id(&self) -> &str { "extract-semantic" }
            fn input_kind(&self) -> &str { "extract-semantic" }
            async fn process(
                &self,
                input: &AdapterInput,
                _sink: &dyn AdapterSink,
            ) -> Result<(), AdapterError> {
                let semantic = input.downcast_data::<SemanticInput>()
                    .ok_or(AdapterError::InvalidInput)?;
                *self.vocab.lock().unwrap() = semantic.vocabulary.clone();
                self.section_count.store(
                    semantic.sections.len(),
                    std::sync::atomic::Ordering::SeqCst,
                );
                Ok(())
            }
        }

        coordinator.register_semantic_extraction(Arc::new(VocabVerifier {
            vocab: vr,
            section_count: sr,
        }));

        let dir = create_temp_file("vocab-test.md", "# Introduction\n\nPlexus knowledge graph.");
        let file_path = dir.path().join("vocab-test.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();
        let results = coordinator.wait_for_background().await;
        assert!(results.iter().all(|r| r.is_ok()), "Should succeed");

        let vocab = vocab_received.lock().unwrap();
        assert_eq!(vocab.len(), 2);
        assert!(vocab.contains(&"Plexus".to_string()));
        assert!(vocab.contains(&"knowledge graph".to_string()));
        assert_eq!(
            sections_received.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "Should receive 1 section from structural analysis"
        );
    }

    // --- Scenario: Empty module registry passes through (Invariant 52) ---

    #[tokio::test]
    async fn empty_registry_passthrough() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // No structural modules registered, but semantic extraction is registered
        let received = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = received.clone();

        struct SemanticMarker {
            ran: Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait]
        impl Adapter for SemanticMarker {
            fn id(&self) -> &str { "extract-semantic" }
            fn input_kind(&self) -> &str { "extract-semantic" }
            async fn process(
                &self,
                _input: &AdapterInput,
                _sink: &dyn AdapterSink,
            ) -> Result<(), AdapterError> {
                self.ran.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        coordinator.register_semantic_extraction(Arc::new(SemanticMarker { ran: flag }));

        let dir = create_temp_file("passthrough.md", "# Passthrough\n\nContent.");
        let file_path = dir.path().join("passthrough.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();
        coordinator.wait_for_background().await;

        assert!(
            received.load(std::sync::atomic::Ordering::SeqCst),
            "semantic extraction should run even with no structural modules (empty passthrough)"
        );
    }

    // --- Scenario: Semantic extraction failure does not affect earlier phases ---

    #[tokio::test]
    async fn semantic_failure_does_not_affect_earlier_phases() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Structural module that emits a concept
        let emitting: Arc<dyn StructuralModule> = Arc::new(EmittingModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
            concept_label: "structural-discovery",
        });
        coordinator.register_structural_module(emitting);

        // semantic extraction: fails (llm-orc unavailable)
        struct FailingSemantic;
        #[async_trait]
        impl Adapter for FailingSemantic {
            fn id(&self) -> &str { "extract-semantic" }
            fn input_kind(&self) -> &str { "extract-semantic" }
            async fn process(
                &self, _: &AdapterInput, _: &dyn AdapterSink,
            ) -> Result<(), AdapterError> {
                Err(AdapterError::Internal("llm-orc unavailable".to_string()))
            }
        }
        coordinator.register_semantic_extraction(Arc::new(FailingSemantic));

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

        let results = coordinator.wait_for_background().await;
        assert!(
            results.iter().any(|r| r.is_err()),
            "semantic extraction should have failed"
        );

        // registration results remain
        let snapshot = ctx.lock().unwrap();
        let file_id = NodeId::from_string(format!("file:{}", file_path_str));
        assert!(snapshot.get_node(&file_id).is_some(), "registration file node persists");
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:resilience")).is_some(),
            "registration frontmatter concept persists"
        );

        // Structural analysis results remain
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:structural-discovery")).is_some(),
            "Structural module output persists despite semantic extraction failure"
        );

        // semantic extraction status shows failure
        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("structural_analysis"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        let semantic_status = status.properties.get("semantic_extraction")
            .and_then(|pv| match pv { PropertyValue::String(s) => Some(s.as_str()), _ => None })
            .unwrap();
        assert!(semantic_status.starts_with("failed:"), "semantic extraction status: {}", semantic_status);
    }

    // --- Scenario: semantic extraction graceful degradation (ADR-021) ---

    #[tokio::test]
    async fn semantic_extraction_graceful_degradation_when_unavailable() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        let module: Arc<dyn StructuralModule> = Arc::new(EmittingModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
            concept_label: "structural-result",
        });
        coordinator.register_structural_module(module);

        // semantic extraction: skipped (llm-orc not running)
        struct SkippingSemantic;
        #[async_trait]
        impl Adapter for SkippingSemantic {
            fn id(&self) -> &str { "extract-semantic" }
            fn input_kind(&self) -> &str { "extract-semantic" }
            async fn process(
                &self, _: &AdapterInput, _: &dyn AdapterSink,
            ) -> Result<(), AdapterError> {
                Err(AdapterError::Skipped("llm-orc not running".to_string()))
            }
        }
        coordinator.register_semantic_extraction(Arc::new(SkippingSemantic));

        let dir = create_temp_file(
            "graceful.md",
            "---\ntags: [graceful]\n---\n\n# Graceful degradation test",
        );
        let file_path = dir.path().join("graceful.md");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path_str.clone() },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();

        let results = coordinator.wait_for_background().await;
        assert!(results.iter().all(|r| r.is_ok()), "Skipped is not an error");

        let snapshot = ctx.lock().unwrap();
        assert!(snapshot.get_node(&NodeId::from_string("concept:structural-result")).is_some());

        let status_id = NodeId::from_string(format!("extraction-status:{}", file_path_str));
        let status = snapshot.get_node(&status_id).expect("status should exist");
        assert_eq!(
            status.properties.get("structural_analysis"),
            Some(&PropertyValue::String("complete".to_string()))
        );
        let semantic_status = status.properties.get("semantic_extraction")
            .and_then(|pv| match pv { PropertyValue::String(s) => Some(s.as_str()), _ => None })
            .unwrap();
        assert!(semantic_status.starts_with("skipped:"), "semantic extraction: {}", semantic_status);
    }

    // --- Scenario: Multiple modules fan-out and merge (Invariant 53) ---

    #[tokio::test]
    async fn multiple_modules_fan_out_and_merge() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Two modules matching text/markdown — both should run
        let module_a: Arc<dyn StructuralModule> = Arc::new(EmittingModule {
            id: "module-a",
            mime: "text/",
            concept_label: "concept-from-a",
        });
        let module_b: Arc<dyn StructuralModule> = Arc::new(EmittingModule {
            id: "module-b",
            mime: "text/markdown",
            concept_label: "concept-from-b",
        });
        coordinator.register_structural_module(module_a);
        coordinator.register_structural_module(module_b);

        let dir = create_temp_file("fanout.md", "# Fan-out test\n\nContent.");
        let file_path = dir.path().join("fanout.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();
        coordinator.wait_for_background().await;

        let snapshot = ctx.lock().unwrap();
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:concept-from-a")).is_some(),
            "Module A's concept should be emitted"
        );
        assert!(
            snapshot.get_node(&NodeId::from_string("concept:concept-from-b")).is_some(),
            "Module B's concept should be emitted"
        );
    }

    // --- Scenario: Merge deduplicates vocabulary case-insensitively ---

    #[test]
    fn merge_deduplicates_vocabulary() {
        let a = StructuralOutput {
            vocabulary: vec!["Plexus".to_string(), "Graph".to_string()],
            sections: vec![],
            emissions: vec![],
        };
        let b = StructuralOutput {
            vocabulary: vec!["plexus".to_string(), "Knowledge".to_string()],
            sections: vec![],
            emissions: vec![],
        };

        let merged = merge_structural_outputs(a, b);
        assert_eq!(merged.vocabulary.len(), 3);
        assert!(merged.vocabulary.contains(&"Plexus".to_string()));
        assert!(merged.vocabulary.contains(&"Graph".to_string()));
        assert!(merged.vocabulary.contains(&"Knowledge".to_string()));
    }

    // --- Scenario: Merge sorts sections by start_line ---

    #[test]
    fn merge_sorts_sections_by_start_line() {
        let a = StructuralOutput {
            vocabulary: vec![],
            sections: vec![SectionBoundary { label: "Second".into(), start_line: 50, end_line: 100 }],
            emissions: vec![],
        };
        let b = StructuralOutput {
            vocabulary: vec![],
            sections: vec![SectionBoundary { label: "First".into(), start_line: 1, end_line: 49 }],
            emissions: vec![],
        };

        let merged = merge_structural_outputs(a, b);
        assert_eq!(merged.sections.len(), 2);
        assert_eq!(merged.sections[0].label, "First");
        assert_eq!(merged.sections[1].label, "Second");
    }

    // ================================================================
    // Engine persistence scenarios
    // ================================================================

    #[tokio::test]
    async fn structural_emissions_persist_through_engine() {
        use crate::graph::{ContextId, PlexusEngine};
        use crate::storage::{SqliteStore, OpenStore};

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));
        let context_id = ContextId::from_string("test");
        let mut ctx = Context::new("test");
        ctx.id = context_id.clone();
        engine.upsert_context(ctx).unwrap();

        let mut coordinator = ExtractionCoordinator::new()
            .with_engine(engine.clone(), context_id.clone());

        let module: Arc<dyn StructuralModule> = Arc::new(EmittingModule {
            id: "extract-analysis-text-headings",
            mime: "text/",
            concept_label: "engine-structural",
        });
        coordinator.register_structural_module(module);

        let primary_sink = EngineSink::for_engine(engine.clone(), context_id.clone())
            .with_framework_context(FrameworkContext {
                adapter_id: "extract-coordinator".to_string(),
                context_id: "test".to_string(),
                input_summary: None,
            });

        let dir = create_temp_file("engine-test.md", "# Engine persistence test\n\nContent.");
        let file_path = dir.path().join("engine-test.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &primary_sink).await.unwrap();
        let results = coordinator.wait_for_background().await;
        assert!(results.iter().all(|r| r.is_ok()));

        let ctx = engine.get_context(&context_id).expect("context should exist");
        assert!(ctx.get_node(&NodeId::from_string("concept:engine-structural")).is_some());

        // Verify persistence across reload
        let engine2 = Arc::new(PlexusEngine::with_store(store.clone()));
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&context_id).expect("should survive reload");
        assert!(ctx2.get_node(&NodeId::from_string("concept:engine-structural")).is_some());
    }

    #[tokio::test]
    async fn semantic_emissions_persist_through_engine() {
        use crate::graph::{ContextId, PlexusEngine};
        use crate::storage::{SqliteStore, OpenStore};

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));
        let context_id = ContextId::from_string("test");
        let mut ctx = Context::new("test");
        ctx.id = context_id.clone();
        engine.upsert_context(ctx).unwrap();

        let mut coordinator = ExtractionCoordinator::new()
            .with_engine(engine.clone(), context_id.clone());

        // Structural module (needed to trigger background task)
        let module: Arc<dyn StructuralModule> = Arc::new(PassthroughModule {
            id: "passthrough", mime: "text/",
        });
        coordinator.register_structural_module(module);

        // semantic extraction emits a concept
        struct EmittingSemantic;
        #[async_trait]
        impl Adapter for EmittingSemantic {
            fn id(&self) -> &str { "extract-semantic" }
            fn input_kind(&self) -> &str { "extract-semantic" }
            async fn process(&self, _: &AdapterInput, sink: &dyn AdapterSink) -> Result<(), AdapterError> {
                let mut concept = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
                concept.id = NodeId::from_string("concept:engine-p3");
                concept.properties.insert("label".into(), PropertyValue::String("engine-p3".into()));
                sink.emit(Emission::new().with_node(AnnotatedNode::new(concept))).await?;
                Ok(())
            }
        }
        coordinator.register_semantic_extraction(Arc::new(EmittingSemantic));

        let primary_sink = EngineSink::for_engine(engine.clone(), context_id.clone())
            .with_framework_context(FrameworkContext {
                adapter_id: "extract-coordinator".to_string(),
                context_id: "test".to_string(),
                input_summary: None,
            });

        let dir = create_temp_file("engine-p3.md", "# semantic extraction engine test\n\nContent.");
        let file_path = dir.path().join("engine-p3.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &primary_sink).await.unwrap();
        let results = coordinator.wait_for_background().await;
        assert!(results.iter().all(|r| r.is_ok()));

        let engine2 = Arc::new(PlexusEngine::with_store(store.clone()));
        engine2.load_all().unwrap();
        let ctx = engine2.get_context(&context_id).expect("should survive reload");
        assert!(ctx.get_node(&NodeId::from_string("concept:engine-p3")).is_some());
    }

    // ================================================================
    // Coordinator-to-semantic extraction Type Alignment
    // ================================================================

    #[tokio::test]
    async fn coordinator_sends_semantic_input_to_semantic_extraction() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone(), "extract-coordinator");

        let mut coordinator = ExtractionCoordinator::new().with_context(ctx.clone());

        // Structural module so background task runs
        let module: Arc<dyn StructuralModule> = Arc::new(PassthroughModule {
            id: "passthrough", mime: "text/",
        });
        coordinator.register_structural_module(module);

        let received = Arc::new(std::sync::atomic::AtomicBool::new(false));

        struct SemanticInputVerifier {
            received: Arc<std::sync::atomic::AtomicBool>,
        }
        #[async_trait]
        impl Adapter for SemanticInputVerifier {
            fn id(&self) -> &str { "extract-semantic" }
            fn input_kind(&self) -> &str { "extract-semantic" }
            async fn process(&self, input: &AdapterInput, sink: &dyn AdapterSink) -> Result<(), AdapterError> {
                use crate::adapter::semantic::SemanticInput;
                let _semantic = input.downcast_data::<SemanticInput>()
                    .ok_or(AdapterError::InvalidInput)?;
                self.received.store(true, std::sync::atomic::Ordering::SeqCst);
                let mut concept = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
                concept.id = NodeId::from_string("concept:verified");
                sink.emit(Emission::new().with_node(AnnotatedNode::new(concept))).await?;
                Ok(())
            }
        }

        coordinator.register_semantic_extraction(Arc::new(SemanticInputVerifier { received: received.clone() }));

        let dir = create_temp_file("type-align.md", "# Type alignment test\n\nContent.");
        let file_path = dir.path().join("type-align.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &sink).await.unwrap();
        let results = coordinator.wait_for_background().await;
        assert!(results.iter().all(|r| r.is_ok()), "semantic extraction should succeed — got: {:?}", results);

        assert!(
            received.load(std::sync::atomic::Ordering::SeqCst),
            "semantic extraction should receive SemanticInput"
        );
    }

    // --- Scenario: Coordinator dispatches to real SemanticAdapter (integration) ---

    #[tokio::test]
    async fn coordinator_dispatches_to_real_semantic_adapter() {
        use crate::adapter::semantic::SemanticAdapter;
        use crate::graph::{ContextId, PlexusEngine};
        use crate::llm_orc::{AgentResult, InvokeResponse, MockClient};
        use crate::storage::{SqliteStore, OpenStore};
        use std::collections::HashMap;

        let mut results = HashMap::new();
        results.insert(
            "synthesizer".to_string(),
            AgentResult {
                response: Some(r#"{"concepts": [{"label": "Integration", "confidence": 0.95}]}"#.to_string()),
                status: Some("success".to_string()),
                error: None,
            },
        );
        let response = InvokeResponse {
            results,
            status: "completed".to_string(),
            metadata: serde_json::Value::Null,
        };
        let mock_client = Arc::new(
            MockClient::available().with_response("extract-semantic", response),
        );

        let semantic = Arc::new(SemanticAdapter::new(mock_client, "extract-semantic"));

        let store = Arc::new(SqliteStore::open_in_memory().unwrap());
        let engine = Arc::new(PlexusEngine::with_store(store.clone()));
        let context_id = ContextId::from_string("test");
        let mut ctx = Context::new("test");
        ctx.id = context_id.clone();
        engine.upsert_context(ctx).unwrap();

        let mut coordinator = ExtractionCoordinator::new()
            .with_engine(engine.clone(), context_id.clone());

        // Passthrough structural module so semantic extraction chains
        let module: Arc<dyn StructuralModule> = Arc::new(PassthroughModule {
            id: "passthrough", mime: "text/",
        });
        coordinator.register_structural_module(module);
        coordinator.register_semantic_extraction(semantic);

        let primary_sink = EngineSink::for_engine(engine.clone(), context_id.clone())
            .with_framework_context(FrameworkContext {
                adapter_id: "extract-coordinator".to_string(),
                context_id: "test".to_string(),
                input_summary: None,
            });

        let dir = create_temp_file("integration.md", "# Integration test\n\nContent about integration.");
        let file_path = dir.path().join("integration.md");

        let input = AdapterInput::new(
            "extract-file",
            ExtractFileInput { file_path: file_path.to_str().unwrap().to_string() },
            "test",
        );

        coordinator.process(&input, &primary_sink).await.unwrap();
        let results = coordinator.wait_for_background().await;
        assert!(results.iter().all(|r| r.is_ok()), "Real SemanticAdapter should succeed — got: {:?}", results);

        let ctx = engine.get_context(&context_id).expect("context should exist");
        assert!(
            ctx.get_node(&NodeId::from_string("concept:integration")).is_some(),
            "Concept from real SemanticAdapter should be persisted"
        );

        let engine2 = Arc::new(PlexusEngine::with_store(store.clone()));
        engine2.load_all().unwrap();
        let ctx2 = engine2.get_context(&context_id).expect("should survive reload");
        assert!(ctx2.get_node(&NodeId::from_string("concept:integration")).is_some());
    }
}
