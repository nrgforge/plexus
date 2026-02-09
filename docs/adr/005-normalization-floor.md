# ADR-005: Normalization Floor — Dynamic Epsilon in Scale Normalization

**Status:** Proposed

**Date:** 2026-02-08

**Refines:** ADR-003 Decision 2 (scale normalization function)

**Research:** [Essay 07](../research/semantic/essays/07-first-adapter-pair.md), [Research Log](../research-log.md)

**Scenarios:** [004-first-adapter-pair.md](../scenarios/004-first-adapter-pair.md) — Normalization Floor section

---

## Context

ADR-003 established scale normalization using divide-by-range: `(value - min) / (max - min)`. This maps each adapter's contributions to [0, 1], preventing scale dominance. ADR-003 noted a known issue: the minimum contribution from any adapter maps to exactly 0.0. Real evidence — a concept pair that co-occurs once, a test that covers one path — becomes invisible after scale normalization.

Design analysis during Essay 07 demonstrated this is a practical problem. The CoOccurrenceAdapter produces co-occurrence scores ranging from `count / max_count` (e.g., 0.5 for single co-occurrence, 1.0 for double). After scale normalization, all single-co-occurrence proposals map to 0.0 — the weakest real evidence vanishes.

ADR-003 anticipated this: "Adapter-declared ranges or a shifted formula could address this if it proves problematic." It proves problematic.

---

## Decision

### Dynamic epsilon proportional to range

Replace the scale normalization formula with:

```
(value - min + ε) / (max - min + ε)   where ε = α × (max - min)
```

Which simplifies to:

```
(value - min + α·range) / ((1 + α)·range)
```

The floor coefficient α defaults to 0.01. This gives:

- **At the minimum:** `α / (1 + α)` ≈ 0.0099 — the same proportional floor for every adapter, regardless of range.
- **At the maximum:** exactly 1.0.
- **Degenerate case (range = 0):** ε = 0, handled by the existing special case → 1.0.

The floor coefficient α is a constant, not configurable per-adapter. All adapters get the same proportional floor.

**Alternatives considered:**

- *Static epsilon (fixed small constant).* Rejected: the floor is proportionally larger for narrow-range adapters than wide-range adapters. An adapter with range [0.5, 1.0] gets a much higher floor (~4%) than an adapter with range [1, 500] (~0.002%).
- *Divide-by-max (`value / max`).* Avoids the zero-out entirely (minimum maps to `min/max`). Rejected: breaks on signed ranges (sentiment -1 to 1 gives `-1/1 = -1.0`). Divide-by-range handles signed ranges correctly.
- *Adapter-declared ranges.* Each adapter declares its expected min/max. Rejected for now: adds configuration burden and requires adapter authors to predict their range. Can be added later if the dynamic epsilon proves insufficient.
- *No change — accept the zero-out.* Rejected: the weakest real evidence from the co-occurrence adapter becomes invisible, which is incorrect. A single co-occurrence is weak evidence but it is evidence.

---

## Consequences

**Positive:**

- Real evidence is never invisible — the weakest contribution from any adapter maps to ~1% of that adapter's strongest, not 0.0
- Every adapter gets the same proportional floor regardless of its contribution range — fair across adapters
- One-line change to `Context.recompute_raw_weights()` — minimal blast radius
- Degenerate case behavior is unchanged
- Relative ordering within an adapter is preserved

**Negative:**

- The floor coefficient (α = 0.01) is a magic number chosen without empirical tuning. If 1% is too high or too low, it will need adjustment. The formula makes the tradeoff explicit — changing α is a single constant.
- Existing scale normalization tests that assert exact values (e.g., "minimum maps to 0.0") will need updating.

**Neutral:**

- This refines ADR-003's implementation choice, not its architecture. ADR-003 committed to "engine-side scale normalization as a concept" and explicitly said the function is "an implementation choice, not an architectural commitment."
