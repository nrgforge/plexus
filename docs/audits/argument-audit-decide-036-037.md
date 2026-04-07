# Argument Audit Report

**Audited documents:**
- `/Users/nathangreen/Development/plexus/docs/decisions/037-consumer-spec-loading.md`
- `/Users/nathangreen/Development/plexus/docs/decisions/036-mcp-query-surface.md`

**Evidence trail:**
- `docs/essays/001-query-surface-design.md`
- `docs/essays/reflections/003-multi-consumer-lens-interaction.md`
- `docs/product-discovery.md` (updated 2026-04-02)
- `docs/domain-model.md` (Invariants 56–62, Amendment 7)

**Prior ADRs:**
- ADR-033 (lens declaration), ADR-034 (composable query filters), ADR-035 (event cursor persistence)
- ADR-014 (transport-independent API), ADR-028 (universal MCP ingest)

**Date:** 2026-04-02

---

## Summary

- **Argument chains mapped:** 14
- **Issues found:** 9 (2 P1, 4 P2, 3 P3)

---

## Argument Chains Mapped

### ADR-037 chains

**Chain 1.** Reflection 003 identifies that no runtime spec loading mechanism exists (gap: pipeline built at construction time, no API for post-startup registration) + product discovery job "Load my spec onto a context" + domain model Amendment 7 (Invariant 60–62) → Decision: add `load_spec` to `PlexusApi`. Chain is sound.

**Chain 2.** Invariant 62 (vocabulary layers are durable graph data; lens enrichments are durably registered on the context) + library rule (Invariant 41: Plexus never requires a persistent runtime) → Decision: persist spec YAML in SQLite `specs` table; re-register on startup. Chain is sound, but see Issue 1.

**Chain 3.** Invariant 60 (fail-fast: if validation fails, nothing happens) → Decision: validate before wiring, before enrichment, before graph work. Chain is sound.

**Chain 4.** Product discovery three-effect model (durable graph data / durable enrichment registration / transient adapter wiring) → ADR three-effect model. Direct one-to-one correspondence. Sound.

**Chain 5.** Reflection 003 identifies `register_specs_from_dir` does not wire lens enrichments (known gap) → Decision: fix `register_specs_from_dir` to call equivalent of `register_integration`. Chain is sound.

**Chain 6.** Product discovery (unload is the inverse of load for transient + registration effects; graph data is permanent per Invariant 62) → `unload_spec` removes routing and registration but not graph edges. Sound.

### ADR-036 chains

**Chain 7.** Essay 001 identifies pull paradigm as fundamental + product discovery "CRON job checking 'what new connections emerged since yesterday'" + Invariant 38 (thin shell) → expose `changes_since`, `find_nodes`, `traverse`, `find_path` via MCP. Sound.

**Chain 8.** Reflection 003 (consumer cannot declare identity via MCP; no `load_spec` equivalent) → include `load_spec` as MCP tool. Sound; correctly depends on ADR-037.

**Chain 9.** ADR-034 `QueryFilter` fields are a Rust struct with nested semantics → flat optional fields on MCP tool parameters. Premise: "LLM consumers construct flat JSON more reliably than nested structures." This premise is asserted without citation. See Issue 7.

**Chain 10.** Essay 001 identifies `StepQuery` as a building block for programmatic consumers, not an end-user primitive → exclude `StepQuery` from MCP surface. Chain is defensible. See Issue 9 (P3).

**Chain 11.** ADR-035 defines cursor state as sequence numbers → MCP `changes_since` is client-managed cursor, no server-side state. Sound; consistent with Invariant 41.

**Chain 12.** Invariant 59 (provenance-scoped filtering composable with any query primitive, including `evidence_trail`) → add `QueryFilter` parameter to `evidence_trail`. Formally sound against the invariant, but see Issue 4.

**Chain 13.** ADR-036 claims total tool count of 16 "within usable range for LLM tool selection." See Issue 8 (P3).

**Chain 14.** `load_spec` is described as "the only tool that is not a thin wrapper" (involves validation, persistence, enrichment execution). This is consistent with Invariant 38 — the ADR correctly identifies this as an exception and explains why (spec loading is genuinely a complex API operation, not just transport wiring). Sound.

---

## Issues

### P1 — Must Fix

---

**Issue 1 — Schema defect: `specs` table PRIMARY KEY prevents multi-context spec loading**

- **Location:** ADR-037, Section 2 (Persist specs in SQLite)
- **Claim:** The `specs` table schema uses `adapter_id TEXT NOT NULL PRIMARY KEY`. The document also states "Keyed by `adapter_id` (unique per spec — the adapter ID is the spec's identity). One row per loaded spec per context."
- **Evidence gap:** The out-of-scope section explicitly states "Loading the same spec onto multiple contexts requires separate `load_spec` calls." If the same spec (same `adapter_id`) is loaded onto two different contexts, a second INSERT with the same `adapter_id` will violate the PRIMARY KEY constraint. The schema and the stated multi-context behavior are logically incompatible.

  The correct key is `(context_id, adapter_id)` as a composite primary key — one row per spec per context, as the prose says. The current schema allows only one row per `adapter_id` globally.

- **Recommendation:** Change the schema to:
  ```sql
  CREATE TABLE IF NOT EXISTS specs (
      context_id TEXT NOT NULL,
      adapter_id TEXT NOT NULL,
      spec_yaml TEXT NOT NULL,
      loaded_at TEXT NOT NULL,
      PRIMARY KEY (context_id, adapter_id)
  );
  ```
  This matches the stated semantics ("one row per loaded spec per context") and permits the same spec to be active on multiple contexts simultaneously.

---

**Issue 2 — Startup consequence claim contradicts the three-effect model**

- **Location:** ADR-037, Section "Consequences — Negative" (third bullet)
- **Claim:** "Startup time increases with number of persisted specs (re-instantiation + initial lens run)"
- **Evidence gap:** The three-effect model (Section 3) defines effect (a) as "the lens enrichment runs immediately against existing content and writes vocabulary edges" at `load_spec` time. Effect (a) is described as "durable graph data" — meaning the vocabulary layer edges already exist in the graph after the initial `load_spec` call. They do not need to be re-created on startup.

  Section 2 says startup re-registers adapters and enrichments (effect b) but does not say the lens re-runs on startup. Running the lens again on startup would be wasteful, and the idempotency guard in `LensEnrichment` (ADR-033) would prevent duplicate edges — but this means the startup cost is re-instantiation only, not a new lens run.

  The Consequences claim "re-instantiation + initial lens run" is inaccurate: the lens should not run again on startup because the vocabulary edges already persist as graph data (that is the explicit purpose of effect a). If the intent is that the lens does run on startup as a correctness measure, this contradicts the three-effect model and should be explicitly justified and documented as a design decision (with an explanation of how idempotency is guaranteed).

- **Recommendation:** Decide definitively: does the lens run on startup or only re-register? If re-register only (the logically consistent choice given the three-effect model), change the consequence bullet to "re-instantiation only." If the lens does run on startup, add a section explaining why, confirm idempotency is guaranteed, and update the three-effect model to reflect that effect (a) can also occur at startup.

---

### P2 — Should Fix

---

**Issue 3 — Hidden assumption: `unload_spec` semantics for future emissions are not made explicit**

- **Location:** ADR-037, Section 6 (Unload spec)
- **Claim:** "Removes the adapter from routing and the spec from the `specs` table. Does not remove vocabulary edges from the graph — they are durable graph data (Invariant 62). The vocabulary layer remains queryable; the lens enrichment stops reacting to new emissions; the adapter stops accepting ingest calls."
- **Evidence gap:** The ADR states the lens enrichment stops reacting to new emissions after unload. This creates an asymmetry that is not named or acknowledged: after unloading, Consumer A's existing vocabulary layer persists and is queryable by everyone (Invariant 56: lens output is public) — but future emissions from Consumer B will no longer trigger Consumer A's translation. A consumer querying `lens:trellis:*` after Consumer B injects new content post-unload will see a partial vocabulary layer: old translated edges but no new ones.

  This is a legitimate design choice (unload stops the reactive translation), but it is a consequence that affects other consumers in a multi-consumer context. Product discovery states "the vocabulary layer and the reactive lens survive even if I disconnect" in the consumer mental model — yet `unload_spec` stops the reactive lens. The mental model in product discovery appears to apply to disconnection without unloading; `unload_spec` is an explicit deregistration. But the ADR does not make this distinction, and a consumer reading the mental model might assume their lens keeps translating indefinitely even if they call `unload_spec`.

- **Recommendation:** Add a note in Section 6 making the asymmetry explicit: "After unload, the vocabulary layer (edges already written) persists as durable graph data and is queryable by all consumers. However, the lens enrichment is deregistered — it no longer reacts to new emissions from any consumer. A consumer that wants to maintain reactive translation indefinitely should not call `unload_spec` — the spec will be re-registered from the `specs` table on every startup." This aligns the ADR with the consumer mental model in product discovery.

---

**Issue 4 — Practical utility of `QueryFilter` on `evidence_trail` is undermined by provenance relationship types**

- **Location:** ADR-036, Section 5 (Existing `evidence_trail` gains filter parameter)
- **Claim:** "Invariant 59 requires provenance-scoped filtering composable with any query primitive, including `evidence_trail`. The existing `PlexusApi::evidence_trail` method and its MCP tool gain an optional `QueryFilter` parameter, piped through to the underlying `StepQuery`."
- **Evidence gap:** ADR-034 explicitly documents the interaction between `QueryFilter` and `StepQuery` step relationships: "a step specifies `relationship: 'tagged_with'` and the filter specifies `relationship_prefix: 'lens:trellis:'` — no edge satisfies both predicates and the traversal terminates at that step." The `evidence_trail` operation traverses provenance-dimension edges (`contains`, `references`, `links_to`) and semantic edges — none of which use `lens:` namespace prefixes. Applying `relationship_prefix: "lens:trellis:"` to `evidence_trail` would yield empty results, not scoped results.

  The `contributor_ids` and `min_corroboration` filter fields have more plausible utility on `evidence_trail` (e.g., "show me only the provenance trail for edges contributed by adapter X"). But the ADR does not explain which filter fields are meaningful for `evidence_trail` versus which will produce vacuous results. Describing the filter as simply "piped through" understates the risk that consumers will use it incorrectly.

- **Recommendation:** Add a note in Section 5 scoping which `QueryFilter` fields are useful for `evidence_trail`: `contributor_ids` and `min_corroboration` compose meaningfully; `relationship_prefix` does not compose with the provenance traversal pattern and will typically produce empty results. This is not an argument against adding the parameter (Invariant 59 requires it), but the semantic mismatch should be documented so consumers understand the constraint.

---

**Issue 5 — OQ-25 remains open but ADR-037 contains the answer implicitly; neither ADR acknowledges it**

- **Location:** Reflection 003, OQ-25 ("If a new lens is registered and enriches existing edges, do those enrichment events appear in the cursor stream?"); ADR-037, Section 1 (step 6: run lens enrichment against existing content)
- **Claim:** Reflection 003 marks OQ-25 as proposed/open. ADR-037 step 6 says the lens enrichment "run[s] against existing graph content to produce the initial vocabulary layer." Since these writes go through the engine's emit path, they will produce `EdgesAdded` graph events that land in the event log (ADR-035 persistence boundary: "Events are persisted within `emit_inner()`, after commit succeeds").
- **Evidence gap:** Neither ADR-036 nor ADR-037 explicitly resolves OQ-25. A consumer using `changes_since` immediately after another consumer's `load_spec` will see the vocabulary edges created by the incoming lens in the cursor stream — but this behavior is implicit, not stated. Consumers need to know this because it changes how they interpret cursor events: vocabulary layer creation (bulk `EdgesAdded` from a lens initial run) is a distinct event pattern from incremental graph mutations.

  Additionally, domain model Amendment 7 lists OQ-25 as "resolved" — but no resolution text is provided in the amendment entry.

- **Recommendation:** In ADR-037, add a brief note in Consequences (or Section 1) confirming that the initial lens run produces events that appear in the cursor stream, and what the event pattern looks like (bulk `EdgesAdded` with `lens:{consumer}:*` relationship types). Update Amendment 7 in the domain model to include the resolution text for OQ-25 rather than leaving it blank.

---

**Issue 6 — `register_specs_from_dir` fix is scoped but the scope creates a behavioral inconsistency between file-based and programmatic loading**

- **Location:** ADR-037, Section 4 (Fix `register_specs_from_dir`) and Section 3 (Three-effect model)
- **Claim:** "The file-based path and the programmatic `load_spec` path converge on the same internal wiring logic." Both validate, parse, extract, and register. The difference stated: "file-based discovery happens at build time; `load_spec` happens at runtime and additionally persists the spec."
- **Evidence gap:** The three-effect model says `load_spec` runs the lens enrichment against existing graph content immediately (effect a). The fixed `register_specs_from_dir` path — which wires the complete spec — presumably also needs to run the lens against existing graph content on startup. But the ADR does not state that the file-based discovery path triggers the initial lens run. If `register_specs_from_dir` wires the lens enrichment (re-registration effect b) but does not run it against existing graph content (effect a), then a server restart with file-based specs leaves the vocabulary layer unpopulated until the next emission.

  This may be intentional (the file-based path is for new deployments where no content exists yet), but in a context where `load_spec` was previously called and the spec is now being re-discovered from disk on restart, the vocabulary layer edges already exist in the graph (effect a happened during the original `load_spec`). So the asymmetry only matters for the case where `register_specs_from_dir` is the *first* time a spec is loaded — which is precisely the case for file-based specs, not runtime-loaded specs.

  The distinction between "initial load" (effect a applies) and "startup re-registration" (effect b only, because effect a already happened) is important and should be made explicit for both paths.

- **Recommendation:** Add a paragraph distinguishing the two scenarios: (1) initial spec loading via `load_spec` or first-time `register_specs_from_dir` — runs the full three effects including initial lens run; (2) startup re-registration from the `specs` table — re-registers enrichments (effect b) only, because vocabulary edges already persist in the graph. This also resolves Issue 2 as a side effect.

---

### P3 — Consider

---

**Issue 7 — Flat field serialization rationale is asserted, not evidenced**

- **Location:** ADR-036, Section 2 ("Flat fields over nested objects — LLM consumers construct flat JSON more reliably than nested structures.")
- **Claim:** Flat parameter serialization improves LLM consumer reliability over nested JSON objects.
- **Evidence gap:** This is a reasonable engineering judgment, but it is presented as fact without citation. Essay 001 does not address MCP serialization strategy at all — the serialization decision is original to ADR-036. There is no reference to empirical research, MCP specification guidance, or precedent from other MCP tools. The claim may be true, but it is an assumption the document should acknowledge rather than state as established fact.
- **Recommendation:** Soften the claim: "Flat fields are preferred over nested objects on the assumption that LLM consumers construct flat JSON more reliably — a convention followed by existing MCP tools in this project." If there is external evidence (MCP docs, benchmark results), cite it.

---

**Issue 8 — "16 tools within usable range" claim lacks a defined upper bound**

- **Location:** ADR-036, Section "Consequences — Positive" ("16 tools within usable range for LLM tool selection") and "Consequences — Negative" ("16 tools approaches upper bound for LLM tool selection reliability")
- **Claim:** 16 is "within usable range" but "approaches upper bound."
- **Evidence gap:** The positive and negative consequence bullets make opposing claims in adjacent sentences without establishing what the upper bound actually is. The Negative bullet says "further tiers may need tool grouping or selection hints" — this is the honest engineering hedge — but the Positive bullet frames 16 as clearly within range, implying a known threshold exists. No source is cited. ADR-014's context references "Less is More" from Essay 14, but that essay is not in the evidence trail for this ADR and doesn't establish a numeric threshold.
- **Recommendation:** Either cite the source for the upper bound (e.g., a specific MCP capability study, or an observation from Essay 14 that established the original 9-tool count), or reframe both bullets to acknowledge uncertainty: "16 tools is manageable for current LLM tool selection, though the threshold for reliability degradation is empirically uncertain and warrants monitoring."

---

**Issue 9 — `StepQuery` exclusion rationale inconsistency: `evidence_trail` counterexample is not addressed**

- **Location:** ADR-036, Section 3 (StepQuery kept internal)
- **Claim:** "`StepQuery` is not exposed via MCP. It is a building block for programmatic Rust consumers. LLM consumers needing multi-hop queries compose `traverse` calls."
- **Evidence gap:** `evidence_trail` (already exposed, Section 5) is implemented using `StepQuery` internally. The ADR acknowledges this implicitly in Section 5 when it says the `QueryFilter` is "piped through to the underlying `StepQuery`." The exclusion rationale ("building block for programmatic consumers") is partially contradicted by the fact that `evidence_trail` exposes exactly this building block for a specific use case. The distinction being drawn is that `evidence_trail` is a purpose-built composite operation, while `StepQuery` is raw multi-hop traversal — this is a defensible line to draw, but it is not stated explicitly.
- **Recommendation:** In Section 3, add one sentence acknowledging the relationship: "`evidence_trail` (Section 5) wraps a `StepQuery` internally — the distinction is that `evidence_trail` is a purpose-built composite query for provenance tracing, while raw `StepQuery` exposes arbitrary step sequences that require programmatic construction. LLM consumers use `evidence_trail` for this purpose."

---

## Cross-ADR Observations

**Dependency ordering is correct.** ADR-036 depends on ADR-037 for the `load_spec` tool. The domain model Amendment 7 correctly records both ADRs as needed. The ordering is consistent with the build sequence.

**Invariant coverage is thorough.** Both ADRs correctly cite and apply Invariants 38, 40, 56–62. No invariant is misquoted or applied in a context that contradicts its scope.

**OQ resolution status.** Reflection 003 proposes OQ-24, OQ-25, OQ-26. ADR-037 resolves OQ-24 (persistence is per-spec in SQLite, survives server restart). OQ-25 is implicitly resolved by ADR-037's three-effect model but not explicitly acknowledged (Issue 5 above). OQ-26 (should vocabulary layering be named in the domain model?) is resolved by Amendment 7, which adds the `vocabulary layer` concept — but neither ADR references this resolution, which is a minor documentation gap rather than a logical issue.

**Terminology is consistent.** Both ADRs use the domain model vocabulary correctly throughout: "emission," "enrichment," "lens," "vocabulary layer," "spec," "contributor." No synonym drift detected.
