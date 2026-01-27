# Embedding Storage & Network Science

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 11. Embedding Storage Architecture

### 11.1 The Architectural Question

Embeddings introduce a different query paradigm than graph traversal:

| Query Type | Graph (Current) | Vector (New) |
|------------|-----------------|--------------|
| "Find related" | Traverse edges (BFS) | k-NN similarity search |
| Index structure | B-tree on node IDs | HNSW or IVF index |
| Complexity | $O(d^k)$ for k hops | $O(\log n)$ approximate |
| Relationship type | Explicit (links) | Implicit (similarity) |

**Core tension**: Graph traversal follows *explicit* edges. Vector similarity finds *implicit* relationships. These are complementary but architecturally different.

### 11.2 Storage Options (Local-First)

| Option | Pros | Cons | Recommendation |
|--------|------|------|----------------|
| **SQLite + sqlite-vss** | Single DB, simple | Performance ceiling ~100K vectors | Good for MVP |
| **SQLite + in-memory HNSW** | Fast queries, Rust-native (`instant-distance`) | Memory overhead, rebuilds on startup | Best balance |
| **Separate vector DB** | Purpose-built, scalable | Operational complexity | Overkill for local-first |

**Recommended approach**: SQLite for graph + in-memory HNSW for vectors.

### 11.3 Data Model: Parallel Index

Embeddings should be a **parallel index**, not a node property:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         HYBRID STORAGE ARCHITECTURE                          │
│                                                                             │
│  ┌─────────────────────────────────┐    ┌─────────────────────────────────┐ │
│  │       Graph Layer (SQLite)      │    │    Vector Index (HNSW)          │ │
│  │                                 │    │                                 │ │
│  │  ┌─────────┐    ┌─────────┐    │    │  ┌─────────────────────────┐    │ │
│  │  │  nodes  │───►│  edges  │    │    │  │  concept_id → embedding │    │ │
│  │  └────┬────┘    └─────────┘    │    │  └────────────┬────────────┘    │ │
│  │       │                        │    │               │                 │ │
│  │       │ node_id                │    │               │ k-NN query      │ │
│  │       ▼                        │    │               ▼                 │ │
│  │  ┌─────────┐                   │    │  similar_concepts(query, k=10)  │ │
│  │  │concepts │◄──────────────────┼────┼───────────────────────────────► │ │
│  │  └─────────┘   foreign key     │    │         returns: [concept_ids]  │ │
│  │                                │    │                                 │ │
│  └─────────────────────────────────┘    └─────────────────────────────────┘ │
│                                                                             │
│  On startup: Load embeddings from SQLite → Build HNSW index                 │
│  On update:  Update SQLite → Rebuild affected index partition               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why parallel index?**
- Embeddings may change (different models, recomputation)
- Not all nodes need embeddings (only semantic layer concepts)
- Keeps SQLite schema clean
- Vector index can be rebuilt without touching graph

### 11.4 What Gets Embedded?

With section-level architecture, we have four node types. Not all need embeddings:

| Node Type | Embedded? | Rationale |
|-----------|-----------|-----------|
| **Concepts** | Yes | Primary use case: find semantically similar concepts for clustering and ontology |
| **Sections** | Optional | Enables "find similar sections" without concepts; useful for cold-start exploration |
| **Documents** | No | Too coarse; document similarity better derived from section/concept overlap |
| **Directories** | No | Structural, not semantic |

**MVP**: Concept embeddings only. Section embeddings are a future enhancement.

**Concept embedding creation**:
```
Section text → LLM extraction → Concept name + definition → Embedding model → Vector
```

We embed the concept **definition** (not just name), providing richer semantic signal. Example:
- Concept name: "PageRank"
- Definition: "Graph algorithm that assigns importance scores to nodes based on incoming link structure"
- Embedding: `embed(definition)` → 384-dim vector

**Section embedding (future)**:
```
Section text → Embedding model → Vector
```

For large sections (>2000 words), we'd embed a summary or use chunked embeddings (Late Chunking style).

### 11.5 Hybrid Traversal

With both graph edges and vector similarity, we can offer hybrid traversal:

```rust
pub enum TraversalMode {
    /// Follow explicit edges only (current behavior)
    Graph { max_depth: usize },

    /// Find similar nodes by embedding distance
    Vector { top_k: usize, min_similarity: f32 },

    /// Combine: expand via edges, then add semantically similar
    Hybrid {
        graph_depth: usize,
        vector_top_k: usize,
        /// 0.0 = graph only, 1.0 = vector only
        blend: f32,
    },
}
```

**Use cases**:
- "What documents link to X?" → `Graph`
- "What documents discuss similar topics to X?" → `Vector`
- "Explore the neighborhood of X" → `Hybrid`

### 11.6 Schema Extension

```sql
-- New table for concept embeddings
CREATE TABLE concept_embeddings (
    concept_id TEXT PRIMARY KEY,
    context_id TEXT NOT NULL,
    model_name TEXT NOT NULL,           -- e.g., "all-MiniLM-L6-v2"
    embedding BLOB NOT NULL,            -- serialized f32 vector
    created_at INTEGER NOT NULL,
    FOREIGN KEY (concept_id, context_id)
        REFERENCES nodes(id, context_id)
);

CREATE INDEX idx_embeddings_context ON concept_embeddings(context_id);
CREATE INDEX idx_embeddings_model ON concept_embeddings(model_name);
```

**Decision**: Use local embedding model (`all-MiniLM-L6-v2`, 384 dimensions) for local-first compliance. Can add API embeddings as optional enhancement later.

---

## 12. Network Science Foundations

### 12.1 Scale-Free Networks (Barabási-Albert Model)

The [Barabási-Albert model](https://en.wikipedia.org/wiki/Barab%C3%A1si%E2%80%93Albert_model) describes how real-world networks form through **preferential attachment**: new nodes prefer to connect to already well-connected nodes ("the rich get richer").

**Relevance to Plexus**:
- Document networks often exhibit power-law degree distributions
- A few documents become "hubs" with many incoming links
- These hubs are disproportionately important for semantic analysis

**Implication**: Our PageRank-based seed selection exploits this property. By analyzing hubs first, we capture concepts that are likely to propagate widely.

### 12.2 Network Robustness

From [Barabási's Network Science](http://networksciencebook.com/chapter/5):
- Scale-free networks are **robust to random failures** but **vulnerable to targeted attacks on hubs**
- If we get the semantic analysis of hub documents wrong, errors propagate through the network

**Implication**: Consider a verification pass specifically for hub documents, even if they have high PageRank confidence. The cost of errors is higher.

### 12.3 Small-World Property

Knowledge bases typically exhibit [small-world properties](https://en.wikipedia.org/wiki/Small-world_network) (Watts-Strogatz):
- High clustering coefficient (documents cluster by topic)
- Short average path length (any two documents connected by few hops)

**Implication**: Label propagation converges quickly because the network diameter is small. Typically $T = O(\log n)$ iterations suffice.

### 12.4 Community Structure

The Louvain algorithm (Blondel et al., 2008) detects communities by optimizing modularity. Communities in document networks often correspond to:
- Topic clusters
- Project boundaries
- Authorship groups

**Implication**: Phase 5 (ontology crystallization) should respect community boundaries. Categories that span multiple communities may be too broad.

### 12.5 Connection to Graph Neural Networks

Recent research shows deep connections between label propagation and GNNs:

- [Combining GCN and Label Propagation](https://dl.acm.org/doi/10.1145/3490478) (Wang et al., 2021): Edge weights can be learned, with LPA serving as regularization
- [Label Propagation for Deep Semi-supervised Learning](https://arxiv.org/abs/1904.04717) (Iscen et al., 2019): Transductive label propagation on embedding manifolds

**Implication**: Our label propagation approach is theoretically grounded in the same principles that make GNNs effective. If we later want neural approaches, the architecture is compatible.

### 12.6 Preferential Attachment Justification

The seed selection formula:

$$S = \{v \in V : \text{rank}(PR[v]) \leq p \times |V|\} \cup \text{top}_k(\text{bridge\_score})$$

Is justified by preferential attachment theory:
- High PageRank nodes are the "rich" nodes that attract connections
- These nodes have outsized influence on network structure
- Analyzing them first maximizes information gain per LLM call

---

## Next: [07-experience-vision.md](./07-experience-vision.md) — Experience Vision
