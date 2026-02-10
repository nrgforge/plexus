# Research Log: Plexus Runtime Architecture

## Prior Research (First Adapter Pair)

See `docs/research/semantic/essays/07-first-adapter-pair.md` for the previous research cycle. That work produced FragmentAdapter, CoOccurrenceAdapter, and the normalization floor (ADR-004, ADR-005). The adapter layer is built and tested (197 tests) but has no external surface.

*Note: The previous research log was overwritten before we established the archive-before-new-cycle practice. The essay preserves the findings; the process notes are lost. Future cycles will archive `research-log.md` to `docs/research/semantic/logs/` before starting fresh.*

---

## Question 1: What is the current state of Plexus's MCP server, storage, and adapter layer — and where are they connected vs. disconnected?

**Method:** Code trace — read all files in `src/mcp/`, `src/storage/`, `src/bin/`, and `src/adapter/mod.rs`.

**Findings:**

### Architecture as-built

```
MCP Client (Claude Code)
  ↓ JSON-RPC over stdio
PlexusMcpServer (19 tools: chains, marks, links, contexts)
  ↓ delegates to
ProvenanceApi + PlexusEngine (in-memory DashMap<ContextId, Context>)
  ↓ persists on upsert
SqliteStore (contexts, nodes, edges tables)
  ↓
.plexus.db

SEPARATE, NOT CONNECTED:
Adapter Layer (EngineSink, ProposalSink, FragmentAdapter, CoOccurrenceAdapter)
  → writes to Arc<Mutex<Context>> in test harness only
  → no path to PlexusEngine or SqliteStore
  → no MCP tools expose it
```

### Key discovery: marks ARE graph nodes

Marks, chains, and links are not a separate storage system. They are regular graph nodes and edges stored in a special `"__provenance__"` context using the provenance dimension. The MCP tools (`add_mark`, `create_chain`, `link_marks`) are thin wrappers around `Context.add_node()` and `Context.add_edge()` calls via `PlexusEngine`.

- Chain = Node with `node_type: "chain"`, `dimension: "provenance"`
- Mark = Node with `node_type: "mark"`, `dimension: "provenance"`, properties: file, line, annotation, tags, type
- Link = Edge with `relationship: "links_to"` between marks
- Contains = Edge with `relationship: "contains"` from chain to mark

### Historical context

The marks/chains/links system descends from "clawmarks," a separate MCP server for provenance tracking. It was integrated into the Plexus MCP server. Earlier design docs (PAPER.md, SYSTEM-DESIGN.md, PLEXUS-PAPER.md) reference clawmarks as a separate system in a three-system pipeline (llm-orc → clawmarks → Plexus). The original vision had clawmark IDs stored as properties on graph nodes, bridging provenance to the knowledge graph. That bridge was never built.

### Five gaps identified

1. **Adapter layer bypasses PlexusEngine.** Adapters write to `Arc<Mutex<Context>>` directly. The engine handles persistence (`upsert_context()` → `store.save_context()`), but adapters don't go through it. Nothing an adapter emits reaches SQLite.

2. **No MCP ingestion surface.** The 19 MCP tools cover provenance (marks/chains/links) and workspace management (contexts/sources). No tool accepts external data for adapter processing (e.g., `ingest_fragment`).

3. **Contributions not persisted.** `Edge.contributions: HashMap<AdapterId, f32>` is not stored in SQLite — only `raw_weight` is. After restart, all per-adapter contribution data is lost and `recompute_raw_weights()` cannot run. This is a known gap, not a bug — the SQLite schema predates the contribution system (ADR-003).

4. **`__provenance__` context is isolated.** Marks live in the provenance context. Adapter-produced nodes (fragments, concepts) would live in a different context. No cross-context query mechanism exists. A mark tagged "travel" and a concept node `concept:travel` have no connection.

5. **Context sources are metadata-only.** `context_add_sources()` records file/directory paths in `ContextMetadata.sources` but doesn't trigger analysis, file scanning, or adapter processing.

**Implications:**

The two surfaces are closer than we assumed — same storage model, same engine. But the adapter layer is completely disconnected from the runtime system. To serve external consumers (Trellis, Carrel), we need to:
- Route adapter emissions through PlexusEngine (not bare Context)
- Persist contributions in SQLite
- Expose adapter ingestion via MCP (or another surface)
- Decide whether marks and adapter-produced nodes should cross-reference

The marks-as-nodes architecture is actually a strength — it means unifying the systems doesn't require a storage migration, just wiring.

## Question 2: Can the adapter layer route emissions through PlexusEngine instead of bare Arc<Mutex<Context>>, and what changes are needed?

**Method:** Spike — code trace through EngineSink, PlexusEngine, and SqliteStore to identify the concurrency model mismatch and minimum wiring changes.

**Findings:**

### The concurrency models are compatible

EngineSink uses `Arc<Mutex<Context>>` for exclusive access. PlexusEngine uses `DashMap<ContextId, Context>` with `get_mut()` returning a `RefMut` that holds a shard lock. Both give exclusive `&mut Context` access — just through different mechanisms.

Critically, `EngineSink::emit()` does synchronous work inside an `async fn` — no `.await` points during mutation. This means a DashMap `RefMut` (which is `!Send`) works fine: acquire, mutate, persist, drop, all within one synchronous block.

### What EngineSink does that PlexusEngine doesn't

EngineSink handles the entire adapter protocol:
- **Edge endpoint validation** — rejects edges with missing source/target
- **Contribution tracking** — sets `edge.contributions[adapter_id] = value` (ADR-003)
- **Change detection** — compares old vs new contribution to decide whether to fire `WeightsChanged`
- **Scale normalization** — calls `ctx.recompute_raw_weights()` after edge commits (ADR-003/005)
- **Graph events** — fires `NodesAdded`, `EdgesAdded`, `NodesRemoved`, `EdgesRemoved`, `WeightsChanged`
- **Provenance construction** — builds `ProvenanceEntry` records from `FrameworkContext` + adapter annotations
- **Removal with cascade** — removes node and all connected edges

PlexusEngine's `add_node()`, `add_edge()`, and `apply_mutation()` are thin wrappers around `Context.add_node()`/`add_edge()` + persist. They don't know about contributions, events, or validation.

### What PlexusEngine does that EngineSink doesn't

PlexusEngine handles persistence and context lifecycle:
- **Persistence** — calls `store.save_context()` after every mutation
- **Context lookup** — maps `ContextId` → `Context` via `DashMap`
- **GraphStore abstraction** — `Option<Arc<dyn GraphStore>>` supports BYOS
- **Load/hydrate** — `load_all()` populates DashMap from SQLite on startup

### Confirmed gap: contributions are not persisted

`SqliteStore::row_to_edge()` (sqlite.rs:246) hard-codes `contributions: HashMap::new()`. `edge_to_row()` doesn't include contributions in its output tuple. The `edges` table has no contributions column. After save → load, all per-adapter contribution data is lost and `recompute_raw_weights()` would produce incorrect results (no contributions to normalize).

### Integration path: EngineSink takes Arc<PlexusEngine>

The minimum viable wiring:

1. **EngineSink gains a second constructor:**
   ```
   EngineSink::for_engine(engine: Arc<PlexusEngine>, context_id: ContextId)
   ```
   In `emit()`, instead of `self.context.lock()`, it calls `engine.contexts.get_mut(&context_id)` to get a `RefMut<Context>`. All validation/contribution/event logic is identical — it operates on `&mut Context` either way. After mutations, call `engine.store.save_context()` before dropping the `RefMut`.

2. **The existing `Arc<Mutex<Context>>` constructor stays** for tests. No adapter code changes. The `AdapterSink` trait is unchanged.

3. **PlexusEngine may need to expose `contexts` field** (currently private) or provide a `with_context_mut()` accessor. A closure-based accessor is cleaner:
   ```
   engine.with_context_mut(&ctx_id, |ctx| { /* mutate */ })?
   ```
   This keeps DashMap internals private and handles persistence automatically.

### Schema migration for contributions

Following the dimension migration pattern (sqlite.rs:108-161):

1. Add `contributions_json TEXT NOT NULL DEFAULT '{}'` column to `edges` table
2. `edge_to_row()` serializes `edge.contributions` as JSON
3. `row_to_edge()` deserializes contributions from the column
4. Migration function checks `pragma_table_info('edges')` for the column before ALTER TABLE

This is a backward-compatible change — existing edges get an empty contributions map, which matches their current behavior.

### What doesn't need to change

- **Adapter trait** — unchanged. Adapters call `sink.emit(emission)`.
- **AdapterSink trait** — unchanged. The sink interface is the same.
- **ProposalSink** — wraps any AdapterSink, unchanged.
- **FragmentAdapter, CoOccurrenceAdapter** — unchanged. They don't know about persistence.
- **All 197 existing tests** — continue using `Arc<Mutex<Context>>` constructor.

### Open question surfaced: persist-per-emission vs batch persist

PlexusEngine currently persists after every `add_node()` / `add_edge()` call. EngineSink processes entire emissions (multiple nodes + edges + removals) atomically. Two options:

- **Persist per emission** — save_context once at the end of each `emit()` call. Simple, atomic per-emission. This is the natural fit.
- **Persist on demand** — adapter calls multiple `emit()`, then explicitly triggers persist. More efficient for batch ingestion, but requires new API.

Persist-per-emission is the right default. Batch optimization can come later if needed.

**Implications:**

The wiring is straightforward — no fundamental architectural changes needed. The adapter layer's `AdapterSink` abstraction was well-designed: the sink is the integration point, and swapping its backing from `Arc<Mutex<Context>>` to `PlexusEngine` is a local change to `EngineSink`. The contribution persistence gap requires a schema migration, but it follows an established pattern.

Total estimated changes:
- `EngineSink`: new constructor + `emit()` persistence call (~30 lines)
- `PlexusEngine`: `with_context_mut()` accessor (~15 lines)
- `SqliteStore`: contributions migration + serialize/deserialize (~40 lines)
- New integration tests for persistence round-trip (~50 lines)
