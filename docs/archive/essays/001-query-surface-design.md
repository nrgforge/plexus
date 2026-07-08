# The Query Surface as a Consequence of Write-Time Intelligence
*2026-03-23*

## Abstract

This essay investigates what Plexus's query integration surface should look like for consumer applications that share contexts across domains and users. Through web research into knowledge graph query patterns (Graphiti/Zep, Neo4j GDS, MV4PG) and analysis of Plexus's architectural differentiators, the investigation found that the write-heavy/query-light pattern is well-established — systems that push semantic intelligence to ingestion can offer thin query surfaces. However, Plexus's unique combination of multi-consumer convergence, contribution-tracking provenance, and evidence diversity creates query requirements that no comparison system addresses: provenance-scoped queries, evidence diversity ranking, projection-aware querying over shared graphs, and cross-context federated discovery. The essay proposes a layered query model — enrichment (write-side), projection (read-side view), query (parameterized operation) — and identifies the central design tension: projection-scoped querying versus serendipitous cross-boundary discovery.

---

## The Write-Heavy / Query-Light Hypothesis

A recurring pattern in knowledge graph systems designed for AI agents is the deliberate asymmetry between write-time and query-time complexity. The most instructive example is Zep's Graphiti engine, a temporal knowledge graph architecture for agent memory. Graphiti performs entity extraction, resolution, temporal annotation, embedding computation, and community detection during ingestion. At query time, it offers only three primitives — cosine similarity search, BM25 full-text search, and breadth-first graph traversal — combined through reranking strategies such as Reciprocal Rank Fusion and Maximal Marginal Relevance. No LLM calls are required during retrieval in the default configuration, though optional cross-encoder reranking is available.

This architecture validates a hypothesis central to Plexus's design: if enrichments encode sufficient structure at write time, the query surface can be a small set of search and traversal primitives with configurable ranking. The intelligence is in the graph, not in the query. A caveat: Graphiti's write-time pipeline is more comprehensive than Plexus's current enrichment set — it includes community detection, entity resolution, and temporal annotation that Plexus does not yet implement. The principle transfers; the degree of write-time intelligence required to make queries truly "thin" is an empirical question that implementation will test.

Plexus already follows this pattern. Declarative adapters define ingestion strategies and enrichments in YAML. Core enrichments — co-occurrence detection, embedding similarity, discovery gap analysis, temporal proximity — run reactively after every emission. The result is a graph pre-loaded with derived structure: `may_be_related` edges from co-occurrence, `similar_to` edges from embedding similarity, discovery gap signals from structural absence. Traversal through this enriched graph follows pre-computed paths rather than computing relationships at query time.

The research question, then, is not whether Plexus needs a sophisticated query language. It is what the thin query surface must contain to be sufficient — and where Plexus's requirements diverge from systems like Graphiti that validated the general pattern.

## Where Plexus Diverges

Five architectural properties create query requirements that do not exist in comparison systems.

### Multi-Consumer Convergence Without Coordination

Graphiti is session-scoped. Zep is user-scoped. Neo4j GDS operates on pre-loaded projections of an existing graph. None support multiple independent consumers co-inhabiting the same graph namespace without explicit coordination.

Plexus does. Deterministic concept IDs (Invariant 19: `concept:{lowercase_tag}`) mean two consumers tagging content with the same term converge on the same node via upsert. The consumers never coordinate. Cross-modal agreement — independent adapters producing the same concept label — is itself a signal that strengthens raw weight through the contribution tracking model (Invariant 8). When Trellis tags a writing fragment "epistemology" and Carrel tags a research paper "epistemology," both contribute to the same concept node. The convergence is mechanical, and the graph is richer for it.

This creates a query requirement: a consumer must be able to ask "show me connections in the shared context, but scoped to what I care about." The graph contains everything from every consumer. The query surface must support perspective.

### Contribution-Tracking Provenance as a Graph Dimension

Graphiti tracks which episodes produced a fact. This is lineage — useful for debugging and audit. Plexus treats provenance differently: marks and chains are nodes in a PROVENANCE dimension, connected to semantic nodes via cross-dimensional edges. Provenance participates in graph traversal, not just logging (Invariant 7).

This means provenance is queryable in the same way semantic content is queryable. A consumer can traverse from a concept node, through its provenance edges, to the marks and chains that evidence it, and from there to the source material. The query "what evidence supports this connection?" is a graph traversal, not a metadata lookup.

The implication for the query surface: any query primitive (find, traverse, path) must be composable with provenance-dimensional filtering — "traverse, but only through edges evidenced by sources of type X" or "find nodes, but only those with provenance from adapter Y."

### Evidence Diversity as a Derived Signal

No comparison system computes "how corroborated is this edge?" at query time. Plexus defines evidence diversity as the count of distinct adapter IDs, source types, and contexts that contributed to an edge. The domain model (`docs/domain-model.md`, Evidence diversity concept) states the principle directly: "Four different kinds of evidence are more trustworthy than a hundred of the same kind."

Evidence diversity is not a stored field — it is derived at query time from provenance entries. This is a ranking dimension that has no analog in semantic similarity, keyword relevance, or graph distance. A connection discovered independently by structural analysis, semantic extraction, and co-occurrence enrichment is more significant than one discovered by a single adapter, regardless of raw weight.

The query surface must support evidence diversity as a ranking and filtering criterion: "which connections are most corroborated?" and "show only connections with evidence from at least N distinct sources."

### Independent Contribution Accumulation Across Extraction Phases

Plexus's three-phase extraction pipeline (registration, structural analysis, semantic extraction) assigns distinct adapter IDs to each phase (Invariant 45). The same concept discovered structurally (via link extraction in a markdown heading) and semantically (via LLM-based entity recognition) produces two independent contribution slots on the same edge. The graph strengthens automatically through cross-phase evidence diversity without explicit merge logic.

This architectural property means the query surface inherits a subtle requirement: evidence diversity queries must distinguish between "multiple phases of the same adapter pipeline" and "multiple independent consumers." Both increase corroboration, but they represent different kinds of evidence.

### Declarative Consumer Extension

Graphiti has a fixed extraction pipeline. Neo4j GDS requires Cypher for graph manipulation. Plexus consumers define ingestion logic, enrichment parameters, and graph mapping in YAML without touching the engine. The declarative adapter spec (`DeclarativeSpec`) allows a consumer to declare node types, edge types, relationship semantics, enrichment activation, and provenance structure — all in a configuration file the engine interprets at runtime.

This extensibility creates a natural question: if a consumer can declaratively define how data enters the graph and how the graph is enriched, could it also declaratively define how it *views* the graph? The analogy is suggestive but not inevitable — write-side declaration and read-side declaration solve different problems and may warrant different mechanisms. Still, the pattern of consumer-defined configuration interpreted by the engine is worth exploring for the read side.

## The Projection Layer

The research surfaced a concept from Neo4j's Graph Data Science library that maps directly to Plexus's multi-consumer scenario: **graph projections**. A projection is a named, declarative specification of which node types, edge types, and properties constitute a consumer's view of the graph. Different consumers can create different projections of the same underlying data.

In Neo4j GDS, projections are in-memory subgraphs created from the stored graph. In the MV4PG research (Xu et al., 2024), materialized views for property graphs are defined declaratively using GQL syntax and maintained incrementally. The MV4PG approach involves delta-based view maintenance — a mechanism Plexus is not proposing to replicate. What transfers is the concept: a declarative view definition that selects from the stored graph without modifying it. Both systems treat the projection as a read-side concern.

Plexus's domain model already contains a related concept: the **meta-context**, defined as "a read-only virtual view that unions nodes and edges from multiple constituent contexts at query time." Meta-contexts are pure composition — no data stored, no enrichment possible. This is close to a projection but scoped to context union rather than subgraph selection.

The research suggests that projection is a distinct concern from enrichment, and potentially a new axis in the Plexus extension model:

- **Enrichment** adds derived structure to the graph at write time
- **Projection** selects a consumer-specific subgraph view at read time
- **Query** operates over the projected view with parameterized primitives

This does introduce a tension with Invariant 40, which defines three independent extension axes: adapters (domain), enrichments (graph intelligence), and transports (protocol). A projection axis would be a fourth. Whether this is a genuine new axis or a configuration concern within the existing model — perhaps part of the declarative adapter spec — is a design decision for the DECIDE phase. The current invariant does not preclude it; it simply does not account for it.

## Three Levels of Discovery

The research and user conversations surfaced a discovery hierarchy that the query surface must support:

**Level 1: Intra-domain discovery.** A single consumer discovers connections within its own data. Trellis finds thematic links between writing fragments. This is the simplest case — standard graph traversal through enrichment-derived edges, scoped to one consumer's contributions.

**Level 2: Cross-domain discovery.** Multiple consumers share a context, and connections emerge between their domains. Trellis discovers that a writing fragment connects to a research paper ingested by Carrel. This happens because both consumers' enrichments have laid down structure in the shared graph — co-occurrence edges, similarity edges — that bridge their independently-contributed content.


**Level 3: Cross-person / federated discovery.** Users share or federate contexts, and connections emerge between their independently-contributed content. User A's Sketchbin publishes content that connects to User B's Sketchbin content through shared concepts and enrichment-derived edges. Discovery is emergent from graph structure, not from an explicit social or recommendation system.

The `shared_concepts` query primitive — which takes two context IDs and returns common node IDs — is the starting point for Level 3. It already exists in Plexus's query module but is not exposed via MCP. In its current form, `shared_concepts` returns only node IDs — it does not include edges, provenance, or weights. Building a full federated discovery query ("what concepts do these two contexts share, and which shared connections are most corroborated?") would require composing `shared_concepts` with provenance-dimensional filtering and evidence diversity ranking — capabilities that do not yet exist. The primitive is the seed; the composition is new work.

Each level adds a dimension to the query:
- Level 1 needs graph primitives + normalization
- Level 2 adds provenance-scoped filtering (which consumer contributed what)
- Level 3 adds cross-context operations + the projection concept (each consumer's lens over federated data)

## The Central Tension: Scope Versus Serendipity

The most architecturally novel aspect of Plexus's query surface is the tension between projection-scoped querying and serendipitous cross-boundary discovery.

A consumer wants to see its own domain clearly. Trellis queries about writing; it does not want to wade through Carrel's raw research metadata. A projection scopes the view. But the entire value proposition of a shared context is that connections emerge *between* domains. If the projection is too restrictive, the consumer never discovers that its writing fragment connects to a research paper. If the projection is too permissive, the consumer drowns in irrelevant cross-domain noise.

The resolution likely involves a two-mode query model:

1. **Scoped mode** — query operates within the consumer's projection. Results contain only nodes and edges matching the projection's type filters. This is the default for routine queries.
2. **Discovery mode** — query operates beyond the projection boundary, surfacing cross-domain connections. Results include nodes and edges outside the projection, ranked by evidence diversity and connection strength to the consumer's domain. This is the mode that surfaces latent discoveries.

The escape hatch from scope to discovery is the query surface's most important design problem. It must be easy enough that consumers use it, selective enough that results are meaningful, and lightweight enough that it does not require an LLM at query time.

## What the Query Surface Must Contain

Synthesizing across the research, the minimal query surface for Plexus requires five capabilities:

**1. Standard graph primitives.** `find_nodes`, `traverse`, `find_path`, `shared_concepts`, `evidence_trail` — these already exist in the query module and on `PlexusApi`. The gap is purely in MCP exposure.

**2. Provenance-dimensional filtering.** Any query primitive must accept optional filters that scope results by provenance dimensions: adapter ID, source type, chain, contributing consumer. "Traverse from this node, but only follow edges contributed by Carrel's adapter" is a first-class query shape.

**3. Evidence diversity ranking.** Results must be rankable by corroboration breadth — count of distinct contributing adapters, source types, or contexts. This is a Plexus-specific ranking dimension alongside raw weight and normalized weight.

**4. Projection-scoped views.** A consumer must be able to define (declaratively or at query time) which node types and edge types it cares about. Queries operate within the projection by default.

**5. Cross-boundary discovery.** A consumer must be able to step outside its projection to discover connections that bridge domains or contexts. This is the discovery mode described above — the query surface's most distinctive requirement.

These five capabilities compose. The most powerful query shape is: "traverse from this node, through my projection, ranked by evidence diversity, with an option to expand beyond my projection for discovery." Each dimension is optional — a consumer that does not define a projection gets the full graph; a consumer that does not request evidence diversity ranking gets default weight-based ordering.

## Invariant Tensions

Two existing invariants bear on the proposed model:

**Invariant 28** states that `list_tags(context_id)` is scoped to a single context with "no cross-context tag aggregation at the API layer." The federated discovery model implies cross-context queries as a first-class concern. While `shared_concepts` already performs cross-context operations, elevating cross-context discovery to a primary use case is a substantive expansion of the query surface's design intent, not merely a clarifying amendment to Invariant 28. This warrants explicit treatment in the DECIDE phase — the invariant may need revision, not just reinterpretation.

**Invariant 40** defines three independent extension axes. The proposed projection layer is a potential fourth axis. Whether this genuinely requires amending Invariant 40 or whether projections can be modeled as a configuration concern within the existing declarative adapter spec is an open design question for the DECIDE phase.

## References

- Zep/Graphiti temporal knowledge graph architecture: [Zep: A Temporal Knowledge Graph Architecture for Agent Memory](https://arxiv.org/abs/2501.13956) (Rasmussen et al., 2025)
- Neo4j Graph Data Science projections: [Projecting graphs — Neo4j GDS](https://neo4j.com/docs/graph-data-science/current/common-usage/projecting-graphs/)
- MV4PG materialized views for property graphs: [MV4PG: Materialized Views for Property Graphs](https://arxiv.org/abs/2411.18847) (Xu et al., 2024)
- Provenance-aware knowledge representation survey: [Provenance-Aware Knowledge Representation: A Survey of Data Models and Contextualized Knowledge Graphs](https://link.springer.com/article/10.1007/s41019-020-00118-0) (Sikos and Philp, 2020)
- Domain-specific knowledge graphs survey: [Domain-specific Knowledge Graphs: A survey](https://arxiv.org/abs/2011.00235) (Abu-Salih, 2021)
- Knowledge graph federation: [Towards Knowledge Graphs Federations: Issues and Technologies](https://link.springer.com/chapter/10.1007/978-981-16-0479-9_6) (Zhao, 2021)
