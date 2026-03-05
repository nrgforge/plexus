//! Read-only provenance API wrapping PlexusEngine graph operations.
//!
//! Maps the chain/mark/link provenance model onto nodes and edges
//! in the `"provenance"` dimension. All writes go through
//! `ProvenanceAdapter` via the ingest pipeline.

use chrono::Utc;
use std::collections::HashSet;

use crate::{
    dimension, Context, ContextId, Node, NodeId, PlexusEngine, PlexusError,
    PlexusResult, PropertyValue,
};

use super::types::{ChainStatus, ChainView, MarkView};

/// Read-only provenance API scoped to a single context.
///
/// All provenance writes (create, update, archive, delete) route through
/// `ProvenanceAdapter` via the ingest pipeline.
pub struct ProvenanceApi<'a> {
    engine: &'a PlexusEngine,
    context_id: ContextId,
}

impl<'a> ProvenanceApi<'a> {
    /// Create a new ProvenanceApi for the given context.
    pub fn new(engine: &'a PlexusEngine, context_id: ContextId) -> Self {
        Self { engine, context_id }
    }

    // === Chain reads ===

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

    // === Mark reads ===

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

// Tests for provenance reads are covered by:
// - api.rs tests (PlexusApi delegates to ProvenanceApi reads)
// - provenance_adapter.rs tests (write operations via pipeline)
// - integration_tests.rs (end-to-end provenance workflows)
