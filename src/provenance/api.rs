//! High-level provenance API wrapping PlexusEngine graph operations.
//!
//! Maps the chain/mark/link provenance model onto nodes and edges
//! in the `"provenance"` dimension.

use chrono::Utc;
use std::collections::HashSet;

use crate::{
    dimension, ContentType, Context, ContextId, Edge, Node, NodeId, PlexusEngine, PlexusError,
    PlexusResult, PropertyValue,
};

use super::types::{ChainStatus, ChainView, MarkView};

/// High-level provenance API scoped to a single context.
pub struct ProvenanceApi<'a> {
    engine: &'a PlexusEngine,
    context_id: ContextId,
}

impl<'a> ProvenanceApi<'a> {
    /// Create a new ProvenanceApi for the given context.
    pub fn new(engine: &'a PlexusEngine, context_id: ContextId) -> Self {
        Self { engine, context_id }
    }

    // === Chain operations ===

    /// Create a new provenance chain.
    pub fn create_chain(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> PlexusResult<String> {
        let mut node = Node::new_in_dimension("chain", ContentType::Provenance, dimension::PROVENANCE);
        node.properties.insert("name".into(), PropertyValue::String(name.into()));
        if let Some(desc) = description {
            node.properties.insert("description".into(), PropertyValue::String(desc.into()));
        }
        node.properties.insert(
            "status".into(),
            PropertyValue::String("active".into()),
        );

        let id = self.engine.add_node(&self.context_id, node)?;
        Ok(id.to_string())
    }

    /// List chains, optionally filtered by status.
    pub fn list_chains(&self, status: Option<&str>) -> PlexusResult<Vec<ChainView>> {
        let filter: Option<ChainStatus> = match status {
            Some(s) => Some(s.parse().map_err(|e: String| PlexusError::NodeNotFound(e))?),
            None => None,
        };

        let context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let chains: Vec<ChainView> = context.nodes()
            .filter(|n| n.node_type == "chain" && n.dimension == dimension::PROVENANCE)
            .filter(|n| {
                match &filter {
                    Some(s) => {
                        let node_status = n.properties.get("status")
                            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
                            .unwrap_or("active");
                        let target = match s {
                            ChainStatus::Active => "active",
                            ChainStatus::Archived => "archived",
                        };
                        node_status == target
                    }
                    None => true,
                }
            })
            .map(|n| node_to_chain_view(n))
            .collect();

        Ok(chains)
    }

    /// Get a chain and its marks.
    pub fn get_chain(&self, chain_id: &str) -> PlexusResult<(ChainView, Vec<MarkView>)> {
        let context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let chain_node_id = NodeId::from(chain_id);
        let chain_node = context.get_node(&chain_node_id)
            .ok_or_else(|| PlexusError::NodeNotFound(chain_id.into()))?;

        let chain_view = node_to_chain_view(chain_node);

        // Find marks connected to this chain via "contains" edges
        let mark_ids: Vec<NodeId> = context.edges()
            .filter(|e| e.source == chain_node_id && e.relationship == "contains")
            .map(|e| e.target.clone())
            .collect();

        let marks: Vec<MarkView> = mark_ids.iter()
            .filter_map(|mid| context.get_node(mid))
            .map(|n| node_to_mark_view(n, &context))
            .collect();

        Ok((chain_view, marks))
    }

    /// Archive a chain.
    pub fn archive_chain(&self, chain_id: &str) -> PlexusResult<()> {
        self.set_chain_status(chain_id, "archived")
    }

    // === Mark operations ===

    /// Add a mark to a chain.
    #[allow(clippy::too_many_arguments)]
    pub fn add_mark(
        &self,
        chain_id: &str,
        file: &str,
        line: u32,
        annotation: &str,
        column: Option<u32>,
        mark_type: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> PlexusResult<String> {
        // Verify chain exists
        let context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;
        let chain_node_id = NodeId::from(chain_id);
        if context.get_node(&chain_node_id).is_none() {
            return Err(PlexusError::NodeNotFound(format!("chain not found: {}", chain_id)));
        }
        drop(context);

        let mut node = Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
        node.properties.insert("chain_id".into(), PropertyValue::String(chain_id.into()));
        node.properties.insert("file".into(), PropertyValue::String(file.into()));
        node.properties.insert("line".into(), PropertyValue::Int(line as i64));
        node.properties.insert("annotation".into(), PropertyValue::String(annotation.into()));
        if let Some(col) = column {
            node.properties.insert("column".into(), PropertyValue::Int(col as i64));
        }
        if let Some(t) = mark_type {
            node.properties.insert("type".into(), PropertyValue::String(t.into()));
        }
        if let Some(ref t) = tags {
            let tag_vals: Vec<PropertyValue> = t.iter()
                .map(|s| PropertyValue::String(s.clone()))
                .collect();
            node.properties.insert("tags".into(), PropertyValue::Array(tag_vals));
        }

        let mark_id = self.engine.add_node(&self.context_id, node)?;

        // Create "contains" edge from chain to mark
        let edge = Edge::new_in_dimension(
            chain_node_id,
            mark_id.clone(),
            "contains",
            dimension::PROVENANCE,
        );
        self.engine.add_edge(&self.context_id, edge)?;

        // Note: Tag-to-concept bridging (ADR-009) is handled by TagConceptBridger
        // enrichment when marks are created through the IngestPipeline.

        Ok(mark_id.to_string())
    }

    /// Update a mark's fields.
    pub fn update_mark(
        &self,
        mark_id: &str,
        annotation: Option<&str>,
        line: Option<u32>,
        column: Option<u32>,
        mark_type: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> PlexusResult<()> {
        let mut context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let node_id = NodeId::from(mark_id);
        let node = context.get_node_mut(&node_id)
            .ok_or_else(|| PlexusError::NodeNotFound(mark_id.into()))?;

        if let Some(a) = annotation {
            node.properties.insert("annotation".into(), PropertyValue::String(a.into()));
        }
        if let Some(l) = line {
            node.properties.insert("line".into(), PropertyValue::Int(l as i64));
        }
        if let Some(col) = column {
            node.properties.insert("column".into(), PropertyValue::Int(col as i64));
        }
        if let Some(t) = mark_type {
            node.properties.insert("type".into(), PropertyValue::String(t.into()));
        }
        if let Some(t) = tags {
            let tag_vals: Vec<PropertyValue> = t.iter()
                .map(|s| PropertyValue::String(s.clone()))
                .collect();
            node.properties.insert("tags".into(), PropertyValue::Array(tag_vals));
        }

        node.metadata.modified_at = Some(Utc::now());
        self.engine.upsert_context(context)?;
        Ok(())
    }

    /// List marks with optional filters.
    pub fn list_marks(
        &self,
        chain_id: Option<&str>,
        file: Option<&str>,
        mark_type: Option<&str>,
        tag: Option<&str>,
    ) -> PlexusResult<Vec<MarkView>> {
        let context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let marks: Vec<MarkView> = context.nodes()
            .filter(|n| n.node_type == "mark" && n.dimension == dimension::PROVENANCE)
            .filter(|n| {
                let cid_match = chain_id.map_or(true, |cid| {
                    prop_str(&n.properties, "chain_id") == Some(cid)
                });
                let file_match = file.map_or(true, |f| {
                    prop_str(&n.properties, "file") == Some(f)
                });
                let type_match = mark_type.map_or(true, |t| {
                    prop_str(&n.properties, "type") == Some(t)
                });
                let tag_match = tag.map_or(true, |tg| {
                    prop_tags(&n.properties).iter().any(|t| t == tg)
                });
                cid_match && file_match && type_match && tag_match
            })
            .map(|n| node_to_mark_view(n, &context))
            .collect();

        Ok(marks)
    }

    // === Link operations ===

    /// Get incoming and outgoing links for a mark.
    pub fn get_links(&self, mark_id: &str) -> PlexusResult<(Vec<String>, Vec<String>)> {
        let context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let node_id = NodeId::from(mark_id);
        if context.get_node(&node_id).is_none() {
            return Err(PlexusError::NodeNotFound(mark_id.into()));
        }

        let outgoing: Vec<String> = context.edges()
            .filter(|e| e.source == node_id && e.relationship == "links_to")
            .map(|e| e.target.to_string())
            .collect();

        let incoming: Vec<String> = context.edges()
            .filter(|e| e.target == node_id && e.relationship == "links_to")
            .map(|e| e.source.to_string())
            .collect();

        Ok((outgoing, incoming))
    }

    /// List all unique tags used across all marks.
    pub fn list_tags(&self) -> PlexusResult<Vec<String>> {
        let context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let mut tags: Vec<String> = context.nodes()
            .filter(|n| n.node_type == "mark" && n.dimension == dimension::PROVENANCE)
            .flat_map(|n| prop_tags(&n.properties))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        tags.sort();
        Ok(tags)
    }

    // === Internal helpers ===

    fn set_chain_status(&self, chain_id: &str, status: &str) -> PlexusResult<()> {
        let mut context = self.engine.get_context(&self.context_id)
            .ok_or_else(|| PlexusError::ContextNotFound(self.context_id.clone()))?;

        let node_id = NodeId::from(chain_id);
        let node = context.get_node_mut(&node_id)
            .ok_or_else(|| PlexusError::NodeNotFound(chain_id.into()))?;

        node.properties.insert("status".into(), PropertyValue::String(status.into()));
        node.metadata.modified_at = Some(Utc::now());
        self.engine.upsert_context(context)?;
        Ok(())
    }
}

// === Free helper functions ===

fn prop_str<'a>(props: &'a std::collections::HashMap<String, PropertyValue>, key: &str) -> Option<&'a str> {
    match props.get(key) {
        Some(PropertyValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn prop_int(props: &std::collections::HashMap<String, PropertyValue>, key: &str) -> Option<i64> {
    match props.get(key) {
        Some(PropertyValue::Int(n)) => Some(*n),
        _ => None,
    }
}

fn prop_tags(props: &std::collections::HashMap<String, PropertyValue>) -> Vec<String> {
    match props.get("tags") {
        Some(PropertyValue::Array(arr)) => arr.iter()
            .filter_map(|v| if let PropertyValue::String(s) = v { Some(s.clone()) } else { None })
            .collect(),
        _ => vec![],
    }
}

fn node_to_chain_view(n: &Node) -> ChainView {
    let status_str = prop_str(&n.properties, "status").unwrap_or("active");
    let status = match status_str {
        "archived" => ChainStatus::Archived,
        _ => ChainStatus::Active,
    };
    ChainView {
        id: n.id.to_string(),
        name: prop_str(&n.properties, "name").unwrap_or("").to_string(),
        description: prop_str(&n.properties, "description").map(|s| s.to_string()),
        status,
        created_at: n.metadata.created_at.unwrap_or_else(Utc::now),
    }
}

fn node_to_mark_view(n: &Node, context: &Context) -> MarkView {
    let node_id = &n.id;

    // Collect outgoing "links_to" targets
    let links: Vec<String> = context.edges()
        .filter(|e| &e.source == node_id && e.relationship == "links_to")
        .map(|e| e.target.to_string())
        .collect();

    MarkView {
        id: n.id.to_string(),
        chain_id: prop_str(&n.properties, "chain_id").unwrap_or("").to_string(),
        file: prop_str(&n.properties, "file").unwrap_or("").to_string(),
        line: prop_int(&n.properties, "line").unwrap_or(0) as u32,
        column: prop_int(&n.properties, "column").map(|v| v as u32),
        annotation: prop_str(&n.properties, "annotation").unwrap_or("").to_string(),
        r#type: prop_str(&n.properties, "type").map(|s| s.to_string()),
        tags: prop_tags(&n.properties),
        links,
        created_at: n.metadata.created_at.unwrap_or_else(Utc::now),
    }
}

// ================================================================
// ADR-008: Project-Scoped Provenance Tests
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_engine_with_context(name: &str) -> (PlexusEngine, ContextId) {
        let engine = PlexusEngine::new();
        let ctx_id = ContextId::from(name);
        engine.upsert_context(Context::with_id(ctx_id.clone(), name)).unwrap();
        (engine, ctx_id)
    }

    // === Scenario: Mark is created in a project context ===
    #[test]
    fn mark_created_in_project_context() {
        let (engine, ctx_id) = setup_engine_with_context("provence-research");
        let api = ProvenanceApi::new(&engine, ctx_id.clone());

        let chain_id = api.create_chain("reading-notes", None).unwrap();
        let mark_id = api.add_mark(
            &chain_id, "notes.md", 42, "walking through Avignon",
            None, None, None,
        ).unwrap();

        // Mark node exists in context with provenance dimension
        let ctx = engine.get_context(&ctx_id).unwrap();
        let mark_node = ctx.get_node(&NodeId::from(mark_id.as_str())).unwrap();
        assert_eq!(mark_node.dimension, dimension::PROVENANCE);
        assert_eq!(mark_node.node_type, "mark");

        // "contains" edge from chain to mark
        let contains = ctx.edges().find(|e| {
            e.source == NodeId::from(chain_id.as_str())
                && e.target == NodeId::from(mark_id.as_str())
                && e.relationship == "contains"
        });
        assert!(contains.is_some(), "chain should have 'contains' edge to mark");
    }

    // === Scenario: Mark creation without a context fails ===
    #[test]
    fn mark_creation_without_context_fails() {
        let engine = PlexusEngine::new();
        // No contexts created — use a non-existent context
        let api = ProvenanceApi::new(&engine, ContextId::from("nonexistent"));

        let result = api.create_chain("reading-notes", None);
        assert!(result.is_err(), "creating chain in non-existent context should fail");
    }

    // === Scenario: No __provenance__ context is auto-created ===
    #[test]
    fn no_provenance_context_auto_created() {
        let engine = PlexusEngine::new();
        assert_eq!(engine.context_count(), 0);

        // list_contexts returns nothing — no __provenance__ auto-created
        let contexts = engine.list_contexts();
        assert!(contexts.is_empty());
        assert!(!contexts.iter().any(|c| c.as_str() == "__provenance__"));
    }

    // Note: Tag-to-concept bridging (ADR-009) is now handled by
    // TagConceptBridger enrichment in the ingest pipeline. See
    // tag_bridger.rs unit tests and integration_tests.rs for coverage.

    // === Scenario: Chains are scoped to their context ===
    #[test]
    fn chains_scoped_to_context() {
        let engine = PlexusEngine::new();
        let ctx1 = ContextId::from("provence-research");
        let ctx2 = ContextId::from("desk");
        engine.upsert_context(Context::with_id(ctx1.clone(), "provence-research")).unwrap();
        engine.upsert_context(Context::with_id(ctx2.clone(), "desk")).unwrap();

        let api1 = ProvenanceApi::new(&engine, ctx1.clone());
        let api2 = ProvenanceApi::new(&engine, ctx2.clone());

        api1.create_chain("reading-notes", None).unwrap();
        api2.create_chain("desk-notes", None).unwrap();

        // Listing chains in provence-research returns only reading-notes
        let chains1 = api1.list_chains(None).unwrap();
        assert_eq!(chains1.len(), 1);
        assert_eq!(chains1[0].name, "reading-notes");

        // Listing chains in desk returns only desk-notes
        let chains2 = api2.list_chains(None).unwrap();
        assert_eq!(chains2.len(), 1);
        assert_eq!(chains2[0].name, "desk-notes");
    }
}
