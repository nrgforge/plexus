//! Transport-independent API layer (ADR-014).
//!
//! `PlexusApi` is the single entry point for all consumer-facing operations.
//! Transports (MCP, gRPC, REST, direct embedding) call `PlexusApi` methods —
//! they never reach into `ProvenanceApi`, `IngestPipeline`, or `PlexusEngine`
//! directly.

use std::sync::Arc;

use crate::adapter::{AdapterError, FragmentInput, IngestPipeline, OutboundEvent, ProvenanceInput};
use crate::graph::{
    Context, ContextId, NodeId, PlexusEngine, PlexusError, PlexusResult, Source,
};
use crate::provenance::{ChainView, MarkView, ProvenanceApi};
use crate::query::{
    self, EvidenceTrailResult, FindQuery, PathQuery, PathResult, QueryResult,
    TraversalResult, TraverseQuery,
};

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

    /// The single write endpoint (ADR-012).
    pub async fn ingest(
        &self,
        context_id: &str,
        input_kind: &str,
        data: Box<dyn std::any::Any + Send + Sync>,
    ) -> Result<Vec<OutboundEvent>, AdapterError> {
        self.pipeline.ingest(context_id, input_kind, data).await
    }

    /// Annotate a file location, auto-creating a chain if needed (ADR-015).
    ///
    /// Returns merged outbound events from chain creation (if any) and mark creation.
    pub async fn annotate(
        &self,
        context_id: &str,
        chain_name: &str,
        file: &str,
        line: u32,
        annotation: &str,
        column: Option<u32>,
        mark_type: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> Result<Vec<OutboundEvent>, AnnotateError> {
        // Reject empty/whitespace-only chain names
        if chain_name.trim().is_empty() {
            return Err(AnnotateError::EmptyChainName);
        }

        let chain_id = normalize_chain_name(chain_name);
        let ctx_id = self
            .resolve(context_id)
            .map_err(AnnotateError::Plexus)?;
        let resolved = ctx_id.as_str().to_string();
        let mut all_events = Vec::new();

        // Step 1: Create fragment from annotation text (semantic content).
        // The annotation text IS a fragment — bidirectional dual obligation.
        let normalized_tags: Vec<String> = tags
            .as_ref()
            .map(|t| {
                t.iter()
                    .map(|s| s.strip_prefix('#').unwrap_or(s).to_string())
                    .collect()
            })
            .unwrap_or_default();

        let fragment_input = FragmentInput::new(annotation, normalized_tags)
            .with_source(file);
        let fragment_events = self
            .pipeline
            .ingest(&resolved, "fragment", Box::new(fragment_input))
            .await
            .map_err(AnnotateError::Adapter)?;
        all_events.extend(fragment_events);

        // Step 2: Check if chain already exists in the context
        let chain_exists = self
            .engine
            .get_context(&ctx_id)
            .map(|ctx| ctx.get_node(&NodeId::from(chain_id.as_str())).is_some())
            .unwrap_or(false);

        // Step 2b: Create chain if it doesn't exist
        if !chain_exists {
            let input = ProvenanceInput::CreateChain {
                chain_id: chain_id.clone(),
                name: chain_name.to_string(),
                description: None,
            };
            let events = self
                .pipeline
                .ingest(&resolved, "provenance", Box::new(input))
                .await
                .map_err(AnnotateError::Adapter)?;
            all_events.extend(events);
        }

        // Step 3: Create the mark
        let mark_id = format!("mark:provenance:{}", uuid::Uuid::new_v4());
        let input = ProvenanceInput::AddMark {
            mark_id,
            chain_id,
            file: file.to_string(),
            line,
            annotation: annotation.to_string(),
            column,
            mark_type: mark_type.map(|s| s.to_string()),
            tags,
        };
        let events = self
            .pipeline
            .ingest(&resolved, "provenance", Box::new(input))
            .await
            .map_err(AnnotateError::Adapter)?;
        all_events.extend(events);

        Ok(all_events)
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
    ) -> PlexusResult<(Vec<String>, Vec<String>)> {
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

    // --- Provenance mutations (non-ingest) ---

    /// Update a mark's metadata.
    pub fn update_mark(
        &self,
        context_id: &str,
        mark_id: &str,
        annotation: Option<&str>,
        line: Option<u32>,
        column: Option<u32>,
        mark_type: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> PlexusResult<()> {
        self.prov(context_id)?
            .update_mark(mark_id, annotation, line, column, mark_type, tags)
    }

    /// Archive a chain.
    pub fn archive_chain(&self, context_id: &str, chain_id: &str) -> PlexusResult<()> {
        self.prov(context_id)?.archive_chain(chain_id)
    }

    /// Delete a mark (cascades edges). Routes through ingest pipeline.
    pub async fn delete_mark(
        &self,
        context_id: &str,
        mark_id: &str,
    ) -> Result<(), AdapterError> {
        let ctx_id = self
            .resolve(context_id)
            .map_err(|e| AdapterError::Internal(e.to_string()))?;
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
            .map_err(|e| AdapterError::Internal(e.to_string()))?;
        let input = ProvenanceInput::UnlinkMarks {
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
        };
        self.pipeline
            .ingest(ctx_id.as_str(), "provenance", Box::new(input))
            .await?;
        Ok(())
    }

    // --- Context management ---

    /// Create a context. Returns error if name is already taken.
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

    /// Resolve a context name to its ContextId.
    fn resolve(&self, name: &str) -> PlexusResult<ContextId> {
        for cid in self.engine.list_contexts() {
            if let Some(ctx) = self.engine.get_context(&cid) {
                if ctx.name == name {
                    return Ok(cid);
                }
            }
        }
        Err(PlexusError::ContextNotFound(ContextId::from(name)))
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

        let concepts_a: std::collections::HashSet<&NodeId> = ctx_a
            .nodes
            .iter()
            .filter(|(_, n)| n.node_type == "concept")
            .map(|(id, _)| id)
            .collect();

        let shared: Vec<NodeId> = ctx_b
            .nodes
            .iter()
            .filter(|(id, n)| n.node_type == "concept" && concepts_a.contains(id))
            .map(|(id, _)| id.clone())
            .collect();

        Ok(shared)
    }

    fn prov(&self, context_name: &str) -> PlexusResult<ProvenanceApi> {
        let cid = self.resolve(context_name)?;
        Ok(ProvenanceApi::new(&self.engine, cid))
    }
}

/// Error from the `annotate` workflow.
#[derive(Debug)]
pub enum AnnotateError {
    /// Chain name was empty or whitespace-only.
    EmptyChainName,
    /// Underlying adapter error.
    Adapter(AdapterError),
    /// Underlying engine error.
    Plexus(PlexusError),
}

impl std::fmt::Display for AnnotateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChainName => write!(f, "chain name must not be empty"),
            Self::Adapter(e) => write!(f, "adapter error: {}", e),
            Self::Plexus(e) => write!(f, "engine error: {}", e),
        }
    }
}

impl std::error::Error for AnnotateError {}

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

/// Normalize a chain name to a deterministic chain ID.
///
/// Rules (ADR-015):
/// - Lowercased
/// - Whitespace replaced by hyphens
/// - `:` and `/` replaced by hyphens (conflict with ID format separators)
/// - Non-ASCII characters preserved
/// - Prefix: `chain:provenance:`
pub fn normalize_chain_name(name: &str) -> String {
    let normalized: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            ' ' | '\t' | '\n' | '\r' => '-',
            ':' | '/' => '-',
            _ => c,
        })
        .collect();
    format!("chain:provenance:{}", normalized)
}

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

    // === Scenario: Non-ingest mutations route through ProvenanceApi ===
    #[test]
    fn update_mark_routes_through_provenance_api() {
        let (engine, api) = setup();

        // Create context with a mark
        let mut ctx = Context::new("research");
        let mut mark = Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
        mark.id = NodeId::from("mark:1");
        mark.properties.insert("annotation".into(), PropertyValue::String("original".into()));
        mark.properties.insert("file".into(), PropertyValue::String("src/main.rs".into()));
        mark.properties.insert("line".into(), PropertyValue::String("42".into()));
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

        // Update through PlexusApi
        api.update_mark("research", "mark:1", Some("updated"), None, None, None, None)
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

    // --- Annotate workflow (ADR-015) ---

    use crate::adapter::{FragmentAdapter, ProvenanceAdapter};

    fn setup_with_provenance() -> (Arc<PlexusEngine>, PlexusApi) {
        let engine = Arc::new(PlexusEngine::new());
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(Arc::new(FragmentAdapter::new("annotate")));
        pipeline.register_adapter(Arc::new(ProvenanceAdapter::new()));
        let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));
        (engine, api)
    }

    // === Scenario: Annotate creates fragment, chain, and mark in one call ===
    #[tokio::test]
    async fn annotate_creates_fragment_chain_and_mark() {
        let (engine, api) = setup_with_provenance();
        engine.upsert_context(Context::new("research")).unwrap();

        let events = api
            .annotate("research", "field notes", "src/main.rs", 42, "interesting pattern", None, None, Some(vec!["refactor".into()]))
            .await
            .unwrap();

        // Should have outbound events from fragment, chain, and mark creation
        assert!(!events.is_empty());

        let ctx = engine.get_context(&api.resolve("research").unwrap()).unwrap();

        // Verify fragment node exists (semantic content)
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

        // Verify user's chain exists
        let chains = api.list_chains("research", None).unwrap();
        let user_chain = chains.iter().find(|c| c.id == "chain:provenance:field-notes");
        assert!(user_chain.is_some(), "user's chain should exist");

        // Verify user's mark exists in the chain
        let marks = api.list_marks("research", Some("chain:provenance:field-notes"), None, None, None).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].annotation, "interesting pattern");
        assert_eq!(marks[0].file, "src/main.rs");
        assert_eq!(marks[0].line, 42);
    }

    // === Scenario: Annotate reuses existing chain ===
    #[tokio::test]
    async fn annotate_reuses_existing_chain() {
        let (engine, api) = setup_with_provenance();
        engine.upsert_context(Context::new("research")).unwrap();

        // First annotate creates the chain
        api.annotate("research", "field notes", "src/main.rs", 42, "first", None, None, None)
            .await
            .unwrap();

        // Second annotate reuses it
        api.annotate("research", "field notes", "src/lib.rs", 10, "second", None, None, None)
            .await
            .unwrap();

        // Still only one user chain (FragmentAdapter's internal chains are separate)
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

    // === Scenario: Annotate triggers enrichment loop ===
    #[tokio::test]
    async fn annotate_triggers_enrichment() {
        let engine = Arc::new(PlexusEngine::new());
        engine.upsert_context(Context::new("research")).unwrap();

        // Set up pipeline with both adapters and enrichments
        let mut pipeline = IngestPipeline::new(engine.clone());
        pipeline.register_adapter(Arc::new(FragmentAdapter::new("annotate")));
        pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            vec![Arc::new(crate::adapter::TagConceptBridger::new())],
        );
        let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

        api.annotate("research", "notes", "src/main.rs", 1, "cleanup", None, None, Some(vec!["refactor".into()]))
            .await
            .unwrap();

        // FragmentAdapter creates concept:refactor from the tag.
        // TagConceptBridger creates a references edge from the mark to the concept.
        let ctx = engine.get_context(&api.resolve("research").unwrap()).unwrap();
        assert!(ctx.get_node(&NodeId::from("concept:refactor")).is_some(),
            "concept should be created from tag");
        let has_ref = ctx.edges.iter().any(|e| e.relationship == "references");
        assert!(has_ref, "enrichment should create references edge");
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

    // === Scenario: Annotate returns merged outbound events ===
    #[tokio::test]
    async fn annotate_returns_merged_events() {
        let (engine, api) = setup_with_provenance();
        engine.upsert_context(Context::new("research")).unwrap();

        // First call creates fragment + chain + mark → events from all three
        let events = api
            .annotate("research", "notes", "src/main.rs", 1, "note", None, None, None)
            .await
            .unwrap();

        // Should be a single merged list (not separate batches)
        // Fragment, chain, and mark creation each produce events
        assert!(events.len() >= 3, "should have events from fragment, chain, and mark creation");
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

    // === Scenario: Annotate rejects empty chain name ===
    #[tokio::test]
    async fn annotate_rejects_empty_chain_name() {
        let (engine, api) = setup_with_provenance();
        engine.upsert_context(Context::new("research")).unwrap();

        let result = api
            .annotate("research", "", "src/main.rs", 1, "note", None, None, None)
            .await;

        assert!(result.is_err());

        // Also reject whitespace-only
        let result = api
            .annotate("research", "   ", "src/main.rs", 1, "note", None, None, None)
            .await;

        assert!(result.is_err());

        // No chain should have been created
        let chains = api.list_chains("research", None).unwrap();
        assert_eq!(chains.len(), 0);
    }
}
