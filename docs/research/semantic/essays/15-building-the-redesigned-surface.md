# Building the Redesigned Surface: From Design to Working API

Essay 14 described the public surface that emerged from analyzing what consumers need: a transport-independent API layer, a typed multi-hop traversal for cross-dimensional queries, and a workflow-oriented write surface that hides graph primitives behind user intent. Three ADRs captured the decisions (013, 014, 015). A fourth (008) prescribed eliminating the `__provenance__` context — a historical artifact that isolated provenance marks from semantic nodes and prevented the cross-dimensional connections that are the whole point of the system. This essay describes what happened when those decisions were built.

## What Was Built

The build phase produced 6 commits across four ADRs (008, 013–015), growing the test suite from 245 to 272 tests. The MCP tool count went from 19 (pre-redesign) to 14, with the remaining tools doing more per call and routing through the correct architectural layers.

### StepQuery: The Missing Primitive (ADR-013)

The gap identified in Essay 14 was structural: the query system had no way to express typed multi-hop traversals. `TraverseQuery` does BFS with one optional relationship filter; cross-dimensional queries need different relationships at each hop.

`StepQuery` fills this gap as a builder:

```rust
StepQuery::from(concept_id)
    .step(Direction::Incoming, "references")   // marks
    .step(Direction::Incoming, "contains")     // chains
    .execute(&context)
```

Each step follows edges matching the specified relationship and direction from the current frontier, then uses the discovered nodes as the next step's frontier. The result preserves per-step structure — you know which nodes were discovered at which depth, not just a flat list.

Nine tests cover `StepQuery`: single step, multi-step chaining, empty results, frontier propagation (step N's results feed step N+1), outgoing direction, and the three evidence trail composition tests.

### Evidence Trail: The Composite (ADR-013)

`evidence_trail(node_id)` composes two `StepQuery` branches:

- Branch 1: concept ← marks (via `references`) ← chains (via `contains`)
- Branch 2: concept ← fragments (via `tagged_with`)

The results merge into an `EvidenceTrailResult` with typed fields: marks, fragments, chains, and all traversed edges. This is the query that makes the multi-dimensional graph useful — the single question all four representative consumers converge on.

`evidence_trail` lives in `PlexusApi`, not in the query system. It's a convenience that composes query primitives, not a primitive itself. If the graph schema changes (new relationship types, new dimensions), the convenience updates but `StepQuery` doesn't.

### PlexusApi: The Single Entry Point (ADR-014)

Before this cycle, the MCP server reached directly into `ProvenanceApi`, `PlexusEngine`, and the query system. Each new transport would need to rediscover which methods to call and how to compose them.

`PlexusApi` formalizes the consumer-facing surface as a single struct. All transports call `PlexusApi` — they never reach past it. The operations:

**Writes:** `ingest()`, `annotate()` (composite — see below)

**Provenance reads:** `list_chains()`, `get_chain()`, `list_marks()`, `list_tags()`, `get_links()`

**Graph reads:** `evidence_trail()`, `find_nodes()`, `traverse()`, `find_path()`

**Mutations (non-ingest):** `update_mark()`, `archive_chain()`

**Destructive writes:** `delete_mark()`, `delete_chain()`, `link_marks()`, `unlink_marks()` — all routing through the ingest pipeline

**Context management:** `context_create()`, `context_delete()`, `context_list()`, `context_rename()`, `context_info()`, `context_list_info()`

Seven API-level tests verify delegation: provenance reads route to ProvenanceApi, graph queries route to the query system, ingest routes to IngestPipeline, non-ingest mutations route through ProvenanceApi, and `list_tags` is context-scoped (ADR-014's explicit scoping correction to ADR-012).

`PlexusApi` derives `Clone` — it holds `Arc` references to engine and pipeline, so clones share state. This matters for the MCP server, which clones the handler per connection.

### Annotate: The Workflow (ADR-015)

`annotate` replaces `create_chain` + `add_mark` as a single operation. The consumer says "annotate this file location, in this chain, with these tags." If the chain doesn't exist, it's created automatically.

Chain name normalization produces deterministic IDs: `"Field Notes"` → `chain:provenance:field-notes`. Lowercased, whitespace to hyphens, colons and slashes (ID format separators) to hyphens, non-ASCII preserved. Empty and whitespace-only names are rejected before any ingest call.

`annotate` is a `PlexusApi` composite, not a single ingest call. It resolves the chain name, checks existence, optionally creates the chain via `ingest("provenance", CreateChain{...})`, then creates the mark via `ingest("provenance", AddMark{...})`. Events from both calls are merged into a single return value.

Five tests cover annotate: chain-and-mark creation, chain reuse on second call, enrichment loop triggering (TagConceptBridger creates cross-dimensional edges for tagged marks), merged outbound events, and empty name rejection. Two tests cover normalization: case-insensitive determinism and special character handling.

### Destructive Operations Through Ingest (ADR-015)

Four operations that previously called ProvenanceApi directly now route through the ingest pipeline:

- `delete_mark()` — `ProvenanceInput::DeleteMark`
- `delete_chain()` — `ProvenanceInput::DeleteChain` (pre-resolves contained mark IDs)
- `link_marks()` — `ProvenanceInput::LinkMarks` (validates both endpoints exist)
- `unlink_marks()` — `ProvenanceInput::UnlinkMarks`

Routing through ingest means these operations get contribution tracking, enrichment loop execution, and outbound events — the "dual obligation" from Essay 12. `link_marks` returns a typed `LinkError` (with `MarkNotFound` variant) rather than a generic error. `delete_chain` returns `DeleteChainError` with `ChainNotFound`.

### Session Context: Eliminating __provenance__ (ADR-008)

The MCP server's `__provenance__` context was the last remnant of the pre-integration era. It hard-coded all 13 provenance tools to a single global context, preventing marks from living alongside semantic nodes in project contexts — which defeats cross-dimensional bridging.

The fix: a `set_context` tool sets the active context for the MCP session. All tools read from `active_context: Arc<Mutex<Option<String>>>` instead of a constant. If no context is set, tools return an `INVALID_REQUEST` error directing the caller to use `set_context` first. `set_context` auto-creates the context if it doesn't exist, matching the auto-creation pattern from `annotate`.

The `__provenance__` constant, the bootstrap block in `new()`, and the `Context` import were removed. The `context()` helper is four lines.

### MCP Surface Reduction

The MCP server went from 19 tools (pre-redesign) through an intermediate state to 14 tools:

- 6 context management tools removed (commit `05e2d41`) — context operations are API-level, not MCP-level
- `create_chain` absorbed into `annotate` (ADR-015)
- `add_mark` replaced by `annotate` (ADR-015)
- `evidence_trail` added (ADR-013)
- `set_context` added (ADR-008)

The remaining 14: `set_context`, `annotate`, `list_chains`, `get_chain`, `archive_chain`, `delete_chain`, `update_mark`, `delete_mark`, `list_marks`, `link_marks`, `unlink_marks`, `get_links`, `list_tags`, `evidence_trail`.

## What Was Validated

**The typed traversal primitive was the right abstraction.** `StepQuery` is 150 lines including tests. It replaced the hand-written `ctx.edges().filter()` traversals from Essay 13 with a composable builder that any query can use. The `evidence_trail` convenience composes two `StepQuery` calls — proof that the primitive is general enough.

**The API layer simplifies transports.** The MCP server went from reaching into three separate subsystems (ProvenanceApi, PlexusEngine, query system) to calling one struct. Each MCP tool handler is now a thin translation: parse params, call `self.api.method()`, format the result. No domain logic in the transport.

**Auto-chain creation works.** The `annotate` workflow handles both the first-call (create chain + mark) and subsequent-call (reuse chain, create mark) cases cleanly. The enrichment loop runs after annotate — a mark tagged "refactor" in a context containing `concept:refactor` gets a `references` edge automatically.

**Session context eliminates the isolation problem.** Marks created via `set_context("research")` → `annotate(...)` live in the "research" context alongside semantic nodes. TagConceptBridger can bridge between them. The `__provenance__` silo is gone.

**PlexusError::Other was sufficient.** `context_create` and `context_rename` needed to reject duplicates. Rather than adding specialized error variants for a simple string message, `PlexusError::Other(String)` handled it. One variant, two call sites. No over-engineering.

## What Was Deferred

**ingest_fragment MCP tool.** ADR-015 describes it. FragmentAdapter exists and works. The MCP tool wrapping it hasn't been built — no consumer needs it via MCP yet. When Trellis or Carrel connects, this becomes a single-function addition.

**find_nodes / traverse / find_path MCP tools.** PlexusApi exposes all three graph query methods. The MCP server doesn't surface them. `evidence_trail` is the only graph read in the MCP surface. Generic graph queries via MCP are a consumer-driven addition.

**update_chain.** ADR-014 lists it as a provenance mutation. Not implemented — no consumer has needed to modify chain metadata after creation.

**OQ8 — Event streaming.** Outbound events remain synchronous. No persistence, no cursor-based delivery.

**OQ9 — Wire protocol schema.** MCP is the only transport. The protobuf schema for gRPC hasn't been designed.

## What Surprised Us

**The committed tree was broken.** The MCP server's tool methods had been committed calling `self.api.delete_chain()` and `self.api.link_marks()`, but the `PlexusApi` methods they call hadn't been committed yet. The build phase was partially applied across sessions — some commits landed, others didn't. This is the cost of multi-session development with uncommitted work: the artifact boundary (a compilable commit) was violated.

**context_create needed existence checks.** The original `context_create` was a thin wrapper around `engine.upsert_context()` — idempotent but silent on duplicates. `set_context`'s auto-creation pattern needs to distinguish "context exists" from "context created." Adding `PlexusError::Other` and an existence check was three lines, but the need only surfaced when the MCP session model was implemented.

**The MCP surface count matters less than the routing.** The reduction from 19 to 14 tools is less important than the architectural change: every tool now calls `PlexusApi`, which calls the right subsystem. The transport is truly thin. Adding a gRPC transport means writing 14 handler functions that call the same `PlexusApi` methods — no new domain logic.

## Test Suite

272 tests, zero failures. Growth by build phase:

| Phase | Tests | Cumulative |
|-------|-------|------------|
| Adapter architecture (ADRs 001, 003, 005) | 57 | 57 |
| First adapter pair (ADR-004) | 140 | 197 |
| Runtime architecture (ADRs 006–009) | 21 | 218 |
| Public surface (ADRs 010–012) | 27 | 245 |
| Public surface redesign (ADRs 008, 013–015) | 27 | 272 |
