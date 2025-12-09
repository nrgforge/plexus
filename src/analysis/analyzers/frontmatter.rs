//! Frontmatter analyzer
//!
//! Parses YAML frontmatter from markdown files and extracts metadata.

use crate::analysis::{
    AnalysisCapability, AnalysisError, AnalysisResult, AnalysisScope, ContentAnalyzer,
};
use crate::graph::{ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use std::collections::HashMap;

/// Analyzer that extracts YAML frontmatter from markdown
///
/// Target dimension: `structure` (metadata properties)
///
/// Parses frontmatter between `---` delimiters and maps fields to node properties.
///
/// Special handling for:
/// - `tags`: creates `tagged_with` edges to tag nodes
/// - `aliases`: stored as array property
/// - `date`/`created`/`modified`: temporal metadata
pub struct FrontmatterAnalyzer {
    priority: u32,
}

impl Default for FrontmatterAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrontmatterAnalyzer {
    pub fn new() -> Self {
        Self { priority: 5 } // Run early, before structure analysis
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Extract frontmatter content from markdown
    fn extract_frontmatter(content: &str) -> Option<&str> {
        let content = content.trim_start();

        if !content.starts_with("---") {
            return None;
        }

        let after_first = &content[3..];
        let end_pos = after_first.find("\n---")?;

        Some(&after_first[..end_pos])
    }

    /// Simple YAML parser (handles common cases without full YAML dependency)
    fn parse_simple_yaml(yaml: &str) -> HashMap<String, YamlValue> {
        let mut result = HashMap::new();
        let mut current_key: Option<String> = None;
        let mut list_items: Vec<String> = Vec::new();
        let mut in_list = false;

        for line in yaml.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for list item
            if let Some(stripped) = trimmed.strip_prefix("- ") {
                if in_list {
                    list_items.push(stripped.trim().to_string());
                }
                continue;
            }

            // End any current list
            if in_list {
                if let Some(key) = current_key.take() {
                    result.insert(key, YamlValue::List(std::mem::take(&mut list_items)));
                }
                in_list = false;
            }

            // Parse key: value
            if let Some(colon_pos) = trimmed.find(':') {
                let key = trimmed[..colon_pos].trim().to_string();
                let value = trimmed[colon_pos + 1..].trim();

                if value.is_empty() {
                    // Could be start of a list
                    current_key = Some(key);
                    in_list = true;
                    list_items.clear();
                } else if value.starts_with('[') && value.ends_with(']') {
                    // Inline array
                    let items: Vec<String> = value[1..value.len() - 1]
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    result.insert(key, YamlValue::List(items));
                } else {
                    // Simple value
                    let clean_value = value.trim_matches('"').trim_matches('\'').to_string();
                    result.insert(key, YamlValue::String(clean_value));
                }
            }
        }

        // Handle any remaining list
        if in_list {
            if let Some(key) = current_key {
                result.insert(key, YamlValue::List(list_items));
            }
        }

        result
    }

    /// Convert YamlValue to PropertyValue
    fn yaml_to_property(value: &YamlValue) -> PropertyValue {
        match value {
            YamlValue::String(s) => {
                // Try to parse as number or boolean
                if let Ok(n) = s.parse::<i64>() {
                    PropertyValue::Int(n)
                } else if let Ok(f) = s.parse::<f64>() {
                    PropertyValue::Float(f)
                } else if s == "true" {
                    PropertyValue::Bool(true)
                } else if s == "false" {
                    PropertyValue::Bool(false)
                } else {
                    PropertyValue::String(s.clone())
                }
            }
            YamlValue::List(items) => {
                PropertyValue::Array(items.iter().map(|s| PropertyValue::String(s.clone())).collect())
            }
        }
    }

    /// Parse frontmatter and create nodes/edges
    fn parse_content(&self, content: &str, source_id: &str, result: &mut AnalysisResult) {
        let Some(frontmatter) = Self::extract_frontmatter(content) else {
            return;
        };

        let parsed = Self::parse_simple_yaml(frontmatter);

        if parsed.is_empty() {
            return;
        }

        // Create frontmatter node
        let fm_node_id = NodeId::from_string(format!("{}:frontmatter", source_id));
        let mut fm_node = Node::new_in_dimension("frontmatter", ContentType::Document, "structure");
        fm_node.id = fm_node_id.clone();
        fm_node.properties.insert(
            "_source_content".into(),
            PropertyValue::String(source_id.to_string()),
        );

        // Add all parsed values as properties
        for (key, value) in &parsed {
            fm_node
                .properties
                .insert(key.clone(), Self::yaml_to_property(value));
        }

        result.nodes.push(fm_node);

        // Link frontmatter to document
        let doc_id = NodeId::from_string(format!("{}:document", source_id));
        let edge = Edge::new_in_dimension(doc_id, fm_node_id.clone(), "has_frontmatter", "structure");
        result.edges.push(edge);

        // Handle tags specially - create tag nodes and edges
        if let Some(YamlValue::List(tags)) = parsed.get("tags") {
            for tag in tags {
                let tag_node_id = NodeId::from_string(format!("tag:{}", tag.to_lowercase()));
                let mut tag_node =
                    Node::new_in_dimension("tag", ContentType::Concept, "relational");
                tag_node.id = tag_node_id.clone();
                tag_node
                    .properties
                    .insert("name".into(), PropertyValue::String(tag.clone()));

                result.nodes.push(tag_node);

                // Create tagged_with edge
                let tag_edge = Edge::new_cross_dimensional(
                    fm_node_id.clone(),
                    "structure".to_string(),
                    tag_node_id,
                    "relational".to_string(),
                    "tagged_with",
                );
                result.edges.push(tag_edge);
            }
        }

        // Handle aliases
        if let Some(YamlValue::List(aliases)) = parsed.get("aliases") {
            for (i, alias) in aliases.iter().enumerate() {
                let alias_node_id = NodeId::from_string(format!("{}:alias:{}", source_id, i));
                let mut alias_node =
                    Node::new_in_dimension("alias", ContentType::Document, "structure");
                alias_node.id = alias_node_id.clone();
                alias_node
                    .properties
                    .insert("name".into(), PropertyValue::String(alias.clone()));
                alias_node.properties.insert(
                    "_source_content".into(),
                    PropertyValue::String(source_id.to_string()),
                );

                result.nodes.push(alias_node);

                // Create has_alias edge
                let alias_edge = Edge::new_in_dimension(
                    fm_node_id.clone(),
                    alias_node_id,
                    "has_alias",
                    "structure",
                );
                result.edges.push(alias_edge);
            }
        }
    }
}

/// Simple YAML value type
#[derive(Debug, Clone)]
enum YamlValue {
    String(String),
    List(Vec<String>),
}

#[async_trait]
impl ContentAnalyzer for FrontmatterAnalyzer {
    fn id(&self) -> &str {
        "frontmatter-analyzer"
    }

    fn name(&self) -> &str {
        "Frontmatter Analyzer"
    }

    fn dimensions(&self) -> Vec<&str> {
        vec!["structure", "relational"] // Tags create relational nodes
    }

    fn capabilities(&self) -> Vec<AnalysisCapability> {
        vec![AnalysisCapability::Structure, AnalysisCapability::Entities]
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

    #[test]
    fn test_extract_frontmatter() {
        let content = r#"---
title: Test
tags: [one, two]
---

# Content
"#;
        let fm = FrontmatterAnalyzer::extract_frontmatter(content).unwrap();
        assert!(fm.contains("title: Test"));
        assert!(fm.contains("tags: [one, two]"));
    }

    #[test]
    fn test_extract_frontmatter_none() {
        let content = "# No frontmatter";
        assert!(FrontmatterAnalyzer::extract_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_simple_yaml() {
        let yaml = r#"
title: My Document
author: John Doe
tags: [rust, programming]
count: 42
"#;
        let parsed = FrontmatterAnalyzer::parse_simple_yaml(yaml);

        assert!(matches!(
            parsed.get("title"),
            Some(YamlValue::String(s)) if s == "My Document"
        ));
        assert!(matches!(
            parsed.get("author"),
            Some(YamlValue::String(s)) if s == "John Doe"
        ));
        assert!(matches!(
            parsed.get("tags"),
            Some(YamlValue::List(v)) if v.len() == 2
        ));
        assert!(matches!(
            parsed.get("count"),
            Some(YamlValue::String(s)) if s == "42"
        ));
    }

    #[test]
    fn test_parse_yaml_list() {
        let yaml = r#"
tags:
  - one
  - two
  - three
"#;
        let parsed = FrontmatterAnalyzer::parse_simple_yaml(yaml);
        let tags = parsed.get("tags").unwrap();

        if let YamlValue::List(items) = tags {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], "one");
            assert_eq!(items[1], "two");
            assert_eq!(items[2], "three");
        } else {
            panic!("Expected list");
        }
    }

    #[tokio::test]
    async fn test_analyze_frontmatter() {
        let analyzer = FrontmatterAnalyzer::new();
        let content = r#"---
title: Test Document
author: Jane
tags: [rust, testing]
---

# Content here
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        // Should have frontmatter node
        let fm_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "frontmatter")
            .collect();
        assert_eq!(fm_nodes.len(), 1);

        let fm = fm_nodes[0];
        assert!(matches!(
            fm.properties.get("title"),
            Some(PropertyValue::String(s)) if s == "Test Document"
        ));

        // Should have tag nodes
        let tag_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "tag")
            .collect();
        assert_eq!(tag_nodes.len(), 2);

        // Should have tagged_with edges
        let tagged_edges: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_edges.len(), 2);
    }

    #[tokio::test]
    async fn test_analyze_with_aliases() {
        let analyzer = FrontmatterAnalyzer::new();
        let content = r#"---
title: Main Title
aliases:
  - Alt Name
  - Another Name
---

# Content
"#;

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();

        let alias_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.node_type == "alias")
            .collect();
        assert_eq!(alias_nodes.len(), 2);

        let has_alias_edges: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relationship == "has_alias")
            .collect();
        assert_eq!(has_alias_edges.len(), 2);
    }

    #[tokio::test]
    async fn test_no_frontmatter() {
        let analyzer = FrontmatterAnalyzer::new();
        let content = "# Just a heading\n\nNo frontmatter here.";

        let items = vec![ContentItem::new("test.md", ContentType::Document, content)];
        let scope = AnalysisScope::new(ContextId::from_string("test"), items);

        let result = analyzer.analyze(&scope).await.unwrap();
        assert!(result.nodes.is_empty());
    }
}
