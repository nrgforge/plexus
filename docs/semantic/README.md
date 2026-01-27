# Plexus Semantic Analysis

> **Status**: Post-Spike — Architecture Validated
> **Last Updated**: 2025-12-20
> **Branch**: `feature/plexus-llm-semantic-spike`
> **Research Trail**: `semantic-spike-research` (t_0jihblgl)

## Overview

This specification describes the architecture for LLM-based semantic extraction in Plexus. Our spike investigation validated a **tree-first approach** that uses file hierarchy structure—not network algorithms—as the primary mechanism for document traversal and semantic inference.

**Key Findings** (validated via experiments):

| Finding | Evidence |
|---------|----------|
| Tree traversal achieves 100% document coverage | PageRank BFS only 44-72% (EXPERIMENT-LOG.md:461) |
| Directory co-location provides 9.3× stronger semantic signal than explicit links | Siblings vs wikilinks comparison (EXPERIMENT-LOG.md:570) |
| Tree structure IS semantic information | Directory names enable domain inference (ENSEMBLE-EXPERIMENTS.md:461) |
| LLM extraction achieves 0% hallucination on technical corpora | With evidence-grounded prompting (EXPERIMENT-LOG.md:1068) |
| Compositional extraction works for large documents | Chunk→fan-out→aggregate→synthesize validated (SPIKE-OUTCOME.md:47) |

## Document Structure

### Research Documents

| Document | Purpose | Status |
|----------|---------|--------|
| [PAPER.md](./PAPER.md) | Journal-ready research findings | Primary output |
| [EXPERIMENT-LOG.md](./EXPERIMENT-LOG.md) | Raw timestamped experiment data | Evidence source |
| [EXPERIMENT-RESULTS-REVALIDATION.md](./EXPERIMENT-RESULTS-REVALIDATION.md) | Revalidation experiment results | Evidence source |
| [ENSEMBLE-EXPERIMENTS.md](./ENSEMBLE-EXPERIMENTS.md) | Multi-agent extraction experiments | Evidence source |
| [SPIKE-OUTCOME.md](./SPIKE-OUTCOME.md) | Spike summary and status | Complete |

### Implementation Documents

| Document | Purpose | Status |
|----------|---------|--------|
| [SYSTEM-DESIGN.md](./SYSTEM-DESIGN.md) | Implementation architecture | Has speculative claims |

### Archived Documents

Pre-spike documents (PageRank-centric design) are archived in `archive/`. See [archive/README.md](./archive/README.md) for details.

## Architecture

### Tree-First Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TREE-FIRST EXTRACTION                              │
│                                                                              │
│   1. TRAVERSE                2. EXTRACT              3. PROPAGATE            │
│   ─────────────             ─────────────           ─────────────            │
│   Walk file tree            LLM extraction          Concept spreading        │
│   (100% coverage)           per document            via siblings             │
│                                                                              │
│   ┌─────────┐               ┌─────────┐             ┌─────────┐             │
│   │   /     │──────────────►│ llm-orc │────────────►│ sibling │             │
│   │  ├─A/   │  DFS walk     │ensemble │  concepts   │  edges  │             │
│   │  │ ├─1  │               └─────────┘             └─────────┘             │
│   │  │ └─2  │                    │                       │                  │
│   │  └─B/   │                    ▼                       ▼                  │
│   │    └─3  │               ┌─────────┐             ┌─────────┐             │
│   └─────────┘               │clawmarks│             │ plexus  │             │
│                             │  (MCP)  │             │  graph  │             │
│                             └─────────┘             └─────────┘             │
│                              provenance              knowledge              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Why Tree-First?

The spike investigation revealed that for well-organized corpora:

1. **Tree traversal solves coverage trivially** — Every document belongs to a directory, forming a fully connected structure. PageRank seed selection is unnecessary overhead.

2. **Directory co-location encodes author intent** — Authors organize related content together. This organizational structure provides 9.3× stronger semantic signal than explicit cross-references.

3. **PageRank solves the wrong problem** — PageRank optimizes for "importance" in a link graph. We need coverage and semantic proximity—both provided trivially by the tree.

### System Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              MANZA UI                                        │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                  │
│   │   Editor     │◄───│   Plexus     │───►│   Narrator   │                  │
│   │   (files)    │    │   Graph      │    │  (insights)  │                  │
│   └──────────────┘    └──────────────┘    └──────────────┘                  │
│          ▲                   │                                               │
│          │                   │ clawmark_id                                   │
│          │                   ▼                                               │
│          │            ┌──────────────┐                                       │
│          └────────────│  Clawmarks   │ ◄── "Go to source" UX                │
│         file:line     │    (MCP)     │                                       │
│                       └──────────────┘                                       │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                           BACKEND PIPELINE                                   │
│                                                                              │
│   Document ──► llm-orc ──► Clawmarks ──► Plexus                             │
│               (extract)    (provenance)   (graph)                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Ensemble Selection

| Content Type | Size | Ensemble | Rationale |
|--------------|------|----------|-----------|
| Technical | < 3000 words | `plexus-semantic` | Direct extraction, 100% grounding |
| Technical | > 3000 words | `plexus-compositional` | Chunk→fan-out→aggregate |
| Literary | < 3000 words | `plexus-refinement` | Better categorization |
| Literary | > 3000 words | `plexus-compositional` | Same pipeline, different prompts |
| Flat corpus | any | `plexus-refinement` | No tree signal available |

## Experimental Gaps

These claims in SYSTEM-DESIGN.md are not yet validated:

| Claim | Status | Experiment |
|-------|--------|------------|
| Propagation: decay=0.7, threshold=0.4, hops=3 | **Partial** | P1: 29% appropriate with real LLM extraction |
| Singularization is safe | **Gap** | P3: Normalization ablation |
| 5 concurrent workers | **Gap** | S2: Throughput testing |
| Latency < 5s per doc | **Gap** | S1: Latency profiling |

See [PAPER.md](./PAPER.md) Section 6 for full experiment plan.

## Quick Links

### For Implementation
- **Start here**: [SPIKE-OUTCOME.md](./SPIKE-OUTCOME.md) — Validated decisions
- **Architecture**: [SYSTEM-DESIGN.md](./SYSTEM-DESIGN.md) — Implementation design (note gaps above)
- **Ensembles**: `.llm-orc/ensembles/plexus-*.yaml` — LLM orchestration configs

### For Research
- **Paper**: [PAPER.md](./PAPER.md) — Journal-ready findings with evidence
- **Raw data**: [EXPERIMENT-LOG.md](./EXPERIMENT-LOG.md) — Timestamped results
- **Evidence trail**: clawmarks trail `t_0jihblgl`

### Test Corpora
- `plexus-test-corpora/pkm-webdev` — Structured technical corpus (50 files, 28 dirs)
- `plexus-test-corpora/arch-wiki` — Medium-structured wiki (2,487 files)
- `plexus-test-corpora/shakespeare` — Flat corpus (43 files, 1 dir)

## Key Insight

> **The file tree is the semantic graph.** For structured corpora, directory organization encodes topic relationships more reliably than explicit links. Tree traversal provides complete coverage with O(n) complexity. PageRank and network science approaches are unnecessary overhead that solve the wrong problem.
