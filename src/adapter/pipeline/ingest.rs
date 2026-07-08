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
use crate::llm_orc::LlmOrcClient;
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
    enrichments: Arc<RwLock<Arc<EnrichmentRegistry>>>,
    /// Optional llm-orc client. When present, attached to declarative
    /// adapters (with `ensemble:` field) at `load_spec` time and shared
    /// with the built-in SemanticAdapter wired by `with_llm_client` on
    /// the builder. Stored as a single Option so all consumers of the
    /// pipeline see the same client without duplication.
    pub(crate) llm_client: Option<Arc<dyn LlmOrcClient>>,
    /// Spec rows already examined by `sync_spec_lenses`, keyed by
    /// `(context_id, adapter_id, loaded_at)` and mapping to the lens id
    /// the row contributed (None if the spec had no lens or failed to
    /// parse). A re-loaded spec (new `loaded_at`) is re-examined;
    /// unchanged rows are not re-parsed on every ingest; a vanished row
    /// (unload_spec in another process, issue #11) deregisters its lens
    /// when no other examined row still references it.
    synced_specs: RwLock<std::collections::HashMap<(String, String, String), Option<String>>>,
}

impl IngestPipeline {
    /// Create a pipeline with no adapters or enrichments.
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        Self {
            engine,
            adapters: RwLock::new(Vec::new()),
            enrichments: Arc::new(RwLock::new(Arc::new(EnrichmentRegistry::empty()))),
            llm_client: None,
            synced_specs: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Sync lens enrichments from the context's specs table before an
    /// ingest (Invariant 62 across processes). A spec loaded by another
    /// consumer in another process persists a row this pipeline has never
    /// seen; without this sync, that consumer's lens would not fire on
    /// emissions from this process. Lens-only, mirroring rehydration
    /// (`with_persisted_specs`): adapter wiring stays transient to the
    /// loading consumer's own process.
    ///
    /// Failures are logged and non-fatal — the same availability-over-
    /// strictness stance as rehydration.
    fn sync_spec_lenses(&self, context_id: &str) {
        use crate::adapter::declarative::DeclarativeAdapter;

        let specs = match self.engine.query_specs_for_context(context_id) {
            Ok(specs) => specs,
            Err(e) => {
                tracing::warn!(context_id, error = %e, "spec-lens sync: query failed, skipping");
                return;
            }
        };

        let current: std::collections::HashSet<(String, String, String)> = specs
            .iter()
            .map(|s| (s.context_id.clone(), s.adapter_id.clone(), s.loaded_at.clone()))
            .collect();

        for spec in &specs {
            let marker = (
                spec.context_id.clone(),
                spec.adapter_id.clone(),
                spec.loaded_at.clone(),
            );
            if self
                .synced_specs
                .read()
                .expect("synced_specs lock poisoned")
                .contains_key(&marker)
            {
                continue;
            }

            let mut lens_id: Option<String> = None;
            match DeclarativeAdapter::from_yaml(&spec.spec_yaml) {
                Ok(adapter) => {
                    if let Some(lens) = adapter.lens() {
                        lens_id = Some(lens.id().to_string());
                        let mut lock =
                            self.enrichments.write().expect("enrichments lock poisoned");
                        let already = lock.enrichments().iter().any(|e| e.id() == lens.id());
                        if !already {
                            tracing::info!(
                                context_id = %spec.context_id,
                                adapter_id = %spec.adapter_id,
                                lens_id = %lens.id(),
                                "spec-lens sync: registered lens loaded by another consumer"
                            );
                            let mut all: Vec<Arc<dyn Enrichment>> = lock.enrichments().to_vec();
                            all.push(lens);
                            *lock = Arc::new(EnrichmentRegistry::new(all));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        context_id = %spec.context_id,
                        adapter_id = %spec.adapter_id,
                        error = %e,
                        "spec-lens sync: failed to parse persisted spec, skipping"
                    );
                }
            }

            // Mark examined either way — a parse failure won't heal by
            // re-parsing the same row on every ingest.
            self.synced_specs
                .write()
                .expect("synced_specs lock poisoned")
                .insert(marker, lens_id);
        }

        // Reverse direction (issue #11): rows for this context that we
        // examined earlier but no longer exist were unloaded elsewhere.
        // Deregister their lenses unless another examined row (any
        // context) still references the same lens id.
        let vanished: Vec<((String, String, String), Option<String>)> = {
            let markers = self.synced_specs.read().expect("synced_specs lock poisoned");
            markers
                .iter()
                .filter(|(k, _)| k.0 == context_id && !current.contains(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };
        if !vanished.is_empty() {
            let mut markers = self.synced_specs.write().expect("synced_specs lock poisoned");
            for (key, _) in &vanished {
                markers.remove(key);
            }
            for (key, lens_id) in vanished {
                let Some(lens_id) = lens_id else { continue };
                let still_referenced = markers.values().any(|v| v.as_deref() == Some(&lens_id));
                if !still_referenced {
                    tracing::info!(
                        context_id = %key.0,
                        adapter_id = %key.1,
                        lens_id = %lens_id,
                        "spec-lens sync: deregistered lens unloaded by another consumer"
                    );
                    drop(markers);
                    self.deregister_enrichment(&lens_id);
                    markers = self.synced_specs.write().expect("synced_specs lock poisoned");
                }
            }
        }
    }

    /// Get the configured llm-orc client, if any.
    ///
    /// Used by `PlexusApi::load_spec` to attach the client to declarative
    /// adapters whose specs declare an `ensemble:` field.
    pub fn llm_client(&self) -> Option<Arc<dyn LlmOrcClient>> {
        self.llm_client.clone()
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

    /// Shared handle to the live enrichment registry cell. Handed to the
    /// ExtractionCoordinator at build time (issue #5) so background
    /// extraction phases run the enrichment loop with whatever
    /// enrichments — including runtime-loaded and spec-synced lenses —
    /// the pipeline holds at that moment.
    pub(crate) fn enrichment_cell(&self) -> Arc<RwLock<Arc<EnrichmentRegistry>>> {
        self.enrichments.clone()
    }

    /// List the input kinds handled by registered adapters.
    pub fn registered_input_kinds(&self) -> Vec<String> {
        self.adapters.read().expect("adapters lock poisoned")
            .iter().map(|a| a.input_kind().to_string()).collect()
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

        // Invariant 62 across processes — same sync as `ingest()`.
        self.sync_spec_lenses(context_id);

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

        // Invariant 62 across processes: pick up lenses other consumers
        // loaded onto this context since this pipeline was constructed.
        self.sync_spec_lenses(context_id);

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
