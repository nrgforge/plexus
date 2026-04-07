# Product Discovery: Plexus

*2026-04-02 (updated from 2026-03-25; MCP consumer interaction cycle)*

## Stakeholder Map

### Direct Stakeholders

**Consumer application developers** — build applications that ingest domain-specific data into a knowledge graph and take action based on the graph's evolving state. They interact with Plexus through an adapter (Rust or YAML spec) and a transport (MCP, gRPC, direct embedding). They need Plexus to handle graph mechanics (contribution tracking, enrichment, persistence, normalization) so they can focus on domain-specific extraction and domain-appropriate responses to structural signals. Actions vary by domain: surfacing latent connections, building outlines from accumulated fragments, coordinating research across shared contexts, or triggering real-time responses to structural changes. The graph itself may or may not be visualized directly. Example consumers from the research corpus: Trellis (creative writing scaffolding), EDDI (interactive performance), Carrel (research coordination), Manza (code analysis), Sketchbin (multimedia metadata).

Consumers own their specs. The adapter spec (including lens and declared enrichments) is part of the consumer application — authored, maintained, and delivered by the consumer. Whether the spec arrives as a file on disk (auto-discovery at startup) or programmatically through the API, Plexus receives and acts on it. Plexus never generates or manages specs on the consumer's behalf.

A context is typically multi-consumer. Multiple consumers may load their specs onto the same context, each encoding their own vocabulary layer through their lens. A consumer connecting to an existing context may encounter other consumers' vocabulary layers already in the graph — this is the normal case, not a special one. Orientation (discovering what vocabularies exist) precedes or accompanies specialization (loading your own spec).

**Domain-specific extractor authors** — write the extraction logic that produces structured JSON from raw source material (scripts, LLM prompts, Rust functions). They don't need to understand graph structure. They write against the extractor contract: raw input in, structured JSON out. The declarative adapter spec (YAML) maps their output to graph structure.

**The engine developer** — maintains and extends Plexus itself. Needs the architecture to be navigable, the module boundaries to be clear, and the test suite to verify invariants. The primary stakeholder for the system design and this product discovery artifact.

### Indirect Stakeholders

**End users of consumer applications** — writers using Trellis, performers using EDDI, researchers using Carrel. They never interact with Plexus directly. They experience it through the consumer application's actions on graph state — surfaced insights, suggested connections, structured outlines, responsive environments. Their needs are mediated by the consumer developer's adapter and application logic.

**Collaborators in federated contexts** — users who share a context across Plexus instances. They contribute data independently; the graph converges through deterministic concept IDs and replication. They experience shared understanding without shared source material. (Federation is designed but not yet implemented.)

## Jobs and Mental Models

### Consumer Application Developer

**Jobs:**
- Ingest my domain's data and get it into the graph with provenance — without learning graph internals
- Receive structural signals back that I can act on in my domain's language (not raw graph mutations)
- Trust that evidence from multiple sources strengthens the signal automatically
- Extend extraction to my domain without writing Rust (via YAML adapter specs and llm-orc ensembles)
- Deploy Plexus in my application's context — as an embedded library, a dev tool, or a service — without Plexus dictating storage location or runtime constraints
- Load my spec onto a context and have Plexus wire the adapter and run the lens enrichment — the same API operation regardless of transport (MCP, gRPC, embedded)
- Ingest through my loaded spec's adapter, potentially over a long-running workflow
- Query the graph on my own schedule — through my lens, another consumer's lens, or the raw underlying relationships
- Discover what vocabulary layers already exist on a context — "what lenses are here, what can I query through?" — before or alongside loading my own spec
- Navigate other consumers' vocabulary layers — "show me this graph through Trellis's vocabulary" when I'm not Trellis
- Discover unexpected connections across domains — things I didn't know to look for, surfaced in terms I understand
- Understand why a connection exists — what evidence supports it, how corroborated it is, where it came from

**Mental model:**
"I own my spec — it defines my adapter, my lens, and my enrichment config. I load it onto a context and Plexus wires everything up. My lens enriches the graph with my vocabulary: edges like `lens:trellis:thematic_connection` that translate the graph's underlying relationships into terms I understand. That vocabulary layer persists in the graph — it's data, not configuration. Even if I disconnect, the edges my lens created are still there for anyone to query.

When I query, I can look through my vocabulary layer, explore another consumer's, or query the raw graph. The `relationship_prefix` filter is how I navigate between layers. I can discover what vocabularies exist by querying for `lens:*` edges — the namespace convention is the discovery mechanism.

Loading a spec has three effects: (a) my lens runs immediately and writes vocabulary edges to the graph — that's durable graph data; (b) my lens translation rules persist on the context, so when *any* consumer ingests new content, my lens fires and translates it — that's durable enrichment registration; (c) my adapter is available for routing ingest calls — that's transient, lasting as long as my workflow needs. The vocabulary layer and the reactive lens survive even if I disconnect. A long-running consumer keeps ingesting; a one-shot consumer loads, ingests, and walks away — but their lens keeps translating new content from other consumers.

Individual ingestions may take time — semantic extraction via llm-orc, declared enrichments that invoke external processes, whatever my spec requires. I don't block on all of that completing. The graph enriches incrementally: fast results arrive immediately, deeper extraction follows asynchronously. The graph is useful at every stage."

The consumer developer thinks in terms of their domain (fragments, gestures, code files, citations), not graph primitives (nodes, edges, contributions). They expect the adapter spec to be the single artifact they need to author. Everything behind the spec — sink mechanics, enrichment loops, scale normalization — is invisible infrastructure.

There are two interaction paradigms: **push** (Plexus emits events through the adapter's outbound side; the consumer reacts) and **pull** (the consumer queries on its own schedule through its lens; the consumer controls timing and question). Many real consumer workflows are pull — a CRON job checking "what new connections emerged since yesterday, in my vocabulary" is fundamentally pull. The lens concept lives primarily in the pull paradigm: the consumer looks through it when they choose to look.

### Domain-Specific Extractor Author

**Jobs:**
- Process raw source material into structured concepts and relationships
- Not need to know how the graph works — just produce JSON that the mapper understands
- Use the tools I know (Python scripts, LLM prompts) rather than learning a new framework
- Have my extraction logic compose with Plexus's enrichment loop — my extracted concepts should trigger co-occurrence detection, bridging, and discovery gaps automatically

**Mental model:**
"I write an extractor (script or LLM prompt) that understands my source format. I write a YAML spec that maps my extractor's output to graph structure. Plexus handles the rest — contribution tracking, normalization, enrichment, persistence."

### Engine Developer

**Jobs:**
- Navigate the codebase quickly — know which module owns which responsibility
- Trust that tests verify invariants, not just exercise code paths
- Make structural changes (refactoring) with confidence that behavior is preserved
- Understand why each architectural decision was made (provenance to ADRs and essays)

**Mental model:**
"The system has clear layers: graph primitives at the bottom, adapter/enrichment pipeline in the middle, transports at the top. Changes in one layer don't ripple to others. The domain model and system design are the authoritative maps."

## Value Tensions

- **Abstraction vs. domain fidelity:** Plexus must be general enough to serve any content domain (writing, movement, code, research), yet the value it provides to each domain depends on domain-specific extraction quality. How much domain-specific logic should live inside Plexus (as adapters) vs. outside (as extractors)?

- **Immediate value vs. deep extraction:** Rust-native phases provide instant feedback; LLM-based phases provide richer semantic understanding but take seconds and may be unavailable. The system bets that layered, incremental value is better than all-or-nothing — but this means consumers must handle a graph that enriches asynchronously over time.

- **Library autonomy vs. shared infrastructure:** The library rule (Invariant 41) gives consumers full control over storage and deployment. But federation requires coordination infrastructure that a pure library can't provide. How much coordination logic belongs in the library vs. in a separate coordination layer?

- **Evidence accumulation vs. noise tolerance:** Multi-source reinforcement strengthens signal, but it also means that systematic bias in extractors compounds. Evidence persists until explicitly retracted (Invariant 10). This is by design — the system is async, and a user returning after a year should find their graph intact. The question is whether the retraction mechanism (ADR-027) is sufficient for consumers who need to prune, or whether additional hygiene tools are needed.

- **Provenance completeness vs. ingestion throughput:** The dual obligation (Invariant 7) requires every write to carry both semantic content and provenance. This is epistemologically sound but adds overhead to every ingest call. At what scale does this become a bottleneck?

- **Scope vs. serendipity:** A consumer wants to see its domain clearly — Trellis queries about writing, not raw research metadata. But the entire value of a shared graph is cross-domain discovery. The lens-as-enrichment resolves this toward translation rather than filtering: the lens creates domain-translated relationships at write time, so cross-domain discoveries arrive pre-translated into the consumer's vocabulary. The consumer sees everything, expressed in terms they understand. The remaining tension: how much translation fidelity is possible? Some cross-domain connections may resist translation into a specific domain's vocabulary — the concept may not have a natural analog. How a lens handles untranslatable connections (surface them raw? Omit them? Create a generic "cross-domain" relationship?) is a design question for DECIDE.

- **Query simplicity vs. query power:** The write-heavy/query-light pattern promises a thin query surface — few primitives, intelligence at write time. But composing those primitives for real consumer queries (provenance-filtered traversal ranked by evidence diversity through a lens) is not simple. How much composition should Plexus handle versus exposing primitives for the consumer (or an LLM intermediary) to compose?

- **Vocabulary layer transparency vs. cognitive load:** *(Added 2026-04-02, multi-consumer interaction)* In a multi-consumer context, a consumer can query through its own lens, another consumer's lens, or the raw underlying relationships. More vocabulary layers means richer cross-domain discovery — but also more cognitive load for a consumer trying to understand what it's looking at. How much should Plexus help consumers navigate vocabulary layers (structured discovery tools, vocabulary metadata) vs. relying on the namespace convention and composable filters as sufficient?

## Assumption Inversions

| Assumption | Inverted Form | Implications |
|------------|--------------|-------------|
| Domain-agnosticism is achievable | What if some domains are fundamentally incompatible with a shared graph model? | Movement gestures and literary fragments share no vocabulary. If cross-domain bridging never produces useful signal, the shared namespace is overhead, not value. This is the starting point and aspiration — text-based domains are the clear first milestone. EDDI (movement/performance) is a later validation case. Open to this not playing out for all domains. |
| Tags are a primary input signal | What if users don't tag at all? | Tags are one form of semantic data among many — no more special than any other ingested content. The system's purpose is to derive structure from unstructured input (via extraction: SpaCy NER, LLM-based concept extraction, co-occurrence). Tags supplement but should not be required. If the enrichment chain depends too heavily on tags specifically, domains without tagging vocabularies get no value. |
| Plexus surfaces connections; consumers decide what to do | What if consumers need Plexus to go further — summarize, recommend, or generate? | The architectural stance is that Plexus derives structure and surfaces connections; consumer applications decide how to act on them (Essay 04's "mirror, not oracle" framing, implicitly reinforced by the enrichment loop architecture). But some consumers may need Plexus itself to produce higher-order outputs — summaries, recommendations, generated content. The current architecture doesn't serve these jobs directly; consumers would need to build that layer themselves on top of graph queries. |
| Evidence accumulates but never decays | What if stale evidence actively misleads? | By design, a user can return after a year and find their graph intact — async, long-lived use is a core use case. Temporal decay would only make sense in session-based contexts (e.g., EDDI performances). For most consumers, persistence is the feature. The question is whether retraction tools (ADR-027) are sufficient for consumers who do need pruning. |
| LLM extraction is optional | What if the Rust-native extraction provides so little value that users skip Phase 1 and wait for Phase 3? | The phased model assumes Phase 1 provides immediate, meaningful value. If Phase 1 is just file registration metadata, consumers might perceive the system as slow (waiting for Phase 3) rather than fast (Phase 1 is instant). |
| Federation is peer-to-peer | What if most real deployments are hub-and-spoke (one shared server, many clients)? | The CRDT-aligned replication design optimizes for autonomous peers. A centralized model would be simpler, require less coordination, and cover the majority use case. The peer-to-peer design may be over-engineering. |
| One user per instance (for now) | What if multi-user access on a single instance is needed before federation? | The current MCP server has no authentication or authorization. Adding multi-user to a single instance is a different problem than federation — it needs access control, not replication. |
| The query surface should be thin (few primitives, intelligence at write time) | What if consumers actually need rich query-time computation? | The write-heavy/query-light pattern works for Graphiti's single-consumer case. Multi-consumer convergence + provenance-scoped queries + evidence diversity ranking may demand more query-time intelligence than the "thin" framing admits. The question is whether "thin" means few primitives or simple composition — Plexus may need few primitives but rich composition. |
| A lens is runtime configuration that shapes what the consumer sees | **Validated as wrong.** A lens encodes persistent graph structure — vocabulary layers are graph data, not ephemeral configuration | Lens "registration" is a misnomer for what actually happens. A spec is loaded, its lens enrichment runs, and edges are written to the graph. After that, the vocabulary layer exists regardless of whether the spec remains loaded. Discovery is graph introspection (query `lens:*` edge prefixes), not registry lookup. The graph is the registry. *(Surfaced during MODEL attempt, validated by user 2026-04-01)* |
| A lens is a read-side concern separate from the write-side adapter | **Validated as wrong.** The lens IS a write-side concern — an enrichment | Resolved toward write-side. The lens is an enrichment that translates cross-domain graph content into one consumer's domain vocabulary at write time. This preserves write-heavy/query-light, keeps the three extension axes (Invariant 40), and means each consumer's lens makes the graph richer for all consumers. Fully resolved — no longer an open tension. *(Validated 2026-03-25)* |
| Each consumer operates independently on a shared context | What if consumers benefit from awareness of each other's vocabulary layers? | A consumer arriving at a context with existing lenses can orient through others' vocabularies before (or instead of) defining its own. Cross-consumer vocabulary browsing is emergent from Invariant 56 (public output) + composable filters. The system enables this without requiring coordination between consumers. *(Added 2026-04-02)* |
| Lens discovery requires a registry or metadata store | What if the graph itself is the discovery mechanism? | Since lenses write edges with `lens:{consumer}:{relationship}` namespace, querying for `lens:*` edge prefixes reveals all vocabulary layers. No registry, no metadata store — the namespace convention IS the interface. This collapses a potential infrastructure concern into a query pattern. *(Added 2026-04-02)* |
| Spec delivery requires a specific mechanism (file on disk, API call, etc.) | What if the delivery mechanism doesn't matter — only the API operation does? | The consumer owns the spec. Whether it arrives as a file in a directory (auto-discovery) or programmatically through the API, Plexus receives a spec and acts on it. The API operation is "load this spec onto this context." The delivery path is a transport/deployment concern, not a Plexus concern. This collapses the deployment-time vs. interaction-time tension into a single API operation with multiple delivery paths. *(Added 2026-04-02)* |
| Cross-domain discovery requires explicit consumer action | What if the most valuable discoveries happen when users aren't looking for them? | If discovery requires the consumer to switch modes or issue special queries, users may never discover cross-domain connections. Push-based discovery signals — "this new connection crosses your domain boundary and is highly corroborated" — might be more valuable than pull-based discovery queries. The push paradigm (outbound events) already exists; the question is whether discovery events should be a first-class event type. |

## Product Vocabulary

| User Term | Stakeholder | Context | Notes |
|-----------|-------------|---------|-------|
| fragment | Consumer developer | "I'm sending a fragment to Plexus" — a piece of writing, an annotation, a note | The minimum unit of content. Not a graph term — the user doesn't think in nodes. |
| tag | Consumer developer, end user | "I tagged this with #travel" — a human-applied label | One form of semantic data among many. Tags are additional signal but not assumed to be present — Plexus derives structure from unstructured input via extraction. Not more special than any other ingested content. |
| connection | End user, consumer developer | "Plexus found a connection between X and Y" | The user-facing word for what the system models as edges with contributions and normalized weights. Connections are the primary signal to consumer applications — the basis for whatever action the consumer takes. Users don't think in weights. |
| chain | Consumer developer | "Create a research chain for this project" | A grouping of related marks. Users think of it as a project or trail, not a provenance-dimension node. |
| mark | Consumer developer | "Mark this passage" — annotate a location in a file | A provenance record. Users think of it as a bookmark with notes, not a graph node. |
| discovery | End user, consumer developer | "Plexus discovered that these two themes are related" | The user-facing word for cross-domain or unexpected connections surfaced by the graph. Includes both intra-domain discoveries (connections within one consumer's data) and cross-domain discoveries (connections between consumers' data, translated through the consumer's lens). |
| extraction | Consumer developer | "Run extraction on this file" | The phased process of turning a file into graph structure. Users think of it as "analyze this file," not as three-phase adapter dispatch. |
| adapter | Consumer developer | "I need to write an adapter for my data" | The extension point. Consumer developers think of it as a plugin or connector, not a bidirectional integration contract. |
| spec | Extractor author, consumer developer | "I wrote a YAML spec for my extractor" / "Load my spec onto this context" | The declarative adapter specification — the single artifact a consumer authors. Contains adapter definition, lens rules, and declared enrichment config. The consumer owns and delivers it; Plexus receives and acts on it. *(Updated 2026-04-02 — expanded from extractor author to consumer developer; spec is the unit of consumer identity)* |
| analyze | Engine developer, consumer developer | "plexus analyze my-context" | On-demand external enrichment via llm-orc. Users think of it as "run analysis," not "execute external enrichment ensemble and re-ingest results." |
| lens | Consumer developer | "Define a lens for how Trellis sees the graph" — the consumer's definition of how cross-domain content translates into their domain vocabulary | A write-time enrichment, not a read-time filter. The lens operates over the whole graph (like any enrichment) and creates domain-translated relationships — e.g., "this-research-concept-relates-to-writing-theme." Translation happens at enrichment time, so by the time the consumer pulls, the graph already speaks their language. Lens output is public: other consumers can see the translated relationships. This means each consumer's lens makes the graph richer for everyone — cross-domain pollination compounds with each consumer added. Defined within the adapter spec (the adapter already knows the domain vocabulary). Preserves Invariant 40 — the lens is an enrichment, not a fourth extension axis. |
| corroboration | Consumer developer, end user | "How corroborated is this connection?" — how many independent sources support it | User-facing name for evidence diversity. "Corroboration" is closer to how users think about trust than "evidence diversity count." Four different kinds of evidence are more trustworthy than a hundred of the same kind. |
| vocabulary layer | Consumer developer | "What vocabulary layers exist on this context?" — the set of translated edges a consumer's lens has created | Not a stored entity — it's a query pattern over edges sharing a `lens:{consumer}:` namespace prefix. Each consumer's lens creates a vocabulary layer. Discovery is graph introspection: query for `lens:*` prefixes. *(Added 2026-04-02)* |
| load spec | Consumer developer | "Load my spec onto this context" — activate an adapter spec (with lens and enrichments) for use | The API operation that validates the spec, wires the adapter, persists the lens translation rules, and runs the lens enrichment. Three effects: (a) durable graph data — lens writes vocabulary edges immediately; (b) durable enrichment registration — lens translation rules persist on the context, reactive to all future emissions by any consumer; (c) transient adapter wiring — adapter available for ingest calls during the consumer's workflow. On startup, Plexus re-registers persisted lens enrichments. Transport-independent. *(Added 2026-04-02)* |

## End-to-End Acceptance Criterion (MCP Consumer Interaction Cycle)

*(Added 2026-04-02)*

Given Plexus + llm-orc + Ollama all running, and given valid llm-orc ensembles and a valid declarative spec:

1. Create a context
2. Load a spec (adapter + lens + declared enrichments) onto that context
3. Ingest content through the loaded spec's adapter — including semantic extraction via llm-orc
4. Query the graph over that context — through the consumer's lens, raw relationships, or both
5. Load a second spec onto the same context, ingest through it, and query across both vocabulary layers

This is the first real end-to-end consumer workflow through MCP. Completion means Plexus is usable via MCP for actual work, not just infrastructure testing.

## Product Debt

Assumptions baked into the architecture that may not match actual user needs.

| Assumption | Baked Into | Actual User Need | Gap Type | Resolution |
|------------|-----------|-----------------|----------|------------|
| All consumers need provenance | Invariant 7 (dual obligation) | Some consumers may want lightweight ingestion without provenance overhead | Over-abstraction | Validate: do any real consumer workflows skip provenance? If so, consider a "lightweight ingest" path. Deferred — no consumer has requested this. |
| Tags are a significant input signal | *(TagConceptBridger removed)* co-occurrence enrichment chain | Tags are one input among many — users may not tag at all. Plexus should derive structure from unstructured input via extraction (SpaCy, LLM). | Over-reliance on single input type | **Resolved:** TagConceptBridger was removed from the codebase — tag bridging is domain-specific. Extraction (NER, concept extraction, co-occurrence) and embedding similarity (ADR-026) provide structure without tags. Domains that use tags and need bridging implement their own adapter. The enrichment chain no longer depends on tags specifically. |
| Outbound events are sufficient feedback | Adapter's transform_events() | Consumers may need richer query-time feedback (e.g., "what changed since my last query?") beyond the event stream | Missing workflow | **Resolved:** Event cursors (ADR-035) deliver persistent sequence-numbered event log in SQLite, pull-based "changes since N" queries. The pull paradigm no longer forces Plexus-as-server. |
| MCP is the right transport for interactive use | mcp/ module; stdio transport | MCP works well for LLM-mediated use but poorly for direct app-to-app integration without an LLM host | Over-abstraction of transport needs | Essay 09 identified this: gRPC is better for app-to-app. MCP is one transport; others are needed for non-LLM consumers. |
| Single-context operation is the default | MCP server's active_context model | Some workflows may need cross-context operations (compare two projects, merge contexts) | Missing workflow | Meta-context (read-only union query) is designed but not implemented. Cross-context writes are not designed. |
| Push (events) is the primary feedback paradigm | Adapter's `transform_events()`, outbound event model | Many consumer workflows are pull-based — a CRON job checking for new connections, a user-initiated query, a scheduled analysis | Missing workflow | **Resolved:** Event cursors (ADR-035) enable pull paradigm. The library rule (Invariant 41) is preserved for read workflows. |
| Query results are raw graph data | `PlexusApi` query methods return nodes, edges, paths | Consumers need results expressed in their domain vocabulary — not "node concept:travel has normalized weight 0.73" but "the travel theme is strongly connected to your provence fragments" | Mental model mismatch | The lens-as-enrichment approach: domain-translated relationships are created at write time, so query results already contain domain-vocabulary structure. The gap narrows to: do the existing query primitives plus lens-created structure give consumers enough to work with? |
| Spec loading is a deployment-time concern | `register_specs_from_dir` in pipeline construction | Consumers need to load specs at interaction time — "I'm connecting now, here is my spec" — not just at server startup | Missing workflow | The API needs a `load_spec` operation at the PlexusApi level. File-based auto-discovery is one delivery path; programmatic loading is the general case. Both converge on the same API operation. *(Added 2026-04-02)* |
| Lens wiring happens automatically when specs are registered | `register_specs_from_dir` registers adapters but does NOT extract or register lens enrichments | Consumers expect loading a spec to wire everything — adapter, lens, enrichments — as a single operation | Implementation gap | `register_specs_from_dir` has a known gap: it registers adapters but doesn't wire lens enrichments. The `load_spec` API operation must wire the complete spec — adapter + lens + declared enrichments — atomically. *(Added 2026-04-02)* |
| Spec validation can happen lazily during ingestion | Implicit — no upfront validation step in current pipeline | Malformed specs must fail fast, before any graph work begins. Graph ingestion is resource-intensive; discovering a bad spec mid-pipeline wastes compute and may leave partial state. | Missing validation gate | `load_spec` must validate the spec (structure, schema, lens rule consistency) and return a clear error on failure — before wiring any adapter or running any enrichment. If `load_spec` succeeds, the spec is valid and everything is wired. If it fails, nothing happened. The contract abstracts across transports. *(Added 2026-04-02)* |
