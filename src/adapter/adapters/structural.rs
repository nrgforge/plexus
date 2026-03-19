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
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

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

/// Built-in structural module for markdown files (ADR-032).
///
/// Uses pulldown-cmark to extract:
/// - Section boundaries from ATX headings
/// - Vocabulary from heading text, link display text, and code block languages
///
/// MIME affinity: `text/markdown` — does not match `text/plain` or other text types.
pub struct MarkdownStructureModule;

impl MarkdownStructureModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StructuralModule for MarkdownStructureModule {
    fn id(&self) -> &str {
        "extract-analysis-markdown-structure"
    }

    fn mime_affinity(&self) -> &str {
        "text/markdown"
    }

    async fn analyze(&self, _file_path: &str, content: &str) -> StructuralOutput {
        let mut vocabulary: Vec<String> = Vec::new();
        let mut sections: Vec<SectionBoundary> = Vec::new();
        let mut in_heading = false;
        let mut current_heading_text = String::new();

        // Build a line offset table: byte offset → 1-based line number
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(content.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let total_lines = content.lines().count();

        let byte_to_line = |byte_offset: usize| -> usize {
            match line_starts.binary_search(&byte_offset) {
                Ok(idx) => idx + 1,
                Err(idx) => idx, // byte is in the middle of line idx (1-based)
            }
        };

        // Track heading positions for section boundary computation
        struct HeadingInfo {
            label: String,
            start_line: usize,
        }
        let mut heading_infos: Vec<HeadingInfo> = Vec::new();

        let mut in_link = false;
        let mut link_text = String::new();

        let parser = Parser::new_ext(content, Options::empty()).into_offset_iter();

        for (event, range) in parser {
            match event {
                Event::Start(Tag::Heading { .. }) => {
                    in_heading = true;
                    current_heading_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    if in_heading {
                        let heading_line = byte_to_line(range.start);
                        let text = current_heading_text.trim().to_string();
                        if !text.is_empty() {
                            let lower = text.to_lowercase();
                            if !vocabulary.iter().any(|v| v.to_lowercase() == lower) {
                                vocabulary.push(lower);
                            }
                            heading_infos.push(HeadingInfo {
                                label: text,
                                start_line: heading_line,
                            });
                        }
                        in_heading = false;
                        current_heading_text.clear();
                    }
                }
                Event::Start(Tag::Link { .. }) => {
                    in_link = true;
                    link_text.clear();
                }
                Event::End(TagEnd::Link) => {
                    if in_link {
                        let text = link_text.trim().to_string();
                        if !text.is_empty() {
                            let lower = text.to_lowercase();
                            if !vocabulary.iter().any(|v| v.to_lowercase() == lower) {
                                vocabulary.push(lower);
                            }
                        }
                        in_link = false;
                        link_text.clear();
                    }
                }
                Event::Code(code) if in_heading => {
                    current_heading_text.push_str(&code);
                }
                Event::Code(code) if in_link => {
                    link_text.push_str(&code);
                }
                Event::Text(text) if in_heading => {
                    current_heading_text.push_str(&text);
                }
                Event::Text(text) if in_link => {
                    link_text.push_str(&text);
                }
                Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(lang))) => {
                    let lang_str = lang.trim().to_string();
                    if !lang_str.is_empty() {
                        let lower = lang_str.to_lowercase();
                        if !vocabulary.iter().any(|v| v.to_lowercase() == lower) {
                            vocabulary.push(lower);
                        }
                    }
                }
                _ => {}
            }
        }

        // Compute section boundaries from heading positions.
        // Each heading starts a section; it ends at the line before the next
        // heading of equal or higher level, or at end-of-file.
        for (i, info) in heading_infos.iter().enumerate() {
            let end_line = if i + 1 < heading_infos.len() {
                // Section ends at the line before the next heading
                heading_infos[i + 1].start_line.saturating_sub(1).max(info.start_line)
            } else {
                // Last heading — section runs to end of file
                total_lines
            };
            sections.push(SectionBoundary {
                label: info.label.clone(),
                start_line: info.start_line,
                end_line,
            });
        }

        StructuralOutput {
            vocabulary,
            sections,
            emissions: Vec::new(),
        }
    }
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

    // --- MarkdownStructureModule tests ---

    #[test]
    fn markdown_module_has_correct_id_and_affinity() {
        let m = MarkdownStructureModule::new();
        assert_eq!(m.id(), "extract-analysis-markdown-structure");
        assert_eq!(m.mime_affinity(), "text/markdown");
    }

    #[test]
    fn markdown_affinity_does_not_match_plain_text() {
        let m = MarkdownStructureModule::new();
        // MIME dispatch uses starts_with — "text/plain".starts_with("text/markdown") is false
        assert!(!"text/plain".starts_with(m.mime_affinity()));
        assert!("text/markdown".starts_with(m.mime_affinity()));
    }

    #[tokio::test]
    async fn markdown_heading_extraction_produces_sections() {
        let content = "# Title\nIntroduction paragraph.\n## Architecture\nArchitecture details.\n## Testing\nTest details.\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        assert_eq!(output.sections.len(), 3);

        assert_eq!(output.sections[0].label, "Title");
        assert_eq!(output.sections[0].start_line, 1);
        assert_eq!(output.sections[0].end_line, 2);

        assert_eq!(output.sections[1].label, "Architecture");
        assert_eq!(output.sections[1].start_line, 3);
        assert_eq!(output.sections[1].end_line, 4);

        assert_eq!(output.sections[2].label, "Testing");
        assert_eq!(output.sections[2].start_line, 5);
        assert_eq!(output.sections[2].end_line, 6);
    }

    #[tokio::test]
    async fn markdown_link_extraction_produces_vocabulary() {
        let content = "Check out [Plexus](https://example.com) and [knowledge graph](./docs/kg.md).\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        assert!(output.vocabulary.contains(&"plexus".to_string()));
        assert!(output.vocabulary.contains(&"knowledge graph".to_string()));
    }

    #[tokio::test]
    async fn markdown_heading_text_contributes_to_vocabulary() {
        let content = "## Extraction Architecture\nSome text.\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        assert!(output.vocabulary.contains(&"extraction architecture".to_string()));
    }

    #[tokio::test]
    async fn markdown_vocabulary_is_deduplicated_case_insensitively() {
        let content = "## Plexus\nRead about [plexus](url) and [PLEXUS](url2).\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        let plexus_count = output.vocabulary.iter()
            .filter(|v| v.to_lowercase() == "plexus")
            .count();
        assert_eq!(plexus_count, 1, "plexus should appear exactly once");
    }

    #[tokio::test]
    async fn markdown_code_block_language_in_vocabulary() {
        let content = "```rust\nfn main() {}\n```\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        assert!(output.vocabulary.contains(&"rust".to_string()));
    }

    #[tokio::test]
    async fn markdown_no_structure_returns_empty() {
        let content = "Just some paragraphs of text.\n\nNo headings here.\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        assert!(output.sections.is_empty());
        assert!(output.emissions.is_empty());
        // No error raised — graceful empty output
    }

    #[tokio::test]
    async fn markdown_empty_content_returns_empty() {
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", "").await;

        assert!(output.sections.is_empty());
        assert!(output.vocabulary.is_empty());
        assert!(output.emissions.is_empty());
    }

    #[tokio::test]
    async fn markdown_emissions_are_empty_by_default() {
        let content = "# Title\n[link](url)\n";
        let m = MarkdownStructureModule::new();
        let output = m.analyze("test.md", content).await;

        // Module produces vocabulary and sections but no graph emissions yet
        assert!(output.emissions.is_empty());
        assert!(!output.vocabulary.is_empty());
        assert!(!output.sections.is_empty());
    }
}
