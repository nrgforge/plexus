# ADR-012: Unified Ingest Pipeline

**Status:** Proposed

## Context

Plexus currently has no external write surface. The adapter infrastructure works but has no endpoint. Consumers cannot push data in. The MCP layer exposes 19 provenance tools but doesn't route through the adapter pipeline — it writes directly to the graph.

Essay 09 found that all consumers follow the same pattern: send domain data → adapter transforms it → graph mutates → events flow back. If all writes go through the adapter pipeline, the public surface collapses from 19 tools to approximately seven: one write endpoint and five or six read queries.

The essay also found that MCP is not the universal protocol — it presupposes an LLM host. App-to-app consumers (Trellis, EDDI) need a wire protocol that doesn't require LLM infrastructure. The solution is to separate the API surface (what operations exist) from the transport (how they're delivered).

## Decision

### Ingest endpoint

A single write endpoint: `ingest(context_id, input_kind, data) -> Vec<OutboundEvent>`.

The pipeline on ingest:

1. Transport receives the request (MCP tool call, gRPC request, REST POST, etc.)
2. Input router matches adapter(s) by `input_kind` — fan-out if multiple match (invariant 16)
3. For each matched adapter: `adapter.process()` commits via sink → primary events
4. Enrichment loop runs once (globally, not per-adapter) until quiescence (ADR-010) → enrichment events
5. For each matched adapter: `adapter.transform_events()` translates all accumulated events → outbound events (ADR-011)
6. Transport returns merged outbound events to consumer

In the common case, one adapter matches and the pipeline is linear. When multiple adapters match the same `input_kind`, steps 3 and 5 fan out. The enrichment loop (step 4) runs once after all primary emissions, operating on the combined events. Each adapter's outbound receives the same full event set and filters independently.

### All writes through ingest

There is no separate public API for raw graph primitives. Consumers say "here is a fragment," not "create node X with edge Y." Provenance operations (create chain, add mark, link marks) go through the adapter pipeline via a provenance input kind — the ProvenanceAdapter transforms them into graph mutations with contribution tracking.

> **Note (Essay 12):** Semantic adapters (FragmentAdapter, future DocumentAdapter) also produce provenance — chain and mark nodes alongside their semantic output. ProvenanceAdapter is not a special case; it handles user-driven provenance (explicit annotations), while semantic adapters produce automatic evidence trails. Both paths produce identical provenance-dimension nodes that participate in tag-to-concept bridging and cross-dimensional traversal.

Two operations (`unlink_marks`, `delete_chain` with cascade) require an edge removal variant in `Emission` that doesn't exist yet (domain model OQ10). Until resolved, these remain engine-level commands.

### Integration registration

An integration bundles an adapter with its enrichments for a consumer:

```
register_integration("trellis",
    adapter: FragmentAdapter,
    enrichments: [TagConceptBridger, CoOccurrenceEnrichment],
)
```

Enrichments shared across integrations are deduplicated by `id()`.

### Transport as thin shell

A transport is any protocol that can accept an ingest request and return outbound events. Transports are thin shells — they don't touch adapters, enrichments, or the engine. All transports call the same `ingest()` and query functions.

Adding a new transport means implementing the protocol shell. It doesn't affect the rest of the system. This is the third independent extension point alongside adapters (domain) and enrichments (graph intelligence).

### Read queries

The read surface is outside the adapter pipeline. Queries go directly to the engine:

- `list_chains(context_id)`
- `list_marks(context_id, filters)`
- `list_tags()`
- `get_chain(context_id, chain_id)`
- `traverse(context_id, start_node, depth)`
- `get_events(context_id, cursor)` (deferred — requires event persistence, OQ8)

## Consequences

**Positive:**

- One write endpoint instead of 19 tools — simpler public surface
- All writes get contribution tracking, scale normalization, provenance, and enrichment for free
- Transport independence: the same API works over MCP, gRPC, REST, WebSockets
- Three independent extension points: adapters (domain), enrichments (graph intelligence), transports (protocol)

**Negative:**

- Provenance operations that don't fit the adapter pipeline (`unlink_marks`, `delete_chain` with cascade) require either an `Emission` removal variant or engine-level escape hatches. This is a known gap (OQ10).
- The ingest pipeline is synchronous — the consumer waits for the full pipeline (process + enrichment loop + outbound transformation) to complete. For burst ingestion, pipelining or async enrichment may be needed later.

**Neutral:**

- The read surface bypasses the adapter pipeline entirely. This is intentional — reads don't need contribution tracking or enrichment. But it means the adapter only controls the write path, not the read path. If consumers need domain-specific query transformations, that's a future concern.
- Event persistence and cursor-based delivery (OQ8) is deferred. The pipeline produces outbound events synchronously; async delivery is layered on later.
