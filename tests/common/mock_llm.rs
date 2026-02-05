//! Mock LLM analyzer for spike tests
//!
//! Provides a deterministic semantic analyzer that extracts concepts
//! from markdown content without actual LLM calls. Used when the
//! `real_llm` feature is disabled.

use async_trait::async_trait;
use plexus::{
    AnalysisCapability, AnalysisError, AnalysisResult, AnalysisScope, ContentAnalyzer,
    ContentType, Edge, Node, PropertyValue,
};

/// Mock semantic analyzer that extracts concepts from markdown structure
///
/// This analyzer simulates LLM-based concept extraction by:
/// - Extracting headings as concepts (H1, H2, H3)
/// - Extracting code block languages as technology concepts
/// - Extracting bold/emphasized terms as potential concepts
///
/// All mock concepts have `_mock: true` in their properties.
pub struct MockSemanticAnalyzer {
    /// Minimum confidence threshold for concept extraction
    min_confidence: f32,
}

impl Default for MockSemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            min_confidence: 0.5,
        }
    }

    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence;
        self
    }

    /// Extract concepts from markdown content
    fn extract_concepts(&self, content: &str, source_id: &str) -> Vec<MockConcept> {
        let mut concepts = Vec::new();

        // Extract heading-based concepts
        for line in content.lines() {
            let trimmed = line.trim();

            // H1 headers -> high confidence concepts
            if trimmed.starts_with("# ") {
                let name = trimmed.trim_start_matches("# ").trim();
                if !name.is_empty() {
                    concepts.push(MockConcept {
                        name: name.to_string(),
                        confidence: 0.9,
                        source: source_id.to_string(),
                        concept_type: ConceptType::Topic,
                    });
                }
            }
            // H2 headers -> medium-high confidence
            else if trimmed.starts_with("## ") {
                let name = trimmed.trim_start_matches("## ").trim();
                if !name.is_empty() {
                    concepts.push(MockConcept {
                        name: name.to_string(),
                        confidence: 0.8,
                        source: source_id.to_string(),
                        concept_type: ConceptType::Subtopic,
                    });
                }
            }
            // H3 headers -> medium confidence
            else if trimmed.starts_with("### ") {
                let name = trimmed.trim_start_matches("### ").trim();
                if !name.is_empty() {
                    concepts.push(MockConcept {
                        name: name.to_string(),
                        confidence: 0.7,
                        source: source_id.to_string(),
                        concept_type: ConceptType::Detail,
                    });
                }
            }
        }

        // Extract code block languages as technology concepts
        let code_block_pattern = regex_lite::Regex::new(r"```(\w+)").unwrap();
        for cap in code_block_pattern.captures_iter(content) {
            if let Some(lang) = cap.get(1) {
                let lang_name = lang.as_str();
                // Skip common/generic language names
                if !["text", "plaintext", "output", "console"].contains(&lang_name) {
                    concepts.push(MockConcept {
                        name: lang_name.to_string(),
                        confidence: 0.75,
                        source: source_id.to_string(),
                        concept_type: ConceptType::Technology,
                    });
                }
            }
        }

        // Filter by confidence threshold and deduplicate
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        concepts
            .into_iter()
            .filter(|c| c.confidence >= self.min_confidence)
            .filter(|c| seen.insert(c.name.to_lowercase()))
            .collect()
    }
}

#[async_trait]
impl ContentAnalyzer for MockSemanticAnalyzer {
    fn id(&self) -> &str {
        "mock-semantic"
    }

    fn name(&self) -> &str {
        "Mock Semantic Analyzer"
    }

    fn dimensions(&self) -> Vec<&str> {
        vec!["semantic"]
    }

    fn capabilities(&self) -> Vec<AnalysisCapability> {
        vec![AnalysisCapability::Entities, AnalysisCapability::Semantics]
    }

    fn handles(&self) -> Vec<ContentType> {
        vec![ContentType::Document]
    }

    fn requires_llm(&self) -> bool {
        // This is a mock - pretends to be LLM but isn't
        false
    }

    async fn analyze(&self, scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
        let mut result = AnalysisResult::new();

        for item in scope.items_to_analyze() {
            let source_id = item.id.as_str();
            let concepts = self.extract_concepts(&item.content, source_id);

            // Create concept nodes
            for concept in &concepts {
                let mut node =
                    Node::new_in_dimension("concept", ContentType::Document, "semantic");
                node.properties
                    .insert("name".into(), PropertyValue::String(concept.name.clone()));
                node.properties.insert(
                    "confidence".into(),
                    PropertyValue::Float(concept.confidence as f64),
                );
                node.properties.insert(
                    "type".into(),
                    PropertyValue::String(concept.concept_type.as_str().to_string()),
                );
                node.properties
                    .insert("source".into(), PropertyValue::String(source_id.to_string()));
                // Mark as mock for test verification
                node.properties
                    .insert("_mock".into(), PropertyValue::Bool(true));

                result.add_node(node, &item.id);
            }

            // Create edges between concepts in same document (co-occurrence)
            if concepts.len() > 1 {
                // Collect node IDs first to avoid borrow issues
                let concept_node_ids: Vec<_> = result
                    .nodes
                    .iter()
                    .filter(|n| {
                        n.properties
                            .get("source")
                            .map(|v| matches!(v, PropertyValue::String(s) if s == source_id))
                            .unwrap_or(false)
                    })
                    .map(|n| n.id.clone())
                    .collect();

                for i in 0..concept_node_ids.len() {
                    for j in (i + 1)..concept_node_ids.len() {
                        let mut edge = Edge::new_in_dimension(
                            concept_node_ids[i].clone(),
                            concept_node_ids[j].clone(),
                            "co_occurs",
                            "semantic",
                        );
                        edge.raw_weight = 0.5; // Default co-occurrence raw weight
                        result.add_edge(edge);
                    }
                }
            }
        }

        Ok(result)
    }
}

/// Internal concept representation
#[derive(Debug, Clone)]
struct MockConcept {
    name: String,
    confidence: f32,
    source: String,
    concept_type: ConceptType,
}

/// Types of concepts the mock analyzer can identify
#[derive(Debug, Clone, Copy)]
enum ConceptType {
    /// Main topic (from H1)
    Topic,
    /// Subtopic (from H2)
    Subtopic,
    /// Detail (from H3)
    Detail,
    /// Technology/language (from code blocks)
    Technology,
}

impl ConceptType {
    fn as_str(&self) -> &'static str {
        match self {
            ConceptType::Topic => "topic",
            ConceptType::Subtopic => "subtopic",
            ConceptType::Detail => "detail",
            ConceptType::Technology => "technology",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plexus::{ContextId, ContentId, ContentItem};

    #[tokio::test]
    async fn test_mock_analyzer_extracts_headings() {
        let analyzer = MockSemanticAnalyzer::new();

        let content = r#"
# Main Topic

Some intro text.

## Subtopic One

Details here.

### Detail Point

More details.
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        // Should extract 3 concepts from headings
        assert_eq!(result.nodes.len(), 3);

        // All should be marked as mock
        for node in &result.nodes {
            assert!(matches!(
                node.properties.get("_mock"),
                Some(PropertyValue::Bool(true))
            ));
        }
    }

    #[tokio::test]
    async fn test_mock_analyzer_extracts_code_languages() {
        let analyzer = MockSemanticAnalyzer::new();

        let content = r#"
# Code Examples

```rust
fn main() {}
```

```python
print("hello")
```
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        // Should have: "Code Examples" (topic) + "rust" + "python"
        assert_eq!(result.nodes.len(), 3);

        let tech_concepts: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.properties.get("type"),
                    Some(PropertyValue::String(s)) if s == "technology"
                )
            })
            .collect();

        assert_eq!(tech_concepts.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_analyzer_creates_cooccurrence_edges() {
        let analyzer = MockSemanticAnalyzer::new();

        let content = r#"
# Topic A

## Subtopic B

## Subtopic C
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        // 3 concepts = 3 co-occurrence edges (A-B, A-C, B-C)
        assert_eq!(result.edges.len(), 3);
        assert!(result.edges.iter().all(|e| e.relationship == "co_occurs"));
    }
}
