# Roadmap: Plexus

**Last updated:** 2026-03-17
**Derived from:** System Design v1.0, ADR-029, Essay 26, conformance audit, operationalization design

## Current Cycle: Operationalization (2026-03-17 — )

**Objective:** Make Plexus production-ready for Trellis integration. Two parallel tracks.
**Design spec:** `docs/superpowers/specs/2026-03-17-operationalization-design.md`

### Track A — Phase 2 Pipeline Design (RDD)

| Phase | Scope | Status |
|-------|-------|--------|
| Model | Module, ModuleRegistry, StructuralOutput — extend domain model | Pending |
| Decide | ADRs for dispatch design, output schema, default modules | Pending |
| Architect | Module decomposition within adapter/ | Pending |
| Build | TDD — markdown structure parser as first module | Pending |

### Track B — Operationalization

| WP | Title | Dependencies | Status |
|----|-------|-------------|--------|
| WP-B1 | .llm-orc cleanup | None | Pending |
| WP-B2 | Tier 1 acceptance tests | WP-B1 | Pending |
| WP-B3 | Research graduation | WP-B2 | Pending |
| WP-B4 | Tier 2 acceptance tests | Track A, WP-B2 | Pending |

### Dependency Graph

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

---

## Open Decision Points

- **Pipeline construction location** — Currently in `PipelineBuilder`. Consider extracting to transport-neutral binary entry point. Deferred — revisit if transport proliferation creates duplication.
- **Batched normalization / persist** — O(edges × adapters) per emission scaling concern. Deferred — requires profiling data from real Trellis workloads.
- **Context field encapsulation** — `Context.nodes` and `Context.edges` are public fields. Enable future `Vec<Edge>` → `HashMap` migration. Deferred — internal refactor, no consumer impact.
- **Release process** — Crate publishing, semver, changelogs. Deferred — downstream of operationalization.
- **Research graduation format** — How to archive RDD research corpus once it's served its purpose. Defined during WP-B3 execution.

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
