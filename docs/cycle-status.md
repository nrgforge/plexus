# Active RDD Cycle: MCP Consumer Interaction Surface

**Started:** 2026-04-01
**Current phase:** BUILD (in progress — WP-A through WP-H.1 shipped; **WP-H.2 pending** for live MCP subprocess acceptance test)
**Artifact base:** ./docs/
**Scope:** Scoped cycle — MCP transport query surface + multi-consumer spec/lens interaction model

## Phase Status

| Phase | Status | Artifact | Key Epistemic Response |
|-------|--------|----------|----------------------|
| DECIDE (partial) | ⏸ Paused | ADR-036 draft, scenarios/036 draft, reflections/003 | User identified gap: query tools incomplete without consumer configuration model. Corrected tag bias in scenarios. Clarified provenance writes out of scope. |
| DISCOVER (update) | ✅ Complete | product-discovery.md (updated 2026-04-02) | User reframed spec loading as transport-independent API operation; consumer owns spec. Clarified durable/transient distinction. Established fail-fast validation as design constraint. Named the e2e acceptance criterion: first real MCP consumer workflow. |
| MODEL (amendment) | ✅ Complete | domain-model.md (amended 2026-04-02) | User confirmed amendments capture the right scope. Vocabulary layer named as concept. Three new invariants (60-62). |
| DECIDE (complete) | ✅ Complete | ADR-037, ADR-036 update, scenarios 036-037, interaction-specs, audits | Audits passed, fixes applied. CRUD-on-spec mental model accepted; concrete operation framing kept in ADR. Update path uses retract_contributions + load_spec composition. |
| ARCHITECT | ✅ Complete | system-design.md v1.2 (Amendment 5), roadmap.md regenerated, ORIENTATION.md regenerated | User corrected framing from server-mode to library-mode, which sharpened the startup rehydration decision: `PipelineBuilder::with_persisted_specs` at construction (Option C) with host reading specs table and passing data in (C2 sub-option, no new dependency edges). User framed the specs table as "the context's lens registry" — any library instance against that context transiently runs those lenses on behalf of the context. User chose six-WP granularity for reviewability and structure/behavior separation. User folded `RankBy::NormalizedWeight` into WP-G as G.2 with own commit/test cycle. User set the standing principle: ADRs are immutable unless genuinely superseded. |
| BUILD (WP-A) | ✅ Complete | `925d76a` fix: register_specs_from_dir wires enrichments and lens | — |
| BUILD (WP-B) | ✅ Complete | `7a12874` feat: specs persistence foundation, interior mutability, builder rehydration | Interior mutability: `RwLock<Vec<...>>` chosen (Tier 1). `PersistedSpec` struct (not tuple). Builder rehydration enrichment-only. Non-fatal "log and continue" for malformed specs. |
| BUILD (WP-C) | ✅ Complete | `22838b5` feat: load_spec and unload_spec on PlexusApi | Three-effect model working. Manual rollback on failure. Validation: YAML parse + DeclarativeAdapter::from_yaml + lens extraction. |
| BUILD (WP-C fix) | ✅ Complete | `fbe7fb7` fix: store context UUID in specs table | Latent WP-C bug surfaced while writing WP-D test: specs table was keyed by caller-supplied string (effectively name) not stable UUID. Rename would orphan persisted specs. Fixed in load_spec and unload_spec by resolving name → UUID before all specs-table I/O. |
| BUILD (WP-D) | ✅ Complete | `6661d2c` feat: startup spec rehydration via host + builder | `gather_persisted_specs(&engine)` helper iterates contexts and accumulates specs. `default_pipeline` auto-rehydrates. Acceptance test uses on-disk SQLite across two consumer lifetimes — real types, no mocks. |
| BUILD (pre-WP-F fix) | ✅ Complete | `0b9d9d3` fix: api.ingest accepts context name, not UUID | Closed latent MCP ingest bug. api.ingest and api.ingest_with_adapter now resolve names internally, consistent with load_spec/find_nodes/evidence_trail/etc. TestEnv gained ctx_name() helper; call sites throughout updated from env.ctx_id() to env.ctx_name() for ingest (UUID convention preserved where it belongs: GraphEvent construction). |
| BUILD (WP-E) | ✅ Complete | `38612bd` feat: 6 MCP query tools | Thin wrappers over PlexusApi: find_nodes, traverse, find_path, changes_since, list_tags, shared_concepts. Flat optional parameters (ADR-036 §2). Query result types gained serde::Serialize. 8 boundary integration tests (one per tool + no-active-context error path) using real SqliteStore + real pipeline (no mocks). Tool count 9 → 15. |
| BUILD (WP-F) | ✅ Complete | feat: MCP load_spec tool | 16th MCP tool. Inline spec_yaml parameter per ADR-036 §2. Handler delegates to `PlexusApi::load_spec`, marshalls `SpecLoadResult` via inline `serde_json::json!` (keeps MCP wire format decoupled from the API type), marshalls `SpecLoadError` via existing `err_text` pattern. 2 boundary tests: valid spec → JSON carries adapter_id + lens_namespace + vocabulary_edges_created; malformed YAML → `is_error: true` with "validation" substring. No-active-context error path is covered by WP-E's shared test (N+M+1 pruning). TS-6 reached: end-to-end MCP consumer workflow (create context → load_spec → ingest → query through lens) is now achievable. |
| BUILD (WP-F follow-up) | ✅ Complete | test: load_spec returns error without active context | Dedicated boundary test for load_spec's `self.context()?` branch. Redundant with WP-E's shared-path test on paper but catches a specific regression mode (future change to load_spec's no-context handling that diverges from query tools). 8 lines, strictly additive coverage. |
| BUILD (WP-G.1) | ✅ Complete | `98343bb` feat: evidence_trail accepts optional QueryFilter | Closes Invariant 59 for evidence_trail. `contributor_ids` and `min_corroboration` compose meaningfully with evidence-dimension edges; `relationship_prefix` included for API consistency (typically empty results — docstring and tool description note this). Breaking change to `PlexusApi::evidence_trail` and `query::evidence_trail` signatures: both gain required `Option<QueryFilter>` parameter. Threading applied to both StepQuery branches (references+contains branch 1, tagged_with branch 2). 2 new tests: unit-level filter-threading test + MCP-level boundary test using nonexistent-contributor filter to prove filter is actually applied. |
| BUILD (WP-G.2) | ✅ Complete | `22e24a2` feat: RankBy::NormalizedWeight variant | Closes ADR-034 conformance debt. `Box<dyn NormalizationStrategy>` variant; Clone/Copy/PartialEq/Eq derives dropped (zero usage in codebase); manual `Debug` impl. Signature change: `TraversalResult::rank_by` gains `&Context` parameter. New `PlexusApi::rank_traversal()` helper resolves the context internally so MCP remains thin-shell. NormalizedWeight NOT exposed via MCP — ADR-036 §1 specifies only "raw_weight" and "corroboration". Wart did not cascade — roadmap's worst-case (revert G.2) didn't materialize. 2 unit tests in `query::types::tests`. |
| BUILD (WP-H.1) | ✅ Complete | `81ce6ef` refactor: remove file-based spec auto-discovery | Scope-reductive removal delivered in a single commit. Net −224 lines across 11 files. ADR-037 §4 superseded with dated note; domain-model Invariant 61 narrowed (Amendment 8); system-design Amendment 6 records the removal. Also removed `PlexusMcpServer::with_project_dir` (sole caller of `with_adapter_specs`) and the `project_dir` parameter on `default_pipeline` and `run_mcp_server` — scope broader than roadmap line-item list but follows mechanically from `with_adapter_specs` removal. 494 → 492 tests. Orthogonal rehydration path (`with_persisted_specs` → `specs` table) verified unaffected. |
| BUILD (WP-H.2) | ☐ Pending | — | Live MCP subprocess acceptance test covering the two-consumer vocabulary-layer cross-pollination story end-to-end. Satisfies the product-discovery acceptance criterion at the transport layer. Raw JSON-RPC over stdin/stdout (no rmcp client dependency). Hard dep on H.1 (now unblocked). |

## Feed-Forward Signals

### From DECIDE (partial)
1. ADR-036 covers 6 query tools + flat filter serialization + client-managed cursors — still valid
2. Provenance writes explicitly out of scope for MCP — ingest is the only write, provenance flows from ingestion
3. Lens wiring gap found: `register_specs_from_dir` registers adapters but doesn't extract/register lens enrichments
4. Multi-consumer model surfaced: a context is a graph with multiple consumers, each encoding their own lens vocabulary
5. Spec/lens interaction needs to be at PlexusApi level (transport-independent), not MCP-specific
6. Scenarios should not bias toward tags — Plexus derives structure from unstructured input

### From MODEL (attempted, paused)
7. Key reframing from user: lenses are not "registered" as runtime configuration — they **encode structure into the graph**. Vocabulary layers are persistent graph data, not ephemeral config.
8. Lens discovery is graph introspection (querying `lens:*` edge prefixes), not registry lookup.
9. Loading a spec causes the lens enrichment to run and encode edges. After that, the vocabulary layer exists in graph data regardless of lens state.
10. New stakeholder jobs surfaced (discovering vocabularies, navigating other consumers' lenses) — needs product discovery update before domain modeling.

### From DECIDE (complete)
24. ADR-037 (consumer spec loading): `load_spec` and `unload_spec` on PlexusApi. Three-effect model. SQLite `specs` table with `(context_id, adapter_id)` composite key. Startup re-registration is enrichment-only (no lens re-run). Spec update path: retract old contributions + load new spec.
25. ADR-036 updated: 7 new MCP tools (16 total). `evidence_trail` filter field scoping documented.
26. Conformance debt to fix in BUILD: (a) `register_specs_from_dir` drops enrichments+lens — independent two-line fix, (b) `evidence_trail` missing `QueryFilter` parameter, (c) `RankBy::NormalizedWeight` variant absent — deferred scope.
27. Build order from conformance scan: fix Violation 3 first → ADR-037 §2/§5 (specs table + interior mutability) → §1/§2 startup → §6 unload → ADR-036 tools → evidence_trail filter.
28. Interior mutability decision deferred to BUILD: `Arc<RwLock<IngestPipeline>>` vs. moving registration to interior-mutable fields. Affects PlexusApi clone semantics.
29. ADRs 033-035 marked Accepted (housekeeping per conformance scan).

### From ARCHITECT
30. Library-mode framing is the correct deployment model for the foreseeable future. Server mode is future work. Every architectural decision should assume: fresh process, opens SQLite, constructs library, serves one consumer or a small number, exits.
31. Startup rehydration lives at `PipelineBuilder::with_persisted_specs(Vec<(ContextId, String)>)` construction time, not at engine/api/host-ceremony level. Host reads specs table via `GraphStore::query_specs_for_context` and passes data into the builder. No new dependency edges introduced by the cycle.
32. The specs table semantics: it IS the context's lens registry. Any library instance holding a context transiently runs every lens registered on it. Cross-pollination between consumer domains is automatic as a consequence of construction, not a consequence of interactive coordination. This is how Invariant 62 effect (b) manifests in library mode.
33. Interior mutability on IngestPipeline is scoped strictly to the adapter vector and enrichment registry; core routing logic stays non-mutable on `&self`. Specific mechanism (RwLock vs DashMap vs ArcSwap) is a WP-B decision. Lock holding across `ingest()` is forbidden.
34. Six work packages (A-G) with A, B, E, G.1, G.2 as open-choice starting points. WP-A (register_specs_from_dir bug fix) ships independently ahead of the rest of the cycle. Hard dependencies: C on B, D on B+C, F on C.
35. ADR-034 conformance debt (`RankBy::NormalizedWeight`) folded into WP-G as G.2 with own commit/test cycle. Independently revertible if the type-system wart is worse than expected.
36. Standing principle: ADRs are immutable authoritative records; amend them only when a later decision genuinely supersedes them. "What shipped was slightly different from what the text said" is not sufficient reason to amend — but "a later cycle made a contradicting decision" is. Mark supersessions explicitly in the ADR file.
37. Fitness criterion carried forward: no new module-level dependency edges. If a future decision needs a new edge, that should be a deliberate architectural call, not a side effect.
38. **Specs table is the authoritative lens registry — no runtime filtering by consumers.** A consumer constructing the library against a context MUST rehydrate every spec in the table for that context. Selective loading would break Invariant 62 (vocabulary layers become non-deterministic, cross-pollination degrades to single-consumer semantics, debugging gets ambiguous). Legitimate curation paths: `unload_spec` (durable, public, affects all consumers), reload with fixed spec, new context boundary, or a future `ingest_without_enrichment` flag — never runtime startup filtering.
39. **Interior mutability decision has two tiers, not one.** Tier 1 (RwLock vs ArcSwap within Vec-based storage) is a local BUILD decision — swappable without ripple. Tier 2 (restructuring adapter storage to a keyed collection like DashMap) is a structural change that would need its own ADR. The fitness criterion in system-design.md explicitly forbids Tier 2 in this cycle. **If BUILD discovers reason to want Tier 2, pause and escalate.** Updated docs accordingly.
40. **`PersistedSpec` is a struct, not a tuple.** `GraphStore::query_specs_for_context` returns `Vec<PersistedSpec>`. This enables non-breaking evolution of the persisted-spec shape — additional fields can be added without breaking callers. Small investment at WP-B time; large payoff whenever the specs table schema evolves.
41. **Spec YAML grammar is additive-only until versioning is introduced.** No renaming fields, no removing primitives, no restructuring sections — any such change would break rehydration of existing spec rows under the current "log and continue" error policy. When the first breaking grammar change is genuinely needed, pause and add `spec_version` field + migration path + **fail-loud** unknown-version policy. Not in scope for this cycle, but the discipline is active now.
42. **Concurrent-process spec cache staleness is latent.** The specs table is authoritative, but each process's in-memory pipeline is a cache that doesn't auto-refresh. If two processes hold the library simultaneously and one updates a spec, the other doesn't see the change until next restart. Library mode assumes one-process-at-a-time workflows, so this is fine today. Becomes relevant with concurrent embedded consumers or Plexus-as-server — flagged in open decisions so it doesn't get discovered under pressure.

### From DISCOVER (update)
11. Spec loading is a transport-independent API operation (`load_spec` on PlexusApi). File-based auto-discovery and programmatic loading are both delivery paths to the same operation.
12. Consumer owns the spec — Plexus never generates or manages specs on the consumer's behalf.
13. `load_spec` has three effects: (a) durable graph data (lens writes vocabulary edges immediately); (b) durable enrichment registration (lens translation rules persist on context, reactive to all future emissions); (c) transient adapter wiring (available for ingest routing during workflow). On startup, Plexus re-loads persisted lens specs.
14. Fail-fast validation: `load_spec` validates the spec before any graph work. If it succeeds, everything is wired. If it fails, nothing happened. Resource-intensive ingestion justifies strict upfront validation.
15. Individual ingestions may be long-running (semantic extraction via llm-orc, declared enrichments). The graph enriches incrementally — consumers don't block on all enrichments completing.
16. Vocabulary layer discovery is graph introspection — emergent from `lens:*` namespace + composable filters. May not need a dedicated query, just documentation of the pattern.
17. End-to-end acceptance criterion: create context → load spec → ingest (including semantic extraction) → query through lens → load second spec → query across both vocabulary layers. First real MCP consumer workflow.

### From MODEL (amendment)
18. Three new invariants: 60 (upfront spec validation), 61 (consumer owns spec), 62 (vocabulary layers are durable graph data). These are constitutional — ADR-037 must respect them.
19. Vocabulary layer is a named concept (resolves OQ-26). Emergent from lens output + namespace convention, not a stored structure.
20. `load_spec` is a new action — the declarative equivalent of `register_integration()`. Transport-independent, fail-fast.
21. `discover vocabularies` is a query pattern, not a dedicated API method. Uses composable filters with `relationship_prefix: "lens:"`.
22. OQ-23, OQ-24, OQ-25, OQ-26 resolved. OQ-20 partially resolved (ADR-033 settled the contract; delivery/wiring path is for ADR-037). Lens persistence mechanism (storage for translation rules) is a design question for ADR-037.
23. Spec concept expanded: unit of consumer identity containing adapter + lens + enrichment config. Consumer concept updated for multi-consumer contexts.

## Context for Resumption

The cycle started as "expose query tools via MCP" (decide+build). During decide, the user identified that the query surface is incomplete without the consumer configuration model. During model attempt, the user clarified that lenses encode persistent graph structure (not runtime configuration) and that new stakeholder jobs need product discovery before domain modeling. Product discovery was updated with the multi-consumer interaction model, spec loading as transport-independent API operation, fail-fast validation, and the e2e acceptance criterion. Domain model amended with invariants 60-62 and vocabulary layer concept. ADRs 036 and 037 accepted, scenarios written, interaction specs derived, audits passed.

ARCHITECT complete (2026-04-07): system-design.md v1.2 with Amendment 5, roadmap.md regenerated with six work packages (A-G), ORIENTATION.md regenerated. Key design refinements from the architect phase:
- **Library-mode framing confirmed** (user correction): Plexus is a library operating on SQLite, not a server. "Restart" = fresh process constructing the library against the same database. This framing shaped every startup-related decision.
- **Startup rehydration via PipelineBuilder** (not via engine or an api-level reload method): `PipelineBuilder::with_persisted_specs(Vec<(ContextId, String)>)` rehydrates persisted lens enrichments at construction time. Host reads specs table; builder takes data. No new dependency edges. Rehydration is enrichment-only (skip effect a, skip adapter wiring) because vocabulary edges already persist from the original `load_spec` call.
- **The specs table is the context's lens registry** — any library instance against a context transiently runs those lenses on behalf of the context, not on behalf of the originally-loading consumer. This is how multi-consumer cross-pollination works.
- **Six-WP decomposition**: A (bug fix) / B (foundation + interior mutability) / C (load_spec / unload_spec on api) / D (rehydration via host + builder) / E (6 MCP query tools) / F (MCP load_spec tool) / G (evidence_trail filter + RankBy::NormalizedWeight, two commits).
- **Standing principle set**: ADRs are immutable unless genuinely superseded — never amended casually to match what shipped.

### From BUILD (WP-A/B/C)
43. WP-A shipped as a two-line fix plus regression test. `register_specs_from_dir` now calls `register_integration` (extracts enrichments + lens) instead of `register_adapter` alone.
44. WP-B open decisions resolved: interior mutability uses `RwLock<Vec<Arc<dyn Adapter>>>` + `RwLock<EnrichmentRegistry>` (Tier 1, simple, registration-infrequent). `PersistedSpec` is a struct with `context_id`, `adapter_id`, `spec_yaml`, `loaded_at`. Rehydration error handling: non-fatal "log and continue" — a broken persisted spec doesn't prevent library startup. Builder method signature: `with_persisted_specs(specs: Vec<PersistedSpec>)`.
45. WP-C open decisions resolved: manual rollback on failure (undo each successful step on error). Validation: YAML parses + `DeclarativeAdapter::from_yaml` succeeds + lens extraction succeeds. Three-effect model (persist spec row, register enrichments/lens on pipeline, run lens on existing content) working end-to-end.
46. 476 tests total (410 lib + 66 acceptance), all passing as of WP-C completion.

### From BUILD (WP-C fix + WP-D)
47. WP-C latent bug surfaced by WP-D test writing: specs table was keyed by caller-supplied string, which is effectively the context NAME because `PlexusApi::resolve` only accepts names. This silently orphaned specs on rename and broke UUID-based rehydration iteration. Fixed in `fbe7fb7`: `load_spec` and `unload_spec` resolve name → UUID before all specs-table I/O. Table is now keyed by stable `ContextId`.
48. WP-D design decision: put gather-specs logic in a `gather_persisted_specs(&engine)` pub free function in `adapter/pipeline/builder.rs`, and have `default_pipeline` call it automatically. Result: all hosts using `default_pipeline` (including `run_mcp_server`) rehydrate automatically without binary-level changes. Explicit hosts can still call `gather_persisted_specs` + `with_persisted_specs` manually. No new module dependency edges (builder already depended on engine).
49. Acceptance test `persisted_spec_rehydrates_across_restart` is fully-integrated (real SqliteStore on disk across `drop` + reopen, real builder chain, real enrichment loop). Proves Invariant 62 effect (b) end-to-end in library-mode.
50. **Follow-up concerns discovered but explicitly out of WP-D scope:**
    - `PlexusApi::ingest` passes `context_id` straight to `pipeline.ingest` which treats it as a literal UUID, unlike `load_spec`/`find_nodes`/`evidence_trail` which resolve by name. All existing `api.ingest` callers use UUIDs. API contract inconsistency.
    - **Probable latent MCP bug:** `set_context` stores a name; `ingest` tool passes that name to `api.ingest`, which would fail because pipeline expects UUID. No existing test exercises MCP ingest end-to-end, so the bug is dormant. Needs attention before WP-F ships.
    - File-based + persisted spec interaction: if a consumer has both `{project_dir}/adapter-specs/` AND a persisted spec for the same `adapter_id`, the lens enrichment registers twice. Idempotency protects edge output but the enrichment loop may fire twice per event. Consider de-duplication or choice-of-source policy in a later cycle.
51. 477 tests total (410 lib + 67 acceptance), all passing as of WP-D completion.

### From BUILD (pre-WP-F fix + WP-E)
52. MCP name-vs-UUID latent bug closed (`0b9d9d3`). Same class as the WP-C specs-table fix: `context_id: &str` was semantically ambiguous and different endpoints had different expectations. `api.ingest` and `api.ingest_with_adapter` now resolve names internally, consistent with every other PlexusApi method. `MCP::set_context` stores names → passes to `api.ingest` → which now resolves → works. This fix is the pre-WP-F blocker — without it, WP-F's e2e acceptance criterion cannot pass.
53. WP-E shipped 6 MCP query tools as thin wrappers (Invariant 38). Key design choices: flat optional parameters (ADR-036 §2), direction/rank_by parsed from strings with explicit "invalid X" error messages, composable filter elided when no filter fields set. Query result types (`QueryResult`, `TraversalResult`, `PathResult`, `ChangeSet`) gained `serde::Serialize` — minor extension consistent with inner types (Node, Edge, NodeId already Serialize).
54. Boundary integration tests: 8 total (one per tool + one no-active-context shared path). All use real SqliteStore + real pipeline — no mocks at any layer. Scenario coverage intentionally partial per N+M+1: wiring verification doesn't need to duplicate across every tool when one tool already proves the wiring.
55. Tool count: 9 → 15. WP-F adds the 16th.
56. 487 tests total (419 lib + 67 acceptance + 1 doc), all passing as of WP-E completion.

## Context for Resumption

The cycle started as "expose query tools via MCP" (decide+build). During decide, the user identified that the query surface is incomplete without the consumer configuration model. During model attempt, the user clarified that lenses encode persistent graph structure (not runtime configuration) and that new stakeholder jobs need product discovery before domain modeling. Product discovery was updated with the multi-consumer interaction model, spec loading as transport-independent API operation, fail-fast validation, and the e2e acceptance criterion. Domain model amended with invariants 60-62 and vocabulary layer concept. ADRs 036 and 037 accepted, scenarios written, interaction specs derived, audits passed.

ARCHITECT complete (2026-04-07): system-design.md v1.2 with Amendment 5, roadmap.md regenerated with six work packages (A-G), ORIENTATION.md regenerated. Key design refinements from the architect phase:
- **Library-mode framing confirmed** (user correction): Plexus is a library operating on SQLite, not a server. "Restart" = fresh process constructing the library against the same database. This framing shaped every startup-related decision.
- **Startup rehydration via PipelineBuilder** (not via engine or an api-level reload method): `PipelineBuilder::with_persisted_specs(Vec<PersistedSpec>)` rehydrates persisted lens enrichments at construction time. Host reads specs table; builder takes data. No new dependency edges. Rehydration is enrichment-only (skip effect a, skip adapter wiring) because vocabulary edges already persist from the original `load_spec` call.
- **The specs table is the context's lens registry** — any library instance against a context transiently runs those lenses on behalf of the context, not on behalf of the originally-loading consumer. This is how multi-consumer cross-pollination works.
- **Six-WP decomposition**: A (bug fix) / B (foundation + interior mutability) / C (load_spec / unload_spec on api) / D (rehydration via host + builder) / E (6 MCP query tools) / F (MCP load_spec tool) / G (evidence_trail filter + RankBy::NormalizedWeight, two commits).
- **Standing principle set**: ADRs are immutable unless genuinely superseded — never amended casually to match what shipped.

BUILD in progress. WP-A through WP-G.2 shipped (see Phase Status table above for full commit trail). **WP-H pending** — the cycle's e2e acceptance criterion requires live MCP transport verification, which was not delivered by the prior WPs. WP-H also folds in a scope-reductive design correction (remove file-based auto-loading).

**Current test count:** 494 (426 lib + 67 acceptance + 1 doc). WP-H will remove a handful of tests tied to deleted code (H.1) and add one substantial subprocess acceptance test (H.2); net change likely small.

**When WP-H ships:** cycle reaches TS-7 (full e2e verification + intentional-only spec loading). At that point: optional follow-up phases `/rdd-play` (post-build experiential discovery), `/rdd-synthesize` (essay outline), or `/rdd-graduate` (fold into native docs + archive).

**Previously claimed "BUILD complete" (in an earlier revision of this document) was premature.** The product-discovery-defined acceptance criterion is verified end-to-end only at TS-7, not TS-6. The distinction between "capability achievable in principle" (TS-6) and "capability verified under real MCP framing" (TS-7) is load-bearing for a cycle whose stated goal is "first real MCP consumer workflow."

### From BUILD (WP-F)
57. WP-F shipped as a single thin-wrapper handler plus one `LoadSpecParams` struct — 18 lines of delegation + JSON marshalling + 2 boundary integration tests. No changes to `PlexusApi::load_spec` or any downstream code. All cycle work for ADR-036 §1 is now complete.
58. Inline `serde_json::json!` was preferred over deriving `Serialize` on `SpecLoadResult`. Rationale: the API struct is a Rust return type, not a public JSON contract; keeping wire format decoupled means MCP can evolve the response shape without forcing the API type to follow. Consistent with the `shared_concepts` pattern already established in WP-E, not with the query-result-type pattern (which was `Serialize` because those types are already shared between in-process callers and transports).
59. Boundary test scope pruned per N+M+1: MCP-layer tests verify JSON delegation + marshalling only. Adapter routing verification (`registered_input_kinds`), three-effect model end-to-end, and unload lifecycle are all covered by `tests/acceptance/spec.rs` at the API layer — duplicating them at the MCP layer would add no predictive power.
60. 489 tests total (421 lib + 67 acceptance + 1 doc), all passing as of WP-F completion. No new conformance debt introduced. Cycle fitness criterion held: no new module-level dependency edges.

### From BUILD (WP-G.1)
61. WP-G.1 closes Invariant 59 for `evidence_trail` — previously the one query primitive that couldn't be provenance-scoped. Filter threading applied to both StepQuery branches (references+contains; tagged_with).
62. Breaking signature change on both `PlexusApi::evidence_trail` (two args → three) and `query::evidence_trail` (two args → three). All in-project callers updated to pass `None` for the filter. Pre-1.0 crate; breaking changes are acceptable.
63. `relationship_prefix` included for API consistency but typically yields empty results for evidence trails because evidence-dimension edges (`references`, `contains`, `tagged_with`) do not use `lens:` prefixes. Tool description and docstrings document this — explicit upfront rather than a surprise for LLM consumers.
64. Boundary test design pattern for filter-threading verification: use a filter that SHOULD yield empty results if applied (e.g., nonexistent contributor ID), then assert the empty outcome. This proves the filter is actually threaded, not silently ignored.
65. 492 tests total (424 lib + 67 acceptance + 1 doc), all passing as of WP-G.1 completion. No new module-level dependency edges; the `composable_filter()` helper from WP-E was reused verbatim — WP-E's pattern held up without modification.

### From BUILD (WP-G.2)
66. WP-G.2 closes ADR-034 conformance debt (`RankBy::NormalizedWeight` variant specified but never implemented). Type-system wart flagged in the roadmap did not cascade: `Clone/Copy/PartialEq/Eq` were unused on `RankBy` (grep-verified), so dropping them was free. Manual `Debug` impl was 8 lines.
67. Container choice: `Box<dyn NormalizationStrategy>` (not `Arc`). Rationale: Arc's payoff is cheap-Clone; nothing in the codebase clones `RankBy`. Box is simpler and smaller today. If a future consumer needs Clone, revisit.
68. `TraversalResult::rank_by` signature grew a `&Context` parameter — needed for NormalizedWeight's per-node-neighborhood computation; uniformly accepted for RawWeight and Corroboration. To preserve the thin-shell transport principle (Invariant 38), MCP never fetches Context directly; `PlexusApi::rank_traversal` helper does it internally.
69. NormalizedWeight NOT exposed via MCP — ADR-036 §1 specifies only "raw_weight" and "corroboration". Adding it to `parse_rank_by` would be feature creep. If LLM consumers need normalized ranking later, amend ADR-036 formally rather than sneaking it in.
70. Test design for rank dispatch: construct two source nodes with different neighborhood densities so normalized and raw orderings diverge. A silent-broken branch returning raw weights instead of normalized would fail the assertion — not a passable-by-accident test.
71. 494 tests total (426 lib + 67 acceptance + 1 doc), all passing as of WP-G.2 completion. No new module-level dependency edges.

### From BUILD (WP-H.1)
79. WP-H.1 shipped as `81ce6ef`. Net −224 lines across 11 files — scope-reductive refactor. One commit containing code + ADR supersession + domain-model amendment + system-design amendment + product-discovery revision + scenarios removal.
80. Scope broader than the roadmap's WP-H.1 line-item list: `PlexusMcpServer::with_project_dir` and the `project_dir` parameter on `default_pipeline`/`run_mcp_server` also removed. Rationale: these existed solely to thread the project directory into `with_adapter_specs`; once the latter is gone, the plumbing is dead. Removing it keeps the API surface honest — callers that want to pass a project dir into the MCP server would now have no reason to.
81. OQ-H5 resolved cleanly: `integration_tests.rs:4324-4360` test ("Specs loaded from directory") was exclusively exercising `register_specs_from_dir`. No orthogonal intent — deleted without translation.
82. OQ-H2 resolved as deliberate behavior change: dropping YAML into `{project_dir}/adapter-specs/` now silently does nothing. Documented in ADR-037 supersession note.
83. Invariant 61 amendment trail: the invariant's core assertion ("consumer owns the spec") is unchanged. The trailing clause "Whether the spec arrives as a file on disk or programmatically through the API, the operation is the same" was factually wrong post-H.1 and is struck. Recorded as domain-model Amendment 8. System-design Amendment 6 is the architectural record of the same change. ADR-037 §4 supersession note cites both.
84. Orthogonal rehydration verified unaffected: `persisted_spec_rehydrates_across_restart`, `two_consumers_two_lenses_on_same_context`, and `builder_with_persisted_specs_{rehydrates_lens_only,skips_malformed}` all pass. The `specs` table → `with_persisted_specs` path is now the unique startup rehydration mechanism.
85. 492 tests total (425 lib + 66 acceptance + 1 doc), all passing as of WP-H.1 completion. No new module-level dependency edges.

### For BUILD (WP-H.2) — upcoming
72. **Cycle acceptance criterion under-verified:** product-discovery (2026-04-02) named "first real MCP consumer workflow" as the e2e acceptance definition. WP-A through WP-G.2 built the components and verified each at the boundary test layer, but no single test exercises the full workflow through live MCP protocol framing. WP-H closes that gap via subprocess-driven acceptance test.
73. **Design correction identified at WP-G.2 reflection:** file-based spec auto-loading (`register_specs_from_dir`) violates consumer-intent (Invariant 61) and creates latent double-registration with persisted specs. Removing it converges to one intentional path: `load_spec`. The original ADR-037 framing ("file-based auto-discovery is one delivery path; programmatic loading is the general case") is superseded — not casually, per the standing principle, but as a genuine decision change where the new rationale supersedes the old.
74. **WP-H structure:** two sub-packages in one WP.
    - **H.1 — Remove file-based spec auto-loading** (`refactor:` commit): delete `register_specs_from_dir`, `with_adapter_specs`, and the `adapter-specs/` auto-call in `default_pipeline`. Update ADR-037 with supersession note. Update product-discovery.md and domain-model.md. Delete the WP-A regression test (it tests removed code). Verify orthogonal rehydration path still works.
    - **H.2 — Live MCP e2e harness** (`test:` commit): subprocess spawns `plexus mcp --db <temp>` via `CARGO_BIN_EXE_plexus`; raw JSON-RPC over stdin/stdout (no rmcp client dep); one test exercising the two-consumer happy path end-to-end.
75. **Hello-world scenario specified in roadmap.md WP-H.2:** set_context → load_spec(Consumer-1) → ingest → find_nodes(lens:consumer-1:) → load_spec(Consumer-2) → ingest → find_nodes(lens:consumer-1:) shows edges from both → find_nodes(lens:consumer-2:) shows edges from both → cross-pollination verified.
76. **Open questions for WP-H implementation** (also recorded in roadmap.md under "Open questions raised during WP-H planning"):
    - **OQ-H1** rmcp protocol version in `initialize` handshake — discover by observation on first attempt
    - **OQ-H2** `default_pipeline` semantics after H.1 — anything dropping YAML into `adapter-specs/` now silently does nothing; document this deliberate behavior change in the supersession note
    - **OQ-H3** cross-pollination visibility via `find_nodes` after H.2 — confirm the event log persists lens-created edges from the second consumer's ingest (should already hold after WP-C fix for enrichment event persistence, but worth an explicit harness check)
    - **OQ-H4** test timing budget — tentative 5s per MCP call, 30s whole test; revisit if flaky
    - **OQ-H5** `src/adapter/integration_tests.rs:4350` uses `register_specs_from_dir` — check whether its test intent is still covered elsewhere before deletion; if not, translate to `load_spec`-based test before removing the old one
77. **Adapter choice for H.2 ingest:** built-in content adapter (tag-based `FragmentInput`). Deterministic, Rust-only, no llm-orc dependency. Verifying transport, not extraction.
78. **Subprocess over in-process rmcp client:** maximum fidelity (actual compiled binary, actual stdio, actual protocol framing). One subprocess test is affordable; if the harness grows, reconsider.

## Resumption Instructions for Fresh Session

When resuming WP-H.2 in a new session:

1. Invoke `/rdd-build` — the skill will re-orient from cycle-status.md and roadmap.md.
2. Read `docs/roadmap.md` § **WP-H.2** (live MCP e2e harness sub-package: test architecture, hello-world scenario, subprocess invocation, adapter choice, risk section).
3. Read `docs/roadmap.md` § **Open questions raised during WP-H planning** — OQ-H1 (protocol version handshake), OQ-H3 (cross-pollination visibility via `find_nodes`), OQ-H4 (test timing budget) still open; OQ-H2 and OQ-H5 resolved by WP-H.1.
4. The binary invocation shape is: `Command::new(env!("CARGO_BIN_EXE_plexus")).args(["mcp", "--db", &temp_db_path])`. The clap binary at `src/bin/plexus.rs` accepts `mcp --db <path>` as verified during WP-H planning.
5. Test scope: ONE subprocess acceptance test exercising the two-consumer happy path. Use raw JSON-RPC over stdin/stdout (no rmcp client dependency). Verify spec loading is intentional-only after WP-H.1 removed the file-based path — dropping a YAML into any directory will not affect the test.
6. At WP-H.2 completion, the cycle reaches TS-7 and BUILD is genuinely complete. Optional follow-up phases (play, synthesize, graduate) become available.
