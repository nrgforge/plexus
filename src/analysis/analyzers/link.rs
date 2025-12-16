//! Link analyzer
//!
//! Extracts markdown links and creates relational edges between documents.

use crate::analysis::{
    AnalysisCapability, AnalysisError, AnalysisResult, AnalysisScope, ContentAnalyzer,
};
use crate::graph::{ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Link classification
#[derive(Debug, Clone, PartialEq)]
pub enum LinkType {
    /// Link to another file in the same tree
    Internal,
    /// Link to external URL
    External,
    /// Anchor link within the same document
    Anchor,
    /// Wikilink style [[link]]
    WikiLink,
}

/// Analyzer that extracts links from markdown content
///
/// Target dimension: `relational`
///
/// Creates nodes for:
/// - `link` with url, text, link_type, status
/// - `url` for external URLs (as targets)
///
/// Creates edges for:
/// - `links_to`: source document -> target document/url
/// - `references`: for external URLs
pub struct LinkAnalyzer {
    priority: u32,
}

impl Default for LinkAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkAnalyzer {
    pub fn new() -> Self {
        Self { priority: 20 } // Run after structure analysis
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Classify a link URL
    fn classify_link(url: &str) -> LinkType {
        if url.starts_with('#') {
            LinkType::Anchor
        } else if url.starts_with("http://") || url.starts_with("https://") {
            LinkType::External
        } else if url.starts_with("[[") && url.ends_with("]]") {
            LinkType::WikiLink
        } else {
            LinkType::Internal
        }
    }

    /// Extract wikilinks from content (not handled by pulldown-cmark)
    fn extract_wikilinks(content: &str) -> Vec<(String, String)> {
        let mut links = Vec::new();
        let mut chars = content.chars().peekable();
        let mut in_link = false;
        let mut link_text = String::new();

        while let Some(c) = chars.next() {
            if c == '[' && chars.peek() == Some(&'[') {
                chars.next(); // consume second '['
                in_link = true;
                link_text.clear();
            } else if in_link && c == ']' && chars.peek() == Some(&']') {
                chars.next(); // consume second ']'
                in_link = false;

                // Parse link text (may have display text after |)
                let (target, display) = if let Some(pipe_pos) = link_text.find('|') {
                    let target = link_text[..pipe_pos].trim().to_string();
                    let display = link_text[pipe_pos + 1..].trim().to_string();
                    (target, display)
                } else {
                    (link_text.clone(), link_text.clone())
                };

                links.push((target, display));
            } else if in_link {
                link_text.push(c);
            }
        }

        links
    }

    /// Parse content and extract links
    fn parse_content(&self, content: &str, source_id: &str, result: &mut AnalysisResult) {
        let options = Options::all();
        let parser = Parser::new_ext(content, options);

        let mut current_link_url = String::new();
        let mut current_link_text = String::new();
        let mut in_link = false;
        let mut link_count = 0;

        for event in parser {
            match event {
                Event::Start(Tag::Link { dest_url, .. }) => {
                    in_link = true;
                    current_link_url = dest_url.to_string();
                    current_link_text.clear();
                }
                Event::End(TagEnd::Link) => {
                    in_link = false;
                    link_count += 1;

                    let link_type = Self::classify_link(&current_link_url);
                    self.create_link_nodes(
                        source_id,
                        &current_link_url,
                        &current_link_text,
                        &link_type,
                        link_count,
                        result,
                    );

                    current_link_url.clear();
                    current_link_text.clear();
                }
                Event::Text(text) if in_link => {
                    current_link_text.push_str(&text);
                }
                Event::Code(code) if in_link => {
                    current_link_text.push_str(&code);
                }
                _ => {}
            }
        }

        // Also extract wikilinks
        for (target, display) in Self::extract_wikilinks(content) {
            link_count += 1;
            self.create_link_nodes(
                source_id,
                &target,
                &display,
                &LinkType::WikiLink,
                link_count,
                result,
            );
        }
    }

    /// Create nodes and edges for a link
    fn create_link_nodes(
        &self,
        source_id: &str,
        url: &str,
        text: &str,
        link_type: &LinkType,
        index: usize,
        result: &mut AnalysisResult,
    ) {
        let link_type_str = match link_type {
            LinkType::Internal => "internal",
            LinkType::External => "external",
            LinkType::Anchor => "anchor",
            LinkType::WikiLink => "wikilink",
        };

        // Create link node
        let link_node_id = NodeId::from_string(format!("{}:link:{}", source_id, index));
        let mut link_node =
            Node::new_in_dimension("link", ContentType::Document, "relational");
        link_node.id = link_node_id.clone();
        link_node
            .properties
            .insert("url".into(), PropertyValue::String(url.to_string()));
        link_node
            .properties
            .insert("text".into(), PropertyValue::String(text.to_string()));
        link_node.properties.insert(
            "link_type".into(),
            PropertyValue::String(link_type_str.to_string()),
        );
        link_node.properties.insert(
            "_source_content".into(),
            PropertyValue::String(source_id.to_string()),
        );

        result.nodes.push(link_node);

        // Create source document node reference
        let source_doc_id = NodeId::from_string(format!("{}:document", source_id));

        // Connect link node to its parent document (prevents orphaned link nodes)
        let contains_edge = Edge::new_in_dimension(
            source_doc_id.clone(),
            link_node_id.clone(),
            "contains",
            "structure",
        );
        result.edges.push(contains_edge);

        // Create edge based on link type
        match link_type {
            LinkType::Internal | LinkType::WikiLink => {
                // Resolve target path (simplified - just use the URL as-is for now)
                let target_path = if url.ends_with(".md") {
                    url.to_string()
                } else {
                    format!("{}.md", url)
                };
                let target_doc_id = NodeId::from_string(format!("{}:document", target_path));

                // Create links_to edge from source doc to target doc
                let edge = Edge::new_in_dimension(
                    source_doc_id,
                    target_doc_id.clone(),
                    "links_to",
                    "relational",
                );
                result.edges.push(edge);

                // Also connect link node to target doc
                let link_to_target = Edge::new_in_dimension(
                    link_node_id,
                    target_doc_id,
                    "links_to",
                    "relational",
                );
                result.edges.push(link_to_target);
            }
            LinkType::External => {
                // Create URL node as target
                let url_node_id = NodeId::from_string(format!("url:{}", url));
                let mut url_node =
                    Node::new_in_dimension("url", ContentType::Document, "relational");
                url_node.id = url_node_id.clone();
                url_node
                    .properties
                    .insert("url".into(), PropertyValue::String(url.to_string()));

                result.nodes.push(url_node);

                // Create references edge from document to URL
                let edge = Edge::new_in_dimension(
                    source_doc_id.clone(),
                    url_node_id.clone(),
                    "references",
                    "relational",
                );
                result.edges.push(edge);

                // Also connect link node to URL node
                let link_to_url = Edge::new_in_dimension(
                    link_node_id,
                    url_node_id,
                    "links_to",
                    "relational",
                );
                result.edges.push(link_to_url);
            }
            LinkType::Anchor => {
                // Create edge to anchor within same document
                let anchor = url.trim_start_matches('#');
                let target_id = NodeId::from_string(format!("{}:heading:{}", source_id, anchor));

                let edge = Edge::new_in_dimension(
                    link_node_id,
                    target_id,
                    "links_to",
                    "relational",
                );
                result.edges.push(edge);
            }
        }
    }
}

#[async_trait]
impl ContentAnalyzer for LinkAnalyzer {
    fn id(&self) -> &str {
        "link-analyzer"
    }

    fn name(&self) -> &str {
        "Link Analyzer"
    }

    fn dimensions(&self) -> Vec<&str> {
        vec!["relational"]
    }

    fn capabilities(&self) -> Vec<AnalysisCapability> {
        vec![AnalysisCapability::References]
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
            if item.content_type != ContentType::Document {
                continue;
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
    async fn test_extract_markdown_links() {
        let analyzer = LinkAnalyzer::new();
        let content = r#"# Links

Check out [this page](./other.md) for more info.

Also see [external](https://example.com) and [anchor](#section).
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let links: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "link")
            .collect();
        assert_eq!(links.len(), 3);

        // Check link types
        let internal: Vec<_> = links
            .iter()
            .filter(|n| {
                matches!(
                    n.properties.get("link_type"),
                    Some(PropertyValue::String(s)) if s == "internal"
                )
            })
            .collect();
        assert_eq!(internal.len(), 1);

        let external: Vec<_> = links
            .iter()
            .filter(|n| {
                matches!(
                    n.properties.get("link_type"),
                    Some(PropertyValue::String(s)) if s == "external"
                )
            })
            .collect();
        assert_eq!(external.len(), 1);

        let anchors: Vec<_> = links
            .iter()
            .filter(|n| {
                matches!(
                    n.properties.get("link_type"),
                    Some(PropertyValue::String(s)) if s == "anchor"
                )
            })
            .collect();
        assert_eq!(anchors.len(), 1);
    }

    #[tokio::test]
    async fn test_extract_wikilinks() {
        let links = LinkAnalyzer::extract_wikilinks("See [[Other Page]] and [[Folder/Note|Display Name]].");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], ("Other Page".to_string(), "Other Page".to_string()));
        assert_eq!(links[1], ("Folder/Note".to_string(), "Display Name".to_string()));
    }

    #[tokio::test]
    async fn test_creates_links_to_edges() {
        let analyzer = LinkAnalyzer::new();
        let content = "[link](./target.md)";

        let items = vec![ContentItem::new("source.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let links_to: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "links_to")
            .collect();
        // 2 edges: source_doc -> target_doc AND link_node -> target_doc
        assert_eq!(links_to.len(), 2);

        // Should also have contains edge: source_doc -> link_node
        let contains: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "contains")
            .collect();
        assert_eq!(contains.len(), 1);
    }

    #[tokio::test]
    async fn test_creates_references_edges_for_external() {
        let analyzer = LinkAnalyzer::new();
        let content = "[google](https://google.com)";

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let references: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "references")
            .collect();
        assert_eq!(references.len(), 1);

        // Should also create URL node
        let url_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "url")
            .collect();
        assert_eq!(url_nodes.len(), 1);
    }

    #[test]
    fn test_classify_link() {
        assert_eq!(LinkAnalyzer::classify_link("#anchor"), LinkType::Anchor);
        assert_eq!(
            LinkAnalyzer::classify_link("https://example.com"),
            LinkType::External
        );
        assert_eq!(
            LinkAnalyzer::classify_link("http://example.com"),
            LinkType::External
        );
        assert_eq!(
            LinkAnalyzer::classify_link("./local.md"),
            LinkType::Internal
        );
        assert_eq!(
            LinkAnalyzer::classify_link("other.md"),
            LinkType::Internal
        );
    }
}
