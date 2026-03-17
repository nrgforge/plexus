# Field Guide: Plexus

**Generated:** 2026-03-16
**Derived from:** System Design v1.0, current implementation

## How to Use This Guide

This guide maps the system design's modules to their current implementation state. It is a reference — consult the entry for the module you're working in or exploring. For the overall architecture, read the system design. For routing to the right document, read ORIENTATION.md.

---

## Module: graph

**Implementation state:** Complete
**Code location:** `src/graph/` (7 files, ~2070 lines)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| Node | `pub struct Node` | `src/graph/node.rs:60` |
| NodeId | `pub struct NodeId(String)` | `src/graph/node.rs:7` |
| Edge | `pub struct Edge` | `src/graph/edge.rs` |
| EdgeId | `pub struct EdgeId(String)` | `src/graph/edge.rs` |
| AdapterId | `pub type AdapterId = String` | `src/graph/edge.rs` |
| Context | `pub struct Context` | `src/graph/context.rs` |
| ContextId | `pub struct ContextId(String)` | `src/graph/context.rs` |
| PlexusEngine | `pub struct PlexusEngine` | `src/graph/engine.rs` |
| GraphEvent | `pub(crate) enum GraphEvent` | `src/graph/events.rs` |
| ContentType | `pub enum ContentType` | `src/graph/node.rs` |
| Dimension | `pub mod dimension` (constants) | `src/graph/node.rs:12` |
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
**Code location:** `src/adapter/sink/` (3 files: `contract.rs`, `engine_sink.rs`, `provenance.rs`)
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
**Code location:** `src/adapter/enrichment/` (2 files: `traits.rs`, `enrichment_loop.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| Enrichment | `pub trait Enrichment` | `src/adapter/enrichment/traits.rs` |
| EnrichmentRegistry | `pub struct EnrichmentRegistry` | `src/adapter/enrichment/traits.rs` |
| run_enrichment_loop | `pub(crate) async fn run_enrichment_loop()` | `src/adapter/enrichment/enrichment_loop.rs` |
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
**Code location:** `src/adapter/pipeline/` (3 files: `builder.rs`, `ingest.rs`, `router.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| IngestPipeline | `pub struct IngestPipeline` | `src/adapter/pipeline/ingest.rs` |
| PipelineBuilder | `pub struct PipelineBuilder` | `src/adapter/pipeline/builder.rs` |
| classify_input | `pub fn classify_input()` | `src/adapter/pipeline/router.rs` |
| ClassifyError | `pub enum ClassifyError` | `src/adapter/pipeline/router.rs` |
| Input routing (Invariant 17) | Fan-out in `IngestPipeline::ingest()` | `src/adapter/pipeline/ingest.rs` |

### Design Rationale

`IngestPipeline::ingest()` is the single write path (Invariant 34). All mutations enter here. `PipelineBuilder` was extracted from MCP to make pipeline construction transport-neutral — MCP and CLI both call `PipelineBuilder::default_pipeline()` instead of constructing pipelines inline.

`classify_input()` auto-detects input kind from JSON shape (`{text:...}` → content, `{file_path:...}` → extract-file). This powers the MCP `ingest` tool's optional `input_kind` parameter (ADR-028).

### Key Integration Points

- **api** — `PlexusApi` holds `Arc<IngestPipeline>` and delegates all writes to it.
- **adapter/sink** — Pipeline creates `EngineSink` per adapter dispatch.
- **adapter/enrichment** — Pipeline calls `run_enrichment_loop()` after adapter dispatch.
- **adapter/adapters, adapter/enrichments** — Registered at construction time via `PipelineBuilder`.

---

## Module: adapter/adapters

**Implementation state:** Complete
**Code location:** `src/adapter/adapters/` (6 files)
**Stability:** Settled (Rust-native adapters) / In flux (SemanticAdapter convergence question)

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| ContentAdapter | `pub struct ContentAdapter` | `src/adapter/adapters/content.rs` |
| ExtractionCoordinator | `pub struct ExtractionCoordinator` | `src/adapter/adapters/extraction.rs` |
| ProvenanceAdapter | `pub struct ProvenanceAdapter` | `src/adapter/adapters/provenance_adapter.rs` |
| GraphAnalysisAdapter | `pub struct GraphAnalysisAdapter` | `src/adapter/adapters/graph_analysis.rs` |
| SemanticAdapter | `pub struct SemanticAdapter` | `src/adapter/adapters/semantic.rs` |
| DeclarativeAdapter | `pub struct DeclarativeAdapter` | `src/adapter/adapters/declarative.rs` |

### Design Rationale

Adapters fall into three categories: Rust-native (ContentAdapter, ExtractionCoordinator, ProvenanceAdapter, GraphAnalysisAdapter), internal llm-orc (SemanticAdapter), and external declarative (DeclarativeAdapter). See system design § Adapter Taxonomy for details. SemanticAdapter and DeclarativeAdapter are architecturally similar — both invoke llm-orc — but convergence is deferred because SemanticAdapter's bespoke multi-agent result parser handles merging that the current spec primitives don't cover.

### Key Integration Points

- **adapter/sink** — Each adapter receives `&dyn AdapterSink` in `process()`.
- **adapter/pipeline** — Registered at construction time via `PipelineBuilder`.
- **llm_orc** — SemanticAdapter and DeclarativeAdapter use `LlmOrcClient` for subprocess calls.

---

## Module: adapter/enrichments

**Implementation state:** Complete
**Code location:** `src/adapter/enrichments/` (4 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| CoOccurrenceEnrichment | `pub struct CoOccurrenceEnrichment` | `src/adapter/enrichments/cooccurrence.rs` |
| DiscoveryGapEnrichment | `pub struct DiscoveryGapEnrichment` | `src/adapter/enrichments/discovery_gap.rs` |
| TemporalProximityEnrichment | `pub struct TemporalProximityEnrichment` | `src/adapter/enrichments/temporal_proximity.rs` |
| EmbeddingSimilarityEnrichment | `pub struct EmbeddingSimilarityEnrichment` | `src/adapter/enrichments/embedding.rs` |
| Embedder | `pub trait Embedder` | `src/adapter/enrichments/embedding.rs` |
| VectorStore | `pub trait VectorStore` | `src/adapter/enrichments/embedding.rs` |
| FastEmbedEmbedder | `pub struct FastEmbedEmbedder` (behind `embeddings` flag) | `src/adapter/enrichments/embedding.rs` |

### Design Rationale

Each enrichment is a reactive algorithm implementing the `Enrichment` trait. They are domain-agnostic — they operate on graph structure, not content. `EmbeddingSimilarityEnrichment` and its backends are behind the `embeddings` feature flag (ADR-026). TagConceptBridger was removed — tag bridging is domain-specific and belongs in domain code, not the core engine.

### Key Integration Points

- **adapter/enrichment** — All implementations use the `Enrichment` trait from this module.
- **adapter/pipeline** — Registered at construction time via `PipelineBuilder` or adapter spec declarations.

---

## Module: query

**Implementation state:** Complete
**Code location:** `src/query/` (8 files, ~1800 lines)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| FindQuery | `pub struct FindQuery` | `src/query/find.rs` |
| TraverseQuery | `pub struct TraverseQuery` | `src/query/traverse.rs` |
| PathQuery | `pub struct PathQuery` | `src/query/path.rs` |
| StepQuery | `pub struct StepQuery` | `src/query/step.rs` |
| evidence_trail | `pub fn evidence_trail()` | `src/query/step.rs` |
| NormalizationStrategy | `pub trait NormalizationStrategy` | `src/query/normalize.rs` |
| OutgoingDivisive | `pub struct OutgoingDivisive` | `src/query/normalize.rs` |
| Softmax | `pub struct Softmax` | `src/query/normalize.rs` |
| shared_concepts | `pub fn shared_concepts()` | `src/query/shared.rs` |
| Direction | `pub enum Direction` | `src/query/types.rs` |

### Design Rationale

Query is strictly read-only — it never writes to the graph (layering rule 4). All query types take `&Context` directly, not the engine. This means queries operate on in-memory snapshots with no persistence dependency. `evidence_trail()` composes two `StepQuery` branches per ADR-013.

### Key Integration Points

- **graph** — All query types take `&Context` and navigate `Node`/`Edge` structures.
- **api** — `PlexusApi` wraps engine query methods for transport consumption.

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
**Code location:** `src/api.rs` (single file, ~1165 lines)
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
| 9 MCP tools | `#[tool]` methods on `PlexusMcpServer` | `src/mcp/mod.rs` |
| run_mcp_server | `pub fn run_mcp_server()` | `src/mcp/mod.rs` |

### Design Rationale

MCP is a thin transport shell (Invariant 38). It delegates all logic to `PlexusApi`. The only non-delegation code is `set_context` (session state) and `classify_input` routing in `ingest`. Pipeline construction uses `PipelineBuilder::default_pipeline()` — extracted from inline construction to keep the transport thin.

### Key Integration Points

- **api** — All tool handlers delegate to `PlexusApi` methods.
- **adapter** — Uses `PipelineBuilder` for pipeline construction, `classify_input` for auto-routing.

---

## Module: llm_orc

**Implementation state:** Complete
**Code location:** `src/llm_orc/` (client module)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| LlmOrcClient | `pub trait LlmOrcClient` | `src/llm_orc/` |
| SubprocessClient | `pub struct SubprocessClient` | `src/llm_orc/` |

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
| Add a new enrichment | Implement `Enrichment` trait (`adapter/enrichment/traits.rs`), register in `EnrichmentRegistry` |
| Query the graph | `query/` — FindQuery, TraverseQuery, PathQuery, StepQuery |
| Understand provenance reads | `provenance/api.rs` |
| Understand the MCP surface | `mcp/mod.rs` (9 tools) |
| Understand weight normalization | `graph/context.rs` (scale norm), `query/normalize.rs` (query-time norm) |
| Understand evidence trail | `query/step.rs` (`evidence_trail()`), ADR-013 |
| Understand contribution tracking | `graph/edge.rs` (contributions), `adapter/sink/engine_sink.rs` (emit phase 2), ADR-003 |
| Construct a pipeline | `adapter/pipeline/builder.rs` (`PipelineBuilder`) |
| Add a declarative adapter spec | Create YAML in `adapter-specs/`, see ADR-028 |
