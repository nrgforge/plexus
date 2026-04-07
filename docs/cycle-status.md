# Active RDD Cycle: MCP Consumer Interaction Surface

**Started:** 2026-04-01
**Current phase:** ARCHITECT (next)
**Artifact base:** ./docs/
**Scope:** Scoped cycle — MCP transport query surface + multi-consumer spec/lens interaction model

## Phase Status

| Phase | Status | Artifact | Key Epistemic Response |
|-------|--------|----------|----------------------|
| DECIDE (partial) | ⏸ Paused | ADR-036 draft, scenarios/036 draft, reflections/003 | User identified gap: query tools incomplete without consumer configuration model. Corrected tag bias in scenarios. Clarified provenance writes out of scope. |
| DISCOVER (update) | ✅ Complete | product-discovery.md (updated 2026-04-02) | User reframed spec loading as transport-independent API operation; consumer owns spec. Clarified durable/transient distinction. Established fail-fast validation as design constraint. Named the e2e acceptance criterion: first real MCP consumer workflow. |
| MODEL (amendment) | ✅ Complete | domain-model.md (amended 2026-04-02) | User confirmed amendments capture the right scope. Vocabulary layer named as concept. Three new invariants (60-62). |
| DECIDE (complete) | ✅ Complete | ADR-037, ADR-036 update, scenarios 036-037, interaction-specs, audits | Audits passed, fixes applied. CRUD-on-spec mental model accepted; concrete operation framing kept in ADR. Update path uses retract_contributions + load_spec composition. |
| ARCHITECT | ▶ Next | system-design.md (amendment) | — |
| BUILD | ☐ Pending | src/mcp/, src/api.rs, src/adapter/ | — |

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

The cycle started as "expose query tools via MCP" (decide+build). During decide, the user identified that the query surface is incomplete without the consumer configuration model. During model attempt, the user clarified that lenses encode persistent graph structure (not runtime configuration) and that new stakeholder jobs need product discovery before domain modeling. Product discovery is now updated with the multi-consumer interaction model, spec loading as transport-independent API operation, fail-fast validation, and the e2e acceptance criterion.

The revised sequence:
```
DISCOVER (update) ✅ → MODEL (amendment) → DECIDE (ADR-037 + ADR-036 update) → ARCHITECT → BUILD
```

ADR-037 (multi-consumer interaction model / load_spec) is the new ADR to be decided. ADR-036 (MCP query tools) needs updating after ADR-037 to include the `load_spec` MCP tool wrapper. The domain model needs: vocabulary layer as concept (or documented as emergent property), load_spec as action, and updated lens/spec lifecycle semantics.
