# Field Guide: Plexus

**Generated:** 2026-07-08 (post-graduation engineering phase, v0.4.0)
**Derived from:** System Design v1.3, current implementation

## How to Use This Guide

This guide maps the system design's modules to their current implementation state. It is a reference — consult the entry for the module you're working in or exploring. For the overall architecture, read the system design. For routing to the right document, read ORIENTATION.md.

---

## Module: graph

**Implementation state:** Complete
**Code location:** `src/graph/` (7 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| Node, NodeId, ContentType, PropertyValue | `pub struct Node`, `pub struct NodeId(String)`, enums | `src/graph/node.rs` |
| Dimension | `pub mod dimension` (constants) | `src/graph/node.rs` |
| Edge, EdgeId, AdapterId | `pub struct Edge`, `pub struct EdgeId(String)`, `pub type AdapterId = String` | `src/graph/edge.rs` |
| Context, ContextId, Source | `pub struct Context`, `pub struct ContextId(String)`, `pub enum Source` | `src/graph/context.rs` |
| PlexusEngine | `pub struct PlexusEngine` | `src/graph/engine.rs` |
| GraphEvent | `pub enum GraphEvent` (6 variants: NodesAdded, EdgesAdded, NodesRemoved, EdgesRemoved, WeightsChanged, ContributionsRetracted) | `src/graph/events.rs` |
| Scale normalization | `Context::recompute_combined_weights()` | `src/graph/context.rs` |
| Cache coherence | `PlexusEngine::reload_if_changed()` — checks SQLite `data_version`, reloads cache if another connection wrote (ADR-017 §2) | `src/graph/engine.rs` |

### Design Rationale

This module exists because all other modules need a shared graph vocabulary (Invariants 1-4, ADR-006). The engine manages an in-memory DashMap cache with optional persistence delegation — the architectural bet is that Plexus remains single-process-per-consumer with fast reads (ADR-006, Essay 08). `reload_if_changed()` is what makes multiple such processes coexist against one DB file.

All submodules are private; everything surfaces via `mod.rs`. This is the codebase convention for all modules.

### Key Integration Points

- **storage** — `PlexusEngine` holds `Option<Arc<dyn GraphStore>>`. Persistence happens inside `with_context_mut()` (Invariant 30).
- **adapter/sink** — `EngineSink` calls `PlexusEngine::with_context_mut()` for atomic commit+persist.
- **query** — All query types take `&Context` directly. Engine wraps them for cache access.
- **api** — `PlexusApi::resolve` / `resolve_for_ingest` call `reload_if_changed()` before every name resolution.

---

## Module: adapter/sink

**Implementation state:** Complete
**Code location:** `src/adapter/sink/` (4 files: `mod.rs`, `contract.rs`, `engine_sink.rs`, `provenance.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| AdapterSink, EmitResult, Rejection, RejectionReason, AdapterError | trait + result types | `src/adapter/sink/contract.rs` |
| EngineSink | `pub struct EngineSink` | `src/adapter/sink/engine_sink.rs` |
| FrameworkContext, ProvenanceEntry | provenance construction types | `src/adapter/sink/provenance.rs` |

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
| EnrichmentRegistry | `pub struct EnrichmentRegistry` (max_rounds default 10) | `src/adapter/enrichment/traits.rs` |
| run_enrichment_loop | `pub(crate) fn run_enrichment_loop()` | `src/adapter/enrichment/enrichment_loop.rs` |
| Quiescence | `EnrichmentLoopResult { rounds, quiesced }` | `src/adapter/enrichment/enrichment_loop.rs` |

### Design Rationale

The enrichment contract is deliberately separate from the adapter contract (ADR-010). Enrichments are reactive — they respond to `GraphEvent`s — while adapters are imperative (called with input). The loop runs at the pipeline level after all adapter emissions complete, not per-emission (ADR-029). Background extraction phases run the same loop over their own emissions (see adapter/adapters).

### Key Integration Points

- **adapter/pipeline** — Pipeline calls `run_enrichment_loop()` after foreground adapter dispatch.
- **adapter/adapters** — `ExtractionCoordinator` calls it via `run_background_enrichment()` after each background phase commits.
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
| IngestPipeline | `pub struct IngestPipeline` (adapters + enrichments behind `RwLock`, optional `llm_client`, `synced_specs` sync memo) | `src/adapter/pipeline/ingest.rs` |
| Cross-process spec sync | `IngestPipeline::sync_spec_lenses()` — runs before every ingest | `src/adapter/pipeline/ingest.rs` |
| Runtime registration (ADR-037 §5) | `register_integration`, `deregister_adapter`, `deregister_enrichment` on `&self` via interior mutability | `src/adapter/pipeline/ingest.rs` |
| PipelineBuilder | `pub struct PipelineBuilder` (holds `engine` ref for coordinator wiring) | `src/adapter/pipeline/builder.rs` |
| with_structural_module / with_default_structural_modules | registers `StructuralModule`s on the coordinator (default: `MarkdownStructureModule`) | `src/adapter/pipeline/builder.rs` |
| with_default_enrichments | CoOccurrence, DiscoveryGap, TemporalProximity (scoped `with_node_types(["fragment"])`), EmbeddingSimilarity behind `embeddings` feature | `src/adapter/pipeline/builder.rs` |
| with_enrichment | `PipelineBuilder::with_enrichment(Arc<dyn Enrichment>)` — custom enrichment (e.g. lens) | `src/adapter/pipeline/builder.rs` |
| with_llm_client | wires `SemanticAdapter` onto coordinator AND stores client so `load_spec` can propagate it to declarative adapters with `ensemble:` fields | `src/adapter/pipeline/builder.rs` |
| with_persisted_specs | rehydrates lens enrichments at construction (ADR-037 §2) | `src/adapter/pipeline/builder.rs` |
| default_pipeline | wires default adapters + structural modules + enrichments + `SubprocessClient` llm-orc + persisted specs | `src/adapter/pipeline/builder.rs` |
| gather_persisted_specs | `pub fn gather_persisted_specs(engine)` — iterates contexts, reads specs table | `src/adapter/pipeline/builder.rs` |
| classify_input, ClassifyError | input-kind auto-detection | `src/adapter/pipeline/router.rs` |
| Input routing (Invariant 17) | fan-out in `IngestPipeline::ingest()` | `src/adapter/pipeline/ingest.rs` |

### Design Rationale

`IngestPipeline::ingest()` is the single write path (Invariant 34). All mutations enter here. `PipelineBuilder` was extracted from MCP to make pipeline construction transport-neutral — MCP and CLI both call `PipelineBuilder::default_pipeline(engine)`.

`sync_spec_lenses()` closes the cross-process gap in Invariant 62: before each ingest it reads the context's specs table, registers lenses loaded by other processes, and (reverse direction) deregisters lenses whose spec row vanished (unload_spec elsewhere). The `synced_specs` memo keyed by `(context_id, adapter_id, loaded_at)` keeps unchanged rows from being re-parsed per ingest. Lens-only — adapter wiring stays transient to the loading consumer's own process. Failures log and skip, mirroring rehydration's availability-over-strictness stance.

`classify_input()` auto-detects input kind from JSON shape (`{text:...}` → content, `{file_path:...}` → extract-file). This powers the MCP `ingest` tool's optional `input_kind` parameter (ADR-028).

`with_llm_client()` is the single method that unlocks llm-orc for both the built-in `extract-file` path (via SemanticAdapter on the coordinator) and consumer declarative specs with `ensemble:` fields (via `load_spec` attaching the client). Without it, semantic extraction is silently skipped and ensemble-declaring specs fail with `AdapterError::Skipped`.

The `ExtractionCoordinator` serves all contexts the engine owns (library mode); background tasks derive their `ContextId` from the per-ingest input, not from a builder-time binding. `build()` hands the coordinator the pipeline's live enrichment registry cell (`set_enrichment_cell`), so background phases see runtime-loaded and spec-synced lenses.

### Key Integration Points

- **api** — `PlexusApi` holds `Arc<IngestPipeline>` and delegates all writes to it.
- **adapter/sink** — Pipeline creates `EngineSink` per adapter dispatch.
- **adapter/enrichment** — Pipeline calls `run_enrichment_loop()` after adapter dispatch.
- **adapter/adapters, adapter/enrichments** — Registered at construction via `PipelineBuilder`, at runtime via `load_spec` → `register_integration`, or by another process via the specs table → `sync_spec_lenses`.
- **storage** — `sync_spec_lenses` and `gather_persisted_specs` read the specs table via the engine.

---

## Module: adapter/adapters

**Implementation state:** Complete
**Code location:** `src/adapter/adapters/` (8 files, including `structural.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| ContentAdapter | `pub struct ContentAdapter` | `src/adapter/adapters/content.rs` |
| ExtractionCoordinator | `pub struct ExtractionCoordinator` (holds `enrichment_cell`) | `src/adapter/adapters/extraction.rs` |
| Background enrichment | `fn run_background_enrichment()` — runs the enrichment loop over background-phase emissions | `src/adapter/adapters/extraction.rs` |
| ProvenanceAdapter | `pub struct ProvenanceAdapter` | `src/adapter/adapters/provenance_adapter.rs` |
| GraphAnalysisAdapter | `pub struct GraphAnalysisAdapter` | `src/adapter/adapters/graph_analysis.rs` |
| SemanticAdapter, SemanticInput | llm-orc ensemble invoker; `SemanticInput::with_structural_context()` carries sections + vocabulary | `src/adapter/adapters/semantic.rs` |
| DeclarativeAdapter, DeclarativeSpec | YAML spec interpreter | `src/adapter/adapters/declarative.rs` |
| WeightSpec | `pub enum WeightSpec { Literal(f32), Template(String) }` — untagged, so `weight: 1.0` specs parse unchanged; templates resolve per emission (ensemble-computed scores → edge weights) | `src/adapter/adapters/declarative.rs` |
| Dimension validation (ADR-042) | `validate_dimension_syntax` (non-empty, no whitespace, no `:` or `\0`), `validate_spec_dimensions` walks every primitive at spec-load time (Invariant 60 fail-fast) | `src/adapter/adapters/declarative.rs` |
| Lens declaration (ADR-033) | `LensSpec`, `TranslationRule` (with `min_weight`, `min_corroboration`, `involving`), `NodePredicate` | `src/adapter/adapters/declarative.rs` |
| StructuralModule, StructuralOutput, SectionBoundary, ModuleEmission, MarkdownStructureModule | structural analysis contract + built-in | `src/adapter/adapters/structural.rs` |

### Design Rationale

Adapters fall into three categories: Rust-native (ContentAdapter, ExtractionCoordinator, ProvenanceAdapter, GraphAnalysisAdapter), internal llm-orc (SemanticAdapter), and external declarative (DeclarativeAdapter). See system design § Adapter Taxonomy.

`ExtractionCoordinator` runs three phases: registration (synchronous), then structural analysis and semantic extraction (background). Structural modules dispatch fan-out by MIME affinity; merged `StructuralOutput` feeds `SemanticInput::with_structural_context()`. After each background phase commits, `run_background_enrichment()` runs the enrichment loop over that phase's events using the pipeline's live registry cell — so lenses (including runtime-loaded and spec-synced ones) fire on background output. Enrichment failures log without failing the phase; primary emissions are already committed.

`SemanticAdapter::build_input` includes the document `content` in the ensemble payload (capped at `MAX_CONTENT_CHARS`, truncated with a log line; omitted entirely if the file is unreadable rather than failing the phase). Without it, extraction agents would receive only metadata.

`DeclarativeAdapter` validates dimensions syntactically at spec load — the minimal reserved-char set is deliberate (ADR-042; extending it retroactively needs `spec_version` infrastructure). `log_shipped_convention_divergence` adds observational logging when specs diverge from shipped dimension conventions (WP-E; behavior unchanged). Template weights that fail to render degrade loudly to 1.0 rather than dropping the edge.

### Key Integration Points

- **adapter/sink** — Each adapter receives `&dyn AdapterSink` in `process()`; background phases construct `EngineSink` directly.
- **adapter/pipeline** — Registered via `PipelineBuilder`; the coordinator's `enrichment_cell` is set by `build()`.
- **adapter/enrichment** — `run_background_enrichment` delegates to `run_enrichment_loop`.
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
| TemporalProximityEnrichment | `pub struct TemporalProximityEnrichment`; `with_node_types()` restricts pairing to named node types and suffixes the enrichment id | `src/adapter/enrichments/temporal_proximity.rs` |
| EmbeddingSimilarityEnrichment, Embedder, VectorStore, FastEmbedEmbedder | behind `embeddings` feature flag | `src/adapter/enrichments/embedding.rs` |
| LensEnrichment | `pub struct LensEnrichment` — per-rule `min_corroboration` thresholds on merged pairs | `src/adapter/enrichments/lens.rs` |

### Design Rationale

Each enrichment is a reactive algorithm implementing the `Enrichment` trait, domain-agnostic — they operate on graph structure, not content.

`TemporalProximityEnrichment` reads a `created_at` (RFC-3339) property by convention (ADR-039); unparseable timestamps are silently skipped (graceful degradation). The default pipeline scopes it to `fragment` nodes via `with_node_types` — concept nodes also carry timestamps and would otherwise saturate output. Declarative specs can scope it themselves via the `node_types` field on the enrichment declaration.

`LensEnrichment` (ADR-033) translates cross-domain edges into one consumer's vocabulary under the `lens:{consumer}:{to}` namespace, merging many-to-one with per-source contribution keys. A rule's `min_corroboration` requires that many distinct from-relationships to evidence a node pair before the merged translation is emitted (default 1) — keeps a from-list mixing promiscuous and selective relationships from saturating output. Pairs below threshold are quiescence, not an error.

### Key Integration Points

- **adapter/enrichment** — All implementations use the `Enrichment` trait.
- **adapter/pipeline** — Registered at construction via `PipelineBuilder`, via adapter spec declarations, or cross-process via `sync_spec_lenses`.

---

## Module: query

**Implementation state:** Complete
**Code location:** `src/query/` (10 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| FindQuery | `pub struct FindQuery` | `src/query/find.rs` |
| TraverseQuery | `pub struct TraverseQuery` | `src/query/traverse.rs` |
| PathQuery | `pub struct PathQuery` | `src/query/path.rs` |
| StepQuery, evidence_trail | `pub struct StepQuery`, `pub fn evidence_trail()` | `src/query/step.rs` |
| QueryFilter, RankBy | composable filters (ADR-034) | `src/query/filter.rs` |
| NormalizationStrategy, OutgoingDivisive, Softmax | query-time weight normalization | `src/query/normalize.rs` |
| PersistedEvent, ChangeSet, CursorFilter | pull-based event delivery (ADR-035) | `src/query/cursor.rs` |
| shared_concepts | `pub fn shared_concepts()` | `src/query/shared.rs` |
| Direction | `pub enum Direction` | `src/query/types.rs` |

### Design Rationale

Query is strictly read-only. All query types take `&Context` directly, not the engine — queries operate on in-memory snapshots with no persistence dependency. `evidence_trail()` composes two `StepQuery` branches per ADR-013.

`QueryFilter` (ADR-034) composes with all query primitives via an optional `filter` field. Fields are AND-composed; `None` fields apply no constraint. Traversal queries prune edges during traversal (pre-filter); `FindQuery` uses incident-edge semantics (a node qualifies if at least one incident edge passes). `RankBy` post-processes `TraversalResult` via `rank_by()`, reordering within depth levels without affecting reachability.

### Key Integration Points

- **graph** — All query types navigate `Node`/`Edge` structures on `&Context`.
- **api** — `PlexusApi` wraps engine query methods for transport consumption.
- **storage** — `GraphStore` includes `persist_event()`, `query_events_since()`, `latest_sequence()` (default no-ops for non-SQLite backends).

---

## Module: storage

**Implementation state:** Complete
**Code location:** `src/storage/` (4 files: `mod.rs`, `traits.rs`, `sqlite.rs`, `sqlite_vec.rs`)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| GraphStore, OpenStore, StorageError | trait abstraction over persistence | `src/storage/traits.rs` |
| data_version | `GraphStore::data_version()` — backs the engine's `reload_if_changed()` coherence check (ADR-017 §2) | `src/storage/traits.rs` |
| Spec persistence | `persist_spec()`, `query_specs_for_context()`, `delete_spec()`, `pub struct PersistedSpec` — the specs table behind load/unload_spec and cross-process lens sync | `src/storage/traits.rs` |
| SqliteStore | `pub struct SqliteStore` | `src/storage/sqlite.rs` |
| SqliteVecStore | `pub struct SqliteVecStore` (behind `embeddings` flag) | `src/storage/sqlite_vec.rs` |

### Design Rationale

Storage is behind a trait abstraction so the engine doesn't depend on SQLite directly. Contexts are serialized as JSON blobs — the storage layer is a key-value store, not a graph database. Event and spec persistence have default no-op implementations so non-SQLite backends degrade gracefully.

### Key Integration Points

- **graph** — `PlexusEngine` holds `Option<Arc<dyn GraphStore>>`. Calls `save_context()` inside `with_context_mut()`; `data_version()` inside `reload_if_changed()`.
- **adapter/pipeline** — Specs table read by `sync_spec_lenses` and `gather_persisted_specs`.

---

## Module: provenance

**Implementation state:** Complete
**Code location:** `src/provenance/` (3 files)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| ProvenanceApi | `pub struct ProvenanceApi<'a>` | `src/provenance/api.rs` |
| ChainView, MarkView, ChainStatus | read-model types | `src/provenance/types.rs` |

### Design Rationale

Provenance reads are separated from writes. `ProvenanceApi` is read-only — all provenance writes go through `ProvenanceAdapter` via the ingest pipeline. The API is constructed transiently per-request (lifetime `'a` borrows the engine), which avoids stale references.

### Key Integration Points

- **api** — `PlexusApi.prov()` constructs a `ProvenanceApi` per-call.
- **graph** — Queries filter nodes by `node_type == "chain"/"mark"` and `dimension == PROVENANCE`.

---

## Module: api

**Implementation state:** Complete
**Code location:** `src/api.rs` (single file)
**Stability:** Settled

### Domain Concepts in Code

| Concept | Code Manifestation | Location |
|---------|-------------------|----------|
| PlexusApi | `pub struct PlexusApi` | `src/api.rs` |
| ContextInfo | `pub struct ContextInfo` | `src/api.rs` |
| Name resolution + coherence | `resolve()` (reads) and `resolve_for_ingest()` (writes) — both call `engine.reload_if_changed()` before resolving name → `ContextId` (ADR-017 §2) | `src/api.rs` |
| Spec lifecycle | `load_spec()` / `unload_spec()` — three-effect model, durable vocabulary (ADR-037, Invariant 62) | `src/api.rs` |

### Design Rationale

`PlexusApi` is the transport-independent routing facade (ADR-014). It composes engine + pipeline + provenance into a single API surface. Async methods for writes (go through pipeline), sync methods for reads (fast cache hits). `PlexusApi` is `Clone` — transport surfaces hold shared instances.

Every method takes a context *name*; `resolve` / `resolve_for_ingest` are the coherence choke points — the `data_version` check runs there, so multi-process consumers always resolve against a cache reflecting other connections' committed state. The two differ only in error type (`PlexusError` vs `AdapterError`).

### Key Integration Points

- **adapter/pipeline** — Holds `Arc<IngestPipeline>`, delegates all writes.
- **graph** — Holds `Arc<PlexusEngine>`; reads go directly to engine cache after `reload_if_changed()`.
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
| PlexusMcpServer | `pub struct PlexusMcpServer` (constructed via `::new(engine)`; internal `default_pipeline` wiring) | `src/mcp/mod.rs` |
| 17 MCP tools | `#[tool(...)]` methods on `PlexusMcpServer` | `src/mcp/mod.rs` |
| run_mcp_server | `pub fn run_mcp_server(db_path) -> i32` | `src/mcp/mod.rs` |

### Design Rationale

MCP is a thin transport shell (Invariant 38). It delegates all logic to `PlexusApi`. The only non-delegation code is `set_context` (session state) and `classify_input` routing in `ingest`.

The 17-tool surface:
- 1 session: `set_context`
- 1 data write: `ingest`
- 6 context management: `context_list`, `context_create`, `context_delete`, `context_rename`, `context_add_sources`, `context_remove_sources`
- 7 graph read: `evidence_trail`, `find_nodes`, `traverse`, `find_path`, `changes_since`, `list_tags`, `shared_concepts`
- 2 spec lifecycle: `load_spec`, `unload_spec`

`load_spec` and `unload_spec` route through `PlexusApi::load_spec` / `unload_spec`, which enforce the three-effect model and durable-vocabulary contracts (ADR-037, Invariant 62). `unload_spec` deregisters the adapter and lens and deletes the specs table row; vocabulary edges previously written remain queryable. The MCP handlers themselves are thin-wrapper delegation + JSON marshalling.

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

External LLM orchestration runs as a subprocess (ADR-024). The trait abstraction allows SemanticAdapter tests to use mock clients without invoking real llm-orc. `SubprocessClient` invokes the `llm-orc` CLI with JSON stdin/stdout.

### Key Integration Points

- **adapter/adapters** — `SemanticAdapter` and `DeclarativeAdapter` use `LlmOrcClient` for ensemble invocation.
- **bin/plexus** — CLI `analyze` command uses `SubprocessClient` directly.

---

## Quick Reference: Where Things Live

| I need to... | Go to... |
|---|---|
| Understand the graph data model | `graph/node.rs`, `graph/edge.rs`, `graph/context.rs` |
| Understand how writes work | `adapter/pipeline/ingest.rs` → `adapter/sink/engine_sink.rs` |
| Understand multi-process coherence | `api.rs` (`resolve`, `resolve_for_ingest`) → `graph/engine.rs` (`reload_if_changed`) → `storage/traits.rs` (`data_version`) |
| Understand cross-process spec/lens sync | `adapter/pipeline/ingest.rs` (`sync_spec_lenses`) |
| Add a new adapter | Implement `Adapter` trait (`adapter/traits.rs`), register in `PipelineBuilder` |
| Add a structural module | Implement `StructuralModule` trait (`adapter/adapters/structural.rs`), register via `PipelineBuilder::with_structural_module()` |
| Add a new enrichment | Implement `Enrichment` trait (`adapter/enrichment/traits.rs`), register via `PipelineBuilder::with_enrichment()` |
| Understand background extraction + enrichment | `adapter/adapters/extraction.rs` (`run_background_enrichment`, `enrichment_cell`) |
| Query the graph | `query/` — FindQuery, TraverseQuery, PathQuery, StepQuery (all accept optional `QueryFilter`) |
| Filter queries by provenance/corroboration | `query/filter.rs` — QueryFilter, RankBy |
| Pull-based change queries | `query/cursor.rs` — PersistedEvent, ChangeSet, CursorFilter |
| Understand provenance reads | `provenance/api.rs` |
| Understand the MCP surface | `mcp/mod.rs` (17 tools: session, ingest, context CRUD, graph reads, load_spec/unload_spec) |
| Understand weight normalization | `graph/context.rs` (scale norm), `query/normalize.rs` (query-time norm) |
| Understand evidence trail | `query/step.rs` (`evidence_trail()`), ADR-013 |
| Understand contribution tracking | `graph/edge.rs` (contributions), `adapter/sink/engine_sink.rs` (emit phase 2), ADR-003 |
| Write spec YAML (dimensions, weights, lenses) | `adapter/adapters/declarative.rs` — `WeightSpec`, `validate_dimension_syntax`, `LensSpec`/`TranslationRule` |
| Construct a pipeline | `adapter/pipeline/builder.rs` (`PipelineBuilder`) |
| Add a declarative adapter spec | Call `PlexusApi::load_spec(context, yaml)` — the only intentional delivery path (ADR-037; file-based auto-discovery removed 2026-04-14, ADR-037 §4 supersession) |
