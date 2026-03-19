# Roadmap: Plexus

**Last updated:** 2026-03-18 (Track A complete)
**Derived from:** System Design v1.0, ADR-029, Essay 26, conformance audit, operationalization design

## Current Cycle: Operationalization (2026-03-17 — )

**Objective:** Make Plexus production-ready for Trellis integration. Two parallel tracks.
**Design spec:** `docs/superpowers/specs/2026-03-17-operationalization-design.md`

### Track A — Structural Module System (RDD)

| Phase | Scope | Status |
|-------|-------|--------|
| Model | StructuralModule, ModuleRegistry, StructuralOutput, Vocabulary bootstrap — domain model extension (Invariants 51–55) | Done |
| Decide | ADR-030 (trait), ADR-031 (output + handoff), ADR-032 (markdown module) — 22 scenarios, 13 conformance debt items | Done |
| Architect | System design amendment — extraction pipeline flow, responsibility allocation, test architecture | Done |
| Build | TDD — implement structural module system per work packages below | Done |

Track A is complete. Work packages are archived in the Completed Work Log below.

### Track B — Operationalization

| WP | Title | Dependencies | Status |
|----|-------|-------------|--------|
| WP-B1 | .llm-orc cleanup | None | Done (`e29c081`) |
| WP-B2 | Tier 1 acceptance tests | WP-B1 | Done (`4d82b59`, `83176ad`, `6712562`, `a012c5b`) |
| WP-B3 | Research graduation | WP-B2 | Done (`b917ae6`, `1041ef7`) |
| WP-B4 | Tier 2 acceptance tests | Track A, WP-B2 | Pending |

### Cross-Track Dependency Graph

```
Track A (RDD)                    Track B (Operationalization)
─────────────                    ───────────────────────────
                                 WP-B1: .llm-orc cleanup     ✓
WP-A1: Types ──────► ✓               │
WP-A2: Coordinator ► ✓              ▼
WP-A3: Markdown ───► ✓          WP-B2: Tier 1 acceptance     ✓
WP-A4: Builder ────► ✓               │
       │                             ▼
       │                         WP-B3: Research graduation   ✓
       │                             │
       └──────────┬──────────────────┘
                  ▼
              WP-B4: Tier 2 acceptance tests  ← current
```

---

## Open Decision Points

- **Pipeline construction location** — Currently in `PipelineBuilder`. Consider extracting to transport-neutral binary entry point. Deferred — revisit if transport proliferation creates duplication.
- **Batched normalization / persist** — O(edges × adapters) per emission scaling concern. Deferred — requires profiling data from real Trellis workloads.
- **Context field encapsulation** — `Context.nodes` and `Context.edges` are public fields. Enable future `Vec<Edge>` → `HashMap` migration. Deferred — internal refactor, no consumer impact.
- **Release process** — Crate publishing, semver, changelogs. Deferred — downstream of operationalization.
- **Research graduation format** — Resolved: `docs/essays/` moved to `docs/archive/essays/`. Research corpus preserved, operational docs remain at top level.

### Resolved This Cycle

- **Phase 2 extraction** — Track A: RDD cycle to design Phase 2 module system (MIME-dispatched router, registered structural analyzers, output feeds Phase 3).
- **SemanticAdapter / DeclarativeAdapter convergence** — Deferred. Both serve distinct roles (internal vs. consumer-owned). Revisit after Trellis integration reveals whether convergence is needed.
- **WP-6 scope** — Absorbed into WP-B2 (Tier 1 acceptance tests). MCP-layer test gaps from WP-6 become part of the ingest contract tests.

---

## Completed Work Log

### Track A: Structural Module System (2026-03-18)

**Derived from:** ADR-030, ADR-031, ADR-032, operationalization design spec

| WP | Title | Status |
|----|-------|--------|
| WP-A1 | StructuralModule trait + StructuralOutput types | Done |
| WP-A2 | ExtractionCoordinator refactor (fan-out dispatch, module registry) | Done |
| WP-A3 | MarkdownStructureModule (pulldown-cmark, heading/link extraction) | Done |
| WP-A4 | PipelineBuilder wiring (with_structural_module, with_default_structural_modules) | Done |

**What was built:**
- `src/adapter/adapters/structural.rs`: `StructuralModule` trait (async, `id`, `mime_affinity`, `analyze`), `StructuralOutput`, `SectionBoundary`, `ModuleEmission`, `MarkdownStructureModule`
- `src/adapter/adapters/extraction.rs`: `ExtractionCoordinator` refactored to use structural module registry — fan-out dispatch, output merge, per-module emission, `matching_modules()` replaces `find_phase2_adapter()`
- `src/adapter/adapters/semantic.rs`: `SemanticInput` gained `vocabulary: Vec<String>` field and `with_structural_context()` constructor; `SectionBoundary` re-exported from structural
- `src/adapter/pipeline/builder.rs`: `PipelineBuilder` gained `with_structural_module()` and `with_default_structural_modules()`
- New dependency: `pulldown-cmark` (Markdown parsing)

**Unblocks:** WP-B4 (Tier 2 acceptance tests)

---

### Cycle: Architectural Consolidation (2026-03-16 — 2026-03-17)

**Derived from:** ADR-029, Essay 26

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-1 | Decompose adapter/ into submodules | `b94bbc0`, `c466f33`, `ef261c8` | Done |
| WP-2 | Extract pipeline builder from MCP | `37d789e` | Done |
| WP-3 | Remove enrichment from EngineSink (ADR-029 D2) | `19214ba` | Done |
| WP-4 | Remaining ADR-029 cleanup | (prior session commits) | Done |
| WP-5 | Note open questions in domain model | (prior session commits) | Done |

**Post-build:**
- TagConceptBridger removed entirely (`feb6499`) — tag bridging is domain-specific
- Documentation drift fixed across 36+ files (`fc69209`, `d6586b7`)
- `get_links` returns `Vec<MarkView>`, empty chain name validation added (`3e69a16`)
- 2 new integration tests (`a8f5cf5`)

**Transition state achieved:** TS-3 (Fully consolidated) — ADR-029 fully implemented, EngineSink is purely commit+persist, PipelineBuilder owns construction, MCP is a thin shell.

**Final state:** 364 lib tests, clippy clean, all conformance drift addressed.

#### Original Dependency Graph

```
WP-5 ─────────────────────────── (open choice, independent)

WP-1 ◄──── WP-2 (implied logic)
  ▲
  │
  └──── WP-3 (implied logic)
           │
           └──── WP-4 (hard dependency)
```
