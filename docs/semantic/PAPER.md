# Empirical Design of an LLM-Powered Knowledge Graph Construction System for Document Corpora

**Nathaniel Green**
Independent Researcher
nate@nate.green
ORCID: 0000-0003-0157-7744

*Working Paper — January 2026*

---

## Abstract

Building knowledge graphs from document corpora using local LLMs requires solving several interacting design problems: how to traverse documents, how to extract concepts reliably, how to handle documents that exceed context windows, how to spread concepts across related documents, how to normalize terminology, and how to operate within the performance constraints of consumer hardware. Rather than proposing a theoretical architecture, we ran targeted experiments against real corpora to answer each design question empirically. The result is a three-system architecture (orchestration, provenance, knowledge graph) whose design choices are grounded in experimental evidence. Key findings include: file tree traversal provides complete document coverage without the need for network algorithms; directory co-location provides 9.3× stronger semantic signal than explicit links; evidence-grounded prompts achieve 0% hallucination on technical corpora; compositional extraction via chunk-fan-out-aggregate handles large documents autonomously; concept propagation effectiveness depends on corpus organization quality rather than parameter tuning; and local 7B model inference has a ~10s per-document latency floor that is not explained by model size. We report both what worked and what failed, including a 93% extraction failure rate on literary corpora and the inability to meet interactive latency targets on laptop hardware.

**Keywords:** knowledge graphs, LLM extraction, system design, personal knowledge management, semantic extraction, document corpora

---

## 1. Introduction

### 1.1 The Problem

Personal knowledge management (PKM) systems accumulate large document corpora—wikis, notes, documentation—that lack semantic structure beyond what the author imposed through file organization. Building a knowledge graph from such corpora would enable concept search, semantic navigation, and cross-document discovery. But doing so with LLMs raises a series of practical design questions that existing literature largely answers with "process everything with an LLM and figure out the rest later."

We wanted something more principled. Specifically, we needed to decide:

1. **Traversal**: How do we select and order documents for processing?
2. **Extraction**: How do we pull concepts from documents with high fidelity?
3. **Composition**: How do we handle documents that exceed LLM context windows?
4. **Propagation**: How do we spread concepts to related documents without reprocessing them?
5. **Normalization**: How much post-processing do extracted concepts need?
6. **Performance**: What throughput and latency can we expect on consumer hardware?

Each question has multiple plausible answers. Rather than guessing, we ran targeted experiments on real corpora to find out.

### 1.2 Approach

We conducted a spike investigation consisting of 18 experiments across three corpora of different structure and content type. Each experiment was designed to answer a specific design question with measurable outcomes. The experiments were not planned as a single study; they evolved iteratively, with early results redirecting later investigations. We report the sequence honestly, including hypotheses that turned out to be wrong.

### 1.3 Contributions

1. A three-system architecture (orchestration → provenance → knowledge graph) whose every major design choice is backed by experimental evidence
2. Empirical answers to six design questions, including negative results (what didn't work)
3. A methodology for using targeted experiments to make system design decisions—applicable beyond this specific domain
4. Quantitative characterization of local LLM performance constraints on consumer hardware

---

## 2. Related Work

### 2.1 Existing Knowledge Graph Construction Systems

Recent systems share a common assumption: process every document with an LLM, then build relationships after the fact.

| System | Approach | Design Assumptions |
|--------|----------|-------------------|
| **Microsoft GraphRAG** [1] | Entity extraction → community detection → hierarchical summaries | All docs processed; PageRank for importance ranking |
| **LightRAG** [2] | Graph + embedding retrieval with incremental updates | All docs processed; no structural awareness |
| **Neo4j LLM Graph Builder** [3] | Multi-LLM extraction to graph database | All docs processed; documents are atomic units |

All three treat documents as atomic, independent units. None exploit the organizational structure that already exists in the corpus. Our experiments test whether that structure is useful—and find that it often provides more signal than the extraction itself.

### 2.2 Network Science in Document Analysis

**InfraNodus** [4] applies network science (betweenness centrality, modularity) to PKM corpora. It builds co-occurrence graphs and identifies structural gaps between topic clusters. This is the closest prior work to our initial hypothesis that network algorithms would be the right traversal mechanism. Our experiments showed this hypothesis was wrong for structured corpora.

### 2.3 Label Propagation

Semi-supervised label propagation [5] spreads labels from annotated examples to unlabeled data. No existing knowledge graph system applies this to concept spreading across documents. We tested it and found it works within semantically coherent directory subtrees but not across arbitrary groupings.

---

## 3. Experimental Setup

### 3.1 Hardware and Software

All experiments ran on consumer laptop hardware:
- **Hardware**: MacBook Pro M2 Pro, 16GB unified memory
- **LLM Runtime**: Ollama 0.5.x
- **Models**: llama3:8b-instruct-q4_0 (4.7GB), gemma3:1b (815MB)
- **Temperature**: 0.0 (deterministic output)

### 3.2 Corpora

| Corpus | Files | Structure | Content |
|--------|-------|-----------|---------|
| pkm-webdev | 50 | Deep tree (28 dirs) | Web development knowledge base |
| arch-wiki | 2,487 | Medium tree | Arch Linux wiki subset |
| shakespeare | 43 | Flat (1 dir) | Complete plays |

These corpora were chosen to represent different structural extremes: deep hierarchy, moderate hierarchy, and no hierarchy.

### 3.3 Orchestration Platform

Experiments used **llm-orc**, a local LLM orchestration tool that supports multi-agent ensembles, fan-out parallelism, and script-based preprocessing. Ensemble configurations are YAML files specifying agent chains with dependencies.

---

## 4. Design Questions and Experimental Answers

### 4.1 Traversal: How Should We Select Documents?

**Initial hypothesis**: Network science techniques (PageRank [6], label propagation [5], community detection [7]) would efficiently select high-value seed documents, achieving ≥85% coverage at 15% sampling.

**What we tested**: PageRank-based BFS with varying seed counts, random walk with restart, stratified sampling (one per directory), and depth-first tree traversal.

**Results**:

| Strategy | Coverage | Complexity |
|----------|----------|------------|
| PageRank BFS (5 seeds) | 44% | O(k×n×d) |
| PageRank BFS (10 seeds) | 58% | O(k×n×d) |
| Random Walk (p=0.15) | 72% | Probabilistic |
| Stratified (1/dir) | 100% | O(n) |
| Tree Traversal | 100% | O(n) |

PageRank-based seed selection achieved 44–72% coverage—well below our 85% target. The file tree achieves 100% by construction: every document belongs to a directory, every directory has a parent.

We also measured whether directory co-location provides semantic signal by comparing concept overlap (Jaccard similarity) across relationship types:

| Relationship | Mean Jaccard | % With Overlap | vs. Random |
|--------------|--------------|----------------|------------|
| Siblings (same directory) | 0.1108 | 44.4% | 9.3× |
| Linked (explicit wikilinks) | 0.0119 | 13.3% | 1.8× |
| Random pairs | 0.0067 | 6.7% | 1.0× |

The sibling vs. random comparison yields a large effect size (Cohen's d ≈ 0.8, p < 0.01, Mann-Whitney U). The 9.3× ratio should be read as order-of-magnitude, not precise—the linked sample is smaller (n=15 vs n=45).

**Design decision**: Walk the file tree for document selection. Weight sibling edges higher than explicit links. Reserve network algorithms for cross-branch discovery, not primary traversal.

**Boundary condition**: This fails completely for flat corpora. When all 43 Shakespeare plays sit in one directory, every document is siblings with every other, and the signal is zero. Flat corpora require content-only analysis (§4.6).

### 4.2 Extraction: How Do We Pull Concepts Reliably?

**What we tested**: LLM extraction using evidence-grounded prompts (requiring the model to cite specific text spans for each concept), across technical and literary corpora. We also tested five ensemble variations to improve extraction quality.

**Core extraction results**:

| Metric | pkm-webdev | pkm-datascience | shakespeare |
|--------|------------|-----------------|-------------|
| Grounding rate | 100% | 80.7% | 6.7% |
| Concepts/doc | 5.8 avg | Variable | — |
| Hallucination | 0% | ~19% | 93% failure |

"Hallucination" means concepts untraceable to source text. The 0% on technical corpora (n=50 docs, ~290 concepts) reflects evidence-grounded prompting. The literary corpus failed outright—the LLM returned prose summaries instead of JSON for long plays.

**Ensemble experiments** (A–E) tested refinements to the extraction pipeline:

| Experiment | What It Tested | Result | Design Impact |
|------------|---------------|--------|---------------|
| A: Two-Stage Refiner | Second LLM pass to filter noise | Removes 60–75% of over-specific concepts | Add refiner stage for content pages |
| B: Propagation-Aware | Prompt tuned for cross-doc usefulness | Eliminated sibling-specific concepts from index pages | Use different prompts for hub vs. leaf pages |
| C: Normalization | LLM-based deduplication | Case normalization safe; semantic dedup merged unrelated concepts | Keep normalization simple (§4.5) |
| D: Calibration | Rule-based confidence adjustment | 100% precision at ≥0.9 threshold (vs. 75% raw) | Apply calibration as post-processing |
| E: Hierarchical | Tree-informed multi-layer extraction | Avoided function names, discovered higher-level abstractions | Use corpus structure as extraction context |

Experiment E demonstrated that feeding the file tree structure to the LLM as context improved extraction quality. The model correctly inferred "web development, programming languages, software tools" from directory names alone, which guided it toward higher-level concepts and away from code-specific identifiers.

**Design decision**: Use evidence-grounded prompts as the primary extraction mechanism. Detect page type (index vs. content) and apply different ensemble configurations. Add a refiner stage for content pages. Feed tree structure as context for corpus-wide batch extraction.

### 4.3 Composition: How Do We Handle Large Documents?

**The problem**: Shakespeare plays are ~100k tokens each. Even shorter technical documents can exceed practical context windows. Experiment R4 initially used human-written summaries, which invalidated the autonomy claim.

**What we tested**: A chunk→fan-out→aggregate→synthesize pipeline. Documents are split into 150-line chunks with 20-line overlap. Each chunk is extracted independently in parallel, then results are aggregated and synthesized into a document-level representation.

**Results** (Macbeth, 500 lines → 4 chunks):

| Stage | Function | Validated |
|-------|----------|-----------|
| Chunker | Split by line count, overlap boundaries | Yes |
| Fan-out | Parallel extraction per chunk | Yes (via llm-orc) |
| Aggregator | Combine chunk extractions, reconcile overlaps | Yes |
| Synthesizer | Produce document-level coherent output | Yes |

Line-based chunking is deliberately simple—no format detection, no section-boundary heuristics. LLMs handle partial sentences at boundaries; the aggregator reconciles overlapping concepts.

**Design decision**: Use fixed-size line chunking with overlap. Process chunks in parallel via fan-out. This is the default path for any document exceeding 3,000 words.

### 4.4 Propagation: How Do We Spread Concepts?

**What we tested**: Concept propagation via sibling edges (directory co-location) using label propagation with decay. We ran a comprehensive parameter sweep (P1) testing decay values 0.5–0.9, thresholds 0.1–0.5, and hop counts 1–5.

**Results**:

| Evaluation Method | Scope | Appropriateness |
|-------------------|-------|-----------------|
| Manual review (author, n=10) | Coherent directory clusters | 67% appropriate |
| LLM-as-judge (n=50 pairs) | Full corpus | 29% appropriate |

The discrepancy is informative, not contradictory. The manual review happened to sample from semantically coherent directories (TypeScript files, Gnome desktop tools). The LLM judge hit the full corpus, including arbitrary pairings like Docker↔NordVPN that coexist in the vault only because someone's organizational habits are imperfect.

**Best parameters**: decay=0.8, threshold=0.3, hops=3. But the key finding is that **parameter tuning matters less than corpus organization quality**. Within coherent subtrees, propagation works at ~70–80% appropriateness. Across arbitrary groupings, it fails regardless of parameters.

**Design decision**: Enable propagation with conservative defaults (decay=0.7, threshold=0.4, hops=3). Expect it to work well only within well-organized directory subtrees. Do not invest in parameter optimization—invest in understanding corpus structure.

### 4.5 Normalization: How Much Post-Processing?

**What we tested**: Four levels of normalization on extracted concepts: none, case-only (lowercase), singularization (plural→singular), and LLM-based semantic deduplication.

**Results** (P3, 81 concepts from pkm-webdev):

| Level | Merges Found | Precision |
|-------|-------------|-----------|
| None | 0 | 100% |
| Case-only | 0 | 100% |
| +Singular | 0 | 100% |
| +Semantic | 0 | 100% |

Zero merges across all levels. This initially seemed suspicious—surely 81 concepts should have duplicates? On investigation: the evidence-grounded extraction prompt already produces normalized output. The LLM uses canonical lowercase forms and consistent terminology. The corpus (single-author PKM) reinforces this consistency.

The earlier Experiment C, which tested normalization on a different concept set, found case normalization safe but semantic deduplication dangerous (it incorrectly merged "git" with "tag").

**Design decision**: Apply case normalization only. Skip semantic deduplication—it introduces errors, and the LLM normalizes implicitly during extraction. This finding may not hold for multi-author corpora with inconsistent terminology.

### 4.6 Performance: What Can We Expect on Consumer Hardware?

**What we tested**: Latency profiling (S1), concurrency scaling (S2), and model size comparison (S1/S2-Micro) on local Ollama with both 7B and 1B models.

**Latency (S1)**:

| Metric | 7B (llama3) | 1B (gemma3) | Target |
|--------|-------------|-------------|--------|
| p50 | 11.9s | 10.8s | <5s |
| p95 | 16.7s | 17.9s | <10s |
| Failure rate | 23% | 28% | — |

Strong size-latency correlation (r=0.705): `latency ≈ 9.2s + 1.8ms × size_bytes`. The ~9s baseline is an inference floor regardless of document size.

Switching from 7B to 1B gave negligible improvement (1.1× median, with *worse* p95 and higher failure rate). The bottleneck is not model size—it may be Ollama HTTP overhead, memory bandwidth, tokenization, or something else we couldn't isolate without deeper profiling.

**Concurrency (S2)**:

| Workers | Throughput | Mean Latency | Error Rate | Speedup |
|---------|------------|--------------|------------|---------|
| 1 | 6.9/min | 8.8s | 25% | 1.0× |
| 2 | 8.4/min | 13.0s | 20% | 1.2× |
| 4 | 8.6/min | 22.8s | 20% | 1.3× |
| 8 | 10.3/min | 32.7s | 35% | 1.5× |

Throughput plateaus at ~8–10 docs/min regardless of concurrency. Maximum speedup is 1.5× (far below theoretical 8×). Error rates spike above 2 workers.

**Design decision**: Use 2 concurrent workers maximum. Assume background processing for all extraction—interactive latency targets (<5s) are not achievable on this hardware. Implement aggressive caching (content-hash addressed, re-extract only on change). Prefer the 7B model over 1B—better output quality with no meaningful latency penalty.

---

## 5. System Architecture

The experiments produced a three-system architecture where each component has a distinct responsibility:

```
Document ──► llm-orc ──► Clawmarks ──► Plexus
             (extract)    (provenance)   (knowledge graph)
```

| System | Responsibility | Why Separate |
|--------|---------------|--------------|
| **llm-orc** | Orchestrates LLM ensembles, handles chunking and fan-out | Stateless; extraction strategy changes independently of storage |
| **clawmarks** | Records WHERE each concept came from (file, line, evidence) | Enables "go to source" UX; extraction sessions are queryable trails |
| **plexus** | Stores WHAT concepts exist and HOW they relate | Graph traversal and cross-document edges; semantic dimension |

### 5.1 Extraction Pipeline

Document routing is based on content characteristics:

| Content Type | Size | Ensemble | Rationale |
|--------------|------|----------|-----------|
| Technical | < 3000 words | `plexus-semantic` | Direct extraction; 100% grounding validated |
| Technical | > 3000 words | `plexus-compositional` | Chunk→fan-out→aggregate (§4.3) |
| Literary | < 3000 words | `plexus-refinement` | Iterative taxonomy building |
| Literary | > 3000 words | `plexus-compositional` | Same pipeline, literary-tuned prompts |
| Flat corpus | any | `plexus-refinement` | No tree signal; content-only fallback |

For structured corpora, the pipeline is:

1. **Traverse** the file tree (depth-first, 100% coverage)
2. **Classify** each document (index page vs. content page, size threshold)
3. **Extract** concepts using the appropriate ensemble
4. **Record provenance** via clawmarks (file, line, evidence text)
5. **Store** concepts and relationships in the plexus graph
6. **Propagate** concepts to sibling documents with decay

### 5.2 Provenance Model

Every concept links back to its source through a clawmark:

```
Concept: "revenge" (confidence: 0.9)
    └── Clawmark: hamlet.txt:892
        └── Evidence: "May sweep to my revenge"
            └── Trail: hamlet-extraction-2026-01-18
```

This enables a "go to source" UX: click a concept node in the graph → open the file at the exact line where the concept was extracted. Extraction sessions are tracked as trails, making the provenance of every concept in the knowledge graph auditable.

### 5.3 Progressive Processing

To avoid blocking the user, analysis runs in three phases:

1. **Immediate** (<2s): Scan file tree, parse links, display navigable structural graph
2. **Background**: Extract concepts with priority queue (open file → high priority; deep leaves → low)
3. **Incremental**: On file change, re-extract the changed file, invalidate and re-propagate affected concepts

---

## 6. Discussion

### 6.1 What Worked

The most broadly applicable findings:

- **Structure is semantic signal.** Authors organize related content together. This isn't a novel observation, but quantifying it (9.3× stronger than explicit links) and building a system around it is useful. Existing KG systems ignore this signal entirely.
- **Evidence-grounded prompting eliminates hallucination on technical content.** Requiring the LLM to cite text spans is a simple, effective constraint. We saw 0% hallucination across 290 concepts on technical corpora.
- **Compositional extraction works autonomously.** Chunking + fan-out + aggregation handles large documents without human intervention, validating the approach for corpora with diverse document sizes.
- **The LLM is an implicit normalizer.** With constrained prompts, the model produces canonical concept forms without explicit post-processing. This surprised us and simplified the pipeline.

### 6.2 What Failed

- **PageRank for traversal.** Our original hypothesis. It optimizes for node importance, not coverage. The tree solves coverage trivially.
- **Literary corpus extraction.** 93% failure rate on Shakespeare. The LLM returns prose summaries instead of structured output for long literary texts. Content-type detection and specialized prompts are necessary.
- **Interactive latency.** We targeted <5s per document. Actual median is 11.9s with a ~9s floor that persists even with 1B models. Background processing is mandatory.
- **Semantic deduplication.** LLM-based concept merging incorrectly conflated unrelated concepts (e.g., "git" with "tag"). Simple case normalization is the safe ceiling.
- **Propagation across diverse directories.** 29% appropriateness on the full corpus, despite 67–80% within coherent subtrees. The technique works only when the directory structure reflects genuine semantic grouping.

### 6.3 When This Architecture Applies

The tree-first approach works best when:
- The corpus is author-organized into topic directories (PKM vaults typically are)
- Directory depth exceeds 2 levels
- Directories contain fewer than ~20 documents

It degrades gracefully: the system falls back to content-only analysis for flat corpora, but loses the structural signal that makes propagation and traversal efficient.

### 6.4 Limitations

- **Single LLM provider**: All experiments used Ollama on laptop hardware. Cloud APIs or dedicated GPUs may show different latency and quality characteristics.
- **Single-author corpora**: All test corpora were created by single authors with consistent organizational habits. Multi-author corpora may show different patterns.
- **Tags and metadata not examined**: Many PKM systems rely on `#tags` and YAML frontmatter. These explicit semantic signals were not included in our analysis and might provide stronger signal than wikilinks.
- **Small corpus for key claims**: The 9.3× sibling signal strength comes from a 50-file corpus. Larger-scale validation is needed.
- **LLM-as-judge bias**: Propagation evaluation (P1) used the same model family as extraction. A blind human evaluation would be more rigorous.

---

## 7. Conclusion

We set out to build an LLM-powered knowledge graph construction system and discovered that most of the interesting design questions had non-obvious answers. Network algorithms weren't needed for traversal. Explicit links carried less signal than directory structure. Smaller models weren't faster. Concept normalization was unnecessary. Propagation effectiveness was determined by corpus organization, not parameter tuning.

The resulting architecture is straightforward: walk the file tree, extract concepts with evidence-grounded prompts using appropriate ensembles for different document types, record provenance, store in a graph, and propagate cautiously within coherent subtrees. Each choice is backed by experiment rather than assumption.

For practitioners building similar systems, the meta-lesson may be more useful than the specific findings: targeted experiments on real corpora reveal design answers that intuition and literature review alone would miss. We expected PageRank to work and tree traversal to be naive. We expected explicit links to be the strongest signal. We expected smaller models to be faster. All three intuitions were wrong.

---

## References

[1] Edge, D., Trinh, H., Cheng, N., Bradley, J., Chao, A., Mody, A., Truitt, S., & Larson, J. (2024). From Local to Global: A Graph RAG Approach to Query-Focused Summarization. *arXiv preprint arXiv:2404.16130*.

[2] Guo, Z., Xia, L., Yu, Y., Ao, T., & Huang, C. (2025). LightRAG: Simple and Fast Retrieval-Augmented Generation. In *Findings of the Association for Computational Linguistics: EMNLP 2025*, pp. 10746-10761.

[3] Neo4j. (2024). LLM Knowledge Graph Builder. https://neo4j.com/labs/genai-ecosystem/llm-graph-builder/

[4] Paranyushkin, D. (2019). InfraNodus: Generating insight using text network analysis. In *Proceedings of the World Wide Web Conference 2019* (WWW '19), pp. 3584-3589.

[5] Zhu, X., Ghahramani, Z., & Lafferty, J. D. (2003). Semi-supervised learning using Gaussian fields and harmonic functions. In *Proceedings of the 20th International Conference on Machine Learning (ICML-03)*, pp. 912-919.

[6] Page, L., Brin, S., Motwani, R., & Winograd, T. (1999). The PageRank Citation Ranking: Bringing Order to the Web. *Stanford InfoLab Technical Report*.

[7] Blondel, V. D., Guillaume, J. L., Lambiotte, R., & Lefebvre, E. (2008). Fast unfolding of communities in large networks. *Journal of Statistical Mechanics: Theory and Experiment*, 2008(10), P10008.

[8] Meta AI. (2024). Llama 3 Model Card. https://github.com/meta-llama/llama3/blob/main/MODEL_CARD.md

[9] Ollama. (2024). Ollama: Run Large Language Models Locally. https://ollama.com/

---

## Appendix A: Evidence Trail

This paper's claims are tracked via clawmarks trail `t_0jihblgl`. Key evidence:

| Claim | Clawmark | Location |
|-------|----------|----------|
| Tree 100% coverage | c_4ek7eafz | EXPERIMENT-LOG.md:461 |
| Siblings 9.3× | c_2ckf3smk | EXPERIMENT-LOG.md:570 |
| Flat corpus fails | c_euu9kru0 | EXPERIMENT-LOG.md:659 |
| 0% hallucination | c_gi204l8l | EXPERIMENT-LOG.md:1068 |
| Propagation (early sample) | c_uvzsyc5s | EXPERIMENT-LOG.md:1057 |
| Tree IS semantic | c_wmi8ltd6 | ENSEMBLE-EXPERIMENTS.md:461 |
| Compositional works | c_l866p5e7 | SPIKE-OUTCOME.md:47 |
| P1 propagation params | c_r0ecn0pw | spike_p1_llm_propagation.rs:549 |
| P2 multi-corpus | c_59fufuod | spike_p2_multi_corpus.rs:1 |
| P3 normalization | c_8hbmeguh | spike_p3_normalization.rs:1 |
| S1 latency profiling | c_jdo7vstn | spike_s1_latency.rs:1 |
| S2 concurrency | c_bqeip67b | spike_s2_concurrency.rs:1 |
| S1-Micro latency | — | spike_s1_latency_micro.rs:1 |
| S2-Micro concurrency | — | spike_s2_concurrency_micro.rs:1 |

---

## Appendix B: Ensemble Experiments Detail

Five ensemble variations were tested to refine extraction quality:

| Experiment | Method | Key Result |
|------------|--------|------------|
| A: Two-Stage Refiner | Second LLM pass filters over-specific concepts | 60–75% noise removed; core concepts retained |
| B: Propagation-Aware | Prompt optimized for cross-doc usefulness | Eliminated sibling-specific concepts from hub pages |
| C: Normalization | LLM-based deduplication | Case normalization safe; semantic dedup merged "git" with "tag" |
| D: Calibration | Rule-based confidence adjustment | 100% precision at ≥0.9 (vs. 75% raw); code identifier penalty effective |
| E: Hierarchical | Tree structure fed as extraction context | Inferred domain taxonomy from directory names; avoided function-name extraction |

Three ensemble configurations were produced:

| Ensemble | Architecture | Best For |
|----------|-------------|----------|
| `plexus-semantic` | 1 agent, evidence-grounded | Short technical documents |
| `plexus-semantic-v2` | 2 agents (extractor → refiner) | Content pages with code |
| `plexus-semantic-propagation` | 1 agent, propagation-aware prompt | Index/hub pages |

See ENSEMBLE-EXPERIMENTS.md for full experimental details.

---

## Appendix C: Data Model

### Concept Node (Plexus)

```rust
Node {
    id: NodeId("concept:revenge"),
    node_type: "concept",
    content_type: ContentType::Concept,
    dimension: "semantic",
    properties: {
        "name": "revenge",
        "concept_type": "theme",
        "confidence": 0.9,
        "clawmark_id": "clwk_abc123",    // provenance link
        "extraction_trail": "trail_xyz", // session tracking
    },
}
```

### Clawmark (Provenance)

```json
{
  "id": "clwk_abc123",
  "trail_id": "trail_xyz",
  "file": "hamlet.txt",
  "line": 892,
  "annotation": "Hamlet vows revenge: 'May sweep to my revenge'",
  "tags": ["#theme", "#central"]
}
```

---

## Appendix D: Experiment Index

| ID | Experiment | Design Question | Status | Key Finding |
|----|------------|----------------|--------|-------------|
| Inv 1–3 | Graph connectivity, traversal, signal | Traversal | Complete | Tree > PageRank; siblings 9.3× > links |
| Inv 4–5 | LLM extraction quality | Extraction | Complete | 0% hallucination (technical), 93% failure (literary) |
| Inv 6 | Concept propagation | Propagation | Complete | 67% appropriate (coherent subtrees) |
| A–E | Ensemble variations | Extraction refinement | Complete | See Appendix B |
| R4/R4b | Flat corpus taxonomy | Composition | Complete | Compositional pipeline validated |
| P1 | Propagation parameter sweep | Propagation | Complete | 29% overall; corpus structure > parameters |
| P2 | Multi-corpus extraction | Extraction | Complete | 80–100% grounding (technical) |
| P3 | Normalization ablation | Normalization | Complete | LLM normalizes implicitly |
| S1 | Latency profiling | Performance | Complete | p50=11.9s, ~9s floor |
| S2 | Concurrency testing | Performance | Complete | max 2 workers, 1.5× speedup |
| S1/S2-Micro | Model size comparison | Performance | Complete | 1B not faster than 7B |
