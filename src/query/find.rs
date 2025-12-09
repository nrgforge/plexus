//! Find queries for locating nodes

use crate::graph::{Context, ContentType, Node, PropertyValue};
use super::types::QueryResult;

/// Query for finding nodes by various criteria
#[derive(Debug, Clone, Default)]
pub struct FindQuery {
    /// Filter by node type (e.g., "function", "class")
    pub node_type: Option<String>,
    /// Filter by content type
    pub content_type: Option<ContentType>,
    /// Filter by dimension (e.g., "structure", "semantic")
    pub dimension: Option<String>,
    /// Filter by property key existence
    pub has_property: Option<String>,
    /// Filter by property key-value match
    pub property_equals: Option<(String, PropertyValue)>,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Number of results to skip
    pub offset: Option<usize>,
}

impl FindQuery {
    /// Create a new empty query (matches all nodes)
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by node type
    pub fn with_node_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = Some(node_type.into());
        self
    }

    /// Filter by content type
    pub fn with_content_type(mut self, content_type: ContentType) -> Self {
        self.content_type = Some(content_type);
        self
    }

    /// Filter by dimension (e.g., "structure", "semantic")
    pub fn with_dimension(mut self, dimension: impl Into<String>) -> Self {
        self.dimension = Some(dimension.into());
        self
    }

    /// Filter by property existence
    pub fn with_property(mut self, key: impl Into<String>) -> Self {
        self.has_property = Some(key.into());
        self
    }

    /// Filter by property value
    pub fn with_property_value(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.property_equals = Some((key.into(), value));
        self
    }

    /// Limit results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip results (for pagination)
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Execute the query against a context
    pub fn execute(&self, context: &Context) -> QueryResult {
        let mut nodes: Vec<Node> = context
            .nodes
            .values()
            .filter(|node| self.matches(node))
            .cloned()
            .collect();

        let total_count = nodes.len();

        // Apply offset
        if let Some(offset) = self.offset {
            if offset < nodes.len() {
                nodes = nodes.into_iter().skip(offset).collect();
            } else {
                nodes.clear();
            }
        }

        // Apply limit
        if let Some(limit) = self.limit {
            nodes.truncate(limit);
        }

        QueryResult { nodes, total_count }
    }

    /// Check if a node matches all query criteria
    fn matches(&self, node: &Node) -> bool {
        // Check node type
        if let Some(ref expected_type) = self.node_type {
            if &node.node_type != expected_type {
                return false;
            }
        }

        // Check content type
        if let Some(expected_content) = self.content_type {
            if node.content_type != expected_content {
                return false;
            }
        }

        // Check dimension
        if let Some(ref expected_dimension) = self.dimension {
            if &node.dimension != expected_dimension {
                return false;
            }
        }

        // Check property existence
        if let Some(ref key) = self.has_property {
            if !node.properties.contains_key(key) {
                return false;
            }
        }

        // Check property value
        if let Some((ref key, ref expected_value)) = self.property_equals {
            match node.properties.get(key) {
                Some(value) if value == expected_value => {}
                _ => return false,
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Node;

    fn create_test_context() -> Context {
        let mut ctx = Context::new("test");

        // Add some nodes
        let mut node1 = Node::new("function", ContentType::Code);
        node1.properties.insert("language".into(), PropertyValue::String("rust".into()));

        let mut node2 = Node::new("function", ContentType::Code);
        node2.properties.insert("language".into(), PropertyValue::String("python".into()));

        let node3 = Node::new("class", ContentType::Code);
        let node4 = Node::new("concept", ContentType::Concept);

        ctx.add_node(node1);
        ctx.add_node(node2);
        ctx.add_node(node3);
        ctx.add_node(node4);

        ctx
    }

    #[test]
    fn test_find_all() {
        let ctx = create_test_context();
        let result = FindQuery::new().execute(&ctx);
        assert_eq!(result.nodes.len(), 4);
        assert_eq!(result.total_count, 4);
    }

    #[test]
    fn test_find_by_node_type() {
        let ctx = create_test_context();
        let result = FindQuery::new()
            .with_node_type("function")
            .execute(&ctx);
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn test_find_by_content_type() {
        let ctx = create_test_context();
        let result = FindQuery::new()
            .with_content_type(ContentType::Concept)
            .execute(&ctx);
        assert_eq!(result.nodes.len(), 1);
    }

    #[test]
    fn test_find_by_property_value() {
        let ctx = create_test_context();
        let result = FindQuery::new()
            .with_property_value("language", PropertyValue::String("rust".into()))
            .execute(&ctx);
        assert_eq!(result.nodes.len(), 1);
    }

    #[test]
    fn test_find_with_limit() {
        let ctx = create_test_context();
        let result = FindQuery::new()
            .limit(2)
            .execute(&ctx);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.total_count, 4); // Total is still 4
    }

    #[test]
    fn test_find_with_offset_and_limit() {
        let ctx = create_test_context();
        let result = FindQuery::new()
            .offset(1)
            .limit(2)
            .execute(&ctx);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.total_count, 4);
    }

    #[test]
    fn test_find_combined_filters() {
        let ctx = create_test_context();
        let result = FindQuery::new()
            .with_node_type("function")
            .with_content_type(ContentType::Code)
            .execute(&ctx);
        assert_eq!(result.nodes.len(), 2);
    }
}
