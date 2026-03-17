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
use crate::adapter::enrichments::cooccurrence::CoOccurrenceEnrichment;
use crate::adapter::enrichments::discovery_gap::DiscoveryGapEnrichment;
use crate::adapter::enrichments::temporal_proximity::TemporalProximityEnrichment;
use crate::graph::PlexusEngine;
use std::path::Path;
use std::sync::Arc;

/// Builder for `IngestPipeline` with standard adapter/enrichment registration.
///
/// Provides a transport-neutral construction API so that MCP, CLI, and
/// embedded consumers all get the same pipeline without duplicating
/// registration logic.
pub struct PipelineBuilder {
    engine: Arc<PlexusEngine>,
    pipeline: IngestPipeline,
    enrichments: Vec<Arc<dyn Enrichment>>,
}

impl PipelineBuilder {
    /// Start building a pipeline for the given engine.
    pub fn new(engine: Arc<PlexusEngine>) -> Self {
        let pipeline = IngestPipeline::new(engine.clone());
        Self {
            engine,
            pipeline,
            enrichments: Vec::new(),
        }
    }

    /// Register the core adapters: ContentAdapter, ExtractionCoordinator, ProvenanceAdapter.
    pub fn with_default_adapters(mut self) -> Self {
        self.pipeline.register_adapter(Arc::new(ContentAdapter::new("content")));
        self.pipeline.register_adapter(Arc::new(ExtractionCoordinator::new()));
        // ProvenanceAdapter is registered via register_integration in build()
        self
    }

    /// Register the domain-agnostic enrichments.
    ///
    /// Default set: CoOccurrence, DiscoveryGap, TemporalProximity,
    /// and EmbeddingSimilarity (when the `embeddings` feature is enabled).
    ///
    /// TagConceptBridger is intentionally excluded — it is domain-specific
    /// and opt-in via adapter spec `enrichments:` declarations (ADR-025).
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

    /// Consume the builder and return the configured `IngestPipeline`.
    pub fn build(mut self) -> IngestPipeline {
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
            .with_default_enrichments();

        if let Some(dir) = project_dir {
            builder = builder.with_adapter_specs(dir);
        }

        builder.build()
    }
}
