# Experiment 1: Essay 02 (The Opacity Problem) — Decomposed Extraction

Note: `model_substituted: true` on all results — Qwen3:8b may have fallen back to a different model. Need to verify which model actually ran.

## Entity Extraction — Qwen

Extracted 9 concepts (gold standard: 14):
- opacity problem ✓
- working memory ✓
- cognitive load theory ✓ (gold has just "cognitive load")
- sweller's cognitive load theory — duplicate of above, shouldn't be separate
- situation awareness ✓
- epistemic actions ✓ (gold: "epistemic action")
- pragmatic actions — not in gold as primary concept, but present in text
- re-representation ✓
- graphical constraining ✓

**Missing from gold:** external structural representation, cognitive offloading, vibe-coding, material disengagement, knowledge graph, zone of proximal development, AI-assisted composition, computational offloading

**Observations:**
- Used types outside the specified enum ("problem", "mechanism") — didn't follow schema strictly
- Missed the most important concept: "external structural representation" — the proposed remedy
- Missed domain-specific terms (vibe-coding, material disengagement) — these require recognizing neologisms
- Hit 9/14 gold concepts (64% recall), but missed several central ones
- Deduplicated poorly (cognitive load theory + Sweller's cognitive load theory)

## Entity Extraction — Mistral

Extracted 9 concepts:
- opacity problem ✓
- working memory ✓
- cognitive load theory ✓
- schema — not in gold standard
- AI — too generic (gold excludes generic terms)
- situation awareness ✓
- epistemic actions ✓
- pragmatic actions — marginal
- re-representation ✓

**Observations:**
- Confidence scores are linearly descending (0.9, 0.8, 0.7... 0.1) — clearly not calibrated, just a sequence
- Included "AI" which is too generic — didn't follow the exclusion guideline
- Similar recall to Qwen (same core concepts), but with different failures
- Same missing concepts as Qwen: external structural representation, vibe-coding, cognitive offloading
- No meaningful complementarity on this sample — both models found the same things and missed the same things

## Relationship Extraction — Qwen

Extracted 7 relationships:
1. knowledge --erodes--> understanding ✓ (paraphrased)
2. developer's personal knowledge base --contains--> architectural decisions — wrong source concept
3. team's documentation --lacks--> unified structural map — instance, not abstraction
4. creator's artifacts --contains--> structural relationships — too literal
5. information density --erodes--> comprehension ✓ (related to gold's cognitive load mechanism)
6. AI interaction --hinders--> structural awareness ✓ (gold: AI accelerates opacity)
7. creator --requires--> external representation ✓

**Observations:**
- 4/7 are at least partially correct (57%)
- Relationships are too literal — extracted from surface text rather than identifying abstract relationships
- Missed: opacity problem caused by cognitive load, external representation remedies opacity, epistemic/pragmatic distinction
- Used concepts not in any entity list ("developer's personal knowledge base") — entity and relationship extractors don't share vocabulary
- The "evidence" field worked well — every relationship has a supporting quote

## Theme Extraction — Qwen

Extracted 5 themes:
1. "Knowledge Outpaces Understanding" ✓ (argument)
2. "Structural Awareness Erosion" ✓ (tension)
3. "Information Density vs Comprehension" — overlaps significantly with #1 and #2
4. "Domain-Generality of Opacity" ✓ (insight)
5. "Structural Representation as Remedy" ✓ (principle)

**Observations:**
- 4/5 gold themes captured (80% recall) — best performing subtask
- Missed: "tools should externalize awareness, not reasoning" — the cognitive offloading caveat
- Theme 3 is a near-duplicate of themes 1 and 2 — weak deduplication
- Type labels (argument, tension, insight, principle) are appropriate
- Supporting evidence is generic — paraphrases rather than specific quotes
- Themes are at the RIGHT abstraction level — not entity-like, not too vague

## Summary: Essay 02

| Subtask | Gold Count | Extracted | Recall | Precision | Quality |
|---------|-----------|-----------|--------|-----------|---------|
| Entities (Qwen) | 14 | 9 | 64% | ~78% | Misses key concepts |
| Entities (Mistral) | 14 | 9 | 64% | ~67% | Similar gaps, less precise |
| Relationships | 10 | 7 | ~40% | ~57% | Too literal, misses abstract |
| Themes | 5 | 5 | 80% | 80% | Best subtask, some overlap |

**Key finding:** Theme extraction works best as an isolated subtask. Entity extraction misses domain neologisms. Relationship extraction suffers most from isolation — it extracts surface-level relationships from text rather than identifying the abstract conceptual relationships. The relationship extractor would likely benefit from receiving the entity list as context (supporting H1b: sequential, not parallel).
