//! Content analysis pipeline for Plexus knowledge graph
//!
//! This module provides the framework for analyzing content and populating
//! the multi-dimensional knowledge graph. It implements the architecture
//! from the Compositional Intelligence Spec.
//!
//! # Architecture
//!
//! The analysis pipeline consists of:
//!
//! - **ContentAnalyzer trait**: Interface for analyzers that extract nodes/edges
//! - **AnalysisOrchestrator**: Coordinates multiple analyzers, handles parallelism
//! - **ResultMerger**: Combines outputs, deduplicates, creates cross-dimensional edges
//!
//! # Analyzer Types
//!
//! - **Programmatic analyzers**: Fast, deterministic (e.g., markdown parsing)
//! - **LLM analyzers**: AI-powered semantic analysis (rate-limited, async)
//!
//! # Built-in Analyzers
//!
//! - **MarkdownStructureAnalyzer**: Extracts headings, code blocks, lists (structure dimension)
//! - **LinkAnalyzer**: Extracts links and creates relational edges (relational dimension)
//! - **FrontmatterAnalyzer**: Parses YAML frontmatter metadata (structure dimension)
//!
//! # Example
//!
//! ```ignore
//! use plexus::analysis::{AnalysisOrchestrator, AnalysisScope, ContentItem};
//! use plexus::analysis::analyzers::MarkdownStructureAnalyzer;
//! use plexus::graph::{ContentType, ContextId};
//!
//! // Create orchestrator and register analyzers
//! let mut orchestrator = AnalysisOrchestrator::new();
//! orchestrator.register(MarkdownStructureAnalyzer::new());
//!
//! // Prepare content for analysis
//! let items = vec![
//!     ContentItem::from_file(path, ContentType::Document, content),
//! ];
//!
//! let scope = AnalysisScope::new(ContextId::from_string("my-project"), items);
//!
//! // Run analysis
//! let mutation = orchestrator.analyze(&scope).await?;
//!
//! // Apply mutation to graph
//! engine.apply_mutation(context_id, mutation)?;
//! ```

pub mod analyzers;
mod merger;
mod orchestrator;
mod traits;
mod types;

pub use merger::{ConflictStrategy, ResultMerger};
pub use orchestrator::AnalysisOrchestrator;
pub use traits::{AnalyzerRegistry, ContentAnalyzer};
pub use types::{
    AnalysisCapability, AnalysisConfig, AnalysisError, AnalysisResult, AnalysisScope, ContentId,
    ContentItem, GraphMutation, SubGraph,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, ContextId};
    use async_trait::async_trait;

    /// A simple test analyzer that counts words
    struct WordCountAnalyzer;

    #[async_trait]
    impl ContentAnalyzer for WordCountAnalyzer {
        fn id(&self) -> &str {
            "word-counter"
        }

        fn name(&self) -> &str {
            "Word Count Analyzer"
        }

        fn dimensions(&self) -> Vec<&str> {
            vec!["structure"]
        }

        fn capabilities(&self) -> Vec<AnalysisCapability> {
            vec![AnalysisCapability::Structure]
        }

        fn handles(&self) -> Vec<ContentType> {
            vec![ContentType::Document]
        }

        async fn analyze(&self, scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
            let mut result = AnalysisResult::new();

            for item in scope.items_to_analyze() {
                let word_count = item.content.split_whitespace().count();

                let mut node = crate::graph::Node::new_in_dimension(
                    "document_stats",
                    ContentType::Document,
                    "structure",
                );
                node.properties.insert(
                    "word_count".into(),
                    crate::graph::PropertyValue::Int(word_count as i64),
                );
                node.properties.insert(
                    "source_file".into(),
                    crate::graph::PropertyValue::String(item.id.as_str().to_string()),
                );

                result.add_node(node, &item.id);
            }

            Ok(result)
        }
    }

    #[tokio::test]
    async fn test_full_pipeline() {
        // Create orchestrator with test analyzer
        let mut orchestrator = AnalysisOrchestrator::new();
        orchestrator.register(WordCountAnalyzer);

        // Create test content
        let items = vec![ContentItem::new(
            "test.md",
            ContentType::Document,
            "Hello world this is a test",
        )];

        let scope = AnalysisScope::new(ContextId::from_string("test-context"), items);

        // Run analysis
        let mutation = orchestrator.analyze(&scope).await.unwrap();

        // Verify results
        assert_eq!(mutation.upsert_nodes.len(), 1);

        let node = &mutation.upsert_nodes[0];
        assert_eq!(node.node_type, "document_stats");
        assert_eq!(node.dimension, "structure");

        let word_count = node.properties.get("word_count").unwrap();
        assert!(matches!(
            word_count,
            crate::graph::PropertyValue::Int(6)
        ));
    }

    #[tokio::test]
    async fn test_incremental_analysis() {
        let mut orchestrator = AnalysisOrchestrator::new();
        orchestrator.register(WordCountAnalyzer);

        // Create multiple content items
        let items = vec![
            ContentItem::new("file1.md", ContentType::Document, "First file content"),
            ContentItem::new("file2.md", ContentType::Document, "Second file"),
        ];

        // Only analyze file1 (incremental)
        let scope = AnalysisScope::new(ContextId::from_string("test"), items)
            .with_changes(vec![ContentId::new("file1.md")]);

        let mutation = orchestrator.analyze(&scope).await.unwrap();

        // Should only have analyzed file1
        assert_eq!(mutation.upsert_nodes.len(), 1);
        let source = mutation.upsert_nodes[0]
            .properties
            .get("source_file")
            .unwrap();
        assert!(matches!(
            source,
            crate::graph::PropertyValue::String(s) if s == "file1.md"
        ));
    }
}
