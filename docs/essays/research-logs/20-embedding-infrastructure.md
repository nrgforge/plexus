# Research Log: OQ-14 — Embedding Infrastructure for Plexus

## Background

OQ-14 asks how embedding-based similarity should integrate with the Plexus knowledge graph. The domain model already establishes the conceptual framework:

- **Latent evidence** enters via `ingest()` as an external enrichment (ADR-023)
- **`similar_to` edges** carry embedding-derived similarity above a threshold
- **DiscoveryGapEnrichment** (core, reactive) detects latent-structural disagreement
- **Contribution tracking** applies identically — each embedding model gets its own adapter ID (e.g., `embedding:all-MiniLM-L6-v2`)

The constraint is local/offline operation (no cloud API for core functionality) and sub-second latency for EDDI's real-time gesture sessions.

The user also raised a cross-cutting concern: **model-aware provenance**. The graph should track which model (LLM or embedding) produced which contributions, enabling selective replacement, comparison, and auditability. This connects to Plexus's existing contribution tracking (Invariants 8–12) and adapter IDs (Invariant 13).

### Sub-questions from domain model

1. **Embedding production.** Which models for which domains? General-purpose text vs. movement-quality embeddings. External enrichment (batch) vs. per-file extraction phase vs. both?
2. **Discovery gap computation.** Reactive core enrichment (built) vs. periodic batch sweep — is the reactive enrichment sufficient?
3. **Embedding drift.** Model updates shift `similar_to` edges. Re-embed strategy? New adapter ID per model version?
4. **Cross-modal embeddings.** Text and movement in the same vector space — cross-modal discovery gaps?

### Additional questions (from user)

5. **Model-aware provenance.** How does model identity interact with contribution tracking? What does "replace a model's contributions" look like architecturally?

---

## Question 1: What local embedding models and libraries are available for offline use, and what are their latency/quality tradeoffs?

**Method:** Web search (6 searches covering models, Rust libraries, storage, benchmarks)

### Findings

#### Embedding models (local, offline)

| Model | Params | Dims | Context | Quality (MTEB) | Latency (CPU) | Notes |
|-------|--------|------|---------|----------------|---------------|-------|
| all-MiniLM-L6-v2 | 22M | 384 | 256 tok | ~56% avg (lower tier) | ~15ms/doc | De facto standard for local/lightweight. ONNX available. |
| nomic-embed-text-v1.5 | 137M | 768 (Matryoshka) | 8192 tok | ~62% avg | ~50–100ms/doc | Long-context, Matryoshka (variable dims), Ollama-native. |
| snowflake-arctic-embed-s | 33M | 384 | 512 tok | Better than MiniLM for size class | ~20ms/doc | MiniLM-based, punches above weight. |
| mxbai-embed-large | 335M | 1024 | 512 tok | ~64% avg | ~200ms/doc | Highest quality among local options. Ollama-native. |
| BGE-small-en-v1.5 | 33M | 384 | 512 tok | Good for size | ~20ms/doc | Popular alternative to MiniLM. |

**Key tradeoff:** MiniLM (22M) is 5–10x faster but noticeably lower quality than nomic/mxbai. For Plexus, where embeddings produce `similar_to` edges that feed discovery gap detection, quality matters more than raw speed — a bad embedding produces bad similarity signals. But EDDI needs sub-second latency for real-time sessions, which even nomic can deliver (50–100ms per document on CPU).

**Matryoshka embeddings** (nomic-embed-text) allow truncating the embedding dimension (768 → 256 → 128) at query time with graceful quality degradation. This is relevant for storage and search speed — store full 768d, search with truncated 256d for faster KNN, use full dimension for precise comparisons.

#### Rust embedding libraries

| Library | Approach | Dependencies | Notes |
|---------|----------|-------------|-------|
| **fastembed-rs** | ONNX Runtime (ort crate) | ONNX model files | Pure Rust inference. Supports all-MiniLM-L6-v2, BGE, nomic. Claims 10–15x throughput vs. Python on M1. Most mature Rust option. |
| **candle** (Hugging Face) | Native Rust ML | Model weights | Full ML framework in Rust. More control, more complexity. Metal/CUDA support. |
| **EmbedAnything** | Rust core + Python bindings | ONNX or candle | Hybrid approach. Can use either backend. Less mature. |
| **ort** (direct) | Raw ONNX Runtime bindings | ONNX model files | Low-level. fastembed-rs wraps this with higher-level API. |

**fastembed-rs is the clear winner** for Plexus:
- Pure Rust, no Python dependency (Invariant 41: library rule)
- Supports the models we care about (MiniLM, BGE, nomic)
- ONNX Runtime provides CPU optimization (SIMD, threading) out of the box
- Well-maintained (Qdrant team), active development

**Alternative: Ollama embedding API.** Ollama exposes `POST /api/embed` at `localhost:11434`, supporting nomic-embed-text, mxbai-embed-large, and others. This reuses existing infrastructure (Plexus already has the subprocess pattern for llm-orc), but adds a network hop and process dependency. Better for prototyping; fastembed-rs better for production.

#### Vector storage

| Option | Approach | Dependencies | Notes |
|--------|----------|-------------|-------|
| **sqlite-vec** | SQLite extension | C library, no deps | Successor to sqlite-vss. Pure C, no external deps. KNN search via virtual tables. Runs anywhere SQLite runs. |
| **In-memory (Vec<f32>)** | Brute-force scan | None | Simplest. O(n) per query. Fine for <10K nodes. |
| **Separate vector DB** (Qdrant, etc.) | External service | Service process | Overkill for local-first tool. |

**sqlite-vec is the right fit:**
- Plexus already uses SQLite for persistence (`SqliteStore`)
- sqlite-vec is a loadable extension — same DB file, no separate service
- KNN search via `SELECT * FROM vec_items WHERE embedding MATCH ? ORDER BY distance LIMIT 10`
- Handles the scale Plexus operates at (hundreds to low thousands of nodes)
- The embedding vectors would live in a `vec_items` virtual table alongside the regular `nodes` table

**For small graphs (<1K nodes), brute-force in-memory scan is sufficient** and avoids the sqlite-vec dependency. This could be the initial implementation with sqlite-vec as a future optimization.

#### Two integration paths

**Path A: Rust-native via fastembed-rs**
- Embed at ingest time in the adapter pipeline
- Store vectors in sqlite-vec or in-memory
- Compute similarity in Rust → emit `similar_to` edges via `ingest()`
- No external process dependency
- Fits Invariant 41 (library rule)
- Sub-millisecond embedding lookup, ~15–100ms embedding generation

**Path B: Ollama-based (like llm-orc)**
- Embed via Ollama API call during external enrichment
- More models available (any Ollama-supported embedding model)
- Already have subprocess pattern
- Adds network latency (~5–10ms per call) and Ollama dependency
- Better for experimentation, worse for production latency

**Path C: Hybrid**
- fastembed-rs for real-time (EDDI gesture sessions — must be fast)
- Ollama for batch re-embedding or experimenting with new models
- Both produce `similar_to` edges via the same `ingest()` path
- Different adapter IDs distinguish the source model

### Implications

1. **fastembed-rs is the primary integration target.** It's Rust-native, fast, and supports the models we need. No Python dependency.
2. **nomic-embed-text is the likely default model** — good quality, long context, Matryoshka support, reasonable latency on CPU.
3. **sqlite-vec for persistent vector storage** — same DB, loadable extension, KNN search. But brute-force in-memory is fine as v1.
4. **Both paths (fastembed-rs and Ollama) should use the same graph entry point** — `ingest()` with model-specific adapter IDs. This is the contribution tracking / model-aware provenance story.

### Open questions for Q2

- How does model identity map to adapter IDs? Is it `embedding:nomic-embed-text-v1.5` or `embedding:nomic-embed-text` (version-agnostic)?
- What does "replace a model's contributions" mean concretely — delete all `similar_to` edges with that adapter's contribution, then re-embed?
- How do we handle the case where two embedding models both contribute to the same `similar_to` edge?

---

## Question 1 Addendum: Embedding support in llm-orc

**Triggered by:** User observation that llm-orc could provide a third embedding path — composable with existing ensemble pipelines.

**Method:** Codebase exploration (llm-orc architecture) + web research (Ollama embedding API, orchestration framework patterns)

### Current llm-orc state

llm-orc has **no existing embedding support**. Current capabilities: LLM text generation (Anthropic, Google, Ollama providers), script primitives (JSON I/O executables), ensemble orchestration (dependency DAGs, fan-out, phases). The `ModelInterface` ABC exposes only `generate_response(message, role_prompt) -> str`.

### Ollama embedding API

Ollama exposes `POST /api/embed` with clean batch support:

```json
// Request
{"model": "nomic-embed-text", "input": ["text1", "text2", "text3"]}

// Response
{"model": "nomic-embed-text", "embeddings": [[0.01, -0.02, ...], [0.03, 0.01, ...], ...]}
```

Key details:
- Batch input (string array) → batch output (array of float arrays)
- L2-normalized vectors (unit length)
- `dimensions` parameter for Matryoshka truncation
- Same Ollama instance already used by llm-orc for LLM calls
- Also exposes OpenAI-compatible `/v1/embeddings`

### Three integration options in llm-orc

**Option A: Script primitive** (`primitives/embeddings/embed_text.py`)
- Lowest barrier. A script that calls Ollama `/api/embed` and returns vectors as JSON.
- Composes into ensembles via standard dependency graph — other agents receive embedding results.
- No changes to llm-orc core. Just add a script to the library.
- JSON I/O contract: `{texts: [...]} → {embeddings: [[...], [...]]}`

**Option B: Extend ModelInterface** with `async embed(text) -> list[float]`
- Deeper integration. Embedding becomes a model capability, not just a script.
- Provider-specific implementations (Ollama `embed()`, Anthropic embeddings, etc.)
- Model profiles could declare `type: embedding` vs `type: generation`.
- More work, but embeddings become first-class in the orchestration engine.

**Option C: New agent type** (`embedding-agent`)
- Middle ground. Ensemble YAML declares `type: embedding` agents.
- Configured with model and dimensions in YAML.
- Integrates with dependency system naturally.
- More than a script, less than a core model change.

### Industry patterns (from frameworks survey)

| Framework | Embedding explicitness | Pattern |
|-----------|----------------------|---------|
| Haystack | Fully explicit — typed DAG component with named ports | Closest to llm-orc's ensemble model |
| LangGraph | Semi-explicit — named graph node | Embedding is a node, output flows to next |
| LlamaIndex | Semi-explicit — pipeline component | Ingestion and query pipelines both use embedding steps |
| LangChain | Implicit — hidden inside retriever | Not a good model for llm-orc |

Haystack's pattern is most relevant: embedding is a discrete component in a typed pipeline, with explicit input/output ports. This maps directly to llm-orc's agent dependency graph: an `embed-text` agent produces vectors, downstream agents consume them.

### The case for llm-orc embedding (three arguments)

**1. Composability.** The semantic-extraction ensemble already does: `extract_content.py → concept-extractor → synthesizer`. Adding an embedding step would give: `extract_content.py → embed_text.py → concept-extractor + similarity-computer → synthesizer`. One ensemble produces both semantic concepts AND similarity data. The embedding is a pipeline step, not a separate system.

**2. Model provenance for free.** The ensemble YAML already names the model profile for each agent. If the embedding agent uses profile `nomic-embed-384`, that information is in the execution artifact. Plexus can read the ensemble config to know which model produced which embeddings. No additional provenance tracking infrastructure needed — it's already there in the llm-orc execution artifacts.

**3. Experimentation.** Swapping embedding models means changing one line in the ensemble YAML (`model_profile: nomic-embed-384` → `model_profile: mxbai-embed-1024`). Re-running the ensemble with a different model produces a new set of embeddings with a different provenance trail. This is exactly the "run with mistral, then run with sonnet" scenario the user described.

### The case for fastembed-rs (remains strong)

llm-orc embedding doesn't replace fastembed-rs — it complements it:

| | fastembed-rs | llm-orc embedding |
|--|------------|------------------|
| Latency | ~15–100ms (in-process) | ~20–150ms (Ollama API) |
| Dependencies | ONNX model files | Ollama running |
| Composability | Standalone | Pipeline step |
| Model swap | Code change or config | YAML change |
| EDDI real-time | Yes (fast enough) | Marginal (network hop) |
| Batch re-embedding | Possible but manual | Natural (ensemble re-run) |
| Provenance | Manual adapter ID | Automatic from ensemble |

**fastembed-rs for reactive, real-time, in-process embedding** (EDDI gesture sessions, per-ingest embedding).
**llm-orc for batch, composable, experimentable embedding** (re-embedding a context with a new model, embedding as part of extraction).

### Recommendation: Start with script primitive (Option A)

Option A (script primitive) is the pragmatic first step:
1. Write `embed_text.py` — calls Ollama `/api/embed`, returns JSON
2. Add it to the llm-orc library under `primitives/embeddings/`
3. Create an `embedding-enrichment` ensemble that takes graph export → embeds node labels → computes pairwise similarity → returns `similar_to` edge proposals
4. Wire into Plexus as an external enrichment via existing `ingest()` path

This validates the pattern without touching llm-orc core. If it proves valuable, Option B or C can formalize embeddings as first-class llm-orc citizens later.

### Revised integration architecture (three paths)

```
Path A: fastembed-rs (Rust-native)
  → Per-ingest embedding in adapter pipeline
  → Real-time, sub-100ms, no external deps
  → Adapter ID: embedding:{model_name}

Path B: llm-orc ensemble (script primitive)
  → Batch or extraction-pipeline embedding
  → Composable with semantic extraction
  → Adapter ID: from ensemble config (automatic)

Path C: Ollama direct (simple HTTP)
  → Prototyping, ad-hoc embedding
  → Same API as llm-orc uses internally
  → Adapter ID: embedding:{model_name}

All three → ingest() → similar_to edges → DiscoveryGapEnrichment (reactive)
```

### Implications

1. **llm-orc embedding is worth pursuing** — script primitive first, formalize later.
2. **It strengthens the model-aware provenance story** — ensemble YAML is already a provenance record.
3. **It doesn't replace fastembed-rs** — different tradeoffs, complementary use cases.
4. **The three paths all converge at `ingest()`** — same contribution tracking, same enrichment loop, different production methods.

---

## Question 2: Model-aware provenance — how does model identity interact with contribution tracking?

**Method:** Code exploration (contribution tracking internals) + design analysis

**Motivation:** The user raised that Plexus should track which model (LLM or embedding) produced which contributions, enabling replacement, comparison, and auditability. This connects to existing contribution tracking (Invariants 8–12) and adapter IDs (Invariant 13). The question: what does this look like architecturally, and what's missing?

### What exists today

**Contribution tracking is per-adapter-ID.** Each edge has `contributions: HashMap<AdapterId, f32>`. When an adapter emits an edge, its `id()` return value is the key:

```
edge(concept:jazz → concept:improv, "may_be_related")
  contributions: {
    "co_occurrence:tagged_with:may_be_related": 0.85,
    "extract-semantic": 0.6
  }
  raw_weight: 1.72  (sum of scale-normalized contributions)
```

**Adapter IDs are static strings** from each adapter's `id()` method:
- `"extract-semantic"` — SemanticAdapter (single ID for all invocations)
- `"graph-analysis:pagerank"` — GraphAnalysisAdapter (parameterized by algorithm)
- `"co_occurrence:tagged_with:may_be_related"` — CoOccurrenceEnrichment (parameterized)

**What works already:**
- Multiple adapters contributing to the same edge — scale normalization handles it
- Upsert semantics — re-emitting with the same adapter ID overwrites the slot
- Persistence — contributions stored as JSON in SQLite
- Evidence diversity — queryable by counting distinct adapter IDs

**What's missing:**
1. **No model identity in adapter IDs.** `"extract-semantic"` doesn't say which LLM produced the extraction. Two runs with different models overwrite the same slot.
2. **No contribution removal.** There is no way to say "remove adapter X's contribution from this edge." The only operations are add/overwrite (emit) and delete entire edge (edge removal).
3. **No batch contribution retraction.** There is no way to say "remove all contributions from adapter X across all edges."

### Design: Model-aware adapter IDs

The adapter ID is the natural place to encode model identity. The scheme:

```
{adapter_type}:{model_name}

Examples:
  embedding:nomic-embed-text-v1.5
  embedding:all-MiniLM-L6-v2
  extract-semantic:mistral-7b
  extract-semantic:claude-sonnet-4-5
  graph-analysis:pagerank          (no model — pure algorithm)
```

**Why model name, not version?** Model names are the user-facing identity. If nomic releases v2.0 with different embeddings, the user would use a different model name (`nomic-embed-text-v2`). If they want to keep both sets of contributions, both adapter IDs coexist in the contributions map. If they want to replace, they retract the old adapter's contributions and re-embed with the new one.

**For llm-orc ensembles:** The adapter ID can be derived from the ensemble config's model profile. The `SemanticAdapter` would use `extract-semantic:{profile_model}` instead of the static `"extract-semantic"`. This means the same adapter code, parameterized by the model it used.

**For fastembed-rs:** The embedding adapter would use `embedding:{model_name}` directly.

**Impact on existing code:** `SemanticAdapter::id()` changes from a static string to a parameterized one. This is a minor breaking change for existing graphs — old contributions keyed under `"extract-semantic"` wouldn't match the new scheme. Migration path: treat the old key as a legacy contribution (no model attribution).

### Design: Contribution retraction

To support "replace a model's contributions," we need a new emission primitive:

```rust
pub struct ContributionRetraction {
    pub adapter_id: AdapterId,
}
```

Semantics: "Remove all contributions from this adapter ID across all edges in the context." After retraction, `recompute_raw_weights()` runs, and edges that lose their last contribution get pruned (raw_weight drops to 0).

**Why whole-adapter retraction, not per-edge?** The use case is "I ran embedding model X and now I want to undo that." This is always a batch operation — you don't retract from one edge, you retract from all edges that model touched. Per-edge retraction is unnecessary complexity.

**Implementation in emit_inner:**

```rust
// Phase 0: Process retractions
for retraction in &emission.retractions {
    for edge in ctx.edges_mut() {
        edge.contributions.remove(&retraction.adapter_id);
    }
}
// ... then proceed with normal emission phases ...
// recompute_raw_weights() at the end handles the cleanup
```

**Edge pruning:** After retraction + recompute, edges with `contributions.is_empty()` and `raw_weight == 0.0` could be pruned. This is optional — a zero-weight edge is harmless but wasteful. Could be a separate cleanup pass.

**Graph event:** A new event type `ContributionsRetracted { adapter_id, edge_count }` would let enrichments react. For example, DiscoveryGapEnrichment might detect that removing a model's `similar_to` contributions invalidates some `discovery_gap` edges.

### Design: Multi-model composition on the same edge

**Scenario:** Two embedding models both produce `similar_to` edges between concept:jazz and concept:improv, but with different similarity scores.

```
edge(concept:jazz → concept:improv, "similar_to")
  contributions: {
    "embedding:nomic-embed-text-v1.5": 0.82,
    "embedding:all-MiniLM-L6-v2": 0.71
  }
  raw_weight: 1.94  (sum of scale-normalized)
```

**This already works.** Scale normalization ensures the two models' different score ranges don't bias the result. The combined raw_weight reflects agreement — if both models say the pair is similar, the weight is higher than if only one does.

**Comparison:** A user can query contributions to see which models agree and which disagree:
- Both high → strong latent evidence
- One high, one low → models disagree, worth investigating
- One present, one absent → only one model found similarity

**Replacement:** Retract `embedding:all-MiniLM-L6-v2`, re-embed with `embedding:snowflake-arctic-embed-s`. The old contributions disappear, new ones appear. The edge's raw_weight adjusts automatically.

### Design: LLM extraction provenance

The same pattern applies to semantic extraction. Two runs with different LLMs:

```
edge(file:docs/example.md → concept:jazz, "tagged_with")
  contributions: {
    "extract-semantic:mistral-7b": 0.6,
    "extract-semantic:claude-sonnet-4-5": 0.9
  }
```

This captures exactly what the user described: "we ran semantic extraction once with mistral 7B but then we decided to do it again with claude sonnet." Both contributions coexist. The user can:
- **Keep both** — the combined weight reflects multi-model agreement
- **Replace one** — retract `extract-semantic:mistral-7b`, keep sonnet's contributions
- **Compare** — query the contributions map to see which concepts each model found

### Auditability

**Embedding provenance is indirect but traceable.** An embedding isn't directly auditable (you can't read a 768-dimensional vector and understand what it means). But the provenance chain is:

1. **Which model?** Adapter ID: `embedding:nomic-embed-text-v1.5`
2. **What input?** The node's content (label, properties) — the text that was embedded
3. **What output?** The `similar_to` edges it produced, with similarity scores as contribution values
4. **When?** Framework-constructed provenance timestamp
5. **Why these edges?** Similarity threshold (configurable parameter, logged in ensemble config or adapter config)

The user can't audit the embedding itself, but they can audit its *effects* — which pairs it connected and at what strength. Combined with contribution retraction, they can selectively undo those effects.

### Summary: What needs to be built

**Minimal viable model-aware provenance (3 changes):**

1. **Parameterized adapter IDs** — `SemanticAdapter` and the new embedding adapter include model name in their `id()`. Pure rename, no new infrastructure.

2. **ContributionRetraction emission type** — "Remove all contributions from adapter X." New struct, new phase in `emit_inner`, new graph event. Moderate implementation effort.

3. **Edge pruning after retraction** — Clean up zero-contribution edges. Optional but prevents ghost edges.

**Not needed now:**
- CRDT tombstones (ADR-017 mentions for federated replication — defer)
- Per-edge retraction (batch is sufficient)
- Version vectors (simple overwrite is fine for single-writer contexts)

### Implications

1. **Existing contribution tracking does 80% of the work.** Model identity slots into adapter IDs. Multi-model composition slots into scale normalization. The gap is retraction.
2. **ContributionRetraction is the genuinely new primitive.** Everything else is parameterization of existing mechanisms.
3. **This applies to both embeddings AND LLM extraction.** The same mechanism handles "swap embedding model" and "swap extraction LLM."
4. **llm-orc ensemble provenance strengthens this** — the ensemble YAML records which model profile each agent used, providing a durable audit trail for what adapter ID means.

---

## Question 3: Discovery gap computation — is the reactive core enrichment sufficient?

**Method:** Design analysis (existing infrastructure + Q1/Q2 findings)

**Context:** `DiscoveryGapEnrichment` exists as a core enrichment (ADR-024, built and tested). It reacts to new `similar_to` edges and flags structurally-disconnected but latently-similar pairs. The domain model asks: should a batch sweep (external enrichment) also compute gaps for the whole graph periodically?

### Reactive enrichment behavior

`DiscoveryGapEnrichment` fires in the enrichment loop when `EdgesAdded` events include edges with the trigger relationship (e.g., `similar_to`). For each new trigger edge (A, B):
1. Check if any structural edge exists between A and B (excluding trigger and output relationships)
2. If no structural evidence → emit `discovery_gap` edge

This is **incremental** — it only processes new trigger edges, not the full graph. After a batch embedding run that produces 500 new `similar_to` edges, the enrichment loop fires and processes all 500 in one pass.

### When would a batch sweep find something the reactive enrichment missed?

**Case 1: Structural evidence removed.** If a `tagged_with` edge is deleted (e.g., a tag removed from a fragment), a pair that had structural evidence no longer does. If a `similar_to` edge already exists between them, the reactive enrichment won't re-fire because no new `similar_to` edge was added. A batch sweep would catch this.

**Case 2: Pre-existing `similar_to` edges.** If `similar_to` edges existed before `DiscoveryGapEnrichment` was registered, the enrichment never processed them. A batch sweep would catch these.

**Case 3: Schema evolution.** If the definition of "structural evidence" changes (e.g., a new relationship type is added), existing discovery gap assessments may be wrong. A batch sweep would re-evaluate.

### Assessment: Reactive is sufficient for v1

Cases 1–3 are real but rare:
- **Case 1** requires edge deletion, which is uncommon in Plexus (additive graph)
- **Case 2** is a bootstrap problem — solvable by running a one-time batch at registration
- **Case 3** is a schema migration, handled by re-running the embedding pipeline

A batch sweep is trivially implementable as an external enrichment (llm-orc ensemble that reads the graph export, finds all `similar_to` pairs without structural evidence, returns `discovery_gap` proposals). But it's not needed as a built-in — the reactive enrichment handles the steady-state, and a one-time batch handles bootstrap.

### Recommendation

**Ship with reactive only.** If a user needs a full sweep, `plexus analyze` with an appropriate external enrichment ensemble handles it. No new infrastructure needed.

### Implications

The reactive core enrichment is the right default. A batch sweep is a nice-to-have external enrichment, not a core requirement. This simplifies the build scope.

---

## Question 4: Embedding drift — what happens when models are updated?

**Method:** Design analysis (builds on Q2 findings)

**Context:** When an embedding model is updated (new version, fine-tuned variant, or entirely different model), the vector space changes. `similar_to` edges computed by the old model may no longer be valid. How does Plexus handle this?

### The contribution tracking answer

Q2 established that model identity lives in adapter IDs: `embedding:nomic-embed-text-v1.5`. A new model version gets a new adapter ID: `embedding:nomic-embed-text-v2.0`.

**Scenario: Replace old model with new model**

1. **Retract** old contributions: `ContributionRetraction { adapter_id: "embedding:nomic-embed-text-v1.5" }`
   - Removes old model's contribution slot from all edges
   - `recompute_raw_weights()` adjusts weights
   - Edges that lose their last contribution get pruned
2. **Re-embed** with new model: run embedding pipeline with `adapter_id: "embedding:nomic-embed-text-v2.0"`
   - New `similar_to` edges emitted via `ingest()`
   - New contribution slots created
   - DiscoveryGapEnrichment fires on new edges
3. **Result:** Graph reflects new model's understanding. Old contributions gone.

**Scenario: Keep both models for comparison**

1. Run new model alongside old (different adapter ID)
2. Both contribute to `similar_to` edges
3. Where they agree → higher combined raw_weight (scale normalization sums)
4. Where they disagree → one model's contribution present, the other absent
5. User queries contributions to compare

**Scenario: Gradual migration**

1. Run new model on a subset of nodes
2. Old and new contributions coexist
3. When satisfied, retract old model's contributions
4. Complete re-embedding with new model

### Do we need automatic re-embedding?

**No.** Embedding drift is a deliberate model change, not something that happens silently. The user decides when to re-embed. The tools needed are:
- ContributionRetraction (Q2) — remove old model's influence
- Embedding pipeline (Q1) — produce new embeddings
- Adapter ID scheme (Q2) — distinguish old from new

Automatic re-embedding (triggered by model update) is over-engineering for v1. The user runs `plexus analyze` or the embedding ensemble manually.

### Version granularity

**Model name includes version:** `embedding:nomic-embed-text-v1.5`. This is the right granularity because:
- Same model, different versions → different vector spaces → different adapter IDs
- Same model, same version, different parameters (e.g., dimension truncation) → arguably same adapter ID, since the vectors are in the same space
- Different model entirely → obviously different adapter ID

If Ollama auto-updates a model, the user should be aware that re-embedding may be needed. This is a user workflow concern, not an infrastructure concern.

### Implications

1. **Embedding drift is handled by the contribution tracking machinery** — no new infrastructure beyond ContributionRetraction (Q2).
2. **The user drives re-embedding** — no automatic triggers needed for v1.
3. **Model version in adapter ID** provides the mechanism for coexistence, comparison, and replacement.
4. **This is the same mechanism for LLM extraction drift** — "I ran mistral, now I want sonnet" follows the identical workflow.

---

## Question 5: Cross-modal embeddings — can text and movement share a vector space?

**Method:** Web research (cross-modal embedding models, motion representation, IMU embeddings)

### The research landscape

Cross-modal embedding is a very active research area. The dominant approach: **contrastive learning** (CLIP-style), where paired examples from two modalities train dual encoders to produce aligned embedding spaces.

### Models directly relevant to EDDI

| Model | Modalities | Approach | Local? | EDDI relevance |
|-------|-----------|----------|--------|---------------|
| **ImageBind** (Meta) | Text, image, audio, depth, thermal, **IMU** | Contrastive via image anchor | Yes (PyTorch, CC-BY-NC) | **IMU is a first-class modality** — accelerometer/gyroscope data embedded alongside text |
| **TMR / TMR++** | Text + SMPL skeletal motion | Dual-encoder contrastive | Yes (PyTorch, pretrained) | Best for text-motion retrieval, but requires skeletal data format |
| **MotionCLIP** | Text (CLIP space) + motion | Motion autoencoder → CLIP space | Yes | Motion in CLIP's text embedding space |
| **CLAP** (LAION) | Text + audio/music | Contrastive | Yes (HuggingFace) | Relevant if EDDI sessions have audio components |

### ImageBind: The direct path

ImageBind is the most immediately applicable. Its IMU encoder processes 5-second clips of 6-axis sensor data (accelerometer + gyroscope, 2000 Hz sampling) and produces 1024-dimensional embeddings in the same space as text. This means:

```
embed("a person moves with strong, sustained effort") → text vector
embed(imu_clip_from_gesture_session) → IMU vector
cosine_similarity(text_vector, imu_vector) → cross-modal similarity
```

**Caveat:** ImageBind's IMU training used egocentric video + IMU pairs (general activities like walking, cooking, sports). EDDI's expressive performance movement is a different distribution. The embeddings would capture gross movement characteristics but miss Laban-specific qualities. Fine-tuning or adapter training would be needed for high-quality results.

### TMR: The motion-specialist path

TMR produces the highest-quality text-motion embeddings (256d) but requires SMPL skeletal joint data. If EDDI captures or derives skeletal data, TMR is the best choice for "find movement sessions similar to this text description." TMR++ improves cross-dataset generalization.

### The pragmatic path for EDDI (no new model training)

EDDI already works with Laban effort dimensions: weight (strong/light), space (direct/indirect), time (sudden/sustained), flow (bound/free). These are inherently textual descriptions of movement quality.

**Approach: Embed Laban annotations as text.**

1. A gesture session produces structured Laban data: `{weight: 0.8, space: 0.3, time: 0.9, flow: 0.5}`
2. An adapter converts this to natural language: `"strong, indirect, sudden, moderate flow"`
3. A text embedding model (nomic-embed-text via fastembed-rs) embeds this description
4. Text fragments are embedded with the same model
5. Cosine similarity in text space → cross-modal similarity

This is low-fidelity but zero-infrastructure. The text embedding model doesn't understand movement, but it understands the *words* used to describe movement. "Strong, sudden effort" as a Laban annotation will be similar to "powerful explosive motion" in a writing fragment — because the words are semantically related.

**Limitations:** This only captures what's expressible in Laban vocabulary. Subtle movement qualities that don't have good textual descriptions would be lost. But for Plexus's discovery gap use case (flagging potential connections for a human to evaluate), this level of fidelity may be sufficient.

### The future path (adapter fine-tuning)

For higher fidelity:

1. Collect a dataset of (gesture clip, text description) pairs from EDDI performances
2. Train a small MLP adapter that projects EDDI's motion features into the frozen text encoder's embedding space
3. This is the MotionCLIP / TMR approach applied to EDDI's specific motion representation

This requires hundreds (not thousands) of annotated pairs and is a tractable research project, but not v1 scope.

### Implications for Plexus architecture

1. **Cross-modal embeddings don't change the graph architecture.** `similar_to` edges between a movement node and a text fragment node use the same edge structure, contribution tracking, and enrichment pipeline as same-modal similarities.

2. **The adapter ID distinguishes cross-modal from same-modal.** `embedding:nomic-embed-text-v1.5:cross-modal` vs `embedding:nomic-embed-text-v1.5:text-only`. Or more simply, the adapter that produces the embeddings carries its identity.

3. **DiscoveryGapEnrichment works unchanged.** It doesn't care whether the `similar_to` edge connects two text nodes or a text node and a movement node. It checks for structural evidence between them regardless.

4. **The pragmatic Laban-as-text path requires no new embedding infrastructure.** It's a domain adapter concern — EDDI's adapter converts structured Laban data to text, then the standard text embedding pipeline handles it.

5. **ImageBind is worth a spike** when EDDI matures. A focused experiment: embed a few EDDI gesture clips via ImageBind's IMU encoder, embed corresponding text descriptions via ImageBind's text encoder, measure cosine similarity. This would validate (or invalidate) the cross-modal path without committing to infrastructure.

### Addendum: EDDI's mathematical representation

The user clarified an important distinction: EDDI models movement as continuous mathematical properties, not as classification labels. "Flow" isn't a label applied to a gesture — it's a continuous value on a spectrum. `similar_to` edges between movement nodes would come from proximity in this mathematical parameter space (Euclidean distance, cosine similarity on Laban dimension vectors), not from text classification.

This means the cross-modal bridge is the harder problem. A writing fragment about "flowing, sustained motion" lives in text embedding space; a gesture session with `{flow: 0.85, time: 0.2}` lives in a mathematical parameter space. The Laban-as-text approach collapses the mathematical representation into words, losing the continuous nuance that makes EDDI's approach distinctive.

The deeper research question (future cycle): **what projection maps a Laban parameter vector into a text embedding space such that nearby points in parameter space land near semantically related text?** This is the adapter fine-tuning path, but the training signal is interesting because EDDI's mathematical representation could be *more* precise than any text description of movement.

For Plexus architecture: this doesn't change anything. Within-domain `similar_to` edges (movement-to-movement) come from mathematical similarity. Cross-domain `similar_to` edges (movement-to-text) require a projection — the mechanism is TBD but the graph structure is the same. DiscoveryGapEnrichment doesn't care about the geometry that produced the edge.

---
