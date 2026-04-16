//! Transport-neutral pipeline construction (ADR-028, Invariant 38).
//!
//! `PipelineBuilder` encapsulates the adapter + enrichment registration logic
//! so that transports (MCP, gRPC, REST) are thin shells that receive a
//! pre-built `IngestPipeline`. The binary entry point is the single
//! construction site.

use super::ingest::IngestPipeline;
use crate::adapter::enrichment::Enrichment;
use crate::adapter::adapters::content::ContentAdapter;
use crate::adapter::adapters::extraction::ExtractionCoordinator;
use crate::adapter::adapters::provenance_adapter::ProvenanceAdapter;
use crate::adapter::adapters::semantic::SemanticAdapter;
use crate::adapter::adapters::structural::{MarkdownStructureModule, StructuralModule};
use crate::adapter::enrichments::cooccurrence::CoOccurrenceEnrichment;
use crate::adapter::enrichments::discovery_gap::DiscoveryGapEnrichment;
use crate::adapter::enrichments::temporal_proximity::TemporalProximityEnrichment;
use crate::graph::PlexusEngine;
use crate::llm_orc::LlmOrcClient;
use crate::storage::PersistedSpec;
use std::sync::Arc;

/// Builder for `IngestPipeline` with standard adapter/enrichment registration.
///
/// Provides a transport-neutral construction API so that MCP, CLI, and
/// embedded consumers all get the same pipeline without duplicating
/// registration logic.
pub struct PipelineBuilder {
    pipeline: IngestPipeline,
    engine: Arc<PlexusEngine>,
    coordinator: Option<ExtractionCoordinator>,
    enrichments: Vec<Arc<dyn Enrichment>>,
}

impl PipelineBuilder {
    /// Start building a pipeline for the given engine.
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        let pipeline = IngestPipeline::new(engine.clone());
        Self {
            pipeline,
            engine,
            coordinator: None,
            enrichments: Vec::new(),
        }
    }

    /// Register the core adapters: ContentAdapter, ExtractionCoordinator, ProvenanceAdapter.
    ///
    /// The `ExtractionCoordinator` is held by the builder until `build()` so
    /// that structural modules can be registered on it via `with_structural_module()`
    /// and the llm-orc client via `with_llm_client()`. The engine is attached
    /// so background structural analysis and semantic extraction can persist
    /// via `EngineSink` (Invariant 30).
    pub fn with_default_adapters(mut self) -> Self {
        self.pipeline.register_adapter(Arc::new(ContentAdapter::new("content")));
        self.coordinator = Some(ExtractionCoordinator::new().with_engine(self.engine.clone()));
        // ProvenanceAdapter is registered via register_integration in build()
        self
    }

    /// Register a structural module on the `ExtractionCoordinator`.
    ///
    /// Must be called after `with_default_adapters()`. Modules are dispatched
    /// by MIME type affinity during extraction (ADR-030).
    pub fn with_structural_module(mut self, module: Arc<dyn StructuralModule>) -> Self {
        if let Some(ref mut coordinator) = self.coordinator {
            coordinator.register_structural_module(module);
        }
        self
    }

    /// Register the default structural modules (currently: MarkdownStructureModule).
    ///
    /// Called automatically by `default_pipeline()`. Consumers who want
    /// different modules can skip this and call `with_structural_module()` directly.
    pub fn with_default_structural_modules(self) -> Self {
        self.with_structural_module(Arc::new(MarkdownStructureModule::new()))
    }

    /// Register the domain-agnostic enrichments.
    ///
    /// Default set: CoOccurrence, DiscoveryGap, TemporalProximity,
    /// and EmbeddingSimilarity (when the `embeddings` feature is enabled).
    pub fn with_default_enrichments(mut self) -> Self {
        self.enrichments.push(Arc::new(CoOccurrenceEnrichment::new()));
        self.enrichments.push(Arc::new(DiscoveryGapEnrichment::new(
            "similar_to",
            "discovery_gap",
        )));
        self.enrichments.push(Arc::new(TemporalProximityEnrichment::new(
            "created_at",
            86_400_000, // 24 hours in ms
            "temporal_proximity",
        )));

        #[cfg(feature = "embeddings")]
        {
            use crate::adapter::enrichments::embedding::{
                EmbeddingSimilarityEnrichment, FastEmbedEmbedder,
            };
            if let Ok(embedder) = FastEmbedEmbedder::default_model() {
                self.enrichments.push(Arc::new(EmbeddingSimilarityEnrichment::new(
                    "nomic-embed-text-v1.5",
                    0.7,
                    "similar_to",
                    Box::new(embedder),
                )));
            }
        }

        self
    }

    /// Add a custom enrichment.
    pub fn with_enrichment(mut self, enrichment: Arc<dyn Enrichment>) -> Self {
        self.enrichments.push(enrichment);
        self
    }

    /// Configure the llm-orc client for the pipeline.
    ///
    /// Two effects:
    /// 1. **Built-in semantic extraction:** registers `SemanticAdapter` on
    ///    the `ExtractionCoordinator`, so `extract-file` ingest performs
    ///    LLM-based concept extraction (the third extraction phase).
    /// 2. **Declarative ensemble support:** the client is stored on the
    ///    pipeline so that `PlexusApi::load_spec` can attach it to any
    ///    `DeclarativeAdapter` whose spec declares an `ensemble:` field.
    ///
    /// Without this call, `extract-file` ingest produces only registration
    /// + structural analysis output, and `load_spec` will reject specs that
    /// declare an ensemble (`AdapterError::Skipped("ensemble declared but
    /// no LlmOrcClient configured")`). `default_pipeline` calls this with
    /// `SubprocessClient::new()` so production hosts get both behaviors by
    /// default; consumers wanting to inject a mock client (for tests) call
    /// this method explicitly with their own client.
    ///
    /// Must be called after `with_default_adapters()`.
    pub fn with_llm_client(mut self, client: Arc<dyn LlmOrcClient>) -> Self {
        self.pipeline.llm_client = Some(client.clone());
        if let Some(ref mut coordinator) = self.coordinator {
            coordinator.register_semantic_extraction(Arc::new(
                SemanticAdapter::new(client, "extract-semantic"),
            ));
        }
        self
    }

    /// Rehydrate persisted lens enrichments at construction time (ADR-037 §2).
    ///
    /// For each persisted spec: parse the YAML, extract the lens enrichment
    /// (if present), and register it on the pipeline. The original adapter is
    /// NOT registered (the loading consumer may not be present), and the lens
    /// is NOT re-run over existing content (vocabulary edges already persist
    /// from the original `load_spec` call — Invariant 62 effect a).
    ///
    /// Failures to parse a single spec are logged and non-fatal — pipeline
    /// construction continues with remaining specs.
    pub fn with_persisted_specs(mut self, specs: Vec<PersistedSpec>) -> Self {
        use crate::adapter::declarative::DeclarativeAdapter;

        for spec in &specs {
            let adapter = match DeclarativeAdapter::from_yaml(&spec.spec_yaml) {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(
                        context_id = %spec.context_id,
                        adapter_id = %spec.adapter_id,
                        error = %e,
                        "persisted spec: failed to parse, skipping"
                    );
                    continue;
                }
            };

            if let Some(lens) = adapter.lens() {
                tracing::info!(
                    context_id = %spec.context_id,
                    adapter_id = %spec.adapter_id,
                    lens_id = %lens.id(),
                    "persisted spec: rehydrated lens enrichment"
                );
                self.enrichments.push(lens);
            }
        }

        self
    }

    /// Consume the builder and return the configured `IngestPipeline`.
    pub fn build(mut self) -> IngestPipeline {
        // Register ExtractionCoordinator with its structural modules
        if let Some(coordinator) = self.coordinator.take() {
            self.pipeline.register_adapter(Arc::new(coordinator));
        }
        // ProvenanceAdapter is the integration anchor — its enrichments
        // get registered alongside it via register_integration().
        self.pipeline.register_integration(
            Arc::new(ProvenanceAdapter::new()),
            self.enrichments,
        );
        self.pipeline
    }

    /// Convenience: build a fully-configured default pipeline.
    ///
    /// Equivalent to:
    /// ```ignore
    /// let persisted = gather_persisted_specs(&engine);
    /// let client: Arc<dyn LlmOrcClient> = Arc::new(SubprocessClient::new());
    /// PipelineBuilder::new(engine)
    ///     .with_default_adapters()
    ///     .with_default_structural_modules()
    ///     .with_default_enrichments()
    ///     .with_llm_client(client)
    ///     .with_persisted_specs(persisted)
    ///     .build()
    /// ```
    ///
    /// Automatically gathers and rehydrates any persisted specs from the
    /// context's specs table (ADR-037 §2, Invariant 62 effect b). The
    /// specs table is the context's lens registry — any library instance
    /// against a context transiently runs every lens registered on it.
    ///
    /// Wires `SubprocessClient` as the llm-orc client so both built-in
    /// semantic extraction (`extract-file` route) and consumer declarative
    /// specs with `ensemble:` field can invoke llm-orc. When llm-orc is
    /// not running, semantic extraction degrades gracefully (Invariant 47
    /// — `AdapterError::Skipped`); registration and structural analysis
    /// always complete.
    pub fn default_pipeline(engine: Arc<PlexusEngine>) -> IngestPipeline {
        let persisted = gather_persisted_specs(&engine);
        let client: Arc<dyn LlmOrcClient> = Arc::new(crate::llm_orc::SubprocessClient::new());
        Self::new(engine)
            .with_default_adapters()
            .with_default_structural_modules()
            .with_default_enrichments()
            .with_llm_client(client)
            .with_persisted_specs(persisted)
            .build()
    }
}

/// Gather all persisted specs across every context in the engine (ADR-037 §2).
///
/// Iterates `engine.list_contexts()` and accumulates the results of
/// `engine.query_specs_for_context()` for each. This is the host-level
/// "read specs table" step called out by ADR-037 §2 — library hosts that
/// construct their own pipeline via `PipelineBuilder::new().with_...` should
/// call this helper and pass the result to `with_persisted_specs`.
///
/// `default_pipeline()` calls this automatically; hosts using the default
/// construction path get rehydration without needing to invoke this directly.
///
/// Errors from individual context queries are logged and skipped — consistent
/// with the rehydration error policy (non-fatal "log and continue"). A
/// transient storage failure on one context shouldn't prevent library startup.
pub fn gather_persisted_specs(engine: &PlexusEngine) -> Vec<PersistedSpec> {
    let mut specs = Vec::new();
    for ctx_id in engine.list_contexts() {
        match engine.query_specs_for_context(ctx_id.as_str()) {
            Ok(mut ctx_specs) => specs.append(&mut ctx_specs),
            Err(e) => tracing::warn!(
                context_id = %ctx_id,
                error = %e,
                "gather_persisted_specs: failed to query specs for context, skipping"
            ),
        }
    }
    specs
}
