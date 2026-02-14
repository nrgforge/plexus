//! MCP tool parameter structs with schemars-derived JSON schemas.

use schemars::JsonSchema;
use serde::Deserialize;

// ── Chain params ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListChainsParams {
    #[schemars(description = "Filter by status: 'active' or 'archived'")]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChainIdParams {
    #[schemars(description = "The chain ID")]
    pub chain_id: String,
}

// ── Mark params ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnnotateParams {
    #[schemars(description = "Name of the chain (auto-created if it doesn't exist)")]
    pub chain_name: String,
    #[schemars(description = "Path to the file or artifact")]
    pub file: String,
    #[schemars(description = "Line number (1-indexed)")]
    pub line: u32,
    #[schemars(description = "Description of why this location is significant")]
    pub annotation: String,
    #[schemars(description = "Column number (optional)")]
    pub column: Option<u32>,
    #[schemars(description = "Freeform type label (user-defined ontology)")]
    pub r#type: Option<String>,
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMarkParams {
    #[schemars(description = "The mark ID to update")]
    pub mark_id: String,
    pub annotation: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    #[schemars(description = "Freeform type label (user-defined ontology)")]
    pub r#type: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkIdParams {
    #[schemars(description = "The mark ID")]
    pub mark_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListMarksParams {
    #[schemars(description = "Filter by chain")]
    pub chain_id: Option<String>,
    #[schemars(description = "Filter by file path")]
    pub file: Option<String>,
    #[schemars(description = "Filter by type label")]
    pub r#type: Option<String>,
    #[schemars(description = "Filter by tag")]
    pub tag: Option<String>,
}

// ── Link params ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkMarksParams {
    #[schemars(description = "The source mark ID")]
    pub source_id: String,
    #[schemars(description = "The target mark ID")]
    pub target_id: String,
}

// ── Session params ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetContextParams {
    #[schemars(description = "Name of the context to activate (auto-created if it doesn't exist)")]
    pub name: String,
}

// ── Graph read params ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvidenceTrailParams {
    #[schemars(description = "The node ID to query evidence for (e.g. a concept ID)")]
    pub node_id: String,
}

