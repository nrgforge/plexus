# ADR-007: Contribution Persistence

**Status:** Proposed

**Date:** 2026-02-10

**Research:** [Essay 08](../research/semantic/essays/08-runtime-architecture.md), [Research Log Q2](../research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — Invariant 33

**Depends on:** ADR-003 (contribution tracking), ADR-006 (adapter-engine wiring)

---

## Context

ADR-003 introduced per-adapter contributions on edges: `edge.contributions: HashMap<AdapterId, f32>`. Raw weight is computed from contributions via scale normalization. But the SQLite schema predates this system — `SqliteStore::row_to_edge()` hard-codes `contributions: HashMap::new()`, and `edge_to_row()` does not serialize contributions. After save and load, all per-adapter contribution data is lost and `recompute_raw_weights()` produces incorrect results (no contributions to normalize).

This is a known gap, not a bug — the schema was built before the contribution system existed. With ADR-006 routing adapter emissions to persistent storage, contributions must survive the round-trip.

---

## Decisions

### 1. Add `contributions_json` column to the `edges` table

Schema migration adds `contributions_json TEXT NOT NULL DEFAULT '{}'` to the `edges` table. The migration follows the established pattern used for dimension columns: check `pragma_table_info('edges')` for the column's existence, `ALTER TABLE ADD COLUMN` if missing.

Existing edges get an empty contributions map via the default, which matches their current behavior — they have no contributions and their `raw_weight` was set directly.

**Alternatives considered:**

- *Separate contributions table.* Rejected: adds a join for every edge read. Contributions are always read with their edge — they're not independently queryable.
- *Binary serialization (bincode, MessagePack).* Rejected: JSON is human-readable, debuggable via SQLite CLI, and the contribution maps are small (typically 1–3 entries). Performance is not a concern at this scale.

### 2. JSON serialization for contributions

`edge_to_row()` serializes `edge.contributions` as JSON via serde. `row_to_edge()` deserializes it. The HashMap<AdapterId, f32> structure maps directly to JSON object notation: `{"fragment-manual": 1.0, "co-occurrence": 0.75}`.

f32 values may lose precision in the JSON round-trip. This is acceptable — contribution values are approximate assessments, not exact quantities. Scale normalization is robust to minor precision loss.

---

## Consequences

**Positive:**

- Contributions survive persistence. Scale normalization produces correct results after restart.
- Backward-compatible: existing edges get empty contributions, matching their pre-contribution behavior.
- Schema migration follows an established pattern — no new migration infrastructure needed.
- Contributions are human-readable in the database via standard SQLite tooling.

**Negative:**

- JSON serialization adds a small per-edge cost on read/write. Negligible for the expected edge counts.
- f32 → JSON → f32 round-trip may introduce minor floating-point precision differences. Not meaningful for contribution semantics.

**Neutral:**

- The `raw_weight` column remains in the schema. It continues to be written (computed from contributions) but is now derivable from `contributions_json`. Removing it would be a separate decision about storage denormalization.
