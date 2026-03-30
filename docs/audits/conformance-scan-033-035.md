# Conformance Scan Report

**Scanned against:** docs/decisions/033-lens-declaration.md, docs/decisions/034-composable-query-filters.md, docs/decisions/035-event-cursor-persistence.md
**Codebase:** /Users/nathangreen/Development/plexus
**Date:** 2026-03-26

---

## Summary

- **ADRs checked:** 3
- **Conforming:** 0
- **Violations found:** 23

All three ADRs are proposed and not yet implemented. Every violation below is new work rather than a correction of wrong behavior. The existing code is consistent with itself and with the accepted ADRs (ADR-001 through ADR-032); the debt arises entirely from the gap between the proposed decisions and the current implementation.

---

## Conformance Debt Table

### ADR-033: Lens Declaration in Declarative Adapter Spec

| # | Violation | Type | Location | Resolution | Priority |
|---|-----------|------|----------|------------|----------|
| 1 | `DeclarativeSpec` struct has no `lens` field; the YAML `lens:` section cannot be parsed | missing | `src/adapter/adapters/declarative.rs:344–352` | Add `pub lens: Option<LensSpec>` to `DeclarativeSpec` with corresponding `LensSpec` and `TranslationRule` structs | must-fix-before-build |
| 2 | No `LensSpec` / `TranslationRule` types exist anywhere in the codebase | missing | `src/adapter/adapters/declarative.rs` (entire file) | Define `LensSpec { consumer, translations: Vec<TranslationRule> }` and `TranslationRule { from: Vec<String>, to: String, min_weight: Option<f32>, involving: Option<NodePredicate> }` | must-fix-before-build |
| 3 | `DeclarativeAdapter` has no `lens()` method returning `Option<Arc<dyn Enrichment>>` | missing | `src/adapter/adapters/declarative.rs:428–577` | Add `pub fn lens(&self) -> Option<Arc<dyn Enrichment>>` following the pattern of `enrichments()` | must-fix-before-build |
| 4 | No `LensEnrichment` type exists; the `Enrichment` trait is not implemented for lens translation logic | missing | `src/adapter/enrichments/` (entire directory) | Implement `LensEnrichment` in `src/adapter/enrichments/lens.rs` that reacts to `EdgesAdded` events, applies `from`/`involving` predicates, and emits edges with `lens:{consumer}:{to}` relationship and contributions | must-fix-before-build |
| 5 | Namespace convention `lens:{consumer}:{relationship}` is unrepresented: no parsing, validation, or generation helpers exist | missing | `src/adapter/` (throughout) | Add namespace construction helper (e.g., `lens_relationship(consumer, to)`) and use it in `LensEnrichment`; no separate type needed | nice-to-have |
| 6 | `PipelineBuilder` has no `with_lens()` helper; the ADR's registration pattern (`enrichments.push(lens)`) must be done manually by callers | missing | `src/adapter/pipeline/builder.rs` | Add `pub fn with_lens(mut self, lens: Arc<dyn Enrichment>) -> Self` for ergonomics; not strictly required since callers can push manually | nice-to-have |
| 7 | `EnrichmentDeclaration` does not have a `lens` enrichment type variant; the `build_enrichments` match arm would hit `unknown` for any future lens-type string | wrong-structure | `src/adapter/adapters/declarative.rs:510–573` | This is acceptable only if lens is constructed separately via `lens()`, not via the `enrichments:` section — which is the ADR's design. No change needed here, but the error message on unknown type should remain clear | nice-to-have |

---

### ADR-034: Composable Query Filters

| # | Violation | Type | Location | Resolution | Priority |
|---|-----------|------|----------|------------|----------|
| 8 | `QueryFilter` struct does not exist anywhere in the query module | missing | `src/query/` (entire directory) | Define `pub struct QueryFilter { pub contributor_ids: Option<Vec<String>>, pub relationship_prefix: Option<String>, pub min_corroboration: Option<usize>, pub min_weight: Option<f32> }` in `src/query/types.rs` or a new `src/query/filter.rs` | must-fix-before-build |
| 9 | `TraverseQuery` has no `filter: Option<QueryFilter>` field | missing | `src/query/traverse.rs:9–20` | Add `pub filter: Option<QueryFilter>` to `TraverseQuery` and update `edge_matches()` to evaluate `QueryFilter` predicates (contributor intersection, prefix check, corroboration count) | must-fix-before-build |
| 10 | `PathQuery` has no `filter: Option<QueryFilter>` field | missing | `src/query/path.rs:9–20` | Add `pub filter: Option<QueryFilter>` to `PathQuery` and apply it during edge index construction or inline in BFS | must-fix-before-build |
| 11 | `FindQuery` has no `filter: Option<QueryFilter>` field | missing | `src/query/find.rs:8–23` | Add `pub filter: Option<QueryFilter>` to `FindQuery`; apply contributor/prefix/corroboration predicates over edges incident to matching nodes (or over edges in general, depending on query semantics for find) | must-fix-before-build |
| 12 | `StepQuery` has no `filter: Option<QueryFilter>` field | missing | `src/query/step.rs:22–25` | Add `pub filter: Option<QueryFilter>` to `StepQuery` and apply it as an additional edge predicate within each step's candidate evaluation loop (line 83–106) | must-fix-before-build |
| 13 | `RankBy` enum does not exist | missing | `src/query/` (entire directory) | Define `pub enum RankBy { RawWeight, Corroboration, NormalizedWeight(Box<dyn NormalizationStrategy>) }` in `src/query/types.rs`; add ranking post-processing step to query result collection | must-fix-before-build |
| 14 | `TraversalResult`, `PathResult`, and `QueryResult` have no `rank_by()` method or ranked-output constructor | missing | `src/query/types.rs:19–109` | Add ranking support (a `rank_by(RankBy)` method or a ranked variant of each result type) once `RankBy` is defined | must-fix-before-build |
| 15 | `edge.contributions` is a `HashMap<AdapterId, f32>` (correct data), but no query-layer helper derives corroboration count from it; callers cannot currently filter by `min_corroboration` without duplicating `contributions.len()` logic | missing | `src/graph/edge.rs:85` | Add `pub fn corroboration_count(&self) -> usize { self.contributions.len() }` to `Edge` for use by the query filter evaluation | nice-to-have |
| 16 | `PlexusApi` exposes `traverse`, `find_nodes`, and `find_path` — all will need updated call sites once their query structs gain the `filter` field; the engine dispatch methods (`engine.traverse`, `engine.find_nodes`, `engine.find_path`) delegate directly and will propagate the change | wrong-structure | `src/api.rs:136–163` | No change to `PlexusApi` signatures required (they accept the query struct by value); but `src/graph/engine.rs` methods that execute the queries must also be checked to ensure they pass the filter through | must-fix-before-build |
| 17 | `evidence_trail` in `src/query/step.rs:152–179` is a composite of two `StepQuery` branches; neither branch accepts a `QueryFilter`; the ADR requires `filter` to compose with `evidence_trail` | missing | `src/query/step.rs:152–179` | Once `StepQuery` gains `filter`, `evidence_trail` must accept and propagate an optional `QueryFilter` parameter to both branches | must-fix-before-build |

---

### ADR-035: Event Cursor Persistence

| # | Violation | Type | Location | Resolution | Priority |
|---|-----------|------|----------|------------|----------|
| 18 | No `events` table in the SQLite schema; `init_schema()` creates only `contexts`, `nodes`, and `edges` tables | missing | `src/storage/sqlite.rs:40–96` | Add `CREATE TABLE IF NOT EXISTS events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, context_id TEXT NOT NULL, event_type TEXT NOT NULL, node_ids_json TEXT, edge_ids_json TEXT, adapter_id TEXT, created_at TEXT NOT NULL)` and `CREATE INDEX IF NOT EXISTS idx_events_context_sequence ON events (context_id, sequence)` to `init_schema()` | must-fix-before-build |
| 19 | No migration path for `events` table on existing databases; the migration pattern used by `migrate_add_contributions()` etc. is the correct template but has not been applied | missing | `src/storage/sqlite.rs:247–264` | Add `migrate_add_events_table()` following the `has_column` check pattern; call it from `init_schema()` Phase 2 | must-fix-before-build |
| 20 | No `write_event()` or `persist_events()` method on `GraphStore` trait or `SqliteStore`; events produced by `emit_inner()` are not written to any table | missing | `src/storage/traits.rs:39–68` and `src/storage/sqlite.rs` | Add `fn persist_event(&self, context_id: &str, event_type: &str, node_ids: Option<&[NodeId]>, edge_ids: Option<&[EdgeId]>, adapter_id: &str) -> StorageResult<u64>` (returning the assigned sequence number) to `GraphStore` trait; implement in `SqliteStore` | must-fix-before-build |
| 21 | `EngineSink::emit_inner()` fires events into `EmitResult.events` but does not persist them to storage; the ADR requires persistence within `emit_inner()` after commit succeeds | missing | `src/adapter/sink/engine_sink.rs:103–191` | After Phase 5 event firing, call storage's `persist_event()` for each `GraphEvent`; the `Engine` backend path has access to `PlexusEngine` which wraps `GraphStore` — thread this through or add a persistence hook | must-fix-before-build |
| 22 | `PlexusApi` has no `changes_since()` method; no `ChangeSet`, `PersistedEvent`, or `CursorFilter` types exist anywhere | missing | `src/api.rs` (entire file) | Define `CursorFilter`, `PersistedEvent`, and `ChangeSet` structs; add `pub fn changes_since(&self, context_id: &str, cursor: u64, filter: Option<CursorFilter>) -> PlexusResult<ChangeSet>` to `PlexusApi` backed by a `GraphStore::query_events_since()` method | must-fix-before-build |
| 23 | `GraphStore` trait has no `query_events_since()` method; consumers cannot query the event log without it | missing | `src/storage/traits.rs:39–68` | Add `fn query_events_since(&self, context_id: &str, cursor: u64, filter: Option<&CursorFilter>) -> StorageResult<Vec<PersistedEvent>>` to `GraphStore` trait; implement in `SqliteStore` with `WHERE context_id = ? AND sequence > ?` and optional `event_type` / `adapter_id` predicates | must-fix-before-build |

---

## Notes

### Classification of work

Every violation is **new work** (feature), not a correction. The existing code correctly implements ADRs 001–032. The three proposed ADRs are purely additive — none of them require removing or restructuring code that currently exists.

### Build order constraints

The three ADRs have an implicit dependency ordering that maps to implementation order:

1. **ADR-035 (event persistence) first** — it has no dependency on ADR-033 or ADR-034, and its SQLite schema changes are foundational. `init_schema()` is the single place to add the `events` table; deferring this forces a migration later.

2. **ADR-033 (lens) second** — `LensEnrichment` is an `Enrichment` implementor; once events are persisted, lens-created edges will automatically appear in the event log under `EdgesAdded`. No query module changes are required for lens to be useful (ADR-033 Consequences §Positive bullet 3 confirms this).

3. **ADR-034 (query filters) last** — depends on ADR-033 for the `relationship_prefix: "lens:..."` use case to be meaningful; also depends on the `contributions` field being populated correctly, which is already true.

### ADR-034 interaction with existing `min_weight` fields

`TraverseQuery.min_weight` and `PathQuery`'s implicit weight floor (via relationship filtering) are not removed by ADR-034. The ADR explicitly preserves backward compatibility. When implementing `QueryFilter`, the pre-existing `min_weight` field on `TraverseQuery` should be left in place and documented as deprecated-in-favor-of-filter per the ADR's consequence: "the filter's `min_weight` takes precedence when both are present."

### `evidence_trail` and `StepQuery` exposure

`evidence_trail` (violation 17) is a free function in `src/query/step.rs`, not a method on `PlexusApi` directly (the API delegates to `query::evidence_trail`). Propagating `QueryFilter` through `evidence_trail` requires changing the function signature — this will break call sites in `src/api.rs:132`. This is a must-fix because the ADR states filters compose with all query primitives including `evidence_trail`.

### Storage trait extensibility

Adding `persist_event()` and `query_events_since()` to the `GraphStore` trait (violations 20, 23) is a breaking change to the trait — any mock or alternative implementation must also implement them. The existing `data_version()` method has a default (`Ok(0)`) as a precedent. Consider providing default no-op implementations for the event methods to ease adoption, then override in `SqliteStore`.

### No ADR conflicts found

No contradiction was found between ADR-033/034/035 and any of the 32 accepted ADRs. The proposals extend ADR-010 (enrichment loop), ADR-020/025 (declarative spec), ADR-014 (API layer), and ADR-017 (SQLite schema) without reversing any prior decision. ADR-035's note about Invariant 37 (push vs. pull paradigm) is correctly scoped.
