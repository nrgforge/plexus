# ADR-037: Consumer Spec Loading

**Status:** Proposed

## Context

A consumer application interacts with Plexus by loading a declarative adapter spec (containing adapter definition, lens rules, and declared enrichment config) onto a context. The spec is the unit of consumer identity (Invariant 61) — the consumer authors and delivers it; Plexus receives and acts on it.

The current pipeline supports spec loading only at construction time: `PipelineBuilder::with_adapter_specs()` calls `register_specs_from_dir()`, which scans a directory for YAML files and registers adapters. However:

1. **The wiring is incomplete.** `register_specs_from_dir` calls `register_adapter()` — not `register_integration()`. The spec's declared enrichments (`adapter.enrichments()`) and lens (`adapter.lens()`) are never extracted or registered. This is a known gap (product discovery 2026-04-02, conformance debt).

2. **No runtime loading.** `PlexusApi` receives a fully-built pipeline at construction time. A consumer connecting via MCP (or any transport) cannot declare its identity — there is no API operation to load a spec after startup.

3. **Lens enrichments must persist.** A lens is a reactive enrichment (Invariant 57) that fires in the enrichment loop after each emission. When Consumer B ingests content, Consumer A's lens must translate it. This requires the lens translation rules to persist on the context and be re-registered on startup (Invariant 62).

Product discovery identifies the end-to-end consumer workflow as the acceptance criterion: create context → load spec → ingest → query through lens → load second spec → query across both vocabulary layers. This is the first real MCP consumer workflow.

## Decision

### 1. Add `load_spec` to PlexusApi

```
load_spec(context_id: &str, spec_yaml: &str) -> Result<SpecLoadResult, SpecLoadError>
```

The operation:
1. **Validate** the spec YAML — structural completeness, schema conformance, lens rule consistency. If validation fails, return error. No graph work occurs (Invariant 60).
2. **Parse** the spec into a `DeclarativeAdapter` via `DeclarativeAdapter::from_yaml()`.
3. **Extract** the adapter, declared enrichments (via `adapter.enrichments()`), and lens enrichment (via `adapter.lens()`).
4. **Register** the adapter and enrichments on the pipeline (runtime registration — the pipeline supports adding adapters and enrichments after construction).
5. **Persist** the spec YAML in the context's spec store (SQLite `specs` table).
6. **Run** the lens enrichment against existing graph content to produce the initial vocabulary layer.

`SpecLoadResult` contains: adapter ID, registered enrichment IDs, lens namespace (if any), and the number of vocabulary edges created by the initial lens run.

### 2. Persist specs in SQLite alongside context data

A `specs` table in the same SQLite database used by `GraphStore`:

```sql
CREATE TABLE IF NOT EXISTS specs (
    context_id TEXT NOT NULL,
    adapter_id TEXT NOT NULL,
    spec_yaml TEXT NOT NULL,
    loaded_at TEXT NOT NULL,
    PRIMARY KEY (context_id, adapter_id)
);
```

Keyed by `(context_id, adapter_id)` — one row per spec per context. The same spec can be loaded onto multiple contexts via separate `load_spec` calls. The spec YAML is stored verbatim for re-instantiation on startup.

On startup, `PlexusEngine` (or the pipeline builder) queries the `specs` table for each context and re-registers the adapters and enrichments. The lens enrichment is re-registered but **does not re-run against existing content** — the vocabulary edges from the original `load_spec` call already persist in the graph (effect a is durable). The re-registered lens reacts to new emissions only. This is the persistence mechanism for Invariant 62 (durable enrichment registration).

Two scenarios for spec registration:
- **Initial load** (first `load_spec` call, or first-time `register_specs_from_dir` on a new deployment): all three effects apply — validation, lens runs against existing content, adapter wired, enrichments registered.
- **Startup re-registration** (from `specs` table or file-based auto-discovery after a restart): effect (b) only — adapters and enrichments re-registered. Effect (a) is not repeated because vocabulary edges already persist. Effect (c) is restored (adapter available for routing).

### 3. Three-effect model

Loading a spec has three effects:

- **(a) Durable graph data.** The lens enrichment runs immediately against existing content and writes vocabulary edges. These persist in the graph (Invariant 62).
- **(b) Durable enrichment registration.** The lens translation rules persist in the `specs` table. On startup, the lens enrichment is re-instantiated and registered. It fires in the enrichment loop on all future emissions by any consumer on that context.
- **(c) Transient adapter wiring.** The adapter is available for ingest routing. This lasts for the consumer's workflow. On restart, the adapter is also re-registered from the persisted spec — but the consumer must re-connect to route ingest calls through the transport.

### 4. Fix `register_specs_from_dir` to wire complete specs

`register_specs_from_dir` must call the equivalent of `register_integration` — extracting enrichments and lens from each `DeclarativeAdapter` and registering them alongside the adapter. This fixes the existing gap where spec-declared enrichments and lenses are silently dropped during file-based auto-discovery.

The file-based path and the programmatic `load_spec` path converge on the same internal wiring logic. Both validate, parse, extract, and register. The difference: file-based discovery happens at build time; `load_spec` happens at runtime and additionally persists the spec.

### 5. Runtime registration on the pipeline

`IngestPipeline` must support adding adapters and enrichments after construction. The current `register_adapter` and `register_integration` methods take `&mut self`. With the pipeline behind `Arc` in `PlexusApi`, runtime registration requires interior mutability on the adapter vector and enrichment registry. The specific concurrency mechanism (e.g., `RwLock`, `DashMap`) is a BUILD-time decision.

### 6. Unload spec

```
unload_spec(context_id: &str, adapter_id: &str) -> Result<(), SpecUnloadError>
```

Removes the adapter from routing and the spec from the `specs` table. Does **not** remove vocabulary edges from the graph — they are durable graph data (Invariant 62). The vocabulary layer remains queryable; the lens enrichment stops reacting to new emissions; the adapter stops accepting ingest calls.

Unloading is the inverse of loading for the transient and durable-registration effects only. Graph data is permanent.

**Disconnect vs. unload:** A consumer that simply disconnects from the transport retains its durable effects — the lens remains registered (persisted in `specs` table) and continues translating new emissions from other consumers. A consumer that explicitly calls `unload_spec` deregisters the lens — it stops reacting to new emissions. After unload, the vocabulary layer (edges already written) persists and is queryable by all consumers, but no new translations occur. A consumer that wants to maintain reactive translation indefinitely should not call `unload_spec`.

### Out of scope

**Spec versioning.** If a consumer loads an updated version of the same spec (same adapter ID), the new spec replaces the old one in the `specs` table and the adapter/enrichment wiring is updated. No version history is maintained. Vocabulary edges from the old lens persist in the graph (Invariant 62). If the new lens uses different translation rules, old vocabulary edges become orphaned — they exist but no longer correspond to the active lens rules. The consumer can clean up old edges via `retract_contributions` (ADR-027) using the old lens's contribution keys before loading the new spec. The update path is: retract old contributions → `load_spec` with new spec.

**Cross-context specs.** A spec is loaded onto one context. Loading the same spec onto multiple contexts requires separate `load_spec` calls. No cross-context spec management.

**Spec content delivery.** How the spec YAML arrives at the API (file upload, inline in an MCP tool call, embedded in application code) is a transport concern, not an API concern.

## Consequences

**Positive:**
- Consumers can declare their identity at interaction time, not just deployment time
- The initial lens run (effect a) produces `EdgesAdded` graph events that appear in the cursor stream (ADR-035). A consumer polling via `changes_since` after another consumer's `load_spec` will see bulk `EdgesAdded` events with `lens:{consumer}:*` relationship types — this is the lens's initial vocabulary layer creation, distinct from incremental enrichment
- The complete spec (adapter + enrichments + lens) is wired atomically — no silent omissions
- Lens enrichments persist and react to all future emissions, growing vocabulary layers over time
- File-based auto-discovery and programmatic loading converge on the same wiring logic
- The end-to-end MCP consumer workflow becomes possible

**Negative:**
- Runtime registration adds concurrency complexity to `IngestPipeline` (currently immutable after construction)
- The `specs` table adds a new persistence concern alongside contexts and events
- Startup time increases with number of persisted specs (re-instantiation of adapters and enrichments — not a full lens re-run, since vocabulary edges already persist)

**Neutral:**
- `unload_spec` removes routing and registration but not graph data — consumers must understand that vocabulary layers are permanent
- Spec validation duplicates some work already done by `DeclarativeAdapter::from_yaml()` — the upfront validation gate (Invariant 60) may add checks beyond what `from_yaml` currently validates
