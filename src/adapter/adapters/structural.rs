//! Structural module system — MIME-dispatched heuristic analysis (ADR-030, ADR-031).
//!
//! Structural modules run inside the ExtractionCoordinator's structural
//! analysis dispatch, selected by MIME type affinity (Invariant 55).
//! Each module analyzes a file and returns `StructuralOutput` — vocabulary,
//! optional section boundaries, and graph emissions.
//!
//! The coordinator reads the file once, dispatches to all matching modules
//! (fan-out, Invariant 51), merges their outputs (Invariant 53), and
//! hands vocabulary + sections to semantic extraction (ADR-031).

use crate::adapter::types::{AnnotatedEdge, AnnotatedNode};
use async_trait::async_trait;

/// A registered component for heuristic structural analysis.
///
/// Dispatched by the extraction coordinator via MIME type affinity —
/// a different routing mechanism than IngestPipeline's input-kind routing
/// (Invariant 55). Whether this trait extends Adapter or stands alone
/// is a BUILD-time implementation decision.
#[async_trait]
pub trait StructuralModule: Send + Sync {
    /// Stable identifier for contribution tracking.
    /// Convention: `extract-analysis-{modality}-{function}`
    fn id(&self) -> &str;

    /// MIME type prefix this module handles (e.g., `text/`, `text/markdown`).
    fn mime_affinity(&self) -> &str;

    /// Analyze a file and produce structural output.
    ///
    /// The coordinator reads the file and passes content — modules do not
    /// perform file I/O. The `content: &str` parameter works for text-based
    /// modalities; binary-file modules would read from `file_path` directly.
    ///
    /// An empty output is normal — not every file yields useful structure.
    async fn analyze(&self, file_path: &str, content: &str) -> StructuralOutput;
}

/// Combined output from structural analysis modules for a single file.
///
/// Merged by the coordinator when multiple modules match (Invariant 53):
/// vocabulary unioned case-insensitively, sections sorted by start_line,
/// emissions kept per-module.
#[derive(Debug, Clone, Default)]
pub struct StructuralOutput {
    /// Vocabulary terms discovered by structural analysis.
    /// Entity names, key terms, link targets — passed to semantic
    /// extraction as a glossary hint (not a constraint).
    pub vocabulary: Vec<String>,

    /// Optional structural metadata that may improve semantic extraction's
    /// chunking. Section boundaries from headings, function boundaries from
    /// code parsing, page breaks from PDFs.
    pub sections: Vec<SectionBoundary>,

    /// Graph emissions from structural analysis.
    /// Concept nodes, structural edges — emitted by the coordinator
    /// on behalf of each module with the module's adapter ID.
    pub emissions: Vec<ModuleEmission>,
}

/// A structural boundary identified by structural analysis.
#[derive(Debug, Clone)]
pub struct SectionBoundary {
    pub label: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Graph mutations from a single structural module.
#[derive(Debug, Clone)]
pub struct ModuleEmission {
    /// The module's adapter ID (for contribution tracking).
    pub module_id: String,
    /// Nodes to emit (concept nodes, structural metadata nodes).
    pub nodes: Vec<AnnotatedNode>,
    /// Edges to emit.
    pub edges: Vec<AnnotatedEdge>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal structural module for testing.
    struct StubModule {
        id: &'static str,
        mime: &'static str,
    }

    #[async_trait]
    impl StructuralModule for StubModule {
        fn id(&self) -> &str {
            self.id
        }

        fn mime_affinity(&self) -> &str {
            self.mime
        }

        async fn analyze(&self, _file_path: &str, _content: &str) -> StructuralOutput {
            StructuralOutput::default()
        }
    }

    #[test]
    fn structural_module_has_id_and_mime_affinity() {
        let module = StubModule {
            id: "extract-analysis-text-headings",
            mime: "text/markdown",
        };
        assert_eq!(module.id(), "extract-analysis-text-headings");
        assert_eq!(module.mime_affinity(), "text/markdown");
    }

    #[tokio::test]
    async fn analyze_returns_structural_output() {
        let module = StubModule {
            id: "test",
            mime: "text/",
        };
        let output = module.analyze("test.md", "# Hello").await;
        // Default output is empty — that's the expected case for many file types
        assert!(output.vocabulary.is_empty());
        assert!(output.sections.is_empty());
        assert!(output.emissions.is_empty());
    }

    #[test]
    fn structural_output_default_is_empty() {
        let output = StructuralOutput::default();
        assert!(output.vocabulary.is_empty());
        assert!(output.sections.is_empty());
        assert!(output.emissions.is_empty());
    }

    #[test]
    fn module_emission_carries_module_id() {
        let emission = ModuleEmission {
            module_id: "extract-analysis-text-headings".to_string(),
            nodes: vec![],
            edges: vec![],
        };
        assert_eq!(emission.module_id, "extract-analysis-text-headings");
    }
}
