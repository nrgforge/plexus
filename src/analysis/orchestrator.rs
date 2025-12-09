//! Analysis orchestrator for coordinating multiple analyzers
//!
//! Implements parallel execution for programmatic analyzers and
//! sequential execution with rate limiting for LLM analyzers.

use super::merger::ResultMerger;
use super::traits::{AnalyzerRegistry, ContentAnalyzer};
use super::types::{AnalysisError, AnalysisResult, AnalysisScope, GraphMutation};
use crate::graph::ContentType;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Orchestrates content analysis across multiple analyzers
pub struct AnalysisOrchestrator {
    registry: AnalyzerRegistry,
    /// Semaphore to limit concurrent LLM calls
    llm_semaphore: Arc<Semaphore>,
    /// Result merger for combining analyzer outputs
    merger: ResultMerger,
}

impl Default for AnalysisOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisOrchestrator {
    /// Create a new orchestrator with default settings
    pub fn new() -> Self {
        Self {
            registry: AnalyzerRegistry::new(),
            llm_semaphore: Arc::new(Semaphore::new(1)), // One LLM call at a time
            merger: ResultMerger::new(),
        }
    }

    /// Create with a specific LLM concurrency limit
    pub fn with_llm_concurrency(mut self, limit: usize) -> Self {
        self.llm_semaphore = Arc::new(Semaphore::new(limit));
        self
    }

    /// Register an analyzer
    pub fn register<A: ContentAnalyzer + 'static>(&mut self, analyzer: A) {
        self.registry.register(analyzer);
    }

    /// Get the analyzer registry
    pub fn registry(&self) -> &AnalyzerRegistry {
        &self.registry
    }

    /// Run all applicable analyzers on the scope
    ///
    /// Executes programmatic analyzers first (potentially in parallel),
    /// then LLM analyzers sequentially with rate limiting.
    pub async fn analyze(&self, scope: &AnalysisScope) -> Result<GraphMutation, AnalysisError> {
        // Determine which content types we're analyzing
        let content_types: HashSet<ContentType> = scope
            .items_to_analyze()
            .iter()
            .map(|item| item.content_type)
            .collect();

        // Phase 1: Run programmatic analyzers (fast, can run in parallel)
        let programmatic_results = self
            .run_programmatic_analyzers(scope, &content_types)
            .await?;

        // Phase 2: Run LLM analyzers if enabled (slow, rate limited)
        let llm_results = if scope.config.enable_llm_analysis {
            self.run_llm_analyzers(scope, &content_types).await?
        } else {
            Vec::new()
        };

        // Merge all results
        let all_results: Vec<AnalysisResult> = programmatic_results
            .into_iter()
            .chain(llm_results)
            .collect();

        // Use merger to combine and deduplicate
        let mutation = self.merger.merge(all_results, scope);

        Ok(mutation)
    }

    /// Run only programmatic (non-LLM) analyzers
    pub async fn analyze_programmatic(
        &self,
        scope: &AnalysisScope,
    ) -> Result<GraphMutation, AnalysisError> {
        let content_types: HashSet<ContentType> = scope
            .items_to_analyze()
            .iter()
            .map(|item| item.content_type)
            .collect();

        let results = self
            .run_programmatic_analyzers(scope, &content_types)
            .await?;
        let mutation = self.merger.merge(results, scope);

        Ok(mutation)
    }

    /// Run programmatic analyzers
    async fn run_programmatic_analyzers(
        &self,
        scope: &AnalysisScope,
        content_types: &HashSet<ContentType>,
    ) -> Result<Vec<AnalysisResult>, AnalysisError> {
        let analyzers: Vec<_> = self
            .registry
            .programmatic_analyzers()
            .into_iter()
            .filter(|a| content_types.iter().any(|ct| a.can_handle(*ct)))
            .collect();

        let mut results = Vec::with_capacity(analyzers.len());

        // Run analyzers sequentially for now
        // TODO: Consider parallel execution for independent analyzers
        for analyzer in analyzers {
            match analyzer.analyze(scope).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    // Log error but continue with other analyzers
                    let mut error_result = AnalysisResult::new();
                    error_result.add_warning(format!(
                        "Analyzer '{}' failed: {}",
                        analyzer.id(),
                        e
                    ));
                    results.push(error_result);
                }
            }
        }

        Ok(results)
    }

    /// Run LLM analyzers with rate limiting
    async fn run_llm_analyzers(
        &self,
        scope: &AnalysisScope,
        content_types: &HashSet<ContentType>,
    ) -> Result<Vec<AnalysisResult>, AnalysisError> {
        let analyzers: Vec<_> = self
            .registry
            .llm_analyzers()
            .into_iter()
            .filter(|a| content_types.iter().any(|ct| a.can_handle(*ct)))
            .collect();

        let mut results = Vec::with_capacity(analyzers.len());

        for analyzer in analyzers {
            // Acquire semaphore permit for rate limiting
            let _permit = self
                .llm_semaphore
                .acquire()
                .await
                .map_err(|e| AnalysisError::Internal(format!("Semaphore error: {}", e)))?;

            // Run with timeout
            let timeout = tokio::time::Duration::from_secs(scope.config.llm_timeout_seconds);
            match tokio::time::timeout(timeout, analyzer.analyze(scope)).await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    let mut error_result = AnalysisResult::new();
                    error_result.add_warning(format!(
                        "LLM analyzer '{}' failed: {}",
                        analyzer.id(),
                        e
                    ));
                    results.push(error_result);
                }
                Err(_) => {
                    let mut error_result = AnalysisResult::new();
                    error_result.add_warning(format!(
                        "LLM analyzer '{}' timed out after {} seconds",
                        analyzer.id(),
                        scope.config.llm_timeout_seconds
                    ));
                    results.push(error_result);
                }
            }
        }

        Ok(results)
    }

    /// Check if any LLM analyzers are registered
    pub fn has_llm_analyzers(&self) -> bool {
        !self.registry.llm_analyzers().is_empty()
    }

    /// Get list of registered analyzer IDs
    pub fn analyzer_ids(&self) -> Vec<&str> {
        self.registry.analyzers().iter().map(|a| a.id()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::types::{AnalysisCapability, ContentItem};
    use crate::graph::ContextId;
    use async_trait::async_trait;

    struct MockAnalyzer {
        id: &'static str,
        requires_llm: bool,
    }

    #[async_trait]
    impl ContentAnalyzer for MockAnalyzer {
        fn id(&self) -> &str {
            self.id
        }
        fn name(&self) -> &str {
            "Mock"
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
        async fn analyze(&self, _scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
            Ok(AnalysisResult::new())
        }
    }

    #[tokio::test]
    async fn test_orchestrator_registration() {
        let mut orchestrator = AnalysisOrchestrator::new();
        orchestrator.register(MockAnalyzer {
            id: "test1",
            requires_llm: false,
        });
        orchestrator.register(MockAnalyzer {
            id: "test2",
            requires_llm: true,
        });

        assert_eq!(orchestrator.registry().len(), 2);
        assert!(orchestrator.has_llm_analyzers());
    }

    #[tokio::test]
    async fn test_orchestrator_analyze() {
        let mut orchestrator = AnalysisOrchestrator::new();
        orchestrator.register(MockAnalyzer {
            id: "test",
            requires_llm: false,
        });

        let scope = AnalysisScope::new(
            ContextId::from_string("test"),
            vec![ContentItem::new("file1", ContentType::Document, "content")],
        );

        let mutation = orchestrator.analyze(&scope).await.unwrap();
        assert!(mutation.is_empty()); // Mock returns empty result
    }

    #[tokio::test]
    async fn test_orchestrator_skips_llm_when_disabled() {
        let mut orchestrator = AnalysisOrchestrator::new();
        orchestrator.register(MockAnalyzer {
            id: "llm",
            requires_llm: true,
        });

        let scope = AnalysisScope::new(
            ContextId::from_string("test"),
            vec![ContentItem::new("file1", ContentType::Document, "content")],
        )
        .with_config(super::super::types::AnalysisConfig::local_only());

        // Should complete without running LLM analyzer
        let mutation = orchestrator.analyze(&scope).await.unwrap();
        assert!(mutation.is_empty());
    }
}
