//! Core types for the content analysis pipeline
//!
//! Implements types from the Compositional Intelligence Spec section 2.2

use crate::graph::{ContentType, ContextId, Edge, Node, NodeId, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Unique identifier for content items
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentId(String);

impl ContentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn from_path(path: &Path) -> Self {
        Self(path.to_string_lossy().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ContentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ContentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Capabilities that an analyzer can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisCapability {
    /// Extract structural elements (AST, headers, sections)
    Structure,
    /// Extract references (links, imports, citations)
    References,
    /// Extract named entities and concepts
    Entities,
    /// Extract semantic meaning and relationships
    Semantics,
    /// Generate summaries
    Summary,
}

/// A content item to be analyzed
#[derive(Debug, Clone)]
pub struct ContentItem {
    /// Unique identifier for this content
    pub id: ContentId,
    /// File path (if applicable)
    pub path: Option<PathBuf>,
    /// Type of content
    pub content_type: ContentType,
    /// The actual content string
    pub content: String,
    /// SHA-256 hash for change detection
    pub content_hash: String,
    /// Additional metadata
    pub metadata: HashMap<String, PropertyValue>,
}

impl ContentItem {
    /// Create a new content item
    pub fn new(
        id: impl Into<ContentId>,
        content_type: ContentType,
        content: impl Into<String>,
    ) -> Self {
        let content = content.into();
        let content_hash = Self::compute_hash(&content);
        Self {
            id: id.into(),
            path: None,
            content_type,
            content,
            content_hash,
            metadata: HashMap::new(),
        }
    }

    /// Create from a file path
    pub fn from_file(path: PathBuf, content_type: ContentType, content: String) -> Self {
        let content_hash = Self::compute_hash(&content);
        let id = ContentId::from_path(&path);
        Self {
            id,
            path: Some(path),
            content_type,
            content,
            content_hash,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Compute SHA-256 hash of content
    fn compute_hash(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

/// A subgraph snapshot for providing context to analyzers
#[derive(Debug, Clone, Default)]
pub struct SubGraph {
    /// Nodes in the subgraph
    pub nodes: Vec<Node>,
    /// Edges in the subgraph
    pub edges: Vec<Edge>,
}

impl SubGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_nodes(mut self, nodes: Vec<Node>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn with_edges(mut self, edges: Vec<Edge>) -> Self {
        self.edges = edges;
        self
    }
}

/// Configuration for analysis
#[derive(Debug, Clone, Default)]
pub struct AnalysisConfig {
    /// Maximum content size to analyze (in bytes)
    pub max_content_size: Option<usize>,
    /// Whether to run LLM analysis
    pub enable_llm_analysis: bool,
    /// Timeout for LLM analysis in seconds
    pub llm_timeout_seconds: u64,
    /// Dimensions to populate
    pub target_dimensions: Vec<String>,
}

impl AnalysisConfig {
    pub fn new() -> Self {
        Self {
            max_content_size: Some(512 * 1024), // 512KB default
            enable_llm_analysis: true,
            llm_timeout_seconds: 30,
            target_dimensions: vec![
                "structure".to_string(),
                "semantic".to_string(),
                "relational".to_string(),
            ],
        }
    }

    /// Create config for local-only analysis (no LLM)
    pub fn local_only() -> Self {
        Self {
            enable_llm_analysis: false,
            ..Self::new()
        }
    }
}

/// The scope of analysis - what content to analyze and context
#[derive(Debug, Clone)]
pub struct AnalysisScope {
    /// Context being analyzed
    pub context_id: ContextId,
    /// All content items in scope
    pub items: Vec<ContentItem>,
    /// IDs of changed items (for incremental analysis)
    pub changed_items: Vec<ContentId>,
    /// Existing graph state (for reference)
    pub current_graph: Option<SubGraph>,
    /// Analysis configuration
    pub config: AnalysisConfig,
}

impl AnalysisScope {
    /// Create a new analysis scope
    pub fn new(context_id: ContextId, items: Vec<ContentItem>) -> Self {
        Self {
            context_id,
            items,
            changed_items: Vec::new(),
            current_graph: None,
            config: AnalysisConfig::new(),
        }
    }

    /// Set changed items for incremental analysis
    pub fn with_changes(mut self, changed: Vec<ContentId>) -> Self {
        self.changed_items = changed;
        self
    }

    /// Provide existing graph context
    pub fn with_graph(mut self, graph: SubGraph) -> Self {
        self.current_graph = Some(graph);
        self
    }

    /// Set analysis configuration
    pub fn with_config(mut self, config: AnalysisConfig) -> Self {
        self.config = config;
        self
    }

    /// Check if this is an incremental analysis
    pub fn is_incremental(&self) -> bool {
        !self.changed_items.is_empty()
    }

    /// Get items to analyze (changed items if incremental, all otherwise)
    pub fn items_to_analyze(&self) -> Vec<&ContentItem> {
        if self.is_incremental() {
            self.items
                .iter()
                .filter(|item| self.changed_items.contains(&item.id))
                .collect()
        } else {
            self.items.iter().collect()
        }
    }
}

/// Result of analyzing content
#[derive(Debug, Clone, Default)]
pub struct AnalysisResult {
    /// Nodes extracted by this analyzer
    pub nodes: Vec<Node>,
    /// Edges extracted by this analyzer
    pub edges: Vec<Edge>,
    /// Provenance: which content produced which nodes
    pub provenance: HashMap<NodeId, ContentId>,
    /// Errors encountered (non-fatal)
    pub warnings: Vec<String>,
}

impl AnalysisResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty result
    pub fn empty() -> Self {
        Self::default()
    }

    /// Add a node with provenance tracking
    pub fn add_node(&mut self, node: Node, source: &ContentId) {
        self.provenance.insert(node.id.clone(), source.clone());
        self.nodes.push(node);
    }

    /// Add an edge
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Merge another result into this one
    pub fn merge(&mut self, other: AnalysisResult) {
        self.nodes.extend(other.nodes);
        self.edges.extend(other.edges);
        self.provenance.extend(other.provenance);
        self.warnings.extend(other.warnings);
    }

    /// Check if result is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }
}

/// Mutation to apply to the graph after analysis
#[derive(Debug, Clone, Default)]
pub struct GraphMutation {
    /// Nodes to add/update
    pub upsert_nodes: Vec<Node>,
    /// Edges to add/update
    pub upsert_edges: Vec<Edge>,
    /// Node IDs to remove
    pub remove_nodes: Vec<NodeId>,
    /// Edge IDs to remove (source, target, relationship)
    pub remove_edges: Vec<(NodeId, NodeId, String)>,
}

impl GraphMutation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if mutation is empty
    pub fn is_empty(&self) -> bool {
        self.upsert_nodes.is_empty()
            && self.upsert_edges.is_empty()
            && self.remove_nodes.is_empty()
            && self.remove_edges.is_empty()
    }
}

/// Error types for analysis
#[derive(Debug, Clone, thiserror::Error)]
pub enum AnalysisError {
    #[error("Content too large: {size} bytes (max: {max})")]
    ContentTooLarge { size: usize, max: usize },

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("LLM unavailable: {0}")]
    LlmUnavailable(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_id_from_string() {
        let id = ContentId::new("test-id");
        assert_eq!(id.as_str(), "test-id");
    }

    #[test]
    fn test_content_item_hash() {
        let item1 = ContentItem::new("id1", ContentType::Document, "hello world");
        let item2 = ContentItem::new("id2", ContentType::Document, "hello world");
        let item3 = ContentItem::new("id3", ContentType::Document, "different content");

        // Same content should have same hash
        assert_eq!(item1.content_hash, item2.content_hash);
        // Different content should have different hash
        assert_ne!(item1.content_hash, item3.content_hash);
    }

    #[test]
    fn test_analysis_scope_incremental() {
        let ctx = ContextId::from_string("test");
        let items = vec![
            ContentItem::new("file1", ContentType::Document, "content1"),
            ContentItem::new("file2", ContentType::Document, "content2"),
        ];

        let scope = AnalysisScope::new(ctx.clone(), items)
            .with_changes(vec![ContentId::new("file1")]);

        assert!(scope.is_incremental());
        assert_eq!(scope.items_to_analyze().len(), 1);
        assert_eq!(scope.items_to_analyze()[0].id.as_str(), "file1");
    }

    #[test]
    fn test_analysis_result_merge() {
        let mut result1 = AnalysisResult::new();
        result1.add_warning("warning1");

        let mut result2 = AnalysisResult::new();
        result2.add_warning("warning2");

        result1.merge(result2);
        assert_eq!(result1.warnings.len(), 2);
    }
}
