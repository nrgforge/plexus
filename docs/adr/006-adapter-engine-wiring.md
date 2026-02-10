# ADR-006: Adapter-to-Engine Wiring

**Status:** Proposed

**Date:** 2026-02-10

**Research:** [Essay 08](../research/semantic/essays/08-runtime-architecture.md), [Research Log Q2](../research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — Runtime architecture section

**Depends on:** ADR-001 (adapter architecture), ADR-003 (contribution tracking)

---

## Context

The adapter layer (ADR-001, ADR-003, ADR-004) is built and tested — 197 tests covering emission validation, sinks, contributions, scale normalization, and two concrete adapters. But the layer is disconnected from Plexus's runtime: adapters write to a bare `Arc<Mutex<Context>>`, bypassing PlexusEngine and its persistence via GraphStore. Nothing an adapter emits reaches SQLite. External consumers (Trellis, Carrel) cannot use the adapter layer because it has no persistence and no external surface.

A code trace (Essay 08, Q2) confirmed that the concurrency models are compatible: `EngineSink::emit()` does synchronous work inside an `async fn` — no `.await` points during mutation. PlexusEngine's DashMap provides the same exclusive `&mut Context` access that the Mutex provides, via `get_mut()` returning a `RefMut`. The swap is local to EngineSink.

---

## Decisions

### 1. EngineSink gains a constructor taking `Arc<PlexusEngine>` + `ContextId`

`EngineSink::for_engine(engine: Arc<PlexusEngine>, context_id: ContextId)` creates a sink that routes emissions through the engine instead of a bare Mutex. All validation, contribution tracking, change detection, scale normalization, graph event, and provenance construction logic is identical — it operates on `&mut Context` either way.

The existing `Arc<Mutex<Context>>` constructor stays for tests. The `AdapterSink` trait is unchanged. No adapter code changes.

**Alternatives considered:**

- *Adapters call PlexusEngine directly.* Rejected: PlexusEngine's `add_node()`/`add_edge()` methods are thin wrappers that don't handle contributions, events, or validation. EngineSink's emission protocol is the real integration point.
- *Merge EngineSink into PlexusEngine.* Rejected: conflates the engine's context lifecycle role with the adapter protocol role. The sink abstraction keeps them separate.

### 2. PlexusEngine exposes `with_context_mut()` closure-based accessor

```
engine.with_context_mut(&ctx_id, |ctx| { /* mutate */ })?
```

This keeps DashMap internals private and handles persistence automatically after the closure completes. EngineSink calls this instead of accessing the DashMap directly.

**Alternatives considered:**

- *Make the `contexts` DashMap field public.* Rejected: exposes implementation details. Any consumer could bypass persistence.
- *Return a guard type.* Considered: a `ContextGuard` that persists on drop. More ergonomic for complex operations but more complex to implement. The closure approach is simpler and sufficient.

### 3. Persist-per-emission

Each `emit()` call results in exactly one `save_context()` call at the end, after all items in the emission are committed. Emissions are the persistence boundary.

**Alternatives considered:**

- *Persist per item.* Rejected: an emission containing 10 nodes and 5 edges would trigger 15 SQLite writes instead of 1. Wasteful and slower.
- *Persist on demand (batch).* Considered: adapter calls multiple `emit()`, then explicitly triggers persist. More efficient for bulk ingestion but requires a new API surface and changes the atomicity guarantee. Deferred — can be added later as a `batch_emit()` variant without breaking persist-per-emission.

### 4. Context-not-found error variant

The DashMap path can fail to find a context (deleted or never created). The existing Mutex path cannot fail. EngineSink needs a new error case in `AdapterError` for context not found.

---

## Consequences

**Positive:**

- Adapter emissions reach persistent storage without changing any adapter code
- The `AdapterSink` trait boundary is preserved — adapters remain unaware of persistence
- All 197 existing tests continue to pass using the Mutex constructor
- Persist-per-emission is atomic and simple; batch optimization available later

**Negative:**

- Persist-per-emission writes the full context to storage on every `emit()`. For bulk ingestion (hundreds of fragments), this could be slow. Acceptable for initial use; batch persist is a future optimization.
- The `with_context_mut()` closure holds a DashMap shard lock for the duration of the emission. Long-running emissions block other operations on the same shard. In practice, emissions are fast (synchronous, no I/O except the final persist).

**Neutral:**

- This ADR does not add an MCP ingestion surface (gap 2 from Essay 08). That is a feature built on top of this wiring, not an architectural decision.
- This ADR does not address context sources triggering adapters (gap 5). That requires a source-watching mechanism beyond this scope.
