//! Unified ingest pipeline (ADR-012)
//!
//! Single write endpoint: `ingest(context_id, input_kind, data) -> Vec<OutboundEvent>`.
//!
//! Pipeline steps:
//! 1. Route to matching adapter(s) by input_kind
//! 2. Each adapter processes via sink → primary events
//! 2.5. Auto-provenance: create ingest_record nodes with produced_by edges
//! 3. Enrichment loop runs once (globally) until quiescence
//! 4. Each adapter transforms all accumulated events → outbound events
//! 5. Return merged outbound events

use super::engine_sink::EngineSink;
use super::enrichment::{Enrichment, EnrichmentRegistry};
use super::events::GraphEvent;
use super::provenance::FrameworkContext;
use super::sink::{AdapterError, AdapterSink};
use super::traits::{Adapter, AdapterInput};
use super::types::{Emission, OutboundEvent};
use crate::graph::{dimension, ContentType, Edge, Node, NodeId, PropertyValue};
use crate::graph::{ContextId, PlexusEngine};
use std::sync::Arc;

/// The unified ingest pipeline.
///
/// All graph writes go through this pipeline. Consumers call `ingest()`
/// with domain data; the pipeline routes to adapters, runs enrichments,
/// and returns domain-meaningful outbound events.
pub struct IngestPipeline {
    engine: Arc<PlexusEngine>,
    adapters: Vec<Arc<dyn Adapter>>,
    enrichments: Arc<EnrichmentRegistry>,
}

impl IngestPipeline {
    /// Create a pipeline with no adapters or enrichments.
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        Self {
            engine,
            adapters: Vec::new(),
            enrichments: Arc::new(EnrichmentRegistry::empty()),
        }
    }

    /// Register an adapter.
    pub fn register_adapter(&mut self, adapter: Arc<dyn Adapter>) {
        self.adapters.push(adapter);
    }

    /// Set the enrichment registry.
    pub fn with_enrichments(mut self, registry: Arc<EnrichmentRegistry>) -> Self {
        self.enrichments = registry;
        self
    }

    /// Register an integration: an adapter bundled with its enrichments.
    ///
    /// Enrichments are deduplicated by `id()` across all integrations.
    pub fn register_integration(
        &mut self,
        adapter: Arc<dyn Adapter>,
        enrichments: Vec<Arc<dyn Enrichment>>,
    ) {
        self.adapters.push(adapter);
        let mut all: Vec<Arc<dyn Enrichment>> = self.enrichments.enrichments().to_vec();
        all.extend(enrichments);
        self.enrichments = Arc::new(EnrichmentRegistry::new(all));
    }

    /// The single write endpoint (ADR-012).
    ///
    /// 1. Routes to adapters matching `input_kind` (fan-out if multiple)
    /// 2. Each adapter processes via its own sink → primary events
    /// 3. Enrichment loop runs once with combined events
    /// 4. Each adapter's `transform_events()` translates all events
    /// 5. Returns merged outbound events
    pub async fn ingest(
        &self,
        context_id: &str,
        input_kind: &str,
        data: Box<dyn std::any::Any + Send + Sync>,
    ) -> Result<Vec<OutboundEvent>, AdapterError> {
        let ctx_id = ContextId::from(context_id);

        // Verify context exists
        if self.engine.get_context(&ctx_id).is_none() {
            return Err(AdapterError::ContextNotFound(context_id.to_string()));
        }

        let input = AdapterInput::from_boxed(input_kind, data, context_id);

        // Step 1: Find matching adapters
        let matching: Vec<&Arc<dyn Adapter>> = self
            .adapters
            .iter()
            .filter(|a| a.input_kind() == input_kind)
            .collect();

        if matching.is_empty() {
            return Err(AdapterError::Internal(format!(
                "no adapter registered for input_kind '{}'",
                input_kind
            )));
        }

        // Step 2: Process each adapter, collecting events
        let mut all_events: Vec<GraphEvent> = Vec::new();
        for adapter in &matching {
            let sink = EngineSink::for_engine(self.engine.clone(), ctx_id.clone())
                .with_framework_context(FrameworkContext {
                    adapter_id: adapter.id().to_string(),
                    context_id: context_id.to_string(),
                    input_summary: None,
                });

            adapter.process(&input, &sink).await?;
            all_events.extend(sink.take_accumulated_events());
        }

        // Step 2.5: Auto-provenance — create ingest_record nodes for each adapter's emission
        //
        // For every adapter that committed nodes, create a provenance-dimension
        // "ingest_record" node and cross-dimensional produced_by edges from each
        // committed node to the record. This makes the origin of all graph knowledge
        // traversable through the graph itself.
        {
            let snapshot = self.engine.get_context(&ctx_id)
                .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;
            let timestamp = chrono::Utc::now();

            for adapter in &matching {
                let adapter_id = adapter.id().to_string();

                // Collect node IDs committed by this adapter
                let committed_node_ids: Vec<NodeId> = all_events.iter()
                    .filter_map(|e| match e {
                        GraphEvent::NodesAdded { node_ids, adapter_id: aid, .. }
                            if *aid == adapter_id => Some(node_ids.clone()),
                        _ => None,
                    })
                    .flatten()
                    .collect();

                if committed_node_ids.is_empty() {
                    continue;
                }

                // Create the ingest record node
                let record_id = NodeId::from_string(
                    format!("ingest:{}:{}", adapter_id, uuid::Uuid::new_v4())
                );
                let mut record_node = Node::new_in_dimension(
                    "ingest_record",
                    ContentType::Provenance,
                    dimension::PROVENANCE,
                );
                record_node.id = record_id.clone();
                record_node.properties.insert(
                    "adapter_id".to_string(),
                    PropertyValue::String(adapter_id.clone()),
                );
                record_node.properties.insert(
                    "timestamp".to_string(),
                    PropertyValue::String(timestamp.to_rfc3339()),
                );
                record_node.properties.insert(
                    "context_id".to_string(),
                    PropertyValue::String(context_id.to_string()),
                );
                record_node.properties.insert(
                    "node_count".to_string(),
                    PropertyValue::Int(committed_node_ids.len() as i64),
                );

                let mut emission = Emission::new().with_node(record_node);

                // Create produced_by edges from each committed node to the record
                for node_id in &committed_node_ids {
                    if let Some(node) = snapshot.get_node(node_id) {
                        let edge = Edge::new_cross_dimensional(
                            node_id.clone(),
                            &node.dimension,
                            record_id.clone(),
                            dimension::PROVENANCE,
                            "produced_by",
                        );
                        emission = emission.with_edge(edge);
                    }
                }

                // Emit through a provenance-specific sink
                let provenance_sink = EngineSink::for_engine(
                    self.engine.clone(),
                    ctx_id.clone(),
                ).with_framework_context(FrameworkContext {
                    adapter_id: "plexus:auto-provenance".to_string(),
                    context_id: context_id.to_string(),
                    input_summary: Some(format!("ingest record for {}", adapter_id)),
                });
                provenance_sink.emit(emission).await?;
                all_events.extend(provenance_sink.take_accumulated_events());
            }
        }

        // Step 3: Enrichment loop runs once with combined events
        if !self.enrichments.enrichments().is_empty() && !all_events.is_empty() {
            let enrichment_result = EngineSink::run_enrichment_loop(
                &self.engine,
                &ctx_id,
                &self.enrichments,
                &all_events,
            )?;
            all_events.extend(enrichment_result.events);
        }

        // Step 4: Transform events through each matched adapter
        let snapshot = self
            .engine
            .get_context(&ctx_id)
            .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;

        let mut outbound = Vec::new();
        for adapter in &matching {
            outbound.extend(adapter.transform_events(&all_events, &snapshot));
        }

        Ok(outbound)
    }
}
