# Operationalization Design: Plexus → Trellis Integration Readiness

**Date:** 2026-03-17
**Derived from:** Brainstorming session, roadmap.md, Essays 18/25, ADR-029
**Goal:** Make Plexus production-ready for its first consumer (Trellis) by formalizing the extraction pipeline, establishing acceptance testing, and graduating the research corpus.

---

## Context

Plexus has completed a full RDD BUILD cycle (WP-1 through WP-5). Architecture is consolidated: 364 tests passing, stable API surface, module decomposition complete. What remains is operationalization — sharpening what exists, proving it works end-to-end, and making it consumable.

This is not a new RDD cycle for the whole system. It's two parallel tracks: targeted RDD for one unsettled subsystem (Phase 2 pipeline), and execution work for everything else.

---

## Two-Track Structure

### Track A — Phase 2 Pipeline Design (RDD: model → decide → architect → build)

**Why RDD:** The Phase 2 module system — MIME-dispatched routing, registered structural analyzers, output handoff to Phase 3 — has multiple valid designs and the wrong abstraction will constrain consumer extensibility. Research is done (Essays 18, 25). Product context is established (previous cycle). Straight to domain modeling.

**Pipeline shape:**

```
Phase 1 (instant, Rust)      → file registration, frontmatter extraction
Phase 2 (fast, Rust/script)  → registered structural analyzers, MIME-dispatched
Phase 3 (slow, ensemble)     → extract-semantic with SpaCy + LLM agents
```

Phase 2 always runs. Empty registry = passthrough. Registered modules activate by MIME type. All matching modules run (fan-out, not find-first). Each module emits with its own adapter ID for contribution tracking. Structural output (SectionBoundary, extracted links, AST nodes) feeds forward into Phase 3 via SemanticInput.

**RDD phases:**

- **Model:** Extend domain model with Module, ModuleRegistry, StructuralOutput. Define how modules compose, what MIME dispatch means, what the output contract is.
- **Decide:** ADRs for dispatch design (fan-out semantics, failure isolation, ordering), StructuralOutput schema (extending SemanticInput.sections), default vs. consumer-registered modules.
- **Architect:** Module decomposition within adapter/. Trait signature for modules. Registry location (PipelineBuilder or ExtractionCoordinator).
- **Build:** TDD. First module: markdown structure parser (pulldown-cmark → heading tree, section boundaries, link extraction). Validates the module interface end-to-end.

**Outputs:** ADR-030+, updated domain model glossary, updated scenarios (likely extending `019-023-phased-extraction.md` or new scenario file), new code in adapter/, updated ExtractionCoordinator.

**Key research findings informing this design:**
- Essay 25: Phase 2 and Phase 3 work best as independent accumulators, not sequential priming. Union gives ~94% gold recall vs ~81% for Phase 3 alone.
- Essay 25: Vocabulary bootstrapping (entity names) is beneficial. Relationship priming is harmful (makes LLM validate instead of explore).
- Essay 18: Phase 2 is modality-specific — text gets different treatment than audio, code, images.
- extract-semantic.yaml already embodies this: SpaCy runs as a script agent inside the ensemble, entity-primed agents get vocabulary hints, unprimed/relationship/theme agents run independently.

### Track B — Operationalization (parallel, no RDD)

Four work packages, loosely sequential:

#### WP-B1: .llm-orc cleanup

**Objective:** Strip .llm-orc/ to production essentials, archive research artifacts.

**Changes:**
- Promote `extract-semantic.yaml` as the single canonical production ensemble
- Archive or remove `plexus-semantic.yaml` (predecessor)
- Archive `code-concepts.yaml` and `graph-analysis.yaml` unless a production use case is identified during cleanup
- Move `ensembles/research/` (3 subdirectories: Essays 23-25 spike ensembles) to archive
- Delete `artifacts/` execution history (transient llm-orc outputs, not source-controlled research)
- Audit `scripts/` beyond `extraction/` — archive any non-production scripts
- Trim `profiles/` to what `extract-semantic.yaml` actually references
- Preserve `scripts/extraction/` (spacy-extract.py, textrank-extract.py are production dependencies)

**Dependencies:** None

#### WP-B2: Tier 1 acceptance tests (supersedes WP-6)

**Objective:** Contract-based acceptance test suite for stable Plexus invariants. Reorganize existing integration_tests.rs (4500 lines) into focused test modules. All deterministic, all run with `cargo test`.

**Test modules by contract:**
- `tests/acceptance/ingest.rs` — adapter dispatch, pipeline routing, input classification
- `tests/acceptance/extraction.rs` — Phase 1 → Phase 3 with mock ensemble
- `tests/acceptance/enrichment.rs` — enrichment loop fires on correct events, quiescence
- `tests/acceptance/provenance.rs` — chain/mark lifecycle, dual obligation (Inv 7)
- `tests/acceptance/contribution.rs` — per-adapter tracking, retraction
- `tests/acceptance/persistence.rs` — save/load roundtrip preserves all state
- `tests/acceptance/degradation.rs` — graceful skip when llm-orc unavailable
- `tests/acceptance/query.rs` — find/traverse/path on known graph state

**Infrastructure:**
- Fixture documents in `tests/fixtures/` — small markdown with frontmatter, code file, plain text
- All tests use real `PlexusEngine` + `IngestPipeline` + mock llm-orc client
- Property-based assertions, not exact graph snapshots

**Relationship to existing tests:** The 4500-line `integration_tests.rs` is migrated into the new acceptance modules — it does not remain as a parallel test file. Tests that don't fit a contract category are evaluated for removal or consolidation.

**Relationship to WP-6:** WP-B2 absorbs and expands WP-6. The MCP-layer test gaps from WP-6 fold into `ingest.rs` (transport boundary → pipeline routing). WP-6's open questions are resolved: mock llm-orc for Tier 1, real Ollama for Tier 2.

**Dependencies:** WP-B1 (cleaner .llm-orc makes ensemble references simpler)

#### WP-B3: Research graduation

**Objective:** Archive the research corpus, verify operational docs are accurate, define the post-RDD documentation surface.

**Changes:**
- Run rdd-conform drift detection against operational docs (ADRs, system design, domain model, field guide, scenarios)
- Fix any drift found
- Archive `docs/essays/` and `docs/essays/research-logs/` to `docs/archive/` (or similar — exact structure TBD during execution)
- Update ORIENTATION.md to reflect post-archive structure
- Resolve open decision points in roadmap.md that this cycle addresses (Phase 2 extraction, SemanticAdapter convergence)

**Note:** The graduation format is not yet well-defined in RDD. This work package will define it by doing it. The key principle: knowledge that graduated into operational docs (ADRs, domain model, system design) stays living. Research trail (essays, logs) moves to archive.

**Dependencies:** WP-B2 (seeing the test surface clarifies what operational docs need to stay living)

#### WP-B4: Tier 2 acceptance tests

**Objective:** Gated ensemble integration tests that exercise the real extraction pipeline against a running Ollama instance.

**Changes:**
- Gated behind `PLEXUS_INTEGRATION=1` env var
- Invoke real `extract-semantic` ensemble against fixture documents
- Property-based assertions: "produces concepts", "produces relationships between concepts", "enrichments fire on extraction output"
- Validates: ensemble YAML is well-formed, SpaCy script executes, model output parses correctly through SemanticAdapter

**What these tests do NOT assert:**
- Exact node/edge counts (LLM output varies)
- Specific concept labels (model-dependent)
- Exact graph snapshots (brittle)

**Dependencies:** Track A complete (canonical pipeline finalized), WP-B2 (test infrastructure and fixtures exist)

---

## Dependency Graph

```
Track A (RDD)                    Track B (Operationalization)
─────────────                    ───────────────────────────
                                 WP-B1: .llm-orc cleanup
MODEL ──────────►                    │
DECIDE ─────────►                    ▼
ARCHITECT ──────►                WP-B2: Tier 1 acceptance tests
BUILD ──────────►                    │
       │                             ▼
       │                         WP-B3: Research graduation
       │                             │
       └──────────┬──────────────────┘
                  ▼
              WP-B4: Tier 2 acceptance tests
```

Track A and Track B (B1-B3) run in parallel. WP-B4 is the join point.

---

## Open Decision Points Resolved

| Decision Point (from roadmap.md) | Resolution |
|----------------------------------|------------|
| Phase 2 extraction | Track A: RDD cycle to design Phase 2 module system |
| SemanticAdapter / DeclarativeAdapter convergence | Deferred — both serve distinct roles (internal vs. consumer-owned). Revisit after Trellis integration reveals whether convergence is needed. |

## Open Decision Points Remaining

| Decision Point | Status |
|----------------|--------|
| Pipeline construction location | Deferred — revisit if transport proliferation creates duplication |
| Batched normalization / persist scaling | Deferred — requires profiling data from real Trellis workloads |
| Context field encapsulation | Deferred — internal refactor, no consumer impact |
| Release process (crate publishing, semver) | Deferred — downstream of operationalization |
| Research graduation format | Defined during WP-B3 execution |

---

## Success Criteria

- [ ] Phase 2 module system designed, built, and tested (Track A)
- [ ] `extract-semantic.yaml` is the sole production ensemble in .llm-orc/
- [ ] Tier 1 acceptance tests cover all 8 contract areas, all pass with `cargo test`
- [ ] Research corpus archived, operational docs verified accurate
- [ ] Tier 2 tests pass against running Ollama with fixture documents
- [ ] Roadmap updated with completed cycle and next priorities
