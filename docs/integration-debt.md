# Integration Conformance Debt — ADRs 001–018

**Date:** 2026-02-19
**Trigger:** Essay 22 identified integration gaps in ADRs 019–025. This audit extends backward to ADRs 001–018 using the same methodology: for each component boundary, check whether integration tests use real implementations or only stubs/mocks.

**Self-contained ADRs (no integration risk):** ADR-002 (storage location, superseded), ADR-016 (XDG path convention).

**Unimplemented ADR:** ADR-018 (emission-level replication) — zero code exists. No debt to track until implementation begins.

**False positive (verified not a bug):** ADR-006 `EngineSink::new()` usage. `router.rs:158` is test-only code (`#[cfg(test)]`); the router itself is sink-agnostic and not yet called in production. `bin/plexus.rs:338` uses the Mutex sink intentionally as a batch-accumulation pattern — persistence is handled via `engine.upsert_context()` at line 358.

---

## Conformance Debt Table

| # | ADR | Boundary | Gap | Severity | Resolution |
|---|-----|----------|-----|----------|------------|
| 1 | **003/005** | adapter → scale normalization → persist → load → query normalization | No test runs the full pipeline: adapter emission → `recompute_raw_weights` → `save_context` → `load_context` → `NormalizationStrategy.normalize` → assert `normalized_weight`. Each layer tested separately, never chained. | **High** | Write one integration test that emits through a real adapter, persists, reloads, and asserts the final query-time `normalized_weight` (divisive or softmax) is correct. |
| 2 | **009/015** | `annotate()` with `#`-prefixed tags | `annotate_triggers_enrichment` passes `"refactor"` (no `#`), so the `#`-stripping in `api.rs:77` is never exercised. If a caller passes `"#travel"` directly to `FragmentInput::new()` (bypassing `annotate()`), the concept ID becomes `concept:#travel` — the bridger looks for `concept:travel` and silently finds nothing. | **High** | Write an integration test calling `api.annotate()` with `"#travel"` as input, then assert the `references` edge exists from mark to `concept:travel`. |
| 3 | **007** | `raw_weight` column vs `contributions_json` consistency | Tests verify contributions survive persistence and that `recompute_raw_weights()` is idempotent, but no test verifies the stored `raw_weight` value matches what recomputation would independently produce without calling `recompute_raw_weights()` explicitly. | **Medium** | Write a test that loads from store and asserts the stored `raw_weight` equals the expected value from contributions — without calling `recompute_raw_weights()` in the test body. |
| 4 | **014** | `PlexusApi` → persistent store | All `PlexusApi` tests use `PlexusEngine::new()` (in-memory, no `SqliteStore`). No test of `PlexusApi` backed by a real store. `list_tags` scoping tested with hand-inserted nodes, not through `annotate()`. | **Medium** | Write a `PlexusApi` test using `PlexusEngine::with_store()`. Combine: `annotate()` in two contexts, then `list_tags()` on each, assert strict separation. |
| 5 | **017** | Two-engine concurrent access | `test_reload_if_changed_detects_external_write` is sequential (engine_a writes, then engine_b polls). No concurrent write test, no stale-cache-then-reload test, no duplicate enrichment test. `PlexusApi` has no `reload_if_changed()` method. | **Medium** | Write a test with two engines writing concurrently to the same file DB. Verify merged result is coherent. Test that enrichments don't duplicate when both engines run the loop independently. |

---

## Notes

- **Debt #2 is a fragile coupling.** The system works today because `TagConceptBridger.extract_normalized_tags()` strips `#` at line 113 of `tag_bridger.rs`. But `ProvenanceAdapter` stores tags verbatim (with `#`), while `FragmentAdapter` via `annotate()` generates concept IDs without `#`. The bridge is the only thing reconciling these conventions. An integration test makes this coupling explicit.
- **ADR-018** is excluded from the debt table. The entire replication subsystem (ReplicatedStore, emission journal, ingest_replicated) is unimplemented. Integration debt will be tracked when the code exists.

---

## Approach

These gaps become scenarios in the next `/rdd-decide` cycle that touches the relevant components. They can also be addressed as a focused integration-test sprint: two high-priority tests (#1, #2), then three medium-priority tests (#3, #4, #5). No production code changes are expected — these are `test:` commits that may reveal bugs requiring subsequent `fix:` commits.
