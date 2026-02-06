# Reinforcement Spike: Research Log

> **Status**: Research in progress
> **Date**: 2026-02-05
> **Blocking**: WeightsChanged event (defined but never fired), reinforcement mechanics undefined

## Core Question

When an adapter emits an edge that already exists in the graph, what should happen to `raw_weight`?

## Context

The adapter layer infrastructure is built (`src/adapter/`). `Context.add_edge()` currently uses `max(existing.raw_weight, new.raw_weight)` for exact duplicates — meaning re-emitting an edge with default weight (1.0) does nothing. The `WeightsChanged` graph event is defined but never fired.

The user identified that reinforcement semantics are **domain-dependent**:

- **CodeCoverageAdapter**: "Here's a codebase we're generating with AI. As structure emerges we strengthen edges when components have behavioral, unit, and integration tests." Re-running the same test suite shouldn't reinforce — only net-new tests should. Reinforcement = evidence breadth.
- **MovementAdapter**: Repeating the same gesture indicates importance. Repetition IS the signal. Reinforcement = frequency.

These are different adapters with different semantics. The engine can't hardcode one policy.

---

## Question 1: How do existing systems handle multi-source reinforcement with different update semantics?

**Method:** Web search (two parallel threads — KG/memory systems and software architecture patterns)

### Knowledge Graph Systems

| System | Who controls? | Mechanism | New vs. Repeat? |
|--------|--------------|-----------|-----------------|
| Google Knowledge Vault | Engine (learned fusion) | Supervised classifier over extractor features | Implicit — different extractors = different sources |
| NELL (CMU) | Engine + source diversity | Confidence thresholding + provenance | Explicit — 3 different subsystems > same subsystem 3× |
| Wikidata | Humans | Rank labels, no numeric fusion | Multiple statements preserved |
| Neo4j MERGE | Caller (ON MATCH) | Configurable per-call | Caller must implement |
| GraphRAG | Engine (count) | Normalized extraction frequency | None — full recompute |

**Key finding (NELL):** Cross-source evidence is qualitatively stronger than same-source repetition. A belief proposed by 3 independent subsystems has higher confidence than one proposed by the same subsystem 3 times.

### Hebbian / Neural Patterns

- Classical Hebbian is additive but unbounded — needs normalization (Oja's rule, BCM, soft bounds)
- **Differentiable Plasticity (Miconi 2018):** Separates `fixed_weight` (stable) from `plastic_trace` (Hebbian accumulation). The plasticity coefficient is learned per-connection.
- Plexus already has `NormalizationStrategy` (OutgoingDivisive, Softmax) — normalization infrastructure exists

### Software Architecture Patterns

| Pattern | Policy owner | Communicated via | Multi-producer? |
|---------|-------------|-----------------|----------------|
| Event Sourcing | Consumer/projection | Implicit in code | Works |
| CRDTs (G-Counter) | Data type definition | Structural | Works — per-replica max, cross-replica sum |
| Apache Hudi | Table-level config | Merge mode enum | Works — all writers same treatment |
| Neo4j APOC | Caller, per-call | Strategy map | Breaks — inconsistent across callers |
| Redux | Slice owner | Reducer function | Works — strict ownership |
| Git merge drivers | Repo config, dispatched by file type | `.gitattributes` | Works — dispatch table |

### Implications

**The G-Counter insight is the strongest fit.** CRDT G-Counters track contributions per-replica-ID: `max` within a replica, `sum` across replicas. Applied to plexus:

- Per-adapter weight contributions on each edge
- Same adapter re-emitting = `max` (idempotent) — CodeCoverageAdapter re-running tests
- Different adapter emitting same edge = `sum` (genuine cross-source reinforcement)

Since CodeCoverageAdapter and MovementAdapter are different adapters with different semantics, the question becomes: **who computes the weight before emission?**

- **CodeCoverageAdapter** emits weight = (number of distinct tests covering this relationship). Re-running the same suite → same weight → `max` = no change. Adding a new test → higher weight → `max` = update.
- **MovementAdapter** emits weight = (cumulative gesture count). Each repetition → higher weight → `max` = update.

The adapter owns the semantics of *what its weight means*. The engine just stores each adapter's latest contribution and sums across adapters. This is the G-Counter pattern exactly.

**Risk:** Adding `HashMap<AdapterId, f32>` to `Edge` increases per-edge storage. But this is the price of provenance, and it enables:
- Full attribution (who contributed what)
- Clean rollback (remove an adapter's contribution without recalculating everything)
- Natural `WeightsChanged` firing (when an adapter's contribution changes)

### The spectrum

```
Adapter owns fully                              Engine owns fully
     |                                                |
  APOC-style        Git drivers       G-Counter    Hudi-style
  (per-call map)    (dispatch table)  (per-adapter) (single policy)
  ⚠ inconsistent    ✓ registered      ✓ structural  ⚠ inflexible
```

G-Counter sits in a sweet spot: the engine owns the merge rule (max-within, sum-across), but each adapter owns the semantics of its own weight value.

---

---

## Question 2: Does G-Counter sum produce sensible normalized weights when adapters operate at different scales?

**Method:** Spike (`scratch/spike-reinforcement/`)

**Spike question:** "Does additive per-adapter contribution (G-Counter: max-within, sum-across) produce sensible rank ordering under OutgoingDivisive normalization when adapters operate at different scales?"

### Setup

One source node "A" with outgoing edges to B, C, D, E. Two adapters: CodeCoverage (scale 1-20) and Movement (scale 1-500). Six scenarios testing G-Counter behavior, per-adapter normalization, and source diversity.

### Results

#### Scenario 1: Single adapter, repeated emissions — WORKS

CodeCoverage runs 3 times as tests are added. G-Counter `max` gives idempotent re-runs.

```
Edge | CC contribution | raw_weight | OutDiv rank
  B  |            2.0  |       2.0  |     3
  C  |            3.0  |       3.0  |     2
  D  |            4.0  |       4.0  |     1
```

Rank matches test count. Re-running with same weights = no change. Adding tests = weight increases. Correct.

#### Scenario 2: Two adapters, same scale — WORKS

```
Edge | CC  |  Mv | raw  | OutDiv
  B  | 5.0 | 3.0 |  8.0 | 0.286
  C  | 2.0 | 8.0 | 10.0 | 0.357
  D  | 7.0 | 1.0 |  8.0 | 0.286
  E  | 1.0 | 1.0 |  2.0 | 0.071
```

When scales are comparable, naive sum produces sensible ordering.

#### Scenario 3: Two adapters, DIFFERENT scales — BROKEN

```
Edge |   CC |    Mv |  raw  | OutDiv | Softmax
  B  | 15.0 |  10.0 |  25.0 | 0.033  | ~0.000
  C  |  2.0 | 400.0 | 402.0 | 0.535  | ~1.000
  D  | 18.0 | 300.0 | 318.0 | 0.423  | ~0.000
  E  |  1.0 |   5.0 |   6.0 | 0.008  | ~0.000
```

**C ranks above D** despite D being strong in BOTH domains (18 tests + 300 gestures vs 2 tests + 400 gestures). Movement's scale dominates — CodeCoverage's 2 vs 18 is invisible. Softmax collapses to a one-hot vector on C.

**This is the core problem with naive G-Counter sum: scale dominance.**

#### Scenario 4: Per-adapter normalization before summing — FIXES IT

Normalize each adapter's contributions to [0,1] (divide by adapter's max across its edges), then sum.

```
Edge |   CC_norm |  Mv_norm |  raw  | OutDiv
  B  |     0.833 |    0.025 | 0.858 | 0.227
  C  |     0.111 |    1.000 | 1.111 | 0.293
  D  |     1.000 |    0.750 | 1.750 | 0.462  ← now #1
  E  |     0.056 |    0.013 | 0.068 | 0.018
```

D correctly ranks first — strong in both domains. Both OutgoingDivisive and Softmax produce sensible distributions.

#### Scenario 5: Adapter-side normalization — EQUIVALENT

If adapters emit values on [0,1] scale themselves (adapter computes `weight / max_in_my_domain`), the numbers are identical to Scenario 4. **It doesn't matter where normalization happens** — engine-side or adapter-side — as long as contributions are on comparable scales.

#### Scenario 6: Source diversity — TIED (optional bonus)

```
Edge |  CC  |  Mv  | raw  | sources | OutDiv
  B  | 0.8  |  —   | 0.8  |    1    | 0.500
  C  | 0.4  | 0.4  | 0.8  |    2    | 0.500
```

Same total weight, but C has cross-modal agreement (two independent adapters). Under naive sum, they tie. A small diversity bonus (`raw * (1 + 0.1 * (n_sources - 1))`) breaks the tie: B=0.800, C=0.880.

### Findings

1. **G-Counter (max-within, sum-across) works when adapters emit on comparable scales.** Scenarios 1, 2, 4, 5 all produce correct rank ordering.

2. **Naive sum fails when adapters emit on different scales.** Scenario 3 demonstrates that a high-frequency adapter (Movement: 1-500) drowns out a low-frequency adapter (CodeCoverage: 1-20). This is the critical failure mode.

3. **Per-adapter normalization fixes scale dominance.** Dividing each adapter's contribution by its own max (or having the adapter self-normalize to [0,1]) produces correct rankings. Both engine-side and adapter-side normalization give identical results.

4. **Source diversity is a nice-to-have, not essential.** Cross-modal agreement (Scenario 6) is only visible through adapter count, not through weight alone. A diversity bonus is simple to add but not required for correct basic operation.

5. **Softmax amplifies scale problems.** Under different scales, Softmax collapses to a one-hot vector. OutgoingDivisive is more forgiving but still wrong. Per-adapter normalization fixes both.

### Implications for design

The spike resolves ADR-001 Open Question 1. Two viable paths:

**Option A — Adapter self-normalizes (simplest):**
- Contract: adapters SHOULD emit `raw_weight` on [0,1] scale
- Engine: G-Counter merge (max-within, sum-across), no per-adapter normalization
- Pro: engine stays simple, no per-adapter tracking needed on Edge
- Con: adapter authors must understand the contract; no enforcement
- Con: `raw_weight` on Edge is still a single f32 — loses per-adapter attribution

**Option B — Engine normalizes per-adapter (richer):**
- Edge stores `HashMap<AdapterId, f32>` (per-adapter contributions)
- Engine normalizes each adapter's contributions before summing
- `raw_weight()` is a computed property: `sum(normalized contributions)`
- Pro: full provenance, adapter rollback, diversity bonus trivial to add
- Pro: adapters can emit in whatever scale is natural to them
- Con: per-edge storage cost (HashMap), more complex engine

**Recommendation:** Option B aligns better with ADR-001's two-layer provenance (Decision 5) and the principle that adapters shouldn't coordinate with each other (Decision 11). The engine already handles structural context — it should handle scale normalization too. Adapters emit what's natural; the engine makes it fair.

---

## Open Questions

1. **Source diversity bonus:** Is it worth implementing? The NELL research says cross-source > same-source, but it adds complexity. Could defer to a future spike.
2. **Per-adapter normalization strategy:** Divide by max? Divide by sum? Z-score? The spike used divide-by-max (maps to [0,1]). Other strategies may be worth exploring if edge cases arise.
3. **Storage format:** `HashMap<AdapterId, f32>` on Edge, or a separate per-edge provenance table? The HashMap is simple but grows Edge size. A separate table is normalized but requires joins.
