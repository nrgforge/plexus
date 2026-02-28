# Q2 Spike: Entity Extraction 2×2 Matrix — Analysis

## Gold Standard (Essay 12, 13 entities)

1. provenance
2. epistemology
3. chain (provenance)
4. mark (provenance)
5. fragment
6. TagConceptBridger
7. Hebbian contribution
8. dual obligation (Invariant 7)
9. adapter
10. enrichment
11. multi-phase processing
12. deterministic ID
13. cross-dimensional traversal

## Timing

| Condition | Total | Gen time | Gen tokens | Entities |
|-----------|-------|----------|------------|----------|
| A: Unprimed + Free | 36.9s | 24.5s | 687 | 38 |
| B: Unprimed + Structured | 12.7s | 11.9s | 347 | 13 |
| C: Primed + Free | 20.6s | 10.2s | 264 | 15 |
| D: Primed + Structured | 15.6s | 14.8s | 386 | 15 |

## Gold Standard Scoring

### Condition A: Unprimed + Unconstrained (38 entities)
- provenance: PARTIAL (via "provenance marks", "provenance dimension", "provenance output")
- epistemology: ✓
- chain: ✓ ("chains")
- mark: ✓ ("marks")
- fragment: PARTIAL ("fragment node")
- TagConceptBridger: ✓
- Hebbian contribution: ✓ ("hebbian contributions")
- dual obligation: ✗
- adapter: ✗ (not standalone — "adapter-shaped things", "adapter_id")
- enrichment: ✗
- multi-phase processing: ✓
- deterministic ID: ✗
- cross-dimensional traversal: ✗
**Score: 6/13 clear + 2 partial = ~54%**
**Noise: 38 entities for 13 gold — 2.9× overgeneration**

### Condition B: Unprimed + Structured (13 entities)
- provenance: PARTIAL ("provenance marks", "provenance trail")
- epistemology: PARTIAL ("epistemological infrastructure")
- chain: ✓
- mark: ✓
- fragment: PARTIAL ("fragment adapter")
- TagConceptBridger: ✗
- Hebbian contribution: ✗
- dual obligation: ✗
- adapter: PARTIAL ("fragment adapter")
- enrichment: ✗
- multi-phase processing: ✗
- deterministic ID: ✗
- cross-dimensional traversal: ✗
**Score: 2/13 clear + 4 partial = ~31%**

### Condition C: Glossary-primed + Unconstrained (15 entities)
- provenance: ✓
- epistemology: ✓
- chain: ✓
- mark: ✓
- fragment: ✓
- TagConceptBridger: ✗
- Hebbian contribution: PARTIAL ("hebbian")
- dual obligation: ✗
- adapter: ✓
- enrichment: ✗
- multi-phase processing: ✓
- deterministic ID: ✗
- cross-dimensional traversal: ✗
**Score: 7/13 clear + 1 partial = ~58%**
**Plus domain terms not in gold: concept, tag, contribution**

### Condition D: Glossary-primed + Structured (15 entities)
- provenance: ✓
- epistemology: ✓
- chain: ✓
- mark: ✓
- fragment: ✓
- TagConceptBridger: ✗
- Hebbian contribution: ✗
- dual obligation: ✗
- adapter: ✓
- enrichment: ✓ ← unique find
- multi-phase processing: ✗
- deterministic ID: ✗
- cross-dimensional traversal: ✗
**Score: 7/13 clear = 54%**
**Plus domain terms not in gold: tag, concept, source, annotation, ingest, pipeline**

## Key Observations

### 1. Three categories of missed entities
The missed entities fall into distinct categories requiring different interventions:

**Category 1: Normalized domain terms** (provenance, adapter, fragment, mark, chain)
→ Glossary priming fixes these. C and D extract clean, normalized terms. A paraphrases ("provenance marks" instead of "provenance"). B misses many.

**Category 2: Component/class names** (TagConceptBridger, EngineSink, FragmentAdapter)
→ A (unprimed) finds TagConceptBridger verbatim. C and D (primed) DO NOT — priming biases the model toward normalized vocabulary, suppressing verbatim component names.

**Category 3: Compound neologisms** (dual obligation, deterministic ID, cross-dimensional traversal)
→ NONE of the 4 conditions extract these. "dual obligation" never appears as that phrase in Essay 12. "deterministic ID" is discussed but not used as a standalone term. "cross-dimensional traversal" appears literally but the model doesn't recognize it as a named concept.

### 2. Gold standard validity issue
"dual obligation" is in the gold standard but does NOT appear as a phrase in Essay 12. The human annotator recognized the concept from domain knowledge. This is not a model extraction failure — it's a concept recognition task that requires prior knowledge. An extraction model can only extract what the text says.

### 3. Priming suppresses discovery
TagConceptBridger appears in A but not C or D. The glossary priming normalizes the model's focus toward single-word domain terms, suppressing verbatim extraction of CamelCase component names. Trade-off: better normalization, worse verbatim recall.

### 4. Structured output alone hurts entity extraction
B (structured, no priming) is the worst performer — 31% recall. Unlike relationship extraction (where schema constraint improved quality), entity extraction benefits more from having vocabulary context than from output format.

### 5. Best overall: C or D (primed)
Both achieve 54-58% recall with 15 clean entities. D adds "enrichment" (unique find). C adds "multi-phase processing". Neither over-generates like A (38 entities).
