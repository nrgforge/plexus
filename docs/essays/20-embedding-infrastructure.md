# Embedding Infrastructure: Latent Evidence in the Knowledge Graph

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — February 2026*

---

## The Problem

Plexus builds knowledge graphs from fragments, tags, and adapter-produced semantic structure. Every connection in the graph is explicit — a `tagged_with` edge exists because a human or LLM applied a tag; a `may_be_related` edge exists because CoOccurrenceEnrichment detected shared sources. You can trace any edge back to the adapter and source that produced it.

This is a strength. But it leaves a class of relationships invisible: pairs that are semantically similar but have never been explicitly connected. Two writing fragments about similar themes that were tagged differently. A concept from one research project that overlaps with a concept from another. A movement quality and a textual description that describe the same thing from different modalities.

Embedding models can detect these latent relationships. By projecting nodes into a continuous vector space, similarity becomes a distance computation rather than a structural query. Two concept nodes might share no explicit edges but sit close together in embedding space — indicating a relationship that the explicit graph structure hasn't captured.

The domain model already defines this architecture (OQ-14): latent evidence enters via `ingest()` as `similar_to` edges, DiscoveryGapEnrichment (built, reactive) detects structurally-disconnected but latently-similar pairs, and contribution tracking distinguishes which embedding model produced which evidence. The question is how to implement it: which models, which libraries, what storage, and how does model identity interact with the graph's provenance story.

---

## Two Evidence Layers

The domain model distinguishes structural evidence from latent evidence (Disambiguation §14). They are independent layers on the same graph, and the independence is what makes their disagreement informative.

**Structural evidence** is explicit, named, and traceable. A `tagged_with` edge has per-adapter contributions, provenance entries, and a clear explanation: "this fragment was tagged with this concept by this adapter." Four different adapters independently producing the same `tagged_with` edge is strong structural evidence — evidence diversity (distinct adapter IDs, source types, contexts) is directly queryable.

**Latent evidence** is implicit, continuous, and opaque. An embedding captures semantic proximity without naming the relationship. You can measure how close two nodes are but not articulate why. The embedding model's internal representations — attention patterns, learned features — are not human-interpretable.

The discovery gap between these layers is the productive signal. Structural evidence without latent support may indicate incidental co-occurrence — two concepts tagged together once but not semantically related. Latent similarity without structural support may indicate unexplored territory — a connection the graph hasn't surfaced through its explicit structure. DiscoveryGapEnrichment flags these pairs, not as errors, but as opportunities for a human to investigate.

---

## Embedding Production: Three Paths

Research identified three complementary paths for producing embeddings, all converging at the same graph entry point.

### Path A: Rust-native via fastembed-rs

fastembed-rs wraps ONNX Runtime in a Rust API, providing in-process embedding with no Python dependency. It supports the models that matter: all-MiniLM-L6-v2 (22M parameters, 384 dimensions, ~15ms per document), nomic-embed-text (137M parameters, 768 dimensions with Matryoshka truncation, ~50–100ms per document), and BGE variants.

This is the production path. Embeddings computed in-process, with sub-100ms latency. For EDDI's real-time gesture sessions, this is the only path fast enough — a gesture enters the graph, the embedding enrichment fires, `similar_to` edges emit, DiscoveryGapEnrichment fires, and environmental parameters shift. The loop completes within the enrichment cycle.

fastembed-rs fits Plexus's library rule (Invariant 41): the engine takes a model path, the host layer decides where models are stored. No cloud API, no subprocess, no service dependency.

### Path B: llm-orc ensemble

llm-orc currently has no embedding support, but the architecture accommodates it naturally. A script primitive (`embed_text.py`) calls Ollama's batch embedding API (`POST /api/embed`) and returns vectors as JSON. This composes into ensembles via the standard dependency graph — an embedding step alongside extraction and analysis in the same pipeline.

The case for this path is composability and provenance. An ensemble that extracts content, embeds it, and computes pairwise similarity in one pipeline produces a complete provenance trail: the ensemble YAML records which model profile each agent used, and Plexus can read this to know exactly which model produced which embeddings. Swapping models means changing one line in YAML and re-running the ensemble.

This is the batch and experimentation path. Re-embedding a context with a different model. Embedding as a step in a semantic extraction pipeline. A/B testing embedding models by running both and comparing contribution maps.

### Path C: Ollama direct

Ollama exposes `POST /api/embed` with clean batch support: an array of strings in, an array of float arrays out. This reuses infrastructure Plexus already has (the subprocess client pattern from llm-orc integration) and provides access to any embedding model Ollama supports.

This is the prototyping path. Quick experiments, ad-hoc embedding, trying new models before committing to fastembed-rs integration.

### Embedding is enrichment, not adaptation

A key architectural insight emerged during review: embedding computation is an enrichment, not an adapter. In Plexus's vocabulary (Disambiguation §10), an adapter bridges between a consumer's domain and the graph — it has an `input_kind`, receives external domain data, and transforms events back. An enrichment reacts to graph events and produces derived structure from existing graph content.

Embedding operates on nodes already in the graph. It doesn't bring new external knowledge from a consumer — it computes relationships between things that are already there. That's enrichment.

The trigger determines the category:

- **Core enrichment** (fastembed-rs, Path A): Reactive. Fires in the enrichment loop when new nodes arrive. Embeds them in-process, compares against cached embeddings for existing nodes, emits `similar_to` edges. The embedding step is slower than co-occurrence (~15–100ms vs. microseconds), but the similarity computation over cached vectors is fast. For EDDI's real-time loop, this is the right model — embed once per node, compare many times.

- **External enrichment** (llm-orc, Path B; Ollama, Path C): Batch. Re-embeds all nodes with a (possibly different) model, computes all pairs, results re-enter via `ingest()` (Invariant 49). This is the experimentation and model-comparison path.

Both categories converge at the same graph entry point. All three paths produce `similar_to` edges with similarity scores as contribution values, entering via `ingest()`. Contribution tracking, scale normalization, the rest of the enrichment loop — all existing machinery applies unchanged.

The enrichment ID carries the model identity: `embedding:nomic-embed-text-v1.5` for the core enrichment, derived from ensemble config for llm-orc, `embedding:{model}` for Ollama. Different paths, different IDs, same graph structure.

---

## Model-Aware Provenance

The user raised a cross-cutting concern that expanded the research scope: the graph should track which model — LLM or embedding — produced which contributions, enabling replacement, comparison, and auditability. This is not specific to embeddings. It applies equally to semantic extraction ("we ran mistral, now we want sonnet") and to any computation where model choice affects graph content.

### What already works

Plexus's contribution tracking (Invariants 8–12) stores per-adapter contributions on every edge as `HashMap<AdapterId, f32>`. When two adapters emit the same edge, they occupy separate slots. Scale normalization brings different value ranges to a common scale before summing into raw weight. Evidence diversity — how many distinct adapters corroborate an edge — is directly queryable.

Model identity slots into adapter IDs naturally. Instead of `"extract-semantic"` (static, model-agnostic), use `"extract-semantic:claude-sonnet-4-5"` (parameterized, model-specific). Two extraction runs with different models produce two contribution slots on the same edge:

```
edge(file:docs/example.md → concept:jazz, "tagged_with")
  contributions: {
    "extract-semantic:mistral-7b": 0.6,
    "extract-semantic:claude-sonnet-4-5": 0.9
  }
```

The user can keep both (combined weight reflects multi-model agreement), compare them (query contributions to see which concepts each model found), or replace one (retract the old model's contributions, keep the new).

Multi-model composition on the same embedding edge works identically:

```
edge(concept:jazz → concept:improv, "similar_to")
  contributions: {
    "embedding:nomic-embed-text-v1.5": 0.82,
    "embedding:all-MiniLM-L6-v2": 0.71
  }
```

Both models say the pair is similar — the combined raw weight is higher than either alone. If they disagreed (one present, one absent), that itself would be informative.

### What's missing: contribution retraction

The contribution tracking machinery handles addition and update. It does not handle removal. There is no way to say "remove all contributions from adapter X across all edges" while preserving other adapters' contributions.

This is the gap that model replacement requires. A new emission type — `ContributionRetraction { adapter_id }` — would remove the named adapter's contribution slot from every edge in the context. After retraction, `recompute_raw_weights()` adjusts all affected edges. Edges that lose their last contribution can be pruned.

This is a batch operation by design. The use case is always "undo this model's influence on the graph," not "adjust one edge." Per-edge retraction is unnecessary complexity.

With retraction, the full model replacement workflow becomes:
1. Retract old model: remove `embedding:nomic-embed-text-v1.5` from all edges
2. Re-embed with new model: `embedding:nomic-embed-text-v2.0` produces new `similar_to` edges
3. DiscoveryGapEnrichment fires on new edges, re-evaluating discovery gaps

Three operations, no new infrastructure beyond the retraction primitive.

### Auditability

Embeddings are not directly auditable — a 768-dimensional vector is opaque. But their effects are traceable:

1. **Which model?** The adapter ID: `embedding:nomic-embed-text-v1.5`
2. **What input?** The node's content (label, properties) — what was embedded
3. **What output?** The `similar_to` edges produced, with similarity scores as contributions
4. **When?** Framework-constructed provenance timestamp
5. **Why these pairs?** Similarity threshold (configurable, logged in adapter or ensemble config)

You can't read an embedding and understand what it means. But you can audit which pairs it connected, at what strength, using which model — and selectively undo those effects via retraction. This is the right level of auditability for opaque computations: trace the effects, control the source.

---

## Storage

### Vectors

Two options for storing embedding vectors, appropriate at different scales.

**In-memory brute-force** for small graphs (<1K nodes). Store vectors as `Vec<f32>` alongside node properties. Compute pairwise cosine similarity with a linear scan. No dependencies, no complexity. This is the v1 implementation.

**sqlite-vec** for larger graphs. A loadable SQLite extension (successor to sqlite-vss) that provides KNN search via virtual tables. Same database file as the existing `SqliteStore`, no separate service. Query: `SELECT * FROM vec_items WHERE embedding MATCH ? ORDER BY distance LIMIT 10`. Appropriate when brute-force scans become a bottleneck — likely above a few thousand nodes.

### Similarity threshold

Not every pair of embedded nodes should produce a `similar_to` edge. A configurable threshold (e.g., cosine similarity > 0.7) filters noise. Below the threshold, no edge is emitted — the pair isn't similar enough to be worth tracking.

The threshold is a parameter of the embedding enrichment, not a global setting. Different embedding models produce different similarity distributions. A threshold that works for nomic-embed-text may filter too aggressively for MiniLM. The enrichment knows its model and sets an appropriate threshold.

---

## Discovery Gap: Reactive Is Sufficient

DiscoveryGapEnrichment (ADR-024, built) reacts to new `similar_to` edges in the enrichment loop. For each new trigger edge between nodes A and B, it checks whether any structural edge connects them (excluding trigger and output relationships). If no structural evidence exists, it emits a `discovery_gap` edge.

The domain model asked whether a periodic batch sweep should also compute gaps across the full graph. The answer: not for v1. The reactive enrichment handles the steady state — every new `similar_to` edge is evaluated immediately. A batch sweep would catch edge cases (structural evidence removed after the gap was computed, pre-existing `similar_to` edges from before the enrichment was registered), but these are rare in Plexus's additive graph model.

If a batch sweep is needed, it's trivially implementable as an external enrichment: an llm-orc ensemble reads the graph export, finds all `similar_to` pairs without structural evidence, and returns `discovery_gap` proposals via `ingest()`. No new infrastructure — just an ensemble definition.

---

## Embedding Drift

When an embedding model is updated, the vector space changes. Similarity scores shift. `similar_to` edges computed by the old model may no longer be valid.

Contribution tracking handles this through adapter ID versioning. A new model version gets a new adapter ID: `embedding:nomic-embed-text-v2.0`. The replacement workflow is the same as any model swap: retract old contributions, re-embed with new model. Old and new can coexist during migration — scale normalization composes them correctly.

There is no need for automatic re-embedding. Embedding drift is a deliberate model change, not something that happens silently. The user decides when to re-embed. The tools are: ContributionRetraction (remove old influence), the embedding pipeline (produce new embeddings), and adapter ID versioning (distinguish old from new).

This mechanism applies identically to LLM extraction drift. "I ran mistral, now I want sonnet" follows the same workflow: retract `extract-semantic:mistral-7b`, re-extract with `extract-semantic:claude-sonnet-4-5`. The contribution tracking machinery doesn't know or care whether the model produced embeddings or extracted concepts.

---

## Cross-Modal Embeddings

Plexus serves multiple domains: Trellis (writing fragments), Carrel (academic citations), and EDDI (movement analysis). Within each domain, same-modal embeddings are straightforward — embed text with a text model, compute mathematical similarity in movement parameter space. Cross-modal embeddings — finding that a writing fragment is latently similar to a movement quality — are harder and more speculative.

### The landscape

Research found several relevant models. **ImageBind** (Meta) is the most immediately applicable: it embeds six modalities (text, image, audio, depth, thermal, and IMU sensor data) into a shared 1024-dimensional space. IMU as a first-class modality means accelerometer/gyroscope data from gesture sessions could be embedded alongside text. The caveat: ImageBind was trained on general activities, not expressive performance movement.

**TMR** (Text-to-Motion Retrieval) produces high-quality text-motion embeddings but requires skeletal joint data in SMPL format. **CLAP** (Contrastive Language-Audio Pretraining) handles text-audio alignment for musical contexts. **MotionCLIP** places motion embeddings directly in CLIP's text space.

### EDDI's mathematical approach

A critical distinction: EDDI models movement as continuous mathematical properties informed by the Viewpoints framework, not as classification labels from a fixed ontology. Viewpoints — the improvisation technique developed by Mary Overlie and Anne Bogart — organizes movement through abstractions like spatial relationship, kinesthetic response, tempo, duration, repetition, shape, gesture, and architecture. Where Laban provides a categorical vocabulary for naming movement qualities (weight, space, time, flow), Viewpoints' emphasis on abstraction invites formal mathematical modeling. EDDI draws on Laban's ontological vocabulary but organizes around Viewpoints' more open-ended framework.

"Flow" or "tempo" isn't a label applied to a gesture — it's a continuous value on a spectrum. `similar_to` edges between movement nodes come from proximity in this mathematical parameter space (Euclidean distance, cosine similarity on Viewpoints dimension vectors), not from text embedding similarity.

This means within-domain similarity for EDDI is a solved problem — it's distance computation in the parameter space the adapter already produces. The cross-modal bridge is the open question: how do you connect a movement parameter vector `{tempo: 0.85, duration: 0.2}` to a text fragment about "slow, sustained motion"?

Three paths, in order of pragmatism:

1. **Viewpoints-as-text** (now). Convert structured movement annotations to natural language descriptions, embed with a text model. Zero new infrastructure. Loses continuous precision but captures enough semantic proximity for discovery gap flagging.

2. **ImageBind spike** (near-term). Test whether ImageBind's IMU encoder captures expressive movement quality. A focused experiment: embed gesture clips via IMU encoder, embed text descriptions via text encoder, measure cosine similarity. Validates or invalidates the cross-modal path.

3. **Projection fine-tuning** (future). Train a small MLP that projects EDDI's movement parameter vectors into a frozen text encoder's embedding space. Requires a few hundred annotated (gesture, description) pairs. The deeper research question: what projection maps mathematical movement space to text embedding space such that nearby points land near semantically related text? EDDI's mathematical representation may be *more* precise than any text description of movement — the challenge is finding the right bridge without losing that precision.

### Architectural non-impact

None of this changes the Plexus graph architecture. `similar_to` edges between a movement node and a text fragment use the same edge structure, contribution tracking, and enrichment pipeline as same-modal similarities. DiscoveryGapEnrichment doesn't care about the geometry that produced the edge — it checks for structural evidence regardless of modality.

The enrichment ID distinguishes the source: `embedding:nomic-embed-text-v1.5` for text-to-text, `embedding:eddi-viewpoints-projection` for cross-modal. Different enrichments, same graph.

---

## What Emerges

The research reveals that embedding infrastructure for Plexus is less about new architecture and more about parameterizing existing mechanisms.

**Contribution tracking already handles multi-model composition.** Model identity lives in adapter IDs. Scale normalization handles different value ranges. Evidence diversity queries tell you which models agree.

**The enrichment pipeline already handles latent evidence.** `similar_to` edges enter via `ingest()`. DiscoveryGapEnrichment reacts. CoOccurrenceEnrichment parameterized on `similar_to` finds second-order latent bridges.

**The only genuinely new primitives are:**
1. An embedding enrichment (core via fastembed-rs for reactive, external via llm-orc for batch)
2. ContributionRetraction (remove a model's influence — needed for model replacement)
3. Vector storage (in-memory for v1, sqlite-vec for scale)

Everything else — similarity thresholds, enrichment ID versioning, cross-modal bridges, discovery gap detection — is configuration of existing machinery.

This is the payoff of Plexus's design: because all evidence enters through `ingest()`, because all edges carry per-adapter contributions, because enrichments are structure-aware (Invariant 50) rather than type-aware, a new evidence layer slots in without structural changes. The embedding model is just another enrichment with an opinion about edge strength.

---

## Invariant Check

The following domain model invariants are relevant. None are contradicted.

- **Invariant 6 (enrichments don't produce provenance):** Embedding enrichments produce derived structure (`similar_to` edges), not source evidence. Framework-constructed provenance (enrichment ID encoding model identity, timestamp) is sufficient. This is the same as co-occurrence and tag bridging.
- **Invariant 7 (dual obligation):** Does not apply. Invariant 7 governs adapters — components that bridge between a consumer's domain and the graph. Embedding computation is an enrichment: it operates on existing graph content, not external domain data. Recognizing embedding as enrichment dissolves what initially appeared to be a tension.
- **Invariant 13 (stable enrichment IDs):** Model-parameterized IDs (`embedding:nomic-embed-text-v1.5`) are stable across sessions for the same model. New model version → new ID, which is the intended behavior for contribution slot management.
- **Invariant 34 (all writes through ingest):** All three embedding paths converge at `ingest()`.
- **Invariant 41 (library rule):** fastembed-rs takes a model path. The host layer decides where model files are stored.
- **Invariant 49 (external enrichments outside loop):** Batch embedding via llm-orc is an external enrichment. Results re-enter via `ingest()`, triggering core enrichments on the new data.
- **Invariant 50 (structure-aware, not type-aware):** DiscoveryGapEnrichment fires on `similar_to` edges regardless of which enrichment or modality produced them.

No invariant tensions. The "embedding as enrichment" framing aligns cleanly with the existing architecture — enrichments produce derived structure, adapters process source material, and the distinction holds.

---

## Build Scope

For the next phase:

**Minimal viable embedding (3 items):**
1. Embedding core enrichment via fastembed-rs — model loading, in-process embedding, `similar_to` edge emission in the enrichment loop
2. ContributionRetraction emission type — batch removal of a model's contributions
3. In-memory vector storage with similarity threshold

**llm-orc embedding (1 item, parallel):**
4. `embed_text.py` script primitive in llm-orc library — calls Ollama `/api/embed`, returns vectors as JSON

**Deferred:**
- sqlite-vec (scale optimization — not needed until node count exceeds ~1K)
- Cross-modal embedding (EDDI-specific, needs more research on the Viewpoints parameter-to-text projection)
- Automatic re-embedding on model change (user-driven is sufficient for v1)
