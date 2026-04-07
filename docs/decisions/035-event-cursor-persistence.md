# ADR-035: Event Cursor Persistence

**Status:** Accepted

**Research:** [Essay 001](../essays/001-query-surface-design.md)

**Domain model:** [domain-model.md](../domain-model.md) — event cursor, graph event, outbound event, pull changes, library rule

**Depends on:** ADR-012 (unified ingest pipeline), ADR-029 (architectural consolidation)

**Resolves:** OQ-8 (event persistence and cursor-based delivery), OQ-22 (event cursor schema and retention)

---

## Context

Graph events are currently produced during the enrichment loop and discarded after the loop completes. The push paradigm (outbound events via `transform_events()`) requires the consumer to be listening when events fire. Without a persistent event log, the pull paradigm — "what changed since I last looked?" — forces Plexus into an always-on server role, violating the spirit of the library rule (Invariant 41).

Invariant 58 establishes the requirement: event cursors preserve the library rule for read workflows. With cursors, the graph is SQLite on disk — consumers write, walk away, come back, query "changes since sequence N."

Product discovery (2026-03-25) identifies the pull paradigm as critical: many real consumer workflows are pull-based — a CRON job checking for new connections, a user-initiated query, a scheduled analysis.

## Decision

### Event log table in SQLite

A new `events` table persists graph events alongside the existing `nodes` and `edges` tables:

```sql
CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    context_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    node_ids_json TEXT,
    edge_ids_json TEXT,
    adapter_id TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_events_context_sequence ON events (context_id, sequence);
```

- **`sequence`**: monotonically increasing integer. This IS the cursor — a consumer stores the last-seen sequence number and queries `WHERE sequence > ?`.
- **`event_type`**: one of the six `GraphEvent` variants: `NodesAdded`, `EdgesAdded`, `NodesRemoved`, `EdgesRemoved`, `WeightsChanged`, `ContributionsRetracted`.
- **`node_ids_json` / `edge_ids_json`**: serialized lists of affected node or edge IDs. Lightweight: only IDs, not full node/edge payloads. The consumer uses these IDs to query current graph state if needed.
- **`adapter_id`**: which adapter or enrichment produced the event. Enables provenance-scoped cursor queries: "changes from this adapter since sequence N."
- **`created_at`**: timestamp for time-based queries and retention.

### Persistence boundary

Events are persisted within `emit_inner()`, after commit succeeds and before returning to the enrichment loop. Each enrichment round's events are written to the event log as part of the same persistence operation that saves context state. This means the event log is consistent with the graph — if a commit succeeds, the events are logged; if it fails, neither persists.

The full event stream for one `ingest()` call spans multiple enrichment rounds. Each round's events are persisted independently, producing multiple event log entries with consecutive sequence numbers. A consumer querying the cursor sees the complete sequence of changes — initial adapter emission, first enrichment round, second round, and so on.

### Raw events, not transformed events

The event log persists raw graph events (`NodesAdded`, `EdgesAdded`, etc.), not adapter-transformed outbound events. Two reasons:

1. **Consumers with a lens receive pre-translated events.** With the lens-as-enrichment (ADR-033), cursor events for `EdgesAdded` with relationship `lens:trellis:thematic_connection` are already domain-meaningful — the translation happened at write time. Consumers without a lens receive raw relationship types (`may_be_related`, `similar_to`), which they interpret through their adapter's domain knowledge or by consulting `evidence_trail`. The lens is not a universal solution, but it is the write-time translation mechanism that makes raw events domain-useful for consumers who define one.

2. **Library rule compatibility.** Adapter-transformed events require the adapter to be available at query time. Raw events are self-contained data in SQLite — queryable without loading any adapter code. This preserves the library rule: the event log is just a table, readable by any SQLite client.

### Relationship to Invariant 37

Invariant 37 states: "Outbound events flow through the adapter. Consumers never see raw graph events." This ADR introduces a second event delivery path — the cursor — where consumers do see raw graph events. Invariants 37 and 58 together define a two-paradigm model:

- **Invariant 37 governs push delivery:** outbound events are adapter-transformed via `transform_events()`. The consumer receives domain-translated events synchronously.
- **Invariant 58 governs pull delivery:** event cursors preserve the library rule for read workflows. The consumer queries a persistent change log asynchronously.

This ADR operationalizes Invariant 58. Invariant 37 requires amendment to scope it to the push paradigm, with a note that Invariant 58 governs the pull path. See domain model Amendment 6.

The cursor supplements push delivery — it does not replace `transform_events()` for consumers using the push model.

### Cursor query API

A new method on `PlexusApi`:

```rust
pub fn changes_since(
    &self,
    context_id: &str,
    cursor: u64,
    filter: Option<CursorFilter>,
) -> PlexusResult<ChangeSet>
```

Where:

```rust
pub struct CursorFilter {
    pub event_types: Option<Vec<String>>,
    pub adapter_id: Option<String>,
    pub limit: Option<usize>,
}

pub struct ChangeSet {
    pub events: Vec<PersistedEvent>,
    pub latest_sequence: u64,
}
```

The consumer stores `latest_sequence` from the response and uses it as the cursor for the next query. `CursorFilter` allows scoping by event type or adapter — a consumer interested only in lens-created edges queries `adapter_id: "lens:trellis:thematic_connection"` and `event_types: ["EdgesAdded"]`.

### Retention policy

Context-scoped, declared in context metadata:

- **Default: keep all events.** No retention limit. Suitable for small-to-medium contexts where the event log growth is negligible compared to graph size.
- **Count-based retention:** keep the most recent N events per context. Older events are pruned on write (piggyback on `emit_inner()`).
- **Time-based retention:** keep events newer than a duration (e.g., 30 days). Pruned on write or on a periodic cleanup pass.

Retention is declared per-context, not globally. A research context with rare writes keeps all events; a real-time performance context with high-frequency writes limits retention.

A consumer that falls behind the retention window (its cursor points to a pruned sequence) receives an error indicating the cursor is stale. The consumer's recovery path: reload the full context via `load_context()` and reset its cursor to the latest sequence.

## Consequences

**Positive:**

- The pull paradigm is fully supported without requiring a persistent runtime. A consumer writes data, shuts down, restarts days later, and queries "what changed since sequence 47." The library rule is preserved for read workflows.
- The event log provides an audit trail of graph mutations, useful for debugging and understanding graph evolution beyond what the current snapshot-only persistence captures.
- Provenance-scoped cursor queries (`adapter_id` filter) compose naturally with the lens: "what did my lens create since I last looked?"

**Negative:**

- Storage growth. Each `ingest()` call produces events across multiple enrichment rounds. A context with 4 enrichments producing 3 rounds of events per ingest generates ~12 event log entries per write. For contexts with heavy write traffic, the event table grows faster than the graph itself. Retention policy mitigates this.
- The event log records IDs, not payloads. A consumer that needs the full node/edge data must follow up with a graph query. This is intentional (IDs are stable; payloads may have changed since the event), but adds a round-trip for consumers that need complete change data.
- Stale cursor recovery requires a full context reload. For large contexts, this is expensive. An alternative (cursor compaction — summarizing old events into a single "here was the state at sequence N" checkpoint) is deferred.

**Neutral:**

- The existing push paradigm (`transform_events()`) is unaffected. Consumers using push continue to receive adapter-transformed outbound events as before. The cursor is an additional capability, not a replacement.
- The `events` table is created alongside the existing `nodes`, `edges`, and `metadata` tables during `GraphStore` initialization. No migration needed for existing databases — the table is created if absent.
- MCP exposure of `changes_since` is a transport concern, not an architectural one. It follows the same pattern as exposing any `PlexusApi` method as an MCP tool.
