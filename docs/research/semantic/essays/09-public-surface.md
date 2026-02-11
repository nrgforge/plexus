# The Adapter as Integration Contract: Plexus's Public Surface

Plexus has a working graph engine. Adapters transform domain data into nodes and edges. Contributions are tracked per-adapter and normalized across scales. Provenance marks connect to semantic concepts through cross-dimensional edges. Storage persists everything to SQLite. 218 tests pass, zero failures. But no external consumer can use any of it. Trellis cannot push fragments. Carrel cannot query the graph. EDDI cannot receive events. The engine is internally complete and externally invisible.

This essay describes the public surface that emerged from examining what consumers actually need, whether the current protocol (MCP) is the right one, and where graph-level intelligence should live.

## What Consumers Need

Four consumers were analyzed: Trellis (creative writing), EDDI (interactive performance), Manza (code analysis), and Carrel (research coordination). Despite radically different domains, all follow the same pattern:

```
Domain-specific data → Adapter → Graph mutations → Events → Consumer rendering
```

The graph is domain-agnostic. A `WeightsChanged` event means "show a coaching prompt" to Trellis, "adjust lighting intensity" to EDDI, "update visual opacity" to Manza, and "re-rank evidence relevance" to Carrel. The engine doesn't know or care. Interpretation happens at the consumer layer.

Every consumer needs six things from Plexus:

1. **Ingestion** — push domain data in. The adapter transforms it; the consumer never touches graph primitives.
2. **Processing** — per-adapter contribution tracking, scale normalization, reinforcement. Automatic.
3. **Storage** — state survives restarts. Contributions persist. ACID semantics.
4. **Query** — retrieve nodes, traverse edges, trace provenance.
5. **Events** — typed mutation events emitted when the graph changes.
6. **Validation signals** — domain-specific evidence (test results, user marks, gesture repetitions) reinforces edges through the same contribution mechanism.

The Trellis paper's "fragment" concept generalizes across all four consumers: an atomic unit of evidence with content, source attribution, tags, and timestamp. Whether it's a writing fragment, a code function, a gesture sequence, or a research passage, the structural pattern is the same. The adapter transforms it into typed nodes and edges.

Three gaps prevent consumers from using this today. There is no ingestion surface — the adapter infrastructure works but has no external endpoint. Events are produced during emission and then discarded — no listener, no queue, no delivery. And there is no way to discover or declare which adapters process what data.

## MCP Is Not the Universal Protocol

The MCP specification models a three-tier architecture: Host (LLM application) → Client (connector) → Server (tool provider). The protocol presupposes an LLM host on the client side — sampling requests, elicitation, model preferences, and human-in-the-loop controls all require LLM infrastructure.

MCP is the right surface for Claude Code using Plexus interactively. It is not the right surface for Trellis pushing 50 fragments per day or EDDI streaming pose data at 30fps. A pure Python web service acting as an MCP client would be fighting the protocol.

Production knowledge graph systems with polyglot consumers converge on a different pattern. Qdrant (Rust core) uses gRPC as its canonical API with REST generated via gateway. Weaviate (Go core) offers REST, GraphQL, and gRPC. Temporal (Rust core, closest analogue to Plexus) uses FFI via PyO3/Neon. The practical recommendation: define the API in protobuf, serve via tonic (Rust gRPC), generate clients for Python/JS/Go.

All successful multi-writer graph systems expose domain operations as the primary API — not raw node/edge creation. When multiple consumers write to the same graph, raw access creates semantic conflicts and bypasses contribution tracking, scale normalization, and provenance. The adapter layer is the natural domain boundary. Consumers say "here is a fragment," not "create node X with edge Y."

For event delivery, the established pattern is pull-based cursors: maintain an ordered log of mutations, consumers poll with a cursor ("give me changes since sequence N"). Push delivery is layered on top, not instead. Plexus already produces `GraphEvent` variants during emission — the missing piece is persistence and sequence numbering.

Plexus needs two complementary surfaces. MCP for LLM-mediated, interactive use — marks, chains, queries, orchestration. A wire protocol (gRPC or similar) for app-to-app integration — fragment ingestion, event subscriptions, adapter invocation. Trying to make MCP serve both purposes would mean every consumer needs an LLM host to mediate, or consumers implement a fake MCP client that ignores sampling and elicitation.

## Everything Goes Through the Adapter

A spike tested whether provenance operations — the 19 current MCP tools — could go through the adapter pipeline as just another kind of domain input. A `ProvenanceAdapter` was built, handling `CreateChain`, `AddMark`, `LinkMarks`, `DeleteMark`, and `DeleteChain` as input variants.

Five of nine provenance write operations work naturally through the pipeline. `create_chain` uses deterministic IDs for upsert semantics. `add_mark` emits a mark node and "contains" edge in a single emission with automatic contribution tracking. `link_marks` emits a "links_to" edge with merge semantics for idempotency. `delete_mark` cascades edge removal automatically. Updates work by re-emitting a node with the same deterministic ID — `Context.add_node()` uses `HashMap::insert`, which overwrites.

Two operations don't fit cleanly. `unlink_marks` requires explicit edge removal, which `Emission` doesn't support. `delete_chain` with cascade requires reading the context to discover which marks belong to the chain, but the adapter doesn't have context read access during `process()`. These are edge cases solvable by adding an edge removal variant to `Emission` or handling them as engine-level commands.

If all writes go through `Ingest`, the public surface collapses from 19 tools to approximately seven: one write endpoint (`ingest(context_id, input_kind, data)`) and five or six read queries (`list_chains`, `list_marks`, `list_tags`, `get_chain`, `traverse`, `get_events`). MCP becomes just one of potentially several transports, not a special API layer.

## The Graph as a Reactive System

The deeper question emerged from the spike: when a mark is created with a tag that matches an existing concept, who creates the cross-dimensional bridge? The current answer is inline code in `ProvenanceApi::add_mark()`. If provenance operations move to the adapter pipeline, that code has no home.

This is a specific instance of a general problem: mutations in one dimension should trigger effects in other dimensions. The codebase already handles this three different ways — contribution tracking is transparent in the sink, tag-to-concept bridging is inline in the API, co-occurrence detection is a reflexive adapter with a context snapshot. Three patterns for the same kind of problem.

Four options were evaluated for where this enrichment logic should live. Fat adapters (each adapter handles its own enrichment) scatter bridging logic everywhere and don't compose. Engine post-commit hooks make the domain-agnostic engine domain-aware. Both were rejected. The remaining options — enrichment as a separate reactive component, or enrichment as sink middleware — trade visibility against transparency.

A spike tested the reactive model: an enrichment loop that runs after each primary emission. Registered `Enrichment` implementations receive the events from the previous round plus a context snapshot. If they have work to do, they return an `Emission`; if not, `None`. The loop repeats until all enrichments return `None` — quiescence.

Five tests validated the mechanics:

1. A `TagConceptBridger` enrichment detects new concept nodes, finds marks with matching tags, and creates cross-dimensional `references` edges.
2. The loop terminates naturally — the second round finds edges already exist and returns `None`.
3. No-op when nothing matches — immediate quiescence.
4. The full pipeline works end-to-end: primary emission → enrichment cascade → outbound event transformation.
5. Multiple marks bridge correctly to the same concept.

Termination relies on the enrichment checking context state before emitting. The event system can't distinguish "genuinely new" from "re-emitted" — `EdgesAdded` fires on every commit including upserts. So the enrichment author implements the idempotency check. This is the right place for the responsibility: the enrichment knows its own semantics, the framework just runs the loop.

The `Enrichment` trait is deliberately separate from `Adapter`. An enrichment has no `input_kind`, accepts no domain data, and doesn't serve consumers. It reacts to graph events and mutates the graph. Adapters bridge between a consumer's domain and the graph; enrichments bridge between dimensions within the graph.

## The Adapter as a Bidirectional Lens

The most significant insight from this research: the adapter should be the complete integration contract. Not just inbound (domain data → graph mutations) but also outbound (graph events → domain events for the consumer).

When the enrichment loop completes, all raw `GraphEvent` variants are transformed through the adapter's outbound side. The consumer receives domain-meaningful events — never raw graph events. In the spike, a Trellis-like scenario produced:

```
concepts_detected: travel, provence
bridges_created: 2 cross-dimensional links
```

Not "NodesAdded with node_ids [concept:travel, concept:provence]." The adapter translates graph internals into the consumer's language. A `WeightsChanged` event becomes "concept X is now more strongly associated" to Trellis, or "adjust lighting for gesture Y" to EDDI.

This makes the adapter the single thing a consumer needs to understand. When you build a Trellis integration, you define what data Trellis sends (inbound) and what Trellis hears back (outbound). The adapter is one trait with two sides:

```rust
trait Adapter: Send + Sync {
    fn id(&self) -> &str;
    fn input_kind(&self) -> &str;

    // Inbound: domain data → graph mutations
    async fn process(&self, input: &AdapterInput, sink: &dyn AdapterSink)
        -> Result<(), AdapterError>;

    // Outbound: raw events → domain events for consumer
    fn transform_events(&self, events: &[GraphEvent], context: &Context)
        -> Vec<OutboundEvent> {
        vec![] // default: no outbound events
    }
}
```

The outbound receives all events — from the primary emission, from all enrichment rounds, from every adapter that contributed. The adapter filters what its consumer cares about. This enables cross-pollination visibility: if Manza creates a concept that bridges to a Trellis mark, Trellis's outbound can surface that event. The filtering logic lives where the domain knowledge lives.

## Architecture

The system has three registries and one pipeline.

**Registries:**

- **Adapters** — registered by `input_kind`. Route domain data inbound, transform events outbound. Each adapter is a bidirectional integration contract.
- **Enrichments** — registered globally on the engine. React to graph events, produce additional graph mutations. Self-select based on events and context. Terminate via idempotency.
- **Transports** — any protocol that can ferry an ingest request in and domain events out. Neither is special; all call the same `ingest()` and query endpoints.

**Pipeline on ingest:**

```
Consumer sends domain data
  → Transport receives (MCP tool call or gRPC request)
    → Router matches adapter by input_kind
      → Adapter.process() commits via sink → events₀
        → Enrichment loop: enrich(events, snapshot) → more events → quiescence
          → Adapter.transform_events(all_events, context) → domain events
            → Transport delivers domain events to consumer
```

Registration bundles the pieces:

```
register_integration("trellis",
    adapter: FragmentAdapter,         // inbound + outbound
    enrichments: [TagConceptBridger], // reactive graph enrichment
)
```

Enrichments shared across integrations are deduplicated by `id()`. The consumer interacts with a high-level API that hides the internal decomposition.

## Three Independent Extension Points

The architecture has a symmetry worth making explicit. There are three dimensions of extensibility, and none are coupled to each other:

**Adapters** extend the domain side. A new consumer with a new data type (gesture sequences, code ASTs, research passages) means a new adapter. The adapter defines how that data becomes graph mutations and how graph events become domain-meaningful feedback. No existing adapter, enrichment, or transport changes.

**Enrichments** extend the graph intelligence side. A new cross-dimensional relationship (code functions referencing design pattern concepts, gesture sequences bridging to musical concepts) means a new enrichment. It registers globally, self-selects based on events and context, and composes with every existing adapter and transport without modification.

**Transports** extend the protocol side. The transport's job is thin — accept an ingest request (context_id, input_kind, data), forward it to the router, return domain events to the consumer. Any protocol that can do request/response works:

- **gRPC** — typed, streaming, generated clients. App-to-app integration.
- **REST** — POST /ingest, GET /events. Debugging, curl, ad hoc use.
- **MCP** — tool calls within an LLM host session. Claude Code.
- **WebSockets** — persistent connection, server-push events. Real-time UIs.
- **WebRTC** — peer-to-peer, low latency. EDDI streaming pose data at 30fps.
- **FFI** (PyO3/Neon) — in-process, zero serialization. Tight coupling.

Adding a new transport means implementing the shell for that protocol. It doesn't touch adapters, enrichments, or the engine — it calls the same `ingest()` and `query()` functions that every other transport calls.

This means the system grows in three orthogonal directions. A new consumer brings an adapter. A new kind of graph intelligence brings an enrichment. A new deployment context brings a transport. Each is independently testable, independently deployable, and invisible to the others.

## What This Means

The adapter is not just a data transformer. It is the integration contract — the single artifact that defines a consumer's relationship with Plexus. Data flows in through it, events flow back out through it, and the consumer never sees graph primitives in either direction.

The graph is reactive. Mutations in one dimension automatically trigger effects in other dimensions through registered enrichments. A mark tagged "travel" bridges to `concept:travel` without anyone asking. A new concept bridges to existing marks without anyone asking. The intelligence is in the graph, not in the caller.

The cross-pollination value — marks from research connecting to concepts from code analysis connecting to fragments from creative writing — happens at the graph level regardless of transport or consumer. Shared contexts and the contribution system handle the wiring. The adapter handles the translation. The enrichment handles the bridging. The consumer just sends data and listens.

What remains is implementation. The `Adapter` trait needs the outbound method. The `Enrichment` trait needs to be built and the loop integrated into the engine. Event persistence needs sequence numbering for cursor-based delivery. And a wire protocol needs to be chosen and implemented for app-to-app consumers. The architecture is clear; the work is construction.
