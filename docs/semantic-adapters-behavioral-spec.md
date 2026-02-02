# Semantic Adapters: Behavioral Specification

Plain-English behavioral spec covering every decision path in the adapter layer.
Organized by trigger event. Each rule follows the form: **When** X happens, **Then** Y.

Companion to [semantic-adapters.md](./semantic-adapters.md) and [semantic-adapters-diagrams.md](./semantic-adapters-diagrams.md).

---

## 1. Input Routing

### 1.1 Input arrives at the adapter layer

**When** any input arrives at the adapter layer,
**Then** the router inspects its `input_kind` and dispatches it to every registered adapter whose `input_kind()` matches.

**When** multiple adapters match the same `input_kind`,
**Then** all matching adapters receive the input, scheduled according to their declared tier (see §2).

**When** no adapter matches the `input_kind`,
**Then** the input is rejected with an error. No graph mutation occurs.

### 1.2 Input kind determines pipeline

**When** input kind is `file_content`,
**Then** route to: DirectoryAdapter (Tier 0), MarkdownStructureAdapter (Tier 1), LinkAdapter (Tier 2), LLMSemanticAdapter (Tier 3).

**When** input kind is `text_fragment`,
**Then** route to: FragmentStructureAdapter (Tier 1), LLMSemanticAdapter (Tier 3).

**When** input kind is `gesture_encoding`,
**Then** route to: GestureNodeAdapter (Tier 0), LabelMappingAdapter (Tier 1), ClusterAdapter (Tier 2).

**When** input kind is `graph_state`,
**Then** route to reflexive adapters: NormalizationAdapter, TopologyAdapter, CoherenceAdapter (all Tier 4/Background).

### 1.3 Context scoping

**When** input arrives with a `context_id`,
**Then** all graph mutations produced by adapters processing that input are scoped to that context.

**When** an adapter produces nodes or edges,
**Then** those nodes and edges belong to the context specified in `AdapterInput.context_id`.

---

## 2. Tier Scheduling

### 2.1 Tier execution order

**When** adapters at multiple tiers are triggered by the same input,
**Then** tiers execute in strict order: Instant (0) → Fast (1) → Moderate (2) → Slow (3) → Background (4).

**When** a lower tier completes,
**Then** its output (mutations + metadata) is committed to the engine before the next tier begins.

**When** a lower tier completes,
**Then** an event is emitted for each committed mutation, so consumers can react to partial results immediately.

### 2.2 Tier 0 (Instant)

**When** a Tier 0 adapter runs,
**Then** it executes synchronously and blocks until complete. No background queuing.

**When** Tier 0 completes for a file input,
**Then** the graph contains at minimum: a file node with metadata (path, size, content type).

**When** Tier 0 completes for a gesture input,
**Then** the graph contains at minimum: a gesture node with timestamp and session ID.

### 2.3 Tier 1 (Fast)

**When** multiple Tier 1 adapters are triggered,
**Then** they may run in parallel (they do not depend on each other).

**When** Tier 1 completes for a file input,
**Then** the graph contains structural nodes (sections, chunks) with `contains` edges from the file node.

**When** Tier 1 completes for a file input,
**Then** chunk boundaries are available to downstream tiers (Tier 2 and Tier 3) as context.

> **This is load-bearing.** Bad chunking from Tier 1 ruins all downstream extraction. The structural adapter's section boundaries become the units of incremental reprocessing.

**When** Tier 1 completes for a gesture input,
**Then** concept nodes are created (or found) for each label in `GestureEncoding.labels`, and edges connect the gesture node to those concept nodes.

### 2.4 Tier 2 (Moderate)

**When** a Tier 2 adapter runs,
**Then** it runs in the background and emits events on completion.

**When** Tier 2 runs for a file input,
**Then** it receives chunk boundaries from Tier 1 and analyzes cross-references, citations, and links within and between chunks.

**When** Tier 2 runs for a gesture input (ClusterAdapter),
**Then** it compares the gesture's feature vector against existing gesture nodes and produces cluster-membership edges.

**When** a gesture clusters with existing gestures,
**Then** edges of type `same_cluster` connect them, and shared-cluster edges between those gestures are reinforced.

### 2.5 Tier 3 (Slow)

**When** a Tier 3 adapter runs,
**Then** it always runs in the background. It involves LLM calls or comparably expensive computation.

**When** Tier 3 processes file content,
**Then** it sends only the chunks identified as changed by Tier 1 to the LLM — not the full file. Cost is proportional to the delta, not the corpus size.

**When** Tier 3 receives a text fragment (Trellis),
**Then** it processes the fragment in isolation (no incremental delta — each fragment is self-contained).

**When** Tier 3 completes,
**Then** concept nodes appear in the semantic dimension, with `found_in` cross-dimensional edges linking concepts to their source material in the structure dimension.

### 2.6 Tier 4 (Background)

**When** Tier 4 adapters are triggered,
**Then** they do not run immediately. They are scheduled based on trigger conditions (see §6).

**When** Tier 4 produces mutations,
**Then** those mutations are committed through the same merger and provenance pipeline as any other tier.

### 2.7 Parallelism rules

**When** two adapters are at the same tier,
**Then** they may run in parallel.

**When** an adapter at Tier N depends on output from Tier N-1,
**Then** Tier N-1 must complete and commit before Tier N begins. The tier boundary is a synchronization point.

---

## 3. Output Merging and Commitment

### 3.1 Node deduplication

**When** two adapters at the same tier produce a node with the same ID (e.g., both create `concept:sudden`),
**Then** the merger keeps one node and combines their provenance entries. The node is not duplicated.

**When** an adapter produces a node whose ID already exists in the graph (from a previous tier or previous input),
**Then** the existing node is found (not recreated). New edges from the adapter connect to the existing node.

### 3.2 Edge merging

**When** two adapters produce an edge between the same source and target with the same relationship type,
**Then** the merger combines them into a single edge with reinforcement from both sources.

**When** an adapter produces an edge that already exists in the graph,
**Then** the existing edge is reinforced (see §5) rather than duplicated.

### 3.3 Node removal (incremental updates)

**When** an adapter's output includes node IDs in the `removals` list,
**Then** those nodes and all their incident edges are removed from the graph.

**When** a node is removed,
**Then** its provenance marks remain in the provenance dimension (epistemological record persists even when the ontological claim is retracted).

> **OPEN QUESTION:** Should removed provenance marks be annotated as "retracted"? Or should provenance for removed nodes be garbage-collected? The current design says provenance persists, but this needs validation against real usage.

### 3.4 Provenance generation

**When** an adapter's output includes `ProvenanceEntry` items,
**Then** the adapter layer automatically converts each entry into a provenance mark in the provenance dimension.

**When** a provenance mark is created,
**Then** it receives cross-dimensional `derived` edges pointing to the nodes/edges listed in `ProvenanceEntry.explains`.

**When** an adapter processes input,
**Then** a provenance chain is created (or appended to) for that processing run. All marks from that run belong to the chain.

**When** a provenance mark has `entry_type = Question`,
**Then** the mark exists in the graph as a flag of uncertainty, queryable for human review.

**When** a provenance mark has `entry_type = NeedsReview`,
**Then** the mark exists in the graph as a flag requiring human attention before the assertion should be trusted.

---

## 4. Cross-Modal Concept Bridging

### 4.1 Concept node identity

**When** an adapter produces a concept node,
**Then** the node's identity is determined by its label in the semantic dimension (e.g., `concept:sudden`).

**When** two adapters from different domains produce a concept node with the same label,
**Then** they reference the same node. This is the bridging mechanism — shared vocabulary creates shared nodes.

### 4.2 Cross-modal reinforcement

**When** a second adapter references a concept node that already exists (created by a different adapter),
**Then** the new edge to that concept constitutes a `MultipleAnalyzers` reinforcement. Confidence on the concept's edges increases.

**When** a concept is reinforced by adapters from independent modalities (e.g., text and movement),
**Then** the confidence boost is significant — independent modalities agreeing is stronger evidence than the same modality agreeing twice.

> **AMBIGUITY:** The current design does not specify *how much* more significant cross-modal reinforcement is versus same-modal. Should there be a multiplier? Or is the existing `calculate_confidence()` (which counts unique ReinforcementType discriminants) sufficient? Currently, confidence = `min(unique_types * 0.25, 1.0)`. Two different adapter sources using the same ReinforcementType (MultipleAnalyzers) would not increase the type count. This may undercount cross-modal evidence.

### 4.3 Label quality

**When** a gesture arrives with labels like `["sudden", "strong", "indirect"]` (domain vocabulary),
**Then** concept nodes are created for each label, and the gesture connects to the shared semantic namespace.

**When** a gesture arrives with labels like `["cluster-7"]` (opaque identifier),
**Then** a concept node is created, but it connects to nothing outside the movement domain. Cross-modal bridging fails.

> **DESIGN IMPLICATION:** The richness of cross-modal connections is determined entirely by the quality of labels upstream. This is a responsibility of the input source (EDDI, Trellis, Manza), not Plexus.

---

## 5. Edge Reinforcement and Decay

### 5.1 Edge creation

**When** an external adapter produces an edge,
**Then** the edge is created with `weight: 1.0`, `strength: 1.0`, `confidence: 0.0`. No confidence until reinforced.

**When** a reflexive adapter proposes a `may_be_related` edge,
**Then** the edge is created with `weight: 0.3`, `confidence: 0.15`. Weak by design.

### 5.2 Reinforcement

**When** an edge receives a new `Reinforcement`,
**Then** the reinforcement is appended to the edge's `reinforcements` vector, `last_reinforced` is updated to now, strength is recalculated, and confidence is recalculated.

**When** strength is recalculated,
**Then** `strength = (weight + reinforcement_count * 0.1) * recency_factor`.

**When** confidence is recalculated,
**Then** `confidence = min(unique_reinforcement_types * 0.25, 1.0)`. Confidence reflects evidence diversity, not volume.

> **CONSEQUENCE:** An edge reinforced 100 times by the same type of evidence (e.g., all `CoOccurrence`) has confidence 0.25. An edge reinforced 4 times by 4 different types has confidence 1.0. Diversity of evidence matters more than quantity.

### 5.3 Decay

**When** time passes without reinforcement,
**Then** the edge's effective strength decays exponentially: `recency_factor = 0.5^(hours_since_last_reinforced / half_life)`.

**When** using the default half-life,
**Then** `half_life = 168 hours (1 week)`. After 1 week without reinforcement, an edge is at 50% strength. After 2 weeks, 25%. After 1 month, ~6%.

**When** a context has a custom decay configuration,
**Then** the context's half-life overrides the default. Manza contexts might use 168 hours (weekly). Trellis contexts might use several thousand hours (months). EDDI session contexts might use no decay within a session.

> **OPEN QUESTION:** The current implementation has a hardcoded `DECAY_HALF_LIFE_HOURS: f32 = 168.0` in `edge.rs`. The architecture doc says decay is per-context. This discrepancy needs resolution — the implementation needs a mechanism for context-level decay configuration to override the default.

### 5.4 Edge lifecycle terminal states

**When** an edge's effective strength approaches zero (recency factor negligible, no new reinforcement),
**Then** the edge becomes "negligible" — effectively invisible to queries but not deleted.

**When** an edge decays to negligible,
**Then** it remains in the graph. It can be resurrected if new evidence arrives.

> **OPEN QUESTION:** Should negligible edges ever be garbage-collected? The current design says no — they persist. But over long timescales with many proposed `may_be_related` edges, the graph could accumulate dead weight. A garbage-collection threshold or periodic cleanup of edges below some strength floor may be needed.

---

## 6. Reflexive Adapters

### 6.1 Trigger conditions

**When** the number of new graph mutations since the last reflexive run exceeds a threshold,
**Then** reflexive adapters (Tier 4) are scheduled to run.

> **OPEN QUESTION:** What is the mutation threshold? "50 new edges since last run" is mentioned as an example, but no specific number is specified. This likely needs to be configurable per-context (EDDI sessions with rapid mutation need different thresholds than Trellis).

**When** a reflexive adapter is triggered,
**Then** it receives a `GraphState` input with a `ReflexiveScope` indicating what to examine.

**When** the scope is `Full`,
**Then** the reflexive adapter examines the entire context graph.

**When** the scope is `Since(timestamp)`,
**Then** the reflexive adapter examines only nodes and edges added since that timestamp.

**When** the scope is `Nodes(list)`,
**Then** the reflexive adapter examines only the specified nodes (e.g., "all concept nodes touched by multiple adapters").

### 6.2 NormalizationAdapter behavior

**When** the NormalizationAdapter runs,
**Then** it scans concept nodes in the semantic dimension and identifies candidate pairs with similar labels (fuzzy matching, potentially LLM-assisted).

**When** a candidate pair is found whose similarity exceeds `similarity_threshold`,
**Then** a `may_be_related` edge is created between them with `weight: 0.3`, `confidence: 0.15`.

**When** a `may_be_related` edge is created,
**Then** a provenance mark records: which adapter proposed it, the similarity score, the method used (fuzzy match, LLM, embedding distance), and the initial confidence.

**When** a `may_be_related` edge already exists between two concepts,
**Then** no duplicate is created. The existing edge may receive additional reinforcement if the normalization adapter found new evidence.

**The NormalizationAdapter NEVER merges nodes.** It only proposes edges. The graph's reinforcement dynamics determine whether the relationship is real.

### 6.3 TopologyAdapter behavior

**When** the TopologyAdapter runs,
**Then** it performs community detection (e.g., Louvain algorithm) on the current graph state.

**When** a community is detected,
**Then** the adapter may emit:
- A community node (metadata about the cluster)
- Edges connecting member nodes to the community node
- A `topology_changed` event

**When** a hub node is identified (high degree centrality),
**Then** the adapter may annotate the node with hub metadata.

**When** a new community forms, an existing community splits, or two communities merge,
**Then** a `topology_changed` event is emitted. This is the primary output for EDDI's environment control.

### 6.4 CoherenceAdapter behavior

**When** a concept node has edges from multiple adapter sources,
**Then** the CoherenceAdapter examines whether the contributions are semantically consistent.

**When** contributions from different adapters to the same concept are consistent,
**Then** no action is taken (the graph is coherent at that node).

**When** contributions from different adapters to the same concept appear inconsistent,
**Then** the CoherenceAdapter creates a provenance mark of type `Question`, flagging the divergence for human review or for graph dynamics to settle.

**The CoherenceAdapter NEVER resolves conflicts.** It only surfaces them.

---

## 7. Incremental Processing

### 7.1 File change (Manza pattern)

**When** a file changes,
**Then** the adapter layer receives a `FileChanged` trigger with the file path.

**When** Tier 1 runs on a changed file,
**Then** it compares the current structure against the previous snapshot (`AdapterInput.previous`) and identifies which chunks changed.

**When** chunks that haven't changed are identified,
**Then** higher tiers skip those chunks entirely. Only changed chunks are sent to Tier 2 and Tier 3.

**When** a chunk is removed (section deleted),
**Then** the structure adapter includes those chunk node IDs in `AdapterOutput.removals`.

**When** a chunk is added (new section),
**Then** new structure nodes and edges are created. Higher tiers process the new chunk as fresh content.

**When** a chunk is modified,
**Then** higher tiers re-process that chunk. The semantic adapter may produce updated concept nodes, removed concept nodes (if a concept was deleted from the text), or reinforced edges (if the concept was preserved).

### 7.2 Fragment arrival (Trellis pattern)

**When** a text fragment arrives,
**Then** it is processed in isolation — no delta computation. Each fragment is self-contained.

**When** a fragment produces concept nodes that already exist in the graph,
**Then** edges from the fragment to those concepts reinforce the existing connections. The graph accumulates.

**When** enough fragments have accumulated to form a detectable community,
**Then** the TopologyAdapter (Tier 4) detects the community and Trellis can harvest the implicit structure.

### 7.3 Gesture arrival (EDDI pattern)

**When** a gesture encoding arrives,
**Then** it is processed as a discrete event — no delta, no incremental state. Each gesture stands alone.

**When** a gesture's labels match existing concept nodes,
**Then** edges from the gesture to those concepts reinforce the existing connections.

**When** a gesture's feature vector clusters with existing gestures,
**Then** cluster-membership edges are created or reinforced.

**When** the graph has accumulated enough gestures to form topology changes,
**Then** the TopologyAdapter detects the change and emits events for environment control.

---

## 8. Event Emission

### 8.1 Mutation events

**When** a node is added to the graph,
**Then** an event is emitted: `node_added { node_id, node_type, content_type, dimension }`.

**When** an edge is added to the graph,
**Then** an event is emitted: `edge_added { edge_id, source, target, relationship, is_cross_dimensional }`.

**When** an edge is reinforced,
**Then** an event is emitted: `edge_reinforced { edge_id, new_strength, new_confidence, reinforcement_type }`.

**When** a node is removed,
**Then** an event is emitted: `node_removed { node_id }`.

### 8.2 Tier completion events

**When** a tier completes processing for a given input,
**Then** an event is emitted: `tier_completed { tier, adapter_id, input_kind, mutations_count }`.

### 8.3 Topology events

**When** the TopologyAdapter detects a new community,
**Then** an event is emitted: `community_formed { community_id, member_count, member_ids }`.

**When** the TopologyAdapter detects a hub node,
**Then** an event is emitted: `hub_emerged { node_id, degree, dimension }`.

**When** the TopologyAdapter detects community structural change,
**Then** an event is emitted: `topology_changed { change_type, affected_communities }`.

**When** an edge crosses a strength threshold (configurable),
**Then** an event is emitted: `edge_threshold_crossed { edge_id, threshold, new_strength }`.

---

## 9. Application-Specific Behavior

### 9.1 Manza (ambient/continuous)

**When** Manza receives a `tier_completed` event,
**Then** the graph visualization animates the new mutations appearing.

**When** Manza receives events from Tier 0 and Tier 1,
**Then** the UI can show structural information immediately, before semantic analysis completes.

**When** Manza receives events from Tier 3 (delayed),
**Then** new concept connections animate in, adding to the already-visible structure.

**When** the user edits a file again before Tier 3 completes from the previous edit,
**Then** the in-progress Tier 3 work for unchanged chunks remains valid. Only the newly changed chunks need re-queuing.

> **OPEN QUESTION:** What happens to in-flight Tier 3 LLM calls when the same chunk changes again? Cancel and re-queue? Let the old result land and then re-queue the delta? This needs a cancellation/invalidation strategy.

### 9.2 Trellis (accumulative/periodic)

**When** Trellis queries the graph for emergent patterns,
**Then** it uses: community detection (topic clusters), high-degree nodes (recurring themes), path analysis (fragment chains forming implicit outlines), and edge strength (which connections are reinforced across many fragments).

**When** Trellis "harvests" an implicit outline,
**Then** it reads the community structure, ranks fragments within the community by connection strength, and presents them as a draft structure.

**When** the writer doesn't respond to a surfaced connection,
**Then** nothing happens. The graph doesn't penalize silence. (Trellis is "mirror not oracle.")

### 9.3 EDDI (streaming/session-based)

**When** EDDI begins a new session,
**Then** a session context is created (or a persistent context is continued).

**When** EDDI receives a `topology_changed` event,
**Then** the subscribing client translates the change into environmental response (light, sound, projection).

**When** a session ends,
**Then** the session's graph state persists. Cross-session patterns (movement style evolution, recurring motifs) accumulate over time.

**When** within-session patterns emerge (escalating movement vocabulary, repeated gesture sequences),
**Then** these are distinct from cross-session patterns and may trigger different events.

---

## 10. Provenance Tracing

### 10.1 Forward tracing (what did this adapter produce?)

**When** a user queries "what did AdapterX produce?",
**Then** follow the provenance chain for that adapter's processing run → find all marks → follow `derived` edges to the ontological nodes/edges they explain.

### 10.2 Backward tracing (where did this concept come from?)

**When** a user queries "where did concept:X come from?",
**Then** find all provenance marks with `derived` edges pointing to concept:X → read the marks' descriptions, confidence levels, and source locations → follow the chain to see the full processing context.

### 10.3 Uncertainty surfacing

**When** a user queries for uncertain assertions,
**Then** find all provenance marks of type `Question` → return the concepts they're attached to, the confidence levels, and the adapter's reasoning.

### 10.4 Cross-modal provenance

**When** a concept has provenance marks from multiple adapters,
**Then** the provenance trail shows independent convergence — e.g., "DocAdapter extracted this from text AND MovementAdapter derived it from gesture labels."

---

## 11. The SemanticAdapter ↔ ContentAnalyzer Relationship

### 11.1 Migration path

**When** `SemanticAdapter` is implemented,
**Then** the existing `ContentAnalyzer` trait is not deleted. It becomes one specialization: a content analyzer is a semantic adapter with `input_kind = "file_content"`.

**When** an existing `ContentAnalyzer` implementation needs to work with the new adapter layer,
**Then** it can be wrapped in a shim that implements `SemanticAdapter`, translating `AnalysisContext` to `AdapterInput` and `AnalysisResult` to `AdapterOutput`.

### 11.2 What ContentAnalyzer has that SemanticAdapter generalizes

**When** `ContentAnalyzer` provides `handles() -> Vec<ContentType>`,
**Then** the equivalent in `SemanticAdapter` is `input_kind()` — broader, since it covers non-file inputs.

**When** `ContentAnalyzer` provides `requires_llm() -> bool`,
**Then** the equivalent in `SemanticAdapter` is `tier()` — Tier 3 (Slow) implies LLM usage, but the tier system is more granular than a boolean.

**When** `ContentAnalyzer` provides `capabilities()`,
**Then** the equivalent is `dimensions()` — which graph dimensions the adapter populates.

---

## 12. Invariants (things that must always hold)

1. **Every graph mutation has provenance.** No node or edge enters the graph without an associated provenance mark explaining where it came from.

2. **Adapters never know about each other.** Cross-adapter reinforcement happens through shared concept nodes in the semantic dimension, not through direct adapter-to-adapter communication.

3. **Reflexive adapters never destroy information.** They only propose edges. They never merge, delete, or modify existing nodes.

4. **Tiers execute in order.** Tier N never runs before Tier N-1 completes for the same input.

5. **Decay is per-context, not per-adapter.** The same adapter may produce edges with different decay characteristics depending on which context it's operating in.

6. **Labels are the bridge.** Cross-modal concept bridging happens through shared vocabulary in the semantic dimension. No other mechanism is needed or wanted.

7. **The graph is always partially built.** At any moment, some tiers have completed and others haven't. This is correct behavior, not an error state. Consumers receive events as each tier finishes.

8. **Provenance is epistemological, the rest is ontological.** Nodes/edges in structure, semantic, relational, and temporal dimensions represent things that exist. Nodes/edges in the provenance dimension represent how we came to assert what exists.

9. **Concept node identity is label-based.** `concept:sudden` from DocAdapter and `concept:sudden` from MovementAdapter are the same node.

10. **Evidence diversity drives confidence, not evidence volume.** 4 different types of reinforcement → confidence 1.0. 100 of the same type → confidence 0.25.

---

## 13. Identified Ambiguities and Gaps

Items flagged during this audit that need design decisions:

### 13.1 Cross-modal reinforcement weighting

The confidence formula (`unique_types * 0.25`) does not distinguish between same-modal and cross-modal reinforcement. Two independent modalities agreeing should arguably be stronger evidence than two instances of the same modality. **Decision needed:** add a cross-modal multiplier, or accept current formula as sufficient?

### 13.2 In-flight work cancellation

When a file changes while Tier 3 is still processing the previous version of the same chunk, the behavior is unspecified. **Decision needed:** cancel and re-queue? Let old result land then re-process? Debounce at the adapter layer?

### 13.3 Hardcoded decay vs per-context decay

The implementation has `DECAY_HALF_LIFE_HOURS = 168.0` hardcoded. The architecture says decay is per-context. **Decision needed:** move half-life to context configuration, with 168h as default.

### 13.4 Negligible edge garbage collection

Edges that decay to near-zero persist indefinitely. Over long timescales with many reflexive proposals, this could accumulate dead weight. **Decision needed:** introduce a cleanup threshold, or accept accumulation as intentional?

### 13.5 Provenance for removed nodes

When a node is removed (via `AdapterOutput.removals`), its provenance marks persist. **Decision needed:** should provenance marks for removed nodes be annotated as "retracted"? Should they be garbage-collected after some period?

### 13.6 Reflexive adapter trigger thresholds

The mutation count that triggers Tier 4 is unspecified. Different contexts likely need different thresholds. **Decision needed:** make this configurable per-context, and define sensible defaults.

### 13.7 AdapterSnapshot design

The `AdapterInput.previous: Option<AdapterSnapshot>` field is mentioned but `AdapterSnapshot` is not defined. **Decision needed:** what does a snapshot contain? For files: processed chunk hashes and previous output node IDs. For gestures: current cluster centroids. For graph state: timestamp of last reflexive run. This likely needs to be adapter-specific.

### 13.8 Community node representation

The TopologyAdapter "may emit a community node" but the representation is unspecified. **Decision needed:** are communities first-class nodes in the graph (what dimension? what content type?) or metadata annotations on existing nodes?

### 13.9 Canonical pointers vs pure emergence

When a `may_be_related` edge between two concepts strengthens to high confidence (e.g., 0.9), should the system eventually designate one as canonical? Or do they remain as two nodes with a strong equivalence edge? **Decision needed:** canonical pointers simplify queries but introduce arbitrary primacy. Strong equivalence edges are more honest but create permanent duplication.

### 13.10 Session boundary semantics for EDDI

Within-session vs cross-session patterns are described as "distinct" but the mechanism for distinguishing them is unspecified. **Decision needed:** separate session contexts? Same context with temporal windowing? Session metadata on nodes/edges?
