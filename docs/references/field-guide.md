# Field Guide: Plexus

**Generated:** 2026-04-13 (post MCP cycle BUILD)
**Derived from:** System Design v1.2, current implementation

## How to Use This Guide

This guide maps the system design's modules to their current implementation state. It is a reference — consult the entry for the module you're working in or exploring. For the overall architecture, read the system design. For routing to the right document, read ORIENTATION.md.

---

## Module: graph

**Implementation state:** Complete
**Code location:** `src/graph/` (7 files, ~2270 lines)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| Node | `pub struct Node` | `src/graph/node.rs:149` |
| NodeId | `pub struct NodeId(String)` | `src/graph/node.rs:36` |
| Edge | `pub struct Edge` | `src/graph/edge.rs` |
| EdgeId | `pub struct EdgeId(String)` | `src/graph/edge.rs` |
| AdapterId | `pub type AdapterId = String` | `src/graph/edge.rs` |
| Context | `pub struct Context` | `src/graph/context.rs` |
| ContextId | `pub struct ContextId(String)` | `src/graph/context.rs` |
| PlexusEngine | `pub struct PlexusEngine` | `src/graph/engine.rs` |
| GraphEvent | `pub enum GraphEvent` | `src/graph/events.rs` |
| ContentType | `pub enum ContentType` | `src/graph/node.rs` |
| Dimension | `pub mod dimension` (constants) | `src/graph/node.rs:11` |
| Source | `pub enum Source` | `src/graph/context.rs` |
| PropertyValue | `pub enum PropertyValue` | `src/graph/node.rs` |
| Scale normalization | `Context::recompute_combined_weights()` | `src/graph/context.rs` |

### Design Rationale

This module exists because all other modules need a shared graph vocabulary (Invariants 1-4, ADR-006). The engine manages an in-memory DashMap cache with optional persistence delegation — the architectural bet is that Plexus remains single-process with fast reads (ADR-006, Essay 08).

All submodules are private; everything surfaces via `mod.rs`. This is the codebase convention for all modules.

### Key Integration Points

- **storage** — `PlexusEngine` holds `Option<Arc<dyn GraphStore>>`. Persistence happens inside `with_context_mut()` (Invariant 30).
- **adapter/sink** — `EngineSink` calls `PlexusEngine::with_context_mut()` for atomic commit+persist.
- **query** — All query types take `&Context` directly. Engine wraps them for cache access.

---

## Module: adapter/sink

**Implementation state:** Complete
**Code location:** `src/adapter/sink/` (4 files: `mod.rs`, `contract.rs`, `engine_sink.rs`, `provenance.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| AdapterSink | `pub trait AdapterSink` | `src/adapter/sink/contract.rs` |
| EngineSink | `pub struct EngineSink` | `src/adapter/sink/engine_sink.rs` |
| EmitResult | `pub struct EmitResult` | `src/adapter/sink/contract.rs` |
| Rejection, RejectionReason | `pub struct Rejection`, `pub enum RejectionReason` | `src/adapter/sink/contract.rs` |
| AdapterError | `pub enum AdapterError` | `src/adapter/sink/contract.rs` |
| FrameworkContext | `pub struct FrameworkContext` | `src/adapter/sink/provenance.rs` |
| ProvenanceEntry | `pub struct ProvenanceEntry` | `src/adapter/sink/provenance.rs` |

### Design Rationale

The emission contract is separated from the pipeline and from adapters so that adapters depend on a stable trait (`AdapterSink`) rather than on the pipeline or engine directly (ADR-001). `EngineSink` has two backends — `Mutex` for tests and `Engine` for production — allowing unit tests without a full engine.

### Key Integration Points

- **graph** — `EngineSink` calls `PlexusEngine::with_context_mut()` for atomic writes.
- **adapter/pipeline** — Pipeline creates `EngineSink` per adapter dispatch.
- **adapter/adapters** — Each adapter receives `&dyn AdapterSink` in `process()`.

---

## Module: adapter/enrichment

**Implementation state:** Complete
**Code location:** `src/adapter/enrichment/` (3 files: `mod.rs`, `traits.rs`, `enrichment_loop.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| Enrichment | `pub trait Enrichment` | `src/adapter/enrichment/traits.rs` |
| EnrichmentRegistry | `pub struct EnrichmentRegistry` | `src/adapter/enrichment/traits.rs` |
| run_enrichment_loop | `pub(crate) fn run_enrichment_loop()` | `src/adapter/enrichment/enrichment_loop.rs` |
| Quiescence | `EnrichmentLoopResult { rounds, quiesced }` | `src/adapter/enrichment/enrichment_loop.rs` |
| max_rounds | `DEFAULT_MAX_ROUNDS = 10` | `src/adapter/enrichment/traits.rs` |

### Design Rationale

The enrichment contract is deliberately separate from the adapter contract (ADR-010). Enrichments are reactive — they respond to `GraphEvent`s — while adapters are imperative (called with input). The loop runs at the pipeline level after all adapter emissions complete, not per-emission (ADR-029).

### Key Integration Points

- **adapter/pipeline** — Pipeline calls `run_enrichment_loop()` after adapter dispatch.
- **adapter/enrichments** — Concrete implementations implement the `Enrichment` trait.
- **adapter/sink** — The loop uses `EngineSink` to commit enrichment emissions.

---

## Module: adapter/pipeline

**Implementation state:** Complete
**Code location:** `src/adapter/pipeline/` (4 files: `mod.rs`, `builder.rs`, `ingest.rs`, `router.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| IngestPipeline | `pub struct IngestPipeline` | `src/adapter/pipeline/ingest.rs` |
| PipelineBuilder | `pub struct PipelineBuilder` | `src/adapter/pipeline/builder.rs` |
| with_structural_module | `PipelineBuilder::with_structural_module()` | `src/adapter/pipeline/builder.rs` |
| with_default_structural_modules | `PipelineBuilder::with_default_structural_modules()` | `src/adapter/pipeline/builder.rs` |
| classify_input | `pub fn classify_input()` | `src/adapter/pipeline/router.rs` |
| ClassifyError | `pub struct ClassifyError` | `src/adapter/pipeline/router.rs` |
| Input routing (Invariant 17) | Fan-out in `IngestPipeline::ingest()` | `src/adapter/pipeline/ingest.rs` |

### Design Rationale

`IngestPipeline::ingest()` is the single write path (Invariant 34). All mutations enter here. `PipelineBuilder` was extracted from MCP to make pipeline construction transport-neutral — MCP and CLI both call `PipelineBuilder::default_pipeline()` instead of constructing pipelines inline.

`classify_input()` auto-detects input kind from JSON shape (`{text:...}` → content, `{file_path:...}` → extract-file). This powers the MCP `ingest` tool's optional `input_kind` parameter (ADR-028).

`with_structural_module()` registers a `StructuralModule` with the `ExtractionCoordinator`. `with_default_structural_modules()` registers `MarkdownStructureModule` as a built-in default. Both are called before `build()`.

### Key Integration Points

- **api** — `PlexusApi` holds `Arc<IngestPipeline>` and delegates all writes to it.
- **adapter/sink** — Pipeline creates `EngineSink` per adapter dispatch.
- **adapter/enrichment** — Pipeline calls `run_enrichment_loop()` after adapter dispatch.
- **adapter/adapters, adapter/enrichments** — Registered at construction time via `PipelineBuilder`.

---

## Module: adapter/adapters

**Implementation state:** Complete
**Code location:** `src/adapter/adapters/` (8 files, including `structural.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| ContentAdapter | `pub struct ContentAdapter` | `src/adapter/adapters/content.rs` |
| ExtractionCoordinator | `pub struct ExtractionCoordinator` | `src/adapter/adapters/extraction.rs` |
| ProvenanceAdapter | `pub struct ProvenanceAdapter` | `src/adapter/adapters/provenance_adapter.rs` |
| GraphAnalysisAdapter | `pub struct GraphAnalysisAdapter` | `src/adapter/adapters/graph_analysis.rs` |
| SemanticAdapter | `pub struct SemanticAdapter` | `src/adapter/adapters/semantic.rs` |
| DeclarativeAdapter | `pub struct DeclarativeAdapter` | `src/adapter/adapters/declarative.rs` |
| StructuralModule | `pub trait StructuralModule` | `src/adapter/adapters/structural.rs` |
| StructuralOutput | `pub struct StructuralOutput` | `src/adapter/adapters/structural.rs` |
| SectionBoundary | `pub struct SectionBoundary` | `src/adapter/adapters/structural.rs` |
| ModuleEmission | `pub struct ModuleEmission` | `src/adapter/adapters/structural.rs` |
| MarkdownStructureModule | `pub struct MarkdownStructureModule` | `src/adapter/adapters/structural.rs` |

### Design Rationale

Adapters fall into three categories: Rust-native (ContentAdapter, ExtractionCoordinator, ProvenanceAdapter, GraphAnalysisAdapter), internal llm-orc (SemanticAdapter), and external declarative (DeclarativeAdapter). See system design § Adapter Taxonomy for details. SemanticAdapter and DeclarativeAdapter are architecturally similar — both invoke llm-orc — but convergence is deferred because SemanticAdapter's bespoke multi-agent result parser handles merging that the current spec primitives don't cover.

`ExtractionCoordinator` was refactored to use a structural module registry (fan-out dispatch). On extraction, the coordinator reads the file, calls `module.analyze()` on all matching modules, merges their `StructuralOutput`, and passes merged vocabulary and sections to `SemanticInput::with_structural_context()`. Modules that don't match a file's MIME type are skipped; unregistered file types pass through unchanged (empty registry passthrough, Invariant 52).

`SemanticInput` gained a `vocabulary: Vec<String>` field and a `with_structural_context()` constructor. `SectionBoundary` is re-exported from `structural.rs` for use in semantic coordination. `MarkdownStructureModule` uses `pulldown-cmark` to extract headings (as sections) and link text + heading text (as vocabulary).

### Key Integration Points

- **adapter/sink** — Each adapter receives `&dyn AdapterSink` in `process()`.
- **adapter/pipeline** — Registered at construction time via `PipelineBuilder`. Structural modules registered via `with_structural_module()`.
- **llm_orc** — SemanticAdapter and DeclarativeAdapter use `LlmOrcClient` for subprocess calls.

---

## Module: adapter/enrichments

**Implementation state:** Complete
**Code location:** `src/adapter/enrichments/` (6 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| CoOccurrenceEnrichment | `pub struct CoOccurrenceEnrichment` | `src/adapter/enrichments/cooccurrence.rs` |
| DiscoveryGapEnrichment | `pub struct DiscoveryGapEnrichment` | `src/adapter/enrichments/discovery_gap.rs` |
| TemporalProximityEnrichment | `pub struct TemporalProximityEnrichment` | `src/adapter/enrichments/temporal_proximity.rs` |
| EmbeddingSimilarityEnrichment | `pub struct EmbeddingSimilarityEnrichment` | `src/adapter/enrichments/embedding.rs` |
| LensEnrichment | `pub struct LensEnrichment` | `src/adapter/enrichments/lens.rs` |
| Embedder | `pub trait Embedder` | `src/adapter/enrichments/embedding.rs` |
| VectorStore | `pub trait VectorStore` | `src/adapter/enrichments/embedding.rs` |
| FastEmbedEmbedder | `pub struct FastEmbedEmbedder` (behind `embeddings` flag) | `src/adapter/enrichments/embedding.rs` |

### Design Rationale

Each enrichment is a reactive algorithm implementing the `Enrichment` trait. They are domain-agnostic — they operate on graph structure, not content. `EmbeddingSimilarityEnrichment` and its backends are behind the `embeddings` feature flag (ADR-026). `LensEnrichment` (ADR-033) is a consumer-scoped enrichment that translates cross-domain edges into one consumer's vocabulary. TagConceptBridger was removed — tag bridging is domain-specific and belongs in domain code, not the core engine.

### Key Integration Points

- **adapter/enrichment** — All implementations use the `Enrichment` trait from this module.
- **adapter/pipeline** — Registered at construction time via `PipelineBuilder` or adapter spec declarations.

---

## Module: query

**Implementation state:** Complete
**Code location:** `src/query/` (9 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| FindQuery | `pub struct FindQuery` | `src/query/find.rs` |
| TraverseQuery | `pub struct TraverseQuery` | `src/query/traverse.rs` |
| PathQuery | `pub struct PathQuery` | `src/query/path.rs` |
| StepQuery | `pub struct StepQuery` | `src/query/step.rs` |
| QueryFilter | `pub struct QueryFilter` | `src/query/filter.rs` |
| RankBy | `pub enum RankBy` | `src/query/filter.rs` |
| evidence_trail | `pub fn evidence_trail()` | `src/query/step.rs` |
| NormalizationStrategy | `pub trait NormalizationStrategy` | `src/query/normalize.rs` |
| OutgoingDivisive | `pub struct OutgoingDivisive` | `src/query/normalize.rs` |
| Softmax | `pub struct Softmax` | `src/query/normalize.rs` |
| PersistedEvent | `pub struct PersistedEvent` | `src/query/cursor.rs` |
| ChangeSet | `pub struct ChangeSet` | `src/query/cursor.rs` |
| CursorFilter | `pub struct CursorFilter` | `src/query/cursor.rs` |
| shared_concepts | `pub fn shared_concepts()` | `src/query/shared.rs` |
| Direction | `pub enum Direction` | `src/query/types.rs` |

### Design Rationale

Query is strictly read-only — it never writes to the graph. All query types take `&Context` directly, not the engine. This means queries operate on in-memory snapshots with no persistence dependency. `evidence_trail()` composes two `StepQuery` branches per ADR-013.

`QueryFilter` (ADR-034) composes with all query primitives via an optional `filter` field. Fields are AND-composed; `None` fields apply no constraint. For traversal queries, the filter prunes edges during traversal (pre-filter, not post-filter). For `FindQuery`, filter uses incident-edge semantics: a node qualifies if at least one incident edge passes.

`RankBy` is applied as post-processing on `TraversalResult` via `rank_by()`, reordering nodes within depth levels without affecting traversal reachability.

Cursor types (`PersistedEvent`, `ChangeSet`, `CursorFilter`) support pull-based event delivery (ADR-035). Storage implementations persist events; query types define the query interface.

### Key Integration Points

- **graph** — All query types take `&Context` and navigate `Node`/`Edge` structures.
- **api** — `PlexusApi` wraps engine query methods for transport consumption.
- **storage** — `GraphStore` trait includes `persist_event()`, `query_events_since()`, `latest_sequence()` (default no-ops for non-SQLite backends).

---

## Module: storage

**Implementation state:** Complete
**Code location:** `src/storage/` (3-4 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| GraphStore | `pub trait GraphStore` | `src/storage/traits.rs` |
| OpenStore | `pub trait OpenStore` | `src/storage/traits.rs` |
| SqliteStore | `pub struct SqliteStore` | `src/storage/sqlite.rs` |
| SqliteVecStore | `pub struct SqliteVecStore` (behind `embeddings` flag) | `src/storage/sqlite_vec.rs` |
| StorageError | `pub enum StorageError` | `src/storage/traits.rs` |

### Design Rationale

Storage is behind a trait abstraction (`GraphStore`) so that the engine doesn't depend on SQLite directly. Contexts are serialized as JSON blobs — the storage layer is a key-value store, not a graph database. `data_version()` enables cache coherence (ADR-017).

### Key Integration Points

- **graph** — `PlexusEngine` holds `Option<Arc<dyn GraphStore>>`. Calls `save_context()` inside `with_context_mut()`.

---

## Module: provenance

**Implementation state:** Complete
**Code location:** `src/provenance/` (3 files, ~300 lines)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| ProvenanceApi | `pub struct ProvenanceApi<'a>` | `src/provenance/api.rs` |
| ChainView | `pub struct ChainView` | `src/provenance/types.rs` |
| MarkView | `pub struct MarkView` | `src/provenance/types.rs` |
| ChainStatus | `pub enum ChainStatus` | `src/provenance/types.rs` |

### Design Rationale

Provenance reads are separated from writes. `ProvenanceApi` is read-only — all provenance writes go through `ProvenanceAdapter` via the ingest pipeline. The API is constructed transiently per-request (lifetime `'a` borrows the engine), which avoids stale references.

### Key Integration Points

- **api** — `PlexusApi.prov()` constructs a `ProvenanceApi` per-call.
- **graph** — Queries filter nodes by `node_type == "chain"/"mark"` and `dimension == PROVENANCE`.

---

## Module: api

**Implementation state:** Complete
**Code location:** `src/api.rs` (single file, ~1139 lines)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| PlexusApi | `pub struct PlexusApi` | `src/api.rs` |
| ContextInfo | `pub struct ContextInfo` | `src/api.rs` |

### Design Rationale

`PlexusApi` is the transport-independent routing facade (ADR-014). It composes engine + pipeline + provenance into a single API surface. The key design split: async methods for writes (go through pipeline), sync methods for reads (fast cache hits). `PlexusApi` is `Clone` — transport surfaces hold shared instances.

### Key Integration Points

- **adapter/pipeline** — Holds `Arc<IngestPipeline>`, delegates all writes.
- **graph** — Holds `Arc<PlexusEngine>`, reads go directly to engine cache.
- **query** — Wraps engine query methods.
- **provenance** — Constructs `ProvenanceApi` per-call via `prov()` helper.
- **mcp** — MCP tool handlers call `PlexusApi` methods.

---

## Module: mcp

**Implementation state:** Complete
**Code location:** `src/mcp/` (2 files: `mod.rs`, `params.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| PlexusMcpServer | `pub struct PlexusMcpServer` | `src/mcp/mod.rs` |
| 16 MCP tools | `#[tool]` methods on `PlexusMcpServer` | `src/mcp/mod.rs` |
| run_mcp_server | `pub fn run_mcp_server()` | `src/mcp/mod.rs` |

### Design Rationale

MCP is a thin transport shell (Invariant 38). It delegates all logic to `PlexusApi`. The only non-delegation code is `set_context` (session state) and `classify_input` routing in `ingest`. Pipeline construction uses `PipelineBuilder::default_pipeline()` — extracted from inline construction to keep the transport thin.

The 16-tool surface is organized as 1 session (`set_context`) + 1 data-write (`ingest`) + 6 context management + 7 graph read (`evidence_trail`, `find_nodes`, `traverse`, `find_path`, `changes_since`, `list_tags`, `shared_concepts`) + 1 spec load (`load_spec`). `load_spec` is the only non-thin-wrapper — it routes through `PlexusApi::load_spec`, which enforces the three-effect model (ADR-037, Invariant 62) — but the MCP-layer handler itself is still 18 lines of delegation and JSON marshalling.

### Key Integration Points

- **api** — All tool handlers delegate to `PlexusApi` methods.
- **adapter** — Uses `PipelineBuilder` for pipeline construction, `classify_input` for auto-routing.

---

## Module: llm_orc

**Implementation state:** Complete
**Code location:** `src/llm_orc.rs` (single file)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| LlmOrcClient | `pub trait LlmOrcClient` | `src/llm_orc.rs` |
| SubprocessClient | `pub struct SubprocessClient` | `src/llm_orc.rs` |

### Design Rationale

External LLM orchestration runs as a subprocess (ADR-024). The trait abstraction allows SemanticAdapter tests to use mock clients without invoking real llm-orc. `SubprocessClient` invokes `llm-orc` CLI with JSON stdin/stdout.

### Key Integration Points

- **adapter/adapters** — `SemanticAdapter` and `DeclarativeAdapter` use `LlmOrcClient` for ensemble invocation.
- **bin/plexus** — CLI `analyze` command uses `SubprocessClient` directly.

---

## Quick Reference: Where Things Live

| I need to... | Go to... |
|---|---|
| Understand the graph data model | `graph/node.rs`, `graph/edge.rs`, `graph/context.rs` |
| Understand how writes work | `adapter/pipeline/ingest.rs` → `adapter/sink/engine_sink.rs` |
| Add a new adapter | Implement `Adapter` trait (`adapter/traits.rs`), register in `PipelineBuilder` |
| Add a structural module | Implement `StructuralModule` trait (`adapter/adapters/structural.rs`), register via `PipelineBuilder::with_structural_module()` |
| Add a new enrichment | Implement `Enrichment` trait (`adapter/enrichment/traits.rs`), register in `EnrichmentRegistry` |
| Query the graph | `query/` — FindQuery, TraverseQuery, PathQuery, StepQuery (all accept optional `QueryFilter`) |
| Filter queries by provenance/corroboration | `query/filter.rs` — QueryFilter, RankBy |
| Pull-based change queries | `query/cursor.rs` — PersistedEvent, ChangeSet, CursorFilter |
| Understand provenance reads | `provenance/api.rs` |
| Understand the MCP surface | `mcp/mod.rs` (9 tools) |
| Understand weight normalization | `graph/context.rs` (scale norm), `query/normalize.rs` (query-time norm) |
| Understand evidence trail | `query/step.rs` (`evidence_trail()`), ADR-013 |
| Understand contribution tracking | `graph/edge.rs` (contributions), `adapter/sink/engine_sink.rs` (emit phase 2), ADR-003 |
| Construct a pipeline | `adapter/pipeline/builder.rs` (`PipelineBuilder`) |
| Add a declarative adapter spec | Call `PlexusApi::load_spec(context, yaml)` — the only intentional delivery path (ADR-037; file-based auto-discovery removed 2026-04-14, ADR-037 §4 supersession) |
