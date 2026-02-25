# Semantic Extraction Research Spike

**Date:** 2026-02-24
**Phase:** RDD Research (precedes model, decide, build)
**Goal:** Determine the right extraction decomposition and ensemble topology for Plexus's general-purpose text/document semantic adapter.

## Context

Plexus has a complete ingest pipeline (ADR-028) with three core adapters, YAML-driven declarative specs, llm-orc integration, and enrichment loop. The infrastructure is tested and ready. What's missing: actual adapter specs and ensembles that make it work "for real."

llm-orc now supports composable ensembles (ADR-013), input key routing (ADR-014), and typed agent configs (ADR-012). This enables ensemble-of-ensembles architectures where a parent ensemble orchestrates child ensembles.

The llm-conductor's tiered profile set provides the model foundation:
- **Router tier** (1.7B): Qwen3:1.7b for classification/dispatch
- **Analyst tier** (7-8B): Qwen3:8b + Mistral:7b for extraction
- **Reasoner tier** (8B): DeepSeek-R1:8b for chain-of-thought tasks
- **Synthesizer tier** (14B): Qwen3:14b for merging 3+ upstream outputs

Key design constraint: verification within ensemble DAGs uses classical ML (scripts, embeddings), not SLM self-assessment (Invariant 14).

## Research Questions

### Q1: Extraction Decomposition

What semantic extraction subtasks can 7-8B models handle reliably in isolation?

- **H1a:** Entity/concept extraction is reliably decomposable to a single 7-8B agent
- **H1b:** Relationship extraction requires seeing entities first (sequential, not parallel)
- **H1c:** Theme/topic extraction is a distinct skill from entity extraction
- **H1d:** Qwen3:8b and Mistral:7b have meaningfully different extraction profiles (complementarity)
- **H1e:** A 1.7B model can reliably classify content type for routing

### Q2: Composition Topology

What DAG shape produces the best extraction quality for the token budget?

- **H2a:** Parallel independent extractors + synthesizer outperforms monolithic extraction
- **H2b:** Sequential chain (entities -> relationships -> themes) outperforms parallel
- **H2c:** The 14B synthesizer adds measurable value over mechanical merging
- **H2d:** Verification scripts catch errors that SLM self-assessment misses

## Method

### Test Corpus

5-8 real text samples spanning target use cases:
- 2 technical docs (ADRs or design docs from this project)
- 2 research/essay pieces
- 2 general notes/annotations
- 1-2 mixed/edge cases (code-heavy docs, short fragments)

### Evaluation

Claude produces a "gold standard" extraction per sample (concepts, relationships, themes). SLM outputs are measured against it. Claude-as-judge evaluates final quality — not SLMs judging themselves.

### Experiment Sequence

1. **Baseline:** Each model (Qwen3:8b, Mistral:7b, DeepSeek-R1:8b) on monolithic extraction. Establish single-model capability.
2. **Decomposed:** Each model on isolated subtasks (concepts only, relationships only, themes only). Compare per-dimension quality against monolithic.
3. **Topology:** Candidate DAG shapes — parallel fan-out, sequential chain, hybrid — using best decomposition from step 2.
4. **Router:** Qwen3:1.7b on content classification. Can it reliably distinguish content types?
5. **Verification:** Schema validation script tested against extraction errors.

### Profiles

Adopt the conductor's `config.yaml` profile set as plexus's `.llm-orc/` foundation before experiments begin.

## Deliverables

1. Research essay documenting findings per hypothesis
2. Raw experiment data (llm-orc artifacts)
3. Recommended extraction decomposition with evidence
4. Candidate ensemble topology
5. Profile recommendations (which models for which roles)

## Scope Boundaries

**In scope:** Extraction quality, decomposition viability, topology comparison, model complementarity, router reliability, verification scripts.

**Out of scope (deferred to /rdd-decide):**
- Final adapter spec YAML design
- Enrichment interactions (TagConceptBridger, CoOccurrence, etc.)
- Reflexive adapter patterns
- Schedule monitor / background triggering

## What Follows

Research -> `/rdd-model` (domain vocabulary) -> `/rdd-decide` (ADR for semantic extraction architecture) -> `/rdd-build` (implement adapter specs and ensembles).
