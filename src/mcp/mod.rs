//! MCP server for Plexus — provenance tracking via the Model Context Protocol.
//!
//! Tools: 14 total (13 provenance + 1 graph read).

pub mod params;

use params::*;
use crate::api::PlexusApi;
use crate::adapter::{
    CoOccurrenceEnrichment, IngestPipeline,
    ProvenanceAdapter, TagConceptBridger,
};
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

    // ── Chain tools ─────────────────────────────────────────────────────

    #[tool(description = "List all chains, optionally filtered by status")]
    fn list_chains(
        &self,
        Parameters(p): Parameters<ListChainsParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.list_chains(&self.context()?, p.status.as_deref()) {
            Ok(chains) => ok_text(serde_json::to_string_pretty(&chains).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Get a chain with all its marks")]
    fn get_chain(
        &self,
        Parameters(p): Parameters<ChainIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.get_chain(&self.context()?, &p.chain_id) {
            Ok((chain, marks)) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "chain": chain,
                    "marks": marks,
                }))
                .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Archive a chain (mark as no longer active)")]
    fn archive_chain(
        &self,
        Parameters(p): Parameters<ChainIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.archive_chain(&self.context()?, &p.chain_id) {
            Ok(()) => ok_text(format!("archived chain {}", p.chain_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Delete a chain and all its marks")]
    async fn delete_chain(
        &self,
        Parameters(p): Parameters<ChainIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.delete_chain(&self.context()?, &p.chain_id).await {
            Ok(()) => ok_text(format!("deleted chain {} and its marks", p.chain_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Annotate (replaces create_chain + add_mark) ─────────────────────

    #[tool(description = "Add an annotated mark to a location in a file or artifact")]
    async fn annotate(
        &self,
        Parameters(p): Parameters<AnnotateParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .api
            .annotate(
                &self.context()?,
                &p.chain_name,
                &p.file,
                p.line,
                &p.annotation,
                p.column,
                p.r#type.as_deref(),
                p.tags,
            )
            .await
        {
            Ok(_events) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "chain": p.chain_name,
                    "file": p.file,
                    "line": p.line,
                }))
                .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Mark tools ──────────────────────────────────────────────────────

    #[tool(description = "Update an existing mark")]
    fn update_mark(
        &self,
        Parameters(p): Parameters<UpdateMarkParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.update_mark(
            &self.context()?,
            &p.mark_id,
            p.annotation.as_deref(),
            p.line,
            p.column,
            p.r#type.as_deref(),
            p.tags,
        ) {
            Ok(()) => ok_text(format!("updated mark {}", p.mark_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Delete a mark")]
    async fn delete_mark(
        &self,
        Parameters(p): Parameters<MarkIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.delete_mark(&self.context()?, &p.mark_id).await {
            Ok(()) => ok_text(format!("deleted mark {}", p.mark_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "List marks with optional filters")]
    fn list_marks(
        &self,
        Parameters(p): Parameters<ListMarksParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.list_marks(
            &self.context()?,
            p.chain_id.as_deref(),
            p.file.as_deref(),
            p.r#type.as_deref(),
            p.tag.as_deref(),
        ) {
            Ok(marks) => ok_text(serde_json::to_string_pretty(&marks).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Link tools ──────────────────────────────────────────────────────

    #[tool(description = "Create a link from one mark to another")]
    async fn link_marks(
        &self,
        Parameters(p): Parameters<LinkMarksParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .api
            .link_marks(&self.context()?, &p.source_id, &p.target_id)
            .await
        {
            Ok(()) => ok_text(format!("linked {} -> {}", p.source_id, p.target_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Remove a link between two marks")]
    async fn unlink_marks(
        &self,
        Parameters(p): Parameters<LinkMarksParams>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .api
            .unlink_marks(&self.context()?, &p.source_id, &p.target_id)
            .await
        {
            Ok(()) => ok_text(format!("unlinked {} -> {}", p.source_id, p.target_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Get all links to and from a mark (incoming and outgoing)")]
    fn get_links(
        &self,
        Parameters(p): Parameters<MarkIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.api.get_links(&self.context()?, &p.mark_id) {
            Ok((outgoing, incoming)) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "outgoing": outgoing,
                    "incoming": incoming,
                }))
                .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "List all unique tags used across all marks")]
    fn list_tags(&self) -> Result<CallToolResult, McpError> {
        match self.api.list_tags(&self.context()?) {
            Ok(tags) => ok_text(serde_json::to_string_pretty(&tags).unwrap()),
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
                "Plexus MCP server — provenance tracking (chains, marks, links). Call set_context before using other tools."
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

pub fn run_mcp_server(db_path: Option<PathBuf>) -> i32 {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to create tokio runtime: {}", e);
            return 1;
        }
    };

    rt.block_on(async {
        let engine = match db_path {
            Some(ref path) => {
                let store = match SqliteStore::open(path) {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        eprintln!("failed to open database at {}: {}", path.display(), e);
                        return 1;
                    }
                };
                let eng = PlexusEngine::with_store(store);
                if let Err(e) = eng.load_all() {
                    eprintln!("failed to load contexts: {}", e);
                    return 1;
                }
                eng
            }
            None => {
                // Default: store in current directory
                let path = std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".plexus.db");
                let store = match SqliteStore::open(&path) {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        eprintln!("failed to open database at {}: {}", path.display(), e);
                        return 1;
                    }
                };
                let eng = PlexusEngine::with_store(store);
                if let Err(e) = eng.load_all() {
                    eprintln!("failed to load contexts: {}", e);
                    return 1;
                }
                eng
            }
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
