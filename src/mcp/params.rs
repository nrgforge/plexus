//! MCP tool parameter structs with schemars-derived JSON schemas.

use schemars::JsonSchema;
use serde::Deserialize;

// ── Ingest params (ADR-028) ────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IngestParams {
    #[schemars(description = "Input data as a JSON object. For content: {\"text\": \"...\", \"tags\": [...], \"source\": \"...\"}. For file extraction: {\"file_path\": \"...\"}. For annotations: include \"chain_name\", \"file\", \"line\".")]
    pub data: serde_json::Value,
    #[schemars(description = "Optional input kind for direct routing (e.g. \"content\", \"extract-file\"). When omitted, auto-detected from data shape.")]
    pub input_kind: Option<String>,
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
