# Essay 18: Phased Extraction Architecture

How Plexus should support multi-phase, progressively-deeper extraction from source files — where early phases are fast and blocking, later phases are slow and background, and each phase's contribution strengthens the graph independently.

---

## The problem

A knowledge graph that only accepts pre-tagged fragments is limited to what humans or external LLMs have already annotated. To be useful across domains — creative media, movement analysis, technical documentation, research papers — Plexus needs to extract knowledge from raw files: markdown, audio, video, code, movement encodings, images.

Extraction is not one-size-fits-all. Detecting a file's MIME type takes microseconds. Parsing YAML frontmatter takes milliseconds. Analyzing section structure takes seconds. Asking an LLM to discover abstract themes takes minutes. These vary by three orders of magnitude in cost, and each produces valuable but different kinds of knowledge.

The design question: how should these phases compose within Plexus's adapter pipeline, and how much of this can be expressed declaratively — without writing Rust?

## Cheap-first is universal

Every system surveyed — Elasticsearch ingest pipelines, Apache Tika, Apple Spotlight, Unstructured.io, LlamaIndex, knowledge graph construction pipelines — orders its stages from cheapest to most expensive. The pattern is so universal it barely needs stating, but it has a non-obvious consequence for Plexus: **cheap phases should not wait for expensive phases.**

Most systems process synchronously — a document blocks until the full pipeline completes. This means the caller waits for the slowest stage. For Plexus, where Phase 4 (LLM semantic extraction) can take minutes for a long document, blocking the caller is unacceptable. The graph should be immediately useful from cheap phases, with expensive phases enriching it progressively in the background.

No surveyed system does this. Progressive async phased extraction — where the graph grows incrementally as each phase completes — is architecturally novel to Plexus. It works because of two existing properties: deterministic node IDs (concepts discovered by multiple phases converge to the same node) and per-adapter contribution tracking (each phase's evidence accumulates in its own slot without overwriting others).

## Four phases

Four phases, ordered by cost:

**Phase 1 — File info (instant, blocking).** MIME type detection, file size, extension, modification timestamp. Produces a file node in the structure dimension. Useful immediately for queries by type or path.

**Phase 2 — Metadata (fast, blocking).** Format-specific metadata extraction without deep content analysis. YAML frontmatter, ID3 tags, EXIF data, wiki categories. Produces concept nodes from declared metadata — author-stated, high confidence. The file's own claims about itself.

**Phase 3 — Heuristic (moderate, background).** Algorithmic analysis, no LLM. Section structure parsing, link extraction, term frequency, statistical properties, structural similarity. For non-text media: spectral analysis, beat detection, color histograms, AST parsing. Produces section nodes (structure dimension), link-extracted concepts, and statistical properties on existing nodes. The domain-specific analysis layer.

**Phase 4 — Semantic (slow, background, LLM).** Discovers abstract concepts not present in the text literally — themes, relationships between concepts, implied connections. For long documents: chunking, parallel extraction via fan-out, synthesis of chunk-level concepts into file-level themes. Produces concept-to-concept edges that heuristics cannot infer.

Each phase has a distinct adapter ID, so their contributions accumulate rather than overwrite. When Phase 3 discovers `concept:xdg-open` via link extraction (contribution 1.0) and Phase 4 independently discovers the same concept via semantic analysis (contribution 0.8), the edge has two contribution slots — evidence diversity across extraction methods strengthens the raw weight automatically.

## Each phase is a regular adapter emission

The existing pipeline — route to adapter, process via sink, run enrichment loop, transform events — is sufficient. No new execution machinery is needed.

An "extraction coordinator" adapter handles the `extract-file` input kind. It runs Phases 1-2 synchronously within the `ingest()` call and returns their outbound events immediately. It also spawns Phases 3-4 as `tokio` tasks that call `ingest()` again with their own input kinds when they complete. Each background phase is a separate adapter with its own ID, so its emission is independent — if Phase 4 fails, Phases 1-3 are already persisted (Invariant 30: persist-per-emission).

Enrichments fire after each phase's emission. When Phase 2 adds concept nodes, TagConceptBridger creates `references` edges to any matching marks. When Phase 3 adds more concepts, CoOccurrenceEnrichment detects new co-occurrence pairs. The graph enriches incrementally as phases complete — no special coordination, no "wait for all phases" barrier.

A status node in the graph tracks which phases have completed for each file. An MCP tool like `extraction_status(file_path)` exposes this. The status node is cheap (one per file), queryable by any client, and participates in the graph's own consistency model.

Concurrency control via `tokio::sync::Semaphore` per phase type prevents resource exhaustion. Conservative defaults: 4 concurrent heuristic tasks, 2 concurrent semantic tasks.

## Phase 4 and llm-orc

Phase 4 is the only phase that requires LLM calls. The question of where this runs — inside Plexus (Rust) or externally — has a clear answer rooted in latency arithmetic.

llm-orc is a Python-based DAG orchestrator with fan-out, script agents, model fallback chains, and artifact tracking. Its framework overhead is under 200 milliseconds. LLM API calls are 500ms to 5 seconds each. Porting the DAG execution to Rust would save less than 200ms on operations that take 2-20 seconds. The bottleneck is network I/O to the LLM, not framework overhead.

The recommended integration: **llm-orc runs as a persistent MCP service; Plexus calls it via HTTP for Phase 4 work.** Phases 1-3 stay pure Rust. The boundary between Phase 3 and Phase 4 is a natural integration seam — Phase 3 produces structural output (sections, extracted terms, statistics) that becomes Phase 4's input.

The Phase 4 adapter in Plexus is thin: serialize Phase 3 output to JSON, call llm-orc's `invoke` endpoint, deserialize the response into an Emission. The complex work — chunking, fan-out, LLM prompting, synthesis — lives in llm-orc ensemble YAML and script agents.

When llm-orc is not running, Phases 1-3 complete normally and Phase 4 is skipped. The graph is useful but not semantically enriched. This matches every surveyed system's pattern: cheap phases produce immediate value. Graceful degradation, not hard failure.

### Long documents

A Shakespeare play (6,000 lines, no frontmatter, no links) stresses Phase 4. It won't fit in a single LLM context at quality. The pattern: **chunk → fan-out → synthesize.**

Phase 3 identifies structural boundaries (acts, scenes). Phase 4 uses these boundaries for intelligent chunking rather than naive text splitting — the partition-then-chunk pattern from Unstructured.io and LlamaIndex. llm-orc's `fan_out: true` runs parallel semantic extraction per chunk. A downstream synthesis agent merges chunk-level concepts into file-level themes.

Each chunk emission persists independently. If extraction fails on chunk 7 of 12, chunks 1-6 are already in the graph. Progressive and resilient.

## Declarative adapter specs

Writing a Rust adapter for every application and file type doesn't scale. The insight from analyzing FragmentAdapter: its logic is a structured transformation from input fields to graph mutations. No arbitrary control flow, no complex algorithms — just "take this field, make a node; for each tag, make a concept; hash these fields for a deterministic ID."

A declarative YAML spec can express this:

```yaml
adapter:
  id: "sketchbin-metadata"
  input_kind: "media-metadata"

  emit:
    - create_node:
        id: { hash: ["{adapter_id}", "{input.file_path}"] }
        id_prefix: "artifact"
        type: "artifact"
        dimension: structure
        properties:
          mime_type: "{input.mime_type}"

    - for_each: "{input.tags}"
      as: tag
      emit:
        - create_node:
            id: "concept:{tag | lowercase}"
            type: "concept"
            dimension: semantic
        - create_edge:
            source: "{artifact.id}"
            target: "concept:{tag | lowercase}"
            relationship: "tagged_with"
            contribution: 1.0

    - create_provenance:
        chain_id: "chain:{adapter_id}:{input.source}"
        mark_annotation: "{input.title}"
        tags: "{input.tags}"
```

A `DeclarativeAdapter` struct implements the `Adapter` trait and interprets this spec at runtime. Input is always JSON (from any transport). The spec's `input_schema` validates it. Existing Rust adapters remain valid — the declarative path is for external consumers who can't write Rust.

Seven core primitives cover the fragment adapter and all four extraction phases:

| Primitive | Purpose |
|---|---|
| `create_node` | Node with type, dimension, content_type, properties |
| `create_edge` | Same or cross-dimensional, with relationship and weight |
| `for_each` | Iterate over array fields |
| `id_template` | Deterministic ID from string interpolation |
| `hash_id` | Deterministic ID from content hash (UUID v5) |
| `create_provenance` | Composite: chain + mark + contains edge (enforces Invariant 7) |
| `update_properties` | Add properties to existing node via upsert |

The `create_provenance` primitive is deliberate — it encapsulates the dual obligation (Invariant 7: every adapter emission produces both semantic and provenance contributions). A declarative spec that uses `create_provenance` cannot violate this invariant. Pit of success.

The template language is intentionally limited: field access, filters (`lowercase`, `sort`, `join`, `default`), and context variables (`adapter_id`, `context_id`). Complex transformations belong in script agents or Rust, not in templates.

## Two layers: extractor + mapper

Analyzing Sketchbin (creative media) and EDDI (movement analysis) reveals that the declarative spec handles the *graph mapping* layer but not the *domain extraction* layer.

Sketchbin needs `ffprobe` for audio metadata, `librosa` for spectral analysis, Whisper for transcription, vision LLMs for image understanding. EDDI needs custom movement analysis algorithms — the user's own analytical framework. These are domain-specific computations that produce structured JSON.

The architecture separates cleanly:

**Layer 1 — Extractor:** Domain-specific code that produces JSON. Script agents in llm-orc (Python/Bash), standalone processes, or Rust functions. This is where domain expertise lives.

**Layer 2 — Declarative mapper:** YAML spec that maps JSON to graph nodes and edges. Same `create_node`, `create_edge`, `for_each` primitives regardless of domain.

The mapper doesn't need to understand audio codecs or movement notation. It maps structured data to graph structure. Domain knowledge is pushed to the extraction boundary, which is already outside Plexus.

Neither Sketchbin nor EDDI needs a custom Rust adapter. Both use `DeclarativeAdapter` with domain-appropriate YAML specs. Their extractors (Python audio libraries, custom movement algorithms) produce JSON that the mapper consumes.

The adapters are fully compatible — different `input_kind` values, different adapter IDs, same pipeline. If both tag the concept "improvisation," cross-domain enrichment fires automatically.

## Three tiers of enrichment

Enrichments are reactive — they fire when new data enters the graph and update the *full graph* based on new information. This graph-wide scope is their defining property and distinguishes them from per-file extraction.

### Tier 0: Parameterized built-ins

The two existing enrichments — TagConceptBridger and CoOccurrenceEnrichment — are generic graph patterns that happen to be hardcoded.

CoOccurrenceEnrichment's algorithm is: "find target nodes that share a source via relationship X → bridge them with relationship Y." Currently hardcoded to `tagged_with` → Concept → `may_be_related`. But parameterized:

| Application | Source relationship | Target type | Output relationship |
|---|---|---|---|
| Fragment (current) | `tagged_with` | Concept | `may_be_related` |
| EDDI | `exhibits` | movement-quality | `co_exhibited` |
| Carrel | `cited_by` | Paper | `co_cited` |

EDDI's "MovementBridger" isn't a new algorithm — it's CoOccurrenceEnrichment configured with `exhibits` instead of `tagged_with`. Sketchbin doesn't need a custom enrichment at all — if its adapter uses `tagged_with` edges, the existing enrichments fire automatically.

Parameterized enrichments are declared in the adapter spec:

```yaml
enrichments:
  - type: co_occurrence
    params:
      edge_relationship: "exhibits"
      output_relationship: "co_exhibited"
```

The enrichment loop's graph-wide reactivity is preserved — the parameterized instance still runs in the loop, scanning the full context snapshot on every emission.

### Tier 1: Declarative enrichments

For bridging patterns not covered by built-in parameterizations. YAML spec with `match` (filter events), `find_nodes` (query context), `guard` (idempotency check), `emit` (create edges/nodes). Still graph-wide and reactive.

### Tier 2: Batch graph analysis

Network science algorithms — PageRank, community detection, betweenness centrality, HITS, label propagation — are a different category. They operate on global graph topology, are computationally expensive, and have a different execution cadence. Running PageRank after every fragment ingestion is wasteful.

These run as **llm-orc ensembles with script agents**, not in the per-emission enrichment loop:

```yaml
ensemble:
  name: graph-analysis
  agents:
    - name: export-graph
      type: script
      script: export_subgraph.py

    - name: pagerank
      type: script
      script: run_pagerank.py
      depends_on: [export-graph]

    - name: community-detection
      type: script
      script: run_communities.py
      depends_on: [export-graph]

    - name: apply-scores
      type: script
      script: format_for_plexus.py
      depends_on: [pagerank, community-detection]
```

PageRank and community detection run in parallel (both depend on export, not each other). Python's NetworkX handles the computation. Results are property updates on existing nodes (`pagerank_score: 0.034`, `community: 7`) applied back through the adapter pipeline.

Triggered on-demand (`plexus analyze my-context`) or by threshold ("after 50 new nodes"). Not reactive per-emission.

This reveals that **llm-orc is more than an LLM orchestrator for Plexus.** It's a general DAG executor for any expensive computation — LLM calls, network science, media analysis. The Python ecosystem access is the real value.

## Invariant tensions

Two findings create tension with existing invariants:

**Enrichment generalization (affects Invariant 35, 40).** Q1 found that CoOccurrenceEnrichment should be structure-aware, not type-aware — it should fire for any node with `tagged_with` edges, not just fragment nodes. The current implementation checks `ContentType::Concept` on targets, which is correct, but also implicitly assumes fragment sources. Generalizing to "any source node" aligns with Invariant 40 (enrichments extend graph intelligence independently of domain) but requires verifying that the enrichment still reaches quiescence with the broader trigger scope.

**Batch enrichments (extends the enrichment concept).** Network science algorithms operating outside the enrichment loop are a new execution model not covered by the current invariants. The enrichment loop (Invariant 36) assumes all enrichments are reactive and terminate via idempotency. Batch graph analysis doesn't run in the loop at all — it's scheduled externally and routes results through the adapter pipeline. This doesn't *violate* existing invariants (the batch results enter via `ingest()`, which is Invariant 34), but it extends the meaning of "enrichment" beyond reactive graph intelligence. The domain model may need a new concept — perhaps "graph analysis" distinct from "enrichment."

Neither tension requires an invariant amendment — they require clarification in the domain model during the `/rdd-model` phase.

## What this means

The phased extraction architecture lets Plexus grow from "accepts pre-tagged fragments" to "extracts knowledge from any file" without changing its core pipeline. Each phase is a regular adapter emission. The contribution tracking system handles multi-phase evidence naturally. The enrichment loop fires incrementally. The graph is useful from the moment the first phase completes.

For application developers, the declarative spec means writing a YAML file instead of Rust code. Domain-specific extraction lives in script agents (Python, Bash) that produce JSON. The spec maps JSON to graph structure. Parameterized enrichments handle cross-file knowledge integration without custom code.

For Plexus's architecture, the key decision is the Phase 3→4 boundary: Phases 1-3 are Rust-native and fast; Phase 4 delegates to llm-orc as a persistent service for LLM calls and expensive computation. This is the thinnest integration — a single HTTP call per file — and it leverages llm-orc's mature DAG execution, fan-out, and Python ecosystem without porting any of it to Rust.

The architecture is progressive at every level. Files produce immediate value from cheap phases. Background phases enrich progressively. Enrichments bridge incrementally. Network science runs on demand. Each layer adds value independently, and failure at any layer doesn't compromise the others.
