# Research Outcomes

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## Overview

This document tracks research artifacts, findings, and outcomes from implementing and validating the Plexus semantic analysis pipeline. Update this document as experiments are conducted and results are obtained.

---

## Experiment Results

### Experiment 1: Sampling Efficiency (H1)

| Date | Corpus | p | Concept F1 | LLM Calls | Notes |
|------|--------|---|------------|-----------|-------|
| - | - | - | - | - | *Pending implementation* |

**Key Finding**: *TBD*

### Experiment 2: Strategy Comparison (H2)

| Date | Strategy | Corpus | Concept F1 | Seed Quality | Notes |
|------|----------|--------|------------|--------------|-------|
| - | - | - | - | - | *Pending implementation* |

**Key Finding**: *TBD*

### Experiment 3: Propagation Accuracy (H3)

| Date | Hop Distance | Precision | Recall | Confidence ρ | Notes |
|------|--------------|-----------|--------|--------------|-------|
| - | - | - | - | - | *Pending implementation* |

**Key Finding**: *TBD*

### Experiment 4: Validation Effectiveness (H4)

| Date | Validation Level | Hallucination Rate | Precision | Notes |
|------|------------------|-------------------|-----------|-------|
| - | - | - | - | *Pending implementation* |

**Key Finding**: *TBD*

### Experiment 5: Local Model Parity (H6)

| Date | Model | F1 vs Ground Truth | Latency (ms) | Cost | Notes |
|------|-------|-------------------|--------------|------|-------|
| - | - | - | - | - | *Pending implementation* |

**Key Finding**: *TBD*

---

## Hypothesis Status

| Hypothesis | Status | Evidence | Confidence |
|------------|--------|----------|------------|
| **H1**: Sampling Efficiency | Pending | - | - |
| **H2**: Strategy-Corpus Fit | Pending | - | - |
| **H3**: Propagation Accuracy | Pending | - | - |
| **H4**: Validation Effectiveness | Pending | - | - |
| **H5**: Optimal Sampling | Pending | - | - |
| **H6**: Local Model Parity | Pending | - | - |

**Status Legend**: Pending | In Progress | Supported | Partially Supported | Rejected

---

## Ablation Study Results

| Ablation | Effect on F1 | Effect on Coverage | Notes |
|----------|--------------|-------------------|-------|
| No sampling (p=1.0) | - | - | Baseline |
| No propagation | - | - | *TBD* |
| No validation | - | - | *TBD* |
| No bridges | - | - | *TBD* |
| Random sampling | - | - | *TBD* |
| Single model | - | - | *TBD* |

---

## Artifacts

### Ground Truth Datasets

| Corpus | Ground Truth File | Annotators | IAA Score | Date Generated |
|--------|------------------|------------|-----------|----------------|
| `pkm-datascience` | - | - | - | *Pending* |
| `arch-wiki` | - | - | - | *Pending* |
| `pkm-webdev` | - | - | - | *Pending* |
| `shakespeare` | - | - | - | *Pending* |

### Generated Figures

| Figure | Description | File | Status |
|--------|-------------|------|--------|
| Pareto Frontier | F1 vs LLM Calls by p value | `figures/pareto_frontier.pdf` | Pending |
| Strategy Heatmap | F1 by (Strategy × Corpus) | `figures/strategy_heatmap.pdf` | Pending |
| Propagation Decay | Precision vs Hop Distance | `figures/propagation_decay.pdf` | Pending |
| Validation Impact | Hallucination Rate by Level | `figures/validation_impact.pdf` | Pending |
| Model Comparison | F1 by Model (local vs API) | `figures/model_comparison.pdf` | Pending |

### Analysis Notebooks

| Notebook | Purpose | Status |
|----------|---------|--------|
| `analysis/exp1_pareto_analysis.ipynb` | Sampling efficiency analysis | Pending |
| `analysis/exp2_strategy_analysis.ipynb` | Strategy comparison ANOVA | Pending |
| `analysis/exp3_propagation_analysis.ipynb` | Propagation regression | Pending |
| `analysis/exp4_validation_analysis.ipynb` | Chi-squared tests | Pending |
| `analysis/exp5_model_comparison.ipynb` | Paired t-tests | Pending |

---

## Publication Progress

### Draft Papers

| Title | Target Venue | Status | Draft Link |
|-------|--------------|--------|------------|
| "Network-Guided Sampling for Efficient Knowledge Graph Construction" | EMNLP | Not Started | - |
| "Label Propagation Meets LLMs" | ACL | Not Started | - |
| "Micro-Ensembles for Local-First KG Construction" | CIKM | Not Started | - |

### Key Claims

Based on experimental results, document which claims can be made:

| Claim | Supporting Evidence | Strength |
|-------|---------------------|----------|
| Network-guided sampling reduces LLM calls by >80% | Exp1 | *Pending* |
| HITS outperforms PageRank on PKM corpora | Exp2 | *Pending* |
| Label propagation achieves >70% precision | Exp3 | *Pending* |
| Validation pyramid halves hallucination rate | Exp4 | *Pending* |
| Local models achieve API-level quality | Exp5 | *Pending* |

---

## Lessons Learned

### Implementation Insights

*Document insights gained during implementation that may be valuable for the paper or future work.*

1. *TBD*

### Unexpected Findings

*Document any surprising results or behaviors discovered during experimentation.*

1. *TBD*

### Limitations Discovered

*Document limitations encountered that should be acknowledged in publications.*

1. *TBD*

---

## Changelog

| Date | Update |
|------|--------|
| 2025-12-12 | Initial research outcomes document created |

---

## Back to: [README.md](./README.md) — Overview
