# Argument Audit Report

**Audited documents:** docs/decisions/033-lens-declaration.md, docs/decisions/034-composable-query-filters.md, docs/decisions/035-event-cursor-persistence.md
**Evidence layer:** docs/essays/001-query-surface-design.md, docs/essays/002-lens-storage-mechanism.md
**Prior ADRs:** docs/decisions/010-enrichment-trait-and-loop.md, docs/decisions/024-core-and-external-enrichment-architecture.md, docs/decisions/025-declarative-adapter-spec-extensions.md
**Constitutional authority:** docs/domain-model.md (invariants 37, 40, 41, 56–59)
**Product context:** docs/product-discovery.md
**Date:** 2026-03-26

---

## Summary

- **Argument chains mapped:** 11
- **Issues found:** 9 (2 P1, 4 P2, 3 P3)

### Chains mapped

1. Write-heavy/query-light pattern (Essay 001) → enrichment loop as write-time hook (ADR-010) → lens is an enrichment not a new axis → ADR-033
2. Lens output is public (Invariant 56) + lens is enrichment (Invariant 57) → first-class edges in shared graph → ADR-033
3. Many-to-one translation (ADR-033 design) → contributions map records lens ID → ADR-033 contribution tracking claim
4. Untranslatable connections stay raw (ADR-033) → scope-versus-serendipity resolution → product discovery value tension
5. Provenance-scoped filtering composable with any query primitive (Invariant 59) → QueryFilter struct with optional fields → ADR-034
6. Evidence diversity derived from contributions at query time (Essay 001, domain model) → Corroboration as RankBy dimension → ADR-034
7. Post-query filtering fails for traversal (ADR-034 justification) → filter applied during traversal, not after → ADR-034
8. StepQuery has per-step relationship filters (domain model, ADR-013) → QueryFilter as additional predicate per step → ADR-034
9. Pull paradigm requires persistent event log (product discovery) + library rule (Invariant 41) → event cursor in SQLite → ADR-035
10. Raw events vs. transformed events (ADR-035) → Invariant 37 scoping argument → ADR-035 consistency claim
11. Lens resolves domain translation need (ADR-035 justification for raw events) → cursor consumers read pre-translated lens edges → ADR-035

---

## Issues

### P1 — Must Fix

---

**Issue P1-A: ADR-035's Invariant 37 resolution is an interpretation, not a derivation — and the boundary is not operationally defined**

- **Location:** ADR-035, "Raw events, not transformed events" section
- **Claim:** "Invariant 37 ('outbound events flow through the adapter; consumers never see raw graph events') applies to the push paradigm. The pull paradigm via cursors is a separate interaction path where the consumer explicitly queries a change log."
- **Evidence gap:** This paragraph asserts a scope restriction on Invariant 37 that the invariant itself does not state. Invariant 37 reads: "Outbound events flow through the adapter. Consumers never see raw graph events." The word "never" is unqualified — it does not contain an exemption for pull-based query paths. The ADR invokes the distinction between push and pull as justification but does not establish that this distinction was part of Invariant 37's intent, nor does it propose an amendment to the invariant.

  The invariant exists in the domain model under "Public surface rules," which is the architectural section governing all consumer-facing interactions, not just push-based ones. The pull paradigm is also a public surface interaction. An ADR cannot narrow the scope of a "constitutional" invariant by assertion; it must either amend the invariant with a stated rationale or show that the invariant was already implicitly scoped.

  There is a legitimate argument available: Invariant 58 ("event cursors preserve the library rule for read workflows") was added to the domain model alongside Invariant 37 and explicitly contemplates pull-based event access. If the two invariants are read together, Invariant 58 can be understood as a structured exception to Invariant 37 for the cursor path. But ADR-035 does not make this argument — it simply asserts the scope restriction. The inference chain from "Invariant 58 exists" to "therefore Invariant 37 does not apply to cursors" is not drawn.

- **Recommendation:** Either (a) add an explicit domain model amendment to Invariant 37 scoping it to the push paradigm, with a note that Invariant 58 governs the pull path; or (b) restate the argument in ADR-035 as: "Invariants 37 and 58 together define the two-paradigm model — Invariant 37 governs push delivery, Invariant 58 governs pull delivery. This ADR operationalizes Invariant 58." This is a critical structural issue because if Invariant 37 is read as unqualified, raw events in the cursor log directly contradict it, and any downstream implementer reading the domain model in isolation will find a contradiction with the ADR.

---

**Issue P1-B: ADR-033's many-to-one translation creates a contribution tracking ambiguity that the ADR does not resolve**

- **Location:** ADR-033, "Relationship namespace convention" section and "Negative consequences" bullet
- **Claim:** "The enrichment ID (used in contributions) follows the same pattern: `lens:{consumer}:{to_relationship}`." For a many-to-one rule mapping `[may_be_related, similar_to]` → `thematic_connection`, both source relationships produce a contribution keyed `"lens:trellis:thematic_connection"`.
- **Evidence gap:** The contribution tracking model (Invariant 11, ADR-003) uses latest-value-replace per adapter ID. If a `may_be_related` edge and a `similar_to` edge between the same node pair both trigger the lens, both translation events would write to the same contribution slot `"lens:trellis:thematic_connection"` with the same key. The second write overwrites the first. This means a translated edge whose weight was established from two source edges will, after a context load-and-reload cycle, reflect only one source's weight — the one that was written last.

  The domain model defines contributions as `HashMap<AdapterId, f32>` where each key is one adapter ID. A lens with one `to` relationship has one contribution slot regardless of how many `from` relationships triggered it. The essay (Essay 002) notes that "contribution tracking records the lens as a distinct contributor" — but does not address the many-to-one case. This is a genuine information loss: the consumer cannot, from the translated edge alone, determine that two independent source relationship types agreed on the translation. That corroboration signal is lost in the contributions map.

  The ADR acknowledges that "if the same source edge matches multiple translation rules, the lens creates multiple translated edges" (a different but related problem), but does not address the case where multiple source relationship types map to the same translated edge and the same contribution key. The original source edges carry the full evidence trail, so provenance is not lost from the graph — but the translated edge's weight reflects only the most recent source, not their combination. This is a subtle but consequential correctness gap in contribution tracking for the primary use case of many-to-one translation.

- **Recommendation:** Add a section to ADR-033 addressing weight semantics for many-to-one translations. Options include: (a) use the maximum weight among all triggering source edges, (b) sum the weights (capped at 1.0 or per scale normalization), or (c) document explicitly that the translated edge weight reflects the most recent source edge and that consumers needing full corroboration on the translated edge should consult `evidence_trail` on the source edges. Option (c) is the most honest if the implementation does not aggregate — but it must be stated, not left implicit.

---

### P2 — Should Fix

---

**Issue P2-A: ADR-034's `min_weight` dual-presence creates an underdefined precedence rule**

- **Location:** ADR-034, "Filter semantics" section, `min_weight` bullet
- **Claim:** "The existing `min_weight` field on `TraverseQuery` is retained for backward compatibility but the filter's `min_weight` takes precedence when both are present."
- **Hidden assumption:** This rule assumes that the implementer will always apply the more restrictive constraint — but "filter takes precedence" actually means "filter's value overrides" regardless of which is stricter. If `TraverseQuery.min_weight = 0.5` and `QueryFilter.min_weight = 0.1`, the filter wins and weaker edges pass through. This may be the intended behavior (explicit filter signals intent), but it is not obviously the safer default and is not justified.

  More practically: having two fields that both mean "minimum edge weight" on the same query struct with a defined precedence is a recipe for bugs. A caller who sets `TraverseQuery.min_weight = 0.5` expecting it to filter weak edges will be surprised when a `QueryFilter` with `min_weight: 0.1` overrides it silently. The precedence rule is architecturally incoherent — if `QueryFilter` takes precedence, `TraverseQuery.min_weight` is not backward-compatible in any meaningful sense, because existing callers who compose it with a `QueryFilter` get different behavior than they had.

- **Recommendation:** Remove `min_weight` from `QueryFilter` and keep it on `TraverseQuery` (and other individual query structs) as a structural-filter concern. Let `QueryFilter` own only the provenance-dimensional and corroboration filters that are genuinely new capabilities. If the goal is a unified filter struct, that is a separate migration decision that should be explicit — not a side effect of adding `QueryFilter`. The current design introduces ambiguity without eliminating redundancy.

---

**Issue P2-B: ADR-034 does not address how `QueryFilter` interacts with `StepQuery`'s per-step relationship filters**

- **Location:** ADR-034, "Neutral consequences" section, final bullet
- **Claim:** "`StepQuery` already specifies relationship per-step. The `QueryFilter` applies as an additional edge predicate within each step — it does not override the step's relationship filter."
- **Hidden assumption:** This single sentence is the entirety of the treatment of `StepQuery` interaction. Essay 001 identifies step-based traversal as a core query shape; `StepQuery` is the structured traversal primitive with per-step relationship type and directionality constraints. The ADR states the composition rule but does not address an important edge case: if `QueryFilter.relationship_prefix` is set (e.g., `"lens:trellis:"`), and a given step in the `StepQuery` specifies `relationship: "tagged_with"`, the step filter requires `tagged_with` while the global filter requires `lens:trellis:` prefix. No edge satisfies both predicates simultaneously. The result would be an empty traversal at that step, silently short-circuiting the query.

  This is not necessarily wrong — it may be the intended behavior (a lens-scoped `StepQuery` that specifies non-lens relationships is a consumer error). But the ADR does not acknowledge this failure mode or document the expected result. A consumer composing a `StepQuery` with a `relationship_prefix` QueryFilter needs to understand that every step's relationship must match the prefix, or the traversal terminates early.

- **Recommendation:** Add a paragraph in the Consequences section explicitly documenting the composition rule for `StepQuery` + `relationship_prefix`: when both are set, edges must satisfy both predicates; a step that specifies a relationship type not matching the prefix will find zero edges and the traversal terminates at that step. Note whether this is the intended behavior or whether per-step filters should take priority over global `relationship_prefix`.

---

**Issue P2-C: ADR-035's lens-resolves-translation-need argument is overstated as the justification for raw events**

- **Location:** ADR-035, "Raw events, not transformed events" section, reason #1
- **Claim:** "The lens resolves the domain translation need. With the lens-as-enrichment (ADR-033), cursor events for `EdgesAdded` with relationship `lens:trellis:thematic_connection` are already domain-meaningful."
- **Hidden assumption:** This claim holds for consumers who have defined a lens that covers all relationships they care about. It is false for consumers who have not defined a lens (the majority of consumers at the time of initial deployment — lens is optional and "transparent to consumers who do not need it," per ADR-033). A consumer using the cursor who has not defined a lens will receive raw `EdgesAdded` events with relationships like `may_be_related`, `similar_to`, `co_exhibited` — relationships that are not domain-meaningful in the consumer's vocabulary.

  The implication "lens resolves the translation need, therefore raw events are appropriate for cursors" is valid only if all cursor consumers have lenses covering their domain. This is an unacknowledged scope restriction. The argument conflates "the lens mechanism exists" with "the lens mechanism is always used," which is a scope accuracy error. The second reason (library rule compatibility) is independently sufficient and does not require this overstated claim.

- **Recommendation:** Restate reason #1 as: "Consumers who define a lens receive pre-translated relationships in the cursor event log — `lens:{consumer}:{relationship}` edges are domain-meaningful without further transformation. Consumers without a lens receive raw relationship types, which they may interpret through their adapter's domain knowledge or by consulting `evidence_trail`." This acknowledges that the lens is not a universal solution but does not undermine the architectural decision.

---

**Issue P2-D: ADR-033's untranslatable connections resolution is not fully derived from the essay's scope-versus-serendipity analysis**

- **Location:** ADR-033, "Untranslatable connections (OQ-21)" section
- **Claim:** "Translation rules are opt-in, not exhaustive. Connections that do not match any `from` + `involving` pattern remain in the graph as standard edges — accessible via traversal without a `lens:` prefix filter."
- **Hidden assumption:** Essay 001's scope-versus-serendipity section explicitly identifies "discovery mode" as a first-class concern: "A consumer must be able to step outside its projection to discover connections that bridge domains or contexts." The essay frames this as "the query surface's most important design problem" and proposes a two-mode model (scoped vs. discovery). ADR-033 resolves untranslatable connections by simply leaving them in the graph as raw edges. This is correct as a storage decision but does not address the discovery mode question.

  The essay's argument was: untranslatable connections need a query mechanism to surface them — not just a guarantee that they exist. ADR-033 addresses existence but does not address surfacing. The question "how does a consumer discover untranslated cross-domain edges they don't know to look for?" is left open. The ADR points to ADR-034 for the query side, but ADR-034 does not add a "discovery mode" that expands beyond a consumer's lens. Both ADRs address filtering and ranking within known relationship types, not cross-boundary discovery of unknown relationship types.

  This is not a logical error in ADR-033 (leaving raw edges in the graph is correct), but the claim that this "resolves the scope-versus-serendipity tension" (from product discovery) is an overstatement. The tension is partially resolved — connections are not hidden — but the discovery mechanism for finding them is deferred without acknowledgment.

- **Recommendation:** Narrow the claim: "This resolves the storage aspect of the scope-versus-serendipity tension — no connections are hidden. The discovery mechanism (how consumers find untranslated cross-domain edges proactively) is not addressed by this ADR and remains as an open question for a future query surface ADR."

---

### P3 — Consider

---

**Issue P3-A: ADR-034's corroboration ranking is separated from traversal order, but the separation rationale is incomplete**

- **Location:** ADR-034, "Corroboration as a ranking dimension" section
- **Claim:** "`RankBy` is an optional parameter on query result types, not on the query structs themselves. This keeps the query execution path simple — queries return unranked results, and ranking is applied as a post-processing step."
- **Clarification opportunity:** The ADR notes that "ranking does not affect traversal order (which is BFS by depth), only the final result ordering within each depth level or result set." This is accurate, but it creates a subtle interaction worth noting: for `traverse` queries, the consumer receives all results ranked by corroboration within depth levels — but the traversal itself was BFS, meaning the set of nodes reached is determined by BFS order, not by corroboration. A highly corroborated node at depth 5 may not be reached if the `max_depth` is 3. The ranking is applied to whatever BFS found, not to the globally most corroborated reachable nodes. This is standard BFS behavior and is probably intended, but consumers expecting "find the most corroborated connections" may not understand that `max_depth` gates which connections are eligible to be ranked.

- **Recommendation:** Add a note in the Consequences section: "Corroboration ranking applies to the result set produced by the query — it does not reorder traversal to prioritize highly corroborated paths. For `traverse` queries, `max_depth` limits which nodes are eligible for ranking. Consumers who want globally top-corroborated connections regardless of depth should use `find_nodes` with `min_corroboration` rather than `traverse` with `RankBy::Corroboration`."

---

**Issue P3-B: ADR-035's stale cursor recovery path creates an implicit assumption about context size**

- **Location:** ADR-035, "Retention policy" section and "Negative consequences" section
- **Claim:** "A consumer that falls behind the retention window (its cursor points to a pruned sequence) receives an error indicating the cursor is stale. The consumer's recovery path: reload the full context via `load_context()` and reset its cursor to the latest sequence."
- **Clarification opportunity:** The ADR notes that "for large contexts, this is expensive" and defers cursor compaction. What is not addressed: the stale cursor error is a binary signal — the consumer learns their cursor is invalid, but does not learn how much state they missed. A consumer that missed 100 events has the same recovery path as one that missed 10,000. The recovery is always a full reload, regardless of the actual delta. For contexts with high write throughput and short retention windows, stale cursors may be common and the full-reload recovery path may be disproportionately expensive relative to the data the consumer actually missed.

  This is noted as a known limitation in the Negative consequences, but the domain model definition of `event cursor` describes pull-without-persistent-connection as the key property — the stale cursor recovery pattern introduces a case where the consumer must make a synchronous, potentially expensive call before they can resume pull-based operation. This is worth flagging to implementers.

- **Recommendation:** Consider adding the stale cursor error code to the `ChangeSet` type definition (rather than a separate error path), so callers always handle it at the same call site. Also note explicitly that the stale cursor recovery is the only case where pull-based operation requires a full context materialization — it is not "just a table read."

---

**Issue P3-C: ADR-033 and ADR-034 use "relationship prefix" with subtly different semantics**

- **Location:** ADR-033, "Relationship namespace convention" section; ADR-034, "Filter semantics" section, `relationship_prefix` bullet
- **Claim:** ADR-033 establishes `lens:{consumer}:{relationship}` as the edge relationship type. ADR-034 describes `relationship_prefix: "lens:trellis:"` to scope results to one consumer's translated view.
- **Terminology note:** ADR-034 states that `relationship_prefix` "catches all of a consumer's lens translations without enumerating each relationship type." This works for the simple case: `"lens:trellis:"` prefix matches `"lens:trellis:thematic_connection"`, `"lens:trellis:topic_link"`, etc. However, ADR-033 also defines `lens:{consumer}:{to_relationship}` as the contribution key (the enrichment ID), not just the relationship type. ADR-034's `contributor_ids` filter uses contribution keys. The two filter mechanisms — `relationship_prefix` and `contributor_ids: ["lens:trellis:thematic_connection"]` — overlap in what they can achieve for lens-scoped queries, but via different graph attributes.

  The risk is that consumers conflate the two: a consumer who wants "all of Trellis's lens output" should use `relationship_prefix: "lens:trellis:"` (matching the `relationship` field). A consumer who wants "only connections where the lens was the specific contributor" should use `contributor_ids: ["lens:trellis:thematic_connection"]` (matching the contributions map). These are related but not identical: an edge could have `relationship = "lens:trellis:thematic_connection"` but also have contributions from other adapters. The distinction between "this edge has a lens-prefixed relationship type" and "the lens is the contributor" may matter for corroboration reasoning.

- **Recommendation:** Add a brief disambiguation note in ADR-034's filter semantics section: "`relationship_prefix` matches on the edge's relationship type field; `contributor_ids` matches on the edge's contributions map. For lens-scoped queries, `relationship_prefix` returns all edges of a given lens's relationship types regardless of which other adapters contributed; `contributor_ids` returns all edges where the lens was a contributor regardless of relationship type. These are equivalent for edges the lens created alone, but diverge for edges strengthened by multiple contributors."

---

## Cross-ADR Consistency Check

### ADR-033 and ADR-034

The two ADRs are consistent in their primary claims. ADR-034's `QueryFilter` is designed to compose with lens output naturally (via `relationship_prefix`), and ADR-033's namespace convention is specifically designed to enable that filtering. No contradiction found.

The many-to-one translation weight ambiguity (P1-B) in ADR-033 does have a downstream effect on ADR-034: if the translated edge's weight does not accurately reflect the combined evidence from multiple source relationships, `min_weight` and `RankBy::Corroboration` filters in ADR-034 will rank lens-translated edges on incomplete data. This is a dependency: P1-B should be resolved before the ranking behavior in ADR-034 is implemented against many-to-one translations.

### ADR-033 and ADR-035

Consistent. ADR-035 explicitly cites ADR-033 as the reason raw events are sufficient (the lens pre-translates). The P2-C issue notes this argument is overstated, but the two ADRs do not contradict each other. Lens-created `EdgesAdded` events in the cursor log are identified by `lens:{consumer}:{relationship}` in both their relationship type and their adapter_id field, which is how `CursorFilter.adapter_id` scoping works.

### ADR-034 and ADR-035

Consistent and complementary. ADR-034 adds query-time filtering; ADR-035 adds pull-based event delivery. A consumer can use both: cursor to learn what changed, then `QueryFilter` to query the current graph with provenance scoping. No contradiction found.

### All three against domain model invariants 56–59

- **Invariant 56** (lens output is public): Satisfied by ADR-033's first-class edge mechanism.
- **Invariant 57** (lens is an enrichment): Satisfied by ADR-033's `LensEnrichment` implementing the `Enrichment` trait.
- **Invariant 58** (event cursors preserve library rule): Satisfied by ADR-035's SQLite-backed event log.
- **Invariant 59** (provenance-scoped filtering composable with all query primitives): Satisfied by ADR-034's `QueryFilter`. No contradiction found here.
- **Invariant 37** (outbound events through adapter): The contradiction risk is documented as P1-A. The ADRs are not definitively inconsistent with Invariant 37, but the boundary is unresolved and requires an explicit amendment or clarifying argument to close the gap.
- **Invariant 40** (three extension axes): Satisfied. ADR-033 explicitly preserves this via the lens-as-enrichment model.
- **Invariant 41** (library rule): Satisfied by ADR-035's SQLite-only event persistence.
