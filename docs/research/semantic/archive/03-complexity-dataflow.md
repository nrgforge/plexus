# Complexity Analysis & Data Flow

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 7. Complexity Analysis

### 7.1 Variable Definitions (Multi-Level)

The layered model introduces sections as the primary unit of semantic analysis:

| Variable | Meaning | Typical Range |
|----------|---------|---------------|
| $n$ | Number of documents | 100 - 10,000 |
| $s$ | Average sections per document | 3 - 15 |
| $N = n \times s$ | **Total sections** (primary unit) | 300 - 150,000 |
| $d$ | Average document degree (explicit links) | 3 - 20 |
| $d_s$ | Average section degree (structural + anchor links) | 2 - 5 |
| $p$ | Sampling proportion | 0.10 - 0.20 |
| $T$ | Propagation iterations | log N |
| $\|C\|$ | Number of discovered concepts | approx sqrt(N) |
| $K$ | Number of category clusters | approx log \|C\| |

**Key insight**: We now have $N$ sections instead of $n$ documents, but:
- More nodes = more precise semantic placement
- Same asymptotic complexity class
- LLM calls still proportional to sample (p x N)

### 7.2 Per-Phase Complexity (Multi-Level)

| Phase | Operation | Complexity | Justification |
|-------|-----------|------------|---------------|
| 1 | Structural Bootstrap | $O(n \times s)$ | Parse docs into sections |
| 1b | Link extraction | $O(n \times d)$ | Extract doc-level links |
| 1c | Sibling edge creation | $O(N)$ | Create structural edges |
| 2a | Doc-level PageRank | $O(k \times n \times d)$ | $k$ iterations on doc graph |
| 2b | Section scoring | $O(N)$ | Distribute + features |
| 3 | Semantic Seeding | $O(p \times N)$ | LLM calls for sampled sections |
| 4 | Label Propagation | $O(T \times N \times d_s)$ | Multi-level propagation |
| 5 | HAC Clustering | $O(\|C\|^2 \log \|C\|)$ | Standard HAC |

### 7.3 Total Complexity

$$T(n, s) = O(n \times s) + O(n \times d) + O(p \times N) + O(T \times N \times d_s) + O(|C|^2 \log |C|)$$

Substituting $N = n \times s$ and simplifying:

$$T(n, s) = O(N) + O(n \times d) + O(p \times N) + O(N \times d_s \times \log N) + O(N \log N)$$

Dominant term (propagation over section graph):

$$\boxed{T(N) = O(N \times d_s \times \log N)}$$

Where $N$ = total sections, $d_s$ = section connectivity (small constant).

**For typical PKM corpus** (1000 docs, 5 sections/doc = 5000 sections):
- Doc-level: 1000 nodes
- Section-level: 5000 nodes
- Complexity increase: ~5x, but with ~5x better semantic precision

**Comparison**:
| Approach | Complexity | Granularity |
|----------|------------|-------------|
| Naive (all pairs) | $O(N^2)$ | Section |
| Doc-level sampling | $O(n \log n)$ | Document |
| **Section-level sampling** | $O(N \log N)$ | Section |

Same complexity class as doc-level, but finer granularity.

### 7.4 LLM Call Budget (Section-Level)

| Phase | LLM Calls | Model Type | Notes |
|-------|-----------|------------|-------|
| Phase 3 | $p \times N$ | Fast/local | Section extraction |
| Phase 4 (optional) | $v \times N$ where $v \ll p$ | Fast/local | Low-confidence verification |
| Phase 5 | $K \approx \log \|C\|$ | Reasoning | Category naming |

**Total LLM calls**: $O(N)$ fast + $O(\log N)$ expensive

**Example budgets**:

| Corpus Size | Docs | Sections (N) | Sampled (15%) | Fast Calls | Expensive Calls |
|-------------|------|--------------|---------------|------------|-----------------|
| Small | 100 | 500 | 75 | ~75 | ~5 |
| Medium | 1,000 | 5,000 | 750 | ~750 | ~10 |
| Large | 10,000 | 50,000 | 7,500 | ~7,500 | ~15 |

**Tradeoff**: More granular analysis requires more LLM calls, but:
- Each call processes less text (section vs document)
- Faster per-call latency
- Better concept placement accuracy

### 7.5 Extended Features Complexity

The experience vision (Section 13) adds runtime operations. These are amortized over user interaction, not batch processing:

| Feature | Trigger | Complexity | Notes |
|---------|---------|------------|-------|
| **Gap Detection** | Community change | $O(K^2)$ | $K$ = communities, typically $\log n$ |
| **Signal Aggregation** | Edge update | $O(s)$ | $s$ = signals per edge, constant (~5) |
| **Incremental Update** | Document edit | $O(d \times T_{local})$ | $d$ = neighbors, $T_{local}$ = local iterations |
| **Narrator Updates** | Graph change | $O(1)$ | Event-driven, append to log |

**Gap Detection** (from InfraNodus research):
$$\text{gap\_score}(C_i, C_j) = \frac{\text{expected\_edges}(C_i, C_j) - \text{actual\_edges}(C_i, C_j)}{\text{expected\_edges}(C_i, C_j)}$$

Where expected edges are estimated from embedding similarity between cluster centroids.

Complexity: $O(K^2)$ pairwise community comparisons, but $K \approx \log n$, so $O(\log^2 n)$.

**Multi-Signal Aggregation**:
$$\text{strength}(e) = \sum_{s \in \text{signals}(e)} w_s \times c_s$$

Per-edge update is $O(s)$ where $s \approx 5$ signal types. Total for all edges: $O(|E| \times s) = O(n \times d)$.

**Incremental Update Scope**:

| Change Type | Affected Scope | Complexity |
|-------------|---------------|------------|
| Cosmetic | None | $O(1)$ |
| Content edit | Single document | $O(d)$ |
| Structural edit | Local neighborhood | $O(d^2)$ |
| Major rewrite | Propagation subgraph | $O(T \times d_{avg})$ |

### 7.6 Real-Time Latency Budget

For flow-state experience, operations must meet latency targets:

| Operation | Target | Complexity | Strategy |
|-----------|--------|------------|----------|
| Keystroke → diff | < 50ms | $O(\text{edit size})$ | Incremental diff |
| Diff → edge update | < 50ms | $O(d)$ | In-memory graph |
| Edge → visualization | < 16ms | $O(1)$ | WebGL batching |
| Background extraction | < 500ms | $O(1)$ LLM call | Async worker |
| Gap detection | < 2s | $O(\log^2 n)$ | Debounced, on pause |

**Key insight**: Real-time responsiveness is achieved by:
1. Separating fast path (edge updates) from slow path (LLM extraction)
2. Debouncing expensive operations (gap detection, community recalculation)
3. Using in-memory indices (HNSW for vectors, petgraph for structure)

---

## 8. Data Flow Diagram (Multi-Level)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                       MULTI-LEVEL DATA FLOW                                   │
│                                                                              │
│   FILE SYSTEM              STRUCTURAL GRAPH              SEMANTIC GRAPH      │
│   ───────────              ────────────────              ──────────────      │
│                                                                              │
│   /vault/                  ┌─────────────────────────────────────────┐       │
│   ├── /react/              │           G₀ (Phase 1)                  │       │
│   │   ├── hooks.md         │                                         │       │
│   │   │   ├── ## useState  │   Context                               │       │
│   │   │   └── ## useEffect │      │                                  │       │
│   │   └── components.md    │      ├──► Directory ◄──Sibling──► Dir   │       │
│   └── /rust/               │      │       │                          │       │
│       └── tauri.md         │      │       ├──► Document ◄──LinksTo   │       │
│                            │      │       │       │                  │       │
│   Parse hierarchy          │      │       │       ├──► Section       │       │
│         │                  │      │       │       │       │          │       │
│         ▼                  │      │       │       │       └──► Block │       │
│   Extract sections         │      │       │       │                  │       │
│         │                  └──────┼───────┼───────┼──────────────────┘       │
│         ▼                         │       │       │                          │
│   Create edges                    │       │       │                          │
│   (Contains, Sibling,             │       │       │                          │
│    LinksTo, HasSection)           │       │       │                          │
│                                   │       │       │                          │
│                            ┌──────┴───────┴───────┴──────────────────┐       │
│                            │  Phase 2: Hybrid Scoring                 │       │
│                            │                                          │       │
│                            │  Doc-level:  PageRank on LinksTo edges  │       │
│                            │       │                                  │       │
│                            │       ▼                                  │       │
│                            │  Section-level:  Distribute + features  │       │
│                            │       │                                  │       │
│                            │       ▼                                  │       │
│                            │  Seeds S = top 15% of SECTIONS          │       │
│                            └──────────────────┬───────────────────────┘       │
│                                               │                               │
│                                               │ LLM extraction (per section)  │
│                                               ▼                               │
│                            ┌─────────────────────────────────────────┐       │
│                            │  Phase 3: Section→Concept Edges         │       │
│                            │                                         │       │
│                            │  § useState ──Discusses──► "React Hooks"│       │
│                            │  § useEffect ──Discusses──► "Side Effects"      │
│                            │                                         │       │
│                            │  Concept Vocabulary C                   │       │
│                            └──────────────────┬──────────────────────┘       │
│                                               │                               │
│                                               │ Multi-level propagation       │
│                                               ▼                               │
│                            ┌─────────────────────────────────────────┐       │
│                            │  Phase 4: Label Propagation             │       │
│                            │                                         │       │
│                            │  Propagate via:                         │       │
│                            │  • Sibling (same doc): weight 0.8       │       │
│                            │  • Sibling (same dir): weight 0.5       │       │
│                            │  • LinksTo: weight 0.7                  │       │
│                            │  • Parent→child: weight 0.5             │       │
│                            │                                         │       │
│                            │  All sections now have concept labels   │       │
│                            └──────────────────┬──────────────────────┘       │
│                                               │                               │
│                                               │ Clustering + naming           │
│                                               ▼                               │
│                            ┌─────────────────────────────────────────┐       │
│                            │  Phase 5: Dual Hierarchy                │       │
│                            │                                         │       │
│                            │  STRUCTURAL          SEMANTIC           │       │
│                            │  ──────────          ────────           │       │
│                            │  /vault/             All Concepts       │       │
│                            │   ├── /react/         ├── "Frontend"    │       │
│                            │   │   ├── hooks.md    │   ├── "React"   │       │
│                            │   │   │   └── §use*   │   │   └── hooks │       │
│                            │   │   └── components  │   └── "State"   │       │
│                            │   └── /rust/          └── "Backend"     │       │
│                            │       └── tauri.md        └── "Tauri"   │       │
│                            │                                         │       │
│                            │  Two zoom dimensions:                   │       │
│                            │  • Structural: dir → doc → section      │       │
│                            │  • Semantic: category → concept         │       │
│                            └─────────────────────────────────────────┘       │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 8.1 Edge Type Summary

| Edge Type | Direction | Created In | Weight | Purpose |
|-----------|-----------|------------|--------|---------|
| Contains | Dir → Doc/Dir | Phase 1 | 1.0 | Structural hierarchy |
| HasSection | Doc → Section | Phase 1 | 1.0 | Document structure |
| Sibling | Node ↔ Node | Phase 1 | 0.3-0.8 | Same-parent relationship |
| LinksTo | Doc/Section → Doc/Section | Phase 1 | 0.7-0.85 | Explicit references |
| Discusses | Section → Concept | Phase 3 | varies | Semantic extraction |
| RelatedTo | Concept ↔ Concept | Phase 5 | varies | Concept relationships |
| SimilarTo | Section ↔ Section | Phase 4+ | varies | Concept co-occurrence |

---

## Next: [04-implementation.md](./04-implementation.md) — Implementation Plan
