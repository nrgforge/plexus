# ADR-008: Project-Scoped Provenance

**Status:** Accepted

**Date:** 2026-02-10

**Research:** [Essay 08](../research/semantic/essays/08-runtime-architecture.md), [Research Log Q3](../research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — Invariants 28–29, 32

**Depends on:** ADR-006 (adapter-engine wiring), ADR-007 (contribution persistence)

---

## Context

Marks, chains, and links are graph nodes and edges in the provenance dimension — not a separate storage system. Currently they all live in a global `__provenance__` context, auto-created by the MCP server. Adapter-produced nodes (fragments, concepts) live in project contexts. These are separate Context objects with no cross-context edges or queries.

This isolation is a historical artifact. The marks system descends from "clawmarks," a standalone MCP server for provenance tracking that was integrated into Plexus. The `__provenance__` context served as a container for marks that had no project affiliation.

The dimension system was designed for cross-dimensional connections within a context — `Edge::new_cross_dimensional()` connects nodes in different dimensions (e.g., provenance → semantic). But this only works if both nodes share a context. A mark tagged `#travel` and a concept node `concept:travel` have no connection when they live in separate contexts.

Building cross-context edges would solve the same problem but require infrastructure (cross-context edge storage, cross-context queries, cross-context consistency guarantees) that doesn't exist. Moving marks into project contexts works with the existing within-context dimension system.

The system is new enough that backward compatibility is not a constraint.

> **Note (Essay 12):** Marks now enter contexts from two sources: (1) user-driven provenance via ProvenanceApi/ProvenanceAdapter (explicit research annotations), and (2) adapter-produced provenance alongside semantic output (automatic evidence trails from FragmentAdapter and other semantic adapters). Both produce identical provenance-dimension nodes in project contexts, and both participate in tag-to-concept bridging.

---

## Decisions

### 1. Eliminate the `__provenance__` context

The `__provenance__` context, the `provenance_context()` auto-create helper, and all associated logic are removed. Marks always live in a project context, in the provenance dimension.

### 2. `add_mark` requires a context parameter

No default, no fallback. The MCP `add_mark` tool requires a `context` parameter specifying which project context to place the mark in. The context must already exist.

**Alternatives considered:**

- *Optional context with `__provenance__` as fallback.* Rejected: preserves the isolation that prevents provenance-semantic connections. A fallback would mean some marks are connectable and some aren't, creating an inconsistent experience.
- *Auto-create context from file path.* Rejected: implicit context creation based on file location couples marks to filesystem layout. The user should control context boundaries.

### 3. ProvenanceApi scoped to a specific context

`ProvenanceApi::new(engine, context_id)` creates an API scoped to one context. All mark, chain, and link operations within that API instance target that context. The MCP server creates a new `ProvenanceApi` per request with the client-specified context.

### 4. `list_tags()` queries across all contexts

Tag vocabulary remains globally visible. `list_tags()` iterates over all contexts, collects tags from all mark nodes. This is necessary because Carrel's research agent discovers themes via `list_tags()` and shouldn't need to know which contexts contain which marks.

**Alternatives considered:**

- *`list_tags()` scoped to a single context.* Rejected: forces the client to enumerate all contexts and merge tags. The global vocabulary is a feature, not a bug.
- *Both scoped and global variants.* Considered: `list_tags(context_id)` and `list_all_tags()`. Adds API surface for a feature that isn't yet needed. Can be added later if context-scoped tag listing proves useful.

### 5. Chains remain context-scoped

Chains group marks within the same context. A chain in `trellis` context contains marks from `trellis` only. Cross-context chains are not supported — if marks span contexts, they use separate chains.

---

## Consequences

**Positive:**

- Marks and concept nodes share a context, enabling cross-dimensional edges
- The dimension system works as designed — no new infrastructure needed
- Simpler codebase: `__provenance__` special-casing and auto-create logic removed
- Global tag vocabulary preserved via cross-context `list_tags()`

**Negative:**

- Every `add_mark` call must specify a context. Clients that previously relied on the implicit `__provenance__` context must be updated. In practice this is the Plexus MCP server itself and any MCP clients — the system is new enough that this is a small change.
- Marks that don't naturally belong to a project (e.g., quick annotations during general browsing) have no natural context. Users can create a general-purpose context for this, but the system won't do it automatically.
- `list_tags()` becomes O(contexts × marks) instead of O(marks in __provenance__). Acceptable for the expected scale.

**Neutral:**

- This ADR does not address what happens to existing marks in `__provenance__`. Since the system is pre-release, migration of existing data is not a concern. A fresh database is acceptable.
