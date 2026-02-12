# Research Log: Two-Consumer Cross-Dimensional Validation

## Question 1: When two independent consumers contribute to the same context with shared tag vocabulary, does the enrichment loop produce cross-dimensional bridges that enable traversal from research annotations to related writing fragments?

**Method:** Spike (integration test + binary)

**Setup:** 6 Trellis-like fragments with Carrel's real theme vocabulary (distributed-ai, compute-economics, policy, design-constraints, non-generative-ai, federated-learning, network-science). 4 Carrel-like provenance marks (2 writing, 2 research) with overlapping tags. 2 research-to-writing links. Both consumers share the "carrel-workspace" context, registered as separate integrations with shared enrichments (TagConceptBridger + CoOccurrenceEnrichment).

**Findings:**

### The graph structure is correct

19 nodes (7 semantic, 6 structure, 6 provenance), 45 edges across 5 relationship types. Every expected node and edge was created. The three dimensions (semantic, structure, provenance) are cleanly separated, connected only by cross-dimensional edges.

### Cross-dimensional bridges form automatically

TagConceptBridger created 8 `references` edges (provenance → semantic) — 2 per mark, matching each mark's tags to existing concept nodes. All 8 edges have the correct `source_dimension: provenance` and `target_dimension: semantic`. The bridging is automatic and bidirectional: marks created after concepts are bridged immediately.

### The key traversal works

Starting from research-mark-1 ("Chen et al. 2025 — federated learning economics"):

1. **References** (depth 1): reaches `concept:federated-learning` and `concept:compute-economics`
2. **Tagged fragments** (depth 2): reaches 3 writing fragments tagged with `compute-economics` and 2 fragments tagged with `federated-learning`
3. **Other marks** (depth 2): reaches `writing-mark-1` ("Core argument — distributed compute as insurance") via shared `concept:compute-economics`
4. **Direct links** (depth 1): reaches `writing-mark-1` via explicit `links_to` edge

The BFS traversal from research-mark-1 at depth 2 reaches all three dimensions: provenance (the mark's chain, linked writing mark, and the other research mark), semantic (4 concepts), and structure (3 fragments). This is the connection that neither consumer could produce alone.

### Co-occurrence enrichment produces structured results

8 unique `may_be_related` concept pairs were detected, with graduated scores:

- **Score 1.0** (3 pairs): distributed-ai ↔ compute-economics, distributed-ai ↔ policy, compute-economics ↔ policy — these co-occur in 2 fragments each (the maximum for any pair in this dataset)
- **Score 0.5** (5 pairs): lower co-occurrence counts

The scores are normalized relative to the maximum co-occurrence count, so they represent relative rather than absolute co-occurrence strength. At this scale (6 fragments), the distinction between 1.0 and 0.5 is meaningful — it separates the strongly-connected "distributed AI economics" cluster from peripheral connections.

### Outbound events are empty

Neither FragmentAdapter nor ProvenanceAdapter override `transform_events()`. All outbound events were empty vectors. This means consumers currently get no feedback about what the pipeline produced. For real Trellis/Carrel integration, the adapters need outbound event implementations.

### Fragment IDs are UUIDs, not human-readable

Fragment nodes get UUID-based IDs (e.g., `5ed52a7c-e130-44a6-a77c-86e4eeb41217`). This is correct for deduplication but makes the graph hard to inspect visually. The fragment's text content is stored in a `content` property, but the traversal output doesn't surface it well. For consumer-facing tools, the adapter or outbound events should surface human-readable content.

### Persistence survives restart

All 19 nodes, all 45 edges (including contributions), and all cross-dimensional references survived a save-load cycle. The integration test verifies this explicitly.

**Implications:**

1. **The adapter contracts work.** FragmentAdapter and ProvenanceAdapter both fit cleanly into the ingest pipeline. No changes needed to the adapter trait or enrichment trait.

2. **TagConceptBridger is the critical enrichment.** It's what makes the graph cross-dimensional. Without it, provenance and semantic are isolated islands. The tag normalization (`#distributed-ai` → `concept:distributed-ai`) is the glue.

3. **CoOccurrenceEnrichment adds structural value but needs semantic interpretation.** The `may_be_related` edges correctly identify concept clusters. Whether those clusters are meaningful requires domain knowledge — this is the future llm-orc research direction the user identified.

4. **Outbound events are the gap for real consumer integration.** The pipeline produces the right graph mutations but doesn't tell the consumer what happened. Both adapters need `transform_events()` implementations before Trellis/Carrel can react to pipeline results.

5. **The traversal API works across dimensions but has a single-relationship filter.** Multi-hop cross-dimensional walks (mark → concept → fragment) require either no filter (Direction::Both, depth 2) or manual multi-step traversal. A relationship-set filter or typed traversal path would be more ergonomic for real consumer queries.

---

## Open questions surfaced by the spike

**Q1: What outbound events should each consumer receive?**
Trellis cares about: new concepts detected, co-occurrence proposals, bridges formed to research marks. Carrel cares about: bridges formed to writing fragments, co-occurrence context for research papers.

**Q2: Should fragment IDs be deterministic (content-hash) for deduplication?**
Currently UUID-based. If Trellis re-ingests the same fragment, it creates a new node rather than upserting. Deterministic IDs based on content hash would enable idempotent re-ingestion.

**Q3: How should multi-hop cross-dimensional queries be expressed?**
The current TraverseQuery works but is low-level. A higher-level query like "given this mark, find related fragments" would encode the mark → concept → fragment path as a named query pattern.

**Q4: What's the right enrichment loop ordering when both consumers contribute simultaneously?**
Currently each `ingest()` call is independent — fragment ingestion triggers enrichments, then mark ingestion triggers enrichments separately. If both happen in rapid succession, the enrichment results are identical to sequential processing (because the enrichment loop reads context state, which reflects prior committed mutations). No ordering issues observed.

---

## Question 2: What outbound events should FragmentAdapter and ProvenanceAdapter produce for Trellis and Carrel?

**Method:** Code analysis of `transform_events` contract, enrichment adapter IDs, and consumer needs

**Setup:** Analyzed the information flow: `ingest()` accumulates all `GraphEvent`s from both the primary adapter emission and all enrichment loop rounds, then calls `transform_events(all_events, context_snapshot)`. Each `GraphEvent` carries an `adapter_id` field — primary events carry the adapter's own ID, enrichment events carry the enrichment's ID (`"tag-bridger"`, `"co-occurrence"`). The adapter can discriminate all event sources and look up node/edge properties from the context snapshot.

**Findings:**

### The adapter_id field is the key discriminator

Every `GraphEvent` carries `adapter_id`. This means `transform_events` can distinguish:

- **Primary events** — `adapter_id == self.id()` — the direct results of this ingest call
- **TagConceptBridger events** — `adapter_id == "tag-bridger"` — cross-dimensional bridges formed
- **CoOccurrence events** — `adapter_id == "co-occurrence"` — concept relationship proposals

No new infrastructure is needed. The existing event structure carries enough information for rich outbound event generation.

### FragmentAdapter event vocabulary (what Trellis hears)

When Trellis ingests a fragment, four things can happen:

| Event kind | Source | Detail | Consumer reaction |
|---|---|---|---|
| `fragment_indexed` | NodesAdded, adapter_id == self.id(), Document type | Fragment node ID | Trellis stores the Plexus ID for future graph queries |
| `concepts_detected` | NodesAdded, adapter_id == self.id(), Concept type | Concept labels (comma-separated) | Trellis shows auto-detected themes, suggests related tags |
| `bridges_formed` | EdgesAdded, adapter_id == "tag-bridger" | Count of research marks now connected via shared concepts | Trellis shows "Related research" indicator next to fragments |
| `co_occurrences_updated` | EdgesAdded, adapter_id == "co-occurrence" | Concept pairs with scores | Trellis shows theme clusters or suggests tag groupings |

**`fragment_indexed` is operationally critical** — since fragment IDs are UUIDs, this event is Trellis's only way to learn the assigned ID. Without it, Trellis can't query the graph about a specific fragment later.

### ProvenanceAdapter event vocabulary (what Carrel hears)

When Carrel performs a provenance operation, it can receive:

| Event kind | Source | Detail | Consumer reaction |
|---|---|---|---|
| `chain_created` | NodesAdded, adapter_id == "provenance", node_type == "chain" | Chain ID | Carrel confirms creation |
| `mark_added` | NodesAdded, adapter_id == "provenance", node_type == "mark" | Mark ID | Carrel confirms creation |
| `marks_linked` | EdgesAdded, adapter_id == "provenance", relationship == "links_to" | Source → target IDs | Carrel confirms link |
| `mark_removed` | NodesRemoved, adapter_id == "provenance" | Removed node ID | Carrel confirms deletion |
| `marks_unlinked` | EdgesRemoved, adapter_id == "provenance" | Source → target IDs | Carrel confirms unlink |
| `chain_deleted` | NodesRemoved, adapter_id == "provenance" | Chain ID + mark count removed | Carrel confirms cascade deletion |
| `bridges_formed` | EdgesAdded, adapter_id == "tag-bridger" | Concept labels that were bridged | Carrel shows "Connected to writing about X" |

**Note:** CoOccurrence enrichment rarely fires during provenance operations. It triggers on structural events, but AddMark doesn't create `tagged_with` edges — those come from FragmentAdapter. So `co_occurrences_updated` is almost never relevant to Carrel.

### Events should be notifications, not data payloads

The OutboundEvent type (`kind: String, detail: String`) is deliberately simple. This is correct. The event says *what changed* — the consumer queries the graph for *details*.

This is analogous to database change notifications: "table X was updated" rather than "here are the new rows." The consumer already has a connection to Plexus via the transport layer and can issue traversal queries. The event tells it *when* to re-query.

The `detail` field provides enough human-readable context for logging, UI notifications, and simple dispatch, without requiring the consumer to parse structured data for every event.

### Confirmation events vs. discovery events

Two categories emerge:

**Confirmation events** (`fragment_indexed`, `chain_created`, `mark_added`, `marks_linked`, `mark_removed`, `marks_unlinked`, `chain_deleted`) are essentially ACKs. They confirm the operation succeeded and return IDs. These are necessary for error handling but don't carry Plexus's unique value. Any database could provide these.

**Discovery events** (`concepts_detected`, `bridges_formed`, `co_occurrences_updated`) are where Plexus adds value beyond what a simple store would provide. These tell the consumer: "The graph learned something new because of what you just contributed." This is the enrichment loop's value proposition made visible to consumers.

### The OutboundEvent type is sufficient as-is

No changes to `OutboundEvent { kind, detail }` are needed. The `kind` string handles dispatch in the consumer. The `detail` string carries enough context for immediate use. For richer data, the consumer queries the graph.

If a future consumer needs structured event data (e.g., a list of concept IDs rather than comma-separated labels), the `detail` field can carry JSON without changing the type. But this should be deferred until a concrete need arises.

**Implications:**

1. **Both adapters can implement `transform_events` with no changes to existing types or traits.** The information needed is already carried by `GraphEvent` + `Context`.

2. **The enrichment adapter_id convention is an implicit API.** Adapters that want to report on enrichment results depend on knowing the enrichment IDs ("tag-bridger", "co-occurrence"). This coupling is acceptable because enrichments and adapters are registered together via `register_integration()`, but it should be documented.

3. **`fragment_indexed` is not optional.** UUID fragment IDs make this event operationally necessary — without it, Trellis loses the ability to reference its own data in Plexus.

4. **Discovery events are the unique value.** The confirmation events could come from any store. The discovery events — "your fragment is connected to research you didn't know about" — are what justify the complexity of the ingest pipeline.

5. **Implementation is straightforward.** Each adapter's `transform_events` is a filter-and-format pass over the event slice. No graph queries, no complex logic. The spike proved the events carry enough information; the implementation is mechanical.
