# ADR-017: Multi-Application Context Sharing via Shared-DB

**Status:** Proposed

**Research:** [Essay 17](../essays/17-storage-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — context, PlexusEngine, GraphStore, shared-concept convergence, meta-context, persist, commit

**Depends on:** ADR-016 (library rule — shared path resolution is a prerequisite)

---

## Context

Multiple applications need to share the same Plexus contexts. A "network-research" context accessed by Carrel (paper annotations), Manza (code analysis), and Trellis (reflective fragments) is the motivating scenario. Each tool contributes to the same semantic landscape through its own adapter. The shared concept graph reveals connections no tool could discover alone.

Essay 17 evaluated three local architectures: shared-DB (all apps open the same SQLite file), daemon (a `plexus serve` process that apps connect to via IPC), and managed server (the daemon deployed on infrastructure). The essay recommends starting with shared-DB for simplicity and moving to the daemon model when cache coherence or enrichment coordination becomes a real problem.

One prerequisite: the current `save_context()` implementation does a full replace (DELETE all rows, INSERT all rows). This is safe for single-engine use but destructive under concurrent multi-engine writes — Engine A's full replace overwrites Engine B's recent commits.

## Decision

### 1. Shared-DB model as the initial multi-app architecture

Multiple applications open the same SQLite file, each running their own PlexusEngine instance. SQLite's WAL (Write-Ahead Logging) mode supports concurrent reads and serialized writes on the same host. Plexus's persist-per-emission model (invariant 30) means writes serialize at the SQLite level; reads proceed concurrently.

**Prerequisite:** the SQLite database must use WAL mode. WAL is not SQLite's default (the default is rollback journal). The `SqliteStore` implementation must enable it at connection time (e.g., `PRAGMA journal_mode=WAL`). Context naming is a user-level convention — applications must agree on context names out-of-band (e.g., via shared configuration or user input). Context discovery is deferred.

### 2. Cache coherence via `data_version` polling

Each PlexusEngine checks SQLite's `PRAGMA data_version` before reads. The value increments on each write by any connection. If the value has changed since the last check, the engine reloads affected contexts from the store. This adds read latency but preserves the embedded model without IPC.

Polling frequency is a performance tradeoff: checking per-read maximizes freshness but adds a pragma query to every read path. The initial implementation should check per-read and optimize later if the pragma cost is measurable.

### 3. Incremental upserts replace full-context save

`save_context()` changes from DELETE-all + INSERT-all to per-node and per-edge upserts. The individual `save_node()` and `save_edge()` methods already use upsert semantics in the current implementation; the change is to use these instead of the bulk-replace path. This is a prerequisite for shared-DB safety — without it, Engine A's full replace destroys Engine B's recent commits.

This change benefits single-engine use too: incremental upserts are cheaper than full replacement for large contexts with small per-emission changes.

Invariant 30 (persist-per-emission) is preserved — one `save_context()` call per `emit()`. The internal implementation of `save_context()` changes from full-replace to incremental upserts. The invariant's guarantee (each emission triggers one persistence operation) is unchanged.

### 4. Shared-concept convergence query

A new query exposed through `PlexusApi` (delegating to PlexusEngine internally): `shared_concepts(context_a, context_b) -> Vec<ConceptId>`. Returns concept nodes that appear in both contexts, discovered via deterministic concept ID intersection (invariant 19: `concept:{lowercase_tag}`). This requires zero graph changes — it's a property of the ID scheme, surfaced at query time. This extends ADR-014's operation list with a new graph read.

### 5. Daemon model deferred

The daemon model (`plexus serve` with IPC/HTTP/gRPC) is strictly more capable: no stale cache problem, no duplicate enrichment processing, automatic context sharing. But it's operationally heavier — runtime dependency, service management, loss of simple embeddability. The `GraphStore` trait doesn't need to change for either model. Defer the daemon until enrichment coordination or cache coherence becomes a real problem, not a theoretical one.

### 6. Meta-contexts deferred

Meta-contexts (read-only virtual views unioning multiple contexts) are deferred. Shared-concept convergence provides the immediate cross-context awareness. If richer cross-context intelligence is needed, meta-contexts can be added without changing the graph model — they're query-time composition, not stored data.

### Alternatives considered

- **Daemon-first.** More capable but operationally heavier. Deploy complexity is unjustified until the stale-cache and enrichment-coordination problems manifest in practice.
- **Managed server only.** Removes the local multi-app question entirely but requires infrastructure. Not appropriate for solo developers or small teams working locally.
- **Row-level CRDT replication (cr-sqlite).** Proven in production (Fly.io's Corrosion), but replicates everything — no selective replication by tier. Designed for different-host replication, not same-host sharing.

## Consequences

**Positive:**

- Multiple applications can share contexts today with minimal changes — no daemon, no service management
- Incremental upserts improve persistence performance for all use cases, not just shared-DB
- Shared-concept convergence enables cross-context awareness without breaking context isolation
- The path to the daemon model is clear and doesn't require architectural changes — it wraps the same `GraphStore`

**Negative:**

- Stale cache: Engine B doesn't see Engine A's writes until the next `data_version` check. Read operations may see slightly outdated state. Acceptable for the use cases described; unacceptable for real-time collaboration (which needs the daemon model or federation)
- Enrichment coordination: two engines running enrichments independently may produce duplicate work. In practice, idempotency handles this — the same enrichment running on the same data produces the same result (upsert). But if engines have different enrichment registrations, the shared context has inconsistent enrichment state depending on which engine processed each emission
- `save_context()` API change: callers that depend on full-replace semantics (if any) must be updated

**Neutral:**

- The `GraphStore` trait is unchanged — the change is in the `SqliteStore` implementation
- The managed server deployment mode uses the same daemon architecture, just deployed on infrastructure rather than locally. No separate ADR needed
