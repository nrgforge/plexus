# ADR-026: Embedding as Enrichment

**Status:** Proposed

**Research:** [Essay 20](../essays/20-embedding-infrastructure.md)

**Domain model:** [domain-model.md](../domain-model.md) — latent evidence, structural evidence, `similar_to`, discovery gap, core enrichment, external enrichment

**Amends:** ADR-024 (adds a fifth core enrichment: EmbeddingSimilarityEnrichment)

**Depends on:** ADR-010 (enrichment trait and loop), ADR-024 (core/external enrichment architecture), ADR-003 (contribution tracking)

---

## Context

Open Question 14 asks how embedding-based similarity should integrate with the knowledge graph. The domain model already defines the graph-side architecture: latent evidence enters via `ingest()` as `similar_to` edges, DiscoveryGapEnrichment (built, reactive) detects structurally-disconnected but latently-similar pairs, and contribution tracking distinguishes which embedding model produced which evidence.

Essay 20 established three paths for producing embeddings (fastembed-rs, llm-orc ensemble, Ollama direct) and found that embedding computation operates on existing graph content — it doesn't bring new external knowledge from a consumer. It reads node labels and properties, projects them into a vector space, and emits `similar_to` edges between sufficiently similar pairs.

This is enrichment, not adaptation. An adapter (Disambiguation §10) bridges between a consumer's domain and the graph — it has an `input_kind`, receives external domain data, and transforms events back. An enrichment reacts to graph events and produces derived structure from existing graph content. Embedding meets the enrichment definition.

## Decision

### Embedding is an enrichment

Embedding similarity computation is classified as an enrichment, following the same architectural rules as CoOccurrenceEnrichment, TagConceptBridger, DiscoveryGapEnrichment, and TemporalProximityEnrichment. It does not have an `input_kind`, does not receive domain data from a consumer, and does not transform events for a consumer.

Invariant 6 applies: enrichments do not produce provenance. Framework-constructed provenance (enrichment ID encoding model identity, timestamp) is sufficient for `similar_to` edges. Invariant 7 (dual obligation) does not apply — it governs adapters that process source material.

### Two modes: core and external

**Core enrichment (reactive, fastembed-rs):** `EmbeddingSimilarityEnrichment` implements the `Enrichment` trait. Fires in the enrichment loop when `NodesAdded` events arrive. For each new node: embed its label/content via fastembed-rs (in-process ONNX), compare against cached embeddings for existing nodes, emit `similar_to` symmetric edge pairs above a configurable similarity threshold.

```rust
pub struct EmbeddingSimilarityEnrichment {
    model_name: String,              // e.g., "nomic-embed-text-v1.5"
    similarity_threshold: f32,       // e.g., 0.7
    output_relationship: String,     // e.g., "similar_to"
    id: String,                      // "embedding:{model_name}"
    // ... embedding model handle, vector cache
}
```

The enrichment caches embeddings for all nodes in the context. When a new node arrives, it embeds the node, computes cosine similarity against all cached vectors, and emits edges for pairs above the threshold. The embedding step (~15–100ms per node) is orders of magnitude slower than other core enrichments (microseconds). This stretches the "fast" qualifier in ADR-024's definition of core enrichments. The justification for keeping it in the loop rather than as an external enrichment: the similarity *computation* over cached vectors is microsecond-scale (the expensive part is the one-time embed per new node), and EDDI's real-time loop requires reactive response — external enrichment latency (seconds) is too slow for environmental parameter shifts.

**Latency mitigation:** For node bursts (e.g., 10 nodes in one emission), the enrichment batches all new nodes into a single ONNX inference call rather than embedding one at a time. fastembed-rs supports batch embedding natively. A burst of 10 nodes at ~15ms each serially would be 150ms; batched, it's closer to 30-50ms total.

**Output space:** The theoretical edge space is O(N^2) for N nodes, but the similarity threshold bounds actual output well below this. With a threshold of 0.7, empirically only a small fraction of node pairs are above threshold. The threshold directly controls enrichment loop cost — a lower threshold produces more edges and more enrichment work.

Parameterizable in adapter spec YAML:
```yaml
enrichments:
  - type: embedding_similarity
    model_name: nomic-embed-text-v1.5
    similarity_threshold: 0.7
    output_relationship: similar_to
```

**External enrichment (batch, llm-orc):** An llm-orc ensemble with a script primitive (`embed_text.py`) that calls Ollama's `/api/embed`. Reads graph export, embeds all node labels, computes pairwise similarity, returns `similar_to` edge proposals. Results re-enter via `ingest()` per Invariant 49.

This is the batch and experimentation path — re-embedding a context with a different model, A/B testing models, embedding as a step in a semantic extraction pipeline.

### Enrichment ID encodes model identity

The enrichment ID includes the model name: `embedding:nomic-embed-text-v1.5`. Different models produce different contribution slots on the same edge. This enables multi-model composition (scale normalization handles different value ranges) and model comparison (query contributions to see which models agree).

A new model version gets a new enrichment ID: `embedding:nomic-embed-text-v2.0`. Old and new coexist until the old is retracted (see ADR-027).

### Vector storage

**V1: sqlite-vec.** A loadable SQLite extension (MIT OR Apache-2.0, zero external dependencies, pure C) providing KNN search via virtual tables. Same database file as the existing `SqliteStore`. Typical contexts (codebases, corpora) easily reach thousands of nodes, making brute-force O(N^2) scan impractical. sqlite-vec stores vectors in a virtual table alongside the graph, queries via `SELECT * FROM vec_embeddings WHERE embedding MATCH ? ORDER BY distance LIMIT k`, and handles the KNN search efficiently.

The enrichment writes embeddings to sqlite-vec on each new node. Similarity queries use KNN search to find the top-k most similar nodes above the threshold, rather than scanning all cached vectors. This keeps the per-node enrichment cost bounded regardless of context size.

**Fallback: in-memory.** For contexts without persistence (e.g., in-memory-only test contexts), the enrichment falls back to `Vec<f32>` per node with brute-force cosine similarity. This is the test path, not the production path.

### What to embed

The enrichment embeds the node's `label` property (the human-readable label from the node's properties). For concept nodes, this is the concept name. For fragment nodes, this is the fragment text. For file nodes, this could be the file path or extracted title.

The enrichment does not embed all node types indiscriminately. A configurable filter (dimension, or dimension + relationship structure) controls which nodes are embedded. Default: concept nodes in the semantic dimension. The filter is structure-aware per Invariant 50 — it selects nodes by their graph-structural properties (dimension membership, relationship patterns), not by domain-specific content type.

### Scale normalization interaction

Cosine similarity scores after thresholding occupy a narrow band (e.g., 0.7–0.95). Scale normalization (ADR-003) maps the minimum contribution to near-floor and the maximum to near-1.0. The similarity threshold acts as a de facto floor — contributions below it are never emitted, so the threshold becomes the effective minimum for scale normalization. This means the spread between "barely similar" (0.7) and "very similar" (0.95) is preserved in the normalized range, which is the correct behavior for ranking similarity strength.

When multiple embedding models contribute to the same edge, each model's contributions are scale-normalized independently (per ADR-003). If model A produces scores in 0.7–0.9 and model B in 0.75–0.95, each is normalized within its own range before summing.

### Domain model amendment required

The domain model currently states that latent evidence enters "via `ingest()` as an external enrichment (ADR-023)." This ADR adds a second entry path: core enrichment (reactive, in the enrichment loop). Both paths produce `similar_to` edges; the difference is scheduling (reactive vs batch). The domain model's "Evidence layers and discovery" section must be amended to acknowledge both paths.

## Consequences

**Positive:**

- Embedding slots cleanly into the existing enrichment architecture — no new pipeline, no new trait, no new infrastructure beyond the enrichment implementation itself
- Invariant 7 tension dissolves — enrichments don't have a dual obligation, so framework-constructed provenance is sufficient
- The reactive core enrichment enables EDDI's real-time loop: gesture → graph → embed → similar_to → discovery gap → environmental response
- Multi-model composition works automatically via existing contribution tracking and scale normalization

**Negative:**

- The core enrichment's embedding step (~15–100ms per node) is orders of magnitude slower than other core enrichments (microseconds). ADR-024 defines core enrichments as "fast (microseconds)" — this enrichment expands that definition to include latency-tolerant reactive enrichments where the one-time computation cost is bounded and the reactive benefit outweighs the latency cost. Batch embedding within a single ONNX inference call mitigates burst scenarios
- fastembed-rs adds a dependency (ONNX Runtime) to the Plexus binary. This is a non-trivial addition to the build. All dependencies are AGPL-3.0 compatible (fastembed-rs: Apache-2.0, ort: MIT/Apache-2.0, ONNX Runtime: MIT, sqlite-vec: MIT/Apache-2.0). Eigen (MPL-2.0, inside ONNX Runtime) requires attribution in NOTICE file but does not propagate copyleft to the larger work
- sqlite-vec adds a loadable SQLite extension dependency. Zero external dependencies (pure C), but requires building or bundling the extension alongside Plexus

**Neutral:**

- DiscoveryGapEnrichment (ADR-024, built) gains its first trigger data source — `similar_to` edges from the embedding enrichment fire discovery gap detection
- The external enrichment path (llm-orc) requires no Plexus changes — just a new script and ensemble YAML in `.llm-orc/`
