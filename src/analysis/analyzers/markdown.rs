//! Markdown structure analyzer
//!
//! Parses markdown AST and creates nodes for structural elements:
//! headings, sections, code blocks, lists, blockquotes, tables.

use crate::analysis::{
    AnalysisCapability, AnalysisError, AnalysisResult, AnalysisScope, ContentAnalyzer,
};
use crate::graph::{ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// Analyzer that extracts structural elements from markdown content
///
/// Target dimension: `structure`
///
/// Creates nodes for:
/// - `heading` (h1-h6) with level, text, anchor
/// - `section` (content between headings)
/// - `code_block` with language, content
/// - `list` / `list_item`
/// - `blockquote`
/// - `table`
///
/// Creates edges for:
/// - `contains`: section -> child elements
/// - `follows`: sequential sections/headings
pub struct MarkdownStructureAnalyzer {
    /// Priority (lower = earlier). Default 10 for structure analysis.
    priority: u32,
}

impl Default for MarkdownStructureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownStructureAnalyzer {
    pub fn new() -> Self {
        Self { priority: 10 }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Generate a slug/anchor from heading text
    fn slugify(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Parse a single content item and extract structure
    fn parse_content(
        &self,
        content: &str,
        source_id: &str,
        result: &mut AnalysisResult,
    ) {
        let options = Options::all();
        let parser = Parser::new_ext(content, options);

        let mut current_heading: Option<NodeId> = None;
        let mut heading_stack: Vec<(NodeId, u8)> = Vec::new(); // (node_id, level)
        let mut previous_element: Option<NodeId> = None;
        let mut current_text = String::new();
        let mut in_heading = false;
        let mut heading_level: u8 = 0;
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut code_block_content = String::new();
        let mut list_depth: usize = 0;
        let mut _current_list: Option<NodeId> = None;
        let mut in_blockquote = false;
        let mut blockquote_content = String::new();

        // Track line numbers (approximate)
        let mut line_number: usize = 1;

        // Create document root node
        let doc_node_id = NodeId::from_string(format!("{}:document", source_id));
        let mut doc_node = Node::new_in_dimension("document", ContentType::Document, "structure");
        doc_node.id = doc_node_id.clone();
        doc_node.properties.insert(
            "source".into(),
            PropertyValue::String(source_id.to_string()),
        );
        doc_node.properties.insert(
            "_source_content".into(),
            PropertyValue::String(source_id.to_string()),
        );
        result.nodes.push(doc_node);

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = true;
                    heading_level = level as u8;
                    current_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    in_heading = false;
                    let text = current_text.trim().to_string();
                    let anchor = Self::slugify(&text);

                    let node_id = NodeId::from_string(format!("{}:heading:{}", source_id, anchor));
                    let mut node =
                        Node::new_in_dimension("heading", ContentType::Document, "structure");
                    node.id = node_id.clone();
                    node.properties
                        .insert("level".into(), PropertyValue::Int(heading_level as i64));
                    node.properties
                        .insert("text".into(), PropertyValue::String(text));
                    node.properties
                        .insert("anchor".into(), PropertyValue::String(anchor));
                    node.properties
                        .insert("line".into(), PropertyValue::Int(line_number as i64));
                    node.properties.insert(
                        "_source_content".into(),
                        PropertyValue::String(source_id.to_string()),
                    );

                    result.nodes.push(node);

                    // Create "follows" edge from previous element
                    if let Some(prev_id) = &previous_element {
                        let edge = Edge::new_in_dimension(
                            prev_id.clone(),
                            node_id.clone(),
                            "follows",
                            "structure",
                        );
                        result.edges.push(edge);
                    }

                    // Create "contains" edge from document
                    let contains_edge = Edge::new_in_dimension(
                        doc_node_id.clone(),
                        node_id.clone(),
                        "contains",
                        "structure",
                    );
                    result.edges.push(contains_edge);

                    // Update heading hierarchy
                    while let Some((_, parent_level)) = heading_stack.last() {
                        if *parent_level >= heading_level {
                            heading_stack.pop();
                        } else {
                            break;
                        }
                    }

                    // Create parent-child edge if there's a parent heading
                    if let Some((parent_id, _)) = heading_stack.last() {
                        let parent_edge = Edge::new_in_dimension(
                            parent_id.clone(),
                            node_id.clone(),
                            "contains",
                            "structure",
                        );
                        result.edges.push(parent_edge);
                    }

                    heading_stack.push((node_id.clone(), heading_level));
                    current_heading = Some(node_id.clone());
                    previous_element = Some(node_id);
                    current_text.clear();
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    in_code_block = true;
                    code_block_content.clear();
                    code_block_lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;

                    let node_id =
                        NodeId::from_string(format!("{}:code:{}", source_id, line_number));
                    let mut node =
                        Node::new_in_dimension("code_block", ContentType::Code, "structure");
                    node.id = node_id.clone();

                    if !code_block_lang.is_empty() {
                        node.properties.insert(
                            "language".into(),
                            PropertyValue::String(code_block_lang.clone()),
                        );
                    }
                    node.properties.insert(
                        "content".into(),
                        PropertyValue::String(code_block_content.trim().to_string()),
                    );
                    node.properties
                        .insert("line".into(), PropertyValue::Int(line_number as i64));
                    node.properties.insert(
                        "_source_content".into(),
                        PropertyValue::String(source_id.to_string()),
                    );

                    result.nodes.push(node);

                    // Create "contains" edge from current heading or document
                    let parent_id = current_heading.as_ref().unwrap_or(&doc_node_id);
                    let edge = Edge::new_in_dimension(
                        parent_id.clone(),
                        node_id.clone(),
                        "contains",
                        "structure",
                    );
                    result.edges.push(edge);

                    previous_element = Some(node_id);
                    code_block_content.clear();
                    code_block_lang.clear();
                }
                Event::Start(Tag::List(_)) => {
                    list_depth += 1;
                    if list_depth == 1 {
                        let node_id =
                            NodeId::from_string(format!("{}:list:{}", source_id, line_number));
                        let mut node =
                            Node::new_in_dimension("list", ContentType::Document, "structure");
                        node.id = node_id.clone();
                        node.properties
                            .insert("line".into(), PropertyValue::Int(line_number as i64));
                        node.properties.insert(
                            "_source_content".into(),
                            PropertyValue::String(source_id.to_string()),
                        );

                        result.nodes.push(node);

                        // Create "contains" edge from current heading or document
                        let parent_id = current_heading.as_ref().unwrap_or(&doc_node_id);
                        let edge = Edge::new_in_dimension(
                            parent_id.clone(),
                            node_id.clone(),
                            "contains",
                            "structure",
                        );
                        result.edges.push(edge);

                        _current_list = Some(node_id.clone());
                        previous_element = Some(node_id);
                    }
                }
                Event::End(TagEnd::List(_)) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        _current_list = None;
                    }
                }
                Event::Start(Tag::BlockQuote) => {
                    in_blockquote = true;
                    blockquote_content.clear();
                }
                Event::End(TagEnd::BlockQuote) => {
                    in_blockquote = false;

                    let node_id =
                        NodeId::from_string(format!("{}:blockquote:{}", source_id, line_number));
                    let mut node =
                        Node::new_in_dimension("blockquote", ContentType::Document, "structure");
                    node.id = node_id.clone();
                    node.properties.insert(
                        "content".into(),
                        PropertyValue::String(blockquote_content.trim().to_string()),
                    );
                    node.properties
                        .insert("line".into(), PropertyValue::Int(line_number as i64));
                    node.properties.insert(
                        "_source_content".into(),
                        PropertyValue::String(source_id.to_string()),
                    );

                    result.nodes.push(node);

                    // Create "contains" edge
                    let parent_id = current_heading.as_ref().unwrap_or(&doc_node_id);
                    let edge = Edge::new_in_dimension(
                        parent_id.clone(),
                        node_id.clone(),
                        "contains",
                        "structure",
                    );
                    result.edges.push(edge);

                    previous_element = Some(node_id);
                    blockquote_content.clear();
                }
                Event::Start(Tag::Table(_)) => {
                    let node_id =
                        NodeId::from_string(format!("{}:table:{}", source_id, line_number));
                    let mut node =
                        Node::new_in_dimension("table", ContentType::Document, "structure");
                    node.id = node_id.clone();
                    node.properties
                        .insert("line".into(), PropertyValue::Int(line_number as i64));
                    node.properties.insert(
                        "_source_content".into(),
                        PropertyValue::String(source_id.to_string()),
                    );

                    result.nodes.push(node);

                    // Create "contains" edge
                    let parent_id = current_heading.as_ref().unwrap_or(&doc_node_id);
                    let edge = Edge::new_in_dimension(
                        parent_id.clone(),
                        node_id.clone(),
                        "contains",
                        "structure",
                    );
                    result.edges.push(edge);

                    previous_element = Some(node_id);
                }
                Event::Text(text) => {
                    if in_heading {
                        current_text.push_str(&text);
                    } else if in_code_block {
                        code_block_content.push_str(&text);
                    } else if in_blockquote {
                        blockquote_content.push_str(&text);
                    }
                    // Count newlines for line tracking
                    line_number += text.matches('\n').count();
                }
                Event::Code(code) => {
                    if in_heading {
                        current_text.push_str(&code);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    line_number += 1;
                    if in_heading {
                        current_text.push(' ');
                    } else if in_blockquote {
                        blockquote_content.push('\n');
                    }
                }
                _ => {}
            }
        }
    }
}

#[async_trait]
impl ContentAnalyzer for MarkdownStructureAnalyzer {
    fn id(&self) -> &str {
        "markdown-structure"
    }

    fn name(&self) -> &str {
        "Markdown Structure Analyzer"
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

    fn priority(&self) -> u32 {
        self.priority
    }

    async fn analyze(&self, scope: &AnalysisScope) -> Result<AnalysisResult, AnalysisError> {
        let mut result = AnalysisResult::new();

        for item in scope.items_to_analyze() {
            // Only process markdown/document content
            if item.content_type != ContentType::Document {
                continue;
            }

            // Check content size
            if let Some(max_size) = scope.config.max_content_size {
                if item.content.len() > max_size {
                    result.add_warning(format!(
                        "Skipping {} - content too large ({} bytes)",
                        item.id.as_str(),
                        item.content.len()
                    ));
                    continue;
                }
            }

            self.parse_content(&item.content, item.id.as_str(), &mut result);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AnalysisScope, ContentItem};
    use crate::graph::ContextId;

    #[tokio::test]
    async fn test_parse_headings() {
        let analyzer = MarkdownStructureAnalyzer::new();
        let content = r#"# Title

## Section One

Some content here.

## Section Two

More content.

### Subsection

Nested content.
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        // Should have: document + 4 headings
        let headings: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "heading")
            .collect();
        assert_eq!(headings.len(), 4);

        // Check heading levels
        let h1: Vec<_> = headings
            .iter()
            .filter(|n| {
                matches!(n.properties.get("level"), Some(PropertyValue::Int(1)))
            })
            .collect();
        assert_eq!(h1.len(), 1);

        let h2: Vec<_> = headings
            .iter()
            .filter(|n| {
                matches!(n.properties.get("level"), Some(PropertyValue::Int(2)))
            })
            .collect();
        assert_eq!(h2.len(), 2);

        let h3: Vec<_> = headings
            .iter()
            .filter(|n| {
                matches!(n.properties.get("level"), Some(PropertyValue::Int(3)))
            })
            .collect();
        assert_eq!(h3.len(), 1);
    }

    #[tokio::test]
    async fn test_parse_code_blocks() {
        let analyzer = MarkdownStructureAnalyzer::new();
        let content = r#"# Example

```rust
fn main() {
    println!("Hello");
}
```

```python
print("Hello")
```
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let code_blocks: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "code_block")
            .collect();
        assert_eq!(code_blocks.len(), 2);

        // Check languages
        let rust_block = code_blocks
            .iter()
            .find(|n| {
                matches!(
                    n.properties.get("language"),
                    Some(PropertyValue::String(s)) if s == "rust"
                )
            });
        assert!(rust_block.is_some());

        let python_block = code_blocks
            .iter()
            .find(|n| {
                matches!(
                    n.properties.get("language"),
                    Some(PropertyValue::String(s)) if s == "python"
                )
            });
        assert!(python_block.is_some());
    }

    #[tokio::test]
    async fn test_parse_lists_and_blockquotes() {
        let analyzer = MarkdownStructureAnalyzer::new();
        let content = r#"# Notes

- Item 1
- Item 2
- Item 3

> This is a quote
> spanning multiple lines
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let lists: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "list")
            .collect();
        assert_eq!(lists.len(), 1);

        let blockquotes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "blockquote")
            .collect();
        assert_eq!(blockquotes.len(), 1);
    }

    #[tokio::test]
    async fn test_creates_contains_edges() {
        let analyzer = MarkdownStructureAnalyzer::new();
        let content = r#"# Title

```rust
code
```
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let contains_edges: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "contains")
            .collect();

        // Document -> heading, document -> code_block (via heading)
        assert!(contains_edges.len() >= 2);
    }

    #[tokio::test]
    async fn test_creates_follows_edges() {
        let analyzer = MarkdownStructureAnalyzer::new();
        let content = r#"# First

## Second

## Third
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let follows_edges: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "follows")
            .collect();

        // First -> Second, Second -> Third
        assert_eq!(follows_edges.len(), 2);
    }

    #[tokio::test]
    async fn test_slugify() {
        assert_eq!(
            MarkdownStructureAnalyzer::slugify("Hello World"),
            "hello-world"
        );
        assert_eq!(
            MarkdownStructureAnalyzer::slugify("API Reference (v2)"),
            "api-reference-v2"
        );
        assert_eq!(
            MarkdownStructureAnalyzer::slugify("  Spaced  Out  "),
            "spaced-out"
        );
    }
}
