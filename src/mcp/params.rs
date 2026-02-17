//! MCP tool parameter structs with schemars-derived JSON schemas.

use schemars::JsonSchema;
use serde::Deserialize;

// ── Annotate params ────────────────────────────────────────────────────

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

// ── Session params ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetContextParams {
    #[schemars(description = "Name of the context to activate (auto-created if it doesn't exist)")]
    pub name: String,
}

// ── Context management params ──────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextNameParams {
    #[schemars(description = "Name of the context")]
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextRenameParams {
    #[schemars(description = "Current context name")]
    pub old_name: String,
    #[schemars(description = "New context name")]
    pub new_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextSourceParams {
    #[schemars(description = "Name of the context")]
    pub name: String,
    #[schemars(description = "Source paths (files or directories) to add or remove")]
    pub paths: Vec<String>,
}

// ── Graph read params ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvidenceTrailParams {
    #[schemars(description = "The node ID to query evidence for (e.g. a concept ID)")]
    pub node_id: String,
}
