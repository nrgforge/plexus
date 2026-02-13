//! Transport-independent API layer (ADR-014).
//!
//! `PlexusApi` is the single entry point for all consumer-facing operations.
//! Transports (MCP, gRPC, REST, direct embedding) call `PlexusApi` methods —
//! they never reach into `ProvenanceApi`, `IngestPipeline`, or `PlexusEngine`
//! directly.

use std::sync::Arc;

use crate::adapter::{AdapterError, IngestPipeline, OutboundEvent};
use crate::graph::{
    Context, ContextId, PlexusEngine, PlexusError, PlexusResult, Source,
};
use crate::provenance::{ChainView, MarkView, ProvenanceApi};
use crate::query::{
    self, EvidenceTrailResult, FindQuery, PathQuery, PathResult, QueryResult,
    TraversalResult, TraverseQuery,
};

/// Single entry point for all consumer-facing operations.
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

    // --- Provenance mutations (non-ingest, routed directly to ProvenanceApi) ---

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

    // --- Context management ---

    /// Create or upsert a context.
    pub fn context_create(&self, name: &str) -> PlexusResult<ContextId> {
        let context = Context::new(name);
        self.engine.upsert_context(context)
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

    /// Rename a context.
    pub fn context_rename(&self, old_name: &str, new_name: &str) -> PlexusResult<()> {
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

    fn prov(&self, context_name: &str) -> PlexusResult<ProvenanceApi> {
        let cid = self.resolve(context_name)?;
        Ok(ProvenanceApi::new(&self.engine, cid))
    }
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
}
