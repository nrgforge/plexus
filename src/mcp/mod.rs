//! MCP server for Plexus — exposes context management and provenance
//! tracking via the Model Context Protocol.
//!
//! Tools: 6 context + 13 provenance = 19 total.

pub mod params;

use params::*;
use crate::{
    Context, ContextId, PlexusEngine, ProvenanceApi, Source,
    OpenStore, SqliteStore,
};
use crate::adapter::{
    CoOccurrenceEnrichment, IngestPipeline,
    ProvenanceAdapter, ProvenanceInput, TagConceptBridger,
};
use crate::graph::NodeId;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use std::path::PathBuf;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ok_text(text: String) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

fn err_text(msg: String) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(msg)]))
}

/// Find a context by name, return its ContextId.
fn find_context_by_name(engine: &PlexusEngine, name: &str) -> Option<(ContextId, Context)> {
    for cid in engine.list_contexts() {
        if let Some(ctx) = engine.get_context(&cid) {
            if ctx.name == name {
                return Some((cid, ctx));
            }
        }
    }
    None
}

/// Find the provenance context (auto-created, name = "__provenance__").
fn provenance_context(engine: &PlexusEngine) -> ContextId {
    for cid in engine.list_contexts() {
        if let Some(ctx) = engine.get_context(&cid) {
            if ctx.name == "__provenance__" {
                return cid;
            }
        }
    }
    // Auto-create if it doesn't exist
    let ctx = Context::new("__provenance__");
    let id = ctx.id.clone();
    engine.upsert_context(ctx).expect("failed to create provenance context");
    id
}

// ---------------------------------------------------------------------------
// PlexusMcpServer
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PlexusMcpServer {
    engine: Arc<PlexusEngine>,
    pipeline: Arc<IngestPipeline>,
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

        Self {
            engine,
            pipeline: Arc::new(pipeline),
            tool_router: Self::tool_router(),
        }
    }

    fn prov_api(&self) -> ProvenanceApi<'_> {
        let ctx_id = provenance_context(&self.engine);
        ProvenanceApi::new(&self.engine, ctx_id)
    }

    // ── Chain tools ─────────────────────────────────────────────────────

    #[tool(description = "Create a new provenance chain to organize related marks")]
    async fn create_chain(
        &self,
        Parameters(p): Parameters<CreateChainParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx_id = provenance_context(&self.engine);
        let chain_id = NodeId::new().to_string();

        let input = ProvenanceInput::CreateChain {
            chain_id: chain_id.clone(),
            name: p.name,
            description: p.description,
        };

        match self
            .pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await
        {
            Ok(_) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({ "created": chain_id }))
                    .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "List all chains, optionally filtered by status")]
    fn list_chains(
        &self,
        Parameters(p): Parameters<ListChainsParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().list_chains(p.status.as_deref()) {
            Ok(chains) => ok_text(serde_json::to_string_pretty(&chains).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Get a chain with all its marks")]
    fn get_chain(
        &self,
        Parameters(p): Parameters<ChainIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().get_chain(&p.chain_id) {
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
        match self.prov_api().archive_chain(&p.chain_id) {
            Ok(()) => ok_text(format!("archived chain {}", p.chain_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Delete a chain and all its marks")]
    fn delete_chain(
        &self,
        Parameters(p): Parameters<ChainIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().delete_chain(&p.chain_id) {
            Ok(()) => ok_text(format!("deleted chain {} and its marks", p.chain_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Mark tools ──────────────────────────────────────────────────────

    #[tool(description = "Add an annotated mark to a location in a file or artifact")]
    async fn add_mark(
        &self,
        Parameters(p): Parameters<AddMarkParams>,
    ) -> Result<CallToolResult, McpError> {
        let ctx_id = provenance_context(&self.engine);

        // Boundary validation: chain must exist
        let chain_node_id = crate::graph::NodeId::from(p.chain_id.as_str());
        if let Some(ctx) = self.engine.get_context(&ctx_id) {
            if ctx.get_node(&chain_node_id).is_none() {
                return err_text(format!("chain not found: {}", p.chain_id));
            }
        } else {
            return err_text("provenance context not found".to_string());
        }

        let mark_id = NodeId::new().to_string();

        let input = ProvenanceInput::AddMark {
            mark_id: mark_id.clone(),
            chain_id: p.chain_id,
            file: p.file,
            line: p.line,
            annotation: p.annotation,
            column: p.column,
            mark_type: p.r#type,
            tags: p.tags,
        };

        match self
            .pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await
        {
            Ok(_) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({ "created": mark_id })).unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Update an existing mark")]
    fn update_mark(
        &self,
        Parameters(p): Parameters<UpdateMarkParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().update_mark(
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
        let ctx_id = provenance_context(&self.engine);

        let input = ProvenanceInput::DeleteMark {
            mark_id: p.mark_id.clone(),
        };

        match self
            .pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await
        {
            Ok(_) => ok_text(format!("deleted mark {}", p.mark_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "List marks with optional filters")]
    fn list_marks(
        &self,
        Parameters(p): Parameters<ListMarksParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().list_marks(
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
        let ctx_id = provenance_context(&self.engine);

        // Boundary validation: both endpoints must exist
        if let Some(ctx) = self.engine.get_context(&ctx_id) {
            let source_nid = crate::graph::NodeId::from(p.source_id.as_str());
            let target_nid = crate::graph::NodeId::from(p.target_id.as_str());
            if ctx.get_node(&source_nid).is_none() {
                return err_text(format!("source mark not found: {}", p.source_id));
            }
            if ctx.get_node(&target_nid).is_none() {
                return err_text(format!("target mark not found: {}", p.target_id));
            }
        } else {
            return err_text("provenance context not found".to_string());
        }

        let input = ProvenanceInput::LinkMarks {
            source_id: p.source_id.clone(),
            target_id: p.target_id.clone(),
        };

        match self
            .pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await
        {
            Ok(_) => ok_text(format!("linked {} -> {}", p.source_id, p.target_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Remove a link between two marks")]
    fn unlink_marks(
        &self,
        Parameters(p): Parameters<LinkMarksParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().unlink_marks(&p.source_id, &p.target_id) {
            Ok(()) => ok_text(format!("unlinked {} -> {}", p.source_id, p.target_id)),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Get all links to and from a mark (incoming and outgoing)")]
    fn get_links(
        &self,
        Parameters(p): Parameters<MarkIdParams>,
    ) -> Result<CallToolResult, McpError> {
        match self.prov_api().get_links(&p.mark_id) {
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
        match self.prov_api().list_tags() {
            Ok(tags) => ok_text(serde_json::to_string_pretty(&tags).unwrap()),
            Err(e) => err_text(e.to_string()),
        }
    }

    // ── Context tools ───────────────────────────────────────────────────

    #[tool(description = "Create a named context")]
    fn context_create(
        &self,
        Parameters(p): Parameters<ContextCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        if find_context_by_name(&self.engine, &p.name).is_some() {
            return err_text(format!("context '{}' already exists", p.name));
        }
        let ctx = Context::new(&p.name);
        let id = ctx.id.clone();
        match self.engine.upsert_context(ctx) {
            Ok(_) => ok_text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "created": id.to_string(),
                    "name": p.name,
                }))
                .unwrap(),
            ),
            Err(e) => err_text(e.to_string()),
        }
    }

    #[tool(description = "Delete a context by name")]
    fn context_delete(
        &self,
        Parameters(p): Parameters<ContextDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        match find_context_by_name(&self.engine, &p.name) {
            Some((cid, _)) => match self.engine.remove_context(&cid) {
                Ok(_) => ok_text(format!("deleted context '{}'", p.name)),
                Err(e) => err_text(e.to_string()),
            },
            None => err_text(format!("context '{}' not found", p.name)),
        }
    }

    #[tool(description = "Add file or directory sources to a context")]
    fn context_add_sources(
        &self,
        Parameters(p): Parameters<ContextAddSourcesParams>,
    ) -> Result<CallToolResult, McpError> {
        let (cid, _) = match find_context_by_name(&self.engine, &p.name) {
            Some(pair) => pair,
            None => return err_text(format!("context '{}' not found", p.name)),
        };

        let mut added = 0usize;
        let mut warnings: Vec<String> = Vec::new();

        for raw in &p.paths {
            let raw_path = std::path::PathBuf::from(raw);
            let canonical = match raw_path.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    warnings.push(format!("skipping '{}': {}", raw, e));
                    continue;
                }
            };
            let path_str = canonical.to_string_lossy().to_string();
            let source = if canonical.is_dir() {
                Source::Directory {
                    path: path_str,
                    recursive: true,
                }
            } else {
                Source::File { path: path_str }
            };

            match self.engine.add_source(&cid, source) {
                Ok(()) => added += 1,
                Err(e) => warnings.push(format!("error adding '{}': {}", raw, e)),
            }
        }

        ok_text(
            serde_json::to_string_pretty(&serde_json::json!({
                "added": added,
                "warnings": warnings,
            }))
            .unwrap(),
        )
    }

    #[tool(description = "Remove file or directory sources from a context")]
    fn context_remove_sources(
        &self,
        Parameters(p): Parameters<ContextRemoveSourcesParams>,
    ) -> Result<CallToolResult, McpError> {
        let (cid, _) = match find_context_by_name(&self.engine, &p.name) {
            Some(pair) => pair,
            None => return err_text(format!("context '{}' not found", p.name)),
        };

        let mut removed = 0usize;

        for raw in &p.paths {
            let raw_path = std::path::PathBuf::from(raw);
            let path_str = match raw_path.canonicalize() {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => raw.clone(),
            };

            // Try both file and directory variants
            let file_source = Source::File {
                path: path_str.clone(),
            };
            let dir_source = Source::Directory {
                path: path_str,
                recursive: true,
            };

            if let Ok(true) = self.engine.remove_source(&cid, &file_source) {
                removed += 1;
            } else if let Ok(true) = self.engine.remove_source(&cid, &dir_source) {
                removed += 1;
            }
        }

        ok_text(
            serde_json::to_string_pretty(&serde_json::json!({
                "removed": removed,
            }))
            .unwrap(),
        )
    }

    #[tool(description = "List all contexts, or show sources in a specific context")]
    fn context_list(
        &self,
        Parameters(p): Parameters<ContextListParams>,
    ) -> Result<CallToolResult, McpError> {
        match p.name.as_deref() {
            None => {
                let summaries: Vec<serde_json::Value> = self
                    .engine
                    .list_contexts()
                    .iter()
                    .filter_map(|cid| {
                        let ctx = self.engine.get_context(cid)?;
                        // Hide internal provenance context
                        if ctx.name == "__provenance__" {
                            return None;
                        }
                        Some(serde_json::json!({
                            "name": ctx.name,
                            "id": cid.to_string(),
                            "sources_count": ctx.metadata.sources.len(),
                        }))
                    })
                    .collect();
                ok_text(serde_json::to_string_pretty(&summaries).unwrap())
            }
            Some(name) => match find_context_by_name(&self.engine, name) {
                Some((cid, ctx)) => ok_text(
                    serde_json::to_string_pretty(&serde_json::json!({
                        "name": ctx.name,
                        "id": cid.to_string(),
                        "sources": ctx.metadata.sources,
                    }))
                    .unwrap(),
                ),
                None => err_text(format!("context '{}' not found", name)),
            },
        }
    }

    #[tool(description = "Rename an existing context")]
    fn context_rename(
        &self,
        Parameters(p): Parameters<ContextRenameParams>,
    ) -> Result<CallToolResult, McpError> {
        if find_context_by_name(&self.engine, &p.new_name).is_some() {
            return err_text(format!("context '{}' already exists", p.new_name));
        }
        match find_context_by_name(&self.engine, &p.old_name) {
            Some((_, mut ctx)) => {
                ctx.name = p.new_name.clone();
                match self.engine.upsert_context(ctx) {
                    Ok(_) => ok_text(format!(
                        "renamed context '{}' to '{}'",
                        p.old_name, p.new_name
                    )),
                    Err(e) => err_text(e.to_string()),
                }
            }
            None => err_text(format!("context '{}' not found", p.old_name)),
        }
    }
}

#[tool_handler]
impl ServerHandler for PlexusMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Plexus MCP server — context management and provenance tracking (chains, marks, links)"
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
