# Research Log: Plexus Query Surface Design

## Context
Plexus is a knowledge graph engine with provenance tracking. Consumer applications (like Trellis, Carrel) use declarative adapters to define ingestion strategies and enrichments. Multiple consumers can share a context, creating cross-domain graphs. The research cycle investigates what the query integration surface should look like — with a focus on keeping consumers lightweight by leveraging write-time intelligence (enrichments) to reduce query-time complexity.

## Core Research Question
If declarative enrichments encode query-relevant structure into the graph at ingestion time, what is the relationship between enrichment design and query surface design — and can Plexus's query API be thin precisely because enrichments do the heavy lifting? How does this work across multiple consumers sharing a context?

## Open Sub-Question (from scoping)
Does the adapter/enrichment/transport trichotomy need a fourth axis for "query structure" or "projection" — or do enrichments subsume that concern?

---

## Question 1: Write-time enrichment, graph projections, and lightweight query patterns in knowledge graph systems
**Method:** Web search (8 searches + 2 deep fetches)

### Findings

#### 1. The Write-Heavy / Query-Light Pattern is Well-Established

The most directly relevant system is **Zep/Graphiti** — a temporal knowledge graph designed for AI agents. Its architecture validates the core hypothesis: push intelligence to ingestion, keep queries mechanical.

Graphiti's write path does entity extraction, resolution, temporal annotation, embedding computation, and community detection. Its query path uses only three primitives — cosine similarity search, BM25 full-text search, and breadth-first graph traversal — combined via reranking (Reciprocal Rank Fusion, Maximal Marginal Relevance). No LLM calls are required at query time. The graph is pre-enriched enough that mechanical search suffices.

**Key insight for Plexus:** Graphiti proves that if enrichments encode sufficient structure at write time, the query surface can be a small set of search + traversal primitives with configurable reranking. This aligns with Plexus's existing architecture — enrichments run at pipeline level after adapter emissions.

*Source: [Zep: A Temporal Knowledge Graph Architecture for Agent Memory](https://arxiv.org/html/2501.13956v1)*

#### 2. Graph Projections as Consumer-Specific Views

Neo4j's Graph Data Science library introduces the concept of **graph projections** — named, in-memory subgraphs created from the stored graph via declarative configuration. Key properties:

- A projection specifies which node labels and relationship types to include
- Different consumers can create different projections of the same underlying data
- Projections can include "virtual relationships" that don't exist in the physical store
- Projections are named and stored in a "graph catalog" for reuse

This maps closely to the multi-consumer scenario (Trellis + Carrel sharing a context). Each consumer could define a **projection** over the shared graph — selecting the node types, edge types, and traversal orientations relevant to its domain.

**Key insight for Plexus:** The "projection" concept is distinct from enrichment. An enrichment *adds* structure to the graph. A projection *selects* structure for a consumer's perspective. These are complementary, not competing.

*Source: [Projecting graphs — Neo4j GDS](https://neo4j.com/docs/graph-data-science/current/common-usage/projecting-graphs/)*

#### 3. Materialized Views for Property Graphs (MV4PG)

Recent research (November 2024) introduces formal materialized views for property graphs. Key contribution: views are defined declaratively using GQL syntax, maintained incrementally via templates, and achieve 28-100x query speedup.

The MV4PG approach treats a materialized view as a pre-computed subgraph derived from a declarative view definition. When the underlying graph changes, the view is incrementally updated rather than recomputed.

**Key insight for Plexus:** Enrichments in Plexus are already a form of materialized view — they compute derived edges (co-occurrence, similarity) and persist them. The missing concept is the *declarative view definition* that specifies which parts of the enriched graph a consumer cares about.

*Source: [MV4PG: Materialized Views for Property Graphs](https://arxiv.org/html/2411.18847v1)*

#### 4. Multi-Consumer / Multi-Domain Knowledge Graphs

Cross-domain knowledge graph federation uses ontology alignment to bridge separately-constructed graphs. Key patterns:
- Schema-level fusion aligns class/property signatures across domains
- Instance-level fusion resolves entities across sources
- Hybrid frameworks integrate both

In Plexus's model, multiple consumers sharing a context already get implicit cross-domain linkage — enrichments like co-occurrence and embedding similarity create edges between content from different sources. The question is how each consumer *navigates* those cross-domain connections according to its own concerns.

*Sources: [Domain-specific Knowledge Graphs: A survey](https://arxiv.org/pdf/2011.00235), [Knowledge Graph Federation](https://link.springer.com/chapter/10.1007/978-981-16-0479-9_6)*

#### 5. The "Relationships Are Pre-stored, Not Computed" Principle

A recurring theme across all sources: in well-designed knowledge graphs, relationships are explicit edges, not computed at query time via joins. Following a relationship has constant cost regardless of graph size. This is the fundamental argument for enrichment-at-ingestion — the LLM does expensive semantic work once during write, and the result is a first-class edge that traversal can follow cheaply.

### Implications

**For the query surface:**
- The query API can be thin if enrichments are rich. Graphiti's three-primitive model (semantic search, keyword search, graph traversal + reranking) is evidence that a small number of well-designed query operations suffice when the graph carries pre-computed structure.
- Plexus's existing five query primitives (`find_nodes`, `traverse`, `find_path`, `shared_concepts`, `evidence_trail`) may already be close to sufficient — the gap is in MCP exposure and in combining them with consumer-specific perspectives.

**For the adapter/enrichment model:**
- "Projection" or "view" appears to be a distinct concern from enrichment. An enrichment adds edges. A projection selects which nodes, edges, and edge types a consumer cares about — a lens over the enriched graph.
- This could be a declarative concept: part of the adapter spec could define not just ingestion strategies and enrichments, but also **named projections** — "when Trellis queries this context, it sees these node types and edge types; when Carrel queries, it sees these."

**For the open sub-question (adapter vs enrichment vs new axis):**
- The evidence suggests "projection" is indeed a third read-side concern, distinct from both "enrichment" (write-side graph augmentation) and "query" (parameterized operations). The three layers would be:
  1. **Enrichment** — adds derived structure at write time
  2. **Projection** — selects a consumer-specific subgraph view
  3. **Query** — parameterized operations over the projected view

**For cross-consumer shared contexts:**
- Each consumer's enrichments contribute to the shared graph structure
- Each consumer's projections determine what portion of that shared structure is relevant to its queries
- Cross-domain discovery happens when Consumer A's query, through its projection, reaches edges contributed by Consumer B's enrichments

---

## Question 2: What is unique about Plexus compared to these systems, and what must its query surface reckon with that they don't?
**Method:** Codebase analysis + web search

### Findings

#### 1. Plexus's Differentiators

Five architectural properties distinguish Plexus from the comparison systems (Graphiti/Zep, Neo4j GDS, general KG platforms):

**Multi-consumer convergence without coordination.** Graphiti is session-scoped, Zep is user-scoped, Neo4j GDS operates on pre-loaded projections. None support multiple independent consumers co-inhabiting the same graph namespace. Plexus does — deterministic concept IDs (`concept:{lowercase_tag}`) mean two consumers tagging content "travel" converge on the same node via upsert, without coordinating. Cross-modal agreement is itself a signal (it strengthens raw weight through contribution tracking).

**Contribution-tracking provenance as a graph dimension.** Graphiti tracks which "episodes" produced a fact. Neo4j GDS has no provenance model. Plexus treats provenance as a structural obligation (Invariant 7: all knowledge carries semantic content + provenance). Marks and chains are graph nodes in a PROVENANCE dimension, connected to semantic nodes via cross-dimensional edges. Provenance participates in traversal, not just logging.

**Evidence diversity as a derived query-time property.** No comparison system computes "how corroborated is this edge?" at query time. Plexus defines evidence diversity as: count distinct adapter IDs, source types, and contexts that contributed to an edge. "Four different kinds of evidence are more trustworthy than a hundred of the same kind." This is a query concern that has no analog in Graphiti or Neo4j.

**Declarative consumer extension.** Graphiti has a fixed extraction pipeline. Neo4j GDS requires Cypher. Plexus consumers define ingestion logic, enrichment parameters, and (potentially) projections in YAML without touching the engine.

**Independent contribution accumulation across extraction phases.** Plexus's three-phase pipeline (registration → structural → semantic) means the same concept discovered by structural analysis and by LLM extraction produces two independent contribution slots. The graph strengthens through cross-phase evidence without explicit merge logic.

#### 2. What Plexus's Query Surface Must Handle That Others Don't

These differentiators create four query requirements that don't exist in comparison systems:

**Provenance-scoped queries.** A consumer needs to ask: "show me connections, but only those supported by evidence from *these* sources" or "what did Carrel contribute that Trellis hasn't seen?" The query surface must support filtering by provenance dimensions — adapter ID, source type, chain, context of origin. This is not a standard graph query filter. Graphiti's search-and-rerank model has no provenance filtering at all.

**Evidence diversity as a ranking signal.** "Which connections are most corroborated?" requires the query system to compute contribution counts across adapter IDs and source types, and use that as a ranking/filtering criterion. This is a Plexus-specific ranking dimension that doesn't map to semantic similarity, keyword relevance, or graph distance. None of the comparison systems have an equivalent.

**Cross-consumer discovery queries.** "What did Carrel's research contribute to Trellis's writing context?" is a query about the intersection of provenance and graph structure — edges in a shared context where the contributing adapters belong to different consumers. This requires combining graph traversal with provenance-dimension filtering, a query shape unique to the multi-consumer model.

**Projection-aware queries in a shared graph.** If Trellis and Carrel define different projections over a shared context, the query surface must respect those projections — Trellis's query sees the graph through Trellis's lens (relevant node types, edge types). But it also needs to optionally "look beyond" its projection to discover cross-domain connections. This tension between projection-scoped querying and serendipitous cross-boundary discovery is architecturally novel.

#### 3. The Provenance-Aware Querying Literature

Academic research on provenance-aware knowledge graphs (PaCE model, contextualized KGs) confirms this is a recognized but under-served area. The PaCE approach tracks multiple sources stating the same claim and infers confidence from corroboration — similar to Plexus's evidence diversity concept. However, these systems are primarily concerned with data quality and trust, not with multi-consumer cross-domain discovery.

*Source: [Provenance-Aware Knowledge Representation: A Survey](https://link.springer.com/article/10.1007/s41019-020-00118-0)*

### Implications

The query surface Plexus needs is not a generic graph query API with a search layer on top (the Graphiti model). It requires:

1. **Standard graph primitives** — find, traverse, path, shared concepts (these exist)
2. **Provenance-dimensional filtering** — scope any query by adapter, source type, chain, or contributing consumer
3. **Evidence diversity ranking** — rank results by corroboration breadth, not just weight or similarity
4. **Projection-scoped querying** — consumer-specific views over shared graphs, with an escape hatch for cross-boundary discovery
5. **Composability** — these dimensions must combine (e.g., "traverse through Trellis's projection, ranked by evidence diversity, filtered to edges with provenance from at least two sources")

---

## Question 3: Federated contexts and cross-person discovery
**Method:** Conversation with user (design insight, not web research)

### Findings

The multi-consumer model extends beyond apps sharing a context to **federated contexts across users**. The user described a "Sketchbin" scenario:

- User A publishes content in their Sketchbin context
- User B has their own Sketchbin context
- These contexts are federated — cross-pollinated through Plexus
- Peers discover each other's work because enrichments have created graph structure that bridges their independently-contributed content
- Discovery is emergent from graph structure, not from an explicit social/recommendation system

This is a third level of the discovery hierarchy:

1. **Within a context, within a consumer** — Trellis discovers connections between its own writing fragments (intra-domain)
2. **Within a context, across consumers** — Trellis discovers connections to Carrel's research (cross-domain)
3. **Across contexts** — Sketchbin discovers connections between User A's and User B's work (cross-person / federated)

The `shared_concepts` query primitive (already implemented but not exposed via MCP) is the federation query: it takes two context IDs and returns common node IDs. This is the algorithmic basis for cross-context discovery. Combined with provenance filtering and evidence diversity ranking, it enables queries like "what concepts do these two people's work share, and which connections are most corroborated?"

### Implications

- `shared_concepts` moves from "nice to have" to **architecturally central** — it's the primitive that enables federated discovery
- The projection concept applies across contexts too: a federated Sketchbin query might project over both contexts but only through the Sketchbin consumer's lens
- Evidence diversity across contexts becomes a trust signal: a concept that appears in both User A's and User B's independently-contributed content is more significant than one that appears only in one
- The query surface must handle cross-context operations as a first-class concern, not just within-context queries with an optional context parameter
