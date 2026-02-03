# Domain Model: Plexus Semantic Adapter Layer

Ubiquitous language for the adapter subsystem. All ADRs, behavior scenarios, and code must use these terms consistently. If this glossary says "emission," the code says `emission`, not "output batch" or "result set."

Extracted from: ADR-001, semantic-adapters.md, semantic-adapters-design.md, PAPER.md, SPIKE-OUTCOME.md.

---

## Concepts

| Term | Definition | Aliases to Avoid |
|------|-----------|------------------|
| **Adapter** | A self-contained unit that transforms domain-specific input into graph mutations. Owns its entire processing pipeline internally. The framework sees only what exits the sink. | plugin, processor, handler |
| **External adapter** | An adapter triggered by outside input arriving (a file change, text fragment, gesture encoding). Runs when matching input is routed to it. | input adapter (acceptable in diagrams, not in code) |
| **Reflexive adapter** | An adapter triggered by a schedule condition that examines accumulated graph state and proposes refinements. Receives a ProposalSink, not a full AdapterSink. | internal adapter, meta-adapter |
| **Sink** | The interface through which an adapter pushes graph mutations into the engine. `AdapterSink` is the full interface; `ProposalSink` is the constrained variant for reflexive adapters. | output, writer, channel |
| **ProposalSink** | A constrained wrapper around AdapterSink given to reflexive adapters. Clamps edge weights to a cap, only allows `may_be_related` edges, and rejects node removals. The adapter's `process()` signature is unchanged — it still receives `&dyn AdapterSink`. | — |
| **Emission** | The data payload of a single `sink.emit()` call: a bundle of annotated nodes, annotated edges, and removals. Each emission is validated and committed atomically. **Not** the act of emitting — use "emit" as the verb. | batch, result, output (when meaning the data) |
| **Node** | A vertex in the knowledge graph. Has an ID, type, content type, dimension, and properties. | vertex, entity |
| **Edge** | A directed connection between two nodes. Carries a raw weight (accumulated reinforcement) and a relationship type. | link, connection, arc |
| **Raw weight** | The accumulated Hebbian reinforcement strength stored on an edge. Increases when any source reinforces the edge. Ground truth — never decays on a clock. | weight (ambiguous without qualifier), strength |
| **Normalized weight** | Relative importance of an edge, computed at query time via a NormalizationStrategy. Not stored. Different consumers can apply different strategies to the same raw weights. | — |
| **Annotation** | Adapter-provided metadata about a single extraction: confidence, method, source location, detail. Lives on an AnnotatedNode or AnnotatedEdge. Describes *how* the adapter came to know something. | metadata (too generic), tag |
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
| **Context ID** | Identifies the processing context (e.g., a Manza editing session, a Trellis accumulation window, an EDDI performance). Groups related provenance entries. | session ID (close but not identical) |
| **Cancellation token** | A cooperative signal that an adapter's in-flight work has been superseded. Checked between emissions, not during. Already-committed emissions remain valid. | — |
| **Graph event** | A low-level notification fired per mutation type when an emission is committed. Five kinds: NodesAdded, EdgesAdded, NodesRemoved, EdgesRemoved, WeightsChanged. Higher-level events are modeled as nodes/edges from reflexive adapters, not as additional event types. | — |
| **Input kind** | A string declared by each adapter identifying what type of input it consumes. The router uses this for matching. Examples: `file_content`, `text_fragment`, `gesture_encoding`, `graph_state`. | — |
| **Adapter snapshot** | Optional incremental state from a previous run, passed back to the adapter on re-processing. Contents are adapter-specific (e.g., chunk hashes for documents, cluster centroids for movement). Design deferred. | checkpoint, state |
| **Input router** | Framework component that directs incoming input to all adapters whose `input_kind()` matches. Fan-out: when multiple adapters match, all receive the input. | dispatcher |
| **Schedule monitor** | Framework component that evaluates schedule conditions against graph state and fires reflexive adapters when conditions are met. Observes graph events (for MutationThreshold) or queries graph state directly (for Condition). | scheduler, cron |

## Actions

| Action | Actor | Target | Description |
|--------|-------|--------|-------------|
| **emit** | Adapter | Sink | Push an emission (nodes + edges + removals) through the sink. Async — the adapter awaits and receives validation feedback. |
| **intercept** | ProposalSink | Emission | Check emission against proposal constraints (weight cap, relationship type, no removals) *before* forwarding to the engine. Rejects or clamps locally. |
| **commit** | Engine | Graph | Validate each item in the emission and persist valid mutations. Fires graph events for committed items. Invalid items (edges with missing endpoints) are rejected individually; valid items in the same emission still commit. |
| **reject** | Engine | Edge | Refuse an individual edge whose endpoints don't exist in the graph or in the same emission. The rejection is reported in the result returned to the adapter. Other items in the same emission are unaffected. |
| **upsert** | Engine | Node | When a node with a duplicate ID is emitted, update its properties rather than creating a second node. This is not an error — it's the expected path for re-processing. |
| **reinforce** | Engine | Edge | Strengthen an existing edge's raw weight when new evidence arrives. **Mechanics undefined** — this is the blocking open question in ADR-001. The trigger is also undefined: does emitting an edge that already exists implicitly reinforce it, or is reinforcement a separate operation? See Ambiguity §1. |
| **normalize** | NormalizationStrategy | Edge | Compute the relative weight of an edge at query time from its raw weight and graph context. |
| **propose** | Reflexive adapter | Graph (via ProposalSink) | Emit a weak `may_be_related` edge suggesting a connection. "Propose" is the domain verb; the adapter still calls `sink.emit()`. |
| **clamp** | ProposalSink | Edge raw weight | Cap a proposed edge's raw weight to the configured maximum before forwarding to the inner sink. |
| **route** | Input router | Adapter(s) | Direct incoming input to all adapters whose `input_kind()` matches. Fan-out: multiple adapters can declare the same input kind. |
| **cancel** | Framework | Adapter (via token) | Signal that in-flight work has been superseded. Adapter checks cooperatively between emissions. |
| **annotate** | Adapter | Node or Edge | Attach extraction metadata (confidence, method, source location) to a node or edge in the emission. |
| **construct provenance** | Engine | Provenance entry | Combine the adapter's annotation with framework context (adapter ID, timestamp, context ID, input summary) to create a full provenance record. |

## Relationships

### Structural
- **Adapter** emits through **sink**
- **ProposalSink** wraps **AdapterSink** (implements the same trait, adds constraints)
- **Emission** contains **AnnotatedNodes**, **AnnotatedEdges**, and removals
- **AnnotatedNode** pairs a **Node** with an optional **Annotation**
- **AnnotatedEdge** pairs an **Edge** with an optional **Annotation**
- **Engine** constructs **provenance entry** from **annotation** + framework context
- **Edge** carries a **raw weight** and a relationship type
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

## Invariants

### Emission rules (enforced by engine at commit time)
1. The engine validates and commits each item in the emission independently. Valid items commit; invalid items are rejected individually. The emission is **not** all-or-nothing.
2. Per-item behaviors: edges with missing endpoints are **rejected** (endpoint must exist in the graph or in the same emission). Duplicate node IDs **upsert**. Removal of non-existent nodes is a **no-op**. Self-referencing edges are **allowed**. Empty emissions are a **no-op**.
3. `emit()` returns a result describing what was rejected and why. The adapter can act on rejected items or ignore them. Partial success is the normal case, not an error.
4. Cancellation is checked **between** emissions, not during. Committed items are never rolled back.

### ProposalSink rules (enforced by ProposalSink before engine sees the emission)
4. Reflexive adapters can only emit edges with relationship `may_be_related`. Other relationship types are rejected by the ProposalSink.
5. Reflexive adapters cannot remove nodes.
6. ProposalSink clamps edge raw weights to a configurable per-adapter cap.
7. Reflexive adapters **can** emit nodes (e.g., topology metadata) and annotations.

### Provenance rules
8. Adapters **never** construct provenance entries directly — they annotate, the engine wraps.
9. Nodes emitted without annotation still receive provenance marks (structural context only).

### Weight rules
10. Raw weights are stored. Normalized weights are computed at query time. Never the reverse.
11. No temporal decay. Weakening happens only through normalization as the graph grows around an edge.
12. A quiet graph stays stable — silence is not evidence against previous observations.

### Adapter rules
13. The framework never inspects the opaque data payload. Adapters downcast internally.
14. External adapters are independent — they don't know about each other.
15. Reflexive adapters depend on accumulated graph state, not on specific adapter outputs.
16. An adapter owns its full internal pipeline. The framework has no opinion on internal phase ordering.

### Routing rules
17. Input routing is fan-out: all adapters matching the input kind receive the input.
18. Each matched adapter is spawned independently with its own sink and cancellation token.

---

## Resolved Disambiguations

Inconsistencies found across source material, resolved here. These resolutions are binding.

### 1. "Weight" → always qualified
Never use "weight" alone. Always **"raw weight"** (stored, accumulated) or **"normalized weight"** (computed, query-time). The design doc's terminology table already warns about this; this model enforces it.

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

---

## Open Questions

Unresolved issues that block or constrain behavior scenarios and implementation.

### Blocks implementation (needs spike)

**1. Reinforcement and multi-source convergence.**
The domain model defines "reinforce" as an action but cannot define its behavior. Three sub-questions:
- **Trigger:** When an adapter emits an edge that already exists, does the engine implicitly reinforce it? Or is reinforcement a separate explicit operation?
- **Operation:** Additive increment? Multiplicative boost? Source-diversity-weighted? Does the same adapter re-emitting the same edge reinforce (re-processing a re-saved file), or is that a no-op?
- **Property merge on upsert:** When a second adapter emits a node that already exists, upsert updates properties. But what merge semantics apply? Last-writer-wins loses information (e.g., DocumentAdapter sets `concept_type: "theme"`, MovementAdapter overwrites with `concept_type: "effort_quality"`). Options: union of properties (conflicts become lists), thin nodes with detail in provenance entries only, or domain-specific merge rules.

Until resolved, behavior scenarios for reinforcement and multi-source node convergence cannot be written. This is the blocking open question from ADR-001.

### Needs design decision (doesn't block initial implementation)

**2. Reflexive adapter cycle convergence.**
The commit → event → schedule trigger → reflexive adapter → commit loop has no convergence guarantee. A reflexive adapter whose emissions meet its own trigger condition could run indefinitely. Options: damping (reflexive emissions don't count toward mutation thresholds), depth limit (max N reflexive cycles per trigger), or quiescence detection.

**3. ProposalSink and non-relational metadata edges.**
ProposalSink rejects all edges except `may_be_related`. But the TopologyAdapter needs to describe topology (community membership, hub status). If it can emit metadata *nodes* but can only connect them via `may_be_related`, can it actually express "node X is a member of community Y"? Options: allow a small set of metadata relationship types in ProposalSink, or model community membership as node properties rather than edges.

**4. Routing semantics for duplicate input kinds.**
The model now says routing is fan-out. But: should all matching adapters process the same input concurrently? Sequentially? Is there an ordering guarantee? Can one adapter's output influence another's input (no — invariant 14 says they're independent — but confirming this for the router level).
