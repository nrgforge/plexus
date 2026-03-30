# Lens Storage: First-Class Edges as Materialized Translation
*2026-03-25*

## Abstract

This spike investigation addresses the open architectural question (OQ-20) of what a lens-enrichment produces at the database and storage level. Three candidate mechanisms were evaluated — first-class graph edges, per-consumer SQLite index tables, and query-time translation mappings — against Plexus's actual SQLite schema, enrichment loop, and in-memory query model. Web research into comparable systems (Neo4j GDS projections, MV4PG materialized view edges, TinkerPop SubgraphStrategy, Graphiti/Zep) provided external validation. The investigation found that first-class edges, produced through the existing `Enrichment` trait with no infrastructure changes, are the correct mechanism. This result is validated by the MV4PG pattern (physical view edges stored in the graph) and is consistent with Plexus's existing enrichment architecture, where co-occurrence, embedding similarity, discovery gap, and temporal proximity enrichments already produce first-class edges as their output.

---

## The Architectural Question

The domain model (Invariant 56) establishes that lens output is public — visible to all consumers in the shared context. The product discovery (2026-03-25) frames a lens as a consumer-scoped enrichment that translates cross-domain graph content into one consumer's domain vocabulary at write time. What remained unresolved was the storage mechanism: what artifact does this translation produce in the database?

The question matters because the storage mechanism determines query ergonomics (how easily consumers can find and traverse lens output), maintenance cost (what happens when the graph changes), and architectural fit (whether the lens mechanism integrates with or diverges from existing infrastructure).

Three candidates were evaluated:

- **Option A: First-class edges.** The lens produces standard `Edge` structs stored in the `edges` table, identical in schema to adapter-created and enrichment-created edges. The lens implements the `Enrichment` trait.
- **Option B: Per-consumer SQLite index.** The lens writes to a separate table (e.g., `lens_edges`) with a consumer-specific schema, maintained alongside the main `edges` table.
- **Option C: Query-time translation mapping.** The lens registers a translation function that runs at query time, transforming or filtering edges on the fly without storing any new data.

---

## What the Codebase Reveals

### The Storage Model Is Uniform

Plexus stores all edges — from adapters and enrichments alike — in a single `edges` table:

```
(id, context_id, source_id, target_id, source_dimension, target_dimension,
 relationship, raw_weight, created_at, properties_json, contributions_json)
```

There is no per-enrichment partition, no edge-type column, no namespace field. The only signal that identifies an edge's origin is the `contributions_json` column: a `HashMap<AdapterId, f32>` where the key is the adapter or enrichment `id()` string. A co-occurrence edge has contributions keyed by `"co_occurrence:tagged_with:may_be_related"`; an adapter-created edge has contributions keyed by `"content-adapter"` or similar.

This uniformity is not accidental — it is the mechanism by which multiple adapters and enrichments independently strengthen the same edge. An edge discovered by co-occurrence and then confirmed by embedding similarity accumulates two contribution slots. Raw weight is recomputed from all contributions after each emission (ADR-003).

### All Queries Run In-Memory

The query module (`src/query/`) never queries SQLite directly. `load_context()` materializes the full context into memory as `HashMap<NodeId, Node>` + `Vec<Edge>`, and all find/traverse/path/step operations work on that in-memory representation. Traversal queries build temporary hash indexes at query time.

This means any edge stored in the `edges` table is automatically available to all query operations once the context is loaded. Conversely, data stored in a separate table would need separate loading and separate query integration.

### The Enrichment Loop Is the Write-Time Hook

The `Enrichment` trait provides the reactive hook:

```rust
fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission>;
```

Every enrichment receives every round's events and a cloned context snapshot. Each returns `None` (quiescent) or `Some(Emission)` containing new nodes and edges. The enrichment loop runs once per `ingest()` call on the combined events from all matched adapters (ADR-029); it commits emissions through the same `emit_inner()` path used by adapters, which persists per-emission (Invariant 30).

All four existing enrichments — co-occurrence, embedding similarity, discovery gap, temporal proximity — produce standard `Edge` structs with their enrichment ID as the contributions key. A lens implementing `Enrichment` would follow the identical pattern.

---

## What Comparable Systems Show

### MV4PG: View Edges as First-Class Graph Elements

The most directly relevant comparison is MV4PG (Xu et al., 2024), which implements materialized views for property graphs. A view is defined declaratively over path patterns and materialized as **physical edges in the graph itself**. The view edge `(a)-[:ROOT_POST]->(b)` collapses a variable-length `REPLY_OF` path into a single stored edge. Multiple views coexist in the same graph.

MV4PG maintains views incrementally via pre-generated delta templates — parameterized maintenance statements created at view definition time. When the base graph changes, the system substitutes actual values into the templates rather than recomputing from scratch. The measured overhead is O(N) in affected view edges; the measured per-query read speedup is up to ~100x, with full workload speedup of ~28.71x when accounting for mixed read/write operations.

This validates the principle: storing derived relationships as first-class graph elements is an established pattern with proven performance characteristics. A lens-enrichment that produces standard edges is the Plexus equivalent of an MV4PG materialized view edge.

### TinkerPop: Virtual Approaches and Their Costs

TinkerPop's SubgraphStrategy and PartitionStrategy represent the virtual end of the spectrum — traversal-time filters with zero storage overhead. SubgraphStrategy injects predicate checks into every traversal step; PartitionStrategy uses a designated property to tag elements by consumer. Neither materializes results.

The performance cost is proportional to graph size, not result size: every traversal step touches and discards non-matching elements. For Plexus's in-memory model (where the full context is already loaded), this would mean the lens translation function runs on every traversal step of every query. The cost is CPU, not I/O, but it scales linearly with graph size per query.

### Graphiti/Zep: Write-Time Enrichment, Thin Query Surface

Graphiti validates the broader write-heavy/query-light pattern: push semantic intelligence to write time so the query surface can be simple. Graphiti's write pipeline produces enriched edges (entity-extracted, temporally annotated, deduplicated); its query surface offers three primitives (cosine similarity, BM25, BFS traversal) combined through reranking. No LLM calls at query time in the default configuration.

A lens-enrichment follows this pattern exactly: translate at write time so queries operate on pre-translated structure.

---

## Evaluating the Three Options

### Option A: First-Class Edges

A lens implements the `Enrichment` trait, receives graph events, and emits translated edges with consumer-specific relationship types (e.g., `"lens:trellis:thematic_connection"`). These edges are stored in the `edges` table with the lens's ID in the contributions map.

**What this gets right:**
- Zero infrastructure changes. The `Enrichment` trait, `Emission` type, enrichment loop, `emit_inner()` commit path, and contribution tracking all work as-is.
- Lens output is immediately queryable by all existing operations — `traverse`, `find_path`, `step`, `evidence_trail` — without query module changes.
- Contribution tracking records the lens as a distinct contributor. The data needed for evidence diversity queries exists in the contributions map — lens contributions are stored alongside adapter and enrichment contributions. The query interface to surface provenance-scoped filtering (Invariant 59) is not yet implemented (OQ-23), but the storage model supports it without schema changes.
- Cross-enrichment chaining works: a lens can react to co-occurrence edges (round N), and co-occurrence can react to lens edges (round N+1). The enrichment loop's multi-round dispatch handles this natively.
- Matches Invariant 34 (all writes through ingest) and Invariant 56 (lens output is public).

**What this costs:**
- Each lens adds edges to the graph. For a context with E edges and K active lenses, the total edge count grows. The growth is bounded by the number of cross-domain connections worth translating — a fraction of E, not a multiple.
- Edge deduplication in `Context.add_edge()` uses two linear scans — one for exact match and one for cross-dimensional match. More edges → slower dedup per emission (O(n) with a constant factor of 2). This is a pre-existing concern (OQ-16) that applies to all enrichments, not specifically to lenses.
- Recomputing lens output after graph changes requires retracting the lens's contributions and re-running. This is the same pattern used by all enrichments — the enrichment loop handles it naturally through event-driven dispatch.

### Option B: Per-Consumer SQLite Index

A new `lens_edges` table (or per-consumer table) stores the lens translation output separately from the main `edges` table.

**What this gets right:**
- Clean separation: lens output does not inflate the main edge count.
- Could support consumer-specific schema (e.g., pre-computed fields specific to Trellis's needs).

**What this costs:**
- New table schema, new `save_context()`/`load_context()` paths, new migration.
- Lens edges are not in `Context.edges`, so they are invisible to all existing query operations. Every query function would need modification to also check the lens table.
- Bypasses contribution tracking. Lens provenance would need a separate mechanism, diverging from the uniform provenance model.
- Breaks enrichment chaining: other enrichments cannot react to lens edges because they are not in the context snapshot.
- Significant new complexity. The justification would need to be a clear performance advantage — but since queries run in-memory on the loaded context, there is no I/O advantage to a separate table; the edges must be loaded into memory regardless.

### Option C: Query-Time Translation

The lens registers a mapping function that runs during query execution, translating edge types or creating virtual edges on the fly.

**What this gets right:**
- Zero storage overhead. Zero staleness risk — every query sees the latest graph state through the lens.
- Conceptually closest to the database "view" abstraction.

**What this costs:**
- Difficult to create new structural relationships at query time. A lens that discovers "fragment X and research paper Y are thematically related" needs to produce a new edge connecting X and Y. While query-time computation could in principle synthesize virtual edges (as database query engines do with joins), this is architecturally complex and incompatible with Plexus's in-memory traversal model, which operates on a materialized `Vec<Edge>`. In practice, a query-time approach can filter and reweight existing edges but not efficiently synthesize new connections for traversal.
- Runs on every query. Translation cost is proportional to graph size × query frequency.
- The existing `NormalizationStrategy` already serves the query-time interpretive role for edge weights. Adding a second query-time transformation layer (for structure, not just weights) creates a composition ordering question that the current architecture avoids by keeping structure creation at write time.
- Lens output would not be visible to other enrichments, breaking the cross-enrichment chaining that makes the enrichment loop valuable.

---

## The Answer: A Lens Is Just an Enrichment

Option A is the correct mechanism. A lens is an enrichment that translates cross-domain relationships into consumer vocabulary. The translation manifests as new edges with namespaced relationship types. These edges are stored as first-class entries in the shared graph, participate in the enrichment loop, accumulate contributions, and are queryable by all existing primitives.

The naming convention `lens:{consumer}:{relationship}` (e.g., `"lens:trellis:thematic_connection"`) provides clean namespace separation without schema changes. Queries can filter by relationship prefix to scope results to a specific lens's output. The contributions key `"lens:trellis:thematic"` (the enrichment ID) provides provenance tracking.

### Addressing the "Create an Index?" Question

The question raised during the domain modeling epistemic gate — "Are we saying like... create an index?" — has a precise answer. In the SQL sense, an index accelerates queries on existing data by maintaining a secondary lookup structure. A lens-enrichment does something subtly different: it creates new data (edges) that encode the translation. These new edges are a form of **materialized view** — pre-computed relationships that would otherwise require multi-hop traversal or cross-domain reasoning at query time.

The MV4PG analogy is exact. A MV4PG view edge collapses a multi-hop path pattern into a single stored edge. A lens-enrichment collapses a cross-domain relationship (e.g., "fragment A is tagged with concept X, which Carrel also relates to paper B") into a single translated edge (`"lens:trellis:thematic_connection"` from A to B). The query cost drops from multi-hop traversal to single-edge lookup.

### What a Lens-Enrichment Needs That Does Not Yet Exist

The investigation found that zero infrastructure changes are required for the basic mechanism. However, several areas would benefit from attention:

1. **Declarative lens definition.** The existing `DeclarativeSpec` (YAML adapter configuration) could be extended with a `lens:` section that declares what the lens translates from and to. This parallels how the `enrichments:` section activates core enrichments. The lens definition would include: source relationship types to watch, target relationship type to produce, and translation rules. This is a DECIDE-phase question, not a storage question.

2. **Edge annotation is dead code.** `AnnotatedEdge.annotation` is never read by `commit_edges()`. If revived, it would provide a natural place for lens metadata (e.g., translation confidence, source edge ID). This is a bug/debt item independent of the lens question.

3. **Relationship namespace convention.** A formal convention for `lens:` prefixed relationships should be established, including whether the consumer name is part of the relationship type (scoping) or the enrichment ID (provenance). The current investigation assumes both: `relationship = "lens:trellis:thematic_connection"`, `enrichment_id = "lens:trellis:thematic"`.

4. **Lens refresh on graph change.** When new edges enter the graph, the enrichment loop already dispatches to all registered enrichments. A lens will naturally re-evaluate on each new emission. However, if the lens's input edges are *removed* (contribution retraction), the lens needs to retract its own output. The enrichment loop passes `EdgesRemoved` events, so the lens can handle this — but the retraction logic must be implemented per-lens. This is implementation complexity, not architectural complexity.

### Invariant Consistency Check

The proposed mechanism is consistent with all current invariants:

- **Invariant 34** (all writes through ingest): Lens writes go through `emit_inner()` via the enrichment loop, which is part of the ingest pipeline.
- **Invariant 40** (three independent extension axes): A lens is an enrichment, not a new axis. No amendment to Invariant 40 is needed.
- **Invariant 56** (lens output is public): First-class edges in the shared context are visible to all consumers by definition.
- **Invariant 57** (lens is classified as enrichment): The lens implements the `Enrichment` trait directly.
- **Invariant 58** (event cursors preserve library rule): Event cursors are orthogonal to lens storage — they enable pull-based query regardless of how lens output is stored.
- **Invariant 59** (provenance-scoped query composability): Lens edges carry contributions like any other edge, so provenance-scoped queries work without modification.

---

## References

- Xu, X., et al. (2024). MV4PG: Materialized Views for Property Graphs. *arXiv:2411.18847*.
- Neo4j Graph Data Science: Projecting graphs. *Neo4j Documentation, v2.27*.
- Apache TinkerPop: SubgraphStrategy, PartitionStrategy. *TinkerPop Documentation, v3.8.0*.
- Rasmussen, P., et al. (2025). Zep: A Temporal Knowledge Graph Architecture for Agent Memory. *arXiv:2501.13956*.
- Plexus domain model (2026-03-25): Invariants 56–59 (lens and query surface rules).
- Plexus ADR-003: Contribution tracking and scale normalization.
- Plexus ADR-029: Enrichment loop ownership by IngestPipeline.
