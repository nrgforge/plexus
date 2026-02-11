# Wiring the Graph: Plexus as a Persistent, Connected Runtime

Plexus is a knowledge graph engine that maintains weighted, multi-dimensional relationships between concepts. It has an adapter layer that transforms external data into graph nodes and edges, a provenance layer that tracks human annotations (marks, chains, links), and a persistent storage backend. These three layers share the same data model — nodes, edges, dimensions, contexts — but they are not connected. Nothing an adapter produces reaches persistent storage. Nothing a user marks connects to the concepts the adapters discovered. This essay describes the wiring needed to unify them.

## The Problem: Two Disconnected Surfaces

Plexus presents two faces to the world. The first is an MCP server exposing 19 tools for provenance tracking — creating chains (narrative trails), adding marks (annotated bookmarks at file locations), and linking marks to each other. These tools write to a graph engine backed by SQLite. The second is an adapter layer — a framework for transforming external data (text fragments, co-occurrence analysis) into graph nodes and edges with contribution tracking, scale normalization, and graph events. This layer exists only in tests, writing to an in-memory context that nothing persists and nothing else can see.

The disconnect has a historical explanation. Plexus's provenance tools descend from "clawmarks," a separate MCP server for tracking decisions during code exploration. When clawmarks was integrated into Plexus, its marks became graph nodes in a special `__provenance__` context. The adapter layer was built later, independently, with its own concurrency model and no path to the engine or storage. The result is a system where the two most valuable capabilities — semantic analysis and provenance tracking — cannot interact.

External consumers (Trellis, a document capture system; Carrel, a research coordination tool) hit this wall immediately. Both want to push data into Plexus and query the resulting graph. Neither can, because the adapter layer has no external surface and no persistence.

## Five Gaps

A code trace through the MCP server, adapter layer, storage backend, and engine reveals five specific gaps:

1. **Adapters bypass the engine.** The adapter layer writes to `Arc<Mutex<Context>>` — a bare, mutex-guarded context object. The engine (`PlexusEngine`) manages contexts via a concurrent `DashMap` and handles persistence through a `GraphStore` trait. Adapters don't go through it. Nothing an adapter emits reaches SQLite.

2. **No ingestion surface.** The 19 MCP tools cover provenance and workspace management. No tool accepts external data for adapter processing.

3. **Contributions are not persisted.** Each edge tracks per-adapter contribution values (how strongly each adapter believes in the relationship). The SQLite schema predates this system and stores only the computed `raw_weight`. After restart, contribution data is lost and the normalization that depends on it cannot run correctly.

4. **Provenance is isolated from semantics.** Marks live in a global `__provenance__` context. Adapter-produced concept nodes would live in project contexts. A mark tagged "travel" and a concept node `concept:travel` have no connection despite expressing the same semantic idea.

5. **Context sources are metadata-only.** Adding file or directory sources to a context records them in metadata but triggers no analysis or adapter processing.

Gaps 1 and 3 are wiring problems — the pieces exist but aren't connected. Gap 4 is an architectural problem — a historical artifact preventing the system's core value proposition. Gaps 2 and 5 are feature gaps that resolve naturally once the wiring exists.

## Wiring the Adapter Layer to the Engine

The adapter layer communicates through a trait called `AdapterSink`. Adapters call `sink.emit(emission)` to propose graph changes — nodes, edges, removals. The concrete implementation, `EngineSink`, validates each item, tracks per-adapter contributions, fires graph events, and commits changes to the context. It does significant work that the engine's simpler `add_node()`/`add_edge()` methods don't: endpoint validation, contribution change detection, scale normalization, provenance construction.

The key question was whether `EngineSink` could route through `PlexusEngine` instead of holding a bare `Arc<Mutex<Context>>`. A code trace reveals that the concurrency models are compatible. `EngineSink::emit()` does synchronous work inside an `async fn` — no `.await` points during mutation. `PlexusEngine`'s `DashMap` provides exclusive access via `get_mut()` returning a `RefMut`, which gives the same `&mut Context` that the mutex provides. The swap is local to `EngineSink`: acquire context reference from engine instead of mutex, do all the same validation/contribution/event work, persist at the end.

The integration path:

- `EngineSink` gains a constructor taking `Arc<PlexusEngine>` and a `ContextId`. The existing `Arc<Mutex<Context>>` constructor stays for tests.
- `PlexusEngine` exposes a `with_context_mut()` closure-based accessor that keeps `DashMap` internals private and handles persistence automatically.
- After each emission, the engine persists the context to storage. One persist per emission — atomic, simple, optimizable later if batch ingestion demands it.
- The DashMap path introduces a new error case: the context may not exist (deleted or never created). The existing Mutex path cannot fail to find its context. `EngineSink` needs an error variant for "context not found" — a small addition to the adapter error model.

Nothing changes for adapters. The `AdapterSink` trait is unchanged. `FragmentAdapter`, `CoOccurrenceAdapter`, and `ProposalSink` don't know or care about persistence. The abstraction boundary was well-designed.

### Persisting Contributions

The contribution gap requires a schema migration. The SQLite `edges` table needs a `contributions_json` column. The migration follows the established pattern used for dimension columns: check `pragma_table_info` for the column's existence, `ALTER TABLE ADD COLUMN` if missing, default to `'{}'` for existing edges.

`edge_to_row()` serializes `edge.contributions` as JSON. `row_to_edge()` deserializes it. Existing edges get an empty contributions map, which matches their current behavior — they have no contributions and their `raw_weight` was set directly.

## Connecting Provenance to Semantics

The deeper architectural question: should marks connect to the concept nodes that adapters produce? The answer is yes — this is the core value proposition of a knowledge graph with provenance. Knowledge disconnected from the evidence that supports it is just data. Evidence disconnected from the concepts it relates to is just bookmarks.

### Marks as Graph Nodes, Dimensions as the Bridge

Plexus already has the mechanism for this connection. The dimension system defines six named dimensions — structure, semantic, relational, temporal, provenance, and default. Nodes declare which dimension they live in. Cross-dimensional edges connect nodes across dimensions within the same context. A mark (provenance dimension) connecting to a concept node (semantic dimension) via a cross-dimensional `references` edge is exactly what the dimension system was built for.

The obstacle is that marks currently live in a separate context. The `__provenance__` context is a global singleton auto-created by the MCP server, isolated from project contexts where adapter-produced nodes live. Cross-context edges don't exist. Cross-context queries don't exist. The dimension bridge only works within a single context.

One alternative would be to build cross-context edges — keep marks in `__provenance__` and connect them to concept nodes in project contexts. But this would require cross-context edge storage, cross-context query infrastructure, and cross-context consistency guarantees — none of which exist. Moving marks into project contexts solves the same problem by working with the existing within-context dimension system rather than building new infrastructure.

### Eliminating the `__provenance__` Context

The `__provenance__` context is a historical artifact from when clawmarks was a separate system. It served as a container for marks that had no project affiliation. In a system where marks should connect to project-specific knowledge, it prevents the connection.

The system is new enough that backward compatibility is not a constraint. The `__provenance__` context should be eliminated. Marks should always live in a project context, in the provenance dimension. The `provenance_context()` auto-create helper and its associated logic are removed.

`add_mark` requires a context parameter — no default, no fallback. When a user marks a passage while researching "Provence travel," the mark goes into the provence-research context alongside the concept nodes that FragmentAdapter created. Cross-dimensional edges make the connection traversable.

`list_tags()` queries across all contexts, not just a single provenance context. This is a small change to `ProvenanceApi` — iterate over all contexts, collect tags from all mark nodes. The tag vocabulary remains globally visible, which Carrel's research agent requires.

### Automatic Tag-to-Concept Bridging

The connection between marks and concepts should be automatic, not manual. When `add_mark` is called with tags in a project context, the system checks for concept nodes with matching IDs. A mark tagged `#travel` in a context containing `concept:travel` gets a cross-dimensional `references` edge created automatically. This is the bridge the original clawmarks vision aspired to — automated, first-class graph connections between provenance and semantics.

Tag format normalization is an invariant: strip the `#` prefix from mark tags, prepend `concept:` to match concept node IDs. `#travel` matches `concept:travel`. This convention must be consistent across all mark creation and concept node creation paths.

The bridging happens inline in `ProvenanceApi.add_mark()` at mark creation time. This means bridging is one-directional: if a mark tagged `#avignon` is created before any fragment containing "avignon" has been ingested, no `references` edge is created. The mark exists, unbridged, until a future mechanism (a reflexive adapter scanning for unbridged marks, or a hook on concept node creation) closes the gap. The initial implementation is creation-time only — a known limitation, not a defect.

A reflexive adapter could handle both directions more elegantly (scan for tag-concept matches, propose edges through the adapter pipeline), but that requires a schedule monitor that doesn't exist yet. The inline approach is pragmatic and can be replaced by an adapter later without changing the external behavior.

## What This Enables

With these three changes — adapter-to-engine wiring, contribution persistence, and provenance-semantic connection — the architecture becomes:

```
External Consumer (Trellis, Carrel, MCP Client)
  ↓ MCP tools / adapter ingestion
PlexusMcpServer
  ↓ delegates to
PlexusEngine (DashMap<ContextId, Context>)
  ↓ adapters via EngineSink    ↓ provenance via ProvenanceApi
  ↓ both write to same context, different dimensions
  ↓ cross-dimensional edges connect provenance to semantics
  ↓ persists on mutation
GraphStore (SqliteStore / future: R2Store, RemoteDbStore)
  ↓
.plexus.db (or BYOS)
```

A user researching Provence travel can:
1. Ingest reading fragments → FragmentAdapter creates concept nodes
2. CoOccurrenceAdapter discovers travel↔avignon relationship
3. Mark a passage: "walking through Avignon" tagged `#travel`, `#avignon`
4. Mark automatically connects to `concept:travel` and `concept:avignon`
5. Query: "what evidence supports concept:avignon?" traverses from concept to marks
6. Everything persists across restarts, contributions intact

The `GraphStore` trait supports bring-your-own-storage — the same persistence path works whether the backing store is local SQLite, SQLite synced to R2, or a remote database.

## Estimated Scope

The wiring involves changes to four modules:

| Module | Change | Lines |
|--------|--------|-------|
| `EngineSink` | New constructor for `PlexusEngine`, persist after emit | ~30 |
| `PlexusEngine` | `with_context_mut()` accessor | ~15 |
| `SqliteStore` | `contributions_json` migration + serialize/deserialize | ~40 |
| `ProvenanceApi` | Context-scoped marks, cross-context `list_tags()`, tag-to-concept bridging | ~60 |
| MCP tools | Context parameter on mark tools, remove `__provenance__` | ~30 |
| Tests | Persistence round-trip, cross-dimensional bridging | ~80 |

No changes to: `AdapterSink` trait, `Adapter` trait, `ProposalSink`, `FragmentAdapter`, `CoOccurrenceAdapter`, `Context`, `Edge`, `Node`, dimension constants, normalization, or the 197 existing tests.

## What Remains

Two of the original five gaps are deferred:

- **MCP ingestion surface** (gap 2): Once adapter-to-engine wiring exists, an `ingest_fragment` MCP tool is straightforward — accept text and tags, route through FragmentAdapter via EngineSink. This is a feature, not an architectural question.

- **Context sources triggering adapters** (gap 5): Making `context_add_sources()` trigger file scanning and adapter processing requires a source-watching mechanism. Valuable, but not blocking for the core wiring.

Both are downstream of the work described here and can be addressed once the runtime architecture is solid.

## Build Results

The build phase produced 12 commits implementing all 20 behavior scenarios across ADRs 006–009, growing the test suite from 197 to 218 tests with zero failures.

### What was built

**ADR-006: Adapter-to-engine wiring.** `EngineSink` gained a `SinkBackend` enum — `Mutex` for tests, `Engine` for production — and a `for_engine(Arc<PlexusEngine>, ContextId)` constructor. All emission logic was extracted into `emit_inner(&mut Context, Emission, &Option<FrameworkContext>)`, a static method shared by both paths. `PlexusEngine` gained `with_context_mut()`, a closure-based accessor that acquires the DashMap shard lock, runs the closure, and auto-persists. The existing `Arc<Mutex<Context>>` path is untouched — all 197 prior tests continue using it.

**ADR-007: Contribution persistence.** SQLite gained a `contributions_json TEXT NOT NULL DEFAULT '{}'` column via backward-compatible migration. `edge_to_row` serializes contributions as JSON; `row_to_edge` deserializes them. All edge read paths (load_context, get_edges_from, get_edges_to, save_edge, load_subgraph) were updated. Contributions survive save/load cycles and scale normalization works correctly after restart.

**ADR-008: Project-scoped provenance.** `ProvenanceApi` already accepted a context_id — the API layer was correctly scoped. The build confirmed that marks live in project contexts, no `__provenance__` auto-creation occurs, chains are context-scoped, and a new `list_tags_all(engine)` function aggregates tags across all contexts.

**ADR-009: Tag-to-concept bridging.** `add_mark` now normalizes each tag (strip `#`, lowercase), looks up `concept:<normalized>` nodes in the same context's semantic dimension, and creates cross-dimensional `references` edges (provenance → semantic) for each match. Marks created before concepts exist are not retroactively bridged — this is a documented known limitation, not a bug.

**End-to-end.** A single acceptance test exercises the full pipeline: FragmentAdapter ingestion through EngineSink, provenance chain/mark creation with tag-to-concept bridging, and persistence verification after engine restart. Fragment nodes, concept nodes, tagged_with edges, marks, contains edges, references edges, and contribution values all survive the round trip.

### What was validated

The internal architecture is sound. Adapters, the engine, storage, and provenance share one data model and one persistence path. Cross-dimensional edges work. Contribution tracking works. The adapter layer's `AdapterSink` abstraction proved well-designed — swapping its backing from `Arc<Mutex<Context>>` to `PlexusEngine` was a local change to `EngineSink`.

### What remains

The engine's capabilities exceed its public surface. The MCP layer still routes all provenance operations through a hardcoded `__provenance__` context, contradicting ADR-008. No MCP tool exposes fragment ingestion, graph queries, or contribution visibility. The adapter layer — newly wired and tested — is invisible to external consumers. Trellis and Carrel cannot use what was built until the MCP contract is updated.

This is the next research question: what should the public surface look like?
