# Roadmap: Plexus

**Last updated:** 2026-04-01 (WP-C complete — query surface cycle done)
**Derived from:** System Design v1.1, ADR-033, ADR-034, ADR-035, Essays 001–002, conformance scan

## Current State

No active cycle. Query surface cycle completed 2026-04-01.

## Work Packages

### WP-A: Event Cursor Persistence — DONE

**Objective:** Persist graph events in SQLite with sequence numbering, enabling pull-based "changes since N" queries that preserve the library rule (Invariant 58).

**Changes:**
- storage: `events` table schema + migration in `SqliteStore::init_schema()`; `persist_event()` and `query_events_since()` methods on `GraphStore` trait (with default no-op implementations for non-SQLite backends)
- adapter/sink: `EngineSink::emit_inner()` calls `persist_event()` after commit succeeds
- query: Define `PersistedEvent`, `ChangeSet`, `CursorFilter` types
- api: `PlexusApi::changes_since()` method

**Scenarios covered:** 7 of 9 event cursor scenarios satisfied. Deferred: enrichment loop per-round events (testable with WP-B), stale cursor error (requires retention implementation).

**Dependencies:** None — foundational; no dependency on ADR-033 or ADR-034.

**What was built:**
- `src/query/cursor.rs`: `PersistedEvent`, `ChangeSet`, `CursorFilter`
- `src/storage/traits.rs`: `persist_event()`, `query_events_since()`, `latest_sequence()` on `GraphStore` (default no-ops)
- `src/storage/sqlite.rs`: `events` table + migration, full implementations
- `src/graph/engine.rs`: `persist_events()` (best-effort), `query_events_since()`, `latest_sequence()`
- `src/adapter/sink/engine_sink.rs`: event persistence after commit in Engine path
- `src/api.rs`: `PlexusApi::changes_since()`
- `tests/acceptance/cursor.rs`: 7 acceptance tests

**Decisions resolved:** GraphStore extensibility → default no-ops. Event persistence errors → best-effort (log+continue).

**Enhancement noted:** PersistedEvent carries only IDs; light metadata (relationship types, node types) would help consumers filter without fetching. Tracked in OQ-22.

---

### WP-B: Lens Declaration and Translation — DONE

**Objective:** Enable consumers to declare domain vocabulary translation rules in their adapter spec YAML, producing a `LensEnrichment` that creates translated edges at write time (Invariants 56–57).

**Changes:**
- adapter/adapters: `LensSpec`, `TranslationRule`, `NodePredicate` types in `declarative.rs`; `DeclarativeAdapter::lens()` method; YAML deserialization for `lens:` section
- adapter/enrichments: `LensEnrichment` in new `lens.rs` — implements `Enrichment` trait, namespace convention `lens:{consumer}:{to}:{from}`
- adapter/pipeline: Registration wiring — `PipelineBuilder.with_enrichment()` caller pushes lens alongside other enrichments

**What was built:**
- `src/adapter/adapters/declarative.rs`: `LensSpec`, `TranslationRule`, `NodePredicate` types; `DeclarativeSpec.lens` optional field; `DeclarativeAdapter::lens()` → `Option<Arc<dyn Enrichment>>`
- `src/adapter/enrichments/lens.rs`: `LensEnrichment` — reacts to `EdgesAdded`, many-to-one merging (per-source contribution keys), `min_weight` filtering, idempotency guard
- `src/adapter/enrichments/mod.rs`, `src/adapter/mod.rs`: module registration and re-exports
- `tests/acceptance/lens.rs`: 9 acceptance tests (7 scenarios + YAML deser + integration through real pipeline enrichment loop)

**Scenarios covered:** All 7 lens declaration scenarios (033-035-query-surface.md §Lens Declaration) satisfied.

**Decisions resolved:** Many-to-one → single edge with merged contributions, `combined_weight` = max across sources. `NodePredicate` type defined, logic deferred (no scenario exercises `involving` yet).

**Dependencies:** WP-A (implied logic) — lens-created edges automatically appear in the event log once WP-A is in place, but lens can be built without events (lens scenarios don't require cursor queries).

---

### WP-C: Composable Query Filters — DONE

**Objective:** Add optional `QueryFilter` to all query primitives for provenance-scoped filtering and corroboration ranking (Invariant 59).

**What was built:**
- `src/query/filter.rs`: `QueryFilter` (contributor_ids, relationship_prefix, min_corroboration) with `edge_passes()` method; `RankBy` enum (RawWeight, Corroboration)
- `src/query/traverse.rs`: `filter: Option<QueryFilter>` field + `with_filter()` builder; filter applied in `edge_matches()`
- `src/query/find.rs`: `filter: Option<QueryFilter>` field + `with_filter()` builder; incident-edge semantics (node qualifies if any incident edge passes)
- `src/query/path.rs`: `filter: Option<QueryFilter>` field + `with_filter()` builder; filter applied in edge index construction
- `src/query/step.rs`: `filter: Option<QueryFilter>` field + `with_filter()` builder; filter applied as additional predicate per step
- `src/query/types.rs`: `TraversalResult::rank_by()` post-processing method (reorders within depth levels)
- `src/adapter/enrichment/enrichment_loop.rs`: **Bug fix** — enrichment emissions now persist to event log (pre-existing WP-A gap)
- `tests/acceptance/filter.rs`: 12 acceptance tests (9 scenario + 3 cross-WP integration)

**Scenarios covered:** All 9 composable query filter scenarios + all 3 integration scenarios (033-035-query-surface.md).

**Decisions resolved:** FindQuery incident-edge semantics (existential: node qualifies if ≥1 incident edge passes). `evidence_trail()` signature unchanged — filter deferred (no scenario exercises it).

**Dependencies:** WP-B (implied logic) — `relationship_prefix: "lens:trellis:"` use case is meaningful only after lens edges exist, but the filter mechanism works on any edge. WP-A (open choice) — filter and cursor are independent capabilities.

---

## Dependency Graph

```
WP-A: Event Cursors
  │
  │ (implied logic — lens edges appear in cursor)
  ▼
WP-B: Lens Declaration
  │
  │ (implied logic — filter prefix meaningful with lens edges)
  ▼
WP-C: Query Filters
```

**Classification key:**
- **Hard dependency:** cannot build B without A — structural necessity
- **Implied logic:** simpler to build A first, but not required
- **Open choice:** genuinely independent — build any first

All three WPs have **implied logic** dependencies, not hard dependencies. A builder could start with any WP — WP-C works on existing edges without lens, WP-B works without event persistence, WP-A works without either. The recommended order (A → B → C) follows the conformance scan's build order and produces the most natural progression: infrastructure → domain enrichment → query composition.

## Transition States

### TS-1: Pull-Ready (after WP-A)

Event cursors are functional. Consumers can query "changes since sequence N" on any context. The graph still has no lens-translated edges and no composable filters, but pull-based workflows work for consumers who interpret raw graph events.

**Capabilities:** Full write pipeline + push events + pull cursor. No lens translation, no provenance-scoped query.

### TS-2: Lens-Translated (after WP-A + WP-B)

Consumers with a `lens:` section in their adapter spec see domain-translated edges. Cursor events include lens-created edges. The query surface still uses existing query primitives without filters — consumers scope to their lens by relationship-type string matching in their application code.

**Capabilities:** Full write pipeline + push/pull events + lens translation. No composable filters (consumers do their own edge filtering).

### TS-3: Full Query Surface (after WP-A + WP-B + WP-C)

Complete query surface. Consumers query through composable filters — provenance-scoped, corroboration-ranked, lens-prefix-scoped. The pull paradigm (cursor + filter) and push paradigm (transform_events) are both fully operational.

## Open Decision Points

- **GraphStore trait extensibility** — `persist_event()` and `query_events_since()` are breaking additions to the `GraphStore` trait. Default no-op implementations ease adoption but mean non-SQLite backends silently skip event persistence. Alternative: a separate `EventStore` trait.
- **Lens `involving` predicate complexity** — `NodePredicate` type defined (ADR-033), logic deferred. No scenario exercises `involving` yet. Expand when a consumer needs endpoint-filtered translation.
- **`evidence_trail` + QueryFilter** — Deferred. No scenario exercises filtered evidence trails. Straightforward addition when needed.
- **Cross-cutting concern at commit boundary** — Enrichment event persistence was missed because `emit_inner()` and `emit()` are separate commit paths. Consider pushing event persistence into `emit_inner()` or engine-level commit to prevent recurrence.

---

## Completed Work Log

### Cycle: Query Surface (2026-03-26 — 2026-04-01)

**Derived from:** System Design v1.1, ADR-033, ADR-034, ADR-035, Essays 001–002

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-A | Event Cursor Persistence | `7222991` | Done |
| WP-B | Lens Declaration and Translation | `8333d10` | Done |
| WP-C | Composable Query Filters | `8b3230b`, `4cb566e` | Done |

**Summary:**
- WP-A: Event persistence in SQLite, `changes_since()` API, cursor types, 7 acceptance tests
- WP-B: LensSpec/TranslationRule types, LensEnrichment (many-to-one, idempotent), YAML deserialization, 9 acceptance tests
- WP-C: QueryFilter (contributor_ids, relationship_prefix, min_corroboration), RankBy enum, filter on all query structs, 12 acceptance tests (9 scenario + 3 cross-WP integration)
- Bug fix: enrichment loop events now persist to event log (pre-existing gap)
- Final state: 403 lib tests + 58 acceptance tests (461 total)

---

### Cycle: Operationalization (2026-03-17 — 2026-03-20)

**Derived from:** ADR-029, Essay 26, operationalization design spec

**Track A — Structural Module System (RDD)**

| WP | Title | Status |
|----|-------|--------|
| WP-A1 | StructuralModule trait + StructuralOutput types | Done |
| WP-A2 | ExtractionCoordinator refactor (fan-out dispatch, module registry) | Done |
| WP-A3 | MarkdownStructureModule (pulldown-cmark, heading/link extraction) | Done |
| WP-A4 | PipelineBuilder wiring (with_structural_module, with_default_structural_modules) | Done |

**Track B — Operationalization**

| WP | Title | Commit | Status |
|----|-------|--------|--------|
| WP-B1 | .llm-orc cleanup | `e29c081` | Done |
| WP-B2 | Tier 1 acceptance tests | `4d82b59`, `83176ad`, `6712562`, `a012c5b` | Done |
| WP-B3 | Research graduation | `b917ae6`, `1041ef7` | Done |
| WP-B4 | Tier 2 acceptance tests | `bf018cf` | Done |

**Summary:**
- Track A delivered the structural module system (StructuralModule trait, ExtractionCoordinator fan-out, MarkdownStructureModule, PipelineBuilder wiring)
- Track B delivered operationalization (llm-orc cleanup, acceptance tests Tier 1+2, research graduation)
- Final state: 382 lib tests + 30 acceptance tests

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

**Summary:**
- ADR-029 fully implemented. EngineSink is purely commit+persist. PipelineBuilder owns construction. MCP is a thin shell.
- TagConceptBridger removed entirely.
- Final state: 364 lib tests, clippy clean, all conformance drift addressed.
