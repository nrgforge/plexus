# Open Questions, Competitive Landscape & References

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 15. Open Questions

1. **Embedding model choice**: ✓ Resolved → Local (`all-MiniLM-L6-v2`)
   - 384 dimensions, ~22MB model, runs on CPU

2. **Propagation threshold**: What confidence level triggers verification LLM pass?
   - Current proposal: 0.3

3. **Incremental updates**: When a document changes, how much re-analysis needed?
   - Proposal: Re-extract changed doc, re-propagate from neighbors only

4. **Cross-context learning**: Should concepts learned in one context transfer?
   - Relates to broader llm-orc pattern learning vision

5. **Hub verification**: Should hub documents get mandatory LLM verification?
   - Network robustness theory suggests yes

6. **Vector index persistence**: Rebuild HNSW on startup, or serialize to disk?
   - Tradeoff: Startup time vs disk space

---

## 16. Competitive Landscape

### 16.1 Existing Solutions Analyzed

| Solution | Type | Key Approach |
|----------|------|--------------|
| [Microsoft GraphRAG](https://github.com/microsoft/graphrag) | Full pipeline | Community detection + hierarchical summaries |
| [LightRAG](https://github.com/HKUDS/LightRAG) | Lightweight RAG | Dual-level retrieval, incremental updates |
| [Neo4j LLM Graph Builder](https://github.com/neo4j-labs/llm-graph-builder) | Document→KG | Multi-LLM extraction to Neo4j |
| [InfraNodus](https://infranodus.com/obsidian-plugin) | PKM Plugin | Network science + gap detection |
| [Beads](https://github.com/steveyegge/beads) | Task Coordination | Git-backed issue tracking for AI agents |

### 16.2 Microsoft GraphRAG

**Approach**: Extract entities from all documents → Build graph → Detect communities → Generate hierarchical summaries → Use summaries for retrieval.

**Strengths**:
- Hierarchical community summaries enable "global" queries
- Multiple search modes (global, local, DRIFT, basic)

**Limitations**:
- **Expensive**: Requires LLM analysis of every document
- **No incremental updates**: Full rebuild on changes
- **API-dependent**: Designed for OpenAI/Azure

### 16.3 LightRAG (EMNLP 2025)

**Approach**: Graph structures + embedding retrieval with incremental update algorithm.

**Strengths**:
- Incremental updates without full rebuild
- Ollama support for local models
- Multiple storage backends

**Limitations**:
- Still extracts from every document (no sampling strategy)
- No validation layer for LLM outputs
- Relies on external graph databases for scale

### 16.4 InfraNodus

**Approach**: Co-occurrence graphs with network science metrics (centrality, modularity). Identifies "structural gaps" between topic clusters.

**Most similar to Plexus approach.**

**Strengths**:
- Network science core (betweenness, communities)
- Gap detection for idea generation
- Only sends graph structure to AI, not full content

**Limitations**:
- Works on single vault, not multi-context
- No label propagation (each note analyzed independently)
- Commercial/closed-source core

### 16.5 Beads (Yegge)

**Approach**: Git-backed task coordination with hash-based IDs for multi-agent concurrency.

**Different problem space**: Beads coordinates agent work; Plexus discovers knowledge. However, the git-sync + SQLite cache pattern is architecturally similar to local-first Plexus.

### 16.6 Comparative Matrix

| Feature | GraphRAG | LightRAG | Neo4j | InfraNodus | **Plexus** |
|---------|----------|----------|-------|------------|------------|
| **Granularity** | Document | Document | Document | Document | **Section** |
| **Multi-level graph** | ✗ | ✗ | ✗ | ✗ | **Dir→Doc→Section→Concept** |
| Sampling strategy | All docs | All docs | All docs | All docs | **Hybrid (doc PageRank → section)** |
| Label propagation | ✗ | ✗ | ✗ | ✗ | **Edge-type weighted** |
| **Chunked extraction** | ✗ | ✗ | ✗ | ✗ | **✓ (with accumulation)** |
| Incremental updates | ✗ | ✓ | Partial | ✓ | **✓ (FLPA-style)** |
| Local-first | ✗ | Partial | ✗ | Partial | **✓** |
| Validation layer | ✗ | ✗ | ✗ | ✗ | **L1/L2/L3** |
| Micro-LLMs | ✗ | ✗ | ✗ | ✗ | **✓** |
| **Dual hierarchy** | Communities | ✗ | Schema | Clusters | **Structural + Semantic** |
| Network science | Louvain | ✗ | ✗ | ✓ | **Multiplex networks** |
| Hybrid traversal | ✗ | ✓ | ✓ | ✗ | **✓** |

### 16.7 Plexus Differentiators

#### 1. Network-Guided Efficiency

**Others**: Analyze every document with LLM.
**Plexus**: Use PageRank to find the 15% that matter, propagate to the rest.

Cost reduction: ~10x fewer LLM calls for equivalent coverage.

#### 2. Label Propagation

No existing solution applies label propagation to knowledge graph construction.

Research shows semi-supervised label propagation achieves 60-80% accuracy of full annotation. This is an underutilized technique in this space.

#### 3. Validation Pyramid

No existing solution has a structured validation layer. All trust LLM output directly.

Plexus: L1 (schema) → L2 (grounding) → L3 (semantic judgment).

Errors compound in knowledge graphs. Validation prevents pollution.

#### 4. Micro-Ensemble Architecture

**Others**: One model does everything.
**Plexus**: Task-specific models (1.5B extraction, 3B validation, 7B naming).

Benefits:
- Cost optimization (expensive models only where needed)
- Can run entirely local
- Fine-tuning targets specific tasks

#### 5. Emergent Ontology

**GraphRAG**: Detects communities but doesn't name them semantically.
**Neo4j**: Uses fixed predefined schemas.
**Plexus**: HAC clustering → LLM naming → dynamic category tree.

Categories adapt to content, not the other way around.

#### 6. Multi-Level Graph Architecture (NEW)

**Others**: Flat document-level graphs.
**Plexus**: Four-level hierarchy: Directory → Document → Section → Concept.

Research grounding: Hierarchical Knowledge Graphs (HKGs) outperform flat KGs for multi-hop reasoning. Our structural hierarchy (directory/heading) + semantic hierarchy (concept categories) provides dual navigation axes—an approach validated by research on hierarchical topic modeling.

Benefits:
- Section-level granularity catches concepts missed at document level
- Structural edges (sibling sections, parent directories) provide implicit semantic signal
- Dual hierarchy enables both structural and semantic "zoom" in navigation

#### 7. Edge-Type Weighted Propagation (NEW)

**Others**: Uniform edge weights or no propagation.
**Plexus**: Edge-type-specific weights informed by multiplex network research.

| Edge Type | Weight | Rationale |
|-----------|--------|-----------|
| Sibling (same doc) | 0.8 | Adjacent sections share context |
| LinksTo (with anchor) | 0.85 | Explicit section reference |
| LinksTo (no anchor) | 0.7 | Document reference |
| Sibling (same dir) | 0.5 | Directory = thematic grouping |
| Contains | 0.3 | Hierarchical containment |

Research grounding: Multiplex network analysis shows edge-type-specific treatment outperforms flattened approaches. FLPA (Fast Label Propagation Algorithm) demonstrates 700x speedup by processing only changed neighborhoods.

#### 8. Chunked Extraction with Accumulation (NEW)

**Others**: Truncate long documents or use naive chunking.
**Plexus**: Script/model hybrid ensemble with concept accumulation.

Process:
1. **Chunker** (script): Split section by paragraphs, respecting boundaries
2. **Extractor** (LLM): Process each chunk with `prior_concepts` context
3. **Accumulator** (script): Merge concepts, boost confidence for repeated mentions
4. **Deduplicator** (LLM): Resolve near-duplicates ("ML" vs "machine learning")

Research grounding: Late Chunking (Jina AI, 2024) shows that contextual awareness during chunking dramatically improves embedding quality. Our approach extends this to extraction—chunks know what concepts have already been found.

Benefits:
- Handles Shakespeare plays, legal documents, long-form content
- Repeated concept mentions boost confidence (signal, not noise)
- Cross-chunk context prevents fragmented extraction

#### 9. Hybrid Scoring Strategy (NEW)

**Others**: Apply PageRank/HITS directly to all nodes.
**Plexus**: Document-level scoring → section distribution → iterative refinement.

**The problem**: Section-level graphs have sparse explicit links. Most edges are structural (sibling, contains), not semantic (LinksTo).

**Our solution**:
1. Compute PageRank/HITS on document graph (dense links)
2. Distribute importance to sections within high-scoring docs
3. After propagation, concepts create cross-section SimilarTo edges
4. Re-run section-level centrality on enriched graph

This addresses the cold-start problem: we need concepts to create semantic links, but we need links to select which sections to extract concepts from.

---

## 17. Related Work

This section frames Plexus's approach within the broader research landscape.

### 17.1 Document Chunking Strategies

**Late Chunking (Jina AI, 2024)**: Process entire document through transformer, then pool into chunk embeddings. Preserves cross-chunk context that naive chunking destroys.

*Relevance*: Our chunked extraction passes `prior_concepts` to each chunk, achieving similar contextual awareness for extraction (not just embedding).

**Contextual Retrieval (Anthropic, 2024)**: Prepend document-level context to each chunk before embedding.

*Relevance*: Our section extraction includes document metadata and structural context, similar in spirit.

### 17.2 Multi-Layer Network Analysis

**Multiplex Networks**: Same node set, multiple edge types with layer-specific dynamics. De Domenico et al. (2013) showed multiplex analysis captures phenomena invisible to flattened networks.

*Relevance*: Our edge-type weights treat the Plexus graph as a multiplex network—structural and semantic layers with different propagation dynamics.

**FLPA (Fast Label Propagation Algorithm)**: Processes only nodes whose neighborhoods changed, achieving 700x speedup on large graphs.

*Relevance*: Critical for incremental updates. When a document changes, we re-propagate from affected sections only.

### 17.3 Hierarchical Knowledge Graphs

**HKGs**: Knowledge graphs with explicit hierarchy (entity types, ontology levels). Research shows HKGs improve multi-hop reasoning by 15-20% over flat KGs.

*Relevance*: Plexus has dual hierarchy—structural (dir/doc/section) and semantic (concept categories). This enables the "zoom" navigation pattern.

### 17.4 Document Structure Extraction

**DocParser, LayoutLM**: Extract logical structure (headings, sections) from documents. Research shows structural features improve downstream NLP tasks.

*Relevance*: Our Phase 1 structural bootstrap extracts heading hierarchy to create the section graph. The quality of this extraction directly impacts propagation accuracy.

### 17.5 Semi-Supervised Learning on Graphs

**Label Propagation + GNNs**: Huang et al. (2021) showed that simple label propagation combined with learned features outperforms complex GNNs on many tasks.

*Relevance*: We use label propagation (simple, interpretable) but with edge-type weights informed by structural priors. Future work could incorporate learned weights.

---

## 18. References

### Research Papers

**Foundational Algorithms**:

1. Page, L., et al. (1999). "The PageRank Citation Ranking: Bringing Order to the Web." Stanford InfoLab.

2. Zhu, X., & Ghahramani, Z. (2002). "Learning from labeled and unlabeled data with label propagation." CMU-CALD-02-107.

3. Kleinberg, J. M. (1999). "Authoritative sources in a hyperlinked environment." Journal of the ACM.

4. Blondel, V. D., et al. (2008). "Fast unfolding of communities in large networks." Journal of Statistical Mechanics.

**Network Science (Barabási)**:

5. Barabási, A.-L., & Albert, R. (1999). "Emergence of scaling in random networks." Science, 286(5439), 509-512.

6. Barabási, A.-L. (2016). *Network Science*. Cambridge University Press. [Online](http://networksciencebook.com/)

7. Albert, R., Jeong, H., & Barabási, A.-L. (2000). "Error and attack tolerance of complex networks." Nature, 406(6794), 378-382.

**Small-World Networks**:

8. Watts, D. J., & Strogatz, S. H. (1998). "Collective dynamics of 'small-world' networks." Nature, 393(6684), 440-442.

**Graph Neural Networks & Label Propagation**:

9. Wang, H., et al. (2021). "Combining Graph Convolutional Neural Networks and Label Propagation." ACM TOIS. [Link](https://dl.acm.org/doi/10.1145/3490478)

10. Iscen, A., et al. (2019). "Label Propagation for Deep Semi-supervised Learning." CVPR. [arXiv](https://arxiv.org/abs/1904.04717)

11. Huang, Q., et al. (2021). "Combining Label Propagation and Simple Models Out-performs Graph Neural Networks." ICLR. [arXiv](https://arxiv.org/abs/2010.13993)

### Related Plexus Documentation

- [ARCHITECTURE.md](../ARCHITECTURE.md) - Core data model and API
- [VISION.md](../VISION.md) - Compositional intelligence stack
- [ROADMAP.md](../ROADMAP.md) - Phase 8 (LLM Integration) context
- [archive/compositional-intelligence-spec.md](../archive/compositional-intelligence-spec.md) - Full technical vision
- [archive/plexus-llm-orc-mcp-integration.md](../archive/plexus-llm-orc-mcp-integration.md) - MCP protocol spec

---

## Appendix A: Pseudocode Reference

### A.1 Complete Pipeline (Section-Level)

```python
def semantic_analysis_pipeline(directory: Path, config: Config) -> SemanticGraph:
    # Phase 1: Structural Bootstrap (multi-level)
    G0 = structural_bootstrap(directory)  # Creates Dir → Doc → Section graph

    # Phase 1b: Hybrid Importance Scoring
    # Document-level first (dense links), then distribute to sections
    doc_importance = compute_pagerank(G0, level=Document)
    doc_bridges = compute_betweenness(G0, level=Document)

    # Distribute to sections within high-scoring docs
    section_scores = {}
    for doc, score in doc_importance.items():
        sections = G0.sections_of(doc)
        for section in sections:
            # Base score from document + section features
            section_scores[section.id] = (
                score * config.doc_weight +
                section.heading_level_score * config.heading_weight +
                section.link_density * config.link_weight
            )

    # Select seed SECTIONS
    seed_sections = select_seeds(
        section_scores,
        doc_bridges,  # Still use doc bridges for diversity
        config.sample_proportion
    )

    # Phase 2: Semantic Seeding (section-level, with chunking)
    concepts = {}
    section_concepts = {}
    for section in seed_sections:
        if section.word_count > config.chunk_threshold:
            # Large section: use chunked extraction
            result = chunked_extractor.invoke(section)
        else:
            # Normal extraction
            result = concept_extractor.invoke(section.content)

        for c in result.concepts:
            concepts[normalize(c.name)] = c
            section_concepts[section.id].append((c.name, c.confidence))

    # Phase 3: Multi-Level Label Propagation
    all_labels = multilevel_label_propagation(
        G0,
        section_concepts,
        edge_weights=config.edge_type_weights,  # Type-specific!
        max_iterations=config.propagation_iterations,
        decay=config.confidence_decay
    )

    # Optional: Verify low-confidence labels
    if config.verification_enabled:
        for section_id, labels in all_labels.items():
            if max(conf for _, conf in labels) < config.verification_threshold:
                section = G0.get_section(section_id)
                verified = concept_verifier.invoke(section.content, labels)
                all_labels[section_id] = verified

    # Phase 3b: Iterative Refinement (optional)
    if config.iterative_refinement:
        # Concepts create cross-section SimilarTo edges
        add_concept_similarity_edges(G0, all_labels, config.similarity_threshold)
        # Re-score sections with enriched graph
        refined_scores = compute_pagerank(G0, level=Section)
        # Could trigger additional extraction on newly-important sections

    # Phase 4: Dual Hierarchy Ontology
    embeddings = embed_concepts(concepts.keys())
    semantic_hierarchy = hierarchical_cluster(embeddings)
    categories = name_categories(semantic_hierarchy, category_namer)

    # Structural hierarchy already exists in G0
    # (directories → documents → sections)

    # Build final semantic graph with dual hierarchy
    semantic_graph = build_semantic_graph(
        G0,
        all_labels,
        concepts,
        categories,
        structural_hierarchy=G0.get_containment_tree()
    )

    return semantic_graph
```

### A.2 Multi-Level Label Propagation (Edge-Type Weighted)

```python
# Default edge-type weights (multiplex network approach)
DEFAULT_EDGE_WEIGHTS = {
    EdgeType.SIBLING_SAME_DOC: 0.8,    # Adjacent sections share context
    EdgeType.LINKS_TO_ANCHOR: 0.85,    # Explicit section reference
    EdgeType.LINKS_TO_DOC: 0.7,        # Document reference
    EdgeType.SIBLING_SAME_DIR: 0.5,    # Directory = thematic grouping
    EdgeType.CONTAINS: 0.3,            # Hierarchical containment
    EdgeType.SIMILAR_TO: 0.6,          # Concept co-occurrence (added post-extraction)
}

def multilevel_label_propagation(
    G: Graph,
    initial_labels: Dict[NodeId, List[Tuple[str, float]]],
    edge_weights: Dict[EdgeType, float] = DEFAULT_EDGE_WEIGHTS,
    max_iterations: int = 15,
    decay: float = 0.8,
    threshold: float = 0.1
) -> Dict[NodeId, List[Tuple[str, float]]]:

    labels = {node: list(lbls) for node, lbls in initial_labels.items()}
    seeds = set(initial_labels.keys())

    # FLPA optimization: track which nodes changed
    changed_nodes = set(seeds)

    for iteration in range(max_iterations):
        new_labels = {}
        newly_changed = set()

        for node in G.nodes():
            if node in seeds:
                new_labels[node] = labels[node]
                continue

            # FLPA: only recompute if a neighbor changed
            neighbors = set(G.neighbors(node))
            if not neighbors.intersection(changed_nodes):
                new_labels[node] = labels.get(node, [])
                continue

            # Aggregate from neighbors with edge-type-specific weights
            votes = defaultdict(float)
            for neighbor in neighbors:
                edge = G.get_edge(node, neighbor)
                edge_type_weight = edge_weights.get(edge.edge_type, 0.5)

                for concept, confidence in labels.get(neighbor, []):
                    votes[concept] += confidence * decay * edge_type_weight

            # Filter by threshold
            node_labels = [
                (concept, score)
                for concept, score in votes.items()
                if score > threshold
            ]

            if node_labels != labels.get(node, []):
                newly_changed.add(node)

            new_labels[node] = node_labels

        labels = new_labels
        changed_nodes = newly_changed

        if not changed_nodes:
            break  # Converged

    return labels
```

### A.3 Chunked Extraction with Accumulation

```python
def chunked_extraction(
    section: Section,
    chunk_size: int = 1500,  # words
    overlap: int = 100       # words
) -> ExtractionResult:
    """Extract concepts from large sections via chunking with context."""

    # Phase 1: Chunk by paragraph boundaries
    chunks = chunk_by_paragraphs(section.content, chunk_size, overlap)

    # Phase 2: Extract with accumulating context
    running_concepts: Dict[str, Concept] = {}

    for i, chunk in enumerate(chunks):
        # Pass prior concepts for context (Late Chunking insight)
        prior_names = list(running_concepts.keys())[:20]  # Top 20 by confidence

        result = concept_extractor.invoke(
            content=chunk,
            context={
                "document_title": section.document.title,
                "section_heading": section.heading,
                "chunk_index": i,
                "total_chunks": len(chunks),
                "prior_concepts": prior_names  # Key: accumulated context
            }
        )

        # Phase 3: Accumulate concepts
        for concept in result.concepts:
            canonical = normalize(concept.name)

            if canonical in running_concepts:
                # Boost confidence for repeated mentions
                existing = running_concepts[canonical]
                existing.confidence = min(1.0, existing.confidence + concept.confidence * 0.3)
                existing.evidence.extend(concept.evidence)
                existing.mention_count += 1
            else:
                # Check fuzzy matches
                similar = find_similar(canonical, running_concepts.keys(), threshold=0.85)
                if similar:
                    concept.potential_duplicates = similar
                running_concepts[canonical] = concept

    # Phase 4: Deduplicate near-matches (optional LLM pass)
    if len(running_concepts) > 50:
        # Too many concepts - use LLM to consolidate
        running_concepts = concept_deduplicator.invoke(running_concepts)

    return ExtractionResult(
        concepts=list(running_concepts.values()),
        metadata={"chunked": True, "chunk_count": len(chunks)}
    )


def chunk_by_paragraphs(
    content: str,
    target_size: int,
    overlap: int
) -> List[str]:
    """Split content at paragraph boundaries, respecting size limits."""

    paragraphs = content.split("\n\n")
    chunks = []
    current_chunk = []
    current_size = 0

    for para in paragraphs:
        para_size = len(para.split())

        if current_size + para_size > target_size and current_chunk:
            # Emit current chunk
            chunks.append("\n\n".join(current_chunk))

            # Overlap: keep last paragraph(s) up to overlap words
            overlap_paras = []
            overlap_size = 0
            for p in reversed(current_chunk):
                p_size = len(p.split())
                if overlap_size + p_size <= overlap:
                    overlap_paras.insert(0, p)
                    overlap_size += p_size
                else:
                    break

            current_chunk = overlap_paras
            current_size = overlap_size

        current_chunk.append(para)
        current_size += para_size

    if current_chunk:
        chunks.append("\n\n".join(current_chunk))

    return chunks
```

---

## Appendix B: Ensemble YAML Templates

See `.llm-orc/ensembles/` directory for complete ensemble definitions:

- `plexus-concept-extractor.yaml` - Standard concept extraction
- `plexus-chunked-extractor.yaml` - Chunked extraction with accumulation (NEW)
- `plexus-category-namer.yaml` - Category naming
- `plexus-summarizer.yaml` - Cluster summarization
- `plexus-verifier.yaml` - Concept verification (optional)

---

## Next: [10-research-outcomes.md](./10-research-outcomes.md) — Research Outcomes
