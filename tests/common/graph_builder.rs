//! Graph building utilities for spike tests
//!
//! Builds Plexus graphs from test corpora using the analysis pipeline.

use super::corpus::TestCorpus;
use plexus::{
    AnalysisConfig, AnalysisOrchestrator, AnalysisScope, Context, ContextId, Edge, NodeId,
    PlexusEngine,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Configuration for building a test graph
#[derive(Debug, Clone)]
pub struct GraphBuildConfig {
    /// Whether to enable LLM analysis (requires real_llm feature)
    pub enable_llm: bool,
    /// Context name (defaults to corpus name)
    pub context_name: Option<String>,
    /// Add reverse edges for links (linked_from)
    pub add_reverse_edges: bool,
    /// Add sibling edges between documents in same directory
    pub add_sibling_edges: bool,
    /// Add directory hierarchy nodes and edges
    pub add_directory_hierarchy: bool,
    /// Weight for reverse edges (0.0-1.0)
    pub reverse_edge_weight: f32,
    /// Weight for sibling edges (0.0-1.0)
    pub sibling_edge_weight: f32,
    /// Weight for directory hierarchy edges (0.0-1.0)
    pub directory_edge_weight: f32,
}

impl Default for GraphBuildConfig {
    fn default() -> Self {
        Self {
            enable_llm: false,
            context_name: None,
            add_reverse_edges: true,       // Enable by default for better connectivity
            add_sibling_edges: true,       // Enable by default for better connectivity
            add_directory_hierarchy: true, // Enable by default for tree connectivity
            reverse_edge_weight: 0.5,      // Lower weight than forward edges
            sibling_edge_weight: 0.3,      // Lower weight than direct links
            directory_edge_weight: 0.6,    // Moderate weight for hierarchy
        }
    }
}

impl GraphBuildConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create config with no connectivity enhancements (for comparison)
    pub fn minimal() -> Self {
        Self {
            add_reverse_edges: false,
            add_sibling_edges: false,
            add_directory_hierarchy: false,
            ..Self::default()
        }
    }

    pub fn with_directory_hierarchy(mut self, enable: bool) -> Self {
        self.add_directory_hierarchy = enable;
        self
    }

    pub fn with_llm(mut self, enable: bool) -> Self {
        self.enable_llm = enable;
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.context_name = Some(name.into());
        self
    }

    pub fn with_reverse_edges(mut self, enable: bool) -> Self {
        self.add_reverse_edges = enable;
        self
    }

    pub fn with_sibling_edges(mut self, enable: bool) -> Self {
        self.add_sibling_edges = enable;
        self
    }
}

/// Build a structure-only graph from a corpus (no LLM analysis)
///
/// This is the fast path for spike tests that don't need semantic analysis.
/// Uses only programmatic analyzers (markdown structure, links, frontmatter).
pub async fn build_structure_graph(corpus: &TestCorpus) -> Result<BuiltGraph, GraphBuildError> {
    build_graph_from_corpus(corpus, GraphBuildConfig::default()).await
}

/// Build a graph from a corpus with full configuration
pub async fn build_graph_from_corpus(
    corpus: &TestCorpus,
    config: GraphBuildConfig,
) -> Result<BuiltGraph, GraphBuildError> {
    let engine = PlexusEngine::new();
    let context_name = config.context_name.as_deref().unwrap_or(&corpus.name);
    let context = Context::new(context_name);
    let context_id = engine
        .upsert_context(context)
        .map_err(|e| GraphBuildError::Engine(e.to_string()))?;

    // Set up orchestrator with built-in analyzers
    let mut orchestrator = AnalysisOrchestrator::new();

    // Register programmatic analyzers
    use plexus::analysis::analyzers::{FrontmatterAnalyzer, LinkAnalyzer, MarkdownStructureAnalyzer};
    orchestrator.register(MarkdownStructureAnalyzer::new());
    orchestrator.register(LinkAnalyzer::new());
    orchestrator.register(FrontmatterAnalyzer::new());

    // Configure analysis
    let analysis_config = if config.enable_llm {
        AnalysisConfig::new()
    } else {
        AnalysisConfig::local_only()
    };

    // Create analysis scope
    let scope = AnalysisScope::new(context_id.clone(), corpus.items.clone())
        .with_config(analysis_config);

    // Run analysis
    let mutation = if config.enable_llm {
        orchestrator
            .analyze(&scope)
            .await
            .map_err(|e| GraphBuildError::Analysis(e.to_string()))?
    } else {
        orchestrator
            .analyze_programmatic(&scope)
            .await
            .map_err(|e| GraphBuildError::Analysis(e.to_string()))?
    };

    // Apply mutation to graph
    engine
        .apply_mutation(&context_id, mutation.upsert_nodes, mutation.upsert_edges)
        .map_err(|e| GraphBuildError::Engine(e.to_string()))?;

    // Get the context after initial mutation
    let mut context = engine
        .get_context(&context_id)
        .ok_or_else(|| GraphBuildError::Engine("Context not found after mutation".to_string()))?;

    // Collect additional nodes and edges for connectivity enhancement
    let mut additional_nodes: Vec<plexus::Node> = Vec::new();
    let mut additional_edges: Vec<Edge> = Vec::new();

    // Add directory hierarchy nodes and edges
    if config.add_directory_hierarchy {
        let (dir_nodes, dir_edges) =
            create_directory_hierarchy(&context, config.directory_edge_weight);
        additional_nodes.extend(dir_nodes);
        additional_edges.extend(dir_edges);
    }

    // Add reverse edges for better connectivity
    if config.add_reverse_edges {
        let reverse_edges = create_reverse_edges(&context, config.reverse_edge_weight);
        additional_edges.extend(reverse_edges);
    }

    // Add sibling edges for documents in the same directory
    if config.add_sibling_edges {
        let sibling_edges = create_sibling_edges(&context, config.sibling_edge_weight);
        additional_edges.extend(sibling_edges);
    }

    // Apply additional nodes and edges if any
    if !additional_nodes.is_empty() || !additional_edges.is_empty() {
        engine
            .apply_mutation(&context_id, additional_nodes, additional_edges)
            .map_err(|e| GraphBuildError::Engine(e.to_string()))?;

        // Refresh context with new nodes/edges
        context = engine
            .get_context(&context_id)
            .ok_or_else(|| GraphBuildError::Engine("Context not found after enhancement".to_string()))?;
    }

    Ok(BuiltGraph {
        engine,
        context_id,
        context,
        corpus_name: corpus.name.clone(),
        file_count: corpus.file_count,
    })
}

/// A built graph ready for spike testing
#[derive(Debug)]
pub struct BuiltGraph {
    /// The Plexus engine (owns the graph)
    pub engine: PlexusEngine,
    /// The context ID
    pub context_id: ContextId,
    /// Snapshot of the context (may be stale if engine is modified)
    pub context: Context,
    /// Original corpus name
    pub corpus_name: String,
    /// Number of source files
    pub file_count: usize,
}

impl BuiltGraph {
    /// Get total node count
    pub fn node_count(&self) -> usize {
        self.context.nodes.len()
    }

    /// Get total edge count
    pub fn edge_count(&self) -> usize {
        self.context.edges.len()
    }

    /// Get nodes by type
    pub fn nodes_by_type(&self, node_type: &str) -> Vec<&plexus::Node> {
        self.context
            .nodes
            .values()
            .filter(|n| n.node_type == node_type)
            .collect()
    }

    /// Get edges by relationship type
    pub fn edges_by_relationship(&self, relationship: &str) -> Vec<&plexus::Edge> {
        self.context
            .edges
            .iter()
            .filter(|e| e.relationship == relationship)
            .collect()
    }

    /// Get edges in a specific dimension (source or target)
    pub fn edges_in_dimension(&self, dimension: &str) -> Vec<&plexus::Edge> {
        self.context
            .edges
            .iter()
            .filter(|e| e.source_dimension == dimension || e.target_dimension == dimension)
            .collect()
    }

    /// Get nodes in a specific dimension
    pub fn nodes_in_dimension(&self, dimension: &str) -> Vec<&plexus::Node> {
        self.context
            .nodes
            .values()
            .filter(|n| n.dimension == dimension)
            .collect()
    }

    /// Refresh context snapshot from engine
    pub fn refresh(&mut self) {
        if let Some(ctx) = self.engine.get_context(&self.context_id) {
            self.context = ctx;
        }
    }
}

/// Create directory hierarchy nodes and edges
///
/// Creates a directory node for each unique directory containing documents,
/// then creates:
/// - dir → doc contains edges
/// - doc → dir contained_by edges
/// - parent_dir → child_dir contains edges (hierarchical)
/// - child_dir → parent_dir contained_by edges
fn create_directory_hierarchy(
    context: &Context,
    weight: f32,
) -> (Vec<plexus::Node>, Vec<Edge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Collect all document paths and their directories
    let mut doc_dirs: HashMap<String, Vec<NodeId>> = HashMap::new(); // dir -> doc node ids
    let mut all_dirs: HashSet<String> = HashSet::new();

    for (node_id, node) in context.nodes.iter() {
        if node.node_type == "document" {
            if let Some(plexus::PropertyValue::String(source_str)) = node.properties.get("source") {
                let path = Path::new(source_str);
                if let Some(parent) = path.parent() {
                    let dir = parent.to_string_lossy().to_string();

                    // Track this doc's directory
                    doc_dirs.entry(dir.clone()).or_default().push(node_id.clone());

                    // Collect all ancestor directories
                    let mut current = parent;
                    while !current.as_os_str().is_empty() {
                        all_dirs.insert(current.to_string_lossy().to_string());
                        current = match current.parent() {
                            Some(p) => p,
                            None => break,
                        };
                    }
                }
            }
        }
    }

    // Create directory nodes
    let mut dir_node_ids: HashMap<String, NodeId> = HashMap::new();
    for dir in &all_dirs {
        let dir_node_id = NodeId::from_string(format!("dir:{}", dir));
        let mut dir_node =
            plexus::Node::new_in_dimension("directory", plexus::ContentType::Document, "structure");
        dir_node.id = dir_node_id.clone();
        dir_node.properties.insert(
            "path".into(),
            plexus::PropertyValue::String(dir.clone()),
        );

        // Extract directory name
        let name = Path::new(dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "root".to_string());
        dir_node
            .properties
            .insert("name".into(), plexus::PropertyValue::String(name));

        nodes.push(dir_node);
        dir_node_ids.insert(dir.clone(), dir_node_id);
    }

    // Create dir → doc edges (and reverse)
    for (dir, doc_ids) in &doc_dirs {
        if let Some(dir_node_id) = dir_node_ids.get(dir) {
            for doc_id in doc_ids {
                // dir contains doc
                let mut edge = Edge::new(dir_node_id.clone(), doc_id.clone(), "contains");
                edge.weight = weight;
                edge.source_dimension = "structure".to_string();
                edge.target_dimension = "structure".to_string();
                edges.push(edge);

                // doc contained_by dir
                let mut reverse = Edge::new(doc_id.clone(), dir_node_id.clone(), "contained_by");
                reverse.weight = weight * 0.8;
                reverse.source_dimension = "structure".to_string();
                reverse.target_dimension = "structure".to_string();
                edges.push(reverse);
            }
        }
    }

    // Create hierarchical dir → child_dir edges
    for dir in &all_dirs {
        let path = Path::new(dir);
        if let Some(parent) = path.parent() {
            let parent_str = parent.to_string_lossy().to_string();
            if let (Some(parent_id), Some(child_id)) =
                (dir_node_ids.get(&parent_str), dir_node_ids.get(dir))
            {
                // parent contains child
                let mut edge = Edge::new(parent_id.clone(), child_id.clone(), "contains");
                edge.weight = weight;
                edge.source_dimension = "structure".to_string();
                edge.target_dimension = "structure".to_string();
                edges.push(edge);

                // child contained_by parent
                let mut reverse = Edge::new(child_id.clone(), parent_id.clone(), "contained_by");
                reverse.weight = weight * 0.8;
                reverse.source_dimension = "structure".to_string();
                reverse.target_dimension = "structure".to_string();
                edges.push(reverse);
            }
        }
    }

    (nodes, edges)
}

/// Create reverse edges for directional relationships
///
/// For every A→B links_to edge, creates B→A linked_from edge.
/// For every A→B contains edge, creates B→A contained_by edge.
/// This enables bidirectional traversal for label propagation.
fn create_reverse_edges(context: &Context, weight: f32) -> Vec<Edge> {
    let mut edges = Vec::new();

    for e in &context.edges {
        let (reverse_rel, reverse_weight) = match e.relationship.as_str() {
            "links_to" => ("linked_from", weight),
            "contains" => ("contained_by", weight * 0.8), // Slightly lower weight
            _ => continue,
        };

        let mut reverse = Edge::new(e.target.clone(), e.source.clone(), reverse_rel);
        reverse.weight = reverse_weight;
        reverse.source_dimension = e.target_dimension.clone();
        reverse.target_dimension = e.source_dimension.clone();
        edges.push(reverse);
    }

    edges
}

/// Create sibling edges between documents in the same directory
///
/// Groups document nodes by their parent directory and creates
/// bidirectional sibling edges between them.
fn create_sibling_edges(context: &Context, weight: f32) -> Vec<Edge> {
    // Group document nodes by directory
    let mut by_directory: HashMap<String, Vec<NodeId>> = HashMap::new();

    for (node_id, node) in context.nodes.iter() {
        if node.node_type == "document" {
            // Extract directory from source property (set by markdown analyzer)
            if let Some(plexus::PropertyValue::String(source_str)) = node.properties.get("source") {
                if let Some(parent) = Path::new(source_str).parent() {
                    let dir = parent.to_string_lossy().to_string();
                    by_directory.entry(dir).or_default().push(node_id.clone());
                }
            }
        }
    }

    // Create sibling edges between documents in same directory
    let mut edges = Vec::new();
    for (_dir, doc_ids) in by_directory {
        if doc_ids.len() < 2 {
            continue;
        }

        // Create edges between all pairs (bidirectional)
        for i in 0..doc_ids.len() {
            for j in (i + 1)..doc_ids.len() {
                let mut edge_ab = Edge::new(doc_ids[i].clone(), doc_ids[j].clone(), "sibling");
                edge_ab.weight = weight;
                edge_ab.source_dimension = "structural".to_string();
                edge_ab.target_dimension = "structural".to_string();

                let mut edge_ba = Edge::new(doc_ids[j].clone(), doc_ids[i].clone(), "sibling");
                edge_ba.weight = weight;
                edge_ba.source_dimension = "structural".to_string();
                edge_ba.target_dimension = "structural".to_string();

                edges.push(edge_ab);
                edges.push(edge_ba);
            }
        }
    }

    edges
}

/// Errors during graph building
#[derive(Debug, thiserror::Error)]
pub enum GraphBuildError {
    #[error("Engine error: {0}")]
    Engine(String),
    #[error("Analysis error: {0}")]
    Analysis(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::corpus::TestCorpus;

    #[tokio::test]
    async fn test_build_structure_graph() {
        let corpus = TestCorpus::load("pkm-webdev").expect("Failed to load corpus");
        let graph = build_structure_graph(&corpus).await;

        assert!(graph.is_ok(), "Failed to build graph: {:?}", graph.err());
        let graph = graph.unwrap();

        // Should have nodes from markdown analysis
        assert!(graph.node_count() > 0, "Graph should have nodes");
        // Should have edges from link analysis
        // Note: edge count may be 0 if no internal links exist
    }
}
