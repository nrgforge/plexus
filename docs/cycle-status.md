# Active RDD Cycle: MCP Consumer Interaction Surface

**Started:** 2026-04-01
**Current phase:** BUILD (next)
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
| BUILD | ▶ Next | src/mcp/, src/api.rs, src/adapter/, src/storage/, src/graph/ | — |

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

BUILD is the next phase. Build order from roadmap: A (ship independently) → B (infrastructure) → C (spec API) → D (rehydration) → E, G.1, G.2 in parallel or any order → F last. Open decisions deferred to BUILD: interior mutability mechanism, load_spec rollback strategy, validation extent, rehydration error handling, RankBy::NormalizedWeight type-system fallout.
