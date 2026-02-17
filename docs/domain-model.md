# Domain Model: Plexus

Ubiquitous language for the Plexus knowledge graph. All ADRs, behavior scenarios, and code must use these terms consistently. If this glossary says "emission," the code says `emission`, not "output batch" or "result set."

Extracted from: ADR-001, semantic-adapters.md, semantic-adapters-design.md, PAPER.md, SPIKE-OUTCOME.md, Essay 07 (first adapter pair), Essay 08 (runtime architecture), Essay 09 (public surface), Essay 12 (provenance as epistemological infrastructure), Essay 13 (two-consumer validation revisited), Essay 17 (storage architecture), Essay 18 (phased extraction architecture), Essay 19 (declarative adapter primitives). Discovery gap concepts from design discussion (2026-02-14). Core/external enrichment unification from Essay 19 research (2026-02-17).

---

## Concepts

| Term | Definition | Aliases to Avoid |
|------|-----------|------------------|
| **Adapter** | A bidirectional integration contract with a dual obligation. Inbound: transforms domain-specific input into graph mutations via `process()`, producing both semantic contributions (concepts, relationships) and provenance contributions (chains, marks, source evidence). The dual obligation is bidirectional — semantic content requires provenance, AND provenance requires semantic content. There is no adapter that produces only one side. Outbound: transforms raw graph events into domain-meaningful events for the consumer via `transform_events()`. Owns its entire processing pipeline internally. The single artifact a consumer needs to understand. | plugin, processor, handler |
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
| **Provenance entry** | The full record of how a piece of knowledge entered the graph. Two sources: (1) adapter-produced provenance — chain and mark nodes emitted directly by the adapter alongside its semantic output, carrying source-meaningful annotation text, file references, and tags; (2) framework-constructed provenance — the engine combines the adapter's annotation with framework context (adapter ID, timestamp, context ID). Adapter-produced provenance is epistemological (where knowledge came from); framework-constructed provenance is operational (how it was processed). | — |
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
| **Input kind** | A string declared by each adapter identifying what type of input it consumes. The router uses this for matching. Examples: `fragment` (tagged writing), `extract-file` (extraction coordinator), `file_content`, `gesture_encoding`. | — |
| **Adapter snapshot** | Optional incremental state from a previous run, passed back to the adapter on re-processing. Contents are adapter-specific (e.g., chunk hashes for documents, cluster centroids for movement). Design deferred. | checkpoint, state |
| **Input router** | Framework component that directs incoming input to all adapters whose `input_kind()` matches. Fan-out: when multiple adapters match, all receive the input. | dispatcher |
| **Fragment** | A piece of writing — a journal entry, SMS message, email, note, annotation — carrying text and tags. The minimum unit of content entering Plexus. Everything that enters the graph is at least a fragment: an annotation's text is a fragment, a research note is a fragment, a code comment is a fragment. A fragment becomes a fragment node in the graph; its tags become concept nodes. Provenance (marks, chains) layers on top of the semantic content the fragment provides. | entry, note, snippet, annotation text (as domain terms — these are all fragments) |
| **Tag** | A label applied to a fragment, either manually by a human or by an LLM. Each tag produces a concept node in the semantic dimension. Distinct from **annotation** — a tag is input-side vocabulary (what the fragment is about); an annotation is extraction-side vocabulary (how the adapter came to know something). | label (acceptable informally), keyword |
| **`tagged_with`** | The edge relationship type from a fragment node to a concept node. Represents "this fragment was labeled with this concept." Contribution value is 1.0 (binary: the tag was applied). | — |
| **Deterministic concept ID** | The scheme by which concept nodes receive stable, convergent IDs derived from their label: `concept:{lowercase_tag}`. Ensures that two fragments tagged "travel" produce the same concept node, triggering upsert rather than creating duplicates. | — |
| **Deterministic chain ID** | The scheme by which chain nodes receive stable, convergent IDs: `chain:{adapter_id}:{source}`. Ensures that re-ingesting from the same source with the same adapter upserts the existing chain rather than creating duplicates. Different adapters processing the same source receive distinct chains, preserving independent provenance trails per processing phase. | — |
| **Co-occurrence** | The pattern where two concepts appear together across multiple source nodes — both connected to the same source via the configured relationship (e.g., `tagged_with` for fragments, `exhibits` for movement). The signal the CoOccurrenceEnrichment detects. Measured by counting shared source nodes between a concept pair. Structure-aware: fires for any source node type, not just fragments (Invariant 50). | correlation (too statistical), co-location |
| **Co-occurrence score** | The normalized count of shared source nodes between a concept pair, used as the contribution value for a `may_be_related` (or configured output relationship) proposal. Normalized relative to the maximum co-occurrence count in the current graph: `count / max_count`. | — |
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
| **ProvenanceApi** | Internal API for mark, chain, and link graph operations. Scoped to a specific context (not global). Creates marks as provenance-dimension nodes, chains as grouping nodes, and links as directed edges. ProvenanceApi is an implementation detail used by ProvenanceAdapter and PlexusApi — it is not a consumer-facing surface. Consumers interact through `annotate` (which produces semantic content alongside provenance) or through adapter-produced provenance (automatic trails from semantic adapters). There is no consumer path that creates provenance without semantic content. | — |
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
| **Replication layer** | Infrastructure that coordinates emission-level replication between Plexus instances across hosts. Two responsibilities at different stack levels: **outbound** (wraps GraphStore — journals persisted emissions, filters by replication tier, ships to peers) and **inbound** (goes through the engine — applies remote emissions via `ingest_replicated()`, runs validation and enrichment loop, skips outbound replication to prevent echo). Neither a transport (consumer-facing) nor a store (persistence), but a coordination layer that spans both. See Essay 17, invariant 38 resolution. | sync layer, federation layer |
| **Replication tier** | A per-context policy controlling what data replicates to peers. Three levels: **semantic-only** (concept nodes, edges, provenance chains/marks — no fragment text), **metadata+semantic** (adds fragment metadata like title and source type, but not full text), **full** (everything). The tier is declared on the shared context, not set globally. Different contexts can replicate at different tiers. | replication scope, sync level |
| **ReplicatedStore** | Extension trait wrapping a base `GraphStore` to add federation capabilities: emission journaling with replication metadata (origin site, version vector), pull-based sync ("emissions since version N"), and remote emission merge with conflict resolution and raw weight recomputation. The base GraphStore stays simple for single-instance use; ReplicatedStore adds federation without changing the core interface. | FederatedStore, SyncStore |
| **Shared-concept convergence** | Query-time discovery of concept nodes that appear in multiple contexts within the same Plexus instance. Requires zero graph changes — deterministic concept IDs (invariant 19) mean the same tag produces the same node ID in every context. A `shared_concepts(context_a, context_b)` query returns the intersection. Discovers exact tag matches, not semantic similarity. | cross-context query (acceptable informally) |
| **Meta-context** | A read-only virtual view that unions nodes and edges from multiple constituent contexts at query time. No data stored — pure composition. No enrichment can run on a meta-context (enrichments produce emissions, which require a target context). Useful for cross-context traversal without mutating any context. | virtual context, composite context |
| **Extraction phase** | One of three progressively-deeper stages of extracting knowledge from a source file, ordered by cost. Each phase boundary corresponds to a real execution-model difference. Phase 1: registration (instant, blocking) — file node + format-specific metadata in a single pass. Phase 2: analysis (moderate, background, Rust) — modality-dispatched heuristic extraction. Phase 3: semantic (slow, background, LLM via llm-orc). Each phase is a separate adapter with its own adapter ID, so contributions accumulate independently. Cheap phases produce immediate value; expensive phases enrich in the background. | stage, step, pass |
| **Extraction coordinator** | An adapter that handles the `extract-file` input kind. Runs Phase 1 synchronously within `ingest()` and returns its outbound events immediately. Spawns Phases 2–3 as sequential background tasks that call `ingest()` again with their own input kinds. Uses Phase 1's MIME type to dispatch to the appropriate modality-specific Phase 2 adapter. Each background phase is an independent adapter run — if Phase 3 fails, Phases 1–2 are already persisted. | orchestrator (ambiguous with llm-orc), dispatcher |
| **Extraction status** | A structure-dimension node tracking which extraction phases have completed for a source file. One per file, queryable by any client via MCP tool (`extraction_status`). Participates in the graph's own consistency model — not external state. | progress tracker |
| **Declarative adapter spec** | A YAML specification that describes an adapter's behavior without Rust code. Declares adapter ID, input kind, input schema, and emit instructions using adapter spec primitives. Interpreted at runtime by DeclarativeAdapter. For external consumers who can't write Rust. Existing Rust adapters remain valid alongside declarative ones. | adapter config, adapter template |
| **DeclarativeAdapter** | A Rust struct implementing the Adapter trait that interprets a declarative adapter spec at runtime. All input is JSON (from any transport). Validates input against the spec's input schema. Produces emissions by executing the spec's primitive instructions. | YAML adapter, generic adapter |
| **Adapter spec primitive** | One of seven building blocks in declarative adapter specs: `create_node`, `create_edge`, `for_each`, `id_template`, `hash_id`, `create_provenance`, `update_properties`. Designed to cover the fragment adapter and all four extraction phases. The `create_provenance` primitive enforces the dual obligation (Invariant 7) structurally. | operation, instruction |
| **Extractor** | Layer 1 of the two-layer extraction architecture. Domain-specific code that processes raw source material and produces structured JSON. Lives outside Plexus — in script agents (Python/Bash via llm-orc), standalone processes, or Rust functions. This is where domain expertise lives: audio analysis, movement algorithms, code parsing, LLM prompting. The extractor does not know about graph structure. | analyzer (too generic) |
| **Declarative mapper** | Layer 2 of the two-layer extraction architecture. The declarative adapter spec that maps extractor-produced JSON to graph nodes and edges. Domain-agnostic — same primitives regardless of whether the input came from audio analysis or movement encoding. The mapper does not know about domain-specific source formats. | graph mapper |
| **Parameterized enrichment** | A built-in enrichment (TagConceptBridger, CoOccurrenceEnrichment) configured with domain-specific parameters instead of hardcoded values. Example: CoOccurrenceEnrichment with `exhibits`/`co_exhibited` instead of `tagged_with`/`may_be_related`. Declared in the adapter spec YAML. Runs in the enrichment loop with full graph-wide reactivity. Not a new enrichment type; a new instantiation of an existing algorithm. | custom enrichment (misleading — it's parameterized, not custom) |
| **Core enrichment** | A Rust-native enrichment implementing a general graph algorithm fundamental to Plexus's discovery capabilities. Fast (microseconds), reactive (fires in the enrichment loop after every emission), parameterizable, and domain-agnostic. Four core enrichments: TagConceptBridger (provenance → semantic bridging), CoOccurrenceEnrichment (shared-source patterns), DiscoveryGapEnrichment (latent-structural disagreement), TemporalProximityEnrichment (timestamp-based co-occurrence). These are not optional plugins — they define what kind of knowledge graph engine Plexus is. | built-in enrichment, Tier 0 enrichment |
| **External enrichment** | An enrichment implemented as an llm-orc ensemble — network science algorithms (PageRank, community detection, betweenness centrality), LLM-based semantic analysis, or custom computation patterns. Runs outside the per-emission enrichment loop. Two trigger modes: on-demand (`plexus analyze`) or emission-triggered (background, fires when new data enters the graph). Results always re-enter via `ingest()`, which triggers core enrichments on the new data. The same ensemble YAML, the same result path — the difference is only when the flow starts. | graph analysis, batch enrichment, declarative flow, Tier 1/Tier 2 enrichment |
| **Template expression** | The limited interpolation language in declarative adapter specs. Supports field access (`{input.tags}`), filters (`{tag | lowercase}`, `sort`, `join`, `default`), and context variables (`{adapter_id}`, `{context_id}`). Intentionally limited — complex transformations belong in extractors or Rust. | template language |
| **Discovery gap** | The delta between structural evidence and latent evidence for a pair of nodes. Three informative states: (1) structurally connected but not latently similar — co-occurrence without semantic kinship, often indicating contrast or juxtaposition in the source material; (2) latently similar but not structurally connected — unexplored adjacency, a discovery signal pointing to territory worth investigating; (3) both — high-confidence relationship corroborated by independent evidence sources. The gap is informative because the two evidence layers are independently sourced: structural evidence comes from human annotations, heuristic extraction, and metadata; latent evidence comes from embedding models. Unlike systems where the graph and embeddings derive from the same LLM (making agreement circular), Plexus's structural evidence has independent provenance, making disagreement between layers a genuine signal. | missing connection, similarity gap |
| **Latent evidence** | Embedding-derived similarity between graph nodes. Continuous (distance in vector space), opaque (captures semantic proximity without naming the relationship), and independently sourced from structural evidence. Enters the graph as edges (e.g., `similar_to`) with contribution tracking and adapter IDs (e.g., `embedding:all-MiniLM-L6-v2`, `embedding:movement-laban-v1`). Does not require perfect accuracy — even a modest embedding model produces useful signal when cross-referenced against structural evidence, because the value is in the discovery gap between the two layers, not in either layer alone. | vector similarity, embedding similarity |
| **Structural evidence** | Explicit graph edges produced by adapters (e.g., `tagged_with`, `exhibits`) and enrichments (e.g., `may_be_related`, `co_exhibited`). Named, typed, traceable through provenance. Each edge has per-adapter contribution tracking and provenance entries. Captures what has been explicitly stated or inferred from source material. Contrasted with latent evidence, which captures what is implicitly similar. | explicit evidence |

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
| **detect co-occurrence** | CoOccurrenceEnrichment | Context snapshot | Scan concept nodes for pairs that share source nodes (via the configured relationship, e.g. `tagged_with`), count shared sources, and compute co-occurrence scores. Structure-aware: fires for any source node type connected via the configured relationship (Invariant 50). |
| **snapshot** | Framework | Context | Clone the Context to create an immutable context snapshot for enrichments. Ensures enrichments see a consistent view unaffected by concurrent mutations. |
| **persist** | Engine | Context (via GraphStore) | Save the in-memory context to persistent storage after a mutation. Distinct from **commit** — commit is the validate-and-write-to-memory step; persist is the write-to-storage step. Both happen within a single `emit()` call. |
| **hydrate** | Engine | DashMap (from GraphStore) | Load all contexts from persistent storage into the in-memory cache on startup. Called once via `load_all()`. |
| **bridge** | Enrichment (TagConceptBridger) | Mark ↔ Concept | Automatically create `references` cross-dimensional edges between marks and concept nodes whose IDs match (after tag format normalization). Runs as part of the enrichment loop, not inline at creation time. Bridges in both directions: new marks to existing concepts, and new concepts to existing marks. |
| **add mark** | ProvenanceApi | Context | Create a mark node in the provenance dimension of a specified project context. Requires a context, chain, file, line, and annotation. Triggers tag-to-concept bridging if tags are provided. |
| **ingest** | Transport | Engine (via router) | Push domain data through the public surface. The transport forwards `(context_id, input_kind, data)` to the router, which matches the adapter, runs the pipeline (process → enrichment loop → outbound transformation), and returns outbound events. |
| **enrich** | Enrichment | Graph (via emission) | React to graph events and produce additional graph mutations. Called by the enrichment loop after each emission round with the accumulated events and a context snapshot. Returns `Some(Emission)` if there is work to do, `None` if quiescent. |
| **transform events** | Adapter | Graph events | Translate raw graph events into domain-meaningful outbound events for the consumer. Called after the enrichment loop completes with all accumulated events from primary emission and all enrichment rounds. The adapter filters what its consumer cares about. |
| **replicate** | Replication layer (outbound) | Emission | Filter a persisted primary emission by the context's replication tier and ship it to peers. Only primary (adapter-produced) emissions replicate; enrichment-produced emissions are excluded to prevent feedback amplification. |
| **ingest_replicated** | Replication layer (inbound) | Engine | Apply a remote emission to the local replica through the engine pipeline: validate, commit, run the enrichment loop, but skip outbound replication to prevent echo. Maintains all invariants (endpoint validation, contribution tracking, enrichment loop) while preventing replication cycles. |
| **journal** | ReplicatedStore | Emission | Persist an emission alongside replication metadata (origin site, version vector) for sync and replay. Enables pull-based catchup: "give me emissions since version N." |
| **extract** | Extraction coordinator | Source file | Process a source file through extraction phases. Phase 1 runs synchronously within a single `ingest()` call; Phases 2–3 are spawned as sequential background tasks that call `ingest()` again. Each phase produces its own emission with its own adapter ID. |
| **run external enrichment** | llm-orc ensemble | Context | Execute an external enrichment on a context's graph. Export graph → run ensemble (network algorithms, LLM analysis, custom scripts) → apply results back through `ingest()`. Two trigger modes: on-demand (`plexus analyze`) or emission-triggered (background). Results re-enter via `ingest()`, triggering core enrichments on the new data. Not part of the per-emission enrichment loop. |

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
- **FragmentAdapter** produces provenance alongside semantics: a **chain** node (**deterministic chain ID** per adapter+source), a **mark** node (carrying annotation text, source file, and tags), and a `contains` edge (chain → mark). All in the **provenance** dimension.
- **TagConceptBridger** automatically creates `references` edges from adapter-produced **marks** to matching **concept** nodes — identical to how it bridges user-created marks. This makes every concept's origin graph-traversable: concept ← `references` ← mark ← `contains` ← chain.
- **CoOccurrenceEnrichment** reads a **context snapshot**, detects **co-occurrence**, and emits `may_be_related` **symmetric edge pairs** between co-occurring **concepts**
- **Co-occurrence score** is the **contribution** value on `may_be_related` edges
- One **FragmentAdapter** type, instantiated with different **adapter IDs** per evidence source (manual, LLM, OCR) — same `process()` logic, distinct **contributions** and **provenance** chains

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

### Storage and replication
- **GraphStore** takes a path (or connection) — the **transport**/host layer decides what to pass (the library rule)
- **ReplicatedStore** wraps **GraphStore** — adding federation capabilities (journaling, sync, merge) without changing the base trait
- **Replication layer** outbound wraps **GraphStore** — journals persisted emissions, filters by **replication tier**, ships to peers
- **Replication layer** inbound goes through **PlexusEngine** — applies remote emissions via `ingest_replicated()`, runs validation and enrichment loop
- **Replication tier** is a policy on a **context** — controls what data replicates (semantic-only, metadata+semantic, or full)
- **Shared-concept convergence** operates across **contexts** within a single **PlexusEngine** — queries deterministic concept ID intersection
- **Meta-context** unions multiple **contexts** at query time — read-only, no enrichment

### Extraction architecture
- **Extraction coordinator** handles `extract-file` input kind; runs Phase 1 blocking, spawns Phases 2–3 as sequential background `ingest()` calls
- Each **extraction phase** is a separate **adapter** with its own **adapter ID** — **contributions** accumulate independently across phases
- **Extraction status** node tracks phase completion per source file in the **structure** dimension
- **Extractor** (Layer 1) produces JSON; **declarative mapper** (Layer 2) maps it to graph **nodes** and **edges** via **adapter spec primitives**
- **DeclarativeAdapter** interprets a **declarative adapter spec** at runtime, implementing the **Adapter** trait
- Phase 3 delegates to llm-orc (external service) for LLM calls; Phases 1–2 are Rust-native
- When llm-orc is unavailable, Phases 1–2 complete normally and Phase 3 is skipped — graceful degradation
- **Parameterized enrichments** are declared in **declarative adapter specs** and run in the **enrichment loop** with full graph-wide reactivity
- **External enrichments** run outside the **enrichment loop**, results entering via **ingest()**
- **Core enrichments** fire after each phase's **emission** — the graph enriches incrementally as phases complete, no "wait for all phases" barrier

### Evidence layers and discovery
- **Structural evidence** and **latent evidence** are independently sourced layers on the same graph — structural from adapters/enrichments with provenance, latent from embedding models
- **Latent evidence** enters via **ingest()** as an **external enrichment** (ADR-023) — batch-computed embeddings, stored as node properties or emitted as `similar_to` edges above a threshold
- **Discovery gap** is computed from the delta between **structural evidence** and **latent evidence** for node pairs — disagreement between layers is a signal, not noise
- **Latent evidence** edges participate in the **enrichment loop** — CoOccurrenceEnrichment parameterized on `similar_to` produces second-order latent bridges
- **Contribution tracking** applies to latent evidence identically — each embedding model has its own adapter ID (e.g., `embedding:all-MiniLM-L6-v2`), enabling multi-model evidence composition via scale normalization

## Invariants

### Emission rules (enforced by engine at commit time)
1. The engine validates and commits each item in the emission independently. Valid items commit; invalid items are rejected individually. The emission is **not** all-or-nothing.
2. Per-item behaviors: edges with missing endpoints are **rejected** (endpoint must exist in the graph or in the same emission). Duplicate node IDs **upsert**. Removal of non-existent nodes is a **no-op**. Self-referencing edges are **allowed**. Empty emissions are a **no-op**.
3. `emit()` returns a result describing what was rejected and why. The adapter can act on rejected items or ignore them. Partial success is the normal case, not an error.
4. Cancellation is checked **between** emissions, not during. Committed items are never rolled back.

### Provenance rules
5. Adapters produce provenance (chains, marks, contains edges) alongside their semantic output. This is the adapter's dual obligation: semantic contribution AND source evidence. Only the adapter understands its source material well enough to produce meaningful provenance — marks with domain-meaningful annotation text, file references, and tags. The engine adds framework context (adapter ID, timestamp) but does not create the epistemological trail.
6. Enrichments do not produce provenance. They react to graph events and produce semantic/relational mutations (co-occurrence edges, cross-dimensional bridges), but provenance originates exclusively from adapters — the components that understand source material.
7. All knowledge entering the graph carries both semantic content and provenance. There is no consumer-facing path that creates provenance without semantic content. An annotation IS a fragment — the annotation text is semantic content, the tags produce concepts, the mark provides provenance. ProvenanceAdapter handles the provenance-dimension mechanics internally but is never the sole adapter on a consumer-facing path. This is the dual obligation read bidirectionally: semantic adapters must produce provenance, AND provenance operations must produce semantic content.

### Weight rules (updated by ADR-003)
8. Per-adapter contributions are stored. Raw weights are computed from contributions via scale normalization. Normalized weights are computed from raw weights at query time. Three layers: contribution (stored) → raw weight (engine-computed) → normalized weight (query-time-computed).
9. No temporal decay. Weakening happens only through normalization as the graph grows around an edge.
10. A quiet graph stays stable — silence is not evidence against previous observations.
11. Contributions use latest-value-replace: each adapter's slot stores the value from its most recent emission. Contributions can increase or decrease.
12. Contributions can be any finite f32 value. Adapters and enrichments emit in whatever scale is natural to their domain (e.g., 0–20 for test counts, 0–500 for gesture repetitions, 0–127 for MIDI velocities, -1.0–1.0 for sentiment). The engine's scale normalization (initially divide-by-range) maps these to comparable ranges regardless of whether the native scale is signed or unsigned.
13. Adapter and enrichment IDs must be stable across sessions. If reconfigured with a new ID, previous contributions become orphaned. Old contributions should be explicitly removed. **Amendment (Essay 17):** For federated contexts, adapter IDs must also be unique per user-instance, not just per adapter type. Example: `carrel:alice` and `carrel:bob`, not just `carrel`. If two users share the same adapter ID, their contributions collide in the same LWW slot, breaking the CRDT alignment that federation requires. The naming convention `{adapter_type}:{user_or_instance_id}` is a prerequisite for emission-level replication.

### Adapter rules
14. The framework never inspects the opaque data payload. Adapters downcast internally.
15. Adapters are independent — they don't know about each other.
16. An adapter owns its full internal pipeline. The framework has no opinion on internal phase ordering.

### Routing rules
17. Input routing is fan-out: all adapters matching the input kind receive the input.
18. Each matched adapter is spawned independently with its own sink and cancellation token.

### Fragment adapter rules
19. Concept IDs are deterministic: `concept:{lowercase_tag}`. Same tag always produces the same node ID, ensuring convergence via upsert.
20. Chain IDs are deterministic: `chain:{adapter_id}:{source}`. Same adapter + source always produces the same chain node, ensuring convergence via upsert. Different adapters processing the same source produce distinct chains.
21. Tags produce binary contributions (1.0) on `tagged_with` edges. The contribution means "this tag was applied," not a graduated strength.
22. Symmetric relationships (`may_be_related`) are emitted as two directed edges (A→B and B→A) with identical contributions, so query-time normalization sees the relationship from both endpoints.
23. The normalization floor ensures the weakest real contribution from any adapter or enrichment maps to ~α (default 0.01), not 0.0. Formula: `(value - min + α·range) / ((1 + α)·range)`. Degenerate case (range = 0) returns 1.0.

### Runtime architecture rules
24. Marks always live in a project context, in the provenance dimension. There is no global `__provenance__` context.
25. `add_mark` requires a context parameter. No default, no fallback.
26. Tag format normalization is an invariant: strip `#` prefix if present, lowercase, then prepend `concept:` to match concept node IDs. `#Travel` → `concept:travel`, `travel` → `concept:travel`.
27. Tag-to-concept bridging is automatic via the enrichment loop. A `TagConceptBridger` enrichment detects new concept nodes, finds marks with matching tags, and creates cross-dimensional `references` edges. Because the enrichment runs on every emission round (not just mark creation), it bridges in both directions: a new mark bridges to existing concepts, and a new concept bridges to existing marks. Works identically for all marks regardless of origin.
28. `list_tags(context_id)` is scoped to a single context, consistent with all other API operations. Tags are per-context — there is no cross-context tag aggregation at the API layer.
29. Tags are the shared vocabulary between provenance and semantics. A tag string on a mark and the ID of a concept node must use the same normalized form.
30. Persist-per-emission: each `emit()` call results in exactly one `save_context()` call. Emissions are the persistence boundary.
31. Contributions must survive persistence. After save → load, `edge.contributions` must be identical. Scale normalization depends on this.
32. Sources can appear in multiple contexts. A file that is a source in both `trellis` and `desk` contexts produces independent graph nodes in each. Overlap is handled by Plexus, not by the contributing tools.
33. A source update fans out to all contexts that include it. Each context gets its own adapter run through its own context-scoped EngineSink. The fan-out is a routing concern — adapters are context-unaware.

### Public surface rules
34. All writes go through `ingest()`. There is no separate public API for raw graph primitives. Consumers say "here is a fragment," not "create node X with edge Y."
35. Enrichments are registered globally on the engine. They self-select based on events and context state — the framework does not filter events for them.
36. The enrichment loop terminates via idempotency. Each enrichment checks context state before emitting. The framework runs the loop; the enrichment implements the termination condition. `EdgesAdded` fires for ALL committed edges including re-emissions — enrichments must not rely on events alone to detect novelty.
37. Outbound events flow through the adapter. Consumers never see raw graph events. The adapter's `transform_events()` receives all events from the primary emission and all enrichment rounds, and filters what its consumer cares about.
38. Transports are thin shells. All transports call the same `ingest()` and query endpoints. Adding a transport doesn't touch adapters, enrichments, or the engine.
39. Enrichments shared across integrations are deduplicated by `id()`. If two integrations register the same enrichment, it runs once per enrichment loop round.
40. Adapters extend the domain side, enrichments extend the graph intelligence side, transports extend the protocol side. These three dimensions are independent — changes in one don't affect the others.

### Storage rules (added by Essay 17)
41. **The library rule:** `GraphStore` takes a path (or connection). The transport/host layer decides what to pass. Plexus the library never decides where to store data. The MCP server picks the path via XDG conventions; Sketchbin picks from its own config; a managed server picks from deployment config. Storage location is an infrastructure concern, not an engine concern.
42. Only primary (adapter-produced) emissions replicate across federated instances. Enrichment-produced emissions are local to each replica. This prevents feedback amplification — without it, an enrichment's output replicating to a peer would trigger that peer's enrichment loop, which would replicate back, creating an infinite cycle.
43. Replication tier is a per-context policy, not a global setting. A context declares whether it replicates semantic-only, metadata+semantic, or full. Different contexts on the same instance can use different tiers.
44. The replication layer is not a transport. It is infrastructure that coordinates between the engine (inbound: `ingest_replicated()`) and the store (outbound: emission journaling). Consumer-facing transports remain thin shells per invariant 38. The replication layer is invisible to consumers.

### Extraction rules (added by Essay 18)
45. Each extraction phase has a distinct adapter ID. Phase contributions accumulate in separate slots — Phase 2 discovering a concept via link extraction and Phase 3 discovering the same concept via LLM analysis produce two contribution slots on the same edge. Cross-phase evidence diversity strengthens raw weight automatically. Phase 2 adapters are modality-specific, each with their own adapter ID (e.g., `extract-analysis-text`, `extract-analysis-audio`).
46. Background extraction phases (2, 3) are independent adapter runs. Each calls `ingest()` with its own input kind and adapter ID. Failure of a background phase does not affect already-persisted results from earlier phases. The graph is useful from the moment Phase 1 completes.
47. Phase 3 (semantic, LLM) gracefully degrades when llm-orc is unavailable. Phases 1–2 complete normally; Phase 3 is skipped. The graph lacks LLM-derived semantic enrichment but is otherwise fully functional. No hard failure.
48. The `create_provenance` adapter spec primitive enforces the provenance half of the dual obligation (Invariant 7): it always produces chain + mark + contains edge. The semantic half (concept nodes, relationship edges) depends on the spec also containing semantic `create_node` directives. DeclarativeAdapter validates at registration time that specs using `create_provenance` also produce semantic content — enforcing both halves through primitive structure plus validation.
49. External enrichment results enter the graph via `ingest()`, not through the enrichment loop. External enrichments are not reactive per-emission — they are triggered on-demand or by emission (as background tasks). The enrichment loop (Invariant 36) governs core enrichments; external enrichments operate outside it. When external enrichment results re-enter via `ingest()`, core enrichments fire on the new data — creating a layered response: immediate (core, microseconds) → background (external, seconds) → delayed (core again on new data).
50. Enrichments are structure-aware, not type-aware. An enrichment fires based on graph structure (edge relationships, dimension membership) rather than node content type. CoOccurrenceEnrichment fires for any pair of nodes connected via the configured relationship, regardless of which adapter produced the source nodes. This aligns with Invariant 40: enrichments extend graph intelligence independently of domain.

---

## Amendment Log

| # | Date | Invariant | Change | Propagation |
|---|------|-----------|--------|-------------|
| 1 | 2026-02-14 | 13 (adapter ID stability) | **Strengthened:** added federation requirement — adapter IDs must be unique per user-instance (`{type}:{user_id}`), not just per adapter type. Without this, per-adapter LWW contribution slots collide across users, breaking CRDT alignment. | ADR-003 references adapter IDs but does not address federation scoping. Any ADR resolving federation must adopt the `{type}:{instance}` convention. |

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

### 12. "Annotate" has two meanings at different layers
The adapter-level **annotate** action (§Actions table) means attaching extraction metadata (confidence, method, source location) to a node or edge in an emission — an internal action within the adapter's `process()` method. The consumer-facing **annotate** operation (ADR-015, updated) means marking a file location with tags in a named chain — a `PlexusApi` workflow that produces both semantic content (the annotation text as a fragment, tags as concepts) and provenance (mark in a chain). The annotation text IS a fragment; provenance layers on top. Context disambiguates — if a consumer calls it, it's the workflow; if an adapter does it during processing, it's the metadata attachment.

### 13. "Core enrichment" vs "External enrichment"
Both improve the graph beyond what adapters directly emit. They are two categories of the same concept — enrichment — distinguished by where the computation happens and how fast it needs to be. A **core enrichment** is a Rust-native general graph algorithm: reactive (fires in the enrichment loop after every emission), fast (microseconds), and terminates via idempotency. An **external enrichment** is an llm-orc ensemble: runs outside the enrichment loop, triggered on-demand or by emission (as a background task), results re-entering via `ingest()`. Never call an external enrichment a "core enrichment" — it does not participate in the enrichment loop (Invariant 49). But both are enrichments — the old term "graph analysis" obscured this unity.

### 14. "Structural evidence" ≠ "Latent evidence"
Both are evidence layers in the graph, but they capture different kinds of knowledge through different mechanisms. **Structural evidence** is explicit, named, and traceable — edges like `tagged_with`, `co_exhibited`, `may_be_related` with per-adapter contribution tracking and provenance. You can explain *why* two nodes are connected. **Latent evidence** is implicit, continuous, and opaque — embedding-derived proximity that captures semantic similarity without naming the relationship. You can measure *how close* two nodes are but not articulate *why*. The two layers are independently sourced, which makes the discovery gap between them informative. Structural evidence without latent support may indicate incidental co-occurrence; latent similarity without structural support may indicate unexplored territory.

### 15. "Extractor" ≠ "Adapter"
An **extractor** (Layer 1) is domain-specific code that produces structured JSON from raw source material — it lives outside Plexus and knows nothing about graph structure. An **adapter** bridges between a consumer's domain and the graph — it receives domain data and produces graph mutations via emissions. The **declarative mapper** (Layer 2) is an adapter (specifically, a DeclarativeAdapter interpreting a YAML spec) that consumes the extractor's JSON. The extractor is upstream of Plexus; the adapter is inside Plexus.

---

## Open Questions

Unresolved issues that block or constrain behavior scenarios and implementation.

### Resolved by Essay 08 (Runtime Architecture)

**5. Provenance-semantic isolation.** Resolved: marks live in project contexts (provenance dimension), not a global `__provenance__` context. Cross-dimensional `references` edges connect marks to concept nodes via automatic tag-to-concept bridging. See invariants 24–29.

**6. Contribution persistence.** Resolved: `contributions_json` column in edges table. Schema migration follows the dimension migration pattern. See invariant 31.

**7. Adapter-to-engine wiring.** Resolved: EngineSink gains a constructor taking `Arc<PlexusEngine>` + `ContextId`. Routes emissions through the engine with persist-per-emission. See invariant 30.

### Resolved by ADR-003

**1. Reinforcement and multi-source convergence (edge weights).**
Resolved by ADR-003. Reinforcement is implicit: emitting an edge that already exists replaces that adapter's contribution slot with the new value. Per-adapter contributions are stored; raw weight is computed via scale normalization then summation. See ADR-003 for full details.

**Sub-question still open: node property merge on multi-source upsert.** When a second adapter emits a node that already exists, upsert updates properties. But what merge semantics apply? Last-writer-wins loses information (e.g., DocumentAdapter sets `concept_type: "theme"`, MovementAdapter overwrites with `concept_type: "effort_quality"`). Options: union of properties (conflicts become lists), thin nodes with detail in provenance entries only, or domain-specific merge rules. This does not block edge reinforcement implementation but blocks multi-adapter node convergence scenarios.

### Resolved by Essay 09 (Public Surface) — reflexive adapter migration

**2. Reflexive adapter cycle convergence.** Resolved: the reflexive adapter concept has been superseded by enrichments. Enrichments terminate via idempotency — each enrichment checks context state before emitting, and the enrichment loop repeats until quiescence (all enrichments return `None`). See invariant 36. The schedule monitor and ProposalSink are no longer needed.

**3. ProposalSink and non-relational metadata edges.** Resolved: ProposalSink has been removed along with the reflexive adapter concept. Enrichments emit freely through the standard emission pipeline. A topology enrichment can emit any relationship type appropriate to its domain. The constraint that co-occurrence proposals use `may_be_related` is a design convention of the CoOccurrenceEnrichment, not a framework-enforced restriction.

### Needs design decision (doesn't block initial implementation)

**4. Routing semantics: two-dimensional fan-out.**
Routing is now two-dimensional. A source update fans out across contexts (all contexts containing that source) and across adapters (all adapters matching the input kind). One source update hitting 3 contexts with 2 matching adapters each produces 6 independent adapter runs.

Open sub-questions:

- **Concurrency model.** All 6 runs concurrent? Contexts are independent (DashMap per-shard locking, no cross-context edges), and adapters within a context are independent (invariant 15). Full concurrency seems safe but has performance implications for large fan-outs.
- **Ordering.** Fan-out should be context-first (for each context, route to matching adapters), not adapter-first. Each context is the natural unit of independent work. But is there an ordering guarantee within a context across adapters? Probably not needed (invariant 15), but should be stated.
- **Partial failure.** If processing succeeds in 2 of 3 contexts, each context is independently consistent. No cross-context rollback needed. But should failures be surfaced? This is operational, not architectural.
- **Cascade scaling.** A single source update can trigger adapters in N contexts, each producing mutations that trigger enrichment loops. Total work scales as N × (adapters + enrichment rounds per context). Enrichment loops terminate via quiescence (invariant 36).

### Introduced by Essay 09 (Public Surface)

**8. Event persistence and cursor-based delivery.**
Graph events are currently produced and discarded. Cursor-based delivery requires an ordered, persistent event log with sequence numbering. Schema design, retention policy, and the boundary between event persistence and context persistence are unresolved. Pull-based cursors are the recommended foundation; push delivery (WebSockets, SSE) layers on top.

**9. Wire protocol choice.**
gRPC (via tonic) is the recommended app-to-app protocol based on industry survey. But the specific protobuf schema for `ingest()` and query endpoints hasn't been designed. The choice of whether to also offer REST (via tonic-web or a gateway) is open.

### Resolved by ADR-012 implementation

**10. Emission removal variant.**
Resolved: `Emission` now has both `removals: Vec<Removal>` (node removals) and `edge_removals: Vec<EdgeRemoval>` (edge removals). `ProvenanceAdapter` handles `DeleteMark`, `UnlinkMarks`, and `DeleteChain` through the adapter pipeline via these variants. MCP routes all three through `pipeline.ingest()`. The engine's `emit_inner` handles node removals with edge cascade and edge removals as targeted operations. `ProvenanceApi` retains direct methods for these operations but they are vestigial — unused by any transport.

### Introduced by Essay 17 (Storage Architecture)

**11. Enrichment coordination across federated replicas.**
When replicas have different enrichment configurations (e.g., Alice has enrichments A, B, C; Bob has B, C, D), their derived structure diverges permanently. Enrichment output is local (invariant 42 — only primary emissions replicate), so replica A develops graph structure that replica B never sees and vice versa.

A deeper issue: **not all enrichments are deterministic.** Enrichments like CoOccurrenceEnrichment are purely algorithmic — same input, same output. But enrichments involving LLM-based semantic interpretation (e.g., theme extraction, semantic clustering, relationship inference) are inherently non-deterministic. Two replicas running the same LLM enrichment on the same primary data will produce different derived structure — not because their configurations differ, but because the enrichment itself produces different output on each run. This means convergence is not just about matching enrichment *configuration* across replicas; it's about enrichment *nature*. Deterministic enrichments (counting, bridging, co-occurrence) can converge; non-deterministic enrichments (LLM interpretation) cannot, even in the ideal case of identical configuration and identical primary data.

Three philosophical approaches, each with different tradeoffs:

- **Option A: Divergence is the feature.** Each user's enrichment configuration is their analytical lens. Shared primary data (fragments, tags, adapter-produced provenance) is common ground; the enrichment layer is personal. Analogous to Obsidian plugins — everyone sees the same notes, but different users have different derived views. Simplest to implement; no coordination needed. For non-deterministic enrichments, this is the only option that doesn't require designating a canonical replica. Risk: users in the same collaborative context may draw different conclusions from the same evidence because their graphs have different structure.

- **Option B: Context-declared enrichments.** A shared context's metadata declares which enrichments are required. Joining a shared context registers those enrichments on your engine. The enrichment *configuration* replicates (as context metadata), not the enrichment *output*. Each replica still runs its own enrichment loop. For deterministic enrichments, same inputs produce the same derived structure. For non-deterministic enrichments, **convergence is not guaranteed even with identical configuration** — this option only ensures everyone runs the same enrichments, not that they produce the same results. More coordination; preserves local execution. Risk: forces enrichment compatibility across heterogeneous instances; gives a false sense of convergence for LLM-based enrichments.

- **Option C: Canonical enrichment output replicates.** Some enrichments are designated as "canonical" for a shared context. Their output replicates as if it were primary data. Other enrichments remain local lenses. Hybrid: canonical enrichments provide shared derived structure; local enrichments add personal perspective on top. **This is the only option that guarantees convergence for non-deterministic enrichments** — one designated replica runs the canonical enrichment, and its output propagates to all peers. Most complex; requires distinguishing canonical from local enrichment output in the replication layer, and designating which replica is authoritative for each canonical enrichment. Risk: canonical enrichment output triggering local enrichment loops on the receiving end (feedback potential — needs careful design); single point of failure for the canonical replica.

These are genuinely different philosophical choices about what "shared understanding" means in a collaborative knowledge graph. Option A says "shared data, private analysis." Option B says "shared analysis configuration." Option C says "shared canonical analysis, private extensions." The non-determinism of LLM-based enrichments makes Options A and C the most honest — A accepts divergence; C forces convergence through authority. Option B occupies an uncomfortable middle: it coordinates configuration but cannot guarantee convergence for the enrichments most likely to produce interesting semantic structure. The choice affects the replication layer design, the context metadata schema, and the enrichment loop's relationship to federation. Does not block single-instance implementation but must be resolved before federation design.

### Resolved by ADR-022 implementation (Phased Extraction)

**12. Enrichment quiescence under broader trigger scope.**
Resolved. CoOccurrenceEnrichment's idempotency holds with heterogeneous source nodes. The `output_edge_exists()` guard and structure-aware firing (Invariant 50) ensure quiescence regardless of source adapter mix. Test `quiescence_with_heterogeneous_sources` in `src/adapter/cooccurrence.rs` verifies: a context with Document + Code source nodes, both with `tagged_with` edges to shared concepts, reaches quiescence in exactly 2 rounds (1 productive + 1 quiescent). The `count / max_count` normalization is stable because it depends on graph structure (edge relationships), not on which adapter produced the source nodes.

**13. Core enrichment termination guarantees.**
Resolved. Core enrichments terminate via idempotency — each checks context state before emitting. Three structural constraints apply to any future emission-triggered external enrichment that participates in the enrichment loop:

1. **No self-triggering output:** the enrichment MUST NOT create nodes or edges of a type it matches on. This prevents the enrichment from producing output that triggers itself in the next round.
2. **Mandatory guard:** every enrichment spec MUST include a `guard` clause checking output existence before emitting. This is the idempotency mechanism — if the desired edge already exists, the enrichment is quiescent.
3. **Edge-only emission:** enrichments emit edges and property updates on existing nodes — not new node creation. Enrichments bridge existing structure; they don't create new entities. This restricts the output space to a finite set (bounded by existing node pairs).

Combined with the existing safety valve (`max_rounds` in `EnrichmentRegistry`, default 10), these constraints make non-termination structurally impossible for well-formed specs and bounded for malformed ones. Note: external enrichments bypass these constraints because they run outside the enrichment loop — their results re-enter via `ingest()`, and core enrichments apply the termination constraints on the resulting data.

**Related: non-determinism as a confidence signal.** LLM enrichment variance is itself informative. If the same enrichment run N times consistently extracts theme "ambient", that's high confidence. If it produces a different theme each run, either the source material lacks a clear theme or the enrichment is ineffective for this content type. Cross-run consistency could feed into annotation confidence. In a BYO-LLM world (Claude one run, Mistral the next), cross-model agreement is a form of evidence diversity — different LLM providers agreeing on a theme is structurally analogous to different adapters independently producing the same concept. The existing contribution model (per-adapter LWW slots, scale normalization, summation) supports this naturally: `theme-extraction:claude` and `theme-extraction:mistral` as separate adapter IDs, with convergence across slots strengthening raw weight. Research direction for LLM enrichment design.

**14. Embedding integration and discovery gap architecture.**
Latent evidence (embedding-derived similarity) would enter the graph as an external enrichment (ADR-023): batch-computed embeddings stored as node properties, optionally producing `similar_to` edges above a similarity threshold. The discovery gap between structural and latent evidence is informative because the two layers are independently sourced. Open sub-questions:

- **Embedding production.** Which embedding models for which domains? A general-purpose text embedding (e.g., all-MiniLM-L6-v2) covers Trellis and Carrel; a movement-quality embedding informed by Laban effort dimensions covers EDDI. Should embedding be an external enrichment (whole-graph batch), a per-file extraction phase, or both?
- **Discovery gap computation.** Partially resolved by Essay 19: `DiscoveryGapEnrichment` is a core enrichment that reacts to new `similar_to` edges and flags structurally-disconnected but latently-similar pairs. Remaining question: should a batch sweep (external enrichment) also compute gaps for the whole graph periodically, or is the reactive core enrichment sufficient?
- **Embedding drift.** When embedding models are updated, `similar_to` edges shift. Old contribution slots become stale. Should model updates trigger full re-embedding (expensive) or incremental update? How does this interact with contribution tracking — does a new model version get a new adapter ID?
- **Cross-modal embeddings.** If text fragments and movement descriptions are embedded into the same vector space, cross-modal discovery gaps become possible — finding that a writing fragment is latently similar to a movement quality. This is a natural extension of the shared concept namespace but in continuous rather than discrete space.

Does not block extraction architecture or external enrichment work. Shapes the future design of external enrichment ensembles and may introduce a new dimension (latent) alongside structure, semantic, and provenance.
