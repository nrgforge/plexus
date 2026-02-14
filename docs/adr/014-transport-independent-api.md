# ADR-014: Transport-Independent API Layer

**Status:** Accepted

**Research:** [Essay 14](../essays/14-public-surface-redesign.md), [Research Log Q2](../research/research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — transport, ingest, consumer, outbound event

**Depends on:** ADR-011 (bidirectional adapter), ADR-012 (unified ingest pipeline)

---

## Context

Plexus has multiple known consumers (Trellis, Carrel, Manza, Sketchbin among them) that will connect via different transports: MCP, gRPC, REST, direct Rust embedding, or future protocols. Currently the MCP server calls `ProvenanceApi` and `PlexusEngine` methods directly — there is no formal API layer between transports and the engine. This means each new transport must independently discover which engine methods to call and how to compose them.

ADR-012 established that transports are thin shells calling the same `ingest()` and query endpoints. But those endpoints aren't formalized as a single API surface — they're spread across `IngestPipeline`, `ProvenanceApi`, `PlexusEngine`, and the query system.

MCP tool design research (Essay 14) found that tool definitions consume 5–7% of context window, and the "Less is More" principle favors fewer, workflow-oriented tools over granular CRUD. The current 19-tool MCP surface exposes graph primitives (`create_chain`, `add_mark`) rather than user intent. But this is a transport-level concern — the API layer defines what operations exist; transports decide how to present them.

## Decision

Introduce a `PlexusApi` struct that is the single entry point for all consumer-facing operations. All transports call `PlexusApi` methods — they never reach into `ProvenanceApi`, `PlexusEngine`, or the query system directly.

### Operations

**Write:**
- `ingest(context_id, input_kind, data) -> Vec<OutboundEvent>` — the single write endpoint (ADR-012)

**Provenance reads:**
- `list_chains(context_id, status?) -> Vec<ChainView>`
- `get_chain(context_id, chain_id) -> (ChainView, Vec<MarkView>)`
- `list_marks(context_id, filters) -> Vec<MarkView>`
- `list_tags(context_id) -> Vec<String>` (Note: ADR-012 listed `list_tags()` without a context parameter. This ADR scopes it to a context, consistent with all other API operations and with the existing `ProvenanceApi.list_tags()` implementation, which is already context-scoped. Domain model invariant 28 has been updated to match.)
- `get_links(context_id, mark_id) -> (Vec<MarkView>, Vec<MarkView>)`

**Graph reads:**
- `evidence_trail(context_id, node_id) -> EvidenceTrailResult` (ADR-013)
- `find_nodes(context_id, query) -> QueryResult`
- `traverse(context_id, query) -> TraversalResult`
- `find_path(context_id, query) -> PathResult`

**Provenance mutations (non-ingest):**
- `update_mark(context_id, mark_id, changes) -> MarkView`
- `update_chain(context_id, chain_id, changes) -> ChainView`
- `archive_chain(context_id, chain_id) -> ChainView`

**Context management:**
- `context_create(name) -> ContextId`
- `context_delete(name)`
- `context_list(name?) -> Vec<ContextInfo>`
- `context_rename(old_name, new_name)`
- `context_add_sources(name, paths)`
- `context_remove_sources(name, paths)`

`PlexusApi` holds references to `PlexusEngine`, `IngestPipeline`, and `ProvenanceApi`. It delegates to each but presents a unified surface.

### Transport derivation

Each transport maps protocol-specific requests to `PlexusApi` calls. The MCP transport may present operations with workflow-oriented names (ADR-015) but calls the same `PlexusApi` methods underneath. Future gRPC or REST transports define their own protocol mapping but call the same API.

### Alternatives considered

- **Keep transports calling engine/pipeline directly.** The current approach. Works for one transport but doesn't scale — each transport must independently compose the right methods, and there's no single place to add cross-cutting concerns (logging, access control, rate limiting).

- **Trait-based API abstraction.** Rejected for now. A concrete struct is simpler and sufficient. If the API needs multiple implementations (e.g., a mock for testing), a trait can be extracted later.

## Consequences

**Positive:**

- One place to understand all consumer-facing operations — the `PlexusApi` struct
- New transports call `PlexusApi` without knowing about `ProvenanceApi`, `IngestPipeline`, or `PlexusEngine` internals
- Cross-cutting concerns (logging, access control) have a natural interception point
- The MCP server simplifies — it translates MCP tool calls to `PlexusApi` calls, nothing more

**Negative:**

- An additional layer between transports and the engine. For direct Rust embedding, this is one more indirection — though the ergonomic benefit of a unified API likely outweighs the cost.
- `PlexusApi` must be kept in sync with changes to the underlying components. It's a coordination point that can become a bottleneck for changes.

**Neutral:**

- Non-ingest mutations (`update_mark`, `update_chain`, `archive_chain`) remain outside the adapter pipeline. They're read-modify-write operations that don't produce new knowledge. `PlexusApi` routes them directly to `ProvenanceApi`. This is a known design smell (Essay 14) but not something this ADR resolves.
