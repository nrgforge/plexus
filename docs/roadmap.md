# Roadmap: Plexus Architectural Consolidation

**Generated:** 2026-03-16
**Derived from:** System Design v1.0, ADR-029, Essay 26

## Work Packages

### WP-1: Decompose adapter/ into submodules

**Objective:** Break the 18,838-line flat `adapter/` module into the six submodules defined in the system design: sink, enrichment, types, traits, pipeline, adapters, enrichments.

**Changes:**
- Create `adapter/sink/` directory: move `sink.rs`, `engine_sink.rs`, `provenance.rs` (FrameworkContext/ProvenanceEntry)
- Create `adapter/enrichment/` directory: move `enrichment.rs`, `enrichment_loop.rs`
- Create `adapter/adapters/` directory: move `content.rs`, `extraction.rs`, `semantic.rs`, `declarative.rs`, `graph_analysis.rs`, `provenance_adapter.rs`
- Create `adapter/enrichments/` directory: move `cooccurrence.rs`, `tag_bridger.rs`, `discovery_gap.rs`, `temporal_proximity.rs`, `embedding.rs`
- Keep `adapter/types.rs`, `adapter/traits.rs`, `adapter/cancel.rs`, `adapter/router.rs` at adapter/ level (or move cancel into types, router into pipeline)
- `adapter/pipeline/` directory: move `ingest.rs`, `router.rs`
- Update `adapter/mod.rs` to re-export from submodules — public API unchanged
- Update all internal `use` paths

**Scenarios covered:** N/A (structural change, no behavior change)

**Dependencies:** None

---

### WP-2: Extract pipeline builder from MCP

**Objective:** Move pipeline construction (adapter + enrichment registration) out of `mcp/mod.rs` into a transport-neutral builder in `adapter/pipeline`, enforcing Invariant 38 (transports are thin shells).

**Changes:**
- Create `PipelineBuilder` in adapter/pipeline that encapsulates the adapter/enrichment registration logic currently at `mcp/mod.rs:66-96`
- `PipelineBuilder::default_pipeline(engine, project_dir)` registers ContentAdapter, ExtractionCoordinator, ProvenanceAdapter, and core enrichments
- **TagConceptBridger has been removed from the codebase.** *(Done — not a pending change.)* Tags are domain-specific; TagConceptBridger was removed rather than made opt-in. Default pipeline registers only domain-agnostic enrichments: CoOccurrenceEnrichment, DiscoveryGapEnrichment, TemporalProximityEnrichment, EmbeddingSimilarityEnrichment. Domains needing tag-to-concept bridging implement their own adapter.
- MCP server receives a pre-built `IngestPipeline` (or `PlexusApi`) — no longer constructs it
- CLI `cmd_analyze` either uses the builder or continues with its bare pipeline (it intentionally skips enrichments)
- Binary entry point (`bin/plexus.rs`) is the single construction site

**Scenarios covered:** ADR-028 Suite 2 (MCP ingest routing)

**Dependencies:** WP-1 (implied logic — easier after submodule decomposition, but not required)

---

### WP-3: Remove enrichment from EngineSink (ADR-029 D2)

**Objective:** EngineSink currently has two code paths (Mutex-based and Engine-based) and runs enrichments per-emission on the Engine path. This is redundant with IngestPipeline's global enrichment loop and creates behavioral divergence. Remove enrichment from EngineSink.

**Changes:**
- Remove `enrichment_registry` field and enrichment-running code from EngineSink
- EngineSink becomes purely commit+persist — no enrichment
- All enrichment runs at pipeline level (already implemented in `ingest.rs`)
- Update tests that rely on EngineSink-level enrichment to use the pipeline or test enrichment independently
- Remove the Mutex-based code path if it no longer serves a purpose (verify test usage)

**Scenarios covered:** ADR-029 Decision 2

**Dependencies:** WP-1 (implied logic — cleaner after sink is in its own submodule)

---

### WP-4: Remaining ADR-029 cleanup

**Objective:** Complete ADR-029 decisions D1 (dead code — already mostly done) and D4 (pipeline bypass elimination).

**Changes:**
- Verify all D1 dead code removals are complete (PhaseStatus removed, pulldown-cmark removed, parse_response removed — confirmed in prior commits)
- D4: Audit for remaining pipeline bypasses. `PlexusEngine::add_node()`, `add_edge()`, `apply_mutation()` are public methods that bypass the adapter pipeline. Determine if they are test-only or production-used. If production-used, either remove or document as intentional (retract_contributions is already documented as intentional bypass)
- If `ProvenanceApi` direct methods are vestigial (per domain model OQ-10), verify and remove

**Scenarios covered:** ADR-029 Decisions 1, 4

**Dependencies:** WP-3 (hard — D2 must be complete to verify D4 bypass audit is clean)

---

### WP-5: Note open questions in domain model

**Objective:** Document the Context field encapsulation open question and other architectural observations from Phase 5 discussion.

**Changes:**
- Add OQ to `docs/domain-model.md` § Open Questions: "Context field encapsulation — `Context.nodes` and `Context.edges` are public `HashMap`/`Vec` fields. Future migration to encapsulated accessors would enable `Vec<Edge>` → `HashMap` migration without breaking consumers."
- Confirm SemanticAdapter/DeclarativeAdapter distinction is documented (already done in prior commits)

**Scenarios covered:** N/A (documentation)

**Dependencies:** None (open choice)

---

## Dependency Graph

```
WP-5 ─────────────────────────── (open choice, independent)

WP-1 ◄──── WP-2 (implied logic)
  ▲
  │
  └──── WP-3 (implied logic)
           │
           └──── WP-4 (hard dependency)
```

**Classification key:**
- **Hard dependency:** WP-4 requires WP-3 — cannot verify bypass elimination until enrichment is removed from EngineSink
- **Implied logic:** WP-2 and WP-3 are simpler after WP-1's submodule decomposition, but a skilled builder could do them first
- **Open choice:** WP-5 is independent documentation work

## Transition States

### TS-1: Decomposed adapter (after WP-1)

The adapter/ module is organized into submodules matching the system design. All 373 tests pass. Public API is unchanged — `lib.rs` re-exports are identical. The codebase is structurally clearer but behaviorally identical.

### TS-2: Clean pipeline ownership (after WP-1 + WP-2)

Pipeline construction lives in one place (the builder). MCP is a pure thin shell. Adding a new transport (gRPC, REST) requires no pipeline construction knowledge — just `PipelineBuilder::default_pipeline()`. TagConceptBridger has been removed from the codebase — tag bridging is domain-specific; domains that need it implement their own adapter.

### TS-3: Fully consolidated (after WP-1 + WP-2 + WP-3 + WP-4)

ADR-029 is fully implemented. EngineSink is purely commit+persist. No pipeline bypasses remain. The architecture matches the system design exactly.

## Open Decision Points

- **WP-1 granularity:** The decomposition could go further (e.g., each adapter in its own crate). The proposed submodule split is the minimum that resolves the flat-namespace anti-pattern. Further decomposition is a future choice.
- **WP-2 builder location:** The builder could live in `adapter/pipeline/` (proposed) or in a new top-level `builder/` module. The pipeline module is the natural home since it owns IngestPipeline.
- **WP-4 mutation helpers:** `PlexusEngine::add_node()`, `add_edge()`, `apply_mutation()` may be test-only. If so, they could be `#[cfg(test)]` gated. If they're used by the CLI's analyze command, they may need to remain public with documentation noting they bypass the adapter pipeline intentionally.
- **Performance pass (deferred):** Batched normalization and persist-per-batch (instead of persist-per-emission) are performance concerns flagged in Phase 5 discussion. Not included in this roadmap — they require profiling data to justify.
