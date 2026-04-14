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

// ── Query tools (ADR-036 §1) — flat parameter surface (§2) ─────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindNodesParams {
    #[schemars(description = "Filter by node type (e.g. \"concept\", \"fragment\")")]
    pub node_type: Option<String>,
    #[schemars(description = "Filter by dimension (e.g. \"semantic\", \"structural\")")]
    pub dimension: Option<String>,
    #[schemars(description = "Only include nodes with incident edges from at least one of these contributor IDs")]
    pub contributor_ids: Option<Vec<String>>,
    #[schemars(description = "Only include nodes with incident edges whose relationship starts with this prefix (e.g. \"lens:trellis\")")]
    pub relationship_prefix: Option<String>,
    #[schemars(description = "Only include nodes with incident edges having at least this many distinct contributors")]
    pub min_corroboration: Option<usize>,
    #[schemars(description = "Maximum number of nodes to return")]
    pub limit: Option<usize>,
    #[schemars(description = "Number of nodes to skip (pagination offset)")]
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TraverseParams {
    #[schemars(description = "Starting node ID")]
    pub origin: String,
    #[schemars(description = "Maximum depth to traverse (0 = origin only, 1 = immediate neighbors, etc.). Defaults to 1.")]
    pub max_depth: Option<usize>,
    #[schemars(description = "Direction to follow edges: \"outgoing\" (default), \"incoming\", or \"both\"")]
    pub direction: Option<String>,
    #[schemars(description = "Post-ranking: \"raw_weight\" or \"corroboration\". Default is insertion order.")]
    pub rank_by: Option<String>,
    #[schemars(description = "Only traverse edges with a contribution from at least one of these contributor IDs")]
    pub contributor_ids: Option<Vec<String>>,
    #[schemars(description = "Only traverse edges whose relationship starts with this prefix")]
    pub relationship_prefix: Option<String>,
    #[schemars(description = "Only traverse edges having at least this many distinct contributors")]
    pub min_corroboration: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindPathParams {
    #[schemars(description = "Source node ID")]
    pub source: String,
    #[schemars(description = "Target node ID")]
    pub target: String,
    #[schemars(description = "Maximum path length to search. Defaults to 5.")]
    pub max_length: Option<usize>,
    #[schemars(description = "Direction to follow edges: \"outgoing\" (default), \"incoming\", or \"both\"")]
    pub direction: Option<String>,
    #[schemars(description = "Only consider edges with a contribution from at least one of these contributor IDs")]
    pub contributor_ids: Option<Vec<String>>,
    #[schemars(description = "Only consider edges whose relationship starts with this prefix")]
    pub relationship_prefix: Option<String>,
    #[schemars(description = "Only consider edges having at least this many distinct contributors")]
    pub min_corroboration: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ChangesSinceParams {
    #[schemars(description = "Cursor (last observed sequence number). Use 0 to get all events from the beginning.")]
    pub cursor: u64,
    #[schemars(description = "Filter by event types (e.g. [\"NodesAdded\", \"EdgesAdded\"])")]
    pub event_types: Option<Vec<String>>,
    #[schemars(description = "Filter by adapter or enrichment ID")]
    pub adapter_id: Option<String>,
    #[schemars(description = "Maximum number of events to return")]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SharedConceptsParams {
    #[schemars(description = "Name of the first context")]
    pub context_a: String,
    #[schemars(description = "Name of the second context")]
    pub context_b: String,
}
