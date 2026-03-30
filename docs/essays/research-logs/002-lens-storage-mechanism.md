# Research Log: Lens Storage Mechanism Spike

## Question 1: What does Plexus's actual storage layer look like, and what constraints does it impose on a lens?

**Method:** Codebase spike (src/storage/sqlite.rs, src/graph/, src/adapter/)

**Findings:**

The storage layer has four properties that constrain lens design:

1. **Single table for edges.** All edges — adapter-created, enrichment-created — live in the `edges` table with the same schema: `(id, context_id, source_id, target_id, source_dimension, target_dimension, relationship, raw_weight, created_at, properties_json, contributions_json)`. There is no per-enrichment partition, no edge-type table, no namespace column.

2. **Contributions as the only provenance signal.** The `contributions_json` column (`HashMap<AdapterId, f32>`) is the only stored signal that identifies which adapter or enrichment created an edge. The key is the adapter/enrichment `id()` string — e.g., `"co_occurrence:tagged_with:may_be_related"` or `"content-adapter"`. Pattern-matching the key is the only way to distinguish edge origins after storage.

3. **All queries operate on in-memory Context.** The `query/` module never queries SQLite directly. `load_context()` materializes the full context into memory (`HashMap<NodeId, Node>` + `Vec<Edge>`), and all find/traverse/path/step operations work on that in-memory representation. There is no SQL query path for graph traversal.

4. **Enrichment loop is the write-time hook.** The `Enrichment` trait (`fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission>`) is the sole mechanism for reactive graph augmentation at write time. The enrichment loop runs after every adapter emission, receives events, and can produce new nodes and edges via the standard `Emission` type. All four existing enrichments use this path.

**Implications:**

- Any lens mechanism that stores data must either use the existing `edges` table (Option A) or create a new table (Option B). There is no middle ground within the current schema.
- Since queries load the full context into memory, adding lens edges to the `edges` table means they are automatically available to all existing query operations with zero query module changes.
- Creating a separate table would require new load/save paths, new query paths, and would bypass the contribution tracking model.

## Question 2: How do comparable systems handle consumer-scoped views of a shared graph?

**Method:** Web research (Neo4j GDS, SQLite patterns, MV4PG, Graphiti, TinkerPop)

**Findings:**

Six approaches were surveyed:

| System | Mechanism | Stored? | Write Cost | Read Cost |
|--------|-----------|---------|------------|-----------|
| Neo4j GDS projections | In-memory subgraph copy | Materialized (heap) | Projection cost at creation | Near-zero (memory) |
| SQLite trigger tables | Shadow table + triggers | Materialized (disk) | Trigger overhead per write | O(1) indexed read |
| MV4PG (Xu 2024) | Physical view edges in graph | Materialized (in-graph) | Incremental delta templates | Near-zero (first-class edges) |
| Graphiti/Zep | Write-time LLM enrichment | Hybrid | High (LLM inference) | Sub-second hybrid retrieval |
| TinkerPop SubgraphStrategy | Traversal predicate injection | Virtual | None | Full traversal with filter |
| TinkerPop PartitionStrategy | Property tag on elements | Virtual | Tag on creation | Full traversal with filter |

The most instructive comparison is **MV4PG**: materialized view edges stored as first-class edges in the graph. A view is defined declaratively over path patterns, materialized as physical edges, and maintained incrementally via pre-generated delta templates. Multiple views coexist in the same graph. Read speedup up to ~100x; write maintenance cost is O(N) in affected view edges — negligible for typical update sizes.

**Implications:**

- The MV4PG pattern validates Option A (first-class edges) directly. Storing lens output as standard edges in the shared graph is an established pattern with proven performance characteristics.
- Virtual approaches (TinkerPop) have zero write cost but pay full traversal cost. Since Plexus already loads full context into memory, the read-cost difference is between "edge exists in Vec, traversal finds it" vs. "traversal runs translation function on every step."
- Graphiti validates the broader pattern: write-time enrichment produces a graph that needs only simple query primitives at read time. Plexus already follows this pattern; a lens-as-enrichment extends it.

## Question 3: Can the existing enrichment infrastructure support a lens without modification?

**Method:** Codebase spike (src/adapter/enrichment/, src/adapter/enrichments/)

**Findings:**

The `Enrichment` trait is sufficient. A lens would implement:
- `id()` → e.g., `"lens:trellis:thematic"`
- `enrich(events, context)` → examine events and context, produce translated edges

What the existing infrastructure provides:
- **Event routing**: Every enrichment sees every round's events. A lens filters internally (like existing enrichments do).
- **Contribution tracking**: The enrichment loop constructs `FrameworkContext { adapter_id: lens.id() }`, which flows into the edge's `contributions` map at commit time.
- **Cross-enrichment chaining**: Lens-produced edges become events in subsequent rounds, so other enrichments can react to them (and vice versa).
- **Quiescence**: Lens returns `None` when there's nothing new to translate, preventing infinite loops.

What is missing or awkward:
- **Edge annotation is dead code.** `AnnotatedEdge.annotation` is never read by `commit_edges()`. A lens that wants to tag its output edges must use `Edge.properties` or the relationship name.
- **No enrichment namespace.** There is no `edge.source_enrichment` or `edge.is_enrichment_output` field. The only distinguishing signal is the `contributions` key string.
- **No selective event routing.** Every enrichment receives all events. A lens that only cares about co-occurrence edges must filter by inspecting `event.adapter_id` matching `"co_occurrence:*"`.

**Implications:**

- A lens-enrichment requires zero infrastructure changes to work. It registers in `EnrichmentRegistry`, receives events, emits translated edges, and those edges are committed through the standard `emit_inner()` path.
- The relationship name convention (e.g., `"lens:trellis:thematic_connection"`) is the cleanest way to namespace lens output — it requires no schema change and is immediately filterable in queries.
- Edge annotation being dead code is a latent bug/debt item. If fixed, it would provide a natural place for lens metadata.

## Question 4: Which storage mechanism best serves Plexus's architecture?

**Method:** Synthesis of Q1-Q3

**Findings:**

**Option A: First-class edges (existing enrichment path)**

Strengths:
- Zero infrastructure changes. Lens implements `Enrichment` trait, registers in `EnrichmentRegistry`.
- Lens output immediately queryable by all existing query operations (traverse, path, step, evidence_trail).
- Contribution tracking works automatically — lens contributions are recorded alongside adapter and enrichment contributions.
- Cross-enrichment chaining works — a lens can react to co-occurrence edges, and co-occurrence can react to lens edges.
- Matches MV4PG pattern: materialized view edges as first-class graph elements.
- Matches Invariant 34 (all writes through ingest) and Invariant 56 (lens output is public).

Weaknesses:
- Adds edges to the graph, increasing `Context.edges` size. For a context with N edges and K active lenses, the edge count grows by O(K * translation_edges_per_lens).
- Edge deduplication in `add_edge()` is a linear scan (O(n)). More edges → slower dedup.
- No built-in way to "refresh" a lens — rerunning requires retracting old lens contributions and re-emitting.

**Option B: Per-consumer SQLite index/view**

Strengths:
- Clean separation: lens output in dedicated table, doesn't inflate the edge count.
- Could use SQLite indexes for fast lookup by consumer.

Weaknesses:
- Requires new table schema, new save/load paths, new query integration.
- Bypasses contribution tracking — lens provenance would need a separate mechanism.
- Not traversable by existing query operations — would need query module changes.
- Breaks the "all edges in one place" model that makes enrichment chaining work.
- The full context is already loaded into memory; a separate table would need separate loading.
- Significant new complexity with no clear performance advantage given the in-memory query model.

**Option C: Query-time translation mapping**

Strengths:
- Zero storage overhead. Zero staleness risk.
- Conceptually simple: register a translation function, apply at query time.

Weaknesses:
- Cannot create new structural relationships — only reweight or filter existing ones. A lens that needs to say "these two concepts are thematically related" (a new edge that doesn't exist) cannot express this as a query-time filter.
- Runs on every query, adding latency proportional to translation complexity.
- The existing `NormalizationStrategy` already serves the "query-time interpretive lens" role for weights. A lens that does more than reweight needs to produce structure.

**Recommendation: Option A, with naming convention.**

A lens is an enrichment that translates cross-domain relationships into consumer vocabulary. It produces first-class edges with namespaced relationship types (e.g., `"lens:trellis:thematic_connection"`). These edges are stored, queryable, and tracked like any other enrichment output. The contributions key identifies the producing lens. The relationship namespace enables filtering ("show me only Trellis lens edges" = filter by `relationship.starts_with("lens:trellis:")`).

The edge count growth concern is real but bounded: a lens translates existing edges, not creates them from nothing. The number of translated edges scales with the number of cross-domain connections, not with the total graph size. For a Trellis+Carrel shared context, this is the intersection set — likely a small fraction of total edges.
