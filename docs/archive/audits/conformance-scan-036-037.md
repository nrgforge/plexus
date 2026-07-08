# Conformance Scan Report

**Scanned against:** ADR-033, ADR-034, ADR-035, ADR-036, ADR-037
**Codebase:** /Users/nathangreen/Development/plexus/
**Date:** 2026-04-02

---

## Summary

- **ADRs checked:** 5
- **ADRs with full conformance:** 2 (ADR-033, ADR-035)
- **ADRs with partial conformance:** 1 (ADR-034)
- **ADRs with no implementation (expected work items):** 2 (ADR-036, ADR-037)
- **Total violations found:** 14

### Violation breakdown by origin

| Category | Count | Interpretation |
|----------|-------|----------------|
| ADR-033/034/035 bugs (accepted ADRs, code deviates) | 3 | Conformance debt — bugs to fix |
| ADR-037 gaps (proposed ADR, no implementation yet) | 5 | Expected work items |
| ADR-036 gaps (proposed ADR, no implementation yet) | 6 | Expected work items |

---

## Conformance Debt Table

| # | ADR | Violation | Type | Location | Resolution |
|---|-----|-----------|------|----------|------------|
| 1 | ADR-034 | `RankBy::NormalizedWeight(Box<dyn NormalizationStrategy>)` variant prescribed in ADR but absent from code — only `RawWeight` and `Corroboration` exist | missing | `src/query/filter.rs:52-58` | Add `NormalizedWeight` variant and wire to normalization strategies in `src/query/normalize.rs`; or document the deliberate omission in the ADR |
| 2 | ADR-035 | `evidence_trail` at both `PlexusApi` and `query::evidence_trail` has no `filter: Option<QueryFilter>` parameter — ADR-036 §5 requires it (Invariant 59: provenance-scoped filtering composable with *any* query primitive, including `evidence_trail`) | missing | `src/api.rs:122-133`, `src/query/step.rs:169-197` | Add `filter: Option<QueryFilter>` to `evidence_trail()` and pipe through to the underlying `StepQuery` branches |
| 3 | ADR-037 §4 | `register_specs_from_dir` calls `register_adapter()` only — enrichments (`adapter.enrichments()`) and lens (`adapter.lens()`) from the spec are never extracted or registered. ADR-037 explicitly names this a known gap to fix | wrong-structure | `src/adapter/pipeline/ingest.rs:132` | Replace `self.register_adapter(Arc::new(adapter))` with equivalent of `register_integration`, extracting enrichments and lens before registering |
| 4 | ADR-037 §1 | `PlexusApi::load_spec` does not exist | missing | `src/api.rs` (absent) | Implement `load_spec(context_id, spec_yaml) -> Result<SpecLoadResult, SpecLoadError>` per ADR-037 §1: validate, parse, extract adapter+enrichments+lens, register on pipeline, persist to `specs` table, run initial lens pass |
| 5 | ADR-037 §2 | No `specs` table in SQLite schema — neither in base schema nor as a migration | missing | `src/storage/sqlite.rs:37-109` | Add `migrate_add_specs_table()` migration creating the `specs` table keyed by `(context_id, adapter_id)` per ADR-037 §2 schema |
| 6 | ADR-037 §2 | On startup, `PlexusEngine::load_all()` does not re-instantiate persisted specs from the `specs` table | missing | `src/graph/` (startup path, no specs table to query) | After `specs` table exists, query it during `load_all()` and re-register adapters, enrichments, and lens for each persisted spec |
| 7 | ADR-037 §5 | Runtime registration after construction — `IngestPipeline.register_adapter` and `register_integration` take `&mut self`, making post-construction registration on a pipeline behind `Arc` in `PlexusApi` impossible without interior mutability | missing | `src/adapter/pipeline/ingest.rs:43-65` | Add interior mutability to adapter vector and enrichment registry (e.g., `RwLock<Vec<...>>`) to support `load_spec` calling registration at runtime |
| 8 | ADR-037 §6 | `PlexusApi::unload_spec` does not exist | missing | `src/api.rs` (absent) | Implement `unload_spec(context_id, adapter_id) -> Result<(), SpecUnloadError>`: remove adapter from routing, delete row from `specs` table, leave vocabulary edges intact |
| 9 | ADR-036 §1 | MCP tool `find_nodes` is absent — `PlexusApi::find_nodes` exists but is not exposed as an MCP tool | missing | `src/mcp/mod.rs` (absent) | Add `#[tool]` handler for `find_nodes` with flat optional fields: `node_type`, `dimension`, `contributor_ids`, `relationship_prefix`, `min_corroboration` |
| 10 | ADR-036 §1 | MCP tool `traverse` is absent — `PlexusApi::traverse` exists but is not exposed | missing | `src/mcp/mod.rs` (absent) | Add `#[tool]` handler for `traverse` with flat optional fields including `rank_by` and `direction` per ADR-036 §2 |
| 11 | ADR-036 §1 | MCP tool `find_path` is absent — `PlexusApi::find_path` exists but is not exposed | missing | `src/mcp/mod.rs` (absent) | Add `#[tool]` handler for `find_path` with flat optional fields including `direction` |
| 12 | ADR-036 §1 | MCP tool `changes_since` is absent — `PlexusApi::changes_since` exists but is not exposed | missing | `src/mcp/mod.rs` (absent) | Add `#[tool]` handler for `changes_since` with flat `CursorFilter` fields per ADR-036 §2 |
| 13 | ADR-036 §1 | MCP tool `list_tags` is absent — `PlexusApi::list_tags` exists but is not exposed | missing | `src/mcp/mod.rs` (absent) | Add `#[tool]` handler for `list_tags` (no parameters beyond active context) |
| 14 | ADR-036 §1 | MCP tool `shared_concepts` is absent — `PlexusApi::shared_concepts` exists but is not exposed | missing | `src/mcp/mod.rs` (absent) | Add `#[tool]` handler for `shared_concepts` with `context_a` and `context_b` string parameters |

---

## Detailed Findings by ADR

### ADR-033: Lens Declaration — CONFORMING

All prescribed structures are present and correctly wired:

- `LensSpec`, `TranslationRule`, `NodePredicate` types are defined at `src/adapter/adapters/declarative.rs:352-384`
- `DeclarativeSpec.lens: Option<LensSpec>` is present at line 395
- `DeclarativeAdapter::lens() -> Option<Arc<dyn Enrichment>>` is implemented at line 533
- `LensEnrichment` is implemented in `src/adapter/enrichments/lens.rs`
- Namespace convention (`lens:{consumer}:{to}`, contribution keys `lens:{consumer}:{to}:{from}`) is followed per the enrichment implementation
- 9 acceptance tests exist in `tests/acceptance/lens.rs` per memory

No violations.

---

### ADR-034: Composable Query Filters — PARTIAL CONFORMANCE

Core structures are present and correctly wired:

- `QueryFilter` struct with `contributor_ids`, `relationship_prefix`, `min_corroboration` — correct at `src/query/filter.rs:13-20`
- `filter: Option<QueryFilter>` added to `TraverseQuery`, `FindQuery`, `PathQuery`, `StepQuery` — confirmed
- `RankBy` enum present at `src/query/filter.rs:52-58` with `RawWeight` and `Corroboration` variants
- Pre-filter semantics during traversal confirmed in `src/query/step.rs:98-103`

One violation:

**Violation 1 — `RankBy::NormalizedWeight` absent.** ADR-034 prescribes:

```rust
pub enum RankBy {
    RawWeight,
    Corroboration,
    NormalizedWeight(Box<dyn NormalizationStrategy>),
}
```

The code at `src/query/filter.rs:52-58` has only `RawWeight` and `Corroboration`. `NormalizedWeight` is absent. The normalization strategies (`OutgoingDivisive`, `Softmax`) exist in `src/query/normalize.rs` but are not connected to `RankBy`. This may be a deliberate deferral — the ADR places `RankBy` on result types as a post-processing step and the implemented variants are the two most commonly needed — but it is a structural gap against the ADR text.

---

### ADR-035: Event Cursor Persistence — CONFORMING

All prescribed structures are present and correctly wired:

- `events` table created via `migrate_add_events_table()` at `src/storage/sqlite.rs:272-302`. Schema matches ADR exactly (sequence, context_id, event_type, node_ids_json, edge_ids_json, adapter_id, created_at). Index on `(context_id, sequence)` is present.
- `PersistedEvent`, `CursorFilter`, `ChangeSet` types are defined at `src/query/cursor.rs`
- `PlexusApi::changes_since()` exists at `src/api.rs:532-550` with correct signature
- Events are persisted during `emit_inner()` (fix was committed in 8b3230b per git log)

One gap against ADR-036 §5 (which amends the evidence trail to accept `QueryFilter`) is recorded as Violation 2 in the table above — it touches the ADR-035 data model but is an ADR-036 prescription.

---

### ADR-037: Consumer Spec Loading — NOT IMPLEMENTED (expected)

This is a proposed ADR. All gaps are expected work items, not bugs. The complete set:

**Violations 3–8** cover the five implementation areas:

- `register_specs_from_dir` wires only `register_adapter`, silently dropping enrichments and lens from specs (Violation 3 — this is the one violation that touches currently-accepted ADR territory: ADR-033 prescribed `adapter.lens()` and `adapter.enrichments()` be extracted during registration, but the file-based path never calls them)
- `PlexusApi::load_spec` absent (Violation 4)
- `specs` table absent from SQLite schema (Violation 5)
- Startup re-instantiation from `specs` table absent (Violation 6)
- Runtime registration requires interior mutability not yet present on `IngestPipeline` (Violation 7)
- `PlexusApi::unload_spec` absent (Violation 8)

**Note on Violation 3.** Of all the ADR-037 gaps, Violation 3 is the most immediately impactful to already-deployed behavior. Any consumer using file-based spec auto-discovery today (via `PipelineBuilder::with_adapter_specs()`) will have their spec's enrichments and lens silently ignored. The fix is a two-line change: replace `self.register_adapter(Arc::new(adapter))` with a call that first extracts enrichments and lens then calls `register_integration`. This is safe to fix independently of the rest of ADR-037 implementation.

---

### ADR-036: MCP Query Surface — NOT IMPLEMENTED (expected)

This is a proposed ADR. All gaps are expected work items.

**Violations 9–14** are the six absent MCP tools. The corresponding `PlexusApi` methods all exist and are correct — no Rust query logic is missing. These are purely transport wiring gaps:

- `find_nodes`, `traverse`, `find_path`, `changes_since`, `list_tags`, `shared_concepts` — all absent from `src/mcp/mod.rs`
- `load_spec` (ADR-036 §1's seventh tool) depends on ADR-037 implementation; not separately counted

The MCP module comment at line 3 still reads "Tools: 9 total" — this will need updating once the new tools are wired.

Additionally, the `evidence_trail` MCP tool at `src/mcp/mod.rs:238-246` does not accept `QueryFilter` parameters (flat optional fields per ADR-036 §2). This is the same gap as Violation 2 — one fix handles both the API method and the MCP tool.

---

## Notes

**Build order implied by violations.** The violations form a dependency chain that suggests a natural implementation sequence:

1. Fix Violation 3 first (independent, low risk, fixes silent enrichment/lens drops for current users)
2. Implement ADR-037 §2 + §5 (specs table + interior mutability) — these are prerequisites for `load_spec`
3. Implement ADR-037 §1 + §2 startup re-instantiation (load_spec + startup wiring)
4. Implement ADR-037 §6 (unload_spec)
5. Implement ADR-036 tools (thin wrappers once API methods exist)
6. Add `QueryFilter` to `evidence_trail` (Violation 2, affects API + MCP tool)

**`RankBy::NormalizedWeight` ambiguity.** The ADR text includes this variant but no implementation work addressed it during WP-C. The normalization strategies in `src/query/normalize.rs` are used elsewhere (node weight normalization for queries) but are not connected to post-processing ranking. This could be intentional scope reduction or an oversight. It warrants a decision before ADR-034 is marked Accepted.

**ADR status discrepancy.** All five ADRs are marked "Proposed" in their files. The memory and git log indicate ADRs 033-035 were built and merged (commits 4cb566e, 8b3230b, 8333d10). The ADR files should be updated to "Accepted" status — the implementation exists and conforms. ADRs 036 and 037 are correctly "Proposed" as their implementation has not begun.

**Interior mutability scope.** ADR-037 §5 defers the specific concurrency mechanism to build time. The current `IngestPipeline` uses `&mut self` for registration, which is incompatible with the `Arc<IngestPipeline>` held by `PlexusApi`. The fix will require either `Arc<RwLock<IngestPipeline>>` in `PlexusApi`, or moving registration methods to take interior-mutable fields. The choice affects `PlexusApi`'s Clone semantics and should be decided before implementation begins.
