# Plexus Semantic Analysis: Spike Outcome

> **Status**: Complete
> **Date**: 2025-12-18 (finalized 2025-12-20)
> **Trail**: `semantic-spike-research` (t_0jihblgl)

## Executive Summary

The spike validated that **tree-first semantic extraction** is viable for Plexus. The key finding: file hierarchy structure provides both complete coverage and stronger semantic signal than network algorithms.

**Primary Finding**: Tree traversal achieves 100% document coverage with 9.3× stronger semantic signal than PageRank-based approaches.

## Validated Findings

All findings are tracked via clawmarks trail `t_0jihblgl` with full evidence provenance.

| Finding | Confidence | Evidence | Clawmark |
|---------|------------|----------|----------|
| Tree traversal = 100% coverage | High | EXPERIMENT-LOG.md:461 | c_4ek7eafz |
| Siblings 9.3× stronger signal | High | EXPERIMENT-LOG.md:570 | c_2ckf3smk |
| Tree structure IS semantic | High | ENSEMBLE-EXPERIMENTS.md:461 | c_wmi8ltd6 |
| 0% hallucination (technical) | High | EXPERIMENT-LOG.md:1068 | c_gi204l8l |
| Compositional extraction works | High | EXPERIMENT-RESULTS-REVALIDATION.md:93 | c_l866p5e7 |
| Propagation 67%→80% useful | Medium | EXPERIMENT-LOG.md:1057 | c_uvzsyc5s |
| Flat corpora break structure | High | EXPERIMENT-LOG.md:659 | c_euu9kru0 |

## Experiments Conducted

| Experiment | Purpose | Verdict | Details |
|------------|---------|---------|---------|
| A: Two-Stage Refiner | Filter over-specific concepts | **Pass** | ENSEMBLE-EXPERIMENTS.md |
| B: Propagation-Aware | Index page extraction | **Pass** | ENSEMBLE-EXPERIMENTS.md |
| C: Normalization | LLM-based dedup | **Partial** | Case-only safe |
| D: Calibration | Confidence thresholds | **Partial** | 0.9 threshold = 100% precision |
| E: Hierarchical | Tree-informed extraction | **Pass** | ENSEMBLE-EXPERIMENTS.md |
| R4b: Compositional | Large document handling | **Pass** | EXPERIMENT-RESULTS-REVALIDATION.md |

## Remaining Experiments

These experiments are required before publication:

| ID | Experiment | Purpose | Status |
|----|------------|---------|--------|
| **P1** | Propagation parameter sweep | Find optimal decay, hops, threshold | **Not started** |
| **P2** | Multi-corpus extraction | Validate generalization | **Not started** |
| **P3** | Normalization ablation | Identify safe transforms | **Not started** |
| **S1** | Latency profiling | Validate performance claims | **Not started** |
| **S2** | Concurrency testing | Find safe parallelism | **Not started** |

See PAPER.md Section 6 and Appendix C for experiment specifications.

## Key Decisions

Based on validated findings:

1. **Tree-first traversal** — Use file hierarchy, not PageRank (c_ex7c4a7h)
2. **Sibling weighting** — Weight directory co-location higher than links (c_dsmldjx2)
3. **Simple normalization** — Case-only; semantic dedup rejected (c_mnbq0te7)
4. **Three-system architecture** — llm-orc → clawmarks → plexus

## Document Map

| Document | Purpose |
|----------|---------|
| **PAPER.md** | Journal-ready research findings |
| **EXPERIMENT-LOG.md** | Raw timestamped experiment data |
| **EXPERIMENT-RESULTS-REVALIDATION.md** | Revalidation experiment results |
| **ENSEMBLE-EXPERIMENTS.md** | Multi-agent extraction experiments |
| **SYSTEM-DESIGN.md** | Implementation architecture |
| **README.md** | Document index |

## Spike Closure

The spike investigation is **complete**. All core hypotheses have been tested:

- [x] LLM extraction is viable
- [x] Tree structure provides semantic signal
- [x] Compositional extraction handles large documents
- [x] Provenance tracking via clawmarks works

Remaining work (P1-P3, S1-S2) is parameter optimization, not hypothesis validation.
