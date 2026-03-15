# Cross-Essay Synthesis: Extraction Decomposition Results

## Aggregate Performance

| Subtask | Essay 02 | Essay 04 | Essay 12 | Essay 18 | Average |
|---------|----------|----------|----------|----------|---------|
| **Entities (Qwen) Recall** | 64% | 21% | 46% | 18-29% | ~38% |
| **Entities (Mistral) Recall** | 64% | -- | 39% | -- | ~51% |
| **Relationships Recall** | 40% | 28% | 50%* | 0-38%* | ~29% |
| **Themes Recall** | 80% | 30% | 80% | 17-50% | ~50% |

*generous partial-match scoring

## Key Findings

### F1: Theme extraction is the most reliable isolated subtask
- Essays 02 and 12: 80% F1 — the model identified core arguments/principles at the right abstraction level
- Essays 04 and 18: 30% and ~35% — quality drops significantly on certain essay types
- The drop correlates with essay structure: 02 and 12 have explicit argumentative structure (problem → analysis → solution); 04 and 18 are more design-document-style with enumerated features
- **Implication:** Theme extraction works well when the text has clear argumentative structure. It degrades when the text is more descriptive/enumerative.

### F2: Entity extraction misses domain neologisms and compound terms
- Consistently captures: high-frequency, short, repeatedly-mentioned terms (provenance, adapter, fragment)
- Consistently misses: compound technical terms (cross-dimensional traversal, extraction coordinator), domain neologisms (mirror not oracle, seed promotion, sketch weight), proper component names (TagConceptBridger, CoOccurrenceEnrichment)
- **Implication:** Entity extraction needs domain vocabulary priming — a "glossary hint" in the prompt, or a two-pass approach where entities from prior extractions inform future ones.

### F3: Relationship extraction is the weakest isolated subtask
- Extracts surface-level textual co-occurrences, not abstract conceptual relationships
- Misses explicit structural relationships even when literally stated (e.g., "chain contains mark")
- Frequently confuses the essay's argumentative flow (what was tried and rejected) with domain relationships
- Schema compliance is poorest — wrong field names, freeform relationship types, missing evidence
- **Implication:** Relationship extraction cannot work in isolation. It needs the entity list as input (sequential, not parallel). Even then, 7-8B models may not be strong enough for abstract relationship identification.

### F4: Qwen and Mistral show genuine but modest complementarity
- On Essay 02: Same recall, same misses — no complementarity
- On Essay 12: Different unique finds (Qwen: epistemology; Mistral: Hebbian). Union improves recall from ~42% to ~54%
- **Implication:** Complementarity exists but is inconsistent. Worth testing in a fan-out topology where union is cheap (just merge) rather than relying on it.

### F5: Schema compliance degrades with input length
- Short essays (02, 04): JSON output mostly correct, field names mostly right
- Long essays (18): Wrong field names, prose instead of JSON, type vocabulary ignored
- **Implication:** The models trade instruction-following for comprehension as input length grows. Chunking or explicit few-shot examples in the prompt may help.

### F6: Confidence scores are uniformly broken
- Every model produces monotonically decreasing confidence (0.9, 0.8, 0.7...) or all-1.0
- These are rank orderings, not calibrated probabilities
- **Implication:** Don't use SLM-generated confidence for graph weights. Use structural signals instead (entity mention count, section position, cross-source agreement). This validates the Invariant 14 position: verification via classical ML, not SLM self-assessment.

## Hypothesis Evaluation

| Hypothesis | Verdict | Evidence |
|-----------|---------|----------|
| **H1a:** Entity extraction decomposes cleanly | **Partial** | Works for common terms, fails for domain-specific vocabulary. Needs vocabulary priming. |
| **H1b:** Relationship extraction is sequential | **Confirmed** | Relationships extracted in isolation are mostly surface-level or wrong. Must follow entities. |
| **H1c:** Theme extraction is distinct | **Confirmed with caveats** | Best isolated subtask, but quality varies by essay structure (argumentative > descriptive). |
| **H1d:** Qwen/Mistral complementarity | **Weak support** | Genuine on 1 of 2 tested samples. Union helps but inconsistently. |

## Emerging Topology Recommendation

Based on these findings, the DAG should be:

```
                    ┌─── entity-extractor-qwen ──┐
input ──► chunker ──┤                            ├──► relationship-extractor ──► synthesizer
                    └─── entity-extractor-mistral ┘         (receives entity list)

                    ┌─── theme-extractor ─────────────────────────────────────────┘
                    │    (parallel, independent)
```

1. **Chunk** the input (script agent) — essays > ~100 lines need chunking
2. **Fan-out entity extraction** with both Qwen and Mistral (parallel, union results)
3. **Sequential relationship extraction** — receives merged entity list as context
4. **Parallel theme extraction** — independent, runs alongside entities
5. **14B synthesizer** — merges entities, relationships, and themes into final structured output

The chunking addresses F5 (schema compliance with length). The entity fan-out addresses F4 (complementarity). The sequential relationship step addresses F3 (isolation weakness). The parallel themes address F1 (independent quality).

## Open Questions for Q2 (Topology)

- Does providing the entity list to the relationship extractor actually improve recall?
- Does the 14B synthesizer produce better output than mechanical merging?
- What chunking strategy works best (section-based vs. fixed-size)?
- Should the theme extractor also receive the entity list, or does independence help?
