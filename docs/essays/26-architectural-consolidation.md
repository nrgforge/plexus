# Essay 26: Architectural Consolidation

*2026-03-04*

Twenty-five essays built a knowledge graph engine from first principles. Each essay added something: reinforcement mechanics, extraction pipelines, adapter architecture, enrichment loops, provenance tracking. The additions were deliberate — each solved a real problem validated through research spikes. But deliberate additions accumulate incidental structure. Code that was the right answer in Essay 8 becomes dead weight by Essay 25.

This essay reports what three independent codebase audits found when they examined Plexus at the 32,500-line mark, and what the convergence of their findings reveals about how research-driven systems grow.

---

## What the audits found

Three audits were conducted independently: a multi-lens structural analysis (Claude), a pedagogical stewardship assessment (Kimi), and a pattern-focused review (MiniMax). They used different analytical methods, different framing, different priorities. They converged on the same structural issues.

The highest-confidence finding — flagged by all three, across four independent analytical lenses in the primary audit alone — is a thousand lines of dead code in `src/analysis/`. The module defines an `AnalysisOrchestrator`, a `ContentAnalyzer` trait, a `ResultMerger`, four built-in analyzers, and its own LLM subprocess client. All thirteen types are publicly re-exported. Zero production callers exist.

The module predates the adapter pipeline. It was the first attempt at connecting analysis capabilities to the graph. When `src/adapter/` was built (Essay 15, ADR-001), it solved the same problem with better abstractions — sink-based progressive emission, contribution tracking, enrichment loops. The analysis module was superseded. It was never removed.

This is how research-driven codebases grow. Each essay asks a question, builds machinery to answer it, and moves on. The machinery stays. When the next essay builds better machinery, the old machinery doesn't get cleaned up because it still compiles, its tests still pass (all `#[ignore]`d), and nobody depends on it. The public re-exports in `lib.rs` make it look intentional. It isn't. It's vestigial.

## The dependency tree of dead code

The dead code isn't isolated. It forms a dependency tree:

```
spike tests (21 files, all #[ignore]d)
  └── tests/common/ (test utilities: corpus, graph builder, metrics, mock LLM)
        └── analysis/ (orchestrator, analyzers, types)
```

The spike tests were research instruments — each one tested a hypothesis during a specific essay's development cycle. Their findings are captured in the essays themselves. The tests remain because removing them felt premature during active development. But development moved past them. The `tests/common/` module exists only to support these spikes, and `analysis/` types are its primary import.

Two more dead types sit in the adapter layer. `InputRouter` (68 lines) is a routing abstraction that `IngestPipeline` reimplemented inline. `TextAnalysisAdapter` (930 lines) is a complete adapter that nothing instantiates outside its own test suite — `ExtractionCoordinator` handles Phase 2 extraction instead.

The total: roughly 3,000 lines of code and 21 test files that compile, export public types, and do nothing.

## EngineSink and the accumulation of responsibility

`EngineSink` started as a simple adapter sink — a struct that validates emissions and commits them to a context. Over twenty-five essays it accumulated five responsibilities: sink validation, persistence routing, enrichment orchestration, event accumulation, and a test/production behavioral split.

The enrichment responsibility is the most problematic. When an adapter emits through the Engine backend, `EngineSink` runs the enrichment loop — the iterative quiescence process from ADR-010 that creates cross-adapter edges. When an adapter emits through the Mutex backend (the test path), enrichment doesn't run. The Kimi audit called this "false confidence": tests that use `EngineSink::new()` exercise emission mechanics but cannot detect enrichment bugs.

But this is already solved at a different layer. `IngestPipeline.ingest()` runs the enrichment loop globally, after all adapter emissions complete. The per-emission enrichment inside `EngineSink` is redundant on the pipeline path. It exists because `EngineSink` predates `IngestPipeline` — the enrichment capability was added to EngineSink before the pipeline existed to own it.

The fix is to remove enrichment from EngineSink and let the pipeline own it exclusively. The `run_enrichment_loop` function is already structurally independent: it takes an engine, a context ID, and a registry as arguments, not `&self`. Extracting it to a module-level function is a mechanical move. The 14 integration tests that call `with_enrichments()` on EngineSink need migration, but they're testing emission mechanics — they can test that without enrichment, or they can use `IngestPipeline` if they need enrichment to run.

## Pipeline bypasses

ADR-012 established `IngestPipeline.ingest()` as the single write endpoint. Three code paths bypass it:

**`cmd_analyze`** in the CLI binary constructs a bare `EngineSink::new()` (the test path) and processes adapters directly, then calls `engine.upsert_context()` to save. It bypasses enrichment, events, and incremental persistence. This is legacy code from before the pipeline existed.

**`retract_contributions`** in the API layer imports `run_enrichment_loop` directly. Retraction modifies edge weights and needs re-normalization, which the enrichment loop handles. Routing through IngestPipeline would be awkward — retraction isn't an adapter emission. The bypass is intentional.

**`ProvenanceApi`** methods (`update_mark`, `archive_chain`) call `engine.upsert_context()` directly, producing no graph events and triggering no enrichment. Provenance mutations are metadata operations that don't affect graph topology. The bypass is intentional.

The pattern: the first bypass is an accident of history. The other two are legitimate design choices that were never documented as such. The absence of documentation makes all three look like bugs when audited.

## PlexusApi and the gravity of convenience methods

`PlexusApi` was designed as a thin routing facade: resolve a context name, delegate to the pipeline or query system, return. ADR-014 specifies exactly this. But over the course of several essays it accumulated orchestration logic that doesn't belong in a routing layer.

The most visible example is `annotate()` — an 80-line method that sequences three pipeline calls (create fragment, create chain if absent, create mark), strips `#` prefixes from tags, generates UUIDs, and manages its own error type (`AnnotateError`). It was added in Essay 15 as a convenience for the MCP transport, which needed a single tool call to create an annotation. The convenience was real. But the logic belongs in the caller or in a dedicated adapter — not in the routing layer that sits between transports and the pipeline.

The problem with `annotate()` isn't that it's wrong. It works correctly and its tests pass. The problem is that it establishes a precedent: if the API layer can contain orchestration logic, every new multi-step workflow becomes a candidate for another convenience method. The facade thickens. Callers become uncertain whether to use `ingest()` or one of the specialized methods. The "single write endpoint" from ADR-012 becomes one write endpoint among several.

A subtler case: `shared_concepts()` contains inline set-intersection logic — build a `HashSet` of concept IDs from one context, filter the other context against it. Twelve lines. Correct. But this is query logic, and it sits in the API layer rather than the query module where `evidence_trail()`, `find_nodes()`, and `traverse()` live. It's not there because someone decided the API layer should own cross-context queries. It's there because it was convenient to write it next to the context-resolution logic, and there was no reason to move it.

Node construction tells the same story from the adapter side. The `concept_node()` helper in `adapter/types.rs` encapsulates the boilerplate of creating a concept node with the right content type, dimension, and deterministic ID. But chain nodes, mark nodes, and file nodes — which follow the same pattern — are constructed inline at each call site. Four lines of `Node::new_in_dimension` plus `node.id = NodeId::from(...)` repeated across `provenance_adapter.rs` and seven test sites in `semantic.rs`. Each instance is correct. The pattern is obvious. Nobody extracted it because nobody needed to touch all the call sites at once until now.

## What this teaches about research-driven development

RDD is optimized for learning speed: ask a question, build the minimum machinery to answer it, record the finding, move on. This produces correct decisions — each essay's architecture holds up under audit. But it also produces structural debt that accumulates differently from conventional technical debt.

Conventional technical debt comes from shortcuts: you know the right design but ship the fast one. Research-driven debt comes from succession: you build the right design *for that moment*, then build a better one later without removing the first. The original design was correct when built. It's not "debt" in the usual sense — it's a fossil record. The problem isn't that it was wrong, it's that it's still exported as if it were alive.

The three audits' convergence suggests a natural consolidation point. When independent reviewers with different methods flag the same issues, the codebase has accumulated enough succession debt to warrant a deliberate pruning pass. The target isn't zero dead code — some fossils are useful for understanding history — but a clear distinction between the living architecture and its vestigial predecessors.

## The consolidation

The work falls into two phases. The first addresses dead code and EngineSink responsibility. The second addresses PlexusApi surface area. ADR-029 captures the decisions for the first phase.

### Phase 1: Dead code and EngineSink

1. Remove `InputRouter` struct and `TextAnalysisAdapter` from public API
2. Delete `analysis/` module, `tests/common/`, and all spike tests
3. Extract `run_enrichment_loop` to a module-level function
4. Remove per-emission enrichment from `EngineSink`
5. Rename `take_accumulated_events` to `drain_events` (clearer semantics)
6. Document pipeline bypass paths with inline annotations

Commits 1-3 and 5 are pure structure. Commit 4 changes behavior. Commit 6 is documentation.

Outcome: roughly 3,000 fewer lines of code, 30 fewer test files, and an `EngineSink` that does one thing — validate and commit emissions.

### Phase 2: PlexusApi decomposition

1. Extract `shared_concepts` set-intersection logic to `query::shared` — `PlexusApi::shared_concepts()` becomes resolve-and-delegate
2. Extract `chain_node()`, `mark_node()`, `file_node()` construction helpers alongside existing `concept_node()` — replace inline construction in `provenance_adapter.rs` and seven test sites in `semantic.rs`
3. Convert all `annotate()` tests to call `ingest("content", FragmentInput)` and `ingest("provenance", CreateChain/AddMark)` directly
4. Delete `annotate()` and `AnnotateError` from `PlexusApi` (-120 lines)
5. Update `api.rs` module documentation and ADR-014 consequences section

Commits 1-2 are structure. Commit 3 is test migration. Commit 4 is the behavior change — but since commit 3 already removed all callers, it's mechanically safe. Commit 5 is documentation.

Outcome: `PlexusApi` is the thin routing facade ADR-014 intended. Every method resolves a context name and delegates. The public write surface is `ingest()` — the single endpoint ADR-012 prescribed — plus read-modify-write helpers (`update_mark`, `archive_chain`) that exist because the adapter trait's write-only sink can't express them. `retract_contributions()` stays: it coordinates engine retraction with enrichment re-run, which is exactly what a routing layer between two subsystems should do.

### The pattern across both phases

The structure/behavior separation holds throughout. Eleven commits total; nine are pure structure or documentation. The two behavior changes — removing enrichment from EngineSink and removing `annotate()` — are each preceded by commits that migrate their callers, making the deletions mechanically safe. This is the tidying discipline applied at scale: make the change easy, then make the easy change.
