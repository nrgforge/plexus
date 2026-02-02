# Implementation Plan

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 9. Implementation Plan

Each phase includes:
- **Files**: Code to create/modify
- **Acceptance Criteria**: Functional requirements
- **Research Instrumentation**: Logging for experiments (§14)
- **Validation**: Link to experiments and hypotheses

### 9.1 Phase 1 Implementation (Structural Bootstrap + Priority Sampling)

**Research Links**: H1 (Sampling Efficiency), H2 (Strategy-Corpus Fit), Exp1, Exp2

**Phase 1 now includes two sub-phases**:
- **1a**: Build multi-level graph (dirs, docs, sections)
- **1b**: Hybrid importance scoring (doc-level → section-level)

**Files to create/modify**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/graph/nodes.rs` | Multi-level node types (Context, Directory, Document, Section, Block) |
| `crates/plexus/src/graph/edges.rs` | Edge types (Contains, HasSection, Sibling, LinksTo) |
| `crates/plexus/src/parsing/sections.rs` | Parse documents into sections by headings |
| `crates/plexus/src/parsing/links.rs` | Extract wikilinks, markdown links, anchors |
| `crates/plexus/src/analysis/importance/mod.rs` | `ImportanceStrategy` trait definition |
| `crates/plexus/src/analysis/importance/pagerank.rs` | PageRank strategy (doc-level) |
| `crates/plexus/src/analysis/importance/hits.rs` | HITS strategy (hub/authority) |
| `crates/plexus/src/analysis/importance/hybrid.rs` | Doc score → Section distribution |
| `crates/plexus/src/analysis/sampling.rs` | Seed SECTION selection |
| `crates/plexus/src/experiments/sampling.rs` | Experiment harness for Exp1, Exp2 |

**Acceptance Criteria (1a - Structural Bootstrap)**:
- [ ] Directory nodes created with depth tracking
- [ ] Documents parsed into Section nodes (by heading boundaries)
- [ ] Contains edges: Dir → Doc, Doc → Section
- [ ] Sibling edges: same-parent nodes (configurable weights)
- [ ] LinksTo edges: explicit links with optional #anchor
- [ ] Section boundaries detected for: Markdown (H1-H6), Code (fn/class), Literature (ACT/SCENE)

**Acceptance Criteria (1b - Priority Sampling)**:
- [ ] `ImportanceStrategy` trait implemented with `score()` at Document and Section levels
- [ ] Hybrid scoring: PageRank on docs → distribute to sections + features
- [ ] Section features: heading level, position, anchor inlinks, content length
- [ ] Sampling proportion `p` configurable (0.0-1.0), applied to SECTIONS
- [ ] Bridge sections (betweenness centrality) always included
- [ ] Unit tests achieve 80%+ coverage on importance module

**Research Instrumentation**:
```rust
pub struct StructuralMetrics {
    pub total_directories: usize,
    pub total_documents: usize,
    pub total_sections: usize,
    pub avg_sections_per_doc: f32,
    pub total_links: usize,
    pub links_with_anchors: usize,  // Section-level links
    pub sibling_edges_created: usize,
}

pub struct SamplingMetrics {
    pub strategy_name: String,
    pub corpus_sections: usize,  // N = n × s
    pub sample_proportion: f64,
    pub seeds_selected: usize,
    pub bridge_sections_added: usize,
    pub computation_time_ms: u64,
    pub seed_section_ids: Vec<NodeId>,
}
```
- [ ] Structural metrics logged for spike validation (Investigation 7, 8)
- [ ] Section-level seed rankings exportable for manual review

**Quantitative Targets**:
- Parsing time: < 500ms for 1K docs
- Section detection accuracy: ≥ 90% (manual sample check)
- Sampling time: < 1s for 5K sections, < 10s for 50K sections

### 9.2 Phase 2 Implementation (Section-Level Concept Extraction)

**Research Links**: H4 (Validation Effectiveness), H6 (Local Model Parity), Exp4, Exp5

**Key change**: Extract concepts from SECTIONS, not whole documents. For large sections, use chunked extraction with concept accumulation.

**Files**:

| File | Purpose |
|------|---------|
| `.llm-orc/ensembles/plexus-concept-extractor.yaml` | Section extraction ensemble |
| `.llm-orc/ensembles/plexus-chunked-extractor.yaml` | Chunked extraction for large sections |
| `.llm-orc/ensembles/plexus-concept-validator.yaml` | L3 validation ensemble |
| `crates/plexus/src/analysis/concepts.rs` | Concept normalization, deduplication |
| `crates/plexus/src/analysis/chunking.rs` | Section chunking strategies |
| `crates/plexus/src/analysis/accumulation.rs` | Cross-chunk concept accumulation |
| `crates/plexus/src/analysis/validation.rs` | L1/L2/L3 validation pipeline |
| `crates/plexus/src/experiments/extraction.rs` | Experiment harness for Exp4, Exp5 |
| `src-tauri/src/analysis.rs` | Tauri command for semantic analysis |

**Acceptance Criteria**:
- [ ] Concepts extracted from SECTIONS with confidence scores [0.0-1.0]
- [ ] Section→Concept (Discusses) edges created in graph
- [ ] Extraction prompt receives: section content, heading_path, document title, siblings
- [ ] Large sections (>2000 words) use chunked extraction
- [ ] Concept accumulation: merge across chunks, boost confidence for repeated concepts
- [ ] Concepts normalized (lowercase, lemmatization)
- [ ] L1 validation: JSON schema compliance (100%)
- [ ] L2 validation: Evidence grounding against SECTION text (not whole doc)
- [ ] L3 validation: Semantic check on high-importance sections
- [ ] Deduplication merges equivalent concepts (similarity > 0.9)

**Research Instrumentation**:
```rust
pub struct ExtractionMetrics {
    pub section_id: NodeId,
    pub parent_doc_id: NodeId,
    pub heading_path: String,
    pub section_words: usize,
    pub chunks_used: usize,  // 1 = no chunking
    pub model_name: String,
    pub concepts_extracted: usize,
    pub l1_pass_rate: f32,
    pub l2_pass_rate: f32,
    pub l3_pass_rate: Option<f32>,
    pub extraction_time_ms: u64,
    pub concepts: Vec<ConceptRecord>,
}
```
- [ ] Per-section extraction metrics logged
- [ ] Chunking decision logged (size threshold, chunks created)
- [ ] Raw concepts exportable for F1 computation vs ground truth

**Quantitative Targets**:
- L2 Grounding ≥ 85%
- Hallucination ≤ 10%
- Time < 500ms/section (single), < 2s/section (chunked)

---

### 9.3 Phase 3 Implementation (Multi-Level Label Propagation)

**Research Links**: H3 (Propagation Accuracy), Exp3

**Key change**: Propagation operates on the multi-level graph, using different edge weights per edge type.

**Files**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/network/propagation.rs` | Multi-level label propagation algorithm |
| `crates/plexus/src/network/edge_weights.rs` | Edge type → weight mapping |
| `crates/plexus/src/network/mod.rs` | Export propagation module |
| `crates/plexus/src/experiments/propagation.rs` | Experiment harness for Exp3 |

**Edge Weight Configuration**:
```rust
pub struct PropagationConfig {
    pub weights: HashMap<EdgeType, f32>,
    pub decay_per_hop: f32,
    pub threshold: f32,
    pub max_iterations: usize,
}

// Default weights
impl Default for PropagationConfig {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert(EdgeType::Sibling { same_doc: true }, 0.8);
        weights.insert(EdgeType::Sibling { same_doc: false }, 0.5);
        weights.insert(EdgeType::LinksTo { has_anchor: true }, 0.85);
        weights.insert(EdgeType::LinksTo { has_anchor: false }, 0.7);
        weights.insert(EdgeType::HasSection, 0.5);  // Parent→child inheritance
        weights.insert(EdgeType::Contains, 0.3);
        Self { weights, decay_per_hop: 0.8, threshold: 0.1, max_iterations: 15 }
    }
}
```

**Acceptance Criteria**:
- [ ] Labels propagate from seed SECTIONS to unlabeled sections
- [ ] Different edge types use different weights (configurable)
- [ ] Sibling edges (same doc) have higher weight than sibling (same dir)
- [ ] Parent→child propagation: doc concepts flow to sections
- [ ] Child→parent roll-up: section concepts aggregate to doc level
- [ ] Confidence decays with hop distance (configurable decay factor)
- [ ] Convergence in < 20 iterations (configurable)
- [ ] Hop distance from nearest seed tracked per section

**Research Instrumentation**:
```rust
pub struct PropagationMetrics {
    pub corpus_id: String,
    pub seed_section_count: usize,
    pub iterations_to_converge: usize,
    pub sections_labeled: usize,
    pub documents_with_labels: usize,  // After roll-up
    pub avg_confidence: f32,
    pub per_section_metrics: Vec<SectionPropagationRecord>,
}

pub struct SectionPropagationRecord {
    pub section_id: NodeId,
    pub parent_doc_id: NodeId,
    pub hop_distance: usize,
    pub primary_edge_type: EdgeType,  // How label arrived
    pub labels: Vec<(String, f32)>,
    pub source_seed: NodeId,
}
```
- [ ] Per-section hop distance, edge path, and confidence logged
- [ ] Edge type distribution tracked (what % via sibling vs linksTo vs parent)
- [ ] Propagated labels exportable for precision calculation vs LLM extraction

**Quantitative Targets**:
- Propagation Precision ≥ 70%
- Confidence Calibration ρ ≥ 0.5
- Sibling (same-doc) precision should exceed cross-doc precision

---

### 9.4 Phase 4 Implementation (Dual Hierarchy - Ontology)

**Research Links**: Ablation (ontology value)

**Key insight**: We now have TWO hierarchies:
1. **Structural** (from Phase 1): Context → Dir → Doc → Section
2. **Semantic** (this phase): Abstract Category → Category → Concept

Both support "zoom" but in different dimensions.

**Files**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/analysis/ontology.rs` | HAC clustering for concepts |
| `crates/plexus/src/graph/category.rs` | Category node type |
| `crates/plexus/src/graph/hierarchy.rs` | Dual hierarchy traversal |
| `.llm-orc/ensembles/plexus-category-namer.yaml` | Category naming ensemble |

**Acceptance Criteria**:
- [ ] Concepts embedded via local model (all-MiniLM-L6-v2 or similar)
- [ ] HAC clustering at multiple cut heights (fine, medium, coarse)
- [ ] Category nodes created with Contains edges to concepts
- [ ] Category hierarchy: coarse → medium → fine
- [ ] LLM-generated names for each category
- [ ] Structural hierarchy preserved: Context → Dir → Doc → Section
- [ ] Zoom API: `zoom_structural(level)` and `zoom_semantic(level)`
- [ ] Cross-hierarchy queries: "What categories does this directory cover?"

**Dual Zoom Implementation**:
```rust
pub enum ZoomDimension {
    Structural,  // Dir → Doc → Section → Block
    Semantic,    // Coarse Category → Category → Concept
}

pub struct ZoomState {
    pub structural_level: StructuralLevel,  // Directory, Document, Section, Block
    pub semantic_level: SemanticLevel,      // Coarse, Medium, Fine, Concept
}

impl Graph {
    /// Structural zoom: show nodes at specified level
    pub fn zoom_structural(&self, level: StructuralLevel) -> Vec<NodeId>;

    /// Semantic zoom: collapse concepts into categories at specified granularity
    pub fn zoom_semantic(&self, level: SemanticLevel) -> Vec<NodeId>;

    /// Combined: nodes at structural level, with semantic labels at semantic level
    pub fn zoom(&self, state: ZoomState) -> GraphView;
}
```

### 9.5 Phase 5 Implementation (UI Integration)

**Files**:

| File | Purpose |
|------|---------|
| `src/components/Plexus/SemanticControls.tsx` | UI for triggering analysis |
| `src/components/Plexus/ZoomControls.tsx` | Abstraction level selector |
| `src/hooks/usePlexusSemantic.ts` | React hooks for semantic features |

### 9.6 Phase 6 Implementation (Multi-Signal Edge Reinforcement)

**Prerequisite**: Phases 1-5 complete.

**Files**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/signals/mod.rs` | Signal types and aggregation |
| `crates/plexus/src/signals/structural.rs` | Explicit link signals |
| `crates/plexus/src/signals/semantic.rs` | LLM-derived signals |
| `crates/plexus/src/signals/behavioral.rs` | Navigation/co-edit signals |
| `crates/plexus/src/signals/contractual.rs` | Test-based signals |
| `crates/plexus/src/signals/statistical.rs` | Co-occurrence, PMI |

**Validation Criteria**:
- [ ] Signal types defined with weight ranges
- [ ] Edge strength computed from multiple signals
- [ ] Confidence increases with signal count
- [ ] Signals can be added/removed independently

### 9.7 Phase 7 Implementation (Test Integration for Contractual Signals)

**Prerequisite**: Phase 6 complete.

**Files**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/signals/test_parser.rs` | Parse test results (vitest, cargo test) |
| `crates/plexus/src/signals/component_mapping.rs` | Map tests → components exercised |
| `src-tauri/src/test_integration.rs` | Tauri commands for test result ingestion |

**Validation Criteria**:
- [ ] Passing tests reinforce edges between exercised components
- [ ] Failing tests weaken contractual signals
- [ ] Integration test detection identifies component pairs
- [ ] Pass rate history affects confidence

### 9.8 Phase 8 Implementation (Gap Detection)

**Prerequisite**: Phase 4 (Label Propagation) complete.

**Files**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/analysis/gaps.rs` | Gap detection algorithm |
| `crates/plexus/src/network/community.rs` | Community detection (Louvain) |

**Algorithm Reference**: Section 7.5 (gap_score formula)

**Validation Criteria**:
- [ ] Communities detected using Louvain algorithm
- [ ] Expected vs actual edges computed per community pair
- [ ] Gap score identifies disconnected but semantically related clusters
- [ ] Top-k gaps surfaced for narrator

### 9.9 Phase 9 Implementation (Real-Time Incremental Updates)

**Prerequisite**: Phases 3-4 complete.

**Files**:

| File | Purpose |
|------|---------|
| `crates/plexus/src/incremental/mod.rs` | Incremental update coordinator |
| `crates/plexus/src/incremental/diff.rs` | Change type classification |
| `crates/plexus/src/incremental/scope.rs` | Update scope determination |
| `src/hooks/usePlexusRealtime.ts` | Debouncing and async updates |

**Change Types** (from Section 13.5):
- Cosmetic → No update
- Content edit → Single doc edge update
- Structural edit → Local neighborhood + re-extract
- Major rewrite → Re-propagate from changed doc

**Validation Criteria**:
- [ ] 300ms debounce on keystrokes
- [ ] Change type correctly classified
- [ ] Scope limited to affected subgraph
- [ ] Fast path (< 100ms) for cosmetic/content edits

### 9.10 Phase 10 Implementation (Narrator Pane)

**Prerequisite**: Phases 6, 8, 9 complete.

**Files**:

| File | Purpose |
|------|---------|
| `src/components/Plexus/Narrator/index.tsx` | Narrator pane container |
| `src/components/Plexus/Narrator/InsightCard.tsx` | Individual insight display |
| `src/components/Plexus/Narrator/types.ts` | Insight type definitions |
| `crates/plexus/src/narrator/mod.rs` | Insight generation |
| `crates/plexus/src/narrator/insights.rs` | Insight type implementations |

**Insight Types** (from Section 13.2):
- Emerging structure (new concepts, connections)
- Edge reinforcement (multi-signal alignment)
- Structural gaps (disconnected related clusters)
- Hub formation (increasing centrality)
- Drift detection (document leaving cluster)

**Validation Criteria**:
- [ ] Ambient display (visible but not intrusive)
- [ ] Insights accumulate, expandable for detail
- [ ] Real-time updates on graph changes
- [ ] Dismissable insights

### 9.11 Implementation Roadmap Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      IMPLEMENTATION ROADMAP (MULTI-LEVEL)                     │
│                                                                              │
│  FOUNDATION (Phases 1-5)                                                     │
│  ─────────────────────────                                                   │
│  1. Structural Bootstrap ──► Multi-level graph (Dir→Doc→Section)            │
│     + Priority Sampling  ──► Hybrid scoring (doc PageRank → section)         │
│  2. Section Extraction   ──► LLM ensembles, chunking, validation            │
│  3. Label Propagation    ──► Multi-level propagation, edge-type weights     │
│  4. Dual Hierarchy       ──► Structural (existing) + Semantic (HAC)         │
│  5. UI Integration       ──► Dual zoom controls (struct × semantic)         │
│                                                                              │
│  EXPERIENCE (Phases 6-10)                          Requires Foundation       │
│  ────────────────────────                          ────────────────────      │
│  6. Multi-Signal Edges   ──► Signal types, aggregation      │ Phases 1-5    │
│  7. Test Integration     ──► Contractual signals            │ Phase 6       │
│  8. Gap Detection        ──► Community analysis             │ Phase 4       │
│  9. Real-Time Updates    ──► Incremental section updates    │ Phases 3-4    │
│  10. Narrator Pane       ──► Ambient insights UI            │ Phases 6,8,9  │
│                                                                              │
│  KEY DIFFERENCES FROM DOCUMENT-LEVEL:                                        │
│  • N = n × s sections (more nodes, finer granularity)                        │
│  • Extraction at section level (better context)                              │
│  • Chunking for large sections (Shakespeare acts, etc.)                      │
│  • Edge-type-specific propagation weights                                    │
│  • Two zoom dimensions (structural + semantic)                               │
│                                                                              │
│  Suggested Order:                                                            │
│  1 → 2 → 3 → 4 → 5 (serial, core algorithm)                                 │
│  6 → 7 (signal infrastructure)                                               │
│  8, 9 (parallel, independent)                                                │
│  10 (integrates all experience features)                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Next: [05-validation-strategy.md](./05-validation-strategy.md) — Validation Strategy
