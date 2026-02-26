# Research Log: Extraction Pipeline Completion

## Background

Essay 24 (*Semantic Extraction Decomposition*) established a parallel extraction architecture: multiple 7-8B models extracting independently, composed through deterministic merge. Entity extraction works (54-69% per model, up to 85% union). Theme extraction is solved (100%). But the pipeline has three gaps that prevent a production-quality extraction adapter.

This research cycle investigates those gaps in priority order.

---

## Question 1: Can relationship extraction shift from generation to classification?

**Why this matters:** Relationship extraction is the one unsolved subtask. Isolated generative extraction (ask a 7-8B model to produce relationships from text) achieves only 31-35% recall. The model generates plausible inferences — *A causes B*, *X accelerates Y* — but misses explicitly stated structural relationships: *chain contains mark*, *adapter produces provenance*. It invents connections rather than extracting stated ones.

**What we already know:**

- When given entity context (the synthesized pipeline's 14B model), relationship recall jumps to 80-100%. The variable is not topology or model capability — it's whether the extractor has a vocabulary to work with.
- The sequential topology (entity list → relationship extractor at 8B) times out at 120 seconds. The combined input — source text + entity list + extraction instructions — exceeds what an 8B model can process within the time budget on consumer hardware.
- The 14B model that demonstrated high relationship recall requires 87% of system memory and 3+ minutes per run. Not viable for local-first batch extraction.

**The hypothesis:** Relationship extraction fails as generation but may succeed as classification. Instead of asking "what relationships exist in this text?", ask "given entities A and B in the sentence 'A contains B', what is the relationship type?" This constrains the problem: the model selects from a fixed vocabulary of relationship types rather than generating both entities and relationship from scratch.

**Nuances to investigate:**

1. **Candidate pair generation.** Which entity pairs get tested? Co-occurrence within the same sentence is the obvious heuristic, but the essays use multi-sentence arguments where entity A appears in one sentence and entity B in the next, connected by an implicit relationship. Sentence-level co-occurrence may miss these. Paragraph-level co-occurrence may produce too many candidates.

2. **Relationship vocabulary.** The domain model defines relationship types: `may_be_related`, `contains`, `produces`, `transforms`. But Essay 24's gold standards include relationships like *accelerates*, *reduces*, *enables*, *requires* that aren't in the domain model. The classifier's vocabulary needs to be open enough to capture argumentative relationships, not just structural ones. Fixed vocabulary vs. constrained-generation (pick from a list OR propose a new type) is a design choice.

3. **Input format for classification.** The 8B timeout came from long combined inputs (text + entity list + instructions). A classifier that receives only a sentence and two entity labels is a much shorter prompt. But does stripping context below the sentence level lose the information needed for correct classification? Some relationships are only clear in the paragraph context.

4. **Comparison with deterministic co-occurrence.** The simplest approach: entities appearing in the same sentence become `may_be_related` edges with no model call at all. The graph's reinforcement mechanics (Invariant 45 — per-adapter contribution tracking) would then strengthen edges that multiple sources confirm. This requires no LLM inference for relationships and uses only existing engine machinery. The question is whether untyped `may_be_related` edges are useful enough, or whether typed relationships (contains, produces, enables) carry information that `may_be_related` cannot.

5. **Hybrid approach.** Deterministic co-occurrence generates candidate pairs. A lightweight classifier (potentially even a 1-3B model) classifies the relationship type. This separates candidate generation (fast, deterministic) from typing (cheap model call). The time budget becomes: sentence segmentation + entity pair extraction (deterministic, <1s) + N short classification calls (small model, seconds each) instead of one long generative call that times out.

**Method:** Spike. Test on Essays 02 and 12 against gold-standard relationships. Compare: (a) deterministic co-occurrence alone, (b) sentence-level classification with 8B model, (c) paragraph-level classification, (d) co-occurrence + lightweight classifier hybrid.

---

## Question 2: How do you prime extraction for a vocabulary the model has never seen?

**Why this matters:** On Essay 02 (general concepts), multi-model union reaches 85% entity recall. On Essay 12 (domain-specific), it reaches only 62%. The gap is entirely explained by domain neologisms — compound terms coined in the Plexus corpus that have no training-data presence: *dual obligation*, *cross-dimensional traversal*, *deterministic ID*, *multi-phase processing*, *enrichment* (in Plexus's specific sense).

Both models miss these terms regardless of temperature, context window, or prompting. TextRank priming helps for statistically prominent terms but not for terms that appear only once or twice. The models don't recognize them as entities because they've never seen them used as entities.

**What we already know:**

- TextRank/TF-IDF surfaces statistically significant phrases but doesn't know which are domain entities. On Essay 12, TextRank found "provenance" and "fragment" (high co-occurrence) but not "dual obligation" (low frequency, two common words).
- Qwen and Mistral have different extraction biases (abstract vs. structural), but they converge on the same misses for domain neologisms. Complementarity breaks down when both models hit the same training-data ceiling.
- The domain model (`docs/domain-model.md`) already defines 30+ concepts with precise definitions. This is an existing glossary that no extraction prompt currently references.

**The hypothesis:** The extraction pipeline has a bootstrapping problem. The first extraction of a new corpus is the hardest — the model has no domain vocabulary to work with. But Plexus already has a domain model with defined terms. Priming extraction with the existing glossary (not TextRank output, but the actual domain vocabulary) may recover the missed neologisms.

**Nuances to investigate:**

1. **Glossary priming vs. few-shot examples.** Two approaches: (a) prepend the domain model's concept list to the extraction prompt as "known vocabulary — look for these and anything else," or (b) include 2-3 few-shot examples showing what domain entity extraction looks like ("In the text 'the adapter's dual obligation requires both semantic and provenance output', the entities are: adapter, dual obligation, semantic output, provenance output"). Few-shot may teach the model the *pattern* of domain entity recognition; glossary priming may just bias toward known terms.

2. **The cold-start problem.** Glossary priming works when the domain model exists. But what about the first extraction of a genuinely new corpus? The whole point of extraction is to *discover* vocabulary, not just confirm what's already known. Over-priming could suppress novel entity discovery — the model finds what it's told to look for and stops looking. This tension between recall of known terms and discovery of new ones needs explicit investigation.

3. **Vocabulary drift.** The domain model reflects current understanding. As the corpus grows, new terms emerge. If extraction is primed with a stale glossary, it may miss evolving vocabulary. The glossary needs to be a living artifact — updated by the extraction pipeline itself. This is circular: extraction feeds the glossary feeds extraction. What's the convergence behavior?

4. **Interaction with multi-model union.** If both Qwen and Mistral are primed with the same glossary, they may converge on the same entities — reducing the complementarity that makes union valuable. Priming one model but not the other might preserve complementarity while recovering domain terms. Or priming with different subsets of the glossary.

5. **Prompt length budget.** The domain model's concept table is ~50 entries. Adding it to the extraction prompt increases input by ~500-800 tokens. At 8192 context window, this is affordable but reduces the space available for essay text. For longer documents (post-chunking), the budget gets tighter. Glossary priming may need to be selective — top-N most relevant terms rather than the full vocabulary.

**Method:** Spike. Compare: (a) unprimed baseline (from Essay 24), (b) full glossary priming, (c) few-shot examples, (d) primed Qwen + unprimed Mistral union. Measure both recall of known domain terms and discovery of terms not in the glossary.

---

## Question 3: What does sequential GPU scheduling look like in practice?

**Why this matters less for research, more for engineering.** Essay 24 established that concurrent 8B models on consumer hardware (16-32GB unified memory) cause memory contention and timeouts. The practical architecture sequences models through the GPU. This is an operational constraint, not a research question — but the scheduling strategy affects wall time and pipeline design.

**What we already know:**

- Two qwen3:8b agents running concurrently cause timeouts at 87%+ memory usage.
- Sequencing through the GPU gives: TextRank (<1s, CPU) → Qwen entities (~60s) → Mistral entities (~25s) → themes (~50s). Total ~2-3 minutes per essay.
- llm-orc's parallel DAG topology tries to run agents concurrently. Achieving sequential GPU access within a parallel DAG may require llm-orc changes or a different orchestration approach.

**Nuances:**

1. **Ollama model loading/unloading.** When Qwen finishes and Mistral starts, does Ollama unload Qwen from GPU memory? The `OLLAMA_KEEP_ALIVE` setting controls this. If Qwen stays loaded, Mistral competes for memory. If Qwen unloads, Mistral gets full GPU but there's a ~5-10s model load penalty. The optimal setting depends on total available memory.

2. **llm-orc sequencing.** The current ensembles use parallel topology, which llm-orc runs concurrently. Sequential topology (entity → relationship) exists but is designed for data flow, not resource management. We may need a "resource-sequential" topology that sequences agents for GPU access while still treating them as independent (no data dependency). Or the solution is simpler: run separate ensembles in sequence via a shell script.

3. **Batch processing.** For extracting an entire corpus (25+ essays), the 2-3 minute per-essay wall time means 50-75 minutes total. Acceptable as a one-time batch job. But incremental extraction (new essay added to corpus) should not require re-extracting everything. The adapter pipeline's idempotent emission model (Invariant 19 — deterministic concept IDs) means re-extraction of an unchanged essay produces the same emissions. The question is whether the orchestration layer can detect "already extracted" and skip.

**This question is lower priority.** It's engineering work that follows naturally once Q1 and Q2 produce a complete extraction pipeline. Capture here for context but don't spike until the extraction quality questions are resolved.

---

## Research Plan

**Priority order:** Q1 → Q2 → Q3

Q1 (relationship classification) is the critical path — it's the only extraction dimension that doesn't work yet. Q2 (domain vocabulary priming) improves entity recall from 62% to potentially higher on domain-specific text, but 62% is workable; 31% relationship recall is not. Q3 is engineering that follows implementation.

**Dependencies:** Q1's relationship classifier may benefit from Q2's vocabulary priming (the classifier needs entity labels, and better entity extraction means better candidate pairs). But Q1 can be investigated independently using gold-standard entity lists as input — isolating the classification question from entity quality.
