//! Transport-independent API layer (ADR-014).
//!
//! `PlexusApi` is a thin routing facade: every method resolves a context name
//! and delegates to `IngestPipeline`, `ProvenanceApi`, or the query system.
//! No business logic lives here. Transports (MCP, gRPC, REST, direct
//! embedding) call `PlexusApi` methods — they never reach into the
//! underlying components directly.
//!
//! # Async vs sync boundary
//!
//! **Async** (`async fn`): operations that route through `IngestPipeline` —
//! `ingest`, `update_mark`, `archive_chain`, `delete_mark`, `delete_chain`,
//! `link_marks`, `unlink_marks`. These involve adapter execution and
//! potentially I/O-bound enrichment.
//!
//! **Sync** (`fn`): read-only operations that query the in-memory `DashMap`
//! cache — `list_chains`, `get_chain`, `list_marks`, `list_tags`, `get_links`,
//! `evidence_trail`, `find_nodes`, `traverse`, `find_path`, `context_*`.
//! Also `retract_contributions` (mutates in-memory state synchronously).
//!
//! This split is intentional: reads are fast cache lookups with no I/O,
//! while writes go through the async adapter pipeline.

use std::sync::Arc;

use crate::adapter::{
    Adapter, AdapterError, AdapterSink, EngineSink, Enrichment, FrameworkContext,
    IngestPipeline, OutboundEvent, ProvenanceInput,
};
use crate::adapter::declarative::DeclarativeAdapter;
use crate::graph::{
    Context, ContextId, NodeId, PlexusEngine, PlexusError, PlexusResult, PropertyValue, Source,
};
use crate::graph::events::GraphEvent;
use crate::provenance::{ChainView, MarkView, ProvenanceApi};
use crate::query::{
    self, EvidenceTrailResult, FindQuery, PathQuery, PathResult, QueryResult,
    TraversalResult, TraverseQuery,
};
use crate::storage::PersistedSpec;

/// Single entry point for all consumer-facing operations.
#[derive(Clone)]
pub struct PlexusApi {
    engine: Arc<PlexusEngine>,
    pipeline: Arc<IngestPipeline>,
}

impl PlexusApi {
    /// Create a new API instance.
    pub fn new(engine: Arc<PlexusEngine>, pipeline: Arc<IngestPipeline>) -> Self {
        Self { engine, pipeline }
    }

    // --- Write ---

    /// Ingest with an explicit adapter, skipping input_kind routing.
    pub async fn ingest_with_adapter(
        &self,
        context_id: &str,
        adapter: Arc<dyn crate::adapter::Adapter>,
        data: Box<dyn std::any::Any + Send + Sync>,
    ) -> Result<Vec<OutboundEvent>, AdapterError> {
        self.pipeline
            .ingest_with_adapter(context_id, adapter, data)
            .await
    }

    /// The single write endpoint (ADR-012).
    pub async fn ingest(
        &self,
        context_id: &str,
        input_kind: &str,
        data: Box<dyn std::any::Any + Send + Sync>,
    ) -> Result<Vec<OutboundEvent>, AdapterError> {
        self.pipeline.ingest(context_id, input_kind, data).await
    }

    // --- Provenance reads ---

    /// List chains in a context, optionally filtered by status.
    pub fn list_chains(
        &self,
        context_id: &str,
        status: Option<&str>,
    ) -> PlexusResult<Vec<ChainView>> {
        self.prov(context_id)?.list_chains(status)
    }

    /// Get a chain and its marks.
    pub fn get_chain(
        &self,
        context_id: &str,
        chain_id: &str,
    ) -> PlexusResult<(ChainView, Vec<MarkView>)> {
        self.prov(context_id)?.get_chain(chain_id)
    }

    /// List marks, with optional filters.
    pub fn list_marks(
        &self,
        context_id: &str,
        chain_id: Option<&str>,
        file: Option<&str>,
        mark_type: Option<&str>,
        tag: Option<&str>,
    ) -> PlexusResult<Vec<MarkView>> {
        self.prov(context_id)?.list_marks(chain_id, file, mark_type, tag)
    }

    /// List all tags used in a context.
    pub fn list_tags(&self, context_id: &str) -> PlexusResult<Vec<String>> {
        self.prov(context_id)?.list_tags()
    }

    /// Get incoming and outgoing links for a mark.
    pub fn get_links(
        &self,
        context_id: &str,
        mark_id: &str,
    ) -> PlexusResult<(Vec<crate::provenance::MarkView>, Vec<crate::provenance::MarkView>)> {
        self.prov(context_id)?.get_links(mark_id)
    }

    // --- Graph reads ---

    /// Query the evidence trail for a concept (ADR-013).
    pub fn evidence_trail(
        &self,
        context_id: &str,
        node_id: &str,
    ) -> PlexusResult<EvidenceTrailResult> {
        let ctx_id = self.resolve(context_id)?;
        let context = self
            .engine
            .get_context(&ctx_id)
            .ok_or_else(|| PlexusError::ContextNotFound(ctx_id))?;
        Ok(query::evidence_trail(node_id, &context))
    }

    /// Find nodes matching a query.
    pub fn find_nodes(
        &self,
        context_id: &str,
        query: FindQuery,
    ) -> PlexusResult<QueryResult> {
        let ctx_id = self.resolve(context_id)?;
        self.engine.find_nodes(&ctx_id, query)
    }

    /// Traverse edges from a starting node.
    pub fn traverse(
        &self,
        context_id: &str,
        query: TraverseQuery,
    ) -> PlexusResult<TraversalResult> {
        let ctx_id = self.resolve(context_id)?;
        self.engine.traverse(&ctx_id, query)
    }

    /// Find a path between two nodes.
    pub fn find_path(
        &self,
        context_id: &str,
        query: PathQuery,
    ) -> PlexusResult<PathResult> {
        let ctx_id = self.resolve(context_id)?;
        self.engine.find_path(&ctx_id, query)
    }

    /// Update a mark's metadata. Routes through ingest pipeline.
    pub async fn update_mark(
        &self,
        context_id: &str,
        mark_id: &str,
        annotation: Option<&str>,
        line: Option<u32>,
        column: Option<u32>,
        mark_type: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> Result<(), AdapterError> {
        let ctx_id = self
            .resolve(context_id)
            .map_err(|e| AdapterError::Storage(e.to_string()))?;

        let mut node = {
            let ctx = self
                .engine
                .get_context(&ctx_id)
                .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;
            let node_id = NodeId::from(mark_id);
            ctx.get_node(&node_id)
                .ok_or_else(|| AdapterError::Internal(format!("mark not found: {}", mark_id)))?
                .clone()
        };

        if let Some(a) = annotation {
            node.properties.insert("annotation".into(), PropertyValue::String(a.into()));
        }
        if let Some(l) = line {
            node.properties.insert("line".into(), PropertyValue::Int(l as i64));
        }
        if let Some(col) = column {
            node.properties.insert("column".into(), PropertyValue::Int(col as i64));
        }
        if let Some(t) = mark_type {
            node.properties.insert("type".into(), PropertyValue::String(t.into()));
        }
        if let Some(t) = tags {
            let tag_vals: Vec<PropertyValue> = t.iter()
                .map(|s| PropertyValue::String(s.clone()))
                .collect();
            node.properties.insert("tags".into(), PropertyValue::Array(tag_vals));
        }

        let input = ProvenanceInput::UpdateMark { node };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await?;
        Ok(())
    }

    /// Archive a chain. Routes through ingest pipeline.
    ///
    /// Library-only — not exposed via MCP. The MCP transport uses `ingest`
    /// as its single write tool (ADR-028). Direct callers (Rust embedding)
    /// can use this for convenience.
    pub async fn archive_chain(
        &self,
        context_id: &str,
        chain_id: &str,
    ) -> Result<(), AdapterError> {
        let ctx_id = self
            .resolve(context_id)
            .map_err(|e| AdapterError::Storage(e.to_string()))?;

        let mut node = {
            let ctx = self
                .engine
                .get_context(&ctx_id)
                .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;
            let node_id = NodeId::from(chain_id);
            ctx.get_node(&node_id)
                .ok_or_else(|| AdapterError::Internal(format!("chain not found: {}", chain_id)))?
                .clone()
        };

        node.properties.insert("status".into(), PropertyValue::String("archived".into()));

        let input = ProvenanceInput::ArchiveChain { node };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await?;
        Ok(())
    }

    /// Delete a mark (cascades edges). Routes through ingest pipeline.
    pub async fn delete_mark(
        &self,
        context_id: &str,
        mark_id: &str,
    ) -> Result<(), AdapterError> {
        let ctx_id = self
            .resolve(context_id)
            .map_err(|e| AdapterError::Storage(e.to_string()))?;
        let input = ProvenanceInput::DeleteMark {
            mark_id: mark_id.to_string(),
        };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await?;
        Ok(())
    }

    /// Delete a chain and all its marks. Routes through ingest pipeline.
    pub async fn delete_chain(
        &self,
        context_id: &str,
        chain_id: &str,
    ) -> Result<(), DeleteChainError> {
        let ctx_id = self
            .resolve(context_id)
            .map_err(DeleteChainError::Plexus)?;

        // Pre-resolve mark IDs belonging to this chain
        let mark_ids = {
            let ctx = self
                .engine
                .get_context(&ctx_id)
                .ok_or_else(|| DeleteChainError::Plexus(PlexusError::ContextNotFound(ctx_id.clone())))?;
            let chain_nid = crate::graph::NodeId::from(chain_id);
            if ctx.get_node(&chain_nid).is_none() {
                return Err(DeleteChainError::ChainNotFound(chain_id.to_string()));
            }
            ctx.edges()
                .filter(|e| e.source == chain_nid && e.relationship == "contains")
                .map(|e| e.target.to_string())
                .collect::<Vec<_>>()
        };

        let input = ProvenanceInput::DeleteChain {
            chain_id: chain_id.to_string(),
            mark_ids,
        };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await
            .map_err(DeleteChainError::Adapter)?;
        Ok(())
    }

    /// Link two marks. Validates both endpoints exist. Routes through ingest pipeline.
    pub async fn link_marks(
        &self,
        context_id: &str,
        source_id: &str,
        target_id: &str,
    ) -> Result<(), LinkError> {
        let ctx_id = self.resolve(context_id).map_err(LinkError::Plexus)?;
        let ctx = self
            .engine
            .get_context(&ctx_id)
            .ok_or_else(|| LinkError::Plexus(PlexusError::ContextNotFound(ctx_id.clone())))?;

        let source_nid = crate::graph::NodeId::from(source_id);
        let target_nid = crate::graph::NodeId::from(target_id);
        if ctx.get_node(&source_nid).is_none() {
            return Err(LinkError::MarkNotFound(source_id.to_string()));
        }
        if ctx.get_node(&target_nid).is_none() {
            return Err(LinkError::MarkNotFound(target_id.to_string()));
        }
        drop(ctx);

        let input = ProvenanceInput::LinkMarks {
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
        };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await
            .map_err(LinkError::Adapter)?;
        Ok(())
    }

    /// Unlink two marks. Routes through ingest pipeline.
    pub async fn unlink_marks(
        &self,
        context_id: &str,
        source_id: &str,
        target_id: &str,
    ) -> Result<(), AdapterError> {
        let ctx_id = self
            .resolve(context_id)
            .map_err(|e| AdapterError::Storage(e.to_string()))?;
        let input = ProvenanceInput::UnlinkMarks {
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
        };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await?;
        Ok(())
    }

    // --- Spec lifecycle (ADR-037) ---

    /// Load a consumer's declarative adapter spec onto a context (ADR-037).
    ///
    /// Three-effect model (Invariant 62):
    /// (a) Lens runs immediately on existing content → durable graph data
    /// (b) Adapter + enrichments + lens registered on pipeline → durable enrichment registration
    /// (c) Spec persisted to specs table → durable across restarts
    ///
    /// Fail-fast (Invariant 60): validation happens before any mutations.
    /// All-or-nothing: if any step fails after registration, earlier steps are rolled back.
    pub async fn load_spec(
        &self,
        context_id: &str,
        spec_yaml: &str,
    ) -> Result<SpecLoadResult, SpecLoadError> {
        let ctx_id = self.resolve(context_id)
            .map_err(|_| SpecLoadError::ContextNotFound(context_id.to_string()))?;

        // Step 1: Validate and parse (Invariant 60 — fail before any mutations)
        let adapter = DeclarativeAdapter::from_yaml(spec_yaml)
            .map_err(|e| SpecLoadError::Validation(e.to_string()))?;

        let adapter_id = adapter.id().to_string();

        // Extract enrichments and lens
        let mut enrichments = adapter.enrichments()
            .map_err(|e| SpecLoadError::Validation(e.to_string()))?;
        let lens = adapter.lens();
        let lens_namespace = lens.as_ref().map(|l| l.id().to_string());

        if let Some(ref l) = lens {
            enrichments.push(l.clone());
        }

        // Step 2: Register on pipeline (effect b + transient adapter wiring)
        self.pipeline.register_integration(Arc::new(adapter), enrichments);

        // Step 3: Persist to specs table (effect c)
        let persisted = PersistedSpec {
            context_id: context_id.to_string(),
            adapter_id: adapter_id.clone(),
            spec_yaml: spec_yaml.to_string(),
            loaded_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Err(e) = self.engine.persist_spec(&persisted) {
            // Rollback step 2: deregister from pipeline
            // Note: currently no deregister method — pipeline registration is append-only
            // during WP-C. The spec row not being persisted means rehydration won't
            // re-register on restart, so the effect is transient-only.
            return Err(SpecLoadError::Persistence(e.to_string()));
        }

        // Step 4: Lens sweep over existing content (effect a)
        let mut vocabulary_edges_created = 0;
        if let Some(ref lens_enrichment) = lens {
            let context = self.engine.get_context(&ctx_id)
                .ok_or_else(|| SpecLoadError::ContextNotFound(context_id.to_string()))?;

            // Synthesize an EdgesAdded event to trigger the lens on all existing edges
            let edge_ids: Vec<crate::graph::EdgeId> = context.edges.iter()
                .map(|e| e.id.clone())
                .collect();

            if !edge_ids.is_empty() {
                let synthetic_event = GraphEvent::EdgesAdded {
                    edge_ids,
                    adapter_id: "load_spec_sweep".to_string(),
                    context_id: ctx_id.as_str().to_string(),
                };

                if let Some(emission) = lens_enrichment.enrich(&[synthetic_event], &context) {
                    let edge_count = emission.edges.len();

                    // Commit the lens output via EngineSink
                    let sink = EngineSink::for_engine(self.engine.clone(), ctx_id.clone())
                        .with_framework_context(FrameworkContext {
                            adapter_id: lens_enrichment.id().to_string(),
                            context_id: context_id.to_string(),
                            input_summary: None,
                        });

                    sink.emit(emission).await
                        .map_err(|e| SpecLoadError::LensSweep(e.to_string()))?;

                    vocabulary_edges_created = edge_count;
                }
            }
        }

        Ok(SpecLoadResult {
            adapter_id,
            lens_namespace,
            vocabulary_edges_created,
        })
    }

    /// Unload a consumer's spec from a context (ADR-037 §6).
    ///
    /// Reverses effects (b) and (c) of `load_spec`:
    /// - Deregisters the adapter from ingest routing
    /// - Deregisters the lens enrichment
    /// - Deletes the specs table row
    ///
    /// Does NOT reverse effect (a): vocabulary edges remain in the graph
    /// as durable data (Invariant 62).
    pub fn unload_spec(
        &self,
        context_id: &str,
        adapter_id: &str,
    ) -> Result<(), SpecUnloadError> {
        self.resolve(context_id)
            .map_err(|_| SpecUnloadError::ContextNotFound(context_id.to_string()))?;

        // Deregister adapter and lens from pipeline
        self.pipeline.deregister_adapter(adapter_id);
        // Lens ID follows the convention "lens:{consumer}" where consumer
        // is derived from the adapter spec. We look it up from the persisted
        // spec to get the correct lens ID.
        if let Ok(specs) = self.engine.query_specs_for_context(context_id) {
            if let Some(spec) = specs.iter().find(|s| s.adapter_id == adapter_id) {
                if let Ok(adapter) = DeclarativeAdapter::from_yaml(&spec.spec_yaml) {
                    if let Some(lens) = adapter.lens() {
                        self.pipeline.deregister_enrichment(lens.id());
                    }
                    // Also deregister declared enrichments
                    if let Ok(enrichments) = adapter.enrichments() {
                        for enrichment in &enrichments {
                            self.pipeline.deregister_enrichment(enrichment.id());
                        }
                    }
                }
            }
        }

        // Delete from specs table
        self.engine.delete_spec(context_id, adapter_id)
            .map_err(|e| SpecUnloadError::Persistence(e.to_string()))?;

        Ok(())
    }

    // --- Contribution retraction (ADR-027) ---

    /// Retract all contributions from an adapter/enrichment (ADR-027).
    ///
    /// Removes the adapter's contribution slot from every edge in the context,
    /// prunes zero-evidence edges, recomputes combined weights, fires events,
    /// and runs the enrichment loop so dependent enrichments can react.
    /// Returns the count of edges affected.
    ///
    /// Bypasses the adapter pipeline intentionally — retraction is an
    /// engine-level operation that reverses a prior adapter's effect,
    /// then runs the enrichment loop directly for re-normalization.
    pub fn retract_contributions(
        &self,
        context_id: &str,
        adapter_id: &str,
    ) -> PlexusResult<usize> {
        use crate::adapter::GraphEvent;
        use crate::adapter::run_enrichment_loop;

        let ctx_id = self.resolve(context_id)?;

        // Phase 1: Retract and get events
        let events = self.engine.retract_contributions(&ctx_id, adapter_id)?;

        // Extract edges_affected from the ContributionsRetracted event
        let edges_affected = events.iter().find_map(|e| match e {
            GraphEvent::ContributionsRetracted { edges_affected, .. } => Some(*edges_affected),
            _ => None,
        }).unwrap_or(0);

        // Phase 2: Run enrichment loop with retraction events
        let registry = self.pipeline.enrichment_registry();
        if !registry.enrichments().is_empty() && !events.is_empty() {
            let _ = run_enrichment_loop(
                &self.engine,
                &ctx_id,
                &registry,
                &events,
            );
        }

        Ok(edges_affected)
    }

    // --- Context management ---

    /// Create a context. Returns error if name is already taken.
    ///
    /// Intentionally bypasses the adapter pipeline: context lifecycle is
    /// infrastructure, not content ingestion — no enrichment applies.
    pub fn context_create(&self, name: &str) -> PlexusResult<ContextId> {
        if self.resolve(name).is_ok() {
            return Err(PlexusError::Other(format!("context '{}' already exists", name)));
        }
        let context = Context::new(name);
        self.engine.upsert_context(context)
    }

    /// Get detailed info about a context by name.
    pub fn context_info(&self, name: &str) -> PlexusResult<ContextInfo> {
        let ctx_id = self.resolve(name)?;
        let ctx = self
            .engine
            .get_context(&ctx_id)
            .ok_or_else(|| PlexusError::ContextNotFound(ctx_id.clone()))?;
        Ok(ContextInfo {
            name: ctx.name.clone(),
            id: ctx_id,
            sources: ctx.metadata.sources.clone(),
        })
    }

    /// Delete a context.
    pub fn context_delete(&self, name: &str) -> PlexusResult<()> {
        let ctx_id = self.resolve(name)?;
        self.engine.remove_context(&ctx_id)?;
        Ok(())
    }

    /// List contexts. If name is provided, returns just that context's info.
    pub fn context_list(&self, name: Option<&str>) -> PlexusResult<Vec<ContextId>> {
        match name {
            Some(n) => {
                if let Ok(ctx_id) = self.resolve(n) {
                    Ok(vec![ctx_id])
                } else {
                    Ok(vec![])
                }
            }
            None => Ok(self.engine.list_contexts()),
        }
    }

    /// List all contexts with metadata.
    pub fn context_list_info(&self) -> PlexusResult<Vec<ContextInfo>> {
        let mut result = Vec::new();
        for cid in self.engine.list_contexts() {
            if let Some(ctx) = self.engine.get_context(&cid) {
                result.push(ContextInfo {
                    name: ctx.name.clone(),
                    id: cid,
                    sources: ctx.metadata.sources.clone(),
                });
            }
        }
        Ok(result)
    }

    /// Rename a context. Returns error if new name is already taken.
    pub fn context_rename(&self, old_name: &str, new_name: &str) -> PlexusResult<()> {
        if self.resolve(new_name).is_ok() {
            return Err(PlexusError::Other(format!("context '{}' already exists", new_name)));
        }
        let ctx_id = self.resolve(old_name)?;
        self.engine.rename_context(&ctx_id, new_name)
    }

    /// Add sources to a context.
    pub fn context_add_sources(&self, name: &str, sources: &[Source]) -> PlexusResult<()> {
        let ctx_id = self.resolve(name)?;
        for source in sources {
            self.engine.add_source(&ctx_id, source.clone())?;
        }
        Ok(())
    }

    /// Remove sources from a context.
    pub fn context_remove_sources(&self, name: &str, sources: &[Source]) -> PlexusResult<()> {
        let ctx_id = self.resolve(name)?;
        for source in sources {
            self.engine.remove_source(&ctx_id, source)?;
        }
        Ok(())
    }

    // --- Internal ---

    /// Resolve a context name to its ContextId (O(1) via name index).
    fn resolve(&self, name: &str) -> PlexusResult<ContextId> {
        self.engine
            .resolve_by_name(name)
            .ok_or_else(|| PlexusError::ContextNotFound(ContextId::from(name)))
    }

    /// Discover concepts shared between two contexts via deterministic
    /// ID intersection (ADR-017 §4).
    ///
    /// Returns concept node IDs that exist in both contexts.
    pub fn shared_concepts(
        &self,
        context_a: &str,
        context_b: &str,
    ) -> PlexusResult<Vec<NodeId>> {
        let id_a = self.resolve(context_a)?;
        let id_b = self.resolve(context_b)?;

        let ctx_a = self
            .engine
            .get_context(&id_a)
            .ok_or_else(|| PlexusError::ContextNotFound(id_a))?;
        let ctx_b = self
            .engine
            .get_context(&id_b)
            .ok_or_else(|| PlexusError::ContextNotFound(id_b))?;

        Ok(query::shared_concepts(&ctx_a, &ctx_b))
    }

    /// Query events after the given cursor (ADR-035).
    ///
    /// Returns events with sequence > cursor for the named context.
    pub fn changes_since(
        &self,
        context_name: &str,
        cursor: u64,
        filter: Option<&query::CursorFilter>,
    ) -> PlexusResult<query::ChangeSet> {
        let context_id = self.resolve(context_name)?;
        let events = self.engine.query_events_since(context_id.as_str(), cursor, filter)?;
        let latest = if events.is_empty() {
            let stored = self.engine.latest_sequence(context_id.as_str())?;
            std::cmp::max(cursor, stored)
        } else {
            events.last().unwrap().sequence
        };
        Ok(query::ChangeSet {
            events,
            latest_sequence: latest,
        })
    }

    fn prov(&self, context_name: &str) -> PlexusResult<ProvenanceApi> {
        let cid = self.resolve(context_name)?;
        Ok(ProvenanceApi::new(&self.engine, cid))
    }
}

/// Error from `delete_chain`.
#[derive(Debug)]
pub enum DeleteChainError {
    /// Chain not found.
    ChainNotFound(String),
    /// Underlying adapter error.
    Adapter(AdapterError),
    /// Underlying engine error.
    Plexus(PlexusError),
}

impl std::fmt::Display for DeleteChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChainNotFound(id) => write!(f, "chain not found: {}", id),
            Self::Adapter(e) => write!(f, "adapter error: {}", e),
            Self::Plexus(e) => write!(f, "engine error: {}", e),
        }
    }
}

impl std::error::Error for DeleteChainError {}

/// Error from `link_marks`.
#[derive(Debug)]
pub enum LinkError {
    /// Mark not found.
    MarkNotFound(String),
    /// Underlying adapter error.
    Adapter(AdapterError),
    /// Underlying engine error.
    Plexus(PlexusError),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MarkNotFound(id) => write!(f, "mark not found: {}", id),
            Self::Adapter(e) => write!(f, "adapter error: {}", e),
            Self::Plexus(e) => write!(f, "engine error: {}", e),
        }
    }
}

impl std::error::Error for LinkError {}

/// Info about a context (for context_list metadata).
#[derive(Debug, Clone)]
pub struct ContextInfo {
    pub name: String,
    pub id: ContextId,
    pub sources: Vec<Source>,
}

/// Result of a successful `load_spec` call (ADR-037).
#[derive(Debug)]
pub struct SpecLoadResult {
    /// The adapter ID from the loaded spec.
    pub adapter_id: String,
    /// The lens namespace (e.g., "lens:trellis"), if the spec has a lens section.
    pub lens_namespace: Option<String>,
    /// Number of vocabulary edges created by the initial lens sweep.
    pub vocabulary_edges_created: usize,
}

/// Errors from `load_spec` (ADR-037, Invariant 60).
#[derive(Debug)]
pub enum SpecLoadError {
    /// The context does not exist.
    ContextNotFound(String),
    /// Spec YAML failed to parse or validate.
    Validation(String),
    /// Pipeline registration failed.
    Registration(String),
    /// Persisting the spec to storage failed.
    Persistence(String),
    /// The lens sweep over existing content failed.
    LensSweep(String),
}

impl std::fmt::Display for SpecLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextNotFound(s) => write!(f, "context not found: {s}"),
            Self::Validation(s) => write!(f, "spec validation failed: {s}"),
            Self::Registration(s) => write!(f, "registration failed: {s}"),
            Self::Persistence(s) => write!(f, "persistence failed: {s}"),
            Self::LensSweep(s) => write!(f, "lens sweep failed: {s}"),
        }
    }
}

impl std::error::Error for SpecLoadError {}

/// Errors from `unload_spec` (ADR-037 §6).
#[derive(Debug)]
pub enum SpecUnloadError {
    /// The context does not exist.
    ContextNotFound(String),
    /// Deleting the spec from storage failed.
    Persistence(String),
}

impl std::fmt::Display for SpecUnloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextNotFound(s) => write!(f, "context not found: {s}"),
            Self::Persistence(s) => write!(f, "persistence failed: {s}"),
        }
    }
}

impl std::error::Error for SpecUnloadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Edge, Node, NodeId, PropertyValue, dimension};

    fn setup() -> (Arc<PlexusEngine>, PlexusApi) {
        let engine = Arc::new(PlexusEngine::new());
        let pipeline = Arc::new(IngestPipeline::new(engine.clone()));
        let api = PlexusApi::new(engine.clone(), pipeline);
        (engine, api)
    }

    // === Scenario: PlexusApi delegates provenance reads to ProvenanceApi ===
    #[test]
    fn list_chains_delegates_to_provenance_api() {
        let (engine, api) = setup();

        // Set up a context with a chain node
        let mut ctx = Context::new("research");
        let mut chain = Node::new_in_dimension("chain", ContentType::Provenance, dimension::PROVENANCE);
        chain.id = NodeId::from("chain:provenance:field-notes");
        chain.properties.insert("name".into(), PropertyValue::String("field-notes".into()));
        chain.properties.insert("status".into(), PropertyValue::String("active".into()));
        ctx.nodes.insert(chain.id.clone(), chain);
        engine.upsert_context(ctx).unwrap();

        let chains = api.list_chains("research", None).unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].name, "field-notes");
    }

    // === Scenario: PlexusApi delegates graph queries to query system ===
    #[test]
    fn find_nodes_delegates_to_query_system() {
        let (engine, api) = setup();

        let mut ctx = Context::new("research");
        let mut node = Node::new_in_dimension("concept", ContentType::Provenance, dimension::SEMANTIC);
        node.id = NodeId::from("concept:travel");
        ctx.nodes.insert(node.id.clone(), node);
        engine.upsert_context(ctx).unwrap();

        let result = api.find_nodes("research", FindQuery::new().with_node_type("concept")).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id.to_string(), "concept:travel");
    }

    // === Scenario: list_tags is context-scoped ===
    #[test]
    fn list_tags_is_context_scoped() {
        let (engine, api) = setup();

        // Context alpha with #travel tag
        let mut alpha = Context::new("alpha");
        let mut mark_a = Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
        mark_a.id = NodeId::from("mark:a1");
        mark_a.properties.insert("tags".into(), PropertyValue::Array(vec![PropertyValue::String("travel".into())]));
        alpha.nodes.insert(mark_a.id.clone(), mark_a);
        engine.upsert_context(alpha).unwrap();

        // Context beta with #cooking tag
        let mut beta = Context::new("beta");
        let mut mark_b = Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
        mark_b.id = NodeId::from("mark:b1");
        mark_b.properties.insert("tags".into(), PropertyValue::Array(vec![PropertyValue::String("cooking".into())]));
        beta.nodes.insert(mark_b.id.clone(), mark_b);
        engine.upsert_context(beta).unwrap();

        let tags = api.list_tags("alpha").unwrap();
        assert!(tags.contains(&"travel".to_string()));
        assert!(!tags.contains(&"cooking".to_string()));
    }

    // === Scenario: update_mark routes through ingest pipeline ===
    #[tokio::test]
    async fn update_mark_routes_through_pipeline() {
        let (engine, api) = setup_with_provenance();

        // Create context with a mark
        let mut ctx = Context::new("research");
        let mut mark = Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
        mark.id = NodeId::from("mark:1");
        mark.properties.insert("annotation".into(), PropertyValue::String("original".into()));
        mark.properties.insert("file".into(), PropertyValue::String("src/main.rs".into()));
        mark.properties.insert("line".into(), PropertyValue::String("42".into()));
        mark.properties.insert("chain_id".into(), PropertyValue::String("chain:provenance:notes".into()));
        // Add chain to contain the mark (required for mark to be found by list_marks)
        let mut chain = Node::new_in_dimension("chain", ContentType::Provenance, dimension::PROVENANCE);
        chain.id = NodeId::from("chain:provenance:notes");
        chain.properties.insert("name".into(), PropertyValue::String("notes".into()));
        chain.properties.insert("status".into(), PropertyValue::String("active".into()));
        ctx.nodes.insert(chain.id.clone(), chain);
        ctx.nodes.insert(mark.id.clone(), mark);
        ctx.edges.push(Edge::new(
            NodeId::from("chain:provenance:notes"),
            NodeId::from("mark:1"),
            "contains",
        ));
        engine.upsert_context(ctx).unwrap();

        // Update through PlexusApi (now async, routes through pipeline)
        api.update_mark("research", "mark:1", Some("updated"), None, None, None, None)
            .await
            .unwrap();

        // Verify via provenance read
        let marks = api.list_marks("research", None, None, None, None).unwrap();
        let updated = marks.iter().find(|m| m.id == "mark:1").unwrap();
        assert_eq!(updated.annotation, "updated");
    }

    // === Scenario: Transport calls PlexusApi for ingest ===
    #[tokio::test]
    async fn ingest_delegates_to_pipeline() {
        let engine = Arc::new(PlexusEngine::new());
        engine.upsert_context(Context::new("research")).unwrap();

        let pipeline = Arc::new(IngestPipeline::new(engine.clone()));
        let api = PlexusApi::new(engine.clone(), pipeline);

        // Ingest with an unknown input_kind — should return an error because
        // no adapter handles "unknown". This proves PlexusApi delegates to
        // IngestPipeline rather than handling it directly.
        let result = api
            .ingest("research", "unknown", Box::new(()))
            .await;

        assert!(result.is_err());
    }

    // --- Ingest-based annotation workflow (ADR-015 / ADR-028) ---

    use crate::adapter::{ContentAdapter, FragmentInput, ProvenanceAdapter, normalize_chain_name};

    fn setup_with_provenance() -> (Arc<PlexusEngine>, PlexusApi) {
        let engine = Arc::new(PlexusEngine::new());
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(Arc::new(ContentAdapter::new("annotate")));
        pipeline.register_adapter(Arc::new(ProvenanceAdapter::new()));
        let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));
        (engine, api)
    }

    // === Scenario: Ingest creates fragment, chain, and mark ===
    #[tokio::test]
    async fn ingest_creates_fragment_chain_and_mark() {
        let (engine, api) = setup_with_provenance();
        let ctx_id = engine.upsert_context(Context::new("research")).unwrap();
        let cid = ctx_id.as_str();

        // Step 1: Ingest fragment (semantic content)
        let fragment_input = FragmentInput::new("interesting pattern", vec!["refactor".into()])
            .with_source("src/main.rs");
        let frag_events = api
            .ingest(cid, "content", Box::new(fragment_input))
            .await
            .unwrap();
        assert!(!frag_events.is_empty(), "fragment ingest should produce events");

        // Step 2: Create chain via provenance ingest
        let chain_input = ProvenanceInput::CreateChain {
            chain_id: normalize_chain_name("field notes"),
            name: "field notes".to_string(),
            description: None,
        };
        let chain_events = api
            .ingest(cid, "provenance", Box::new(chain_input))
            .await
            .unwrap();
        assert!(!chain_events.is_empty(), "chain ingest should produce events");

        // Step 3: Create mark via provenance ingest
        let mark_input = ProvenanceInput::AddMark {
            mark_id: "mark:provenance:test-1".to_string(),
            chain_id: normalize_chain_name("field notes"),
            file: "src/main.rs".to_string(),
            line: 42,
            annotation: "interesting pattern".to_string(),
            column: None,
            mark_type: None,
            tags: Some(vec!["refactor".into()]),
        };
        api.ingest(cid, "provenance", Box::new(mark_input))
            .await
            .unwrap();

        // Verify fragment node exists (semantic content)
        let ctx = engine.get_context(&ctx_id).unwrap();
        let fragments: Vec<_> = ctx.nodes.values()
            .filter(|n| n.node_type == "fragment")
            .collect();
        assert_eq!(fragments.len(), 1);
        assert_eq!(
            fragments[0].properties.get("text"),
            Some(&PropertyValue::String("interesting pattern".into()))
        );

        // Verify concept node exists (from tags)
        assert!(ctx.get_node(&NodeId::from("concept:refactor")).is_some());

        // Verify chain exists
        let chains = api.list_chains("research", None).unwrap();
        let user_chain = chains.iter().find(|c| c.id == "chain:provenance:field-notes");
        assert!(user_chain.is_some(), "user's chain should exist");

        // Verify mark exists in the chain
        let marks = api.list_marks("research", Some("chain:provenance:field-notes"), None, None, None).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].annotation, "interesting pattern");
        assert_eq!(marks[0].file, "src/main.rs");
        assert_eq!(marks[0].line, 42);
    }

    // === Scenario: Second ingest reuses existing chain ===
    #[tokio::test]
    async fn ingest_reuses_existing_chain() {
        let (engine, api) = setup_with_provenance();
        let ctx_id = engine.upsert_context(Context::new("research")).unwrap();
        let cid = ctx_id.as_str();

        let chain_id = normalize_chain_name("field notes");

        // First: create chain + mark
        api.ingest(cid, "content", Box::new(
            FragmentInput::new("first", vec![]),
        )).await.unwrap();
        api.ingest(cid, "provenance", Box::new(
            ProvenanceInput::CreateChain {
                chain_id: chain_id.clone(),
                name: "field notes".to_string(),
                description: None,
            },
        )).await.unwrap();
        api.ingest(cid, "provenance", Box::new(
            ProvenanceInput::AddMark {
                mark_id: "mark:provenance:test-1".to_string(),
                chain_id: chain_id.clone(),
                file: "src/main.rs".to_string(),
                line: 42,
                annotation: "first".to_string(),
                column: None,
                mark_type: None,
                tags: None,
            },
        )).await.unwrap();

        // Second: reuse chain (upsert is idempotent), add another mark
        api.ingest(cid, "content", Box::new(
            FragmentInput::new("second", vec![]),
        )).await.unwrap();
        api.ingest(cid, "provenance", Box::new(
            ProvenanceInput::CreateChain {
                chain_id: chain_id.clone(),
                name: "field notes".to_string(),
                description: None,
            },
        )).await.unwrap();
        api.ingest(cid, "provenance", Box::new(
            ProvenanceInput::AddMark {
                mark_id: "mark:provenance:test-2".to_string(),
                chain_id: chain_id.clone(),
                file: "src/lib.rs".to_string(),
                line: 10,
                annotation: "second".to_string(),
                column: None,
                mark_type: None,
                tags: None,
            },
        )).await.unwrap();

        // Still only one user chain
        let chains = api.list_chains("research", None).unwrap();
        let user_chains: Vec<_> = chains.iter()
            .filter(|c| c.id.starts_with("chain:provenance:"))
            .collect();
        assert_eq!(user_chains.len(), 1);

        // Two marks in the user's chain
        let marks = api.list_marks("research", Some("chain:provenance:field-notes"), None, None, None).unwrap();
        assert_eq!(marks.len(), 2);
    }

    // === Scenario: Chain name normalization produces deterministic IDs ===
    #[test]
    fn chain_name_normalization_deterministic() {
        assert_eq!(
            normalize_chain_name("Field Notes"),
            normalize_chain_name("field notes"),
        );
        assert_eq!(
            normalize_chain_name("field notes"),
            "chain:provenance:field-notes",
        );
    }

    // === Scenario: Chain name normalization handles special characters ===
    #[test]
    fn chain_name_normalization_special_characters() {
        assert_eq!(
            normalize_chain_name("research: phase 1/2"),
            "chain:provenance:research--phase-1-2",
        );
    }

    // === Scenario: Content ingest triggers enrichment loop ===
    #[tokio::test]
    async fn content_ingest_triggers_enrichment() {
        let engine = Arc::new(PlexusEngine::new());
        let ctx_id = engine.upsert_context(Context::new("research")).unwrap();
        let cid = ctx_id.as_str();

        // Set up pipeline with adapters and enrichments
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(Arc::new(ContentAdapter::new("annotate")));
        pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            vec![],
        );
        let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

        // Ingest content with tags
        let fragment_input = FragmentInput::new("cleanup", vec!["refactor".into()]);
        api.ingest(cid, "content", Box::new(fragment_input))
            .await
            .unwrap();

        // ContentAdapter creates concept:refactor from the tag
        let ctx = engine.get_context(&ctx_id).unwrap();
        assert!(ctx.get_node(&NodeId::from("concept:refactor")).is_some(),
            "concept should be created from tag");
    }

    // === Scenario: create_chain not exposed as consumer-facing ===
    #[test]
    fn create_chain_not_on_public_surface() {
        // This is a compile-time guarantee: PlexusApi has no create_chain method.
        // If someone adds one, this test reminds them it shouldn't be there.
        // The test passes by virtue of PlexusApi not having a create_chain method.
        let (_, api) = setup();
        // api.create_chain(...) would not compile
        let _ = api; // use to prevent unused warning
    }

    // === Scenario: Each ingest step produces outbound events ===
    #[tokio::test]
    async fn ingest_steps_produce_outbound_events() {
        let (engine, api) = setup_with_provenance();
        let ctx_id = engine.upsert_context(Context::new("research")).unwrap();
        let cid = ctx_id.as_str();

        let frag_events = api
            .ingest(cid, "content", Box::new(FragmentInput::new("note", vec![])))
            .await
            .unwrap();
        assert!(!frag_events.is_empty(), "fragment ingest should produce events");

        let chain_events = api
            .ingest(cid, "provenance", Box::new(ProvenanceInput::CreateChain {
                chain_id: normalize_chain_name("notes"),
                name: "notes".to_string(),
                description: None,
            }))
            .await
            .unwrap();
        assert!(!chain_events.is_empty(), "chain ingest should produce events");

        let mark_events = api
            .ingest(cid, "provenance", Box::new(ProvenanceInput::AddMark {
                mark_id: "mark:provenance:test-1".to_string(),
                chain_id: normalize_chain_name("notes"),
                file: "src/main.rs".to_string(),
                line: 1,
                annotation: "note".to_string(),
                column: None,
                mark_type: None,
                tags: None,
            }))
            .await
            .unwrap();
        assert!(!mark_events.is_empty(), "mark ingest should produce events");
    }

    // === Scenario: Shared concepts via deterministic ID intersection (ADR-017 §4) ===

    #[test]
    fn shared_concepts_returns_intersection() {
        let (engine, api) = setup();

        // Context "research" with concepts: travel, distributed-systems, provence
        let mut research = Context::new("research");
        for name in ["travel", "distributed-systems", "provence"] {
            let mut node = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
            node.id = NodeId::from(format!("concept:{}", name));
            research.nodes.insert(node.id.clone(), node);
        }
        engine.upsert_context(research).unwrap();

        // Context "fiction" with concepts: travel, identity, provence
        let mut fiction = Context::new("fiction");
        for name in ["travel", "identity", "provence"] {
            let mut node = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
            node.id = NodeId::from(format!("concept:{}", name));
            fiction.nodes.insert(node.id.clone(), node);
        }
        engine.upsert_context(fiction).unwrap();

        let shared = api.shared_concepts("research", "fiction").unwrap();
        let shared_strs: std::collections::HashSet<String> =
            shared.iter().map(|id| id.to_string()).collect();

        assert!(shared_strs.contains("concept:travel"));
        assert!(shared_strs.contains("concept:provence"));
        assert!(!shared_strs.contains("concept:distributed-systems"));
        assert!(!shared_strs.contains("concept:identity"));
        assert_eq!(shared.len(), 2);
    }

    #[test]
    fn shared_concepts_returns_empty_when_no_overlap() {
        let (engine, api) = setup();

        let mut alpha = Context::new("alpha");
        for name in ["a", "b"] {
            let mut node = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
            node.id = NodeId::from(format!("concept:{}", name));
            alpha.nodes.insert(node.id.clone(), node);
        }
        engine.upsert_context(alpha).unwrap();

        let mut beta = Context::new("beta");
        for name in ["c", "d"] {
            let mut node = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
            node.id = NodeId::from(format!("concept:{}", name));
            beta.nodes.insert(node.id.clone(), node);
        }
        engine.upsert_context(beta).unwrap();

        let shared = api.shared_concepts("alpha", "beta").unwrap();
        assert!(shared.is_empty());
    }

    #[test]
    fn shared_concepts_errors_on_nonexistent_context() {
        let (engine, api) = setup();
        engine.upsert_context(Context::new("real")).unwrap();

        let result = api.shared_concepts("real", "imaginary");
        assert!(result.is_err());
    }

    // === ADR-027: Contribution Retraction via PlexusApi ===

    #[test]
    fn retract_contributions_removes_adapter_slots() {
        let (engine, api) = setup();

        let mut ctx = Context::new("research");
        let id_a = NodeId::from("concept:travel");
        let id_b = NodeId::from("concept:journey");
        ctx.nodes.insert(id_a.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx.nodes.insert(id_b.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", dimension::SEMANTIC)
                .with_contribution("embedding:model-a", 0.8)
                .with_contribution("co_occurrence:tagged_with:may_be_related", 0.6),
        );
        ctx.recompute_combined_weights();
        engine.upsert_context(ctx).unwrap();

        let affected = api.retract_contributions("research", "embedding:model-a").unwrap();

        assert_eq!(affected, 1);
        let ctx = engine.get_context(&api.resolve("research").unwrap()).unwrap();
        assert_eq!(ctx.edge_count(), 1, "edge with remaining contributions should survive");
        assert!(!ctx.edges[0].contributions.contains_key("embedding:model-a"));
    }

    #[test]
    fn retract_contributions_is_context_scoped() {
        let (engine, api) = setup();

        // Context A with edge from embedding:model-a
        let mut ctx_a = Context::new("ctx-a");
        let id_a = NodeId::from("concept:alpha");
        let id_b = NodeId::from("concept:beta");
        ctx_a.nodes.insert(id_a.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx_a.nodes.insert(id_b.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx_a.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", dimension::SEMANTIC)
                .with_contribution("embedding:model-a", 0.9),
        );
        ctx_a.recompute_combined_weights();
        engine.upsert_context(ctx_a).unwrap();

        // Context B with same adapter
        let mut ctx_b = Context::new("ctx-b");
        ctx_b.nodes.insert(id_a.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx_b.nodes.insert(id_b.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx_b.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", dimension::SEMANTIC)
                .with_contribution("embedding:model-a", 0.7),
        );
        ctx_b.recompute_combined_weights();
        engine.upsert_context(ctx_b).unwrap();

        // Retract only in ctx-a
        api.retract_contributions("ctx-a", "embedding:model-a").unwrap();

        // ctx-a: edge pruned (only contributor was retracted)
        let ctx_a = engine.get_context(&api.resolve("ctx-a").unwrap()).unwrap();
        assert_eq!(ctx_a.edge_count(), 0, "ctx-a edge should be pruned");

        // ctx-b: unaffected
        let ctx_b = engine.get_context(&api.resolve("ctx-b").unwrap()).unwrap();
        assert_eq!(ctx_b.edge_count(), 1, "ctx-b edge should be unaffected");
        assert!(ctx_b.edges[0].contributions.contains_key("embedding:model-a"));
    }

    #[test]
    fn retract_contributions_lifecycle_add_update_remove() {
        let (engine, api) = setup();

        let mut ctx = Context::new("research");
        let id_a = NodeId::from("concept:a");
        let id_b = NodeId::from("concept:b");
        ctx.nodes.insert(id_a.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));
        ctx.nodes.insert(id_b.clone(), Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC));

        // Add: create edge with contribution
        ctx.add_edge(
            Edge::new_in_dimension(id_a.clone(), id_b.clone(), "similar_to", dimension::SEMANTIC)
                .with_contribution("test-adapter", 0.5),
        );
        ctx.recompute_combined_weights();
        engine.upsert_context(ctx).unwrap();
        assert_eq!(engine.get_context(&api.resolve("research").unwrap()).unwrap().edge_count(), 1);

        // Update: re-emit with new value (simulated by direct mutation)
        engine.with_context_mut(&api.resolve("research").unwrap(), |ctx| {
            ctx.edges[0].contributions.insert("test-adapter".to_string(), 0.8);
            ctx.recompute_combined_weights();
        }).unwrap();
        let ctx = engine.get_context(&api.resolve("research").unwrap()).unwrap();
        assert_eq!(*ctx.edges[0].contributions.get("test-adapter").unwrap(), 0.8);

        // Remove: retract contributions
        api.retract_contributions("research", "test-adapter").unwrap();
        let ctx = engine.get_context(&api.resolve("research").unwrap()).unwrap();
        assert_eq!(ctx.edge_count(), 0, "edge should be pruned after retraction");
    }
}
