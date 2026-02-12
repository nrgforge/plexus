# Domain Model: Plexus

Ubiquitous language for the Plexus knowledge graph. All ADRs, behavior scenarios, and code must use these terms consistently. If this glossary says "emission," the code says `emission`, not "output batch" or "result set."

Extracted from: ADR-001, semantic-adapters.md, semantic-adapters-design.md, PAPER.md, SPIKE-OUTCOME.md, Essay 07 (first adapter pair), Essay 08 (runtime architecture), Essay 09 (public surface).

---

## Concepts

| Term | Definition | Aliases to Avoid |
|------|-----------|------------------|
| **Adapter** | A bidirectional integration contract. Inbound: transforms domain-specific input into graph mutations via `process()`. Outbound: transforms raw graph events into domain-meaningful events for the consumer via `transform_events()`. Owns its entire processing pipeline internally. The single artifact a consumer needs to understand. | plugin, processor, handler |
| **Sink** | The interface through which an adapter pushes graph mutations into the engine. `AdapterSink` is the trait; `EngineSink` is the production implementation. | output, writer, channel |
| **Emission** | The data payload of a single `sink.emit()` call: a bundle of annotated nodes, annotated edges, and removals. Each emission is validated and committed atomically. **Not** the act of emitting — use "emit" as the verb. | batch, result, output (when meaning the data) |
| **Node** | A vertex in the knowledge graph. Has an ID, type, content type, dimension, and properties. | vertex, entity |
| **Edge** | A directed connection between two nodes. Carries per-adapter contributions, a computed raw weight, and a relationship type. | link, connection, arc |
| **Contribution** | A single adapter's latest assessment of an edge's strength. Stored per-adapter on each edge as `HashMap<AdapterId, f32>`. Each adapter's slot stores the value from its most recent emission. The adapter owns the semantics of what this value means in its domain. See ADR-003. | weight (ambiguous), score |
| **Raw weight** | The combined strength of an edge across all adapter contributions. Computed by the engine: scale-normalize each adapter's contributions to comparable range, then sum. Ground truth for query-time normalization — but itself computed from stored contributions, not stored directly. Never decays on a clock. See ADR-003. | weight (ambiguous without qualifier), strength |
| **Scale normalization** | Engine-side operation that brings each adapter's contributions to a comparable range before summing into raw weight. Uses divide-by-range with dynamic epsilon: `(value - min + ε) / (max - min + ε)` where `ε = α × range`. Prevents high-magnitude adapters from dominating low-magnitude ones, and prevents the weakest real evidence from mapping to zero. Degenerate case (range = 0) normalizes to 1.0. Distinct from query-time normalization. See ADR-003, Essay 07. | — |
| **Normalized weight** | Relative importance of an edge, computed at query time via a NormalizationStrategy from raw weight. Not stored. Different consumers can apply different strategies to the same raw weights. | — |
| **Annotation** | Adapter-provided metadata about a single extraction: confidence, method, source location, detail. Lives on an AnnotatedNode or AnnotatedEdge. Describes *how* the adapter came to know something. Distinct from **tag** — see disambiguation §7. | metadata (too generic) |
| **Annotation confidence** | A single adapter's certainty about a single extraction. Range 0.0–1.0. Lives in the annotation, not on the edge. Distinct from raw weight and normalized weight. | score, certainty |
| **Provenance entry** | The full record of how a piece of knowledge entered the graph. Constructed by the engine (not by adapters) by combining the adapter's annotation with framework context: adapter ID, timestamp, input summary, context ID. | — |
| **Dimension** | A facet of the knowledge graph. Adapters declare which dimensions they populate. Known dimensions: structure, semantic, relational, temporal, provenance. | layer, category |
| **Content type** | A field on Node that disambiguates which domain produced it. Enables the shared semantic namespace — all domains contribute concept nodes, and ContentType tells you where each one came from. | — |
| **Concept** | A node in the semantic dimension representing an extracted idea, theme, or entity. Concepts from different domains share a namespace — cross-modal bridging happens when independent adapters produce the same concept label. | topic, keyword, term |
| **`may_be_related`** | The edge relationship type used by co-occurrence enrichments to propose a connection between concepts. Starts weak. Graph dynamics (reinforcement from actual evidence across adapters) determine whether the proposal is real. | suggested, proposed (as relationship names) |
| **Evidence diversity** | How corroborated an edge is — derived by querying provenance (count distinct adapter IDs, source types, and contexts). Not a stored field. Four different kinds of evidence are more trustworthy than a hundred of the same kind. | — |
| **Normalization strategy** | A pluggable function that computes normalized weight from raw weight at query time. Default: per-node outgoing divisive normalization (`w_ij / Σ_k w_ik`). | — |
| **Adapter input** | The envelope the framework hands to an adapter: context ID, opaque data payload (`Box<dyn Any>`), and trigger type. The framework manages the envelope; the adapter downcasts the data. | — |
| **Context ID** | Identifies a context. Examples from the current ecosystem: `trellis` (all fragments, raw captures), `desk` (promoted seeds, active clusters, research digests), `carrel-research` (research digests, writing marks, voice profiles). The key into PlexusEngine's DashMap. | session ID (close but not identical) |
| **Cancellation token** | A cooperative signal that an adapter's in-flight work has been superseded. Checked between emissions, not during. Already-committed emissions remain valid. | — |
| **Graph event** | A low-level notification fired per mutation type when an emission is committed. Five kinds: NodesAdded, EdgesAdded, NodesRemoved, EdgesRemoved, WeightsChanged. Feeds into the enrichment loop and the adapter's outbound transformation. Note: EdgesAdded fires for every committed edge including re-emissions — it does not distinguish new from updated. | — |
| **Input kind** | A string declared by each adapter identifying what type of input it consumes. The router uses this for matching. Examples: `fragment` (tagged writing), `file_content`, `gesture_encoding`. | — |
| **Adapter snapshot** | Optional incremental state from a previous run, passed back to the adapter on re-processing. Contents are adapter-specific (e.g., chunk hashes for documents, cluster centroids for movement). Design deferred. | checkpoint, state |
| **Input router** | Framework component that directs incoming input to all adapters whose `input_kind()` matches. Fan-out: when multiple adapters match, all receive the input. | dispatcher |
| **Fragment** | A piece of writing — a journal entry, SMS message, email, note — carrying text and tags. The input type for Trellis's FragmentAdapter. A fragment becomes a fragment node in the graph; its tags become concept nodes. | entry, note, snippet (as domain terms — these are all fragments) |
| **Tag** | A label applied to a fragment, either manually by a human or by an LLM. Each tag produces a concept node in the semantic dimension. Distinct from **annotation** — a tag is input-side vocabulary (what the fragment is about); an annotation is extraction-side vocabulary (how the adapter came to know something). | label (acceptable informally), keyword |
| **`tagged_with`** | The edge relationship type from a fragment node to a concept node. Represents "this fragment was labeled with this concept." Contribution value is 1.0 (binary: the tag was applied). | — |
| **Deterministic concept ID** | The scheme by which concept nodes receive stable, convergent IDs derived from their label: `concept:{lowercase_tag}`. Ensures that two fragments tagged "travel" produce the same concept node, triggering upsert rather than creating duplicates. | — |
| **Co-occurrence** | The pattern where two concepts appear together across multiple fragments — both tagged on the same fragment. The signal the CoOccurrenceEnrichment detects. Measured by counting shared fragments between a concept pair. | correlation (too statistical), co-location |
| **Co-occurrence score** | The normalized count of shared fragments between a concept pair, used as the contribution value for a `may_be_related` proposal. Normalized relative to the maximum co-occurrence count in the current graph: `count / max_count`. | — |
| **Context snapshot** | A cloned Context provided to enrichments during the enrichment loop. Provides a consistent, immutable view of the graph at enrichment time. The framework creates the snapshot; enrichments read it. | graph state snapshot (old name) |
| **Symmetric edge pair** | Two directed edges (A→B and B→A) with identical contributions, representing a semantically symmetric relationship in the directed graph model. Used for `may_be_related` edges so that query-time normalization (outgoing divisive) sees the relationship from both endpoints. | bidirectional edge, undirected edge |
| **Normalization floor** | The minimum scale-normalized value for any real contribution. Prevents the weakest evidence from mapping to exactly 0.0. Implemented via dynamic epsilon: `ε = α × (max - min)` where α is the floor coefficient (default 0.01). The floor is proportionally equal for all adapters regardless of their contribution range. | epsilon (acceptable in implementation comments) |
| **Floor coefficient (α)** | The constant that determines the proportional normalization floor. With α = 0.01, the weakest contribution from any adapter maps to ~1% of that adapter's strongest contribution after scale normalization. | — |
| **Context** | A bounded subgraph representing a project or workspace. Contains nodes (across multiple dimensions), edges, and metadata (sources, tags). The unit of persistence and the boundary within which cross-dimensional edges connect. Multiple tools can contribute sources to the same context independently — a Trellis fragment file and a Carrel research digest can both be sources in a shared context without either tool knowing about the other. Plexus handles overlap through deterministic concept IDs and upsert. | workspace, project (as graph terms) |
| **PlexusEngine** | The runtime engine managing all contexts. Holds an in-memory `DashMap<ContextId, Context>` cache and an optional `GraphStore` for persistence. All mutations — adapter emissions, provenance operations — route through the engine. | engine (acceptable shorthand) |
| **GraphStore** | The persistence abstraction trait. Implementations: `SqliteStore` (local file), future `R2Store` or `RemoteDbStore`. Supports bring-your-own-storage (BYOS). The engine calls `save_context()` after mutations. | store, storage backend |
| **Mark** | A provenance-dimension node annotating a specific location in a file. Carries: file path, line number, annotation text, tags, type label, column (optional). Lives in a project context, not a global container. Connected to concept nodes via automatic tag-to-concept bridging. | bookmark, annotation (ambiguous — see §7) |
| **Chain** | A provenance-dimension node grouping related marks into a narrative trail. Either a writing chain (one per writing project) or a research chain (one per research run). Has a status: active or archived. | trail |
| **Link** | A provenance-dimension edge with relationship `links_to` connecting two marks. Encodes a specific cross-reference: "this paper is relevant to this passage." Distinct from adapter-produced edges. | cross-reference (acceptable informally) |
| **ProvenanceApi** | The high-level API for mark, chain, and link operations. Scoped to a specific context (not global). Creates marks as provenance-dimension nodes, chains as grouping nodes, and links as directed edges. Performs tag-to-concept bridging on mark creation. | — |
| **Cross-dimensional edge** | An edge connecting nodes in different dimensions within the same context. The mechanism by which provenance (marks) connects to semantics (concepts). Created by `Edge::new_cross_dimensional()`. | bridge edge (acceptable informally) |
| **`references`** | The edge relationship type from a mark to a concept node, created automatically by tag-to-concept bridging. A cross-dimensional edge (provenance → semantic). | — |
| **Tag-to-concept bridging** | Automatic creation of `references` edges between marks and concept nodes when tags match concept node IDs in the same context. Implemented as an enrichment (`TagConceptBridger`) that runs in the enrichment loop after each emission. Bridges bidirectionally: new marks to existing concepts and new concepts to existing marks. The connection between what a user annotates and what the adapter layer discovered. | auto-bridging |
| **Tag format normalization** | The convention that converts mark tags to concept node IDs: strip `#` prefix if present, lowercase, then prepend `concept:`. Tag `#Travel` and tag `travel` both match node `concept:travel`. Must be consistent across all mark creation and concept node creation paths. | — |
| **Persist-per-emission** | The persistence strategy: one `save_context()` call at the end of each `emit()` invocation. Atomic per-emission. Batch optimization deferred. | — |
| **Consumer** | An external application that sends domain data to and receives domain events from Plexus. Examples: Trellis (creative writing), EDDI (interactive performance), Manza (code analysis), Carrel (research coordination). A consumer interacts with Plexus through an adapter and a transport — it never sees graph primitives. | client (too generic), application |
| **Enrichment** | A reactive component registered globally on the engine. Receives graph events and a context snapshot after each emission round; optionally returns an Emission with additional graph mutations. Self-selects based on events and context state. Terminates via idempotency — checks context state before emitting to avoid infinite loops. Deliberately separate from Adapter: an enrichment has no `input_kind`, accepts no domain data, and doesn't serve a consumer. Enrichments bridge between dimensions within the graph; adapters bridge between a consumer's domain and the graph. | post-processor, hook, trigger |
| **Enrichment loop** | The iterative process following a primary emission: run all registered enrichments with events and a context snapshot; if any produce mutations, commit them, collect their events, and run again; repeat until quiescence. The framework runs the loop; enrichments provide the logic. | cascade, chain reaction |
| **Quiescence** | The state where all enrichments return `None` in a round, indicating no more reactive work to do. The enrichment loop's termination condition. Reached when every enrichment's idempotency check determines that the context already reflects its desired state. | convergence (close but implies approximation), steady state |
| **Outbound event** | A domain-meaningful event produced by the adapter's `transform_events()` method. Translated from raw graph events into the consumer's language. Example: `concepts_detected: travel, provence` rather than `NodesAdded with node_ids [concept:travel, concept:provence]`. The consumer receives only outbound events, never raw graph events. | domain event (acceptable informally), notification |
| **Transport** | Any protocol that can ferry an ingest request in and outbound events out. A thin shell: it accepts `(context_id, input_kind, data)`, forwards to the router, and returns outbound events. Examples: gRPC, REST, MCP, WebSockets, WebRTC, FFI. Adding a transport doesn't touch adapters, enrichments, or the engine. | protocol (too generic), channel |
| **Ingest** | The primary write endpoint on Plexus's public surface: `ingest(context_id, input_kind, data)`. Routes domain data to the matching adapter, runs the enrichment loop, transforms events through the adapter's outbound side, and returns outbound events. All writes go through ingest — there is no separate API for graph primitives. | submit, push, send |
| **Integration** | A registered bundle of adapter + enrichments for a specific consumer. Example: `register_integration("trellis", adapter: FragmentAdapter, enrichments: [TagConceptBridger])`. Enrichments shared across integrations are deduplicated by `id()`. The consumer interacts with a high-level API that hides the internal decomposition. | registration, binding |
| **Event cursor** | A sequence-based position for pull-based event delivery: "give me changes since sequence N." Requires event persistence with sequence numbering. Push delivery is layered on top of cursors, not instead. Design deferred. | offset, checkpoint (for events) |

## Actions

| Action | Actor | Target | Description |
|--------|-------|--------|-------------|
| **emit** | Adapter | Sink | Push an emission (nodes + edges + removals) through the sink. Async — the adapter awaits and receives validation feedback. |
| **commit** | Engine | Graph | Validate each item in the emission and persist valid mutations. Fires graph events for committed items. Invalid items (edges with missing endpoints) are rejected individually; valid items in the same emission still commit. |
| **reject** | Engine | Edge | Refuse an individual edge whose endpoints don't exist in the graph or in the same emission. The rejection is reported in the result returned to the adapter. Other items in the same emission are unaffected. |
| **upsert** | Engine | Node | When a node with a duplicate ID is emitted, update its properties rather than creating a second node. This is not an error — it's the expected path for re-processing. |
| **reinforce** | Engine | Edge | Update an existing edge's contribution for the emitting adapter. Triggered implicitly when an adapter emits an edge that already exists — the engine replaces that adapter's contribution slot with the new value. No separate API; reinforcement happens through `emit()`. Contributions can increase or decrease. See ADR-003. |
| **normalize** | NormalizationStrategy | Edge | Compute the relative weight of an edge at query time from its raw weight and graph context. |
| **route** | Input router | Adapter(s) | Direct incoming input to all adapters whose `input_kind()` matches. Fan-out: multiple adapters can declare the same input kind. |
| **cancel** | Framework | Adapter (via token) | Signal that in-flight work has been superseded. Adapter checks cooperatively between emissions. |
| **annotate** | Adapter | Node or Edge | Attach extraction metadata (confidence, method, source location) to a node or edge in the emission. |
| **construct provenance** | Engine | Provenance entry | Combine the adapter's annotation with framework context (adapter ID, timestamp, context ID, input summary) to create a full provenance record. |
| **tag** | Human or LLM | Fragment | Apply a label to a fragment, upstream of the adapter. The adapter receives already-tagged input. Tagging is not an adapter action — it happens before the framework sees the fragment. |
| **detect co-occurrence** | CoOccurrenceEnrichment | Context snapshot | Scan concept nodes for pairs that share fragments (via `tagged_with` edges), count shared fragments, and compute co-occurrence scores. The enrichment's core computation. |
| **snapshot** | Framework | Context | Clone the Context to create an immutable context snapshot for enrichments. Ensures enrichments see a consistent view unaffected by concurrent mutations. |
| **persist** | Engine | Context (via GraphStore) | Save the in-memory context to persistent storage after a mutation. Distinct from **commit** — commit is the validate-and-write-to-memory step; persist is the write-to-storage step. Both happen within a single `emit()` call. |
| **hydrate** | Engine | DashMap (from GraphStore) | Load all contexts from persistent storage into the in-memory cache on startup. Called once via `load_all()`. |
| **bridge** | Enrichment (TagConceptBridger) | Mark ↔ Concept | Automatically create `references` cross-dimensional edges between marks and concept nodes whose IDs match (after tag format normalization). Runs as part of the enrichment loop, not inline at creation time. Bridges in both directions: new marks to existing concepts, and new concepts to existing marks. |
| **add mark** | ProvenanceApi | Context | Create a mark node in the provenance dimension of a specified project context. Requires a context, chain, file, line, and annotation. Triggers tag-to-concept bridging if tags are provided. |
| **ingest** | Transport | Engine (via router) | Push domain data through the public surface. The transport forwards `(context_id, input_kind, data)` to the router, which matches the adapter, runs the pipeline (process → enrichment loop → outbound transformation), and returns outbound events. |
| **enrich** | Enrichment | Graph (via emission) | React to graph events and produce additional graph mutations. Called by the enrichment loop after each emission round with the accumulated events and a context snapshot. Returns `Some(Emission)` if there is work to do, `None` if quiescent. |
| **transform events** | Adapter | Graph events | Translate raw graph events into domain-meaningful outbound events for the consumer. Called after the enrichment loop completes with all accumulated events from primary emission and all enrichment rounds. The adapter filters what its consumer cares about. |

## Relationships

### Structural
- **Adapter** emits through **sink**
- **Emission** contains **AnnotatedNodes**, **AnnotatedEdges**, and removals
- **AnnotatedNode** pairs a **Node** with an optional **Annotation**
- **AnnotatedEdge** pairs an **Edge** with an optional **Annotation**
- **Engine** constructs **provenance entry** from **annotation** + framework context
- **Edge** carries per-adapter **contributions** and a relationship type
- **Engine** computes **raw weight** from **contributions** via **scale normalization**
- **NormalizationStrategy** derives **normalized weight** from **raw weight** + graph context

### Behavioral
- **Input router** fan-outs to all **adapters** matching **input kind**
- **Engine** fires **graph events** when **emission** is committed
- **Cancellation token** is checked between **emissions**, not during

### Domain
- All domains contribute **concepts** to a shared semantic namespace
- **Content type** disambiguates which domain produced a **node**
- Independent adapters producing the same **concept** label creates cross-modal agreement
- **Enrichments** bridge vocabulary gaps via `may_be_related` edges and cross-dimensional connections
- **Evidence diversity** is derived at query time from **provenance entries** — no component computes or stores it proactively

### Fragment adapter pair
- **Fragment** carries text and **tags**; each tag produces a **concept** node
- **Fragment node** lives in the **structure** dimension; **concept nodes** live in the **semantic** dimension
- **`tagged_with`** edges connect **fragment nodes** to **concept nodes** (one per tag)
- Multiple **fragments** sharing a **tag** converge on the same **concept** node via **deterministic concept ID** and **upsert**
- **CoOccurrenceEnrichment** reads a **context snapshot**, detects **co-occurrence**, and emits `may_be_related` **symmetric edge pairs** between co-occurring **concepts**
- **Co-occurrence score** is the **contribution** value on `may_be_related` edges
- One **FragmentAdapter** type, instantiated with different **adapter IDs** per evidence source (manual, LLM, OCR) — same `process()` logic, distinct **contributions** and **provenance**

### Public surface
- **Consumer** sends domain data through a **transport**, receives **outbound events** back through the same **transport**
- **Transport** accepts an **ingest** request and forwards it to the **input router**
- **Adapter** is bidirectional: **process()** handles inbound, **transform_events()** handles outbound
- **Enrichment** is registered globally on the **engine**, self-selects based on **graph events** and **context** state
- **Enrichment loop** runs after each primary **emission**: enrichments → commit → events → enrichments → quiescence
- **Outbound events** are produced by the **adapter's** outbound side from accumulated **graph events**
- **Consumer** never sees raw **graph events** — only **outbound events** translated by the **adapter**
- **Integration** bundles an **adapter** with its **enrichments** for a **consumer**
- Three independent extension points: **adapters** (domain), **enrichments** (graph intelligence), **transports** (protocol)

### Runtime architecture
- **PlexusEngine** manages multiple **contexts** via in-memory cache
- **EngineSink** routes adapter **emissions** through **PlexusEngine** (not bare Context)
- **PlexusEngine** delegates persistence to **GraphStore** after each mutation
- **ProvenanceApi** is scoped to a specific **context** — marks live in project contexts
- **Chain** contains **marks** via `contains` edges (provenance dimension)
- **Mark** references **concepts** via `references` cross-dimensional edges (provenance → semantic)
- **Mark** links to other **marks** via `links_to` edges (provenance dimension)
- **Tags** on marks map to **concept node IDs** via **tag format normalization**
- **Context** is the boundary — cross-dimensional edges connect within a context, not across contexts
- Multiple external tools contribute **sources** to a shared **context** independently — overlap converges through **deterministic concept IDs** and **upsert**
- A **source** update fans out to all **contexts** that include it — same pattern as **input router** fan-out, but across contexts instead of across adapters

## Invariants

### Emission rules (enforced by engine at commit time)
1. The engine validates and commits each item in the emission independently. Valid items commit; invalid items are rejected individually. The emission is **not** all-or-nothing.
2. Per-item behaviors: edges with missing endpoints are **rejected** (endpoint must exist in the graph or in the same emission). Duplicate node IDs **upsert**. Removal of non-existent nodes is a **no-op**. Self-referencing edges are **allowed**. Empty emissions are a **no-op**.
3. `emit()` returns a result describing what was rejected and why. The adapter can act on rejected items or ignore them. Partial success is the normal case, not an error.
4. Cancellation is checked **between** emissions, not during. Committed items are never rolled back.

### Provenance rules
5. Adapters **never** construct provenance entries directly — they annotate, the engine wraps.
6. Nodes emitted without annotation still receive provenance marks (structural context only).

### Weight rules (updated by ADR-003)
7. Per-adapter contributions are stored. Raw weights are computed from contributions via scale normalization. Normalized weights are computed from raw weights at query time. Three layers: contribution (stored) → raw weight (engine-computed) → normalized weight (query-time-computed).
8. No temporal decay. Weakening happens only through normalization as the graph grows around an edge.
9. A quiet graph stays stable — silence is not evidence against previous observations.
10. Contributions use latest-value-replace: each adapter's slot stores the value from its most recent emission. Contributions can increase or decrease.
11. Contributions can be any finite f32 value. Adapters and enrichments emit in whatever scale is natural to their domain (e.g., 0–20 for test counts, 0–500 for gesture repetitions, 0–127 for MIDI velocities, -1.0–1.0 for sentiment). The engine's scale normalization (initially divide-by-range) maps these to comparable ranges regardless of whether the native scale is signed or unsigned.
12. Adapter and enrichment IDs must be stable across sessions. If reconfigured with a new ID, previous contributions become orphaned. Old contributions should be explicitly removed.

### Adapter rules
13. The framework never inspects the opaque data payload. Adapters downcast internally.
14. Adapters are independent — they don't know about each other.
15. An adapter owns its full internal pipeline. The framework has no opinion on internal phase ordering.

### Routing rules
16. Input routing is fan-out: all adapters matching the input kind receive the input.
17. Each matched adapter is spawned independently with its own sink and cancellation token.

### Fragment adapter rules
18. Concept IDs are deterministic: `concept:{lowercase_tag}`. Same tag always produces the same node ID, ensuring convergence via upsert.
19. Tags produce binary contributions (1.0) on `tagged_with` edges. The contribution means "this tag was applied," not a graduated strength.
20. Symmetric relationships (`may_be_related`) are emitted as two directed edges (A→B and B→A) with identical contributions, so query-time normalization sees the relationship from both endpoints.
21. The normalization floor ensures the weakest real contribution from any adapter or enrichment maps to ~α (default 0.01), not 0.0. Formula: `(value - min + α·range) / ((1 + α)·range)`. Degenerate case (range = 0) returns 1.0.

### Runtime architecture rules
22. Marks always live in a project context, in the provenance dimension. There is no global `__provenance__` context.
23. `add_mark` requires a context parameter. No default, no fallback.
24. Tag format normalization is an invariant: strip `#` prefix if present, lowercase, then prepend `concept:` to match concept node IDs. `#Travel` → `concept:travel`, `travel` → `concept:travel`.
25. Tag-to-concept bridging is automatic via the enrichment loop. A `TagConceptBridger` enrichment detects new concept nodes, finds marks with matching tags, and creates cross-dimensional `references` edges. Because the enrichment runs on every emission round (not just mark creation), it bridges in both directions: a new mark bridges to existing concepts, and a new concept bridges to existing marks.
26. `list_tags()` queries across all contexts, not a single context.
27. Tags are the shared vocabulary between provenance and semantics. A tag string on a mark and the ID of a concept node must use the same normalized form.
28. Persist-per-emission: each `emit()` call results in exactly one `save_context()` call. Emissions are the persistence boundary.
29. Contributions must survive persistence. After save → load, `edge.contributions` must be identical. Scale normalization depends on this.
30. Sources can appear in multiple contexts. A file that is a source in both `trellis` and `desk` contexts produces independent graph nodes in each. Overlap is handled by Plexus, not by the contributing tools.
31. A source update fans out to all contexts that include it. Each context gets its own adapter run through its own context-scoped EngineSink. The fan-out is a routing concern — adapters are context-unaware.

### Public surface rules
32. All writes go through `ingest()`. There is no separate public API for raw graph primitives. Consumers say "here is a fragment," not "create node X with edge Y."
33. Enrichments are registered globally on the engine. They self-select based on events and context state — the framework does not filter events for them.
34. The enrichment loop terminates via idempotency. Each enrichment checks context state before emitting. The framework runs the loop; the enrichment implements the termination condition. `EdgesAdded` fires for ALL committed edges including re-emissions — enrichments must not rely on events alone to detect novelty.
35. Outbound events flow through the adapter. Consumers never see raw graph events. The adapter's `transform_events()` receives all events from the primary emission and all enrichment rounds, and filters what its consumer cares about.
36. Transports are thin shells. All transports call the same `ingest()` and query endpoints. Adding a transport doesn't touch adapters, enrichments, or the engine.
37. Enrichments shared across integrations are deduplicated by `id()`. If two integrations register the same enrichment, it runs once per enrichment loop round.
38. Adapters extend the domain side, enrichments extend the graph intelligence side, transports extend the protocol side. These three dimensions are independent — changes in one don't affect the others.

---

## Resolved Disambiguations

Inconsistencies found across source material, resolved here. These resolutions are binding.

### 1. "Weight" → always qualified
Never use "weight" alone. Three-layer vocabulary (ADR-003): **"contribution"** (stored per-adapter on edge), **"raw weight"** (computed from contributions via scale normalization), or **"normalized weight"** (computed at query time by consumer). Use **"scale normalization"** for the engine-side operation and **"query-time normalization"** for the consumer-side operation — never unqualified "normalization."

### 2. "Emission" is both the domain term and the struct
The Rust struct is `Emission`, matching the domain vocabulary. Older ADRs may reference `AdapterOutput` — that name is superseded.

### 3. "Provenance" → always qualified
Three distinct meanings, always disambiguate:
- **Provenance dimension** — the facet of the graph that stores provenance data
- **Provenance entry** — a single record (the `ProvenanceEntry` struct)
- **Provenance** unqualified — acceptable only when referring to the general concept of tracking origin. Never use it to mean a specific entry or the dimension.

### 4. "Commit" is the verb for the atomic validate-and-persist operation
The design doc sometimes uses "persist." **Use "commit" consistently.** It implies atomicity, which "persist" doesn't.

### 5. Extraction pipeline = adapter internals
The research paper's three-system pipeline (llm-orc → clawmarks → Plexus) maps to the ADR's adapter model as follows: an external adapter like DocumentAdapter *wraps* llm-orc as its internal extraction mechanism and *annotates* with clawmark-equivalent provenance. The pipeline is the adapter's internal business. The framework sees only what exits the sink.

### 6. Emission validation is per-item, not all-or-nothing
The ADR's per-item behavior table (upsert, no-op, reject) applies to each item independently. An edge with a missing endpoint is rejected, but valid nodes and valid edges in the same emission still commit. `emit()` returns a result describing what was rejected. This matches the ADR's resilience-first philosophy ("the framework logs the error and continues") and the practical need for progressive emission — an adapter shouldn't lose 50 valid nodes because one edge has a bad endpoint.

### 7. "Tag" ≠ "Annotation"
Both words appear in the domain, with distinct meanings. A **tag** is an input-side label applied to a fragment (by a human or LLM) describing what the fragment is about — "travel", "avignon". An **annotation** is extraction-side metadata attached to a node or edge in the emission describing how the adapter came to know something — confidence, method, source location. Tags produce concept nodes. Annotations produce provenance entries. Never use "tag" to mean annotation or vice versa.

### 8. "Commit" vs "Persist"
Two distinct steps within a single `emit()` call. **Commit** is the validate-and-write-to-in-memory-context step — endpoint validation, contribution tracking, scale normalization, event firing. **Persist** is the write-to-storage step — `GraphStore.save_context()` called after commit completes. Both happen atomically within `emit()`. Use "commit" when discussing validation and in-memory mutation. Use "persist" when discussing durable storage. The existing disambiguation §4 ("commit is the verb for the atomic validate-and-persist operation") remains correct at the `emit()` level — from the adapter's perspective, `emit()` is one atomic operation that commits and persists.

### 9. "Context" has one meaning
A **context** is a bounded subgraph — a project container holding nodes, edges, and metadata. `ContextId` identifies a context. Do not use "context" to mean "processing environment" or "session." If you mean the adapter's processing environment, use **framework context** (`FrameworkContext` struct). If you mean a user's editing session, use **session**.

### 10. "Adapter" ≠ "Enrichment"
Both produce graph mutations via `Emission`, but they are structurally distinct. An **adapter** has an `input_kind`, receives domain data from a consumer, and transforms events back to the consumer. An **enrichment** has no `input_kind`, receives no domain data, and serves no consumer — it reacts to graph events and mutates the graph. Adapters bridge between a consumer's domain and the graph. Enrichments bridge between dimensions within the graph. Never call an enrichment an adapter or vice versa.

### 11. "Graph event" ≠ "Outbound event"
A **graph event** is a low-level per-mutation notification (`NodesAdded`, `EdgesAdded`, etc.) — internal to the engine. An **outbound event** is a domain-meaningful event translated by the adapter's `transform_events()` for a consumer (`concepts_detected`, `bridges_created`, etc.). Consumers receive outbound events, never raw graph events. When discussing event delivery, always specify which kind.

---

## Open Questions

Unresolved issues that block or constrain behavior scenarios and implementation.

### Resolved by Essay 08 (Runtime Architecture)

**5. Provenance-semantic isolation.** Resolved: marks live in project contexts (provenance dimension), not a global `__provenance__` context. Cross-dimensional `references` edges connect marks to concept nodes via automatic tag-to-concept bridging. See invariants 22–27.

**6. Contribution persistence.** Resolved: `contributions_json` column in edges table. Schema migration follows the dimension migration pattern. See invariant 29.

**7. Adapter-to-engine wiring.** Resolved: EngineSink gains a constructor taking `Arc<PlexusEngine>` + `ContextId`. Routes emissions through the engine with persist-per-emission. See invariant 28.

### Resolved by ADR-003

**1. Reinforcement and multi-source convergence (edge weights).**
Resolved by ADR-003. Reinforcement is implicit: emitting an edge that already exists replaces that adapter's contribution slot with the new value. Per-adapter contributions are stored; raw weight is computed via scale normalization then summation. See ADR-003 for full details.

**Sub-question still open: node property merge on multi-source upsert.** When a second adapter emits a node that already exists, upsert updates properties. But what merge semantics apply? Last-writer-wins loses information (e.g., DocumentAdapter sets `concept_type: "theme"`, MovementAdapter overwrites with `concept_type: "effort_quality"`). Options: union of properties (conflicts become lists), thin nodes with detail in provenance entries only, or domain-specific merge rules. This does not block edge reinforcement implementation but blocks multi-adapter node convergence scenarios.

### Resolved by Essay 09 (Public Surface) — reflexive adapter migration

**2. Reflexive adapter cycle convergence.** Resolved: the reflexive adapter concept has been superseded by enrichments. Enrichments terminate via idempotency — each enrichment checks context state before emitting, and the enrichment loop repeats until quiescence (all enrichments return `None`). See invariant 34. The schedule monitor and ProposalSink are no longer needed.

**3. ProposalSink and non-relational metadata edges.** Resolved: ProposalSink has been removed along with the reflexive adapter concept. Enrichments emit freely through the standard emission pipeline. A topology enrichment can emit any relationship type appropriate to its domain. The constraint that co-occurrence proposals use `may_be_related` is a design convention of the CoOccurrenceEnrichment, not a framework-enforced restriction.

### Needs design decision (doesn't block initial implementation)

**4. Routing semantics: two-dimensional fan-out.**
Routing is now two-dimensional. A source update fans out across contexts (all contexts containing that source) and across adapters (all adapters matching the input kind). One source update hitting 3 contexts with 2 matching adapters each produces 6 independent adapter runs.

Open sub-questions:

- **Concurrency model.** All 6 runs concurrent? Contexts are independent (DashMap per-shard locking, no cross-context edges), and adapters within a context are independent (invariant 14). Full concurrency seems safe but has performance implications for large fan-outs.
- **Ordering.** Fan-out should be context-first (for each context, route to matching adapters), not adapter-first. Each context is the natural unit of independent work. But is there an ordering guarantee within a context across adapters? Probably not needed (invariant 14), but should be stated.
- **Partial failure.** If processing succeeds in 2 of 3 contexts, each context is independently consistent. No cross-context rollback needed. But should failures be surfaced? This is operational, not architectural.
- **Cascade scaling.** A single source update can trigger adapters in N contexts, each producing mutations that trigger enrichment loops. Total work scales as N × (adapters + enrichment rounds per context). Enrichment loops terminate via quiescence (invariant 34).

### Introduced by Essay 09 (Public Surface)

**8. Event persistence and cursor-based delivery.**
Graph events are currently produced and discarded. Cursor-based delivery requires an ordered, persistent event log with sequence numbering. Schema design, retention policy, and the boundary between event persistence and context persistence are unresolved. Pull-based cursors are the recommended foundation; push delivery (WebSockets, SSE) layers on top.

**9. Wire protocol choice.**
gRPC (via tonic) is the recommended app-to-app protocol based on industry survey. But the specific protobuf schema for `ingest()` and query endpoints hasn't been designed. The choice of whether to also offer REST (via tonic-web or a gateway) is open.

**10. Emission removal variant.**
Two provenance operations (`unlink_marks`, `delete_chain` with cascade) don't fit cleanly through the adapter pipeline because `Emission` has no edge removal variant. Adding one would let all writes go through `ingest()`. Without it, these remain engine-level commands outside the adapter pipeline.
