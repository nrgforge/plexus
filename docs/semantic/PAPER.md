# Tree-First Semantic Extraction: Network Structure vs. File Hierarchy for Knowledge Graph Construction

**Nathaniel Green**
Independent Researcher
nate@nate.green
ORCID: 0000-0003-0157-7744

*Working Paper — January 2026*

---

## Abstract

We investigate whether file tree structure can replace link-based network algorithms for semantic extraction in personal knowledge management (PKM) corpora. The hypothesis seemed almost too simple: maybe the directory structure authors create already encodes the semantic relationships we're trying to discover. Through experimentation on structured (pkm-webdev) and unstructured (Shakespeare) corpora, we find this hypothesis largely holds. Tree traversal achieves 100% document coverage by construction; PageRank-based BFS achieves only 44-72%. More surprisingly, directory co-location provides 9.3× stronger semantic signal than explicit wikilinks—authors' implicit organization outperforms their explicit linking. LLM extraction achieves 80-100% grounding on technical corpora with 0% hallucination when using evidence-grounded prompts, though literary corpora pose challenges. Concept propagation works well within coherent directory subtrees (~70-80% appropriate) but poorly across arbitrary groupings (~30% overall), suggesting corpus organization quality matters more than parameter tuning. Performance profiling reveals a ~10s per-document latency floor on laptop hardware that is not explained by model size alone; micro models (1B) show negligible improvement over 7B. For structured corpora, the file hierarchy—not link-based network algorithms—should be the primary mechanism for document traversal and semantic proximity inference.

**Keywords:** knowledge graphs, semantic extraction, file hierarchy, personal knowledge management, LLM, label propagation, PageRank

---

## 1. Introduction

### 1.1 Problem Statement

Semantic extraction from document corpora faces a fundamental scaling challenge that I suspect most practitioners have encountered but few have quantified. The naive approach—send every document to an LLM, compare every pair for similarity—requires O(n) LLM calls plus O(n²) comparisons. For 1,000 documents this means roughly 500,000 comparisons; for 10,000 documents, around 50 million. These numbers become impractical fast.

But computational cost isn't even the main problem. The naive approach makes three questionable assumptions: that documents are atomic units (they're not—a 50-page document contains many distinct topics), that we should ignore file/folder organization (but this organization *is* semantic signal, placed there by the author), and that we don't need hierarchical zoom (we do—users want to move from "big picture" to "specific detail").

### 1.2 The Layered Insight

Here's what I think the field has been missing: **documents are not the atomic unit of semantics.** Structure exists at multiple levels—context, directory, document, section, concept—and each level carries semantic meaning. A corpus contains directories; directories contain documents; documents contain sections; sections contain concepts.

```
Context (corpus/vault)
├── Directory structure ──────── Implicit clustering: siblings are related
│   └── Documents ──────────── Explicit links (wikilinks, imports)
│       └── Sections ────────── Topical boundaries within doc
│           └── Concepts ────── Named entities, ideas, terms
```

The key insight, and it's almost embarrassingly simple once you see it: structure IS semantics. Files in `/hooks/` are related—the author put them there together. Sections under "## Authentication" share a topic. We get semantic signal for free from structure we already have. Why are we computing similarities when the author already told us what's related?

### 1.3 Original Hypothesis

We initially hypothesized that **network science techniques** (PageRank [6], label propagation [5], community detection [7]) would solve the coverage problem efficiently. The approach assumed:

- **H1**: Network-guided sampling at p=0.15 of sections achieves ≥85% coverage
- **H2**: PageRank identifies semantically rich seed documents
- **H7**: Directory sibling edges provide semantic signal (precision ≥50%)

### 1.4 Investigation Pivot

Our spike investigation revealed that **H2 was fundamentally wrong**. PageRank-based seed selection achieved only 44-72% coverage—nowhere near the 85% we'd hoped for. But testing H7 revealed something I hadn't expected at all:

> **Tree structure doesn't just provide signal—it provides COMPLETE coverage and STRONGER signal than links.**

This was surprising enough to force a complete pivot. We stopped asking "how do we improve PageRank coverage?" and started asking different questions entirely: Does tree structure provide equivalent or better coverage than link-based traversal? (RQ1) Is directory co-location a stronger semantic signal than explicit links? (RQ2) How does corpus structure affect these findings? (RQ3) And what propagation parameters work best for tree-structured corpora? (RQ4)

### 1.5 Contributions

1. Empirical demonstration that tree traversal obsoletes PageRank for document coverage
2. Quantitative measurement showing siblings provide 9.3× stronger semantic signal than links
3. Validation of LLM extraction quality (80-100% grounding on technical corpora)
4. Identification of flat corpus limitations requiring content-only fallback
5. Performance characterization: ~10s/doc latency floor is not explained by model size (1B ≈ 7B)
6. Propagation insight: effectiveness depends on corpus semantic coherence, not parameter tuning

---

## 2. Related Work

### 2.1 Knowledge Graph Construction

Recent systems for document-to-knowledge-graph construction share a common assumption: analyze every document with an LLM, then build relationships.

| System | Approach | Limitation |
|--------|----------|------------|
| **Microsoft GraphRAG** [1] | Extract entities → community detection → hierarchical summaries | Expensive (all docs), costly incremental updates |
| **LightRAG** [2] | Graph + embedding retrieval with incremental updates | Still extracts from every document |
| **Neo4j LLM Graph Builder** [3] | Multi-LLM extraction to graph database | Processes every document, no tree-aware sampling |

All treat documents as atomic units. They build relationships from scratch rather than exploiting the structural organization that already exists in the corpus.

### 2.2 Network Science in Document Analysis

**InfraNodus** [4] applies network science (betweenness centrality, modularity) to personal knowledge management. It builds co-occurrence graphs and identifies "structural gaps" between topic clusters.

This is the closest prior work to our initial hypothesis. However, InfraNodus:
- Works on single vaults, not multi-context
- Analyzes each note independently (no propagation)
- Uses network structure for analysis, not for sampling efficiency

Our investigation revealed that network science solves the wrong problem: coverage is already solved by the file tree.

### 2.3 Label Propagation

Semi-supervised label propagation [5] is well-established in machine learning for spreading labels from a small set of annotated examples to unlabeled data. However, no existing knowledge graph system applies label propagation to concept spreading.

Our contribution validates that propagation works for semantic concepts within coherent domains (~70-80% useful), but overall effectiveness across diverse corpora is lower (~30%) and depends more on corpus semantic structure than parameter tuning. Our key finding is that **sibling edges** (directory co-location) provide 9.3× stronger signal than explicit links—inverting the typical assumption that explicit relationships are more valuable.

### 2.4 LLM-Based Extraction

Recent work on LLM extraction focuses on prompting strategies and hallucination reduction. Our validation pyramid (L1 schema, L2 grounding, L3 semantic) follows this trend, but our finding that 0% hallucination is achievable on technical corpora with evidence-grounded prompts suggests simpler approaches may suffice.

---

## 3. Method

### 3.0 Experimental Setup

All experiments were conducted on consumer laptop hardware:
- **Hardware**: MacBook Pro M2 Pro, 16GB unified memory
- **OS**: macOS Sonoma 14.x
- **LLM Runtime**: Ollama 0.5.x
- **Models**: llama3:8b-instruct-q4_0 (4.7GB), gemma3:1b (815MB)
- **Temperature**: 0.0 (deterministic output)
- **Context**: Default (4096 tokens)

Results may vary on different hardware configurations. GPU-accelerated inference or cloud APIs would likely show different latency characteristics.

### 3.1 Corpora

| Corpus | Files | Structure | Description |
|--------|-------|-----------|-------------|
| pkm-webdev | 50 | Deep tree (28 dirs) | Web development knowledge base |
| arch-wiki | 2,487 | Medium | Arch Linux wiki subset |
| shakespeare | 43 | Flat (1 dir) | Complete plays |

### 3.2 Graph Construction

Documents are parsed into a multi-level graph:
- **Structural edges**: parent_of, contains, sibling
- **Semantic edges**: links_to, references
- **Derived edges**: linked_from (reverse), contained_by (reverse)

### 3.3 Traversal Strategies

| Strategy | Description |
|----------|-------------|
| PageRank + BFS | Select top-k seeds by PageRank, expand via BFS |
| Random Walk | Probabilistic exploration with restart |
| Stratified Sampling | One document per directory |
| Tree Traversal | Depth-first walk of file hierarchy |

### 3.4 Semantic Extraction

LLM-based concept extraction using LLaMA 3 [8] via Ollama [9] with structured prompts requiring evidence grounding.

### 3.5 Metrics

| Metric | Definition |
|--------|------------|
| Coverage | % of documents reachable from seeds |
| Jaccard Similarity | Concept overlap between document pairs |
| Grounding Rate | % of concepts with textual evidence |
| Propagation Usefulness | % of propagated concepts judged semantically appropriate |

---

## 4. Results

### 4.1 Tree Structure vs. Network Algorithms (RQ1)

**Finding**: Tree traversal achieves 100% coverage by construction; PageRank-based approaches achieve 44-72%.

| Approach | Coverage | Complexity |
|----------|----------|------------|
| PageRank BFS (5 seeds) | 44% | O(k×n×d) |
| PageRank BFS (10 seeds) | 58% | O(k×n×d) |
| Random Walk (p=0.15) | 72% | Probabilistic |
| **Stratified (1/dir)** | **100%** | O(n) |
| **Tree Traversal** | **100%** | O(n) |

**Evidence**: EXPERIMENT-LOG.md, Investigation 2d

**Interpretation**: This result felt almost too obvious in hindsight. The file tree is inherently a fully connected graph—every document belongs to a directory, every directory has a parent up to root. The "coverage problem" that PageRank attempts to solve? The file system already solved it. We'd been trying to optimize our way around a problem that didn't need to exist.

**Note on baseline selection**: PageRank wasn't designed as a coverage algorithm—it measures node importance/centrality. We tested it because Microsoft GraphRAG [1] and similar systems use PageRank-based seed selection, making it the de facto industry baseline. A coverage-optimal algorithm (e.g., dominating set) would likely perform better than PageRank but still couldn't exceed tree traversal's 100% by construction.

### 4.2 Sibling vs. Link Semantic Signal (RQ2)

**Finding**: Directory co-location (siblings) provides 9.3× stronger semantic signal than explicit wikilinks.

| Relationship | Mean Jaccard | % With Overlap | vs. Random |
|--------------|--------------|----------------|------------|
| Siblings | 0.1108 | 44.4% | **9.3×** |
| Linked | 0.0119 | 13.3% | 1.8× |
| Random | 0.0067 | 6.7% | 1.0× |

**Evidence**: EXPERIMENT-LOG.md, Investigation 3 (lines 554-570)

**Interpretation**: This 9.3× difference surprised me. Authors organize related content into directories—that's not controversial. But I expected explicit wikilinks to carry more signal, since someone took the time to create them. Turns out, explicit cross-references often serve navigational purposes ("see also: X") rather than topical ones. The implicit structure the author created by *placing files together* is more reliable than the explicit links they added later.

**Statistical note**: The sibling vs. random comparison (0.1108 vs 0.0067) yields a large effect size (Cohen's d ≈ 0.8) and is statistically significant (p < 0.01, Mann-Whitney U). The sibling vs. linked comparison (0.1108 vs 0.0119) is also significant, though the linked sample is smaller (n=15 pairs with links vs n=45 sibling pairs). The 9.3× ratio should be interpreted as indicative rather than precise—the key finding is the order-of-magnitude difference, not the exact multiplier.

### 4.3 Extraction Quality

**Finding**: LLM extraction achieves 0% hallucination on technical corpora; ~5% on literary (inferred genre signals).

| Metric | pkm-webdev | shakespeare |
|--------|------------|-------------|
| Grounding rate | 100% | ~95% |
| Concepts/doc | 5.8 avg | Variable |
| Hallucination | 0% | ~5% |

**Evidence**: EXPERIMENT-LOG.md, Investigation 4-5

**Definitions and caveats**: "Hallucination" here means concepts that cannot be traced to any text in the source document. The 0% rate on technical corpora (n=50 documents, ~290 concepts) reflects the use of evidence-grounded prompts that require the LLM to cite specific text spans. The prompt template is available in the llm-orc ensemble configuration (`plexus-semantic.yaml`). This result may not generalize to other prompting strategies, larger corpora, or different LLM providers. The ~5% hallucination on literary content reflects inferred genre signals ("tragedy", "comedy") that aren't explicitly stated but are arguably reasonable inferences.

### 4.4 Propagation Effectiveness

**Finding**: Propagation usefulness depends heavily on corpus semantic coherence, not parameter tuning.

| Experiment | Methodology | Result |
|------------|-------------|--------|
| Investigation 6 (early) | Manual review by author, 10 samples | 67% useful |
| P1 (comprehensive) | LLM judgment (llama3 7B), 50 pairs, real extraction | **29% appropriate** |

**Methodology note**: Investigation 6 used manual human judgment (by the author), which may introduce bias toward favorable assessment. P1 used LLM-as-judge with a different prompt than extraction—the judge saw the source document, the propagated concept, and was asked whether the concept was "semantically appropriate" for that document. The judge model (llama3 7B) was the same family as the extraction model, which may introduce systematic bias. A blind human evaluation would provide stronger validation but was not conducted. The 67% vs 29% discrepancy is likely partly methodological (different judges) and partly sampling (different corpus subsets).

This discrepancy puzzled me until I looked at the sample distributions. Investigation 6 happened to draw from semantically coherent directory clusters—files that genuinely belonged together. P1's broader test hit the full corpus, including arbitrary pairings like Docker↔NordVPN siblings that exist only because someone's PKM vault isn't perfectly organized. The real insight: **corpus structure matters more than parameters**. You can tune decay and threshold all you want, but if the sibling relationship doesn't reflect actual semantic similarity, propagation will fail.

**Parameter sweep results (P1)**:
- Best parameters: decay=0.8, threshold=0.3, hops=3
- Appropriate propagations clustered in coherent domains (Gnome desktop, TypeScript, Dart)
- Cross-domain propagations (Docker→NordVPN) consistently failed

**Implication**: Propagation works well within semantically coherent subtrees but poorly across arbitrary directory groupings. Recommended defaults: decay=0.7, threshold=0.4, hops=3.

**Evidence**: spike_p1_llm_propagation.rs, EXPERIMENT-LOG.md Investigation 6

### 4.5 Compositional Extraction

**Finding**: Large documents can be processed via chunk→fan-out→aggregate→synthesize pipeline.

| Stage | Function | Validated |
|-------|----------|-----------|
| Chunker | Split by 150 lines, 20-line overlap | ✅ |
| Fan-out | Parallel extraction per chunk | ✅ llm-orc 0.13 |
| Aggregator | Combine chunk extractions | ✅ |
| Synthesizer | Document-level coherence | ✅ |

**Evidence**: SPIKE-OUTCOME.md, Experiment R4b (Macbeth: 500 lines → 4 chunks → coherent synthesis)

**Interpretation**: LLMs handle partial sentences at chunk boundaries; the aggregator reconciles overlapping concepts. Line-based chunking provides provenance without format detection complexity.

### 4.6 Ensemble Experiments Summary

Individual experiments validated specific extraction strategies:

| Experiment | Purpose | Verdict | Key Finding |
|------------|---------|---------|-------------|
| A: Two-Stage Refiner | Filter over-specific concepts | ✅ Effective | Removes 60-75% noise |
| B: Propagation-Aware | Prompt for index pages | ✅ Highly effective | Eliminates sibling-specific concepts |
| C: Normalization | LLM-based dedup | ⚠️ Partial | Case normalization safe; semantic dedup risky |
| D: Calibration | Rule-based confidence | ⚠️ Partial | 100% precision at ≥0.9 threshold |
| E: Hierarchical | Tree-informed extraction | ✅ Highly effective | Tree structure enables domain inference |

**Evidence**: ENSEMBLE-EXPERIMENTS.md

### 4.7 Flat Corpus Limitation (RQ3)

**Finding**: All structural signals fail for flat corpora (Shakespeare).

| Signal | pkm-webdev | shakespeare |
|--------|------------|-------------|
| Sibling correlation | 9.3× | 0× |
| Link correlation | 1.8× | 0× |
| Tree utility | High | None |

**Evidence**: EXPERIMENT-LOG.md, Investigation 3b

**Interpretation**: When all documents reside in a single directory, sibling relationships become meaningless (everyone is siblings). Content analysis becomes the only viable path.

### 4.8 Three-System Architecture

**Finding**: Optimal system design separates extraction, provenance, and graph storage.

| System | Role | Benefit |
|--------|------|---------|
| llm-orc | Orchestration | Stateless extraction, fan-out handling |
| clawmarks | Provenance | file:line → concept, evidence tracking |
| plexus | Knowledge graph | Semantic relationships, cross-document edges |

**Evidence**: SPIKE-OUTCOME.md, Architecture section

**Interpretation**: This separation enables "go to source" UX (click concept → open file at line), extraction sessions as queryable trails, and graceful degradation (each system works independently).

---

## 5. Discussion

### 5.1 When Tree Structure Helps

These findings don't generalize to all corpora—that would be too convenient. Tree structure works best when the corpus is author-organized into topic directories (which PKM vaults typically are), when directory depth exceeds 2 levels (giving enough hierarchy to exploit), and when directories contain fewer than 20 documents (so "sibling" still means something specific). The pkm-webdev corpus met all three conditions; the Shakespeare corpus met none.

### 5.2 When Tree Structure Fails

Tree structure provides no useful signal in several common scenarios. Flat corpora (everything in one directory) make sibling relationships meaningless—if everyone's siblings, no one is. Arbitrary organization by date or author rather than topic breaks the semantic assumption entirely. And some corpora just don't have hierarchical structure; code repositories and literary collections often fall into this category.

### 5.3 Implications for System Design

If I were building a knowledge graph system from scratch with these findings, I'd start with tree traversal for document selection—PageRank is just unnecessary overhead when you can walk the tree. I'd weight sibling edges higher than explicit links, probably by that 9.3× factor or something close to it. Link-based algorithms still have their place, but for specific use cases: hub detection, cross-branch discovery, not primary traversal. And I'd build in a content-only fallback from day one, because flat corpora exist and you'll hit one eventually.

### 5.4 Limitations

- **Literary content failure**: LLM extraction fails on long literary works (93% failure rate on Shakespeare). Requires chunking or content-type-specific handling.
- **Single LLM provider**: All experiments used Ollama on laptop hardware. Cloud APIs or dedicated GPU hardware may show different characteristics.
- **German content**: pkm-datascience has German-language documents which may affect grounding measurements.
- **Latency targets unachievable**: S1 proved that <5s targets are unrealistic on 7B models with laptop hardware (~9s minimum floor). S2 showed concurrency provides only ~1.5× speedup with diminishing returns.
- **Model size is not the bottleneck**: Micro model experiments (1B vs 7B) showed negligible latency improvement. The cause of the ~9s floor is unclear without detailed profiling.
- **Tags and metadata not examined**: Many PKM systems rely heavily on `#tags` and YAML frontmatter for organization. These explicit semantic signals were not included in our analysis. Tags might provide stronger signal than wikilinks (since they're explicitly topical), but this remains untested.
- **Single-author corpora**: All test corpora were created by single authors with consistent organizational habits. Multi-author corpora or imported/aggregated content may show different sibling correlation patterns.

---

## 6. Experiment Status

| ID | Experiment | Purpose | Status |
|----|------------|---------|--------|
| P1 | Propagation parameter sweep | Find optimal decay, hops, threshold | **Complete** (29% appropriate, decay=0.7-0.8, threshold=0.3-0.5) |
| P2 | Multi-corpus extraction | Validate generalization | **Complete** (see 6.1) |
| P3 | Normalization ablation | Identify safe transforms | **Complete** (see 6.2) |
| S1 | Latency profiling | Validate performance claims | **Complete** (see 6.3) - TARGETS NOT MET |
| S2 | Concurrency testing | Find safe parallelism | **Complete** (see 6.4) - Limited benefit |
| S1/S2-Micro | Model size comparison | Test if smaller model helps | **Complete** (see 6.5) - No significant improvement |

### 6.1 P2: Multi-Corpus Extraction Results

**Research Question**: Does LLM extraction generalize across corpus types?

| Corpus | Documents | Grounding % | Concept Types | Status |
|--------|-----------|-------------|---------------|--------|
| pkm-webdev | 50 | **100%** | technology (36), topic (22) | Excellent |
| pkm-datascience | 517 | **80.7%** | technology (36), topic (24) | Good |
| shakespeare | 43 | **6.7%** (1/15 success) | - | **FAILURE** |

**Key Finding**: Literary corpus exhibits a clear failure mode. The LLM returns prose summaries instead of JSON for long literary works:
- "This is Act 5 of William Shakespeare's play..." instead of concept JSON
- Occurs consistently on full plays (>10KB content)
- One success was a short poem (the-phoenix-and-turtle)

**Implication**: Content-type detection is necessary. Long literary works require:
1. Chunking into smaller segments before extraction
2. Content-type-specific prompts for literary analysis
3. Or: explicit fallback to structural-only analysis

**Variance Analysis**: Tech corpora show 19.3% variance in grounding (100% vs 80.7%), which is acceptable. The pkm-datascience corpus has more German-language content and specialized ML terminology, which may explain lower grounding.

### 6.2 P3: Normalization Ablation Results

**Research Question**: What level of concept normalization is safe vs destructive?

| Level | Unique Before | Unique After | Merges | Precision |
|-------|---------------|--------------|--------|-----------|
| none | 81 | 81 | 0 | 100% |
| case-only | 81 | 81 | 0 | 100% |
| +singular | 81 | 81 | 0 | 100% |
| +semantic | 81 | 81 | 0 | 100% |

**Key Finding**: Zero merge candidates found across all normalization levels. The LLM extraction already produces normalized concepts implicitly—it outputs lowercase, consistent terminology without explicit post-processing.

**Why zero merges?** This result initially seemed suspicious—surely 81 concepts should have some near-duplicates? On investigation:
- The extraction prompt requests specific concept types (technology, topic, pattern), which constrains output
- LLM tends to use canonical forms ("react" not "React" or "ReactJS")
- The corpus (pkm-webdev) uses consistent terminology (author's personal notes)
- Example concepts extracted: "typescript", "react hooks", "state management"—distinct by design

This finding may not hold for corpora with inconsistent terminology, multiple authors, or prompts that don't constrain concept types. The implication is narrow: *for this extraction strategy on this corpus type*, explicit normalization adds no value.

**Implication**:
- Explicit normalization layers add no value when using LLM extraction with constrained prompts
- Case-only normalization is the conservative default (validated in Experiment C)
- Singularization and semantic normalization are unnecessary overhead for this use case
- The LLM acts as an implicit normalizer during extraction

### 6.3 S1: Latency Profiling Results

**Research Question**: What is the actual latency distribution for LLM extraction on laptop hardware?

| Metric | Result | Target | Status |
|--------|--------|--------|--------|
| p50 (median) | **11.9s** | <5s | ❌ FAIL |
| p95 | **16.7s** | <10s | ❌ FAIL |
| p99 | 24s | - | - |
| Min | 4.9s | - | - |
| Max | 24s | - | - |
| Throughput | 3.8 docs/min | - | Sequential baseline |

**Key Findings**:
- Strong size-latency correlation (r=0.705): `latency ≈ 9.2s + 1.8ms × size_bytes`
- ~9s baseline is LLM inference floor regardless of document size
- 23.4% parse failure rate (LLM not returning valid JSON)
- The ~4s seen in earlier experiments was on the fast tail

**Implication**: The speculative latency targets (<5s single doc) are NOT achievable on 7B models with current hardware. Architecture must assume:
- Background processing (never block UI)
- Aggressive caching (re-extract only on change)
- Incremental processing (one doc at a time on save)

### 6.4 S2: Concurrency Testing Results

**Research Question**: How does a 7B model perform under concurrent load on typical laptop hardware?

| Concurrency | Throughput | Mean Latency | Error Rate | Speedup |
|-------------|------------|--------------|------------|---------|
| 1 | 6.9/min | 8.8s | 25% | 1.00x |
| 2 | 8.4/min | 13.0s | **20%** | 1.21x |
| 4 | 8.6/min | 22.8s | 20% | 1.25x |
| 6 | 8.8/min | 31.9s | 25% | 1.28x |
| 8 | 10.3/min | 32.7s | **35%** | 1.49x |

**Key Findings**:
- Throughput plateau at ~8-10 docs/min regardless of concurrency
- Maximum speedup ~1.5x (far below theoretical 8x)
- Individual request latency degrades significantly (8.8s → 32.7s)
- Error rate spikes at 8 workers (35%) - hitting resource limits
- Sweet spot: **2 workers** (lowest error rate, reasonable speedup)

**Recommendation**: `max_concurrent = 2` for stability on laptop hardware. Higher concurrency yields minimal throughput gain but significantly degrades individual request latency and reliability.

### 6.5 Micro Model Comparison (1B vs 7B)

**Research Question**: Does a smaller model (gemma3:1b, 815MB) provide better performance than llama3 7B (4.7GB)?

**S1-Micro: Latency Results**

| Metric | 7B (llama3) | 1B (gemma3) | Speedup |
|--------|-------------|-------------|---------|
| p50 | 11.9s | 10.8s | 1.1x |
| p95 | 16.7s | **17.9s** | 0.9x (worse) |
| Failure rate | 23% | **28%** | Worse |
| Throughput | 3.8/min | 3.5/min | 0.9x (slower) |

**S2-Micro: Concurrency Results**

| Conc | 7B Thru | 1B Thru | 7B Errors | 1B Errors |
|------|---------|---------|-----------|-----------|
| 1 | 6.9/min | 7.7/min | 25% | 20% |
| 2 | 8.4/min | 7.9/min | 20% | 25% |
| 4 | 8.6/min | 9.0/min | 20% | 30% |
| 6 | 8.8/min | 11.8/min | 25% | **60%** |
| 8 | 10.3/min | 14.8/min | 35% | **65%** |

**Key Findings**:
- **Smaller model does NOT significantly improve latency** - the ~1.1x improvement is negligible
- p95 latency is actually WORSE with the 1B model (17.9s vs 16.7s)
- Higher error rates at all concurrency levels (1B struggles with JSON output)
- 1B scales slightly better under load (1.4x throughput at conc 8) but at the cost of 65% errors
- **Bottleneck is NOT model size** - the cause is unclear without detailed profiling (could be Ollama overhead, memory bandwidth, tokenization, or other factors)

**Implication**: Switching to a smaller model does not solve the latency problem on laptop hardware. The ~9s minimum latency appears to be an infrastructure/overhead floor rather than model inference time. Architecture recommendations remain unchanged:
- max_concurrent = 2 for both 7B and 1B
- Background processing required regardless of model size
- 7B recommended over 1B for better output quality (lower parse failure rate)

---

## 7. Conclusion

The main finding here is almost anticlimactic: for structured document corpora, the file tree provides both complete coverage and superior semantic signal compared to link-based network science approaches. We spent considerable effort testing PageRank and related algorithms only to discover they solve the wrong problem. They optimize for "importance" in a link graph when the actual need is coverage and semantic proximity—both of which the tree provides trivially, by construction.

This finding suggests a "tree-first" architecture for knowledge graph construction:
1. Walk the tree for document selection (100% coverage, O(n))
2. Use sibling co-location for semantic proximity (9.3× signal strength)
3. Reserve LLM analysis for content extraction, not graph traversal
4. Fall back to content-only analysis for flat corpora

**A note on performance**: I had hoped that local LLM inference would be fast enough for interactive use. It isn't—not yet, anyway. On laptop hardware with local 7B models, expect around 10 seconds per document with 3-8 docs/minute throughput. I tried smaller models thinking the bottleneck was inference time, but switching from 7B to 1B gave negligible improvement. Without detailed profiling I can't pinpoint the cause—it could be Ollama HTTP overhead, memory bandwidth, tokenization, or something else in the stack. What I can say is that model size alone doesn't explain the ~9s floor. For now, this means background processing and aggressive caching are mandatory for any interactive UX. For batch processing, two concurrent workers seems to be the sweet spot—more than that just increases error rates without proportional throughput gains.

**Propagation caveat**: Concept propagation via sibling edges works well within semantically coherent directory subtrees (~70-80% appropriate) but poorly across arbitrary groupings (~30% overall). Effectiveness depends on corpus organization quality, not parameter tuning.

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
| S1-Micro latency | - | spike_s1_latency_micro.rs:1 |
| S2-Micro concurrency | - | spike_s2_concurrency_micro.rs:1 |

---

## Appendix B: Data Model

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

### Extraction Trail

```json
{
  "id": "trail_xyz",
  "name": "hamlet-extraction-2026-01-18",
  "clawmarks": ["clwk_abc123", "clwk_def456", ...]
}
```

---

## Appendix C: Experimental Status

All critical experiments completed. Status via clawmarks trail:

| Experiment | Clawmark | Status | Key Finding |
|------------|----------|--------|-------------|
| P1: Propagation params | c_r0ecn0pw | ✅ Complete | 29% appropriate, decay=0.7-0.8, threshold=0.3-0.5 |
| P2: Multi-corpus | c_59fufuod | ✅ Complete | Tech 80-100%, literary fails 93% |
| P3: Normalization | c_8hbmeguh | ✅ Complete | LLM normalizes implicitly, case-only sufficient |
| S1: Latency | c_jdo7vstn | ✅ Complete | p50=11.9s, p95=16.7s, targets NOT met |
| S2: Concurrency | c_bqeip67b | ✅ Complete | max_concurrent=2, 1.5× max speedup |
| S1/S2-Micro | c_uzoap1rn | ✅ Complete | 1B model not faster, bottleneck is infrastructure |

**Remaining untested (lower priority)**:
| Gap | Clawmark | Impact |
|-----|----------|--------|
| Batching small files | c_hjlh31io | Low - optimization detail |
| Content-type detection | c_5e0b5a02 | Low - acknowledged limitation |
| Incremental invalidation | c_yqfx005x | Low - implementation detail |
| Caching strategy | c_snivqlb8 | Low - implementation detail |

---

## Appendix D: Ensemble Selection Matrix

| Content Type | Size | Ensemble | Rationale |
|--------------|------|----------|-----------|
| Technical | < 3000 words | `plexus-semantic` | Direct extraction, 100% grounding |
| Technical | > 3000 words | `plexus-compositional` | Chunk→fan-out→aggregate |
| Literary | < 3000 words | `plexus-refinement` | Better categorization |
| Literary | > 3000 words | `plexus-compositional` | Same pipeline, literary prompts |
| Flat corpus | any | `plexus-refinement` | No tree signal available |
