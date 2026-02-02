# Problem Statement & Core Algorithm

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 1. Problem Statement

### 1.1 The Naive Approach (Why It Fails)

A naive semantic analysis treats documents as atomic units:
1. Send every document to an LLM for concept extraction
2. Compare every pair of documents for semantic similarity
3. Create edges between semantically related documents

**Problems**:

| Issue | Why It Fails |
|-------|--------------|
| **Computational** | $O(n)$ LLM calls + $O(n^2)$ comparisons = infeasible at scale |
| **Granularity** | Documents aren't atomic — a 50-page doc has many distinct topics |
| **Structure blindness** | Ignores that file/folder organization IS semantic signal |
| **Flat model** | No zoom — can't navigate from "big picture" to "specific detail" |

For 1,000 documents: ~500,000 comparisons. For 10,000: ~50,000,000.

### 1.2 The Layered Insight

**Documents are not the atomic unit of semantics.**

Structure exists at multiple levels, and each level carries semantic meaning:

```
Context (corpus/vault)
├── Directory structure ──────────── Implicit clustering: siblings are related
│   └── Documents ────────────────── Explicit links (wikilinks, imports)
│       └── Sections (H1/H2/H3) ──── Topical boundaries within doc
│           └── Blocks ───────────── Paragraphs, code, lists
│               └── Concepts ─────── Named entities, ideas, terms
```

**Key insight**: Structure IS semantics. Files in `/hooks/` are related. Sections under "## Authentication" share a topic. We get semantic signal for free from structure we already have.

### 1.3 Design Constraints

| Constraint | Requirement |
|------------|-------------|
| **Computational** | $O(n \log n)$ or better, where n = sections (not docs) |
| **LLM Calls** | Proportional to sample, not corpus |
| **Multi-Level** | Graph has layers: dirs, docs, sections, concepts |
| **Structure = Signal** | Directory/heading structure informs semantics |
| **Zoom** | Navigate both structurally (drill down) and semantically (category roll-up) |
| **Incremental** | Handle additions at any level efficiently |

---

## 2. Core Data Model: The Multi-Level Graph

### 2.1 Node Types

```rust
pub enum NodeType {
    /// Root of a context (a vault, repo, or document collection)
    Context { path: PathBuf },

    /// Directory in the file system
    Directory {
        path: PathBuf,
        depth: usize,  // 0 = root
    },

    /// A document (file)
    Document {
        path: PathBuf,
        content_hash: String,
        word_count: usize,
    },

    /// A section within a document (defined by headings)
    Section {
        doc_id: NodeId,
        heading: String,
        level: u8,        // 1 = H1, 2 = H2, etc.
        start_line: usize,
        end_line: usize,
    },

    /// A block within a section (paragraph, code block, list, etc.)
    Block {
        section_id: NodeId,
        block_type: BlockType,
        content_hash: String,
    },

    /// A semantic concept (extracted by LLM or propagated)
    Concept {
        name: String,
        canonical: String,  // normalized form
        source: ConceptSource,
    },
}

pub enum BlockType {
    Paragraph,
    CodeBlock { language: Option<String> },
    List { ordered: bool },
    Blockquote,
    Table,
}

pub enum ConceptSource {
    Extracted { model: String, confidence: f32 },
    Propagated { from: NodeId, confidence: f32 },
    Manual,
}
```

### 2.2 Edge Types

```rust
pub enum EdgeType {
    // === Structural (deterministic, from parsing) ===

    /// Directory contains subdirectory or document
    Contains,

    /// Document contains section
    HasSection,

    /// Section contains block (optional granularity)
    HasBlock,

    /// Sibling relationship (same parent directory)
    Sibling,

    /// Explicit link (wikilink, markdown link, import)
    LinksTo {
        link_type: LinkType,
        anchor: Option<String>,  // #section-anchor
    },

    /// Section references another section (internal links)
    InternalRef,

    // === Semantic (from LLM or propagation) ===

    /// Node discusses concept
    Discusses {
        confidence: f32,
        evidence: Option<String>,
    },

    /// Concept is related to concept
    RelatedTo {
        relation_type: String,  // "is-a", "part-of", "causes", etc.
        confidence: f32,
    },

    // === Derived (computed) ===

    /// Semantic similarity above threshold
    SimilarTo {
        similarity: f32,
        source: SimilaritySource,
    },
}

pub enum LinkType {
    Wikilink,
    MarkdownLink,
    Import,
    Url,
}
```

### 2.3 The Multi-Level Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MULTI-LEVEL GRAPH STRUCTURE                          │
│                                                                             │
│  STRUCTURAL HIERARCHY              SEMANTIC OVERLAY                         │
│  (deterministic)                   (LLM-derived)                            │
│                                                                             │
│       Context                                                               │
│          │                                                                  │
│          ├─► Directory ◄───Sibling───► Directory                           │
│          │      │                          │                                │
│          │      ├─► Document ◄──LinksTo──► Document                        │
│          │      │      │                      │                             │
│          │      │      ├─► Section ◄─────────┼───────► Concept             │
│          │      │      │      │              │            │                 │
│          │      │      │      └─► Block      │         RelatedTo           │
│          │      │      │                     │            │                 │
│          │      │      └─► Section ──────────┘            ▼                 │
│          │      │                                      Concept              │
│          │      └─► Document                                                │
│          │                                                                  │
│          └─► Directory                                                      │
│                                                                             │
│  Edges across levels:                                                       │
│    • Contains (dir→doc, doc→section)                                       │
│    • LinksTo (doc→doc, section→section via anchors)                        │
│    • Discusses (section→concept, block→concept)                            │
│    • Sibling (implicit edge between same-parent nodes)                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Algorithm Overview

The algorithm proceeds in five phases, operating on the multi-level graph:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     SEMANTIC ANALYSIS PIPELINE                               │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  PHASE 1: STRUCTURAL BOOTSTRAP                              O(n × d)  │  │
│  │  • Parse directory tree → Directory nodes                             │  │
│  │  • Parse documents → Document + Section + Block nodes                 │  │
│  │  • Extract explicit links → LinksTo edges                             │  │
│  │  • Create implicit edges → Sibling, Contains                          │  │
│  │  • Build multi-level graph G₀                                         │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  PHASE 2: IMPORTANCE SCORING                               O(k × |E|) │  │
│  │  • Score at SECTION level (not document)                              │  │
│  │  • Consider: link structure + directory position + content features   │  │
│  │  • Select seed sections S (top p%)                                    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  PHASE 3: SEMANTIC SEEDING (LLM)                               O(|S|) │  │
│  │  • Extract concepts from seed SECTIONS (not whole docs)               │  │
│  │  • Build concept vocabulary C                                         │  │
│  │  • Create Section→Concept edges                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  PHASE 4: LABEL PROPAGATION                          O(T × n × d)     │  │
│  │  • Propagate concepts through ALL edge types:                         │  │
│  │    - Within doc: section→section (same doc = high weight)             │  │
│  │    - Across docs: via LinksTo edges                                   │  │
│  │    - Via structure: Sibling edges (same dir = related)                │  │
│  │  • Different decay rates per edge type                                │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  PHASE 5: ONTOLOGY CRYSTALLIZATION                    O(|C|² log |C|) │  │
│  │  • Structural hierarchy already exists (dir→doc→section)              │  │
│  │  • Build SEMANTIC hierarchy via concept clustering                    │  │
│  │  • Two zoom dimensions: structural depth + semantic abstraction       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Phase Specifications

### 4.1 Phase 1: Structural Bootstrap

**Goal**: Build the multi-level graph $G_0$ from directory and document structure.

**Algorithm**:
```
STRUCTURAL_BOOTSTRAP(root_path):
    G ← empty_graph()

    // === LEVEL 1: Directory Structure ===
    context_node ← create_node(Context, root_path)
    G.add(context_node)

    for each directory d in walk_dirs(root_path):
        dir_node ← create_node(Directory, d.path, depth=d.depth)
        G.add(dir_node)
        G.add_edge(parent(d), dir_node, Contains)

        // Sibling edges within same directory
        for each sibling in siblings(d):
            G.add_edge(dir_node, sibling, Sibling, weight=0.5)

    // === LEVEL 2: Documents ===
    for each file f in walk_files(root_path):
        doc_node ← create_node(Document, f.path, hash(f), word_count(f))
        G.add(doc_node)
        G.add_edge(parent_dir(f), doc_node, Contains)

        // Sibling edges: files in same directory
        for each sibling_doc in same_dir(f):
            G.add_edge(doc_node, sibling_doc, Sibling, weight=0.3)

        // === LEVEL 3: Sections ===
        sections ← parse_sections(f)  // H1, H2, H3 boundaries
        prev_section ← null

        for each section s in sections:
            section_node ← create_node(Section, doc_node.id, s.heading, s.level, s.lines)
            G.add(section_node)
            G.add_edge(doc_node, section_node, HasSection)

            // Sequential sections are implicitly related
            if prev_section ≠ null:
                G.add_edge(prev_section, section_node, Sibling, weight=0.7)
            prev_section ← section_node

            // === LEVEL 4: Blocks (optional, for deep analysis) ===
            if config.parse_blocks:
                for each block b in parse_blocks(s):
                    block_node ← create_node(Block, section_node.id, b.type, hash(b))
                    G.add(block_node)
                    G.add_edge(section_node, block_node, HasBlock)

        // === CROSS-DOCUMENT LINKS ===
        links ← extract_links(f)  // wikilinks, markdown links, imports
        for each link in links:
            target ← resolve_link(link, G)
            if target exists:
                source_section ← section_containing(link.position)
                target_node ← resolve_to_section_or_doc(target, link.anchor)
                G.add_edge(source_section, target_node, LinksTo, link.type)

    return G
```

**Section Parsing Rules**:

| Content Type | Section Markers | Notes |
|--------------|-----------------|-------|
| Markdown | `# H1`, `## H2`, etc. | Standard heading hierarchy |
| Code | `fn`, `class`, `def`, `impl` | Function/class as section |
| Literature | `ACT`, `SCENE`, `CHAPTER` | Detect via patterns |
| Org-mode | `* H1`, `** H2` | Asterisk-based |

**Complexity**: $O(n \times s)$ where $n$ = documents, $s$ = avg sections per document

**Output**: Multi-level graph with structural hierarchy and explicit links.

---

### 4.2 Phase 2: Importance Scoring

> **Implementation Note**: In [04-implementation.md](./04-implementation.md), this phase is split into **Phase 1a** (Structural Bootstrap) and **Phase 1b** (Priority Sampling). The algorithm here describes the logical flow; implementation may interleave these steps.

**Goal**: Identify which **sections** to analyze semantically.

**Key change from document-level**: We score sections, not documents. A 50-section document might have 3 important sections and 47 boilerplate ones.

#### 4.2.1 Why Document-Level Scoring First

**Critical insight**: Explicit links (`[[wikilinks]]`, markdown links) exist primarily at the **document level**. Section-level links (via `#anchors`) are sparse in most corpora.

This means:
- **PageRank/HITS work well** on the document graph (dense links)
- **PageRank/HITS may not work** on the section graph (sparse links, mostly structural edges)

**Our approach**: Compute importance at document level, then distribute to sections.

```
DOCUMENT GRAPH (rich)              SECTION GRAPH (sparse)
─────────────────────              ─────────────────────
DocA ◄──────► DocB                 §1 ──Contains── §2
  │              │                  │
  └──────► DocC ◄┘                 §3 ──Sibling─── §4

PageRank/HITS work here            Mostly structural edges
```

**Future refinement**: Once concepts are extracted and propagated, sections will share concepts. This creates implicit semantic edges (sections discussing same concept). At that point, section-level centrality becomes meaningful for re-scoring.

#### 4.2.2 Multi-Level Importance

```rust
pub trait ImportanceStrategy: Send + Sync {
    /// Compute importance scores for nodes at a specific level
    fn score(&self, graph: &Graph, level: NodeLevel) -> HashMap<NodeId, f64>;

    fn name(&self) -> &'static str;
}

pub enum NodeLevel {
    Directory,
    Document,  // Primary level for link-based scoring
    Section,   // Derived from doc score + features
    Block,
}
```

#### 4.2.3 Hybrid Scoring Strategy

**Phase 1 (Bootstrap)**: Document-level link analysis
- Run PageRank or HITS on document graph
- Identifies important documents via link structure

**Phase 2 (Distribution)**: Section scoring from doc + features
- Section inherits parent doc importance
- Adjusted by section-specific features

| Factor | Source | Weight | Notes |
|--------|--------|--------|-------|
| **Parent doc importance** | PageRank/HITS on docs | High | Primary signal |
| **Heading level** | Structural | Medium | H1 > H2 > H3 |
| **Section position** | Structural | Low | First sections often important |
| **Content features** | Text analysis | Medium | Length, definitions, code blocks |
| **Inbound anchors** | Links with `#section` | High | Direct section references (rare but strong) |
| **Directory position** | Path patterns | Low | `/core/` vs `/examples/` |

**Algorithm**:
```
SCORE_SECTIONS(G, strategy, weights):
    section_scores ← {}

    // PRIMARY: Document-level importance (where links are dense)
    doc_scores ← strategy.score(G, level=Document)  // PageRank or HITS

    // SECONDARY: Section-specific anchor links (sparse but strong)
    anchor_targets ← count_inbound_anchors(G)  // sections referenced via #anchor

    for each section s in G.sections():
        // Inherit from parent document
        parent_doc ← G.parent(s, type=Document)
        base ← doc_scores[parent_doc] × weights.doc_inheritance

        // Boost for direct anchor references
        anchor_boost ← anchor_targets[s] × weights.anchor_weight

        // Structural features
        level_boost ← (4 - s.heading_level) / 3 × weights.level_weight  // H1=1.0, H2=0.67, H3=0.33
        position_boost ← 1.0 / (1 + s.position) × weights.position_weight

        // Content features
        content_boost ← content_score(s) × weights.content_weight

        // Directory context
        dir ← G.parent(parent_doc, type=Directory)
        dir_boost ← directory_importance(dir.path) × weights.dir_weight

        section_scores[s] ← base + anchor_boost + level_boost + position_boost + content_boost + dir_boost

    return section_scores
```

#### 4.2.4 Iterative Refinement (Post-Propagation)

After Phase 4 (Label Propagation), sections have concept labels. This enables:

```
REFINE_SECTION_SCORES(G, section_concepts):
    // Build concept co-occurrence graph at section level
    for each concept c in all_concepts:
        sections_with_c ← {s : c in section_concepts[s]}

        // Sections sharing concepts are semantically linked
        for each pair (s1, s2) in sections_with_c:
            if s1.doc ≠ s2.doc:  // Cross-document
                G.add_edge(s1, s2, SimilarTo, weight=jaccard(concepts[s1], concepts[s2]))

    // NOW section-level PageRank/HITS becomes meaningful
    refined_scores ← strategy.score(G, level=Section)

    return refined_scores
```

This creates a feedback loop: better scoring → better seeds → better concepts → better scoring.

#### 4.2.5 Seed Selection

```
SELECT_SEEDS(G, config):
    scores ← SCORE_SECTIONS(G, config.strategy, config.weights)

    // Select top p% of sections
    threshold ← percentile(scores.values(), 1.0 - config.sample_proportion)
    seeds ← {s : scores[s] >= threshold}

    // Ensure coverage: at least one section per important document
    important_docs ← top_k_documents(G, k=config.min_doc_coverage)
    for each doc in important_docs:
        if no section from doc in seeds:
            best_section ← argmax(scores[s] for s in doc.sections)
            seeds.add(best_section)

    // Add bridge sections (connect otherwise disconnected clusters)
    bridges ← top_k_by_betweenness(G, level=Section, k=config.bridge_count)
    seeds ← seeds ∪ bridges

    return seeds
```

---

### 4.3 Phase 3: Semantic Seeding

**Goal**: Extract concepts from seed **sections**, not whole documents.

**Why sections?**
- More focused context → better extraction
- Concepts tied to specific locations
- Can detect topic shifts within a document

**Algorithm**:
```
SEMANTIC_SEEDING(G, seeds, ensemble):
    C ← {}  // Concept vocabulary
    section_concepts ← {}

    for each section s in seeds:
        // Get section content with context
        content ← get_section_content(s)
        context ← {
            document: s.parent_doc.title,
            heading_path: s.heading_path,  // e.g., "Authentication > OAuth"
            surrounding: get_sibling_headings(s),
        }

        result ← ensemble.extract_concepts(content, context)

        for each concept in result.concepts:
            canonical ← normalize(concept.name)

            if canonical ∉ C:
                concept_node ← create_node(Concept, canonical, concept.type)
                G.add(concept_node)
                C[canonical] ← concept_node

            // Edge from SECTION to concept (not doc to concept)
            G.add_edge(s, C[canonical], Discusses,
                       confidence=concept.confidence,
                       evidence=concept.evidence)

        section_concepts[s] ← result.concepts

    return C, section_concepts
```

**Extraction Prompt Context**:
```yaml
input:
  content: "The section text..."
  context:
    document: "Authentication.md"
    heading_path: "OAuth > Token Refresh"
    siblings: ["OAuth > Initial Auth", "OAuth > Scopes"]
```

---

### 4.4 Phase 4: Label Propagation

**Goal**: Extend semantic coverage using ALL edge types in the multi-level graph.

**Key insight**: Different edge types have different propagation weights.

#### 4.4.1 Edge Weights for Propagation

| Edge Type | Weight | Rationale |
|-----------|--------|-----------|
| **HasSection** (doc→section) | 0.9 | Sections inherit doc-level concepts |
| **Sibling** (same doc) | 0.8 | Adjacent sections share topics |
| **Sibling** (same dir) | 0.5 | Files in same folder often related |
| **LinksTo** (explicit link) | 0.7 | Author-declared relationship |
| **LinksTo** (with anchor) | 0.85 | Specific section reference = strong |
| **Contains** (dir→doc) | 0.3 | Weak: directory = broad category |

#### 4.4.2 Multi-Level Propagation Algorithm

```
LABEL_PROPAGATION(G, section_concepts, config):
    // Initialize: seed sections have labels
    labels ← {}
    for each node n in G.nodes():
        if n in section_concepts:
            labels[n] ← section_concepts[n]
        else:
            labels[n] ← {}

    // Propagate through multi-level structure
    for iteration in 1..config.max_iterations:
        changed ← 0
        new_labels ← copy(labels)

        for each node n in G.nodes():
            if n in section_concepts:
                continue  // Don't modify seed labels

            // Aggregate from ALL neighbor types
            votes ← Counter()

            for each (neighbor, edge) in G.edges_to(n):
                edge_weight ← EDGE_WEIGHT(edge.type)
                decay ← config.decay_per_hop

                for (concept, conf) in labels[neighbor]:
                    propagated ← conf × edge_weight × decay
                    votes[concept] += propagated

            // Also propagate DOWN the hierarchy (parent → child)
            parent ← G.parent(n)
            if parent and labels[parent]:
                for (concept, conf) in labels[parent]:
                    # Inheritance from parent (doc→section, section→block)
                    votes[concept] += conf × 0.5

            // Accept above threshold
            new_labels[n] ← {(c, score) for (c, score) in votes if score > config.threshold}

            if new_labels[n] ≠ labels[n]:
                changed += 1

        labels ← new_labels
        if changed == 0:
            break

    // Roll up: aggregate section labels to document level
    for each doc in G.documents():
        doc_labels ← aggregate([labels[s] for s in doc.sections])
        labels[doc] ← doc_labels

    return labels
```

#### 4.4.3 Propagation Paths

```
Example propagation in a PKM vault:

/react/
├── hooks.md
│   ├── ## useState ◄─── Seed: extracts ["React Hooks", "State Management"]
│   └── ## useEffect
└── components.md
    ├── ## Button
    └── ## Form

Propagation:
1. useState → useEffect (sibling, weight=0.8)
   "React Hooks" propagates to useEffect section

2. hooks.md → components.md (sibling files, weight=0.5)
   "React Hooks" propagates to components.md (lower confidence)

3. hooks.md sections → hooks.md (roll-up)
   Document gets aggregated concept labels

4. If components.md links to hooks.md#useState:
   "State Management" propagates via LinksTo (weight=0.7)
```

---

### 4.5 Phase 5: Ontology Crystallization

**Goal**: Build a semantic hierarchy that complements the structural hierarchy.

**Key insight**: We now have TWO hierarchies:
1. **Structural** (already exists): Context → Dir → Doc → Section → Block
2. **Semantic** (to build): Abstract Category → Category → Concept

**Zoom operates on both dimensions**:
- Structural zoom: drill into sections, see blocks
- Semantic zoom: collapse concepts into categories

**Algorithm**:
```
CRYSTALLIZE_ONTOLOGY(G, concepts, embedding_model):
    // Step 1: Embed concepts (unchanged)
    embeddings ← {}
    for each concept c in concepts:
        embeddings[c] ← embedding_model.embed(c.name + " " + c.context)

    // Step 2: Cluster concepts into semantic categories
    distance_matrix ← cosine_distances(embeddings)
    dendrogram ← agglomerative_cluster(distance_matrix, linkage="ward")

    // Step 3: Create category nodes at multiple granularities
    for height in [0.3, 0.5, 0.7]:  // fine, medium, coarse
        clusters ← cut_dendrogram(dendrogram, height)

        for each cluster in clusters:
            category_node ← create_node(Category, level=height)
            G.add(category_node)

            // Link concepts to their category
            for each concept in cluster:
                G.add_edge(category_node, concept, Contains)

            // Name the category
            sample ← random_sample(cluster, k=5)
            name ← ensemble.name_category(sample)
            category_node.name ← name

    // Step 4: Link category hierarchy
    for each fine_category in categories[0.3]:
        parent ← find_containing_cluster(fine_category, categories[0.5])
        G.add_edge(parent, fine_category, Contains)

    return G
```

**Dual Hierarchy Visualization**:
```
STRUCTURAL                          SEMANTIC
──────────                          ────────
Context                             All Concepts
├── /frontend/                      ├── "Web Development"
│   ├── components.md               │   ├── "React Patterns"
│   │   ├── ## Props                │   │   ├── useState
│   │   └── ## State ──────────────────────► React Hooks
│   └── hooks.md                    │   │   └── useEffect
│       ├── ## useState             │   └── "State Management"
│       └── ## useEffect            │       └── Redux
└── /backend/                       └── "Backend"
    └── api.md                          └── "API Design"
        └── ## Endpoints                    └── REST
```

---

## 5. Complexity Analysis (Updated)

### 5.1 Variable Definitions

| Variable | Meaning | Typical Range |
|----------|---------|---------------|
| $n$ | Number of documents | 100 - 10,000 |
| $s$ | Average sections per document | 3 - 20 |
| $N = n \times s$ | Total sections (primary unit) | 300 - 200,000 |
| $d$ | Average links per section | 1 - 5 |
| $p$ | Sampling proportion | 0.10 - 0.20 |

### 5.2 Per-Phase Complexity

| Phase | Operation | Complexity | Notes |
|-------|-----------|------------|-------|
| 1 | Structural Bootstrap | $O(n \times s)$ | Parse all docs, extract sections |
| 2 | Section Scoring | $O(k \times \|E\|)$ | PageRank on section-level graph |
| 3 | Semantic Seeding | $O(p \times N)$ | LLM calls for sampled sections |
| 4 | Label Propagation | $O(T \times N \times d)$ | Multi-level propagation |
| 5 | Ontology | $O(\|C\|^2 \log \|C\|)$ | HAC clustering |

### 5.3 Key Difference from Document-Level

**Document-level**: $n$ = 1,000 docs, sample 150, propagate to 850

**Section-level**: $N$ = 5,000 sections, sample 750, propagate to 4,250

More granular, but:
- Better precision (concepts tied to specific locations)
- Better propagation (more edges, more paths)
- Same asymptotic complexity: $O(N \log N)$

---

## Next: [02-ensemble-validation.md](./02-ensemble-validation.md) — Ensemble Architecture & Validation
