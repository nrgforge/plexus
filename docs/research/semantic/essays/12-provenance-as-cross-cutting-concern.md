# Provenance as Automatic Cross-Cutting Concern

Essay 11 proved that two consumers (Trellis and Carrel) can contribute to a shared context and discover connections through the enrichment loop. The cross-dimensional bridges worked: from a research mark, you could traverse through concepts to reach writing fragments neither consumer knew about.

But a question surfaced during the post-spike analysis. The ProvenanceAdapter — born as a migration artifact from wrapping old MCP operations in the adapter pattern (ADR-012) — creates provenance-dimension nodes for user annotations (chains and marks). Yet when FragmentAdapter creates `concept:travel`, the only record of that creation is a `ProvenanceEntry` metadata struct returned in the `EmitResult`. That metadata never becomes a graph node. You can't start at `concept:travel` and traverse through the graph to discover where it came from.

This matters because of what Plexus promises to be: a multi-dimensional knowledge graph where all dimensions are populated and traversable. If the provenance dimension only contains user annotations, it's incomplete. The system's own knowledge of how concepts and fragments entered the graph — which adapter, when, in what context — lives outside the graph entirely.

## The Conceptual Tension

The ProvenanceAdapter is a semantic-first integration. It translates Carrel's domain operations (create chain, add mark, link marks) into graph structure. That's valuable. But it conflates two things that should be separate:

**User provenance** — "I marked this passage as relevant to federated learning." This is a human annotation, recorded as a mark node with tags. The ProvenanceAdapter handles this correctly.

**System provenance** — "The trellis-fragment adapter created concept:travel at 14:32 UTC in the carrel-workspace context." This is the system's record of how knowledge entered the graph. Nothing currently handles this.

The `ProvenanceEntry` struct captures system provenance metadata — adapter ID, timestamp, context, optional annotation with confidence and method. But it's a transient return value. The adapter gets it back from the sink, and then it evaporates. No consumer can query it. No traversal can reach it.

## The Design Question

Should provenance be an automatic cross-cutting concern? Should every adapter ingest produce traversable provenance-dimension nodes alongside its semantic and structural contributions, without the adapter having to do anything?

Three architectural options were evaluated:

**Adapter-level** — each adapter creates its own provenance nodes. This violates separation of concerns. Adapters know about their domain (fragments, marks, code); they shouldn't also know about provenance record structure. It would duplicate provenance logic across every adapter.

**Sink-level** — the `EngineSink` automatically creates provenance nodes for every emission. This changes the sink's contract: it would commit more than the adapter requested. It also complicates the enrichment loop, which uses the same sink. The sink should be a faithful executor, not a policy layer.

**Pipeline-level** — the `IngestPipeline` creates provenance records between adapter processing and the enrichment loop. The pipeline already constructs `FrameworkContext` (adapter ID, context ID, input summary). The `NodesAdded` events from adapter processing carry committed node IDs. The context snapshot has each node's dimension for cross-dimensional edge creation.

The pipeline-level option won. The ingest pipeline is where cross-cutting policy belongs — it's the orchestrator that knows about all adapters, all enrichments, and all events. It's the only component with enough context to create a complete provenance record.

## The Spike

The implementation added a step 2.5 to `IngestPipeline::ingest()`, between adapter processing (step 2) and the enrichment loop (step 3).

For each adapter that committed nodes in step 2:

1. Extract committed node IDs from `NodesAdded` events for that adapter
2. Create an `ingest_record` node in the provenance dimension with properties: `adapter_id`, `timestamp`, `context_id`, `node_count`
3. Look up each committed node's dimension from the context snapshot
4. Create cross-dimensional `produced_by` edges from each committed node to the ingest record
5. Emit through the sink

The implementation is 16 lines of production code in `ingest.rs`. No changes to any adapter, enrichment, sink, or graph type. Zero interface changes.

## What the Spike Proved

### Cross-dimensional traversal works

Starting from `concept:travel`, follow a `produced_by` edge to reach an `ingest_record` node in the provenance dimension. The record carries:

- `adapter_id`: "trellis-fragment" — which adapter created this concept
- `timestamp`: RFC3339 — when it was created
- `context_id`: "test-auto-provenance" — which context it lives in
- `node_count`: 3 — how many nodes were created in the same ingest call

Reverse traversal also works: from the ingest record, follow reverse `produced_by` edges to discover all nodes produced by that ingest call — the fragment node, `concept:travel`, and `concept:avignon`.

This is the traversal that was previously impossible. Before auto-provenance, `concept:travel` existed in the semantic dimension with no graph-traversable path to its origin. Now the origin is one hop away.

### Multiple ingests produce separate records

Two fragment ingests create two ingest records. When both fragments share a tag (like "architecture"), the shared concept gets `produced_by` edges to both records. You can see the full lineage: `concept:architecture` was first produced by ingest A, then upserted by ingest B. Both origins are preserved in the graph.

### The enrichment loop is immune

The enrichment loop runs after auto-provenance records are created. Two enrichments were checked:

**TagConceptBridger** fires on `NodesAdded` but only bridges mark nodes with tag arrays. An `ingest_record` node has properties but no tags array in the bridger's expected format. The bridger ignores it.

**CoOccurrenceEnrichment** fires on `NodesAdded` and `EdgesAdded` but looks for `tagged_with` edges when building its reverse index. Ingest records have `produced_by` edges, not `tagged_with`. The co-occurrence enrichment ignores them.

Quiescence is reached in the same number of rounds as before. No safety valve changes needed.

### All existing tests pass

248 tests (246 existing + 2 spike), zero failures. The two-consumer validation test from essay 11 passes unchanged. Auto-provenance adds nodes in the provenance dimension and `produced_by` edges — a new relationship type in a dimension that existing tests don't count. The change is strictly additive.

## ProvenanceEntry and Ingest Records

Two mechanisms now capture system provenance, at different granularities:

**ProvenanceEntry** — a metadata struct returned in `EmitResult`, constructed per-node. Includes adapter-provided annotations (confidence, method, source location). Transient — consumed by the adapter's `process()` method and then gone. Useful for immediate per-node metadata.

**Ingest record** — a graph node in the provenance dimension, created per-ingest-call. Includes adapter ID, timestamp, context, node count. Persistent — queryable and traversable. Useful for graph-level provenance queries.

They coexist without conflict. ProvenanceEntry is the detailed, per-node, in-process view. The ingest record is the coarse, per-call, graph-traversable view. A future enhancement could link per-node annotations to `produced_by` edge properties, but this isn't needed for the core use case: "Where did this concept come from?"

## What This Means for the Architecture

### Provenance is a pipeline concern, not an adapter concern

Adapters produce semantic and structural contributions. The pipeline adds provenance automatically. This is the right separation: adapters bring domain meaning, the pipeline adds operational metadata. No adapter needs to know about provenance records.

### The provenance dimension serves two populations

**User provenance** (chains, marks, links) — managed by ProvenanceAdapter, representing human annotations and research trails. These have tags, which means TagConceptBridger bridges them to the semantic dimension.

**System provenance** (ingest records) — managed by the pipeline, representing how knowledge entered the graph. These have adapter IDs and timestamps but no tags, so they don't trigger enrichments. They're connected to their produced nodes via `produced_by` edges.

Both live in the provenance dimension. Both are graph-traversable. They're complementary: a mark says "this passage is about federated learning" (human judgment); an ingest record says "the trellis-fragment adapter created concept:federated-learning at 14:32 UTC" (system fact).

### Every node's origin is now one hop away

From any node in any dimension, follow `produced_by` to its ingest record. From the ingest record, inspect properties to learn which adapter, when, and in what context. Follow reverse `produced_by` to see what else was produced at the same time.

This makes the graph self-documenting. You don't need external logs or metadata APIs to understand where knowledge came from. The graph contains its own provenance.

### The enrichment loop is naturally robust

Auto-provenance creates nodes and edges that no current enrichment processes. This is because enrichments are semantic-specific: TagConceptBridger looks for tags, CoOccurrenceEnrichment looks for `tagged_with` edges. Ingest records have neither. If a future enrichment needs to process provenance records (e.g., to detect patterns in adapter activity), it can — the data is in the graph. But existing enrichments are unaffected.

## Test Suite

248 tests, zero failures. The spike added 2 integration tests verifying single-ingest and multi-ingest auto-provenance behavior including cross-dimensional traversal.
