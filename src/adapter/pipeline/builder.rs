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
use crate::adapter::adapters::structural::{MarkdownStructureModule, StructuralModule};
use crate::adapter::enrichments::cooccurrence::CoOccurrenceEnrichment;
use crate::adapter::enrichments::discovery_gap::DiscoveryGapEnrichment;
use crate::adapter::enrichments::temporal_proximity::TemporalProximityEnrichment;
use crate::graph::PlexusEngine;
use crate::storage::PersistedSpec;
use std::path::Path;
use std::sync::Arc;

/// Builder for `IngestPipeline` with standard adapter/enrichment registration.
///
/// Provides a transport-neutral construction API so that MCP, CLI, and
/// embedded consumers all get the same pipeline without duplicating
/// registration logic.
pub struct PipelineBuilder {
    pipeline: IngestPipeline,
    coordinator: Option<ExtractionCoordinator>,
    enrichments: Vec<Arc<dyn Enrichment>>,
}

impl PipelineBuilder {
    /// Start building a pipeline for the given engine.
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        let pipeline = IngestPipeline::new(engine.clone());
        Self {
            pipeline,
            coordinator: None,
            enrichments: Vec::new(),
        }
    }

    /// Register the core adapters: ContentAdapter, ExtractionCoordinator, ProvenanceAdapter.
    ///
    /// The `ExtractionCoordinator` is held by the builder until `build()` so
    /// that structural modules can be registered on it via `with_structural_module()`.
    pub fn with_default_adapters(mut self) -> Self {
        self.pipeline.register_adapter(Arc::new(ContentAdapter::new("content")));
        self.coordinator = Some(ExtractionCoordinator::new());
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

    /// Load declarative adapter specs from a directory (ADR-028).
    ///
    /// Scans `project_dir/adapter-specs/` for `*.yaml` files and registers
    /// each as a `DeclarativeAdapter`. Creates an llm-orc subprocess client
    /// scoped to the project directory.
    pub fn with_adapter_specs(mut self, project_dir: &Path) -> Self {
        let specs_dir = project_dir.join("adapter-specs");
        if specs_dir.is_dir() {
            let client: Arc<dyn crate::llm_orc::LlmOrcClient> = Arc::new(
                crate::llm_orc::SubprocessClient::new()
                    .with_project_dir(project_dir.to_string_lossy().to_string()),
            );
            let n = self.pipeline.register_specs_from_dir(&specs_dir, Some(client));
            if n > 0 {
                tracing::info!(count = n, "adapter-specs: loaded spec(s)");
            }
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
    /// PipelineBuilder::new(engine)
    ///     .with_default_adapters()
    ///     .with_default_structural_modules()
    ///     .with_default_enrichments()
    ///     .with_adapter_specs(project_dir)
    ///     .build()
    /// ```
    pub fn default_pipeline(
        engine: Arc<PlexusEngine>,
        project_dir: Option<&Path>,
    ) -> IngestPipeline {
        let mut builder = Self::new(engine)
            .with_default_adapters()
            .with_default_structural_modules()
            .with_default_enrichments();

        if let Some(dir) = project_dir {
            builder = builder.with_adapter_specs(dir);
        }

        builder.build()
    }
}
