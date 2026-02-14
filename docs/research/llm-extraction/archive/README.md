# Archived Documents

> **Archived**: 2025-12-20
> **Reason**: Superseded by spike investigation findings

These documents represent the **pre-spike design** for Plexus semantic analysis. They assumed a PageRank-centric, network-science approach to document traversal and importance scoring.

## Why Archived

The spike investigation (December 2025) validated that:

1. **Tree traversal achieves 100% coverage** — PageRank-based seed selection is unnecessary
2. **Sibling co-location provides 9.3× stronger semantic signal** than explicit links
3. **Tree structure IS semantic information** — directory organization encodes author intent

These findings invalidate the core assumptions of the archived documents.

## Document Status

### Pre-Spike Design Documents

| Document | Status | Notes |
|----------|--------|-------|
| 01-problem-algorithm.md | **Superseded** | PageRank approach obsolete |
| 02-ensemble-validation.md | Partially valid | Validation pyramid concept useful |
| 03-complexity-dataflow.md | **Superseded** | Tree traversal is O(n), simpler |
| 04-implementation.md | **Superseded** | See SYSTEM-DESIGN.md |
| 05-validation-strategy.md | Partially valid | Experiment infrastructure useful |
| 06-embeddings-network.md | **Superseded** | Sibling signal > network science |
| 07-experience-vision.md | Valid | UX vision unchanged |
| 08-research-methodology.md | Valid | Methodology still applies |
| 09-landscape-references.md | Valid | References still relevant |
| 10-research-outcomes.md | **Superseded** | See PAPER.md |

### Post-Spike Obsolete Documents

| Document | Status | Notes |
|----------|--------|-------|
| IMPLEMENTATION-SPEC.md | **Merged** | Content merged into SYSTEM-DESIGN.md |

**Deleted**: SPIKE-INVESTIGATION.md — Original protocol with incomplete tables; actual results in EXPERIMENT-LOG.md

## Current Documentation

Active documentation is in the parent directory:

- **PAPER.md** — Journal-ready research findings
- **EXPERIMENT-LOG.md** — Raw experiment data
- **SYSTEM-DESIGN.md** — Implementation architecture
- **README.md** — Document index

## Reuse Guidelines

If referencing archived material:
- 07-experience-vision.md: UX concepts are still valid
- 08-research-methodology.md: Research approach is still valid
- 09-landscape-references.md: Academic references are still useful

Do not reuse without critical evaluation:
- Any PageRank-related algorithms
- Network science traversal approaches
- "Importance scoring" concepts
