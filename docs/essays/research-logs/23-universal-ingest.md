# Research Log: Universal Ingest Abstraction and Ensemble Convergence

## Background

Essay 22 identified conformance gaps (items 1–5, now closed) and a forward roadmap (items 6–8). Items 6–8 ask: what is the right ensemble selection strategy, can a literary extraction ensemble work, and does the Macbeth integration spike produce convergence?

In exploring these questions, the user challenged the foundational premise. Instead of domain-specific ensembles (one for code, one for literature, one for documentation), the question became: **can a single general-purpose semantic extraction ensemble produce meaningful convergence across content types?** If semantic knowledge is semantic knowledge regardless of source, then pre-sorting inputs by domain is unnecessary overhead — and may actually prevent the most interesting graph connections from forming.

Three research questions emerged:

1. **Q1: What should `ingest` at the MCP level look like?** The current MCP surface exposes `annotate` — a narrow write path for file-location marks. But the internal API already has `PlexusApi.ingest()`, the universal write endpoint. How do we expose that generality at the MCP transport layer?

2. **Q2: Can a single general-purpose extraction ensemble produce meaningful convergence across content types?** Method: spike with Macbeth + Plexus source file through the same ensemble.

3. **Q3: If extraction lives in ensembles, what's left for Plexus adapters?** With declarative adapters (ADR-020) pointing at llm-orc ensembles, what does the Plexus-side pipeline become?

### Forcing function

The Macbeth integration spike: ingest a scene from Macbeth and a Plexus source file into the same context, through the same ensemble, and observe convergence patterns in the graph. Two new draft essays inform this work: `docs/codebase-semantic-intelligence.md` (ensemble architecture for codebase knowledge) and `docs/mlx-training-guide-expanded.md` (LoRA training and ensemble distribution).

---

## Question 1: What should `ingest` at the MCP level look like?

**Method:** Code exploration and design analysis

### What exists today

**Internal API — `PlexusApi.ingest()`** (`src/api.rs:36`):
```rust
pub async fn ingest(
    &self,
    context_id: &str,
    input_kind: &str,
    data: Box<dyn std::any::Any + Send + Sync>,
) -> Result<Vec<OutboundEvent>, AdapterError>
```

This is the universal write endpoint (ADR-012). All graph writes go through it. The `input_kind` string routes to matching adapters; `data` is type-erased so any adapter can accept any input type.

**`PlexusApi.annotate()`** (`src/api.rs:48`) is convenience orchestration built *on top of* `ingest()`. It calls `ingest()` three times:
1. `ingest(ctx, "annotate", FragmentInput{...})` — create the text fragment
2. `ingest(ctx, "provenance", ProvenanceInput{chain_name, ...})` — ensure chain exists
3. `ingest(ctx, "provenance", ProvenanceInput{mark, ...})` — create the mark

So `annotate` is not a primitive — it's a composed operation that routes through `ingest()`.

**MCP server** (`src/mcp/mod.rs`): Exposes `annotate` as the only write tool. The pipeline registers only `FragmentAdapter("annotate")` + `ProvenanceAdapter` with `TagConceptBridger` and `CoOccurrenceEnrichment`. No `ExtractionCoordinator`, no `SemanticAdapter`, no `DeclarativeAdapter`. File extraction is impossible from MCP.

**IngestPipeline** (`src/adapter/ingest.rs`): Routes by `input_kind` with fan-out (multiple adapters can match). Runs enrichment loop after all matched adapters process. Returns merged outbound events.

**DeclarativeAdapter** (`src/adapter/declarative.rs`): Already accepts JSON (`serde_json::Value`) as input. YAML-driven spec interpreter. This is the natural bridge between MCP's JSON transport and the Rust adapter pipeline.

**LlmOrcClient** (`src/llm_orc.rs`): `SubprocessClient` spawns `llm-orc m serve --transport stdio` and communicates via MCP JSON-RPC. `invoke(ensemble_name, input_data) -> InvokeResponse` with results as `HashMap<String, AgentResult>`.

### The gap

The MCP surface has a structural bottleneck: it can only write through `annotate`, which produces fragment + chain + mark. But the internal pipeline can handle *any* input kind — file extraction, semantic analysis, fragments, provenance, declarative specs. The MCP surface doesn't reflect that generality.

### Design: `ingest` as the MCP write tool

The MCP `ingest` tool should mirror the internal API:

```
Tool: ingest
Parameters:
  input_kind: string   — routes to matching adapter(s)
  data: object          — JSON payload, adapter-specific
```

The MCP server deserializes `data` as `serde_json::Value` and passes it to `PlexusApi.ingest()`. The adapter registered for that `input_kind` receives the JSON and interprets it.

**Input kinds and their data shapes:**

| input_kind | data shape | adapter | what it does |
|-----------|-----------|---------|-------------|
| `"annotate"` | `{text, tags, source}` | FragmentAdapter | Create a text fragment with tags |
| `"extract-file"` | `{file_path}` | ExtractionCoordinator | Full extraction pipeline (Phase 1→2→3) |
| `"semantic"` | `{file_path, sections}` | SemanticAdapter | Phase 3 semantic extraction via llm-orc |
| `"fragment"` | `{text, tags, source}` | FragmentAdapter | Alias for annotate's fragment step |
| `"provenance"` | `{chain_name, file, line, ...}` | ProvenanceAdapter | Create chain or mark |
| *any declarative spec name* | `{...}` | DeclarativeAdapter | Spec-driven graph mutations |

**What happens to `annotate`?**

`annotate` goes away. `ingest` is the single MCP write tool — one tool, one path. The current `annotate` behavior (fragment + chain + mark) is just one kind of ingest: `ingest(input_kind="annotate", data={...})`. No need for two write tools when one handles everything.

**Provenance is derived, not caller-specified.** The current `annotate` flow has the caller orchestrating provenance: specifying chain_name, file, line, annotation. This is backwards. Semantic extraction should derive and report its own provenance. The caller provides input data (a file path, a text fragment, whatever); the extraction pipeline determines what's significant and creates its own provenance trail. The caller receives back what was found, not instructions on what to record.

This means:
- For file extraction: `ingest(input_kind="extract-file", data={file_path: "..."})` — the pipeline extracts, creates fragments, derives provenance from the extraction results, and reports back
- For manual annotation: `ingest(input_kind="annotate", data={file, line, text, tags})` — the pipeline creates the fragment and provenance from the supplied data
- For ensemble-driven extraction: `ingest(input_kind="semantic", data={file_path, ...})` — the llm-orc ensemble extracts and the declarative adapter maps results to graph mutations including provenance

In all cases, `ingest` is the single entry point. What varies is what the adapters do with the input — including how they derive provenance.

### The JSON bridge problem

`PlexusApi.ingest()` takes `Box<dyn Any>`. MCP tools produce JSON. The bridge:

1. MCP server receives `ingest(input_kind, data)` where `data` is `serde_json::Value`
2. For adapters that accept JSON directly (DeclarativeAdapter), pass the JSON through
3. For adapters that expect typed structs (FragmentAdapter, ExtractionCoordinator), the MCP layer deserializes JSON into the expected type before boxing

This means the MCP server needs a small routing table: `input_kind → deserialize JSON into correct type`. Or — simpler — all MCP-facing adapters accept `serde_json::Value`, and the adapter itself handles deserialization. DeclarativeAdapter already does this. The others would need a thin wrapper or a `from_json()` constructor on their input types.

The cleaner path: **define a `JsonInput` wrapper** that any adapter can downcast from:

```rust
pub struct JsonInput(pub serde_json::Value);
```

Adapters that support MCP input check for `JsonInput` first, then fall back to their typed input. This keeps the existing typed-input path (for Rust callers) while adding a JSON path (for MCP callers).

### Pipeline registration for MCP

The current MCP server registers only FragmentAdapter + ProvenanceAdapter. To support `ingest` with file extraction, the MCP server needs:

1. **ExtractionCoordinator** — for `input_kind="extract-file"` (Phase 1→2→3)
2. **SemanticAdapter with llm-orc client** — for Phase 3 semantic extraction
3. **DeclarativeAdapter** — for any YAML-spec-driven extraction
4. **EmbeddingSimilarityEnrichment** — for latent evidence production
5. **DiscoveryGapEnrichment** — for latent-structural gap detection

This is the "full pipeline" MCP server vs. the current "annotate-only" MCP server. The construction in `PlexusMcpServer::new()` would grow to register the full adapter set.

### Implications

1. **`ingest` at MCP level is a thin shell over `PlexusApi.ingest()`** — the internal abstraction already exists and is correct (Invariant 34).
2. **DeclarativeAdapter is the natural bridge** — JSON in, graph mutations out. MCP-facing extraction can be declarative specs that invoke llm-orc ensembles.
3. **`annotate` goes away as a separate tool.** `ingest` is the single write path at every layer: Rust API, MCP transport, everything. One tool, one path.
4. **Provenance is derived, not caller-specified.** Extraction pipelines and ensembles determine and report their own provenance. The caller provides input; the pipeline reports findings.
5. **The MCP server needs the full pipeline** — not just FragmentAdapter + ProvenanceAdapter. This is the structural gap that prevents file extraction from MCP.
6. **A `JsonInput` wrapper** or adapter-level JSON deserialization bridges the `Box<dyn Any>` / `serde_json::Value` gap.

---

## Question 2: Can a single general-purpose extraction ensemble produce meaningful convergence across content types?

**Method:** Spike — run the same ensemble (`plexus-semantic`) on Macbeth Act 1 Scene 7 and Plexus `src/adapter/ingest.rs`, compare outputs.

**Spike question:** Does a domain-agnostic extraction prompt produce meaningful concepts and relationships from both literary text and source code, without domain-specific routing?

### Setup

Ensemble: `plexus-semantic` — single agent, generic system prompt asking for concepts and relationships in JSON. No domain-specific instructions.

Two models tested: llama3 8B and mistral 7B.

Two inputs:
- **Macbeth Act 1, Scene 7** — the "If it were done" soliloquy + Lady Macbeth's persuasion. Rich with themes of ambition, moral conflict, duty, manipulation, trust.
- **`src/adapter/ingest.rs`** — the unified ingest pipeline. Architectural concepts: routing, adapters, enrichment, fan-out, events.

### Results

**Macbeth extraction (mistral 7B):**
```json
{
  "concepts": [
    {"label": "macbeth", "confidence": 0.9},
    {"label": "assassination", "confidence": 0.8},
    {"label": "justice", "confidence": 0.7},
    {"label": "virtue", "confidence": 0.6},
    {"label": "ambition", "confidence": 0.5},
    {"label": "lady macbeth", "confidence": 0.4},
    {"label": "duncan", "confidence": 0.3},
    {"label": "chamberlains", "confidence": 0.2}
  ],
  "relationships": [
    {"source": "macbeth", "target": "assassination", "relationship": "describes|related_to", "weight": 0.7},
    {"source": "justice", "target": "assassination", "relationship": "describes|related_to", "weight": 0.6},
    {"source": "macbeth", "target": "ambition", "relationship": "uses|creates", "weight": 0.5},
    {"source": "lady macbeth", "target": "macbeth", "relationship": "describes|related_to", "weight": 0.4},
    {"source": "duncan", "target": "assassination", "relationship": "describes|related_to", "weight": 0.3},
    {"source": "chamberlains", "target": "macbeth", "relationship": "part_of|uses", "weight": 0.2}
  ]
}
```

**Ingest pipeline extraction (mistral 7B):**
```json
{
  "concepts": [
    {"label": "ingest pipeline", "confidence": 0.9},
    {"label": "unified ingest pipeline", "confidence": 0.8},
    {"label": "pipeline steps", "confidence": 0.7},
    {"label": "engine sink", "confidence": 0.6},
    {"label": "enrichment registry", "confidence": 0.5},
    {"label": "adapter error", "confidence": 0.4},
    {"label": "outbound event", "confidence": 0.3}
  ],
  "relationships": [
    {"source": "ingest pipeline", "target": "engine sink", "relationship": "uses", "weight": 0.9},
    {"source": "ingest pipeline", "target": "enrichment registry", "relationship": "describes", "weight": 0.8},
    {"source": "pipeline steps", "target": "adapter error", "relationship": "related_to", "weight": 0.7},
    {"source": "adapter error", "target": "outbound event", "relationship": "creates", "weight": 0.6}
  ]
}
```

### Analysis

**What worked:**

1. **The ensemble handles both content types.** No domain routing, no content-type detection. The same generic prompt produces valid, parseable JSON from both Shakespeare and Rust code. The pipeline shape is content-agnostic.

2. **Thematic extraction from literature.** The model extracted abstract concepts (ambition, justice, virtue) alongside characters (macbeth, duncan, lady macbeth). This is exactly what a knowledge graph wants — not just named entities, but the thematic layer.

3. **Architectural extraction from code.** The model extracted domain concepts (ingest pipeline, engine sink, enrichment registry, outbound event) and architectural relationships (pipeline uses engine sink). It read the doc comments and code structure, not just identifiers.

**What needs improvement:**

1. **Confidence calibration.** Both models assigned monotonically decreasing scores (0.9, 0.8, 0.7...) rather than genuinely calibrated confidence. This is a prompt issue — the prompt should either drop confidence or provide calibration examples.

2. **Relationship type discipline.** Models used pipe-separated values ("describes|related_to") instead of picking one from the allowed set. The prompt lists relationship types but the models hedge by giving two. Stricter prompting or post-processing would fix this.

3. **Near-duplicate concepts.** "ingest pipeline" and "unified ingest pipeline" — the model extracted both the short and long form. Deduplication is a synthesis concern (the `semantic-extraction` ensemble's synthesizer stage handles this).

4. **Some wrong relationships.** "adapter error creates outbound event" is incorrect. The model is pattern-matching structure names without understanding semantics. A larger model or better prompt would improve this.

### The convergence question

**No direct concept overlap between Macbeth and the pipeline code.** This is expected and correct — they share no domain vocabulary. The interesting question is: what would cause convergence if both lived in the same graph context?

Three mechanisms:

1. **Embedding similarity.** "ambition" (Macbeth) and "ambition" in a project retrospective would land near each other in vector space. "justice" (Macbeth) and "fairness" in an access control document would be similar. Convergence requires more than two sources — it requires a corpus where thematic vocabulary bridges domains.

2. **Co-occurrence via shared context.** If a user annotates both Macbeth and Plexus code in the same context (because they're researching "how power structures manifest in systems"), the co-occurrence enrichment would detect that both sources are relevant to the same investigation. The convergence is in the user's context, not in the content.

3. **Propagation via documentation.** If an essay discusses both Macbeth's themes AND software architecture (as the codebase-semantic-intelligence essay does), concepts from the essay would create bridges. "ambition" connects to both literary and technical concepts through the bridging document.

**Key insight: convergence is a graph-level phenomenon, not an extraction-level one.** Individual extractions from different domains won't share concepts. Convergence emerges when the enrichment loop (co-occurrence, embedding similarity, discovery gaps) operates on a context containing diverse sources. The ensemble's job is to extract meaningful concepts from each source; the graph's job is to find the connections.

### What this means for the ensemble strategy

The spike validates the user's hypothesis: **you don't need domain-specific ensembles.** A single general-purpose extraction ensemble works because:

- The prompt asks for "concepts and relationships" — a universal instruction
- The model's training already includes knowledge of literature, code, science, etc.
- Domain-specific vocabulary emerges naturally from the content, not from the prompt
- What would be lost with domain-specific routing (cross-domain connections) is exactly what makes the graph interesting

**Domain-specificity belongs at the structural layer, not the semantic layer.** A script that reads an AST (code) vs. a script that detects section boundaries (prose) vs. a script that parses YAML (config) — these are structural preprocessors. The semantic extraction that follows is the same regardless of input type.

This maps to the `semantic-extraction` ensemble's architecture: `content-extractor` (script, structural) → `concept-extractor` (LLM, semantic) → `synthesizer` (LLM, deduplication). The first stage is content-type-aware; the second and third are universal.

### Implications

1. **One ensemble, not N.** The `plexus-semantic` / `semantic-extraction` ensemble handles all content types. `code-concepts` is redundant — its code-specific prompts don't add enough value over generic extraction to justify a separate pipeline.
2. **Model quality matters more than prompt specificity.** The difference between llama3 8B and mistral 7B was marginal. The difference between 7B and 70B would be substantial. Investment should go to model capability, not prompt engineering per domain.
3. **The content-extractor script is the right place for structural awareness.** Detect MIME type, chunk appropriately (by AST for code, by section for prose, by paragraph for short text), then hand the chunks to a universal semantic extractor.
4. **Convergence requires the full pipeline** — extraction alone doesn't produce connections. Embeddings, co-occurrence, and discovery gaps are the mechanisms. The spike confirms extraction works; the next step is wiring the enrichment loop.

---

## Question 3: If extraction lives in ensembles, what's left for Plexus adapters?

**Method:** Design analysis — trace current adapter responsibilities against the ensemble architecture

### What SemanticAdapter does today

`src/adapter/semantic.rs` performs five responsibilities:

1. **Orchestration** — calls `client.invoke(ensemble_name, input_text)` to run the llm-orc ensemble
2. **Response extraction** — `extract_json()` strips markdown fences and finds the JSON object in the LLM response
3. **Graph mapping** — `parse_response()` converts JSON concepts/relationships into `Node`/`Edge` graph primitives with correct dimensions, IDs, and content types
4. **Provenance creation** — `add_provenance()` creates chain nodes, mark nodes, contains edges, and tags that trigger TagConceptBridger
5. **Graceful degradation** — checks llm-orc availability, returns `AdapterError::Skipped` when unavailable (Invariant 47)

### What DeclarativeAdapter already does

`src/adapter/declarative.rs` handles responsibility #3 (graph mapping) through YAML specs. It accepts `serde_json::Value` as input and interprets a declarative spec of primitives (`create_node`, `create_edge`, `for_each`, `hash_id`, `create_provenance`, `update_properties`). Template expressions interpolate input fields via `{input.field}` syntax.

This means: **SemanticAdapter's `parse_response()` is a hardcoded version of what DeclarativeAdapter does declaratively.** The 80 lines of Rust that create concept nodes, tagged_with edges, and relationship edges from JSON could be a 20-line YAML spec.

### What the ensemble already does

The `semantic-extraction` ensemble handles responsibilities #1 and #2:
- Stage 1 (content-extractor script): reads the file, detects MIME type, chunks the content
- Stage 2 (concept-extractor LLM): extracts concepts and relationships from each chunk
- Stage 3 (synthesizer LLM): merges chunk-level results into file-level JSON

The ensemble produces structured JSON. SemanticAdapter receives that JSON and maps it to graph primitives.

### What changes when extraction lives fully in ensembles

**Responsibility #1 (orchestration) moves to the ingest pipeline.** When the MCP `ingest` tool receives `input_kind="extract-file"`, the pipeline routes to an adapter that invokes the ensemble. This is still an adapter's job — but a thin one. The adapter calls `llm_orc_client.invoke()` and passes the result to the declarative mapper.

**Responsibility #2 (response extraction) is unnecessary.** If the ensemble's output is already structured JSON (which it is — `output_format: json` in the YAML), the adapter doesn't need to extract JSON from prose. The JSON robustness logic (`extract_json()`) is a workaround for LLMs that don't reliably produce clean JSON. With better models or structured output modes, this goes away.

**Responsibility #3 (graph mapping) moves to DeclarativeAdapter.** A YAML spec maps the ensemble's JSON output to graph primitives. No Rust code needed for new extraction types — add a YAML spec, not an adapter.

**Responsibility #4 (provenance) is derived by the pipeline, not specified by the caller.** Per the user's direction: semantic extraction derives and reports its own provenance. The ensemble produces extraction results; the declarative mapper creates provenance from those results (chain for the extraction run, marks for each section/passage, tags from concept labels). The provenance primitives already exist in DeclarativeAdapter: `create_provenance`.

**Responsibility #5 (graceful degradation) stays.** This is a pipeline concern — if llm-orc is unavailable, the extraction adapter should degrade gracefully. This logic lives in whatever thin adapter wraps the ensemble invocation.

### The new adapter landscape

| Layer | Responsibility | Implementation |
|-------|---------------|----------------|
| **Ensemble** (llm-orc) | Semantic extraction — reads content, produces structured JSON | YAML ensemble spec + model profiles |
| **Declarative mapper** (Plexus) | JSON → graph primitives — nodes, edges, provenance | YAML adapter spec (DeclarativeAdapter) |
| **Pipeline** (Plexus) | Routing, enrichment loop, event merging | IngestPipeline (Rust, unchanged) |
| **Enrichments** (Plexus) | Co-occurrence, embedding similarity, discovery gaps, tag bridging | Existing enrichment implementations (Rust) |

The Plexus adapter becomes a **declarative mapper**: a YAML spec that says "for each concept in the ensemble output, create a node in the semantic dimension; for each relationship, create an edge; create a provenance chain named after the ensemble run."

### What stays in Rust

1. **IngestPipeline** — routing, fan-out, enrichment loop, event merging. This is the core engine. No change needed.
2. **EngineSink** — the emit/validate/persist bridge. No change.
3. **Enrichments** — CoOccurrenceEnrichment, DiscoveryGapEnrichment, EmbeddingSimilarityEnrichment, TagConceptBridger. These are reactive graph transformations that must be fast and correct. Rust is the right home.
4. **DeclarativeAdapter** — the YAML interpreter. Rust implementation, YAML specs.
5. **LlmOrcClient** — the subprocess bridge to llm-orc. Rust implementation.
6. **FragmentAdapter** — still needed for simple text fragment ingestion (annotations, notes). Thin and stable.
7. **ProvenanceAdapter** — still needed for direct provenance operations. Thin and stable.

### What moves to YAML

1. **SemanticAdapter's graph mapping** — the `parse_response()` logic becomes a declarative spec
2. **New extraction types** — instead of writing a new Rust adapter, write a YAML spec + ensemble YAML
3. **Provenance patterns** — chain/mark/contains creation per extraction type, declaratively

### What moves to llm-orc

1. **Content reading and chunking** — the content-extractor script
2. **Concept/relationship extraction** — LLM agents with extraction prompts
3. **Synthesis and deduplication** — synthesizer agents that merge chunk-level results
4. **Any future structural analysis** — AST parsing, section detection, metadata extraction (script agents)

### The thin adapter pattern

For ensemble-driven extraction, the adapter becomes:

```
1. Receive JSON input from ingest()
2. Check llm-orc availability → degrade gracefully if down
3. Invoke ensemble with input data
4. Pass ensemble output JSON to DeclarativeAdapter
5. DeclarativeAdapter maps JSON → graph primitives via YAML spec
6. Pipeline runs enrichment loop
7. Return outbound events
```

Steps 2–3 could be a generic `EnsembleAdapter` that takes an ensemble name and a declarative spec name as config. No custom Rust per extraction type.

### Implications

1. **Plexus adapters become declarative mappers.** Extraction is the ensemble's job. Mapping is the YAML spec's job. The adapter trait provides the pipeline contract (routing, sink, transform).
2. **Adding a new extraction type means: write an ensemble YAML + a mapper YAML.** No Rust code for new content types. This is the power of the llm-orc integration.
3. **SemanticAdapter's Rust code is transitional.** Its `parse_response()` hardcodes what should be a declarative spec. The adapter itself should become a thin `EnsembleAdapter` that delegates to DeclarativeAdapter.
4. **The enrichment pipeline stays in Rust.** Enrichments are reactive graph transformations that run at ingest time — they need to be fast, correct, and tightly coupled to the graph engine. This is not ensemble work.
5. **Provenance is derived from extraction results, not caller-specified.** The declarative mapper creates provenance as part of the graph mapping. The ensemble reports what it found; the mapper creates the evidence trail.
6. **Ensemble composition is the deeper research question.** Domain-specific expertise (LoRA models, specialized scripts) lives inside sub-ensembles that a universal extractor can delegate to. llm-orc doesn't yet support nested ensemble invocation, but the dependency graph architecture extends naturally to it. This reconciles Q2's "one ensemble" finding with the real value of domain-specific extraction: the expertise matters, but the routing is internal to llm-orc, not visible to Plexus.

---
