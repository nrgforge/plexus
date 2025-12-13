//! Graph building utilities for spike tests
//!
//! Builds Plexus graphs from test corpora using the analysis pipeline.

use super::corpus::TestCorpus;
use plexus::{
    AnalysisConfig, AnalysisOrchestrator, AnalysisScope, Context, ContextId, PlexusEngine,
};

/// Configuration for building a test graph
#[derive(Debug, Clone)]
pub struct GraphBuildConfig {
    /// Whether to enable LLM analysis (requires real_llm feature)
    pub enable_llm: bool,
    /// Context name (defaults to corpus name)
    pub context_name: Option<String>,
}

impl Default for GraphBuildConfig {
    fn default() -> Self {
        Self {
            enable_llm: false,
            context_name: None,
        }
    }
}

impl GraphBuildConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_llm(mut self, enable: bool) -> Self {
        self.enable_llm = enable;
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.context_name = Some(name.into());
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

    // Get the updated context
    let context = engine
        .get_context(&context_id)
        .ok_or_else(|| GraphBuildError::Engine("Context not found after mutation".to_string()))?;

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
