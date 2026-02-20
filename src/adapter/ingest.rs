//! Unified ingest pipeline (ADR-012)
//!
//! Single write endpoint: `ingest(context_id, input_kind, data) -> Vec<OutboundEvent>`.
//!
//! Pipeline steps:
//! 1. Route to matching adapter(s) by input_kind
//! 2. Each adapter processes via sink → primary events
//! 3. Enrichment loop runs once (globally) until quiescence
//! 4. Each adapter transforms all accumulated events → outbound events
//! 5. Return merged outbound events

use super::engine_sink::EngineSink;
use super::enrichment::{Enrichment, EnrichmentRegistry};
use super::events::GraphEvent;
use super::provenance::FrameworkContext;
use super::sink::AdapterError;
use super::traits::{Adapter, AdapterInput};
use super::types::OutboundEvent;
use crate::graph::{ContextId, PlexusEngine};
use std::path::Path;
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

    /// Get the enrichment registry (for running enrichment loop outside ingest).
    pub fn enrichment_registry(&self) -> &Arc<EnrichmentRegistry> {
        &self.enrichments
    }

    /// List the input kinds handled by registered adapters.
    pub fn registered_input_kinds(&self) -> Vec<&str> {
        self.adapters.iter().map(|a| a.input_kind()).collect()
    }

    /// Load adapter specs from a directory and register each (ADR-028).
    ///
    /// Scans `dir` for `*.yaml` files, parses each as a `DeclarativeSpec`,
    /// validates it, optionally attaches the llm-orc client, and registers
    /// the resulting adapter. Returns the count of successfully loaded specs.
    /// Invalid specs are logged to stderr and skipped.
    pub fn register_specs_from_dir(
        &mut self,
        dir: &Path,
        llm_client: Option<Arc<dyn crate::llm_orc::LlmOrcClient>>,
    ) -> usize {
        use super::declarative::DeclarativeAdapter;

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("adapter-specs: cannot read {}: {}", dir.display(), e);
                return 0;
            }
        };

        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }

            let yaml = match std::fs::read_to_string(&path) {
                Ok(y) => y,
                Err(e) => {
                    eprintln!("adapter-specs: cannot read {}: {}", path.display(), e);
                    continue;
                }
            };

            let adapter = match DeclarativeAdapter::from_yaml(&yaml) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("adapter-specs: invalid spec {}: {}", path.display(), e);
                    continue;
                }
            };

            let adapter = if let Some(ref client) = llm_client {
                adapter.with_llm_client(client.clone())
            } else {
                adapter
            };

            eprintln!(
                "adapter-specs: registered {} (input_kind={})",
                path.display(),
                adapter.input_kind()
            );
            self.register_adapter(Arc::new(adapter));
            count += 1;
        }

        count
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
