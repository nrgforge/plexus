//! MCP server for Plexus — knowledge graph engine via the Model Context Protocol.
//!
//! Tools: 8 total (1 write + 6 context + 1 graph read).
//!
//! The single write path is `ingest` (ADR-028), which routes to adapters by
//! input_kind (explicit or auto-classified from JSON shape). All writes go
//! through the full pipeline enforcing Invariant 7: all knowledge carries
//! both semantic content and provenance.

pub mod params;

use params::*;
use crate::api::PlexusApi;
use crate::adapter::{
    CoOccurrenceEnrichment, ContentAdapter, IngestPipeline,
    ProvenanceAdapter, TagConceptBridger, classify_input,
};
use crate::graph::Source;
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
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(Arc::new(ContentAdapter::new("annotate")));
        pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            vec![
                Arc::new(TagConceptBridger::new()),
                Arc::new(CoOccurrenceEnrichment::new()),
            ],
        );

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
            .unwrap()
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
        *self.active_context.lock().unwrap() = Some(p.name.clone());
        ok_text(format!("active context set to '{}'", p.name))
    }

    // ── Ingest (single write path — ADR-028, Invariant 7) ────────────────

    #[tool(description = "Ingest data into the knowledge graph. Accepts JSON data and optional input_kind for routing. When input_kind is omitted, auto-detected from data shape: {\"text\": ...} → content, {\"file_path\": ...} → file extraction.")]
    async fn ingest(
        &self,
        Parameters(p): Parameters<IngestParams>,
    ) -> Result<CallToolResult, McpError> {
        let context_id = self.context()?;

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

    #[tool(description = "Query the evidence trail for a concept: marks, fragments, and chains (ADR-013)")]
    fn evidence_trail(
        &self,
        Parameters(p): Parameters<EvidenceTrailParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.evidence_trail(&self.context()?, &p.node_id) {
            Ok(result) => ok_text(serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
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
            eprintln!("failed to create tokio runtime: {}", e);
            return 1;
        }
    };

    rt.block_on(async {
        let engine = {
            let store = match SqliteStore::open(&db_path) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    eprintln!("failed to open database at {}: {}", db_path.display(), e);
                    return 1;
                }
            };
            let eng = PlexusEngine::with_store(store);
            if let Err(e) = eng.load_all() {
                eprintln!("failed to load contexts: {}", e);
                return 1;
            }
            eng
        };

        let server = PlexusMcpServer::new(Arc::new(engine));

        eprintln!("plexus mcp server starting on stdio...");

        let service = match server.serve(rmcp::transport::stdio()).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("failed to start MCP server: {}", e);
                return 1;
            }
        };

        if let Err(e) = service.waiting().await {
            eprintln!("MCP server error: {}", e);
            return 1;
        }

        0
    })
}
