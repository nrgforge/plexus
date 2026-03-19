# Roadmap: Plexus

**Last updated:** 2026-03-18
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
| Build | TDD — implement structural module system per work packages below | Pending |

#### Track A Work Packages

**WP-A1: StructuralModule trait + StructuralOutput types**
- Create `StructuralModule` trait (async, `id`, `mime_affinity`, `analyze`)
- Create `StructuralOutput`, `ModuleEmission` types
- Add `vocabulary: Vec<String>` to `SemanticInput`, add `with_structural_context()` constructor
- Conformance debt items: 5, 7, 8, 10
- Scenarios: trait registration, MIME dispatch routing, module content passing
- Dependencies: None

**WP-A2: ExtractionCoordinator refactor**
- Replace `Phase2Registration` with module registry (`Vec<Arc<dyn StructuralModule>>`)
- Replace `find_phase2_adapter()` with `matching_modules()` (fan-out)
- Rewrite structural analysis dispatch: coordinator reads file, calls `module.analyze()`, merges outputs
- Replace `SemanticInput::for_file()` with `SemanticInput::with_structural_context()`
- Conformance debt items: 1, 2, 3, 4, 6, 9
- Scenarios: fan-out dispatch, empty registry passthrough, merge-not-select, vocabulary handoff, failure isolation
- Dependencies: WP-A1 (hard — needs trait + types)

**WP-A3: MarkdownStructureModule**
- Add `pulldown-cmark` dependency
- Implement `MarkdownStructureModule`: heading extraction → sections, link text + heading text → vocabulary
- Graph emissions: determined empirically during BUILD
- Conformance debt items: 11, 12
- Scenarios: heading extraction, link extraction, heading vocabulary, MIME affinity, no-structure graceful handling
- Dependencies: WP-A1 (hard — needs trait)

**WP-A4: PipelineBuilder wiring**
- Add `with_structural_module()` method
- Wire `MarkdownStructureModule` as default in `with_default_adapters()` or `default_pipeline()`
- Conformance debt item: 13
- Scenarios: default pipeline registers markdown module, non-markdown files pass through
- Dependencies: WP-A2 (hard — coordinator must accept modules), WP-A3 (hard — module must exist)

### Track B — Operationalization

| WP | Title | Dependencies | Status |
|----|-------|-------------|--------|
| WP-B1 | .llm-orc cleanup | None | Done (`e29c081`) |
| WP-B2 | Tier 1 acceptance tests | WP-B1 | Done (`4d82b59`, `83176ad`, `6712562`, `a012c5b`) |
| WP-B3 | Research graduation | WP-B2 | Done (`b917ae6`, `1041ef7`) |
| WP-B4 | Tier 2 acceptance tests | Track A, WP-B2 | Pending — awaits Track A completion |

### Track A Dependency Graph

```
WP-A1: Types + trait ◄──── WP-A2: Coordinator refactor (hard)
       ▲                          ▲
       │                          │
       └──── WP-A3: Markdown module (hard)
                                  │
              WP-A4: Builder wiring (hard ── needs A2 + A3)
```

**Classification:**
- WP-A2 → WP-A1: **Hard dependency** — coordinator imports the trait and types
- WP-A3 → WP-A1: **Hard dependency** — module implements the trait
- WP-A4 → WP-A2 + WP-A3: **Hard dependency** — builder wires modules into coordinator
- WP-A2 ↔ WP-A3: **Open choice** — can build in either order once WP-A1 is done

**Transition state — after WP-A1 + WP-A2:** ExtractionCoordinator has the new module registry and fan-out dispatch, but no modules registered. Empty registry passthrough works (Invariant 52) — the pipeline functions identically to today. Semantic extraction receives empty SemanticInput with no vocabulary. This is a stable intermediate state.

### Cross-Track Dependency Graph

```
Track A (RDD)                    Track B (Operationalization)
─────────────                    ───────────────────────────
                                 WP-B1: .llm-orc cleanup     ✓
WP-A1: Types ──────►                 │
WP-A2: Coordinator ►                ▼
WP-A3: Markdown ───►            WP-B2: Tier 1 acceptance     ✓
WP-A4: Builder ────►                 │
       │                             ▼
       │                         WP-B3: Research graduation   ✓
       │                             │
       └──────────┬──────────────────┘
                  ▼
              WP-B4: Tier 2 acceptance tests
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
