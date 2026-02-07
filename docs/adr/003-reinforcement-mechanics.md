# ADR-003: Reinforcement Mechanics — Per-Adapter Contributions with Scale Normalization

**Status:** Proposed

**Date:** 2026-02-05

**Resolves:** ADR-001 Open Question 1 (edge reinforcement mechanics). Does **not** resolve the node property merge sub-question — that remains open.

**Research:** [SPIKE-REINFORCEMENT.md](../research/semantic/SPIKE-REINFORCEMENT.md), [Essay 06](../research/semantic/essays/06-reinforcement-mechanics.md)

---

## Context

ADR-001 Decision 7 establishes that raw weights are stored and normalized weights are computed at query time. But it leaves the reinforcement operation undefined: what happens to an edge's raw weight when a second adapter emits the same edge, or when the same adapter re-emits it on re-processing?

The research spike established that reinforcement semantics are domain-dependent. A CodeCoverageAdapter re-running the same test suite should not reinforce — only net-new tests provide evidence. A MovementAdapter repeating the same gesture should reinforce — repetition is the signal. The engine cannot hardcode a single policy.

A numerical spike confirmed that when adapters emit on different scales (CodeCoverage: 1–20, Movement: 1–500), naive summation produces incorrect rank ordering — the higher-magnitude adapter drowns out all others. Per-adapter normalization before summing fixes this.

## Decision

### 1. Per-adapter contribution tracking

Each edge stores per-adapter contributions as a map from adapter ID to that adapter's latest contribution value. When an adapter emits an edge that already exists, the engine replaces that adapter's contribution slot with the new value:

- **Same adapter, same value:** no change (naturally idempotent)
- **Same adapter, higher value:** contribution increases (e.g., new tests added)
- **Same adapter, lower value:** contribution decreases (e.g., tests deleted, evidence retracted)
- **Different adapter emitting same edge:** each adapter gets its own slot — cross-source reinforcement

Contributions can be any finite f32 value. Adapters emit in whatever scale is natural to their domain (e.g., 0–20 for test counts, 0–500 for gesture repetitions, -1.0–1.0 for sentiment). The engine's scale normalization function must handle the full range of each adapter's contributions. Note that all contributions — regardless of sign — add positively to raw weight after scale normalization. A contribution value represents the strength of an observation, not its polarity; qualities like sentiment direction belong in annotations or edge properties. Evidence *against* an existing edge is a reflexive adapter concern (examining graph state and proposing refinements), not a negative-contribution concern.

The CRDT G-Counter pattern inspired this decomposition (per-replica tracking, sum across replicas), but we use latest-value-replace rather than G-Counter's max-merge. The G-Counter's commutativity guarantee is unnecessary — each adapter writes only to its own slot, so concurrent emissions from different adapters are partition-safe regardless of ordering. And G-Counter's monotonicity constraint (values can only increase) is actively harmful: if tests are deleted or evidence is retracted, the adapter should be able to emit a lower contribution and have the engine honor it.

### 2. Engine-side scale normalization

The engine normalizes each adapter's contributions to a comparable scale before summing. Adapters emit contribution values in whatever scale is natural to their domain — including signed ranges (e.g., sentiment on -1.0 to 1.0). The engine applies a scale normalization function (initially divide-by-adapter-range) to bring all adapters to a comparable range.

This produces a three-layer weight model:

| Layer | What | Stored or computed? | Who controls? |
|-------|------|-------------------|---------------|
| **Contribution** | Per-adapter value on each edge | Stored (`HashMap<AdapterId, f32>`) | Adapter sets via `emit()` |
| **Raw weight** | Sum of scale-normalized contributions | Computed by engine | Engine owns scale normalization |
| **Normalized weight** | Relative importance per query | Computed at query time | Consumer chooses strategy |

Scale normalization and query-time normalization serve different purposes. Scale normalization (engine-side) prevents one adapter's magnitude from dominating others — fairness across sources. Query-time normalization (consumer-side, per ADR-001 Decision 7) computes relative importance per-node — an interpretive lens. Both are necessary.

The specific scale normalization function (divide-by-range, divide-by-max, z-score) is an implementation choice, not an architectural commitment. The ADR commits to engine-side scale normalization as a concept. The initial implementation uses divide-by-adapter-range: `(value - min) / (max - min)`, which maps any adapter's contributions to [0, 1] regardless of whether the adapter's native scale is signed or unsigned. When an adapter's contributions span zero range (min == max), all contributions from that adapter scale-normalize to 1.0.

**Caveat:** Scale normalization using global anchors (adapter's min and max across all edges) means that a single outlier emission from one adapter shifts all of that adapter's scale-normalized values. This is bounded to one adapter's edges (unlike global query-time normalization, which ADR-001 rejected) but has a wider blast radius than per-node query-time normalization. If this proves problematic, the normalization function can be changed without altering the per-adapter contribution architecture.

### 3. WeightsChanged event fires on contribution change

The `WeightsChanged` graph event (defined in ADR-001 Decision 10 but previously unimplemented) fires when an adapter's contribution to an edge changes — that is, when the new contribution value differs from the stored value for that adapter slot.

**Alternatives considered:**

- *Single f32 with additive increment.* Rejected: same adapter re-emitting inflates weight without new evidence. No way to distinguish "CodeCoverage re-ran tests" from "CodeCoverage found new tests."
- *Adapter self-normalizes to [0,1].* Considered viable. The spike showed identical numerical results to engine-side normalization. Rejected because it violates ADR-001 Decision 11 (adapters don't coordinate with each other) — an adapter would need to understand the scale convention, and a misconfigured adapter emitting on 0–127 would silently dominate.
- *G-Counter max-merge.* The initial research direction. Rejected because G-Counter values can only increase — if tests are deleted or evidence is retracted, the edge retains a stale contribution. Latest-value-replace handles both increases and decreases while remaining naturally idempotent for unchanged values.
- *Explicit `reinforce()` method separate from `emit()`.* Rejected: adds API surface without adding capability. The contribution-replace on `emit()` handles both "new edge" and "update existing edge" transparently.
- *Source-diversity-weighted increment.* Deferred, not rejected. The per-adapter HashMap makes a diversity bonus trivial to add later. The right coefficient is an empirical question. See Open Questions.

## Consequences

**Positive:**

- Reinforcement semantics are adapter-independent — each adapter owns what its contribution value means; the engine provides fair combination
- Full per-adapter attribution on every edge — enables rollback (remove an adapter's contribution), auditing, and provenance queries
- Contributions can decrease as well as increase — evidence retraction, test deletion, and re-assessment are first-class operations
- `WeightsChanged` event can now fire with meaningful semantics
- Adapters remain fully independent (ADR-001 Decision 11) — no scale coordination needed
- Query-time normalization (ADR-001 Decision 7) operates on the computed raw weight unchanged

**Negative:**

- Per-edge storage grows from a single `f32` to a `HashMap<String, f32>`. For graphs with many edges and few adapters, this is modest. For graphs with many adapters contributing to the same edges, it scales linearly with adapter count.
- `Context.add_edge()` must change — current `max(existing.raw_weight, new.raw_weight)` on a single f32 becomes per-adapter-slot replace. This is a structural change to the graph engine.
- Engine must track per-adapter min and max across all edges for scale normalization. This is a graph-wide operation (or cached value) rather than a per-edge operation. A single outlier emission from one adapter shifts scale-normalized values on all of that adapter's edges.

**Neutral:**

- The `raw_weight` field on `Edge` changes from a stored `f32` to a computed property. Code that reads `edge.raw_weight` continues to work; code that writes it must go through the contribution mechanism.

---

## Open Questions (deferred, not blocking)

1. **Source diversity bonus.** Should edges confirmed by more adapters rank higher than edges with equal total weight from fewer adapters? The data structure supports it. The coefficient is empirical.
2. **Scale normalization function.** The initial implementation uses divide-by-range (`(v - min) / (max - min)`). Divide-by-sum, z-score, or outlier-robust alternatives may be worth exploring if edge cases arise (e.g., a single outlier emission compressing all other edges from that adapter into a narrow band). Note that divide-by-range maps the adapter's minimum contribution to 0.0 — if an adapter's minimum is non-trivial (e.g., "one test covers this relationship"), the weakest-but-real evidence becomes invisible after scale normalization. Adapter-declared ranges or a shifted formula could address this if it proves problematic.
3. **Scale normalization cache invalidation.** When a new edge extends an adapter's min or max, all of that adapter's scale-normalized contributions change. Cache strategy TBD during implementation.
4. **Node property merge on multi-source upsert.** When two adapters emit the same node with different properties, what merge semantics apply? This sub-question from ADR-001 Open Question 1 is not resolved by this ADR.
