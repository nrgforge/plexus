//! Analyzer traits defining the content analysis interface
//!
//! Implements the ContentAnalyzer trait from Compositional Intelligence Spec section 2.2

use super::types::{AnalysisCapability, AnalysisError, AnalysisResult, AnalysisScope};
use crate::graph::ContentType;
use async_trait::async_trait;

/// Trait for content analyzers
///
/// Analyzers extract nodes and edges from content, populating specific
/// dimensions of the knowledge graph.
///
/// # Example
///
/// ```ignore
/// struct MarkdownAnalyzer;
///
/// #[async_trait]
/// impl ContentAnalyzer for MarkdownAnalyzer {
///     fn id(&self) -> &str { "markdown-structure" }
///     fn name(&self) -> &str { "Markdown Structure Analyzer" }
///     fn dimensions(&self) -> Vec<&str> { vec!["structure"] }
///     fn capabilities(&self) -> Vec<AnalysisCapability> {
///         vec![AnalysisCapability::Structure]
///     }
///     fn handles(&self) -> Vec<ContentType> {
///         vec![ContentType::Document]
///     }
///
///     async fn analyze(&self, scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
///         // Parse markdown and extract structure
///         Ok(AnalysisResult::new())
///     }
/// }
/// ```
#[async_trait]
pub trait ContentAnalyzer: Send + Sync {
    /// Unique identifier for this analyzer
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Which dimensions this analyzer populates
    fn dimensions(&self) -> Vec<&str>;

    /// What capabilities this analyzer provides
    fn capabilities(&self) -> Vec<AnalysisCapability>;

    /// Which content types this analyzer can handle
    fn handles(&self) -> Vec<ContentType>;

    /// Whether this analyzer requires LLM access
    ///
    /// Affects scheduling: LLM analyzers run in background with rate limiting
    fn requires_llm(&self) -> bool {
        false
    }

    /// Priority for execution order (lower = earlier)
    ///
    /// Default is 100. Use lower values for analyzers that should run first
    /// (e.g., structure before semantic).
    fn priority(&self) -> u32 {
        100
    }

    /// Analyze the content in scope
    ///
    /// Returns nodes and edges to add to the graph.
    async fn analyze(&self, scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError>;

    /// Check if this analyzer can handle the given content type
    fn can_handle(&self, content_type: ContentType) -> bool {
        self.handles().contains(&content_type)
    }
}

/// Registry of available analyzers
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn ContentAnalyzer>>,
}

impl Default for AnalyzerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    /// Register an analyzer
    pub fn register<A: ContentAnalyzer + 'static>(&mut self, analyzer: A) {
        self.analyzers.push(Box::new(analyzer));
    }

    /// Get all analyzers sorted by priority
    pub fn analyzers(&self) -> Vec<&dyn ContentAnalyzer> {
        let mut analyzers: Vec<_> = self.analyzers.iter().map(|a| a.as_ref()).collect();
        analyzers.sort_by_key(|a| a.priority());
        analyzers
    }

    /// Get analyzers that can handle a specific content type
    pub fn analyzers_for(&self, content_type: ContentType) -> Vec<&dyn ContentAnalyzer> {
        self.analyzers()
            .into_iter()
            .filter(|a| a.can_handle(content_type))
            .collect()
    }

    /// Get analyzers that don't require LLM (fast path)
    pub fn programmatic_analyzers(&self) -> Vec<&dyn ContentAnalyzer> {
        self.analyzers()
            .into_iter()
            .filter(|a| !a.requires_llm())
            .collect()
    }

    /// Get analyzers that require LLM (slow path)
    pub fn llm_analyzers(&self) -> Vec<&dyn ContentAnalyzer> {
        self.analyzers()
            .into_iter()
            .filter(|a| a.requires_llm())
            .collect()
    }

    /// Get analyzers for a specific dimension
    pub fn analyzers_for_dimension(&self, dimension: &str) -> Vec<&dyn ContentAnalyzer> {
        self.analyzers()
            .into_iter()
            .filter(|a| a.dimensions().contains(&dimension))
            .collect()
    }

    /// Number of registered analyzers
    pub fn len(&self) -> usize {
        self.analyzers.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test analyzer implementation
    struct TestAnalyzer {
        id: &'static str,
        priority: u32,
        requires_llm: bool,
    }

    #[async_trait]
    impl ContentAnalyzer for TestAnalyzer {
        fn id(&self) -> &str {
            self.id
        }
        fn name(&self) -> &str {
            "Test Analyzer"
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
        fn requires_llm(&self) -> bool {
            self.requires_llm
        }
        fn priority(&self) -> u32 {
            self.priority
        }
        async fn analyze(&self, _scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
            Ok(AnalysisResult::new())
        }
    }

    #[test]
    fn test_registry_ordering() {
        let mut registry = AnalyzerRegistry::new();
        registry.register(TestAnalyzer {
            id: "high",
            priority: 200,
            requires_llm: false,
        });
        registry.register(TestAnalyzer {
            id: "low",
            priority: 50,
            requires_llm: false,
        });
        registry.register(TestAnalyzer {
            id: "medium",
            priority: 100,
            requires_llm: false,
        });

        let analyzers = registry.analyzers();
        assert_eq!(analyzers[0].id(), "low");
        assert_eq!(analyzers[1].id(), "medium");
        assert_eq!(analyzers[2].id(), "high");
    }

    #[test]
    fn test_registry_llm_filtering() {
        let mut registry = AnalyzerRegistry::new();
        registry.register(TestAnalyzer {
            id: "programmatic",
            priority: 100,
            requires_llm: false,
        });
        registry.register(TestAnalyzer {
            id: "llm",
            priority: 100,
            requires_llm: true,
        });

        assert_eq!(registry.programmatic_analyzers().len(), 1);
        assert_eq!(registry.llm_analyzers().len(), 1);
        assert_eq!(registry.programmatic_analyzers()[0].id(), "programmatic");
        assert_eq!(registry.llm_analyzers()[0].id(), "llm");
    }
}
