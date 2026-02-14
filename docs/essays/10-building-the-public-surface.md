# Building the Public Surface: From Architecture to Working Pipeline

Essay 09 described the architecture that emerged from examining what consumers need, how protocols should work, and where graph-level intelligence belongs. The answer was three independent extension points — adapters for domain translation, enrichments for reactive graph intelligence, transports for protocol shells — unified by a single ingest pipeline. This essay describes what happened when that architecture was built.

## What Was Built

The build phase produced 15 commits across three ADRs (010–012), growing the test suite from 218 to 245 tests. Every provenance write operation now routes through the ingest pipeline. The MCP layer — Plexus's only current transport — delegates all graph mutations to adapters.

### Enrichment Trait and Loop (ADR-010)

The enrichment loop is the mechanism that makes the graph reactive. After every primary emission, registered enrichments receive the events and a context snapshot. If they have work, they return an `Emission`; if not, `None`. The loop runs rounds until quiescence — all enrichments return `None`.

Two enrichments were built:

**TagConceptBridger** — when a mark is created with tags in a context containing matching concept nodes, creates cross-dimensional `references` edges from provenance to semantic dimension. Also works in the reverse direction: when a concept node is created via FragmentAdapter, bridges to existing marks with matching tags. Idempotent — checks whether the edge already exists before emitting.

**CoOccurrenceEnrichment** — migrated from the former `CoOccurrenceAdapter`. Same Hebbian co-occurrence detection algorithm; different trigger model. Instead of requiring a schedule monitor to poll on a timer (which was never built), it responds to graph events in the enrichment loop. Self-caps contributions and emits only `may_be_related` edges, preserving the propose-don't-merge principle as a design convention rather than a structural enforcement mechanism.

The migration removed `CoOccurrenceAdapter`, `ProposalSink`, and the schedule-based trigger model. One concept (enrichment) replaced three unbuilt or partially-built ones (reflexive adapter, ProposalSink, schedule monitor).

### Bidirectional Adapter (ADR-011)

The `Adapter` trait gained an outbound method: `transform_events(&self, events: &[GraphEvent], context: &Context) -> Vec<OutboundEvent>`. The default returns an empty vec — backward compatible. Adapters that serve interactive consumers override it to translate raw graph events into domain-meaningful feedback.

`OutboundEvent` is deliberately unstructured: `{ kind: String, detail: String }`. The consumer defines what kinds it cares about. The adapter filters and translates. The framework stays domain-agnostic.

### Unified Ingest Pipeline (ADR-012)

`IngestPipeline` is the single write endpoint: `ingest(context_id, input_kind, data) -> Vec<OutboundEvent>`. The pipeline:

1. Routes to adapter(s) by `input_kind`
2. Adapter processes input via `EngineSink` — graph mutations committed, events fired
3. Enrichment loop runs until quiescence
4. Adapter transforms all accumulated events into outbound events
5. Returns outbound events to caller

Registration bundles an adapter with its enrichments:

```rust
pipeline.register_integration(
    Arc::new(ProvenanceAdapter::new()),
    vec![
        Arc::new(TagConceptBridger::new()),
        Arc::new(CoOccurrenceEnrichment::new()),
    ],
);
```

Enrichments shared across integrations are deduplicated by `id()`.

### ProvenanceAdapter

Provenance operations — the MCP server's primary workload — route through the pipeline as just another kind of domain input. `ProvenanceAdapter` handles six `ProvenanceInput` variants:

- **CreateChain** — emits chain node in provenance dimension
- **AddMark** — emits mark node + "contains" edge in single emission
- **LinkMarks** — emits "links_to" edge between marks
- **DeleteMark** — emits node removal (cascade removes connected edges)
- **UnlinkMarks** — emits targeted edge removal (new `EdgeRemoval` type)
- **DeleteChain** — emits removal of chain node and all contained marks

The last two required resolving OQ10 — edge removal in `Emission`. `Emission` gained an `edge_removals` field containing `EdgeRemoval` structs (source, target, relationship triple). The engine's `emit_inner` processes these as a distinct phase between edge commits and node removals, firing `EdgesRemoved` events with reason "explicit."

### MCP Migration

All six provenance write tools now route through the pipeline:

| Tool | Pipeline route | Notes |
|------|---------------|-------|
| `create_chain` | ProvenanceInput::CreateChain | Caller-generated UUID |
| `add_mark` | ProvenanceInput::AddMark | Boundary validation for chain existence |
| `link_marks` | ProvenanceInput::LinkMarks | Boundary validation for both endpoints |
| `delete_mark` | ProvenanceInput::DeleteMark | — |
| `unlink_marks` | ProvenanceInput::UnlinkMarks | Targeted edge removal |
| `delete_chain` | ProvenanceInput::DeleteChain | Pre-resolved mark IDs |

Seven read tools remain as direct `ProvenanceApi` calls — reads don't need contribution tracking or enrichment. Six context management tools remain as direct engine calls.

The pattern for write tools: MCP handler validates preconditions (chain exists, endpoints exist), generates caller IDs (UUIDs for new chains and marks), constructs `ProvenanceInput`, and calls `pipeline.ingest()`. The adapter trusts pre-validated input and transforms it to graph mutations.

### Tidy: Removing the Redundant Path

With all writes routing through the pipeline, the inline tag-to-concept bridging in `ProvenanceApi::add_mark()` became dead code. Six tests that verified inline bridging were replaced by TagConceptBridger unit tests and the end-to-end integration test. The end-to-end test was updated to route through `IngestPipeline` instead of calling `ProvenanceApi::add_mark()` directly. Net reduction: 138 lines.

## What Was Validated

**The adapter-as-integration-contract works.** Provenance operations — which look nothing like "domain data ingestion" — fit cleanly into the adapter model. CreateChain is just another emission. AddMark is a node + edge in one emission. The pipeline doesn't care what the adapter does internally; it cares that the adapter speaks emissions and the enrichment loop handles cross-dimensional effects.

**The enrichment loop terminates reliably.** TagConceptBridger and CoOccurrenceEnrichment both implement idempotency by checking context state. In every test scenario, the loop reaches quiescence within two rounds.

**OQ10 was solvable within Emission.** Adding `EdgeRemoval` to `Emission` — rather than introducing engine-level escape hatches — kept the architecture clean. All graph mutations, including targeted edge removal, go through the same emit path with contribution tracking and event firing.

**The MCP layer is now a thin transport.** Write tools do boundary validation and ID generation, then delegate. Read tools call the engine directly. The MCP server no longer contains domain logic for how provenance operations affect the graph — that's in the adapter.

## What Was Deferred

**OQ8 — Event persistence and cursor-based delivery.** Outbound events are synchronous with ingest — the caller receives them as the return value. Async delivery (push notifications, event streams, cross-pollination between independent consumers) requires persisting events with sequence numbers and providing a `get_events(context_id, since)` query. The infrastructure for this exists in the literature (Neo4j CDC, Dgraph CDC) but hasn't been built.

**OQ9 — Wire protocol schema.** The protobuf schema for gRPC ingest/query endpoints hasn't been designed. MCP is the only transport. Trellis and Carrel need a wire protocol to push data in without an LLM host.

**Outbound event richness.** The `OutboundEvent` struct is `{ kind, detail }` — intentionally minimal. Real consumers will want structured payloads (node IDs, edge details, provenance trails). The current implementation proves the pipeline works; the outbound vocabulary needs iteration with real consumer feedback.

**Read-modify-write operations.** `update_mark` and `archive_chain` read current state, modify it, and write back. They currently bypass the pipeline (direct ProvenanceApi calls). Routing them through the adapter would require either context read access in the adapter or pre-resolved state passed as input — the same pattern used for `delete_chain`.

## What This Means for Trellis and Carrel

The architecture is ready for real data. The ingest pipeline exists. Adapters transform domain input. Enrichments bridge dimensions. The MCP transport works. What's missing is the domain-specific adapter for each consumer.

### Trellis: The Fragment Path

Trellis produces tagged text fragments — the exact input that `FragmentAdapter` was designed for. The pieces:

1. **FragmentAdapter is already built** — `input_kind: "fragment"`, creates fragment nodes, concept nodes, and `tagged_with` edges from `FragmentInput { text, tags, source, date }`.
2. **TagConceptBridger is already built** — when FragmentAdapter creates concept nodes, the bridger links them to any existing provenance marks with matching tags.
3. **CoOccurrenceEnrichment is already built** — detects concepts that frequently co-occur across fragments and proposes `may_be_related` edges.
4. **The integration registration pattern works** — bundle FragmentAdapter with both enrichments.

What's missing is the transport. Trellis is a Python web application. It can't call `pipeline.ingest()` directly. Options:

- **MCP** — Trellis acts as an MCP client. Technically possible but fights the protocol (no LLM host).
- **gRPC** — define `IngestRequest` in protobuf, serve via tonic, generate a Python client. Production-grade but requires designing the schema.
- **REST** — `POST /ingest` with JSON body. Simplest, good for prototyping. A thin HTTP server wrapping the pipeline.
- **FFI via PyO3** — Plexus as a Python module. Zero serialization overhead. Most work upfront.

For validation with sample data, a REST endpoint or even a CLI tool (`plexus ingest --context provence-research --kind fragment --file fragments.json`) would be sufficient. The pipeline works; the transport is just wiring.

### Carrel: The Provenance Path

> **Superseded.** This section describes Carrel as a provenance-only consumer — marks, chains, and links without semantic content. This framing was wrong: there is no provenance-only path. Every annotation is at minimum a fragment. Carrel's annotations carry text and tags that are semantic content; provenance layers on top. Essay 14 correctly describes Carrel as "a full consumer of the multi-dimensional graph, not just the provenance surface." The description below is preserved for historical context.

Carrel coordinates research across multiple agents and sources. Its primary interaction with Plexus is provenance — creating chains of reasoning, marking significant passages, linking evidence together. This is exactly what the MCP tools already do, and now they route through the adapter pipeline.

What Carrel gains from the pipeline:

- **Contribution tracking** — each agent's marks and links carry per-agent contribution values. When multiple agents mark the same passage, the evidence accumulates.
- **Tag-to-concept bridging** — marks tagged with research concepts automatically connect to concept nodes from ingested literature.
- **Cross-pollination** — Carrel's research marks bridge to Trellis's writing fragments when they share concepts. The graph does this automatically via TagConceptBridger.

### The Sample Data Experiment

The most useful next step is not more infrastructure. It's feeding real data through the pipeline and seeing what the graph produces.

A concrete experiment:

1. Take a small corpus — five or ten of the Provence travel fragments that appear throughout the test fixtures.
2. Ingest them via FragmentAdapter. Observe the concept nodes and `tagged_with` edges.
3. Let CoOccurrenceEnrichment propose `may_be_related` edges between co-occurring concepts.
4. Create a few provenance marks via the MCP tools — mark passages, tag them, link them.
5. Observe TagConceptBridger creating cross-dimensional bridges between marks and concepts.
6. Query the resulting graph: "What evidence supports `concept:travel`?" should traverse from concept to fragments (via `tagged_with`) and from concept to marks (via `references`).

This experiment validates three things: (a) the pipeline handles a realistic volume of related data, (b) enrichments produce useful cross-dimensional connections, and (c) the graph is queryable in a way that surfaces the accumulated knowledge.

The experiment needs no new code — just a way to feed data in. A test harness or CLI tool that reads a JSON file of fragments and calls `pipeline.ingest()` for each one would be enough.

## Architecture Summary

```
Consumer (Trellis, Carrel, Claude Code)
  ↓ domain data
Transport (MCP / future: gRPC, REST)
  ↓ ingest(context_id, input_kind, data)
IngestPipeline
  ↓ routes by input_kind
Adapter (FragmentAdapter, ProvenanceAdapter)
  ↓ process() → Emission → EngineSink
PlexusEngine (DashMap<ContextId, Context>)
  ↓ commit → GraphEvents
Enrichment Loop (TagConceptBridger, CoOccurrenceEnrichment)
  ↓ rounds until quiescence
Adapter.transform_events()
  ↓ outbound events
Transport → Consumer
  ↓ persists on mutation
GraphStore (SqliteStore)
```

Three independent extension points. Adapters extend the domain side. Enrichments extend graph intelligence. Transports extend protocol support. None are coupled. Adding a Trellis integration means writing a FragmentAdapter registration and a transport shell — the engine, enrichments, and persistence are shared infrastructure.

## Test Suite

245 tests, zero failures. Growth by build phase:

| Phase | Tests | Cumulative |
|-------|-------|------------|
| Adapter architecture (ADRs 001, 003, 005) | 57 | 57 |
| First adapter pair (ADR-004) | 140 | 197 |
| Runtime architecture (ADRs 006–009) | 21 | 218 |
| Public surface (ADRs 010–012) | 27 | 245 |
