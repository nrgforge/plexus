//! MCP tool parameter structs with schemars-derived JSON schemas.

use schemars::JsonSchema;
use serde::Deserialize;

// ── Chain params ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateChainParams {
    #[schemars(description = "Name of the chain")]
    pub name: String,
    #[schemars(description = "Optional description of the chain")]
    pub description: Option<String>,
}

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
pub struct AddMarkParams {
    #[schemars(description = "The chain this mark belongs to")]
    pub chain_id: String,
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

// ── Context params ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextCreateParams {
    #[schemars(description = "Name of the context")]
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextDeleteParams {
    #[schemars(description = "Name of the context to delete")]
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextAddSourcesParams {
    #[schemars(description = "Name of the context")]
    pub name: String,
    #[schemars(description = "Source paths (files or directories) to add")]
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextRemoveSourcesParams {
    #[schemars(description = "Name of the context")]
    pub name: String,
    #[schemars(description = "Source paths to remove")]
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextListParams {
    #[schemars(description = "Show sources for this context (omit to list all)")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextRenameParams {
    #[schemars(description = "Current context name")]
    pub old_name: String,
    #[schemars(description = "New context name")]
    pub new_name: String,
}
