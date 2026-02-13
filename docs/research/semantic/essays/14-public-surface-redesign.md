# Essay 14: The Public Surface Redesign

## The gap between architecture and interface

Plexus's internal architecture settled into a clean shape across Essays 10–13. Writes flow through a unified ingest pipeline. Adapters produce both semantic output and provenance trails (the "dual obligation" from Essay 12). Enrichments run until quiescence, bridging tags to concepts and detecting co-occurrences. The graph accumulates knowledge across three dimensions — structure, semantic, provenance — with cross-dimensional edges connecting them.

But the public surface — what consumers actually interact with — hasn't kept pace. The MCP layer still exposes 19 tools inherited from the pre-adapter era, when provenance operations were direct graph mutations. The query system offers three primitives (`FindQuery`, `TraverseQuery`, `PathQuery`) that work for single-dimension queries but can't express the cross-dimensional traversals that are the whole point of a multi-dimensional knowledge graph. The most important queries in the system — the "key traversals" validated in Essay 13 — bypass the query system entirely, using raw edge iteration with hand-written predicates.

The architecture promises rich, traversable, multi-dimensional knowledge. The interface doesn't deliver on that promise.

## Three consumers, one shared need

To understand what the surface should look like, we mapped the concrete queries four known consumers would need.

**Trellis** is an app-to-app consumer that ingests tagged writing fragments. Its needs are write-heavy: send fragments, get back confirmation. On the read side, it needs basic queries — list concepts, find fragments for a concept, check edge weights. These are simple single-dimension queries that the existing `FindQuery` and single-hop `TraverseQuery` handle adequately.

**Carrel** is a writer's research desk — a local workspace that aggregates writing scattered across multiple repositories, runs scheduled research against Semantic Scholar, manages voice profiles, and publishes drafts to multiple targets. Its Plexus contexts are research environments: each context contains the writer's own fragments (possibly arriving via Trellis), research papers discovered by scheduled Semantic Scholar scans, and other relevant material — all ingested through semantic adapters that extract concepts and relationships, with LLM-powered adapters doing deeper analysis (topic extraction, thematic clustering, relevance scoring). Provenance chains and marks layer on top as the writer's explicit annotations — narrative trails through the accumulated material — but the semantic layer is doing heavy lifting underneath. Carrel is a full consumer of the multi-dimensional graph, not just the provenance surface. The query that makes it powerful — "what research supports the themes in this draft?" — requires cross-dimensional traversal connecting the writer's annotations to LLM-extracted concepts to discovered papers, which the current system can't express without manual post-filtering.

**Manza** is a real-time graph visualizer that renders the knowledge graph as users work. It needs full graph snapshots (straightforward — iterate nodes and edges), neighborhood exploration (single-hop traverse works), and streaming deltas for live updates (a separate infrastructure concern, deferred as OQ8). But its core value proposition — letting a user click a concept and see its evidence trail across all dimensions — requires the same cross-dimensional traversal that Carrel needs.

**Sketchbin** is a federated creative workshop where each instance runs Plexus locally as its semantic engine. A SketchAdapter ingests each published sketch (tagged creative artifacts — audio, code, writing, visual work), and a FederationAdapter ingests incoming ActivityPub activities from followed creators. The enrichment loop discovers cross-creator semantic connections: co-occurrences between your "ambient" work and a followed creator's "texture" explorations, concept bridges between your field recordings and someone else's generative code. Discovery propagates through the trust network — two-hop semantic resonance dampened by social distance. Sketchbin's key requirement is provenance transparency: every discovery must be explainable ("Carol's sketch connects to your 'generative' cluster — via Alice's engagement with similar themes"). This is the `evidence_trail` query as a user-facing feature.

The pattern is clear. All four consumers, despite radically different interaction patterns (app-to-app ingestion, writer's research coordination, real-time visualization, federated creative discovery), converge on the same query: **"What is the evidence trail for this node?"**

This query follows a specific shape:
1. From a concept node → traverse `references` edges (incoming) → marks in the provenance dimension
2. From the same concept → traverse `tagged_with` edges (incoming) → fragments in the structure dimension
3. From each mark → traverse `contains` edges (incoming) → the chain it belongs to

Each hop follows a different relationship type. Each hop crosses or stays within specific dimensions. This is not a generic "find everything within 2 hops" — it's a typed, multi-step traversal where the relationship at each step matters.

## Why the existing query system can't express this

`TraverseQuery` does breadth-first search from an origin node to a specified depth. It accepts one optional relationship filter and has no dimension awareness. Asking it for depth-2, direction-both traversal returns every node within 2 hops regardless of relationship type or dimension — a massive over-fetch that the consumer must manually filter.

`FindQuery` filters nodes by type, content type, dimension, and properties. It answers "what nodes match these criteria?" but can't express graph structure — it doesn't follow edges.

`PathQuery` finds the shortest path between two known nodes. It requires you to already know both endpoints — it can't discover the evidence trail, only verify a connection.

The gap isn't about missing features on these primitives. It's a missing *primitive*: a typed multi-hop traversal where each step specifies its own relationship filter and direction. Something like:

```
from(concept_id)
  .step(Incoming, "references")    // → marks
  .step(Incoming, "tagged_with")   // → fragments
  .step(Incoming, "contains")      // → chains
```

This would express the evidence trail as a single query instead of three separate traversals with post-filtering. It belongs in Plexus core — not in transport-specific code — because all four consumers need it regardless of how they connect.

## The MCP surface problem

Separately from query ergonomics, the MCP tool surface has a structural problem. It exposes 19 tools — 6 for context management and 13 for provenance operations. The provenance writes already route through the ingest pipeline internally (ProvenanceAdapter handles `create_chain`, `add_mark`, `link_marks`, etc.), but the MCP surface still presents them as separate tools with graph-primitive names.

Research into MCP tool design patterns revealed a consistent principle: **less is more**. Tool definitions consume 5–7% of the context window before a single prompt. More tools means worse LLM selection accuracy and higher token cost. The recommended pattern is workflow-based design — expose operations that match what the user wants to *do*, not what the graph mutates underneath.

The current tool name `add_mark` exposes implementation details (the mark/chain containment model). A workflow-oriented name like `annotate` matches user intent. The operation "annotate this code location with these tags, in this chain" is one conceptual action that the current surface splits into two tool calls (`create_chain` then `add_mark`).

But the deeper insight is that **MCP is a transport, not the API**. Plexus's API should be transport-independent: one set of operations that any shell — MCP, gRPC, REST, direct Rust embedding — can wrap. The MCP surface derives from the API; it doesn't define it.

## The public surface that emerged

### Write operations

One write endpoint: `ingest(context_id, input_kind, data)`. This already exists and works. Adapters registered for each `input_kind` handle transformation, provenance, enrichment, and outbound events.

Transport layers present this with workflow-oriented names:
- **`annotate`** — add a mark to a file location with tags, in a named chain. Auto-creates the chain if it doesn't exist. Wraps `ingest("provenance", AddMark{...})` with implicit chain creation. This collapses `create_chain` + `add_mark` into one operation for the common case.
- **`ingest_fragment`** — send a tagged text fragment. Wraps `ingest("fragment", FragmentInput{...})`.
- **`link_marks` / `unlink_marks`** — structural annotation operations. Wrap `ingest("provenance", LinkMarks{...})`.
- **`delete_mark` / `delete_chain`** — destructive operations. Wrap `ingest("provenance", DeleteMark{...})`.

Two mutations bypass ingest because they're read-modify-write operations, not graph additions:
- **`update_mark`** — modify mark properties (annotation text, tags, type)
- **`archive_chain`** — change chain status

### Read operations

The existing provenance reads are sound:
- **`list_chains`** — list annotation chains, optionally filtered by status
- **`get_chain`** — a chain with all its marks
- **`list_marks`** — search marks by chain, file, type, or tag
- **`list_tags`** — all tags in use
- **`get_links`** — a mark's incoming and outgoing links

The existing graph queries remain:
- **`find_nodes`** — search nodes by type, content type, dimension, properties
- **`traverse`** — BFS from a start node (single relationship, single direction)
- **`find_path`** — shortest path between two known nodes

The new query:
- **`evidence_trail`** — the cross-dimensional typed traversal. Given a node (typically a concept), returns all evidence: marks that reference it, fragments tagged with it, chains that contain those marks. This is the query that makes the multi-dimensional graph useful to consumers.

### Context management

Unchanged — already clean CRUD:
- `context_create`, `context_delete`, `context_list`, `context_rename`
- `context_add_sources`, `context_remove_sources`

## What this means architecturally

The public surface is now three layers:

```
Transport layer (thin shells)
├── MCP server (workflow-oriented tool names)
├── Future: gRPC / REST / WebSocket
└── Direct Rust embedding (crate API)
         ↓
API layer (transport-independent operations)
├── Write: ingest(context_id, input_kind, data)
├── Read: evidence_trail, find_nodes, traverse, find_path
├── Provenance reads: list_chains, get_chain, list_marks, ...
├── Mutations: update_mark, archive_chain
└── Context: CRUD + sources
         ↓
Core engine
├── IngestPipeline (adapters + enrichments + outbound events)
├── Query system (FindQuery, TraverseQuery, PathQuery, EvidenceTrail)
├── ProvenanceApi (domain facade for provenance reads)
└── Context (in-memory graph + persistence)
```

Transport shells translate protocol-specific requests into API operations. The API layer is what Plexus exposes regardless of transport. The core engine is the implementation.

The key additions this design implies:
1. **A typed multi-hop traversal query** (`EvidenceTrail` or a more general `StepQuery`) in the query system
2. **An API layer** that sits between transports and the engine — currently the MCP server calls ProvenanceApi and engine methods directly; this intermediary would formalize the operation set
3. **Auto-chain creation** in the `annotate` workflow — chains become implicit containers rather than explicit prerequisites

## What this doesn't address

**Event streaming** (Manza's real-time updates) is a separate concern. The current `GraphEvent` system fires events during emission but doesn't persist or stream them. This remains OQ8.

**LLM enrichment strategy** (using LLMs in the enrichment loop) is deliberately deferred. It intersects with LLM Orchestra and deserves its own research cycle.

**Non-ingest mutations** (`update_mark`, `archive_chain`) bypass the pipeline. They're read-modify-write operations that don't produce new knowledge, so the adapter pattern doesn't naturally fit. They work as direct engine operations, but their existence outside the pipeline is a design smell worth monitoring.
