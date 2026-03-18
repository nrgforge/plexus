# Research Log: Architectural Consolidation

## Background

Three independent codebase audits — Claude (multi-lens structural analysis), Kimi (pedagogical stewardship framing), and MiniMax (pattern-focused) — examined ~32,500 lines of Rust across ~60 source files. The audits were conducted independently, with different analytical methods and priorities, then compared for convergence. This research log synthesizes their findings.

Tier 1 mechanical fixes are already committed: petgraph removal (ghost dependency), GraphEvent relocation to `graph/` (circular dependency fix), documentation drift corrections (annotate→ingest, FragmentAdapter→ContentAdapter), and transaction wrapping for `save_context()`.

The remaining findings are design-dependent. They require architectural decisions before implementation.

---

## Q1: What dead code exists and what are its dependencies?

### `analysis/` module — complete but disconnected

All three audits independently flagged `analysis/` as dead code. The module defines `AnalysisOrchestrator`, `ContentAnalyzer` trait, `ResultMerger`, `GraphMutation`, four built-in analyzers, and its own LLM subprocess client (`analyzers/semantic.rs`, 600+ lines). All 13 types are publicly re-exported from `lib.rs`.

Zero production callers exist. The only usage is in `tests/common/graph_builder.rs`. The module predates the adapter pipeline and was superseded without removal.

**Dependencies:** `tests/common/` imports `analysis/` types. Removing `analysis/` requires removing `tests/common/` too. Both are self-contained — no production code depends on either.

### `InputRouter` struct — superseded by `IngestPipeline`

The `InputRouter` struct at `src/adapter/router.rs:L21-68` is a complete routing abstraction with no production callers. `IngestPipeline` reimplements routing inline. The free function `classify_input` at the same module IS used (by MCP ingest), so the module stays but the struct goes.

### `TextAnalysisAdapter` — 930-line orphan

`TextAnalysisAdapter` at `src/adapter/text_analysis.rs` is publicly exported but never instantiated outside its own tests. `IngestPipeline` uses `ExtractionCoordinator` for Phase 2 extraction instead.

### Spike tests — all `#[ignore]`d

21 spike test files in `tests/spike_*.rs`, all with `#[ignore]` attributes. These were research spikes that validated hypotheses during development. Their findings are captured in essays and ADRs. The tests themselves import `tests/common/` types, creating a dependency chain: spike tests → `tests/common/` → `analysis/`.

### `update_chain` — feature ghost

ADR-014 documents an `update_chain(context_id, chain_id, changes)` method that does not exist anywhere in the codebase. Two audits flagged this.

---

## Q2: What responsibilities does EngineSink carry beyond its core job?

`EngineSink` at `src/adapter/engine_sink.rs` (~1,663 lines) accumulated five responsibilities:

1. **Sink validation** — implements `AdapterSink`, validates emissions per-item
2. **Persistence routing** — `SinkBackend` enum dispatches to test (Mutex) or production (Engine) path
3. **Enrichment orchestration** — `run_enrichment_loop` runs the quiescence loop (ADR-010)
4. **Event accumulation** — collects `GraphEvent`s for pipeline collection via `take_accumulated_events`
5. **Test/production behavioral split** — the Mutex path skips persistence and enrichment; the Engine path runs both

The enrichment responsibility is the key concern. On the Engine path, `emit()` runs the enrichment loop after every emission. On the Mutex (test) path, it doesn't. The Kimi audit called this "false confidence" — tests using `EngineSink::new()` cannot detect enrichment bugs.

However, `IngestPipeline.ingest()` already runs the enrichment loop at the pipeline level (after all adapter emissions complete). Running it per-emission inside EngineSink is redundant on the pipeline path and absent on the test path. The cleanest fix: remove enrichment from EngineSink entirely, keep it only in `IngestPipeline`.

The `run_enrichment_loop` function at `engine_sink.rs:L323-388` is already structurally independent — it takes `&PlexusEngine`, `ContextId`, and `&EnrichmentRegistry` as arguments, not `&self`. Extracting it to a module-level function is a mechanical move.

---

## Q3: What are the pipeline bypass paths and are they intentional?

Three bypass paths were identified across audits:

### `cmd_analyze` in `src/bin/plexus.rs`

Constructs `EngineSink::new(shared_ctx)` (the test path — no persistence), processes adapters directly, then calls `engine.upsert_context()` to save. Bypasses enrichment, events, and incremental persistence. Zero test coverage. All three audits flagged this.

**Assessment:** This is legacy code from before `IngestPipeline` existed. It should route through `PlexusApi` or `IngestPipeline`, but that's a separate concern from the consolidation work. For now, document it as an intentional bypass with a TODO.

### `retract_contributions` in `src/api.rs`

Imports `EngineSink::run_enrichment_loop` directly instead of routing through `IngestPipeline`. The enrichment result is discarded (`let _ = ...`).

**Assessment:** Intentional. Retraction modifies weights and needs re-normalization, which the enrichment loop handles. Routing through IngestPipeline would be awkward since retraction isn't an adapter emission. But the discarded result should at minimum be logged. Document as intentional bypass.

### `ProvenanceApi` direct writes

`update_mark` and `archive_chain` call `engine.upsert_context()` directly, producing no `GraphEvent` and triggering no enrichment.

**Assessment:** Intentional. Provenance mutations (updating a mark's annotation, archiving a chain) are metadata operations that don't affect graph topology. They shouldn't trigger enrichment. Document as intentional bypass.

---

## Synthesis

The dead code forms a clean dependency tree: spike tests → `tests/common/` → `analysis/`. All three can be removed together with no production impact.

EngineSink's enrichment responsibility should move to the pipeline level exclusively. The `run_enrichment_loop` function is already structurally independent; extracting it is mechanical. The 14 integration tests that use `with_enrichments()` on EngineSink need migration to use `IngestPipeline` or to test emission mechanics without enrichment.

The three bypass paths are all intentional (legacy CLI, retraction API, provenance metadata) and should be documented as such rather than "fixed" to route through the pipeline.

These findings motivate ADR-029 (architectural consolidation) and Essay 26 (the narrative synthesis).
