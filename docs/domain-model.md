# Domain Model: Plexus

Ubiquitous language for the Plexus knowledge graph. All ADRs, behavior scenarios, and code must use these terms consistently. If this glossary says "emission," the code says `emission`, not "output batch" or "result set."

Extracted from: ADR-001, semantic-adapters.md, semantic-adapters-design.md, PAPER.md, SPIKE-OUTCOME.md, Essay 07 (first adapter pair), Essay 08 (runtime architecture).

---

## Concepts

| Term | Definition | Aliases to Avoid |
|------|-----------|------------------|
| **Adapter** | A self-contained unit that transforms domain-specific input into graph mutations. Owns its entire processing pipeline internally. The framework sees only what exits the sink. | plugin, processor, handler |
| **External adapter** | An adapter triggered by outside input arriving (a file change, text fragment, gesture encoding). Runs when matching input is routed to it. | input adapter (acceptable in diagrams, not in code) |
| **Reflexive adapter** | An adapter triggered by a schedule condition that examines accumulated graph state and proposes refinements. Receives a ProposalSink, not a full AdapterSink. | internal adapter, meta-adapter |
| **Sink** | The interface through which an adapter pushes graph mutations into the engine. `AdapterSink` is the full interface; `ProposalSink` is the constrained variant for reflexive adapters. | output, writer, channel |
| **ProposalSink** | A constrained wrapper around AdapterSink given to reflexive adapters. Clamps contribution values to a cap, only allows `may_be_related` edges, and rejects node removals. The adapter's `process()` signature is unchanged — it still receives `&dyn AdapterSink`. | — |
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
| **`may_be_related`** | The edge relationship type emitted by reflexive adapters to propose a connection between concepts. Starts weak. Graph dynamics (reinforcement from actual co-occurrence) determine whether the proposal is real. | suggested, proposed (as relationship names) |
| **Evidence diversity** | How corroborated an edge is — derived by querying provenance (count distinct adapter IDs, source types, and contexts). Not a stored field. Four different kinds of evidence are more trustworthy than a hundred of the same kind. | — |
| **Normalization strategy** | A pluggable function that computes normalized weight from raw weight at query time. Default: per-node outgoing divisive normalization (`w_ij / Σ_k w_ik`). | — |
| **Schedule** | A trigger condition for reflexive adapters: periodic (timer), mutation threshold (count), or arbitrary condition on graph state. | — |
| **Adapter input** | The envelope the framework hands to an adapter: context ID, opaque data payload (`Box<dyn Any>`), trigger type, and optional previous snapshot. The framework manages the envelope; the adapter downcasts the data. | — |
| **Context ID** | Identifies a context. Examples from the current ecosystem: `trellis` (all fragments, raw captures), `desk` (promoted seeds, active clusters, research digests), `carrel-research` (research digests, writing marks, voice profiles). The key into PlexusEngine's DashMap. | session ID (close but not identical) |
| **Cancellation token** | A cooperative signal that an adapter's in-flight work has been superseded. Checked between emissions, not during. Already-committed emissions remain valid. | — |
| **Graph event** | A low-level notification fired per mutation type when an emission is committed. Five kinds: NodesAdded, EdgesAdded, NodesRemoved, EdgesRemoved, WeightsChanged. Higher-level events are modeled as nodes/edges from reflexive adapters, not as additional event types. | — |
| **Input kind** | A string declared by each adapter identifying what type of input it consumes. The router uses this for matching. Examples: `fragment` (tagged writing), `graph_state` (reflexive adapter snapshot), `file_content`, `gesture_encoding`. | — |
| **Adapter snapshot** | Optional incremental state from a previous run, passed back to the adapter on re-processing. Contents are adapter-specific (e.g., chunk hashes for documents, cluster centroids for movement). Design deferred. | checkpoint, state |
| **Input router** | Framework component that directs incoming input to all adapters whose `input_kind()` matches. Fan-out: when multiple adapters match, all receive the input. | dispatcher |
| **Schedule monitor** | Framework component that evaluates schedule conditions against graph state and fires reflexive adapters when conditions are met. Observes graph events (for MutationThreshold) or queries graph state directly (for Condition). | scheduler, cron |
| **Fragment** | A piece of writing — a journal entry, SMS message, email, note — carrying text and tags. The input type for Trellis's FragmentAdapter. A fragment becomes a fragment node in the graph; its tags become concept nodes. | entry, note, snippet (as domain terms — these are all fragments) |
| **Tag** | A label applied to a fragment, either manually by a human or by an LLM. Each tag produces a concept node in the semantic dimension. Distinct from **annotation** — a tag is input-side vocabulary (what the fragment is about); an annotation is extraction-side vocabulary (how the adapter came to know something). | label (acceptable informally), keyword |
| **`tagged_with`** | The edge relationship type from a fragment node to a concept node. Represents "this fragment was labeled with this concept." Contribution value is 1.0 (binary: the tag was applied). | — |
| **Deterministic concept ID** | The scheme by which concept nodes receive stable, convergent IDs derived from their label: `concept:{lowercase_tag}`. Ensures that two fragments tagged "travel" produce the same concept node, triggering upsert rather than creating duplicates. | — |
| **Co-occurrence** | The pattern where two concepts appear together across multiple fragments — both tagged on the same fragment. The signal the CoOccurrenceAdapter detects. Measured by counting shared fragments between a concept pair. | correlation (too statistical), co-location |
| **Co-occurrence score** | The normalized count of shared fragments between a concept pair, used as the contribution value for a `may_be_related` proposal. Normalized relative to the maximum co-occurrence count in the current graph: `count / max_count`. | — |
| **Graph state snapshot** | A cloned Context passed as the opaque payload in AdapterInput for reflexive adapters. Provides a consistent, immutable view of the graph at trigger time. The framework (or test harness) creates the snapshot; the adapter downcasts and reads it. | — |
| **Symmetric edge pair** | Two directed edges (A→B and B→A) with identical contributions, representing a semantically symmetric relationship in the directed graph model. Used for `may_be_related` proposals so that query-time normalization (outgoing divisive) sees the relationship from both endpoints. | bidirectional edge, undirected edge |
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
| **Tag-to-concept bridging** | Automatic creation of `references` edges from a mark to concept nodes when the mark's tags match concept node IDs in the same context. Happens inline at mark creation time. The connection between what a user annotates and what the adapter layer discovered. | auto-bridging |
| **Tag format normalization** | The convention that converts mark tags to concept node IDs: strip `#` prefix, prepend `concept:`. Tag `#travel` matches node `concept:travel`. Must be consistent across all mark creation and concept node creation paths. | — |
| **Persist-per-emission** | The persistence strategy: one `save_context()` call at the end of each `emit()` invocation. Atomic per-emission. Batch optimization deferred. | — |

## Actions

| Action | Actor | Target | Description |
|--------|-------|--------|-------------|
| **emit** | Adapter | Sink | Push an emission (nodes + edges + removals) through the sink. Async — the adapter awaits and receives validation feedback. |
| **intercept** | ProposalSink | Emission | Check emission against proposal constraints (weight cap, relationship type, no removals) *before* forwarding to the engine. Rejects or clamps locally. |
| **commit** | Engine | Graph | Validate each item in the emission and persist valid mutations. Fires graph events for committed items. Invalid items (edges with missing endpoints) are rejected individually; valid items in the same emission still commit. |
| **reject** | Engine | Edge | Refuse an individual edge whose endpoints don't exist in the graph or in the same emission. The rejection is reported in the result returned to the adapter. Other items in the same emission are unaffected. |
| **upsert** | Engine | Node | When a node with a duplicate ID is emitted, update its properties rather than creating a second node. This is not an error — it's the expected path for re-processing. |
| **reinforce** | Engine | Edge | Update an existing edge's contribution for the emitting adapter. Triggered implicitly when an adapter emits an edge that already exists — the engine replaces that adapter's contribution slot with the new value. No separate API; reinforcement happens through `emit()`. Contributions can increase or decrease. See ADR-003. |
| **normalize** | NormalizationStrategy | Edge | Compute the relative weight of an edge at query time from its raw weight and graph context. |
| **propose** | Reflexive adapter | Graph (via ProposalSink) | Emit a weak `may_be_related` edge suggesting a connection. "Propose" is the domain verb; the adapter still calls `sink.emit()`. |
| **clamp** | ProposalSink | Contribution value | Cap a proposed edge's contribution value to the configured maximum before forwarding to the inner sink. The ProposalSink intercepts before the engine computes raw weight. |
| **route** | Input router | Adapter(s) | Direct incoming input to all adapters whose `input_kind()` matches. Fan-out: multiple adapters can declare the same input kind. |
| **cancel** | Framework | Adapter (via token) | Signal that in-flight work has been superseded. Adapter checks cooperatively between emissions. |
| **annotate** | Adapter | Node or Edge | Attach extraction metadata (confidence, method, source location) to a node or edge in the emission. |
| **construct provenance** | Engine | Provenance entry | Combine the adapter's annotation with framework context (adapter ID, timestamp, context ID, input summary) to create a full provenance record. |
| **tag** | Human or LLM | Fragment | Apply a label to a fragment, upstream of the adapter. The adapter receives already-tagged input. Tagging is not an adapter action — it happens before the framework sees the fragment. |
| **detect co-occurrence** | CoOccurrenceAdapter | Graph state snapshot | Scan concept nodes for pairs that share fragments (via `tagged_with` edges), count shared fragments, and compute co-occurrence scores. The adapter's core computation. |
| **snapshot** | Framework (or test harness) | Context | Clone the Context to create an immutable graph state snapshot for a reflexive adapter's input payload. Ensures the adapter sees a consistent view unaffected by concurrent mutations. |
| **persist** | Engine | Context (via GraphStore) | Save the in-memory context to persistent storage after a mutation. Distinct from **commit** — commit is the validate-and-write-to-memory step; persist is the write-to-storage step. Both happen within a single `emit()` call. |
| **hydrate** | Engine | DashMap (from GraphStore) | Load all contexts from persistent storage into the in-memory cache on startup. Called once via `load_all()`. |
| **bridge** | ProvenanceApi | Mark → Concept | Automatically create `references` cross-dimensional edges from a newly created mark to concept nodes whose IDs match the mark's tags (after tag format normalization). Inline at mark creation time. |
| **add mark** | ProvenanceApi | Context | Create a mark node in the provenance dimension of a specified project context. Requires a context, chain, file, line, and annotation. Triggers tag-to-concept bridging if tags are provided. |

## Relationships

### Structural
- **Adapter** emits through **sink**
- **ProposalSink** wraps **AdapterSink** (implements the same trait, adds constraints)
- **Emission** contains **AnnotatedNodes**, **AnnotatedEdges**, and removals
- **AnnotatedNode** pairs a **Node** with an optional **Annotation**
- **AnnotatedEdge** pairs an **Edge** with an optional **Annotation**
- **Engine** constructs **provenance entry** from **annotation** + framework context
- **Edge** carries per-adapter **contributions** and a relationship type
- **Engine** computes **raw weight** from **contributions** via **scale normalization**
- **NormalizationStrategy** derives **normalized weight** from **raw weight** + graph context

### Behavioral
- **Input router** fan-outs to all **adapters** matching **input kind**
- **Schedule monitor** observes **graph events** and evaluates **schedule** conditions to trigger **reflexive adapters**
- **Engine** fires **graph events** when **emission** is committed
- **Reflexive adapter** receives **ProposalSink**, not **AdapterSink** — this is how the framework selects behavior
- **ProposalSink** intercepts before **engine** commits — two validation layers
- **Cancellation token** is checked between **emissions**, not during

### Domain
- All domains contribute **concepts** to a shared semantic namespace
- **Content type** disambiguates which domain produced a **node**
- Independent adapters producing the same **concept** label creates cross-modal agreement
- **Reflexive adapters** bridge vocabulary gaps via `may_be_related` proposals
- **Evidence diversity** is derived at query time from **provenance entries** — no component computes or stores it proactively

### Fragment adapter pair
- **Fragment** carries text and **tags**; each tag produces a **concept** node
- **Fragment node** lives in the **structure** dimension; **concept nodes** live in the **semantic** dimension
- **`tagged_with`** edges connect **fragment nodes** to **concept nodes** (one per tag)
- Multiple **fragments** sharing a **tag** converge on the same **concept** node via **deterministic concept ID** and **upsert**
- **CoOccurrenceAdapter** reads a **graph state snapshot**, detects **co-occurrence**, and **proposes** `may_be_related` **symmetric edge pairs** between co-occurring **concepts**
- **Co-occurrence score** is the **contribution** value on proposed `may_be_related` edges
- One **FragmentAdapter** type, instantiated with different **adapter IDs** per evidence source (manual, LLM, OCR) — same `process()` logic, distinct **contributions** and **provenance**

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

### ProposalSink rules (enforced by ProposalSink before engine sees the emission)
4. Reflexive adapters can only emit edges with relationship `may_be_related`. Other relationship types are rejected by the ProposalSink.
5. Reflexive adapters cannot remove nodes.
6. ProposalSink clamps contribution values to a configurable per-adapter cap.
7. Reflexive adapters **can** emit nodes (e.g., topology metadata) and annotations.

### Provenance rules
8. Adapters **never** construct provenance entries directly — they annotate, the engine wraps.
9. Nodes emitted without annotation still receive provenance marks (structural context only).

### Weight rules (updated by ADR-003)
10. Per-adapter contributions are stored. Raw weights are computed from contributions via scale normalization. Normalized weights are computed from raw weights at query time. Three layers: contribution (stored) → raw weight (engine-computed) → normalized weight (query-time-computed).
11. No temporal decay. Weakening happens only through normalization as the graph grows around an edge.
12. A quiet graph stays stable — silence is not evidence against previous observations.
13. Contributions use latest-value-replace: each adapter's slot stores the value from its most recent emission. Contributions can increase or decrease.
14. Contributions can be any finite f32 value. Adapters emit in whatever scale is natural to their domain (e.g., 0–20 for test counts, 0–500 for gesture repetitions, 0–127 for MIDI velocities, -1.0–1.0 for sentiment). The engine's scale normalization (initially divide-by-range) maps these to comparable ranges regardless of whether the adapter's native scale is signed or unsigned.
15. Adapter IDs must be stable across sessions. If an adapter is reconfigured with a new ID, its previous contributions become orphaned. The adapter's old contributions should be explicitly removed.

### Adapter rules
16. The framework never inspects the opaque data payload. Adapters downcast internally.
17. External adapters are independent — they don't know about each other.
18. Reflexive adapters depend on accumulated graph state, not on specific adapter outputs.
19. An adapter owns its full internal pipeline. The framework has no opinion on internal phase ordering.

### Routing rules
20. Input routing is fan-out: all adapters matching the input kind receive the input.
21. Each matched adapter is spawned independently with its own sink and cancellation token.

### Fragment adapter rules
22. Concept IDs are deterministic: `concept:{lowercase_tag}`. Same tag always produces the same node ID, ensuring convergence via upsert.
23. Tags produce binary contributions (1.0) on `tagged_with` edges. The contribution means "this tag was applied," not a graduated strength.
24. Symmetric relationships (`may_be_related`) are emitted as two directed edges (A→B and B→A) with identical contributions, so query-time normalization sees the relationship from both endpoints.
25. The normalization floor ensures the weakest real contribution from any adapter maps to ~α (default 0.01), not 0.0. Formula: `(value - min + α·range) / ((1 + α)·range)`. Degenerate case (range = 0) returns 1.0.

### Runtime architecture rules
26. Marks always live in a project context, in the provenance dimension. There is no global `__provenance__` context.
27. `add_mark` requires a context parameter. No default, no fallback.
28. Tag format normalization is an invariant: strip `#` prefix, prepend `concept:` to match concept node IDs. `#travel` → `concept:travel`.
29. Tag-to-concept bridging is automatic at mark creation time. If a mark has tags and matching concept nodes exist in the same context, `references` edges are created. Bridging is creation-time only — marks created before matching concepts exist remain unbridged. A future reflexive adapter can close this gap.
30. `list_tags()` queries across all contexts, not a single context.
31. Tags are the shared vocabulary between provenance and semantics. A tag string on a mark and the ID of a concept node must use the same normalized form.
32. Persist-per-emission: each `emit()` call results in exactly one `save_context()` call. Emissions are the persistence boundary.
33. Contributions must survive persistence. After save → load, `edge.contributions` must be identical. Scale normalization depends on this.
34. Sources can appear in multiple contexts. A file that is a source in both `trellis` and `desk` contexts produces independent graph nodes in each. Overlap is handled by Plexus, not by the contributing tools.
35. A source update fans out to all contexts that include it. Each context gets its own adapter run through its own context-scoped EngineSink. The fan-out is a routing concern — adapters are context-unaware.

---

## Resolved Disambiguations

Inconsistencies found across source material, resolved here. These resolutions are binding.

### 1. "Weight" → always qualified
Never use "weight" alone. Three-layer vocabulary (ADR-003): **"contribution"** (stored per-adapter on edge), **"raw weight"** (computed from contributions via scale normalization), or **"normalized weight"** (computed at query time by consumer). Use **"scale normalization"** for the engine-side operation and **"query-time normalization"** for the consumer-side operation — never unqualified "normalization."

### 2. "Emission" is the domain term; `AdapterOutput` is the struct
The ADR and design docs consistently say "emission." The Rust struct is `AdapterOutput`. **Domain language uses "emission."** The struct should be renamed to `Emission` in implementation to close the gap.

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

---

## Open Questions

Unresolved issues that block or constrain behavior scenarios and implementation.

### Resolved by Essay 08 (Runtime Architecture)

**5. Provenance-semantic isolation.** Resolved: marks live in project contexts (provenance dimension), not a global `__provenance__` context. Cross-dimensional `references` edges connect marks to concept nodes via automatic tag-to-concept bridging. See invariants 26–31.

**6. Contribution persistence.** Resolved: `contributions_json` column in edges table. Schema migration follows the dimension migration pattern. See invariant 33.

**7. Adapter-to-engine wiring.** Resolved: EngineSink gains a constructor taking `Arc<PlexusEngine>` + `ContextId`. Routes emissions through the engine with persist-per-emission. See invariant 32.

### Resolved by ADR-003

**1. Reinforcement and multi-source convergence (edge weights).**
Resolved by ADR-003. Reinforcement is implicit: emitting an edge that already exists replaces that adapter's contribution slot with the new value. Per-adapter contributions are stored; raw weight is computed via scale normalization then summation. See ADR-003 for full details.

**Sub-question still open: node property merge on multi-source upsert.** When a second adapter emits a node that already exists, upsert updates properties. But what merge semantics apply? Last-writer-wins loses information (e.g., DocumentAdapter sets `concept_type: "theme"`, MovementAdapter overwrites with `concept_type: "effort_quality"`). Options: union of properties (conflicts become lists), thin nodes with detail in provenance entries only, or domain-specific merge rules. This does not block edge reinforcement implementation but blocks multi-adapter node convergence scenarios.

### Needs design decision (doesn't block initial implementation)

**2. Reflexive adapter cycle convergence.**
The commit → event → schedule trigger → reflexive adapter → commit loop has no convergence guarantee. A reflexive adapter whose emissions meet its own trigger condition could run indefinitely. Options: damping (reflexive emissions don't count toward mutation thresholds), depth limit (max N reflexive cycles per trigger), or quiescence detection. **Note:** the source-to-context fan-out (invariant 35) multiplies this — a single source update can trigger reflexive cascades in N contexts independently. Convergence is per-context (each has its own graph events and schedule triggers), so the problem doesn't get harder, but the total work from one source update scales with the number of containing contexts.

**3. ProposalSink and non-relational metadata edges.**
ProposalSink rejects all edges except `may_be_related`. But the TopologyAdapter needs to describe topology (community membership, hub status). If it can emit metadata *nodes* but can only connect them via `may_be_related`, can it actually express "node X is a member of community Y"? Options: allow a small set of metadata relationship types in ProposalSink, or model community membership as node properties rather than edges.

**4. Routing semantics: two-dimensional fan-out.**
Routing is now two-dimensional. A source update fans out across contexts (all contexts containing that source) and across adapters (all adapters matching the input kind). One source update hitting 3 contexts with 2 matching adapters each produces 6 independent adapter runs.

Open sub-questions:

- **Concurrency model.** All 6 runs concurrent? Contexts are independent (DashMap per-shard locking, no cross-context edges), and adapters within a context are independent (invariant 17). Full concurrency seems safe but has performance implications for large fan-outs.
- **Ordering.** Fan-out should be context-first (for each context, route to matching adapters), not adapter-first. Each context is the natural unit of independent work. But is there an ordering guarantee within a context across adapters? Probably not needed (invariant 17), but should be stated.
- **Partial failure.** If processing succeeds in 2 of 3 contexts, each context is independently consistent. No cross-context rollback needed. But should failures be surfaced? This is operational, not architectural.
- **Cascade scaling.** A single source update can trigger external adapters in N contexts, each producing mutations that may trigger reflexive adapters. Total work scales as N × (external adapters + reflexive cascades). See open question 2 for convergence within each context.
