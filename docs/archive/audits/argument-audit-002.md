# Argument Audit Report

**Audited document:** `/Users/nathangreen/Development/plexus/docs/essays/002-lens-storage-mechanism.md`
**Evidence trail:** `/Users/nathangreen/Development/plexus/docs/essays/research-logs/research-log.md`
**Date:** 2026-03-25

---

## Summary

- **Argument chains mapped:** 6
- **Issues found:** 6 (0 P1, 3 P2, 3 P3)

### Chains mapped

1. Storage uniformity → Option B costs exceed benefits → Option B rejected
2. In-memory query model → lens edges automatically queryable → Option A "zero query module changes"
3. Enrichment trait sufficiency → zero infrastructure changes → Option A recommended
4. MV4PG pattern → write-time materialization validated → Option A consistent with established systems
5. TinkerPop virtual approaches → full traversal cost at query time → Option C rejected
6. Graphiti write-heavy/query-light pattern → lens-as-enrichment consistent with prior art → Option A reinforced

All six chains are internally sound. The major inferential steps follow from the evidence. The core argument — that a lens is architecturally just an enrichment and should produce first-class edges — is well-supported by codebase evidence and external validation.

---

## Issues

### P1 — Must Fix

None.

---

### P2 — Should Fix

**P2-1: Enrichment loop timing described imprecisely**

- **Location:** "What the Codebase Reveals / The Enrichment Loop Is the Write-Time Hook"
- **Claim:** "Every enrichment receives every round's events and a cloned context snapshot."
- **Evidence gap:** This is accurate for what happens within a single loop run, but the surrounding sentence frames the enrichment loop as running reactively "after every adapter emission." The actual architecture (post-ADR-029) is that the enrichment loop runs once per `ingest()` call, after all adapters matching `input_kind` have processed and their events have been combined. The loop receives the combined events from all adapters in that ingest call, not a per-adapter trigger. The code in `ingest.rs` (Step 3 comment: "Enrichment loop runs once with combined events") is explicit about this. A lens-enrichment's timing depends on this distinction: it fires once per ingest call on all events from that round, not once per adapter emission.
- **Recommendation:** Replace "after every adapter emission" with "once per ingest call, after all adapter emissions in that call have been combined." This does not change the conclusion — Option A is still correct — but it accurately characterizes when a lens-enrichment fires, which matters for consumers reasoning about real-time responsiveness.

**P2-2: Invariant 59 claimed to "work without modification" when it is not yet implemented**

- **Location:** "Evaluating the Three Options / Option A: First-Class Edges"
- **Claim:** "Contribution tracking records the lens as a distinct contributor. Evidence diversity queries (Invariant 59) automatically count lens contributions alongside adapter and enrichment contributions."
- **Evidence gap:** Invariant 59 states that provenance-scoped filtering must be composable with any query primitive. OQ-23 (domain model, open questions section) documents that this is not yet implemented: "The current `FindQuery`, `TraverseQuery`, and `FindPathQuery` have no provenance filter parameters." The essay's claim that evidence diversity queries "automatically count lens contributions" implies this infrastructure is operational. It is partially true — lens contributions accumulate in the `contributions` map on edges, so the raw data exists — but the query-layer interface to filter or count by contribution source does not yet exist. The claim overstates the current state.
- **Recommendation:** Distinguish between what the data model supports (lens contributions are stored in the standard contributions map, so the data for Invariant 59 queries will be present) and what is currently implemented (the query primitives that surface this data do not yet accept provenance filter parameters). A sentence noting that "the lens contributions will be present in the data model, but query-layer provenance filtering (OQ-23) remains unimplemented" accurately reflects the state.

**P2-3: "Edge deduplication is a linear scan" understates the full deduplication cost**

- **Location:** "Evaluating the Three Options / Option A: First-Class Edges"
- **Claim:** "Edge deduplication in `Context.add_edge()` uses linear scan (`find_edge_exact`). More edges → slower dedup per emission."
- **Evidence gap:** The claim is accurate as stated, but incomplete. `add_edge()` in `context.rs` performs two scans: one for an exact match (`find_edge_exact` — a single `iter().position()`) and, if no exact match exists, a second scan for cross-dimensional matches (a second `iter().enumerate().filter()`). The cross-dimensional scan runs on every new edge that does not exactly duplicate an existing one. A lens adding many new edges (which will not exactly match existing edges, since they have new relationship types) will trigger both scans per emission. Framing this as a single linear scan understates the constant factor. The conclusion — that this is a pre-existing O(n) concern, not lens-specific — remains correct.
- **Recommendation:** Note that `add_edge()` performs two linear scans (exact match, then cross-dimensional match) for each new edge. The conclusion about O(n) scaling is unchanged, but the constant factor is higher than "linear scan" implies. This matters for consumers evaluating performance with many active lenses.

---

### P3 — Consider

**P3-1: The Option C structural synthesis limitation needs a more precise boundary**

- **Location:** "Evaluating the Three Options / Option C: Query-Time Translation"
- **Claim:** "Cannot create new structural relationships. A lens that discovers 'fragment X and research paper Y are thematically related' needs to produce a new edge connecting X and Y. A query-time filter can only reweight or hide existing edges — it cannot synthesize new connections."
- **Evidence gap:** This is accurate for TinkerPop's SubgraphStrategy and PartitionStrategy specifically, but the broader claim that "query-time translation cannot synthesize new connections" is a design constraint, not a fundamental impossibility. A query-time mechanism could, in principle, synthesize virtual edges during traversal — this is what TinkerPop's custom traversal strategies do. The essay dismisses this possibility without acknowledging it, then cites TinkerPop's specific implementations (which do not synthesize edges) as evidence. The conclusion against Option C is correct, but the reasoning leans on a constrained example.
- **Recommendation:** Tighten the claim to "a query-time filter of the TinkerPop pattern (predicate injection, property tagging) cannot synthesize new connections." Acknowledge that more invasive query-time mechanisms could synthesize virtual edges, then note the additional costs (traversal ordering, cache coherence, composability with existing primitives) that make this more complex than Option A rather than merely a filter.

**P3-2: The MV4PG ~100x read speedup claim appears without scope qualification**

- **Location:** "What Comparable Systems Show / MV4PG: View Edges as First-Class Graph Elements"
- **Claim:** "The measured read speedup is up to ~100x."
- **Evidence gap:** The research log table confirms this figure is from the Xu 2024 paper, but neither the essay nor the research log specifies what query workload and graph characteristics produced this number. "Up to ~100x" is a peak figure from a specific benchmark; it does not characterize typical speedup. The essay uses it to validate Option A, but the Plexus use case (full in-memory context, Vec-based traversal) differs meaningfully from MV4PG's disk-backed property graph. The speedup comparison is directionally valid (materialized edges are faster to traverse than multi-hop paths), but the specific magnitude should not be adopted without the benchmark context.
- **Recommendation:** Drop the specific figure or qualify it: "MV4PG reports read speedup up to ~100x in their benchmark workload, validating the directional claim that materialized view edges reduce traversal cost." The core argument (materialization is faster than multi-hop recomputation) does not require a specific speedup number to hold.

**P3-3: The naming convention proposal blurs the distinction between relationship type and enrichment ID**

- **Location:** "The Answer: A Lens Is Just an Enrichment / Addressing the 'Create an Index?' Question" and "What a Lens-Enrichment Needs That Does Not Yet Exist"
- **Claim:** The essay proposes `relationship = "lens:trellis:thematic_connection"` and `enrichment_id = "lens:trellis:thematic"` — two closely related but distinct strings serving different purposes. The relationship type goes on the edge's `relationship` field; the enrichment ID goes into the `contributions` map key.
- **Evidence gap:** The essay introduces this without noting that these two strings must be kept distinct by convention alone. Nothing in the current `Enrichment` trait or `commit_edges()` enforces a relationship between the enrichment's `id()` return value and the relationship types it emits. A lens that uses `id() = "lens:trellis:thematic"` but emits edges with `relationship = "thematic"` (omitting the prefix) would break the namespace separation the essay relies on for consumer filtering. The essay flags the namespace convention as needing formal definition (point 3 in "What a Lens-Enrichment Needs") but does not surface this as a risk to the proposed mechanism — a lens using a wrong relationship type would still commit correctly; it would just be unfilterable.
- **Recommendation:** Note explicitly that the relationship namespace convention (`lens:{consumer}:`) is a constraint on lens implementations, not enforced by the infrastructure. The DECIDE phase for lens contract (OQ-20) should include validation of the relationship type prefix as part of the lens registration or spec validation, rather than relying on convention.
