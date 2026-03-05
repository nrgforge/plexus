# ADR-029: Architectural Consolidation

**Status:** Accepted

**Research:** [Essay 26](../essays/26-architectural-consolidation.md), [Research Log](../research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — input routing, enrichment loop, EngineSink, IngestPipeline

**Depends on:** ADR-001 (semantic adapter architecture), ADR-010 (enrichment trait and loop), ADR-012 (unified ingest pipeline)

---

## Context

Three independent codebase audits (Claude multi-lens, Kimi pedagogical stewardship, MiniMax pattern-focused) examined Plexus at ~32,500 lines across ~60 source files. Their convergent findings identify structural debt from research-driven development: code that was correct when built has been superseded by later architecture without being removed.

Tier 1 mechanical fixes are committed: petgraph ghost dependency removed, GraphEvent relocated to `graph/`, documentation drift corrected, `save_context()` wrapped in explicit transaction.

The remaining issues require architectural decisions:

1. **Dead code**: `analysis/` module (1,000+ lines, zero production callers), `InputRouter` struct (superseded by `IngestPipeline`), `TextAnalysisAdapter` (930 lines, superseded by `ExtractionCoordinator`), 21 spike test files (all `#[ignore]`d), `tests/common/` (only supports spike tests and analysis/).

2. **EngineSink scope creep**: EngineSink accumulated five responsibilities including enrichment orchestration. The enrichment loop runs per-emission on the Engine path but not on the test (Mutex) path, creating behavioral divergence. `IngestPipeline` already runs the enrichment loop at the pipeline level, making per-emission enrichment redundant.

3. **Undocumented pipeline bypasses**: Three code paths bypass `IngestPipeline.ingest()` — `cmd_analyze` (legacy CLI), `retract_contributions` (administrative API), and `ProvenanceApi` direct writes (metadata operations). Two are intentional; one is legacy.

## Decisions

### Decision 1: Delete dead code

Remove the following:

- `src/analysis/` module and all its types (`AnalysisOrchestrator`, `ContentAnalyzer`, `ResultMerger`, `GraphMutation`, four analyzers, LLM subprocess client)
- `tests/common/` module (only consumers are spike tests and analysis/ types)
- All 21 `tests/spike_*.rs` files (findings captured in essays; all `#[ignore]`d)
- `InputRouter` struct from `src/adapter/router.rs` public API (retain `classify_input` free function and `ClassifyError` — used by MCP ingest)
- `TextAnalysisAdapter` from `src/adapter/mod.rs` public API (retain the module file for its internal tests; remove re-export from `lib.rs`)

**Rationale:** All three audits independently identified these as dead code. Public re-exports of unused types create a misleading API surface. The dependency chain (spike tests → tests/common/ → analysis/) means all three layers can be removed atomically.

### Decision 2: Remove enrichment from EngineSink

Extract `run_enrichment_loop` to a module-level free function in `src/adapter/engine_sink.rs`. Remove the `with_enrichments()` builder method and `enrichments` field from `EngineSink`. The enrichment loop runs exclusively in `IngestPipeline.ingest()` (after all adapter emissions complete) and in `retract_contributions` (after weight recomputation).

**Behavior change:** `EngineSink.emit()` on the Engine path no longer runs the enrichment loop. All enrichment is pipeline-level.

**Scenarios:**
- "When an adapter emits, EngineSink commits and persists but does NOT run enrichment" — yes, after this change
- "When IngestPipeline ingests, it runs the enrichment loop AFTER adapter emission" — unchanged, already true
- "When cmd_analyze runs, it bypasses enrichment (documented, intentional)" — unchanged

**Rationale:** The Kimi audit identified the test/production behavioral divergence as highest-priority concern. Removing enrichment from EngineSink eliminates the divergence: both test and production paths commit and persist, neither runs enrichment. The pipeline owns enrichment exclusively.

### Decision 3: Rename `take_accumulated_events` to `drain_events`

The method on `EngineSink` that moves accumulated graph events out of the sink. `drain_events` is more idiomatic Rust (matches `Vec::drain`, `HashMap::drain`) and clearer about the move semantics.

### Decision 4: Eliminate pipeline bypass paths

Two of the three original bypasses have been eliminated:

1. **`cmd_analyze`** — now uses `IngestPipeline.ingest_with_adapter()` per algorithm. Dynamic adapters (one per algorithm, e.g. `graph-analysis:pagerank`) use the new `ingest_with_adapter()` method which accepts an explicit adapter instead of routing by input_kind.
2. **`ProvenanceApi` writes** — `update_mark` and `archive_chain` on `PlexusApi` now route through `ProvenanceInput::UpdateMark`/`ArchiveChain` via the ingest pipeline. The dead write methods (`create_chain`, `add_mark`, `set_chain_status`, `archive_chain`) were removed from `ProvenanceApi`, making it read-only.
3. **`retract_contributions`** — remains as an intentional bypass. Retraction is a meta-operation (removes contribution slots, prunes edges, recomputes weights), not an adapter emission.

**Rationale:** Routing through the pipeline means these operations participate in the enrichment loop. For `update_mark`, this fixes a latent bug where tag changes didn't trigger `TagConceptBridger` enrichment.

## Consequences

### Positive

- ~3,000 fewer lines of code, ~30 fewer test files
- `lib.rs` public API shrinks by 13 re-exported types
- `EngineSink` has one job: validate and commit emissions
- No behavioral divergence between test and production `EngineSink` paths
- Pipeline bypass paths reduced from 3 to 1 (only `retract_contributions` remains)

### Negative

- Spike test history is only recoverable via git (mitigated: findings are in essays)
- Integration tests calling `with_enrichments()` on EngineSink need migration (~14 call sites)
- `semantic.rs` production code calling `with_enrichments()` on EngineSink needs migration (1 call site)

### Neutral

- `TextAnalysisAdapter` module file is retained (internal tests remain valid) but not re-exported
- `router.rs` module file is retained (`classify_input` is used) but `InputRouter` struct becomes module-private
- `cmd_analyze` now uses `IngestPipeline` — no longer a legacy bypass
- `ProvenanceApi` is read-only — all writes go through `ProvenanceAdapter`
