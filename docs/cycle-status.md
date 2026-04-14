# Active RDD Cycle: MCP Consumer Interaction Surface

**Started:** 2026-04-01
**Current phase:** BUILD (in progress — WP-A/B/C/D/E done + pre-WP-F MCP ingest fix, WP-F/G remaining)
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
| BUILD (WP-F) | ☐ Pending | — | MCP load_spec tool. Unblocked (hard dep C satisfied, MCP name-resolution fix unblocks the e2e acceptance path). |
| BUILD (WP-G.1) | ☐ Pending | — | evidence_trail + QueryFilter. Unblocked (open choice). |
| BUILD (WP-G.2) | ☐ Pending | — | RankBy::NormalizedWeight. Unblocked (open choice). |

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

BUILD in progress. WP-A/B/C/D/E complete + pre-WP-F MCP name-resolution fix. TS-5 reached: LLM consumers can exercise the pull paradigm (`changes_since`) and composable filters via MCP. Remaining: WP-F (MCP load_spec tool), WP-G.1 (evidence_trail filter), WP-G.2 (RankBy::NormalizedWeight). All unblocked. Remaining open decisions: RankBy::NormalizedWeight type-system fallout (WP-G.2).

WP-F is now genuinely unblocked — the MCP name-vs-UUID bug (signal 50) was closed in `0b9d9d3`. The first-real-MCP-consumer-workflow acceptance criterion can now pass end-to-end.
