# Research Log: Plexus Public Surface

## Prior Research (Runtime Architecture)

See `docs/research/semantic/essays/08-runtime-architecture.md` for the previous research cycle. That work wired the adapter layer to PlexusEngine, persisted contributions in SQLite, scoped provenance to project contexts, and added automatic tag-to-concept bridging (ADRs 006–009). The engine is internally complete — 218 tests, zero failures. But the MCP layer has not been updated to expose these capabilities. External consumers (Trellis, Carrel) cannot use what was built.

*Archived research log: `docs/research/semantic/logs/08-runtime-architecture.md`*

---

## Question 1: What are the core concepts that any Plexus consumer needs — sending data in, receiving events back, knowing which adapters are relevant?

**Method:** Code trace through Trellis paper (fragment model), EDDI/Manza/Carrel essays (consumer diversity), Plexus event system and adapter traits (existing infrastructure), and MCP protocol capabilities.

**Findings:**

### The universal consumer pattern

Four consumers were analyzed: Trellis (creative writing), EDDI (interactive performance), Manza (code analysis), Carrel (research coordination). Despite radically different domains, all follow the same architecture:

```
Domain-specific data → Adapter → Graph mutations → Events → Consumer rendering
```

The graph is domain-agnostic. Interpretation happens at the consumer layer. A `WeightsChanged` event means "show a coaching prompt" to Trellis, "adjust lighting intensity" to EDDI, "update visual opacity" to Manza, and "re-rank evidence relevance" to Carrel. The engine doesn't know or care.

### What every consumer needs (six contracts)

1. **Ingestion contract** — Push domain data in. Adapter transforms it to nodes/edges. Consumer doesn't touch graph primitives directly.
2. **Processing contract** — Per-adapter contribution tracking, scale normalization, reinforcement. Consumer gets fair weight computation automatically.
3. **Storage contract** — State survives restarts. Contributions persist. ACID semantics.
4. **Query contract** — Retrieve nodes by type/dimension/properties. Traverse edges. Trace provenance back to sources.
5. **Event contract** — Typed mutation events emitted when graph changes. Consumer renders domain-specifically.
6. **Validation signal contract** — Domain-specific evidence (test results, user marks, gesture repetitions) reinforces edges through the same contribution mechanism.

### The fragment as universal input

The Trellis paper's "fragment" concept generalizes: an atomic unit of evidence with content, source attribution, tags, and timestamp. Whether it's a writing fragment, a code function, a gesture sequence, or a research passage, the structural pattern is the same — content plus metadata, transformed by an adapter into typed nodes and edges.

### Three gaps in the current public surface

**Gap A: No ingestion surface.** The adapter layer works internally but has no MCP tool to accept data. A consumer cannot push fragments, code, or any domain data into Plexus via the MCP contract. The `AdapterSink.emit()` API exists but isn't exposed.

**Gap B: Events are produced but never delivered.** `GraphEvent` variants (NodesAdded, EdgesAdded, WeightsChanged, etc.) fire during emission and are collected in `EmitResult.events` — then discarded. No listener, no queue, no subscription, no notification mechanism exists. The engine produces events that nobody receives.

**Gap C: No adapter discovery or declaration.** Adapters are instantiated in code. The router matches by `input_kind` string but requires pre-registration. A consumer cannot declare "I produce fragment data; which adapters should process it?" No registry, no configuration, no declarative wiring.

### The adapter-consumer relationship question

The user asked: "Is it a declarative sort of relationship?" Currently no — it's entirely code-level. But the design points toward declarative: adapters already declare `input_kind()` and `id()`. A consumer could declare "I produce data of kind `fragment`" and the system could match adapters automatically. The question is where this declaration lives — in the MCP tool call? In a context configuration? In a separate registration step?

### How events could flow back

Three options within MCP protocol constraints:

1. **Polling** — `get_events(context_id, since_sequence)` tool. Simple, no protocol extensions. Consumer polls periodically. Requires event persistence and sequence numbering.
2. **MCP notifications** — rmcp supports server-initiated notifications. Events could push to connected clients. Requires enabling notification capability and event dispatch infrastructure.
3. **MCP resources** — Events as a subscribable resource (`plexus://context/{id}/events`). Consumer watches for changes. Requires resource infrastructure.

Polling is simplest and works today. Notifications are more useful but require infrastructure. Resources are most MCP-idiomatic but most work.

**Implications:**

The public surface needs three things: an ingestion tool (push data in via adapters), an event mechanism (get mutation events back), and some form of adapter discovery (know which adapters process your data). The ingestion tool is straightforward — the adapter infrastructure exists. The event mechanism requires a design decision about delivery model. Adapter discovery is the most open question — the current system has all the pieces (input_kind matching, router, contribution tracking) but no way to declare or discover the wiring from outside.

## Question 2: Is MCP the right protocol, and where should the integration boundary sit?

**Method:** Protocol audit of MCP specification (2025-11-25), landscape research on knowledge graph API patterns (Neo4j, Qdrant, Weaviate, TinkerPop, Dgraph), and production patterns for Rust-core polyglot integration (Temporal, Qdrant).

**Findings:**

### MCP is designed for LLM-mediated interaction, not app-to-app integration

The MCP spec explicitly models a three-tier architecture: Host (LLM application) → Client (connector) → Server (tool provider). The protocol presupposes an LLM host on the client side — sampling requests, elicitation, model preferences, and human-in-the-loop controls all require LLM infrastructure. A pure Python web service (Trellis) acting as an MCP client would be fighting the protocol.

MCP is the right surface for Claude Code using Plexus interactively — creating marks, querying the graph, orchestrating adapters. It is not the right surface for Trellis pushing 50 fragments per day or EDDI streaming pose data at 30fps.

The protocol does support more than tools: resources (addressable data with URI subscriptions), notifications (server→client push), and async tasks (experimental in 2025-11-25). These could serve event delivery within an MCP session. But the session model is heavyweight (capability negotiation, stateful connection) and MCP's multi-client support requires Streamable HTTP transport (not the stdio transport Plexus currently uses).

### The landscape points to gRPC for app-to-app integration

Production knowledge graph systems with polyglot consumers converge on the same pattern:

- **Qdrant** (Rust core): gRPC as canonical API, REST generated via gateway, typed SDKs per language
- **Weaviate** (Go core): REST + GraphQL + gRPC, clients auto-detect fastest available
- **Neo4j**: Custom binary protocol (Bolt) + official drivers per language
- **Temporal** (Rust core, closest analogue): FFI via PyO3/Neon for tight coupling, but exploring WASM

The practical recommendation from the landscape: define the API in protobuf, serve via tonic (Rust gRPC), generate clients for Python/JS/Go. Add REST via gRPC-Gateway for debugging and ad hoc use.

### Domain operations, not raw graph access

All successful multi-writer graph systems expose domain operations as the primary API — not raw node/edge creation. When Trellis, EDDI, Manza, and Carrel all write to the same graph, raw access creates semantic conflicts and bypasses contribution tracking, scale normalization, and provenance.

The adapter layer is the natural domain boundary. Consumers say "here is a fragment" not "create node X with edge Y." The adapter transforms domain intent into graph mutations with all the machinery (contributions, normalization, events) applied automatically.

Query operations can be more permissive — reading the graph doesn't need the same guardrails as writing. A read-side API can expose graph structure directly.

### Change feed: pull-based cursors are the standard

Neo4j CDC and Dgraph CDC both use the same pattern: maintain an ordered log of mutations, consumers poll with a cursor ("give me changes since sequence N"). This is simple, reliable, and the consumer controls backpressure. Push delivery (Kafka, webhooks) is layered on top of the pull-based log, not instead of it.

Plexus already produces `GraphEvent` variants during emission. The missing piece is persistence and sequence numbering — store events with monotonic IDs, expose a `get_events(context_id, since)` query.

### Two surfaces, not one

The evidence suggests Plexus needs two complementary surfaces:

1. **MCP** — for LLM-mediated, interactive use. Marks, chains, queries, orchestration. Claude Code and similar AI hosts are the clients. This is what Plexus already has (with updates needed for ADR-008).

2. **Wire protocol (gRPC or similar)** — for app-to-app integration. Fragment ingestion, event subscriptions, adapter invocation. Trellis, EDDI, and other applications are the clients. This doesn't exist yet.

The alternative — trying to make MCP serve both purposes — would mean either (a) every consumer needs an LLM host to mediate, or (b) consumers implement a fake MCP client that ignores sampling/elicitation, which is fragile and fights the protocol.

### But the deeper question: what IS the integration boundary?

The adapter trait is already a clean integration contract. An adapter declares its `input_kind`, receives domain data, and emits graph mutations. The question is whether adapters live:

- **Inside Plexus** (baked-in): Plexus ships with FragmentAdapter, MovementAdapter, CodeAdapter, etc. Consumers call a generic `ingest(kind, data)` endpoint. Simple for consumers, but Plexus becomes a monolith.
- **Inside the consumer** (library): Consumers import Plexus as a Rust crate and implement the Adapter trait. Maximum flexibility, but requires Rust or FFI.
- **As separate services** (protocol): Adapters run as gRPC services. Plexus routes data to adapter services by input_kind. Consumers deploy adapters alongside Plexus. Most flexible, most infrastructure.
- **As WASM modules** (portable): Adapters compiled to WASM, loaded dynamically by Plexus. Language-agnostic (any language compiling to WASM), sandboxed, portable. But WASM ecosystem maturity varies.

The cross-pollination value — a mark tagged `#travel` connecting to a concept `concept:travel` from a different app — happens at the graph level regardless of where adapters live. The integration model determines developer ergonomics, not capability.

**Implications:**

MCP is right for its intended use (AI tool integration) but wrong as the universal integration protocol. Plexus likely needs a second surface — probably gRPC — for app-to-app data flow. The adapter trait is the natural API boundary: consumers describe domain data, adapters transform it, the engine handles the graph. The open question is adapter deployment model (baked-in vs. plugin vs. service), which trades off simplicity against extensibility. The cross-pollination value proposition doesn't depend on the integration model — it depends on shared contexts and the graph engine's dimension/contribution system, which already works.

## Spike: Can provenance operations go through the adapter pipeline?

**Question:** Can a single `ingest(context, kind, data)` endpoint replace the current 19-tool MCP surface, with provenance operations modeled as adapter input?

**Method:** Built a `ProvenanceAdapter` implementing the `Adapter` trait, handling `CreateChain`, `AddMark`, `LinkMarks`, `DeleteMark`, and `DeleteChain` as `ProvenanceInput` variants. Ran through `EngineSink` with `FrameworkContext` for contribution tracking. Also sketched a minimal protobuf schema for a unified ingest service.

**Findings:**

### What works: 5 of 9 provenance write operations

All five spike tests pass:

1. **create_chain** — Deterministic ID (`chain:reading-notes`) enables upsert semantics. Chain node created in provenance dimension with proper properties.

2. **add_mark** — Mark node + "contains" edge emitted in single emission. Contribution tracking automatic — the adapter ID ("provenance-claude") appears on the edge's contribution map.

3. **link_marks** — "links_to" edge emitted. Edge merge semantics handle idempotency (re-linking is a no-op, not a duplicate).

4. **delete_mark** — Node removal via emission. Cascade automatically removes connected edges (contains, links_to). Chain survives.

5. **update via upsert** — Re-emitting a node with the same deterministic ID replaces it. `Context.add_node()` uses `HashMap::insert`, which overwrites. This means `update_mark` and `archive_chain` work by re-emitting the node with updated properties — no new Emission variant needed.

### What doesn't work: 2 operations

6. **unlink_marks** — Emission supports node removal (with edge cascade) but not explicit edge removal. Removing a single "links_to" edge without removing either mark requires either a new `Emission` variant or a separate operation outside the adapter pipeline.

7. **delete_chain with marks** — Removing a chain node cascades its edges, but marks become orphaned (they still exist, just disconnected from the chain). Deleting the chain AND its marks requires reading the context to discover which marks belong to the chain, then emitting removals for each. The adapter doesn't have context read access during `process()`.

### Key discovery: tag-to-concept bridging needs context access

The current `add_mark` in `ProvenanceApi` reads the context to look up matching concept nodes for tag-to-concept bridging. The adapter can't do this — `Adapter.process()` receives only the input data and a sink, not the context.

Three options:
- **a) Context snapshot in input** — pass a cloned context as part of `AdapterInput`, like `CoOccurrenceAdapter` already does. Works but expensive for large contexts.
- **b) Post-commit hook in the engine** — bridging happens after the emission commits, as an engine concern. Cleaner separation but requires new infrastructure.
- **c) Bridging stays in the engine/sink layer** — `EngineSink.emit_inner()` checks for matching concepts after committing mark nodes. This keeps the adapter simple and the bridging automatic.

Option (c) is the most natural: the adapter emits what it knows (mark + tags), the engine does what it knows (look up concepts, create references edges). This is consistent with how contribution tracking already works — the adapter doesn't know about it, the engine handles it transparently.

### The protobuf schema is clean

The sketched schema has one write RPC (`Ingest`) and six read RPCs. `IngestRequest` carries `context_id`, `input_kind`, `adapter_id`, and opaque `data` bytes. The response includes commit counts, rejections, and events. This is the entire write surface — every consumer calls the same endpoint.

Read operations (list_chains, get_chain, list_marks, list_tags, traverse, get_events) remain separate RPCs because they're queries, not adapter-mediated writes.

### What this means for MCP

If all writes go through `Ingest`, MCP's role simplifies to:
- A transport for `Ingest` calls (Claude Code sends provenance fragments via MCP tool)
- A transport for queries (Claude Code reads graph state via MCP tools)
- MCP is just one of potentially several transports, not a special API layer

The 19 current MCP tools would collapse to approximately:
- 1 write tool: `ingest(context_id, input_kind, data)`
- 5-6 read tools: `list_chains`, `list_marks`, `list_tags`, `get_chain`, `traverse`, `get_events`

**Implications:**

The adapter pipeline can handle provenance writes. The model works: provenance operations are just another kind of domain input. The two operations that don't fit (unlink_marks, cascade chain deletion) are edge cases that could be solved by adding edge removal to `Emission` or handling them as engine-level commands. The bigger insight is that tag-to-concept bridging is an engine concern, not an adapter concern — the adapter declares what it knows (mark + tags), the engine does the wiring. This suggests a clean separation: adapters handle domain-to-graph transformation, the engine handles cross-dimensional bridging and graph-level invariants.

## Question 3: Where should graph-level enrichment live, and can the adapter be a bidirectional contract?

**Method:** Code audit of EngineSink, CoOccurrenceAdapter, ProvenanceApi, and PlexusEngine enrichment patterns. Spike implementing an enrichment loop with a TagConceptBridger and outbound event transformation.

**Findings:**

### Current enrichment is scattered across three patterns

The codebase handles graph-level enrichment in three inconsistent ways:

1. **Contribution tracking** — transparent, inside `EngineSink.emit_inner()`. Adapter doesn't know.
2. **Tag-to-concept bridging** — inline in `ProvenanceApi::add_mark()`. Reads context directly, adds edges.
3. **Co-occurrence detection** — reflexive adapter (`CoOccurrenceAdapter`). Gets a context snapshot as input.

Three patterns for the same kind of problem: mutations in one dimension triggering effects in another.

### Four options evaluated, two ruled out

**Option A (fat adapters — context snapshot in input):** Adapter handles its own enrichment. Rejected — scatters bridging logic across every adapter, doesn't compose.

**Option B (engine post-commit hooks):** Engine checks enrichment rules after mutations. Rejected — makes the domain-agnostic engine domain-aware, fundamental architectural shift.

**Option C (enrichment as reflexive adapter chain):** Bridging becomes its own adapter. Pros: composable, testable, named. Cons: requires orchestration beyond current router's input_kind dispatch.

**Option D (sink middleware):** Enrichment wraps the sink. Transparent but hidden — harder to discover and debug.

### The enrichment loop works

Five spike tests pass. The loop:

1. Primary emission commits and produces `GraphEvent` variants
2. Each registered `Enrichment` receives events + context snapshot
3. If enrichment returns an `Emission`, it's committed, producing new events
4. Loop repeats until all enrichments return `None` (quiescence)
5. Max rounds as safety limit

**Termination:** The enrichment itself checks context state before emitting. A `TagConceptBridger` looks up whether the cross-dimensional edge already exists — if so, returns `None`. The framework can't guarantee termination (events don't distinguish new from re-emitted); the enrichment author implements the idempotency check. This is the right place for the responsibility — the enrichment knows its own semantics.

### Critical event system observation

`EngineSink` fires `EdgesAdded` for *every* committed edge, including re-emissions of existing edges (upsert). `NodesAdded` fires for every node, including upserts. The events don't distinguish "genuinely new" from "updated." This means enrichments must check context state rather than relying on events alone for idempotency. The enrichment trait signature is:

```rust
trait Enrichment: Send + Sync {
    fn id(&self) -> &str;
    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission>;
}
```

### The adapter as bidirectional contract

The user's key insight: consumers only need to send data to Plexus and receive events back. The adapter defines both sides:

- **Inbound:** `process(input, sink)` — domain data → graph mutations (existing)
- **Outbound:** `transform_events(events, context)` → domain events for consumer

After the enrichment loop completes, raw `GraphEvent` variants are transformed through the adapter's outbound side. The consumer receives domain-meaningful events — never raw graph events.

Spike output for a Trellis-like scenario:
```
concepts_detected: travel, provence
bridges_created: 2 cross-dimensional links
```

The consumer sees "concepts were detected" and "bridges were created" — not "NodesAdded with node_ids [concept:travel, concept:provence]." The adapter is the lens through which both data enters and events exit. This makes the adapter the complete integration contract: when you build a Trellis adapter, you define what data Trellis sends AND what Trellis hears back.

### The concrete scenario that validated this

1. Context has a mark `mark:travel-notes` with tags "travel, provence"
2. FragmentAdapter ingests a Provence essay → creates `concept:travel`, `concept:provence`, fragment node, `tagged_with` edges
3. Enrichment round 1: `TagConceptBridger` sees `NodesAdded` with concept nodes, finds marks with matching tags, creates cross-dimensional `references` edges from provenance → semantic
4. Enrichment round 2: `TagConceptBridger` runs again — edges already exist in context → returns `None` → quiescence
5. All events transformed through `FragmentAdapterOutbound` → consumer receives `concepts_detected` and `bridges_created`

**Implications:**

The enrichment loop is the mechanism for making the graph reactive — mutations in one dimension automatically trigger effects in other dimensions. The adapter as bidirectional contract means consumers never touch graph internals in either direction. Enrichments are composable — register new ones without modifying existing adapters or the engine.

## Question 4: Enrichment registration, outbound event routing, and trait design

**Method:** Design reasoning from spike findings and existing codebase patterns.

**Findings:**

### Enrichment registration: global, self-selecting

Enrichments are registered at the engine level — `PlexusEngine` holds a `Vec<Box<dyn Enrichment>>`. When any adapter emits, the engine runs the enrichment loop with all registered enrichments. Each enrichment self-selects based on the events and context it receives — the `enrich()` method already receives the context, so a code-specific enrichment can inspect context metadata and no-op on non-code contexts.

Global registration avoids per-context configuration complexity for a problem that doesn't exist yet. If selective activation becomes necessary later, enrichments can opt out by inspecting context metadata. Start global, specialize later if needed.

### Outbound event routing: adapter filters, framework passes everything

All events (primary + all enrichment rounds) are passed to the adapter's outbound transform. The adapter filters and transforms based on what its consumer cares about. The framework doesn't guess — the adapter knows its consumer's domain.

This also enables cross-pollination visibility: if Manza creates a concept that bridges to a Trellis mark, Trellis's outbound can surface that event. The filtering logic lives where the domain knowledge lives — in the adapter, not the infrastructure.

### Trait design: adapter is one bidirectional thing, enrichment is separate

The adapter is the complete integration contract — one trait with two sides:

- **Inbound:** `process(input, sink)` — domain data → graph mutations (existing method)
- **Outbound:** `transform_events(events, context)` → domain events for consumer (new method, default returns empty)

This keeps the consumer's mental model simple: one adapter, data in, events out. Adapters that don't need outbound (pure write adapters) use the default empty implementation. Adapters serving interactive consumers (Trellis, EDDI) implement the outbound to surface what their consumer cares about.

Enrichments stay as a separate `Enrichment` trait — they are not adapters. They have no `input_kind`, accept no domain data, and don't serve consumers. They react to graph events and mutate the graph. The separation is conceptual: adapters bridge between a consumer's domain and the graph; enrichments bridge between dimensions within the graph.

Registration bundles the pieces naturally:

```
register_integration("trellis",
    adapter: FragmentAdapter,         // inbound + outbound
    enrichments: [TagConceptBridger], // reactive graph enrichment
)
```

Enrichments shared across integrations (like `TagConceptBridger`) are deduplicated by `id()` — registering the same enrichment twice is a no-op.

**Implications:**

The public surface architecture is now clear. Three flat registries in the engine (adapters, enrichments, outbound is part of adapter). One pipeline on ingest: route → adapter.process() → enrichment loop → adapter.transform_events(). The consumer sends domain data in via the adapter and receives domain events back through the same adapter. The graph's internal reactive behavior (enrichments, cross-dimensional bridging) is invisible to the consumer.

## Question 5: Can reflexive adapters be fully replaced by enrichments?

**Method:** Code exploration + architectural analysis

**Findings:**

The CoOccurrenceAdapter is the only reflexive adapter implementation. The schedule monitor (which would trigger it automatically) was never built — tests invoke it manually. The ProposalSink (which enforces structural constraints) exists in code but its constraints are better expressed as design conventions on enrichments.

Analysis of what changes:
- **Trigger model:** Schedule-based (unimplemented) → event-driven (built into enrichment loop)
- **Constraints:** Structural enforcement via ProposalSink → self-imposed by enrichment convention
- **Termination:** Open question (no convergence guarantee) → solved (quiescence via idempotency)
- **Pipeline position:** Outside ingest pipeline → inside ingest pipeline

What's preserved:
- **Hebbian reinforcement:** The contribution system is independent of whether the emitter is a reflexive adapter or an enrichment. Weak proposals + strong external evidence = same dynamic.
- **Propose-don't-merge:** Survives as a design convention — CoOccurrenceEnrichment self-caps contributions and only emits `may_be_related` edges.

What's removed:
- Reflexive adapter (concept), ProposalSink (concept + code), Schedule and Schedule monitor (concepts), `propose`/`clamp`/`intercept` (actions), 4 invariants (ProposalSink rules)
- Open questions 2 (convergence) and 3 (ProposalSink metadata edges) dissolve

**Decision:** Full migration. Reflexive adapters → enrichments. Applied cleanup to domain model, ADRs 001/004/009, and scenarios 001/004.

**Implications:**

The model is simpler: one concept (enrichment) for all reactive graph intelligence instead of two (reflexive adapter + enrichment). The unimplemented schedule monitor is no longer needed. Two open questions dissolve. The domain model drops from 42 invariants to 38, and from ~65 concepts to ~60.

