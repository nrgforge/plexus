//! MCP server for Plexus — knowledge graph engine via the Model Context Protocol.
//!
//! Tools: 16 total (1 session + 1 ingest + 6 context + 7 graph read + 1 spec load).
//!
//! The single graph-data write path is `ingest` (ADR-028), which routes to
//! adapters by input_kind (explicit or auto-classified from JSON shape).
//! All ingests go through the full pipeline enforcing Invariant 7: all
//! knowledge carries both semantic content and provenance. `load_spec`
//! (ADR-037) is a separate surface — it installs a consumer's adapter +
//! lens onto the active context, which is a configuration write rather
//! than a graph-data write.

pub mod params;

use params::*;
use crate::api::PlexusApi;
use crate::adapter::{PipelineBuilder, classify_input};
use crate::graph::{NodeId, Source};
use crate::query::{CursorFilter, Direction, FindQuery, PathQuery, QueryFilter, RankBy, TraverseQuery};
use crate::{OpenStore, PlexusEngine, SqliteStore};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ok_text(text: String) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

fn err_text(msg: String) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(msg)]))
}

// ---------------------------------------------------------------------------
// PlexusMcpServer
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PlexusMcpServer {
    api: PlexusApi,
    active_context: Arc<Mutex<Option<String>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl PlexusMcpServer {
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        Self::with_project_dir(engine, None)
    }

    pub fn with_project_dir(
        engine: Arc<PlexusEngine>,
        project_dir: Option<&std::path::Path>,
    ) -> Self {
        let pipeline = PipelineBuilder::default_pipeline(engine.clone(), project_dir);

        let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

        Self {
            api,
            active_context: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }

    // ── Session ─────────────────────────────────────────────────────────

    fn context(&self) -> Result<String, McpError> {
        self.active_context
            .lock()
            .map_err(|_| McpError {
                code: rmcp::model::ErrorCode::INTERNAL_ERROR,
                message: "active_context mutex poisoned".into(),
                data: None,
            })?
            .clone()
            .ok_or_else(|| McpError {
                code: rmcp::model::ErrorCode::INVALID_REQUEST,
                message: "no context set — call set_context first".into(),
                data: None,
            })
    }

    #[tool(description = "Set the active context for this session (auto-created if it doesn't exist). Must be called before using other tools.")]
    fn set_context(
        &self,
        Parameters(p): Parameters<SetContextParams>,
    ) -> Result<CallToolResult, McpError> {
        // Auto-create if the context doesn't exist
        if self.api.context_list(Some(&p.name)).unwrap_or_default().is_empty() {
            if let Err(e) = self.api.context_create(&p.name) {
                return err_text(format!("failed to create context '{}': {}", p.name, e));
            }
        }
        *self.active_context.lock().map_err(|_| McpError {
            code: rmcp::model::ErrorCode::INTERNAL_ERROR,
            message: "active_context mutex poisoned".into(),
            data: None,
        })? = Some(p.name.clone());
        ok_text(format!("active context set to '{}'", p.name))
    }

    // ── Ingest (single write path — ADR-028, Invariant 7) ────────────────

    #[tool(description = "Ingest data into the knowledge graph. Accepts JSON data and optional input_kind for routing. When input_kind is omitted, auto-detected from data shape: {\"text\": ...} → content, {\"file_path\": ...} → file extraction.")]
    async fn ingest(
        &self,
        Parameters(p): Parameters<IngestParams>,
    ) -> Result<CallToolResult, McpError> {
        let context_id = self.context()?;

        tracing::debug!(context = %context_id, "mcp ingest");

        // Resolve input_kind: explicit or classified from JSON
        let input_kind = match p.input_kind {
            Some(ref kind) => kind.clone(),
            None => classify_input(&p.data)
                .map(|k| k.to_string())
                .map_err(|e| McpError {
                    code: rmcp::model::ErrorCode::INVALID_PARAMS,
                    message: e.to_string().into(),
                    data: None,
                })?,
        };

        match self
            .api
            .ingest(&context_id, &input_kind, Box::new(p.data))
            .await
        {
            Ok(events) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "input_kind": input_kind,
                    "events": events.len(),
                }))
                .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Context management ─────────────────────────────────────────────

    #[tool(description = "List all contexts with their sources")]
    fn context_list(&self) -> Result<CallToolResult, McpError> {
        match self.api.context_list_info() {
            Ok(infos) => {
                let items: Vec<serde_json::Value> = infos
                    .into_iter()
                    .map(|ci| serde_json::json!({
                        "name": ci.name,
                        "id": ci.id,
                        "source_count": ci.sources.len(),
                        "sources": ci.sources,
                    }))
                    .collect();
                ok_text(serde_json::to_string_pretty(&items).unwrap())
            }
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Create a new context")]
    fn context_create(
        &self,
        Parameters(p): Parameters<ContextNameParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.context_create(&p.name) {
            Ok(id) => ok_text(format!("created context '{}' ({})", p.name, id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Delete a context by name")]
    fn context_delete(
        &self,
        Parameters(p): Parameters<ContextNameParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.context_delete(&p.name) {
            Ok(()) => ok_text(format!("deleted context '{}'", p.name)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Rename an existing context")]
    fn context_rename(
        &self,
        Parameters(p): Parameters<ContextRenameParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.context_rename(&p.old_name, &p.new_name) {
            Ok(()) => ok_text(format!("renamed '{}' to '{}'", p.old_name, p.new_name)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Add file or directory sources to a context")]
    fn context_add_sources(
        &self,
        Parameters(p): Parameters<ContextSourceParams>,
    ) -> Result<CallToolResult, McpError> {
        let sources: Vec<Source> = p.paths.iter().map(|path| {
            if std::path::Path::new(path).is_dir() {
                Source::Directory { path: path.clone(), recursive: false }
            } else {
                Source::File { path: path.clone() }
            }
        }).collect();
        match self.api.context_add_sources(&p.name, &sources) {
            Ok(()) => ok_text(format!("added {} source(s) to '{}'", sources.len(), p.name)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Remove file or directory sources from a context")]
    fn context_remove_sources(
        &self,
        Parameters(p): Parameters<ContextSourceParams>,
    ) -> Result<CallToolResult, McpError> {
        let sources: Vec<Source> = p.paths.iter().map(|path| {
            if std::path::Path::new(path).is_dir() {
                Source::Directory { path: path.clone(), recursive: false }
            } else {
                Source::File { path: path.clone() }
            }
        }).collect();
        match self.api.context_remove_sources(&p.name, &sources) {
            Ok(()) => ok_text(format!("removed {} source(s) from '{}'", sources.len(), p.name)),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Graph reads ────────────────────────────────────────────────────

    #[tool(description = "Query the evidence trail for a concept: marks, fragments, and chains (ADR-013). Optional filter fields scope the trail: contributor_ids limits to edges contributed by specified adapters; min_corroboration requires edges to have at least N distinct contributors. relationship_prefix is included for API consistency but typically returns empty results for evidence trails, since evidence-dimension edges (references, contains, tagged_with) do not use lens prefixes.")]
    fn evidence_trail(
        &self,
        Parameters(p): Parameters<EvidenceTrailParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        let filter = composable_filter(
            p.contributor_ids,
            p.relationship_prefix,
            p.min_corroboration,
        );
        match self.api.evidence_trail(&ctx, &p.node_id, filter) {
            Ok(result) => ok_text(serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Query surface (ADR-036 §1) — flat parameter wrappers ───────────

    #[tool(description = "Find nodes in the active context. Optional filters: node_type, dimension, contributor_ids, relationship_prefix, min_corroboration. When a composable filter is specified, a node qualifies only if it has at least one incident edge passing the filter (ADR-034).")]
    fn find_nodes(
        &self,
        Parameters(p): Parameters<FindNodesParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        let query = FindQuery {
            node_type: p.node_type,
            dimension: p.dimension,
            limit: p.limit,
            offset: p.offset,
            filter: composable_filter(
                p.contributor_ids,
                p.relationship_prefix,
                p.min_corroboration,
            ),
            ..Default::default()
        };
        match self.api.find_nodes(&ctx, query) {
            Ok(result) => ok_text(serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Traverse edges from a starting node in the active context. Returns levels of reachable nodes plus the traversed edges. Optional rank_by: \"raw_weight\" or \"corroboration\" (ADR-034).")]
    fn traverse(
        &self,
        Parameters(p): Parameters<TraverseParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        let direction = match parse_direction(p.direction.as_deref()) {
            Ok(d) => d,
            Err(e) => return err_text(e),
        };
        let rank_by = match p.rank_by.as_deref().map(parse_rank_by).transpose() {
            Ok(r) => r,
            Err(e) => return err_text(e),
        };

        let query = TraverseQuery {
            origin: NodeId::from_string(&p.origin),
            max_depth: p.max_depth.unwrap_or(1),
            direction,
            relationship: None,
            min_weight: None,
            filter: composable_filter(
                p.contributor_ids,
                p.relationship_prefix,
                p.min_corroboration,
            ),
        };

        match self.api.traverse(&ctx, query) {
            Ok(mut result) => {
                if let Some(rank) = rank_by {
                    let edges = result.edges.clone();
                    result.rank_by(rank, &edges);
                }
                ok_text(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Find a path between two nodes in the active context. Returns the path if one exists within max_length hops (default 5).")]
    fn find_path(
        &self,
        Parameters(p): Parameters<FindPathParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        let direction = match parse_direction(p.direction.as_deref()) {
            Ok(d) => d,
            Err(e) => return err_text(e),
        };

        let query = PathQuery {
            source: NodeId::from_string(&p.source),
            target: NodeId::from_string(&p.target),
            max_length: p.max_length.unwrap_or(5),
            direction,
            relationship: None,
            filter: composable_filter(
                p.contributor_ids,
                p.relationship_prefix,
                p.min_corroboration,
            ),
        };
        match self.api.find_path(&ctx, query) {
            Ok(result) => ok_text(serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Query persisted graph events in the active context after a cursor (ADR-035). The pull paradigm: consumers walk away, come back, and ask for changes since the last observed sequence. Returns events and the latest_sequence observed.")]
    fn changes_since(
        &self,
        Parameters(p): Parameters<ChangesSinceParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        let filter = if p.event_types.is_some() || p.adapter_id.is_some() || p.limit.is_some() {
            Some(CursorFilter {
                event_types: p.event_types,
                adapter_id: p.adapter_id,
                limit: p.limit,
            })
        } else {
            None
        };
        match self.api.changes_since(&ctx, p.cursor, filter.as_ref()) {
            Ok(result) => ok_text(serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "List all tags associated with nodes (via provenance marks) in the active context.")]
    fn list_tags(&self) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        match self.api.list_tags(&ctx) {
            Ok(tags) => ok_text(serde_json::to_string_pretty(&tags).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Find concept nodes present in both contexts (ADR-017 §4). Returns node IDs in the intersection.")]
    fn shared_concepts(
        &self,
        Parameters(p): Parameters<SharedConceptsParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.shared_concepts(&p.context_a, &p.context_b) {
            Ok(nodes) => ok_text(serde_json::to_string_pretty(&nodes).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Spec loading (ADR-036 §1, ADR-037) ─────────────────────────────

    #[tool(description = "Load a declarative adapter spec (adapter + optional lens + optional enrichments) onto the active context (ADR-037). The spec_yaml argument is the full YAML content sent inline. Validation is upfront: malformed specs fail before any graph work (Invariant 60). On success, returns the adapter ID, lens namespace (if present), and the count of vocabulary edges created by the initial lens sweep over existing content.")]
    async fn load_spec(
        &self,
        Parameters(p): Parameters<LoadSpecParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = self.context()?;
        match self.api.load_spec(&ctx, &p.spec_yaml).await {
            Ok(result) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "adapter_id": result.adapter_id,
                    "lens_namespace": result.lens_namespace,
                    "vocabulary_edges_created": result.vocabulary_edges_created,
                }))
                .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Query parameter parsing helpers
// ---------------------------------------------------------------------------

fn composable_filter(
    contributor_ids: Option<Vec<String>>,
    relationship_prefix: Option<String>,
    min_corroboration: Option<usize>,
) -> Option<QueryFilter> {
    if contributor_ids.is_none() && relationship_prefix.is_none() && min_corroboration.is_none() {
        return None;
    }
    Some(QueryFilter {
        contributor_ids,
        relationship_prefix,
        min_corroboration,
    })
}

fn parse_direction(s: Option<&str>) -> Result<Direction, String> {
    match s.unwrap_or("outgoing") {
        "outgoing" => Ok(Direction::Outgoing),
        "incoming" => Ok(Direction::Incoming),
        "both" => Ok(Direction::Both),
        other => Err(format!(
            "invalid direction '{}' — expected one of: outgoing, incoming, both",
            other
        )),
    }
}

fn parse_rank_by(s: &str) -> Result<RankBy, String> {
    match s {
        "raw_weight" => Ok(RankBy::RawWeight),
        "corroboration" => Ok(RankBy::Corroboration),
        other => Err(format!(
            "invalid rank_by '{}' — expected one of: raw_weight, corroboration",
            other
        )),
    }
}

#[tool_handler]
impl ServerHandler for PlexusMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Plexus MCP server — knowledge graph engine with provenance tracking. Call set_context before using other tools."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run_mcp_server(db_path: PathBuf) -> i32 {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "failed to create tokio runtime");
            return 1;
        }
    };

    rt.block_on(async {
        let engine = {
            let store = match SqliteStore::open(&db_path) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    tracing::error!(path = %db_path.display(), error = %e, "failed to open database");
                    return 1;
                }
            };
            let eng = PlexusEngine::with_store(store);
            if let Err(e) = eng.load_all() {
                tracing::error!(error = %e, "failed to load contexts");
                return 1;
            }
            eng
        };

        let project_dir = db_path.parent().unwrap_or(std::path::Path::new("."));
        let server = PlexusMcpServer::with_project_dir(
            Arc::new(engine),
            Some(project_dir),
        );

        tracing::info!("plexus mcp server starting on stdio...");

        let service = match server.serve(rmcp::transport::stdio()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "failed to start MCP server");
                return 1;
            }
        };

        if let Err(e) = service.waiting().await {
            tracing::error!(error = %e, "MCP server error");
            return 1;
        }

        0
    })
}

// ---------------------------------------------------------------------------
// Tests (WP-E: boundary integration — assert tool handlers delegate to
// PlexusApi with correct parameter mapping and produce well-formed JSON).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::FragmentInput;
    use crate::graph::Context;
    use rmcp::model::RawContent;

    fn server_with_context(name: &str) -> PlexusMcpServer {
        let store = Arc::new(SqliteStore::open_in_memory().expect("in-memory sqlite"));
        let engine = Arc::new(PlexusEngine::with_store(store));
        engine.upsert_context(Context::new(name)).expect("upsert");
        let server = PlexusMcpServer::new(engine);
        *server.active_context.lock().unwrap() = Some(name.to_string());
        server
    }

    fn text_of(result: &CallToolResult) -> String {
        match &result.content[0].raw {
            RawContent::Text(t) => t.text.clone(),
            other => panic!("expected text content, got {:?}", other),
        }
    }

    async fn seed_fragment(server: &PlexusMcpServer, ctx: &str, text: &str, tags: Vec<&str>) {
        server
            .api
            .ingest(
                ctx,
                "content",
                Box::new(FragmentInput::new(
                    text,
                    tags.into_iter().map(|s| s.to_string()).collect(),
                )),
            )
            .await
            .expect("seed ingest");
    }

    #[tokio::test]
    async fn find_nodes_delegates_to_api_and_returns_json() {
        let server = server_with_context("t");
        seed_fragment(&server, "t", "Graphs and structures", vec!["graphs", "structures"]).await;

        let result = server
            .find_nodes(Parameters(FindNodesParams {
                node_type: Some("concept".into()),
                dimension: None,
                contributor_ids: None,
                relationship_prefix: None,
                min_corroboration: None,
                limit: None,
                offset: None,
            }))
            .expect("find_nodes");

        let body = text_of(&result);
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("json parse");
        let nodes = parsed.get("nodes").and_then(|n| n.as_array()).expect("nodes array");
        assert!(!nodes.is_empty(), "find_nodes should return the seeded concepts");
        assert!(parsed.get("total_count").is_some(), "result carries total_count");
    }

    #[tokio::test]
    async fn traverse_delegates_to_api_and_parses_direction() {
        let server = server_with_context("t");
        seed_fragment(&server, "t", "Graphs and structures together", vec!["graphs", "structures"])
            .await;

        let result = server
            .traverse(Parameters(TraverseParams {
                origin: "concept:graphs".into(),
                max_depth: Some(1),
                direction: Some("both".into()),
                rank_by: Some("corroboration".into()),
                contributor_ids: None,
                relationship_prefix: None,
                min_corroboration: None,
            }))
            .expect("traverse");

        let body = text_of(&result);
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("json parse");
        assert_eq!(parsed["origin"], "concept:graphs");
        assert!(parsed.get("levels").is_some(), "result carries levels");
    }

    #[tokio::test]
    async fn traverse_rejects_invalid_direction() {
        let server = server_with_context("t");
        let result = server
            .traverse(Parameters(TraverseParams {
                origin: "concept:x".into(),
                max_depth: Some(1),
                direction: Some("sideways".into()),
                rank_by: None,
                contributor_ids: None,
                relationship_prefix: None,
                min_corroboration: None,
            }))
            .expect("traverse returns ok with error content");
        assert_eq!(result.is_error, Some(true));
        assert!(text_of(&result).contains("invalid direction"));
    }

    #[tokio::test]
    async fn find_path_delegates_to_api() {
        let server = server_with_context("t");
        seed_fragment(&server, "t", "Patterns in nature", vec!["patterns", "nature"]).await;

        let result = server
            .find_path(Parameters(FindPathParams {
                source: "concept:patterns".into(),
                target: "concept:nature".into(),
                max_length: Some(3),
                direction: Some("both".into()),
                contributor_ids: None,
                relationship_prefix: None,
                min_corroboration: None,
            }))
            .expect("find_path");

        let parsed: serde_json::Value =
            serde_json::from_str(&text_of(&result)).expect("json parse");
        assert!(parsed.get("found").is_some(), "result carries found");
    }

    #[tokio::test]
    async fn changes_since_delegates_to_api_with_cursor() {
        let server = server_with_context("t");
        seed_fragment(&server, "t", "Cursor test", vec!["cursor"]).await;

        let result = server
            .changes_since(Parameters(ChangesSinceParams {
                cursor: 0,
                event_types: None,
                adapter_id: None,
                limit: None,
            }))
            .expect("changes_since");
        let parsed: serde_json::Value =
            serde_json::from_str(&text_of(&result)).expect("json parse");
        let events = parsed.get("events").and_then(|v| v.as_array()).expect("events array");
        assert!(!events.is_empty(), "expected events after ingest");
        assert!(
            parsed.get("latest_sequence").and_then(|s| s.as_u64()).unwrap_or(0) > 0,
            "latest_sequence should advance past 0"
        );
    }

    #[tokio::test]
    async fn list_tags_returns_seeded_tags() {
        let server = server_with_context("t");
        seed_fragment(&server, "t", "Tag surface test", vec!["alpha", "beta"]).await;

        let result = server.list_tags().expect("list_tags");
        let tags: Vec<String> =
            serde_json::from_str(&text_of(&result)).expect("json parse");
        assert!(tags.iter().any(|t| t == "alpha"));
        assert!(tags.iter().any(|t| t == "beta"));
    }

    #[tokio::test]
    async fn query_tool_without_active_context_returns_error() {
        // No set_context was called — any tool touching self.context() must error.
        let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
        let engine = Arc::new(PlexusEngine::with_store(store));
        let server = PlexusMcpServer::new(engine);

        let result = server.find_nodes(Parameters(FindNodesParams {
            node_type: None,
            dimension: None,
            contributor_ids: None,
            relationship_prefix: None,
            min_corroboration: None,
            limit: None,
            offset: None,
        }));
        assert!(
            result.is_err(),
            "find_nodes without set_context should return an McpError"
        );
    }

    #[tokio::test]
    async fn shared_concepts_returns_intersection() {
        let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
        let engine = Arc::new(PlexusEngine::with_store(store));
        engine.upsert_context(Context::new("left")).expect("upsert");
        engine.upsert_context(Context::new("right")).expect("upsert");
        let server = PlexusMcpServer::new(engine);

        // Seed overlapping tag "shared" in both contexts via direct api.ingest
        server
            .api
            .ingest(
                "left",
                "content",
                Box::new(FragmentInput::new("left fragment", vec!["shared".into(), "left_only".into()])),
            )
            .await
            .expect("left ingest");
        server
            .api
            .ingest(
                "right",
                "content",
                Box::new(FragmentInput::new("right fragment", vec!["shared".into(), "right_only".into()])),
            )
            .await
            .expect("right ingest");

        let result = server
            .shared_concepts(Parameters(SharedConceptsParams {
                context_a: "left".into(),
                context_b: "right".into(),
            }))
            .expect("shared_concepts");
        let nodes: Vec<String> =
            serde_json::from_str(&text_of(&result)).expect("json parse");
        assert!(
            nodes.iter().any(|n| n == "concept:shared"),
            "expected shared concept in intersection, got: {:?}",
            nodes
        );
    }

    // ── WP-F: load_spec tool (ADR-036 §1, ADR-037) ──────────────────────

    const LOAD_SPEC_YAML: &str = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;

    #[tokio::test]
    async fn load_spec_wires_adapter_onto_active_context() {
        let server = server_with_context("t");

        let result = server
            .load_spec(Parameters(LoadSpecParams {
                spec_yaml: LOAD_SPEC_YAML.into(),
            }))
            .await
            .expect("load_spec");

        let body = text_of(&result);
        let parsed: serde_json::Value =
            serde_json::from_str(&body).expect("json parse");
        assert_eq!(parsed["adapter_id"], "trellis-content");
        assert_eq!(parsed["lens_namespace"], "lens:trellis");
        assert!(
            parsed.get("vocabulary_edges_created").is_some(),
            "result carries vocabulary_edges_created"
        );
    }

    #[tokio::test]
    async fn load_spec_returns_error_for_malformed_yaml() {
        let server = server_with_context("t");

        let result = server
            .load_spec(Parameters(LoadSpecParams {
                spec_yaml: "this is not valid yaml: [[[".into(),
            }))
            .await
            .expect("load_spec returns ok with error content");

        assert_eq!(result.is_error, Some(true));
        let body = text_of(&result);
        assert!(
            body.contains("validation"),
            "error body should mention validation failure, got: {}",
            body
        );
    }

    #[tokio::test]
    async fn evidence_trail_accepts_filter_params() {
        // WP-G.1: verifies that optional filter fields on EvidenceTrailParams
        // flow through the MCP layer into PlexusApi::evidence_trail. Filter
        // semantics are tested at the query layer in query::step::tests; this
        // test only verifies MCP-to-API parameter threading.
        let server = server_with_context("t");
        seed_fragment(&server, "t", "Filter wiring test", vec!["wiring"]).await;

        let result = server
            .evidence_trail(Parameters(EvidenceTrailParams {
                node_id: "concept:wiring".into(),
                contributor_ids: Some(vec!["nonexistent-adapter".into()]),
                relationship_prefix: None,
                min_corroboration: None,
            }))
            .expect("evidence_trail with filter");

        // With a filter for a contributor that doesn't exist, the trail
        // should be empty — proving the filter was actually applied.
        let parsed: serde_json::Value =
            serde_json::from_str(&text_of(&result)).expect("json parse");
        let marks = parsed.get("marks").and_then(|v| v.as_array()).expect("marks array");
        let fragments = parsed
            .get("fragments")
            .and_then(|v| v.as_array())
            .expect("fragments array");
        assert!(
            marks.is_empty(),
            "filter for nonexistent adapter should yield no marks, got: {:?}",
            marks
        );
        assert!(
            fragments.is_empty(),
            "filter for nonexistent adapter should yield no fragments, got: {:?}",
            fragments
        );
    }

    #[tokio::test]
    async fn load_spec_without_active_context_returns_error() {
        let store = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
        let engine = Arc::new(PlexusEngine::with_store(store));
        let server = PlexusMcpServer::new(engine);

        let result = server
            .load_spec(Parameters(LoadSpecParams {
                spec_yaml: LOAD_SPEC_YAML.into(),
            }))
            .await;

        assert!(
            result.is_err(),
            "load_spec without set_context should return an McpError"
        );
    }
}
