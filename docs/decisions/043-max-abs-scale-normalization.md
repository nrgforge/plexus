# ADR-043: Max-Abs Scale Normalization — Ratio-Preserving Raw Weights

**Status:** Accepted

**Date:** 2026-07-08

**Refines:** ADR-003 Decision 2 (scale normalization function)

**Supersedes:** ADR-005 (dynamic-epsilon divide-by-range)

**Evidence:** M3 blinded-consumer probes (issue #13); both probes independently
found lens raw weights misleading.

---

## Context

ADR-003 scale-normalizes each contributor's values before summing into raw
weight, so heterogeneous adapter scales (0–20 test counts, 0–127 MIDI, −1..1
sentiment) compose without dominance. ADR-005 refined the formula to
divide-by-range with a dynamic epsilon so the weakest real evidence didn't
map to exactly 0.

Divide-by-range has a failure mode ADR-005 didn't anticipate: **when a
contributor's values cluster tightly, range normalization amplifies noise
into signal.** Measured in the M3 probes: cosine similarities
{0.8165, 0.7856, 0.7794} — a near-tie — normalized to {1.0, 0.175, 0.0099},
a 100× spread. Rank order survived; magnitudes became meaningless, and both
blinded consumers flagged them as un-showable to users. The distortion grows
as a contributor gets *more consistent*, which is backwards.

## Decision

Replace the per-contributor normalization with **max-abs scaling**:

```
normalized = value / max(|values across this contributor's edges|)
```

- Ratios are preserved: {0.8165, 0.7856, 0.7794} → {1.0, 0.962, 0.955}.
- Sign is preserved for signed scales (−1..1 sentiment stays negative).
- Cross-adapter comparability is preserved: every contributor maps into
  [−1, 1] by its own magnitude.
- ADR-005's floor concern dissolves rather than transfers: with no min
  subtraction, nonzero evidence can never normalize to 0. The
  `FLOOR_ALPHA` epsilon is removed.
- Degenerate cases: a single value normalizes to ±1.0 (unchanged from
  ADR-005's behavior); an all-zero contributor contributes 0 (zero
  asserted strength is honestly zero).

Raw weight remains the sum of per-contributor normalized values (ADR-003's
three-layer model is untouched: contribution stored → raw weight computed →
normalized weight at query time).

## Consequences

**Positive:** raw weights become ordinal *and* proportionally meaningful;
consumers can display them; consistent contributors are no longer punished.

**Negative:** absolute raw-weight values shift for every existing graph on
first recompute (any write triggers it). Rank order within a contributor is
preserved, so ranking-dependent consumers are unaffected. Release-noted.

**Neutral:** contributors with genuinely meaningful *offsets* (where the
distance from the contributor's own minimum mattered) lose that emphasis;
no shipped or observed consumer relied on it.
