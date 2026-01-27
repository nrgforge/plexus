# Research Methodology

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 14. Research Methodology

This section frames Plexus semantic integration as a research contribution with testable hypotheses, rigorous evaluation, and reproducible experiments.

### 14.1 Research Questions

| ID | Research Question | Spec Reference |
|----|-------------------|----------------|
| **RQ1** | Does network-guided sampling achieve comparable semantic coverage to full-corpus analysis at significantly lower computational cost? | [01-problem §4.2](./01-problem-algorithm.md), [03-complexity §7](./03-complexity-dataflow.md) |
| **RQ2** | Does hybrid scoring (doc-level PageRank → section features) outperform direct section-level PageRank? | [01-problem §4.2.1-4.2.3](./01-problem-algorithm.md) |
| **RQ3** | Does multi-level label propagation with edge-type weights maintain acceptable accuracy? | [01-problem §4.4](./01-problem-algorithm.md) |
| **RQ4** | Does the validation pyramid (L1/L2/L3) reduce hallucination rates in extracted concepts? | [02-ensemble §5](./02-ensemble-validation.md) |
| **RQ5** | What is the optimal sampling proportion $p$ for sections in the accuracy-cost tradeoff? | [01-problem §4.2](./01-problem-algorithm.md) |
| **RQ6** | Do micro-ensembles with small local models achieve comparable quality to large API models? | [02-ensemble §4, §6](./02-ensemble-validation.md) |
| **RQ7** | Does structural hierarchy (directory, heading) provide meaningful semantic signal? | [01-problem §2, §4.4](./01-problem-algorithm.md) |
| **RQ8** | Does chunked extraction with concept accumulation maintain quality for large sections? | [02-ensemble §4.5](./02-ensemble-validation.md) |

### 14.2 Hypotheses

**H1: Sampling Efficiency (Section-Level)**
> Network-guided sampling at $p = 0.15$ of SECTIONS achieves ≥85% of full-corpus semantic coverage while reducing LLM calls by 85%.

**H2: Hybrid Scoring Strategy**
> Hybrid scoring (doc-level PageRank → section distribution + features) outperforms pure section-level PageRank due to sparse section-level links.

**H3: Multi-Level Propagation Accuracy**
> Label propagation with edge-type-specific weights achieves ≥70% precision. Sibling (same-doc) propagation achieves higher precision than cross-document propagation.

**H4: Validation Effectiveness**
> The L1/L2/L3 validation pyramid reduces concept hallucination rate by ≥50% compared to unvalidated extraction.

**H5: Optimal Sampling**
> The accuracy-cost Pareto frontier has an elbow at $p \in [0.10, 0.20]$ of sections, beyond which additional sampling yields diminishing returns.

**H6: Local Model Parity**
> Quantized local models (qwen2.5:3b-7b) achieve ≥90% of the F1 score of claude-3-haiku on section-level concept extraction.

**H7: Structural Signal Value**
> Directory sibling edges provide meaningful semantic signal: precision of sibling propagation ≥ 50% (above random baseline).

**H8: Chunked Extraction Parity**
> Chunked extraction with concept accumulation achieves ≥95% of the F1 score of single-pass extraction on sections under 2000 words, while enabling extraction from arbitrarily large sections.

**H9: Edge-Weight Sensitivity**
> Default edge-type weights (sibling 0.8, links 0.7, directory 0.5, contains 0.3) are within 10% of optimal; weight tuning yields marginal improvement (≤5% F1 gain).

### 14.3 Test Corpora

| Corpus | Files | Est. Sections | Link Type | Characteristics | Tests |
|--------|-------|---------------|-----------|-----------------|-------|
| `manza-specs` | ~20 | ~100 | Wikilinks | Small, controlled, known ground truth | Development, sanity checks |
| `pkm-webdev` | 51 | ~250 | Wikilinks | Small PKM vault | H2, H7, H9 (hybrid scoring, sibling signal, edge weights) |
| `pkm-datascience` | 516 | ~2,500 | Wikilinks (dense) | Medium PKM, high link density | H3, H5, H9 (propagation, optimal p, edge weights) |
| `arch-wiki` | 2,487 | ~15,000 | Markdown links | Large wiki, hyperlink structure | H1, H2 (scaling, PageRank) |
| `shakespeare` | 42 | ~500 | None | Literature, no links, large sections | H6, H8 (semantic-only, chunking) |

**Note**: Section counts are estimates (avg ~5 sections/doc). Actual counts will be measured in spike Investigation 8.

**Ground Truth Generation**:
For validation, we need ground truth labels. Strategy:
1. **Manual annotation**: Small sample (50 docs) across corpora
2. **LLM-as-judge**: Use strong model (claude-opus-4-5) to generate reference extractions
3. **Inter-annotator agreement**: Measure consistency between manual and LLM labels

### 14.4 Evaluation Metrics

#### 14.4.1 Semantic Coverage Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| **Concept Recall** | $\frac{\|C_{sampled} \cap C_{full}\|}{\|C_{full}\|}$ | ≥ 0.85 |
| **Concept Precision** | $\frac{\|C_{sampled} \cap C_{full}\|}{\|C_{sampled}\|}$ | ≥ 0.80 |
| **Document Coverage** | Fraction of docs with ≥1 concept label | ≥ 0.95 |
| **Concept F1** | Harmonic mean of precision and recall | ≥ 0.82 |

Where:
- $C_{sampled}$ = concepts discovered via sampled pipeline
- $C_{full}$ = concepts from full-corpus LLM extraction (ground truth)

#### 14.4.2 Label Propagation Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| **Propagation Precision** | Fraction of propagated labels that match LLM extraction | ≥ 0.70 |
| **Propagation Recall** | Fraction of ground-truth labels recovered via propagation | ≥ 0.60 |
| **Confidence Calibration** | Correlation between confidence scores and actual correctness | ρ ≥ 0.5 |

#### 14.4.3 Efficiency Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| **LLM Call Reduction** | $1 - \frac{\text{calls}_{sampled}}{\text{calls}_{full}}$ | ≥ 0.80 |
| **Wall-Clock Time** | Total pipeline execution time | < 5 min (1K docs) |
| **Cost Reduction** | API cost savings vs full extraction | ≥ 0.85 |

#### 14.4.4 Validation Metrics

| Metric | Definition | Target |
|--------|------------|--------|
| **Hallucination Rate** | Fraction of extracted concepts not grounded in source | ≤ 0.10 |
| **L2 Grounding Score** | Fraction of concepts with evidence in source text | ≥ 0.85 |
| **Validation Overhead** | Additional LLM calls for L3 validation | ≤ 0.15 × extraction calls |

### 14.5 Experimental Design

#### Experiment 1: Sampling Efficiency (H1)

**Design**: Compare sampled vs full-corpus extraction across corpora.

```
Independent Variable: Sampling proportion p ∈ {0.05, 0.10, 0.15, 0.20, 0.30, 0.50, 1.0}
Dependent Variables: Concept F1, LLM calls, wall-clock time
Controls: Same LLM model, same extraction prompt, same corpus

Protocol:
1. For each corpus C in {pkm-datascience, arch-wiki}:
2.   Generate ground truth: full extraction with p=1.0
3.   For each p in sample_proportions:
4.     Run sampled pipeline
5.     Compute concept overlap with ground truth
6.     Record LLM calls, time, cost
7.   Plot Pareto frontier (F1 vs cost)
```

**Expected Output**: Pareto curve showing optimal p ≈ 0.15

#### Experiment 2: Scoring Strategy Comparison (H2)

**Design**: Compare importance scoring strategies, specifically testing hybrid (doc→section) vs pure section-level approaches.

```
Independent Variables:
  - Strategy ∈ {
      Pure-Doc-PageRank,       # Doc scores only, uniform section distribution
      Pure-Section-PageRank,   # Direct section-level PageRank (sparse links)
      Hybrid-Uniform,          # Doc PageRank → uniform to sections
      Hybrid-Features,         # Doc PageRank → section features (heading, links)
      HITS-authority,
      HITS-hub,
      Degree
    }
  - Corpus ∈ {pkm-webdev, pkm-datascience, arch-wiki}

Dependent Variables:
  - Concept F1 (primary)
  - Seed section quality (manual: do selected sections contain key concepts?)
  - Section-level link density (measured, to validate sparse-link assumption)

Protocol:
1. For each corpus, measure section-level link density (validates H2 assumption)
2. For each (strategy, corpus) pair:
3.   Compute importance scores at appropriate level
4.   Select seed SECTIONS at p=0.15
5.   Run extraction + propagation
6.   Compute F1 vs ground truth
7.   Manual assessment: rate seed quality 1-5

Statistical Test: Two-way ANOVA (strategy × corpus) + post-hoc Tukey HSD
```

**Key Comparison**: Hybrid-Features vs Pure-Section-PageRank tests the core H2 hypothesis.

**Expected Output**:
- Section-level link density < 0.1 (sparse), validating hybrid approach
- Hybrid-Features > Pure-Section-PageRank on sparse corpora
- HITS > PageRank on PKM (hub-authority structure), PageRank ≈ HITS on wiki

#### Experiment 3: Propagation Accuracy (H3)

**Design**: Compare propagated labels to direct extraction.

```
Independent Variable: Hop distance from seed {1, 2, 3, 4+}
Dependent Variables: Propagation precision, confidence calibration

Protocol:
1. Run full pipeline on pkm-datascience
2. For non-seed documents, compare:
   - Propagated labels (from pipeline)
   - Direct extraction (LLM on each doc)
3. Stratify by hop distance from nearest seed
4. Compute precision at each hop level

Statistical Test: Regression of precision on hop distance
```

**Expected Output**: Precision decreases with hop distance; confidence scores are calibrated

#### Experiment 4: Validation Effectiveness (H4)

**Design**: Compare extraction with/without validation pyramid.

**Validation Level Definitions** (see [02-ensemble §5](./02-ensemble-validation.md)):

| Level | Name | Method | Cost |
|-------|------|--------|------|
| **None** | Raw extraction | No validation, accept all LLM outputs | 0 |
| **L1** | Schema validation | JSON schema + deterministic rules (confidence bounds, name format) | ~0 (local) |
| **L2** | Grounding check | Verify concept evidence exists in source text (fuzzy match) | ~0 (local) |
| **L3** | Semantic judgment | LLM verifies concept is actually present and correctly characterized | 1 LLM call/concept |

```
Independent Variable: Validation level ∈ {None, L1, L1+L2, L1+L2+L3}
Dependent Variables:
  - Hallucination rate (concepts not grounded in source)
  - Concept precision (vs manual annotation)
  - Validation overhead (additional LLM calls)

Protocol:
1. For corpus in {pkm-datascience, arch-wiki}:
2.   Run extraction with each validation level
3.   Manual annotation: mark hallucinated concepts (sample of 200 concepts)
4.   Compute hallucination rate per level
5.   Measure validation overhead (L3 calls / extraction calls)

Statistical Test: Chi-squared test for hallucination rate reduction
```

**Expected Output**:
- Monotonic decrease in hallucination rate: None > L1 > L1+L2 > L1+L2+L3
- L1+L2 catches ~80% of hallucinations at ~0 cost
- L3 adds ~15% overhead but catches remaining ~20% of hallucinations

#### Experiment 5: Local Model Parity (H6)

**Design**: Compare local vs API models on extraction quality.

```
Independent Variable: Model ∈ {qwen2.5:1.5b, qwen2.5:3b, qwen2.5:7b,
                               llama3.2:3b, claude-3-haiku, claude-3-sonnet}
Dependent Variables: Concept F1, extraction time, cost

Protocol:
1. Sample 100 documents from each corpus
2. Extract concepts with each model
3. Compare to ground truth (claude-opus-4-5 extraction)
4. Compute F1, latency, cost

Statistical Test: Paired t-test (local vs API)
```

**Expected Output**: qwen2.5:7b achieves ≥90% of haiku F1 at ~0 cost

#### Experiment 6: Chunked Extraction Effectiveness (H8)

**Design**: Compare chunked vs single-pass extraction on large sections.

```
Independent Variables:
  - Extraction method ∈ {Single-pass, Chunked-no-context, Chunked-with-prior-concepts}
  - Section size buckets ∈ {<1000, 1000-2000, 2000-5000, 5000+ words}

Dependent Variables:
  - Concept F1 vs ground truth
  - Concept count (chunking may find more concepts)
  - Extraction time

Protocol:
1. Use shakespeare corpus (large sections, no links)
2. For each section, extract with all three methods
3. Generate ground truth via claude-opus-4-5 on full section
4. Compute F1, compare concept counts
5. Stratify results by section size

Statistical Test: Paired t-test (chunked vs single-pass) per size bucket
```

**Expected Output**:
- Single-pass fails or truncates on 5000+ word sections
- Chunked-with-prior-concepts ≥ 95% F1 of single-pass on sections under 2000 words
- Chunked extraction discovers ~10-20% more concepts on large sections (repeated mentions boost confidence)

#### Experiment 7: Edge-Weight Sensitivity (H9)

**Design**: Test sensitivity of propagation to edge-weight choices.

**Default Edge Weights** (from [01-problem §4.4](./01-problem-algorithm.md)):

| Edge Type | Default Weight |
|-----------|----------------|
| Sibling (same doc) | 0.8 |
| LinksTo (with anchor) | 0.85 |
| LinksTo (no anchor) | 0.7 |
| Sibling (same dir) | 0.5 |
| Contains | 0.3 |

```
Independent Variables:
  - Weight configuration ∈ {
      Default,
      Uniform (all 0.5),
      High-sibling (sibling=0.95, others halved),
      High-links (links=0.95, others halved),
      Learned (grid search optimal)
    }
  - Corpus ∈ {pkm-webdev, pkm-datascience}

Dependent Variables:
  - Propagation precision (vs direct extraction)
  - Propagation recall
  - Convergence iterations

Protocol:
1. Run full pipeline with each weight configuration
2. For non-seed sections, compare propagated labels to LLM extraction
3. Compute precision/recall per configuration
4. Grid search for optimal weights (learned baseline)
5. Compare default to learned optimal

Statistical Test: ANOVA across configurations + comparison to learned optimal
```

**Expected Output**:
- Default weights within 5-10% of learned optimal
- Uniform weights significantly worse (~15-20% lower precision)
- Edge-type-specific weights justify the complexity

### 14.6 Ablation Studies

| Ablation | Removes | Tests |
|----------|---------|-------|
| **No sampling** | Importance scoring (p=1.0) | Is sampling necessary? |
| **No propagation** | Label propagation | Does propagation add value? |
| **No validation** | L2/L3 validation | Does validation reduce errors? |
| **No bridges** | Betweenness bridge nodes | Do bridges improve coverage? |
| **Random sampling** | Importance-guided selection | Is importance scoring better than random? |
| **Single model** | Micro-ensemble (one model for all) | Do task-specific models help? |
| **Uniform edge weights** | Type-specific weights | Do edge types matter for propagation? |
| **No chunking** | Chunked extraction | Does chunking help for large sections? |
| **No prior_concepts** | Accumulation context | Does cross-chunk context help? |

### 14.7 Baselines

| Baseline | Description | Source |
|----------|-------------|--------|
| **Full Extraction** | LLM extraction on every document | Our implementation (p=1.0) |
| **Random Sampling** | Random seed selection instead of importance | Our implementation |
| **GraphRAG** | Microsoft's community-summary approach | [microsoft/graphrag](https://github.com/microsoft/graphrag) |
| **LightRAG** | Incremental graph+vector approach | [HKUDS/LightRAG](https://github.com/HKUDS/LightRAG) |
| **TF-IDF + Clustering** | Non-LLM baseline | scikit-learn |

### 14.8 Results Tracking

All experiments should be logged for reproducibility:

```
experiments/
├── config/                    # Experiment configurations (YAML)
│   ├── exp1_sampling.yaml
│   ├── exp2_strategies.yaml
│   └── ...
├── results/                   # Raw results
│   ├── exp1_sampling/
│   │   ├── pkm-datascience_p0.05.json
│   │   ├── pkm-datascience_p0.10.json
│   │   └── ...
│   └── ...
├── analysis/                  # Analysis notebooks
│   ├── exp1_pareto_analysis.ipynb
│   └── ...
└── figures/                   # Generated figures for paper
    ├── pareto_frontier.pdf
    └── ...
```

**Logging Schema** (per experiment run):
```json
{
  "experiment_id": "exp1_sampling_pkm-ds_p0.15",
  "timestamp": "2025-01-15T10:30:00Z",
  "config": {
    "corpus": "pkm-datascience",
    "strategy": "hits-authority",
    "sample_proportion": 0.15,
    "model": "qwen2.5:3b"
  },
  "results": {
    "concept_f1": 0.847,
    "concept_precision": 0.823,
    "concept_recall": 0.872,
    "llm_calls": 78,
    "wall_clock_seconds": 142,
    "documents_covered": 0.97
  },
  "artifacts": {
    "concepts": "results/exp1/.../concepts.json",
    "graph": "results/exp1/.../graph.json"
  }
}
```

### 14.9 Publication Targets

| Venue | Type | Focus | Deadline (typical) |
|-------|------|-------|-------------------|
| **EMNLP** | Conference | NLP, knowledge graphs | May |
| **ACL** | Conference | Computational linguistics | January |
| **NAACL** | Conference | NLP | December |
| **TextGraphs** | Workshop | Text + graphs | Varies |
| **AKBC** | Conference | Knowledge base construction | April |
| **CIKM** | Conference | Information/knowledge management | May |

**Potential Paper Titles**:
1. "Network-Guided Sampling for Efficient Knowledge Graph Construction from Document Corpora"
2. "Label Propagation Meets LLMs: Semi-Supervised Concept Extraction at Scale"
3. "Micro-Ensembles for Local-First Knowledge Graph Construction"

### 14.10 Ethical Considerations

- **Data licensing**: All test corpora use permissively licensed content
- **LLM usage**: Document model versions for reproducibility
- **Environmental impact**: Report compute costs (CO2 equivalent)
- **Bias**: Analyze concept extraction for domain/language bias

### 14.11 Pre-Validation: SPIKE Investigations

Before running formal experiments, the [SPIKE-INVESTIGATION.md](./SPIKE-INVESTIGATION.md) document defines lightweight investigations to validate core assumptions. These are quick, focused tests that prevent wasted effort on flawed foundations.

| Investigation | Validates | Related Hypothesis | Go/No-Go Criteria |
|---------------|-----------|-------------------|-------------------|
| **1. Graph Connectivity** | Link extraction works | Foundation | ≥80% valid edges |
| **2. Importance Scoring** | PageRank/HITS meaningful | H1, H2 | Score variance > random |
| **3. Graph↔Semantic Relatedness** | Structural neighbors are semantically related | H3, H7 | Correlation ρ > 0.3 |
| **4. Local Model Extraction** | Local models extract usable concepts | H6 | ≥70% concept overlap with API |
| **5. L2 Grounding** | Evidence can be found in source | H4 | ≥80% concepts grounded |
| **6. Propagation Usefulness** | Propagated labels are accurate | H3 | Precision ≥ 60% |
| **7. Section-Level Link Density** | Hybrid scoring needed (sparse section links) | H2 | Section link density < 0.1 |
| **8. Structural Hierarchy Quality** | Section parsing works | Foundation | ≥90% sections correctly parsed |
| **9. Sibling Semantic Relatedness** | Directory siblings are related | H7 | Sibling similarity > random |

**Workflow**:
1. Run SPIKE investigations 1, 7, 8 first (foundational)
2. If pass → proceed to Experiments 1-2 (sampling, scoring)
3. Run SPIKE investigations 2, 3, 9 (scoring validation)
4. If pass → proceed to Experiments 3-7 (full evaluation)
5. Run remaining SPIKE investigations as sanity checks

**Key principle**: SPIKE investigations are fast (<1 hour each) and use small corpora (`manza-specs`, `pkm-webdev`). Formal experiments are slow (hours-days) and use full corpora. Don't run formal experiments until SPIKE investigations pass.

---

## Next: [09-landscape-references.md](./09-landscape-references.md) — Open Questions, Landscape & References
