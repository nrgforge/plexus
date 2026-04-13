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

use crate::adapter::sink::{EngineSink, FrameworkContext, AdapterError};
use crate::adapter::enrichment::{Enrichment, EnrichmentRegistry};
use crate::graph::events::GraphEvent;
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::OutboundEvent;
use crate::graph::{ContextId, PlexusEngine};
use std::path::Path;
use std::sync::{Arc, RwLock};

/// The unified ingest pipeline.
///
/// All graph writes go through this pipeline. Consumers call `ingest()`
/// with domain data; the pipeline routes to adapters, runs enrichments,
/// and returns domain-meaningful outbound events.
///
/// Interior mutability (ADR-037 §5): the adapter vector and enrichment
/// registry are behind `RwLock` to support runtime registration via
/// `load_spec`. Core routing logic (`ingest`, `ingest_with_adapter`)
/// takes read locks briefly to snapshot references, then releases before
/// doing any work — no lock is held across an `ingest()` call.
pub struct IngestPipeline {
    engine: Arc<PlexusEngine>,
    adapters: RwLock<Vec<Arc<dyn Adapter>>>,
    enrichments: RwLock<Arc<EnrichmentRegistry>>,
}

impl IngestPipeline {
    /// Create a pipeline with no adapters or enrichments.
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        Self {
            engine,
            adapters: RwLock::new(Vec::new()),
            enrichments: RwLock::new(Arc::new(EnrichmentRegistry::empty())),
        }
    }

    /// Register an adapter.
    pub fn register_adapter(&self, adapter: Arc<dyn Adapter>) {
        self.adapters.write().expect("adapters lock poisoned").push(adapter);
    }

    /// Set the enrichment registry (builder-time bulk replacement).
    pub fn with_enrichments(self, registry: Arc<EnrichmentRegistry>) -> Self {
        *self.enrichments.write().expect("enrichments lock poisoned") = registry;
        self
    }

    /// Register an integration: an adapter bundled with its enrichments.
    ///
    /// Enrichments are deduplicated by `id()` across all integrations.
    pub fn register_integration(
        &self,
        adapter: Arc<dyn Adapter>,
        enrichments: Vec<Arc<dyn Enrichment>>,
    ) {
        self.adapters.write().expect("adapters lock poisoned").push(adapter);
        let mut enrichment_lock = self.enrichments.write().expect("enrichments lock poisoned");
        let mut all: Vec<Arc<dyn Enrichment>> = enrichment_lock.enrichments().to_vec();
        all.extend(enrichments);
        *enrichment_lock = Arc::new(EnrichmentRegistry::new(all));
    }

    /// Deregister an adapter by ID (ADR-037 §6 — unload_spec).
    ///
    /// Removes the first adapter with the matching `id()`. If no adapter
    /// matches, this is a no-op.
    pub fn deregister_adapter(&self, adapter_id: &str) {
        let mut adapters = self.adapters.write().expect("adapters lock poisoned");
        if let Some(pos) = adapters.iter().position(|a| a.id() == adapter_id) {
            adapters.remove(pos);
        }
    }

    /// Deregister an enrichment by ID (ADR-037 §6 — unload_spec).
    ///
    /// Rebuilds the enrichment registry without the matching enrichment.
    /// If no enrichment matches, this is a no-op.
    pub fn deregister_enrichment(&self, enrichment_id: &str) {
        let mut enrichment_lock = self.enrichments.write().expect("enrichments lock poisoned");
        let filtered: Vec<Arc<dyn Enrichment>> = enrichment_lock.enrichments().iter()
            .filter(|e| e.id() != enrichment_id)
            .cloned()
            .collect();
        *enrichment_lock = Arc::new(EnrichmentRegistry::new(filtered));
    }

    /// Get the enrichment registry (for running enrichment loop outside ingest).
    pub fn enrichment_registry(&self) -> Arc<EnrichmentRegistry> {
        self.enrichments.read().expect("enrichments lock poisoned").clone()
    }

    /// List the input kinds handled by registered adapters.
    pub fn registered_input_kinds(&self) -> Vec<String> {
        self.adapters.read().expect("adapters lock poisoned")
            .iter().map(|a| a.input_kind().to_string()).collect()
    }

    /// Load adapter specs from a directory and register each (ADR-028).
    ///
    /// Scans `dir` for `*.yaml` files, parses each as a `DeclarativeSpec`,
    /// validates it, optionally attaches the llm-orc client, and registers
    /// the resulting adapter. Returns the count of successfully loaded specs.
    /// Invalid specs are logged to stderr and skipped.
    pub fn register_specs_from_dir(
        &self,
        dir: &Path,
        llm_client: Option<Arc<dyn crate::llm_orc::LlmOrcClient>>,
    ) -> usize {
        use crate::adapter::declarative::DeclarativeAdapter;

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "adapter-specs: cannot read directory");
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
                    tracing::warn!(path = %path.display(), error = %e, "adapter-specs: cannot read file");
                    continue;
                }
            };

            let adapter = match DeclarativeAdapter::from_yaml(&yaml) {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "adapter-specs: invalid spec");
                    continue;
                }
            };

            let adapter = if let Some(ref client) = llm_client {
                adapter.with_llm_client(client.clone())
            } else {
                adapter
            };

            // Extract enrichments and lens before wrapping the adapter in Arc
            let mut spec_enrichments = match adapter.enrichments() {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "adapter-specs: failed to extract enrichments");
                    continue;
                }
            };
            if let Some(lens) = adapter.lens() {
                spec_enrichments.push(lens);
            }

            tracing::info!(
                path = %path.display(),
                input_kind = %adapter.input_kind(),
                enrichment_count = spec_enrichments.len(),
                "adapter-specs: registered spec"
            );
            self.register_integration(Arc::new(adapter), spec_enrichments);
            count += 1;
        }

        count
    }

    /// Ingest with an explicit adapter, skipping input_kind routing.
    ///
    /// Same pipeline steps as `ingest()` but uses the provided adapter
    /// directly instead of routing by input_kind. Used for dynamic
    /// adapters (e.g., one per algorithm in graph analysis).
    pub async fn ingest_with_adapter(
        &self,
        context_id: &str,
        adapter: Arc<dyn Adapter>,
        data: Box<dyn std::any::Any + Send + Sync>,
    ) -> Result<Vec<OutboundEvent>, AdapterError> {
        let ctx_id = ContextId::from(context_id);

        // Verify context exists
        if self.engine.get_context(&ctx_id).is_none() {
            return Err(AdapterError::ContextNotFound(context_id.to_string()));
        }

        let input = AdapterInput::from_boxed(adapter.input_kind(), data, context_id);

        // Step 1: Process the adapter
        let sink = EngineSink::for_engine(self.engine.clone(), ctx_id.clone())
            .with_framework_context(FrameworkContext {
                adapter_id: adapter.id().to_string(),
                context_id: context_id.to_string(),
                input_summary: None,
            });

        adapter.process(&input, &sink).await?;
        let mut all_events: Vec<GraphEvent> = sink.drain_events();

        // Step 2: Enrichment loop — snapshot the registry, release lock before work
        let enrichments = self.enrichment_registry();
        if !enrichments.enrichments().is_empty() && !all_events.is_empty() {
            let enrichment_result = crate::adapter::enrichment::run_enrichment_loop(
                &self.engine,
                &ctx_id,
                &enrichments,
                &all_events,
            )?;
            tracing::debug!(
                rounds = enrichment_result.rounds,
                quiesced = enrichment_result.quiesced,
                "enrichment loop complete"
            );
            all_events.extend(enrichment_result.result.events);
        }

        // Step 3: Transform events
        let snapshot = self
            .engine
            .get_context(&ctx_id)
            .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;

        let outbound = adapter.transform_events(&all_events, &snapshot);

        Ok(outbound)
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

        // Step 1: Find matching adapters — snapshot refs, release read lock
        let matching: Vec<Arc<dyn Adapter>> = {
            let adapters = self.adapters.read().expect("adapters lock poisoned");
            adapters.iter()
                .filter(|a| a.input_kind() == input_kind)
                .cloned()
                .collect()
        };

        tracing::debug!(
            input_kind,
            adapter_count = matching.len(),
            "routing ingest"
        );

        if matching.is_empty() {
            return Err(AdapterError::Internal(format!(
                "no adapter registered for input_kind '{}'",
                input_kind
            )));
        }

        // Step 2: Process each adapter, collecting events (no lock held)
        let mut all_events: Vec<GraphEvent> = Vec::new();
        for adapter in &matching {
            let sink = EngineSink::for_engine(self.engine.clone(), ctx_id.clone())
                .with_framework_context(FrameworkContext {
                    adapter_id: adapter.id().to_string(),
                    context_id: context_id.to_string(),
                    input_summary: None,
                });

            adapter.process(&input, &sink).await?;
            all_events.extend(sink.drain_events());
        }

        // Step 3: Enrichment loop — snapshot registry, release lock before work
        let enrichments = self.enrichment_registry();
        if !enrichments.enrichments().is_empty() && !all_events.is_empty() {
            let enrichment_result = crate::adapter::enrichment::run_enrichment_loop(
                &self.engine,
                &ctx_id,
                &enrichments,
                &all_events,
            )?;
            tracing::debug!(
                rounds = enrichment_result.rounds,
                quiesced = enrichment_result.quiesced,
                "enrichment loop complete"
            );
            all_events.extend(enrichment_result.result.events);
        }

        // Step 4: Transform events through each matched adapter (no lock held)
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
