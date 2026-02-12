# Research Log: Provenance as Automatic Cross-Cutting Concern

## Question 1: Where should automatic provenance-dimension node creation live in the architecture?

**Method:** Code analysis of IngestPipeline, EngineSink, and enrichment loop

**Setup:** Three candidate locations identified:

- **Option A: Adapter-level** — each adapter creates its own provenance nodes alongside semantic/structural ones. Violates separation of concerns: adapters know about domain semantics, not provenance mechanics.

- **Option B: Sink-level (EngineSink)** — the sink automatically creates provenance nodes for every emission. Changes the sink's contract: it would emit more than the adapter requested. Also complicates the enrichment loop, which uses the same sink.

- **Option C: Pipeline-level (IngestPipeline)** — the ingest pipeline creates provenance records between adapter processing and the enrichment loop. Has access to FrameworkContext (adapter_id, context_id, input_summary) and committed node IDs from GraphEvent::NodesAdded.

**Findings:**

Option C is the clear winner:

1. The IngestPipeline already constructs FrameworkContext per adapter — it knows who produced what.
2. NodesAdded events from step 2 carry committed node IDs — we know what was produced.
3. The context snapshot (post step 2) has each node's dimension — we can create cross-dimensional edges.
4. Injecting between steps 2 and 3 means the enrichment loop sees the provenance nodes and can react to them.
5. Adapters don't change at all — provenance is transparent and automatic.

**Implications:** The ingest pipeline is the right place. One provenance "ingest record" node per adapter invocation, with `produced_by` cross-dimensional edges from each committed node to the record.

---

## Question 2: Does auto-provenance creation cause infinite enrichment loops?

**Method:** Analysis of enrichment idempotency contracts

**Setup:** The enrichment loop runs after primary emissions. If auto-provenance creates new nodes (NodesAdded events), enrichments will see them. Two enrichments to check:

- **TagConceptBridger** — fires on NodesAdded, but only bridges mark nodes with tag arrays. An ingest_record node doesn't have tags → bridger returns None.

- **CoOccurrenceEnrichment** — fires on NodesAdded/EdgesAdded, but looks for `tagged_with` edges. Ingest records have `produced_by` edges → co-occurrence returns None.

**Findings:** No infinite loop. Neither enrichment has reason to process ingest_record nodes. The `produced_by` edges don't match any enrichment's trigger conditions. Enrichment loop reaches quiescence in the same number of rounds as before.

**Implications:** Safe to inject provenance nodes before the enrichment loop. No safety valve changes needed.

---

## Question 3: Can we traverse concept → provenance → source evidence after auto-provenance creation?

**Method:** Spike

**Question:** "When FragmentAdapter produces concept:travel, can a depth-1 traversal from concept:travel follow a produced_by edge to reach an ingest_record node in the provenance dimension, revealing the adapter, timestamp, and context that created the concept?"

**Plan:**
1. Modify IngestPipeline::ingest() to create ingest_record nodes with produced_by edges
2. Write an integration test: ingest a fragment, then traverse from concept to ingest_record
3. Verify enrichment loop still reaches quiescence
4. Run all 246 existing tests

**Findings:**

### The implementation is 16 lines of production code

Added step 2.5 to IngestPipeline::ingest() — after adapter processing, before the enrichment loop. For each adapter that committed nodes:
1. Create one `ingest_record` node (provenance dimension, ContentType::Provenance)
2. Set properties: adapter_id, timestamp (RFC3339), context_id, node_count
3. Create `produced_by` cross-dimensional edges from each committed node to the record
4. Emit through a provenance-specific sink

No changes to any adapter, enrichment, sink, or graph type. Zero interface changes.

### Cross-dimensional traversal works

From `concept:travel`, follow `produced_by` → reach `ingest_record` node with properties:
- adapter_id: "trellis-fragment"
- timestamp: "2026-02-12T..."
- context_id: "test-auto-provenance"
- node_count: 3

Reverse traversal also works: from the ingest_record, follow reverse `produced_by` → reach the fragment node, concept:travel, concept:avignon — all three nodes produced by that ingest call.

### Multiple ingests produce separate records

Two ingest calls create two ingest_record nodes. Shared concepts (like `concept:architecture`, upserted by both) get `produced_by` edges to both records. This means you can see the full history of how a concept entered the graph — which adapter, which ingest call, what timestamp.

### Enrichment loop reaches quiescence

TagConceptBridger and CoOccurrenceEnrichment both ignore ingest_record nodes (no tags, no tagged_with edges). The enrichment loop terminates in the same number of rounds as before auto-provenance.

### All 248 tests pass (246 existing + 2 new)

The two-consumer validation test (essay 11) passes unchanged. Auto-provenance adds nodes in the provenance dimension and `produced_by` edges — neither conflicts with existing assertions which test per-relationship and per-dimension counts.

### The existing ProvenanceEntry becomes the in-process complement

ProvenanceEntry (metadata record in EmitResult) and ingest_record (graph node) capture overlapping information. ProvenanceEntry includes per-node annotations (confidence, method, source_location) that the ingest_record doesn't carry. The two mechanisms serve different purposes:
- ProvenanceEntry: immediate metadata for the adapter's process() method, per-node granularity
- ingest_record: graph-traversable provenance for consumers and queries, per-ingest granularity

They coexist without conflict. A future enhancement could link ProvenanceEntry annotations to produced_by edge properties, but this isn't needed for the core traversal use case.

**Implications:**

1. Auto-provenance is a cross-cutting concern that belongs in the pipeline, not in adapters.
2. The implementation is minimal, non-invasive, and backward-compatible.
3. The enrichment loop is naturally immune to ingest_record nodes.
4. Cross-dimensional traversal from any node to its provenance origin now works.
5. The ProvenanceAdapter (user annotations) and auto-provenance (system records) coexist cleanly — they serve different purposes in the same dimension.
