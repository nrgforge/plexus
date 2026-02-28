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

## Question 0: Can chunked input make the 14B synthesizer viable?

**Why this matters:** Essay 24 rejected the 14B model as too expensive: "87% of system memory and 3+ minutes per run." But the 336s measurement was for the full synthesized pipeline (three 8B specialists competing for GPU + the 14B synthesizer). The 14B model alone, given entity context, demonstrated 80-100% relationship recall — the quality we need. If chunking the input made the 14B model fast enough, the classification approach (Q1) would be unnecessary.

**The hypothesis:** Chunking input into section-level segments would allow Qwen3:14b to complete relationship extraction within 120s per chunk, making total wall time competitive with a single full-text pass.

**Method:** Spike. Ran Qwen3:14b on Essay 02 via Ollama API with gold-standard entity context. Compared: (a) full essay text as single input, (b) essay split into 6 sections, each processed independently.

**Findings:**

| Run | Input chars | Gen tokens | Chars/token | Total time |
|-----|-------------|------------|-------------|------------|
| **Baseline (full text)** | 5,515 | 1,379 | 1.72 | **125.4s** |
| Section 1 (Problem) | 1,012 | 3,118 | 0.35 | 248.5s |
| Section 2 (Cognitive) | 976 | 3,522 | 0.28 | 282.9s |
| Section 3 (AI Acute) | 892 | 1,891 | 0.46 | 147.2s |
| Section 4 (Prompting) | 537 | 2,227 | 0.34 | 172.4s |
| Section 5 (Remedy) | 1,223 | 2,312 | 0.44 | 184.5s |
| Section 6 (Caveat) | 628 | 2,343 | 0.25 | 180.4s |
| **Section total** | — | — | — | **1,215.9s** |

**Result: Hypothesis rejected.** Chunking is 9.7x slower than the baseline.

**F0a: Shorter input triggers more hidden thinking.** Qwen3 generates 2-3x more tokens per section than for the full essay, but the visible output (JSON relationships) is smaller. The chars/token ratio drops from 1.72 (baseline) to 0.25-0.46 (sections). The model is producing extensive invisible thinking tokens despite the `/no_think` flag. On short inputs, the thinking suppression is unreliable — the model compensates for less context by speculating more.

**F0b: The 14B model is already viable on full text.** The baseline completed in 125.4s — well within the 180s profile timeout. Essay 24's "3+ minutes" included specialist agents competing for GPU memory. When the 14B model runs alone (no GPU contention), it processes the full essay in ~2 minutes. This changes the calculus.

**F0c: Baseline recall is lower than Essay 24 suggested.** Against gold-standard relationships, the baseline achieved ~3 clear matches out of 10 (30%) with 2 partial matches. This is better than isolated 8B extraction (31-35%) but well below the "80-100%" cited in Essay 24. The difference: Essay 24's synthesizer received specialist outputs as additional context; this run received only entity labels and raw text. The entity *list* helps, but the specialist outputs' evidence and framing may have contributed to the higher recall.

**F0d: Chunked recall is modestly better but not worth the cost.** Merging results across 6 sections produces 5 clear matches (50%), because some relationships are only visible in their section context (e.g., "AI-assisted composition accelerates opacity problem" appears only in the "AI Makes It Acute" section). But the 10x time cost and higher false positive rate make this a poor tradeoff.

**Implications:**

1. The 14B model is a viable relationship extractor at ~125s when run alone with entity context. The constraint is GPU scheduling (can't run concurrently with other models), not model capability.

2. Q1 (classification approach) is still worth investigating — not because the 14B model is too expensive, but because an 8B classifier might achieve similar recall in less time. The comparison is now: 14B generation at ~125s/~30-50% recall vs. 8B classification at unknown time/unknown recall.

3. The baseline's lower-than-expected recall (30-50% vs. Essay 24's claimed 80-100%) suggests the relationship extraction prompt needs refinement. The prompt constrains to entity labels but doesn't provide enough context about what relationships to look for. The Essay 24 synthesizer had richer context (specialist outputs) that this isolated prompt lacks.

4. Running the 14B model twice with different temperatures and unioning results (analogous to the multi-model entity strategy) might be cheaper than chunking while capturing more relationships through variation.

**Spike artifacts:** `scratch/spike-14b-chunking/` (results JSON, experiment script, findings).

### Q0 Addendum: Structured output and model comparison

The chunking spike revealed that Qwen3's thinking token leak was the dominant timing problem. A follow-up tested Ollama's structured output feature (`format` parameter with JSON schema), which constrains the model to produce only tokens conforming to a schema — physically preventing thinking tokens.

**Structured output comparison (full Essay 02 text, all models):**

| Model | Mode | Total | Gen time | Rels | Clear gold | Partial |
|-------|------|-------|----------|------|-----------|---------|
| Qwen3:14b | unconstrained | 125.4s | 112.5s | 10 | 3 | 2 |
| Qwen3:14b | structured | 149.1s | 67.0s | 14 | 4 | 4 |
| Nemo:12b | structured | 30.0s | 24.8s | 7 | 2 | 3 |
| Qwen3:8b | structured | 147.0s | 37.9s | 12 | 3 | 3 |
| Mistral:7b | structured | 50.0s | 39.7s | 15 | 4 | 3 |

**F0e: Structured output eliminates thinking tokens and improves quality.** With JSON schema constraint, chars/token jumps from 0.25-1.72 to 4.66-4.86 across all models. Every generated token is productive JSON. Qwen3:14b finds 14 relationships (up from 10), Mistral:7b finds 15 (up from no JSON at all without the constraint). The schema does more for quality than increasing model size.

**F0f: Qwen3 has ~100s structured output overhead.** Both Qwen3:8b (147s total, 38s gen) and Qwen3:14b (149s total, 67s gen) show ~100s of unexplained overhead when using structured output. Likely grammar compilation interacting with the thinking architecture. Mistral and Nemo have 3-5s overhead. This makes Qwen3 models poor choices for schema-constrained extraction despite strong recall.

**F0g: Mistral:7b is the best value for relationship extraction.** 50s total, 15 relationships, 4 clear gold matches — matching Qwen3:14b's recall at one-third the time and one-third the model size. Mistral finds relationships the larger models miss (vibe-coding exemplifies opacity, epistemic distinct_from pragmatic). The 7B model with structured output outperforms the 14B model without it.

**F0h: Model size is not the bottleneck — structured output is.** Essay 24 tested 8B models without format constraint and got 31-35% recall. With the constraint, Mistral:7b gets ~40-70%. The format constraint does more work than the extra 6-8B parameters. This invalidates the Essay 24 conclusion that "7-8B models may not be strong enough for abstract relationship identification."

### Q0 Addendum: Run-to-run determinism

Tested whether multiple runs of the same model produce equivalent complementarity to a multi-model union. Ran Mistral:7b structured output 3 times on the same input at temperature 0.3.

| Metric | Value |
|--------|-------|
| Pairs per run | 14-16 |
| Union (all 3 runs) | 32 unique pairs |
| Intersection (all 3 runs) | 4 stable pairs |
| **Stability** | **12%** |

**F0i: Same-model runs are highly non-deterministic.** At temperature 0.3, only 12% of entity pairs appear in all 3 runs. Each run explores a different slice of the relationship space. Run 1 found gold #2 (ESR remedies opacity) while runs 2 and 3 did not. Run 3 found gold #3 (vibe-coding exemplifies opacity) while runs 1 and 2 did not.

**F0j: Multi-run union is a viable strategy.** The 32-pair union across 3 runs covers more gold relationships than any single run. Total time: ~150s (3 × 50s sequential). This is comparable to a Qwen3:14b single run (149s) but produces 2x more candidate relationships with genuine variation.

**F0k: Reinforcement mechanics are the right denoising strategy.** With 12% stability, multi-run union produces significant noise (wrong entity pairs, entities not in the provided list). But Plexus's per-adapter contribution tracking (Invariant 45) handles this naturally: each run gets a distinct adapter ID, relationships found by multiple runs accumulate contributions, single-run relationships stay at low weight. The stable core self-identifies through convergence. This is the Hebbian pattern from Essay 06 applied to relationship extraction.

---

## Q0 Summary

The original question — "can chunked input make the 14B synthesizer viable?" — was wrong. The research uncovered a more fundamental finding: **structured output (JSON schema constraint) is the key intervention for relationship extraction**, not model size or input chunking.

The emerging architecture for relationship extraction:
1. Run Mistral:7b with structured output 3 times sequentially (~150s total)
2. Merge results with per-run contribution tracking
3. Graph reinforcement surfaces stable relationships, suppresses noise
4. No 14B model needed. No chunking needed. No classification needed.

This substantially reshapes the remaining questions:
- **Q1 (classification)** may be unnecessary — generative extraction with structured output achieves workable recall. Classification could still be tested as a comparison, but it's no longer the critical path.
- **Q2 (vocabulary priming)** is still relevant — domain neologisms are still missed regardless of model or format constraint.
- **Q3 (GPU scheduling)** is simplified — single-model sequential runs avoid multi-model contention entirely.
- **New: llm-orc structured output support** — adding the `format` parameter to llm-orc's Ollama provider is a prerequisite for using these findings in the extraction pipeline.

---

## Q2 Findings: Domain Vocabulary Priming

### 2×2 Matrix (Mistral:7b on Essay 12)

Tested two independent variables — glossary priming (domain model concept names in prompt) and structured output (JSON schema constraint) — in a 2×2 design against Essay 12's 13 gold-standard entities.

| Condition | Total | Entities | Gold recall | Key finding |
|-----------|-------|----------|-------------|-------------|
| A: Unprimed + Free | 36.9s | 38 | ~54% | Finds component names, 2.9× overgeneration |
| B: Unprimed + Structured | 12.7s | 13 | ~31% | Fast but too abstract, misses domain terms |
| C: Primed + Free | 20.6s | 15 | ~58% | Clean normalized terms, best single-condition |
| D: Primed + Structured | 15.6s | 15 | ~54% | Clean terms, unique find: "enrichment" |
| E: Fewshot + Primed + Structured | 36.1s | 15 | ~38% | Shifts focus, doesn't broaden coverage |

**F2a: Glossary priming normalizes domain vocabulary.** Primed conditions (C, D) extract clean, well-normalized terms (provenance, adapter, fragment, mark, chain) instead of noisy paraphrases ("provenance marks", "fragment node", "adapter-shaped things"). Priming shifts the model from describing to naming.

**F2b: Structured output alone hurts entity extraction.** Condition B is the worst at 31% recall. Unlike relationship extraction (where schema constraint improved quality in Q0), entity extraction benefits more from vocabulary context than from output format. The schema constrains shape but doesn't help the model know what to look for.

**F2c: Few-shot examples shift focus rather than broadening it.** Condition E recovered TagConceptBridger and Hebbian contribution (lost in C/D) but lost provenance, epistemology, fragment, and multi-phase processing. Net effect: lower recall than simple glossary priming.

**F2d: Three categories of missed entities require different interventions.**

| Category | Examples | Intervention |
|----------|----------|-------------|
| Normalized domain terms | provenance, adapter, fragment | Glossary priming (solved) |
| Component/class names | TagConceptBridger, EngineSink | Unprimed extraction or CamelCase regex |
| Compound neologisms | dual obligation, cross-dimensional traversal | NLP phrase extraction or acceptance |

**F2e: Priming suppresses verbatim component names.** TagConceptBridger was found by condition A (unprimed) but lost in C and D (primed). The glossary biases toward normalized vocabulary, suppressing CamelCase identifiers. Trade-off: better normalization, worse verbatim recall.

**F2f: Some gold entities are concept recognition, not extraction.** "dual obligation" never appears as a phrase in Essay 12. The human annotator recognized the concept from domain knowledge. An extraction model can only extract what the text says. Entity *extraction* recall and entity *recognition* recall are different metrics.

### Multi-Run Union

Tested multi-run union at temperature 0.3 to exploit run-to-run variation, plus cross-condition union combining primed and unprimed approaches.

| Strategy | Recall | Unique entities | Time |
|----------|--------|----------------|------|
| Single D run | 54% | 15 | ~16s |
| D union (3 runs) | **69%** | 21 | ~51s |
| Full union (3×D + 1×A) | **~85%** | 44 | ~78s |

**F2g: Multi-run union closes the domain recall gap.** Three glossary-primed runs union to 69% recall (up from 54% single-run). Adding one unprimed run pushes to ~85% — matching the 85% achieved on general-text Essay 02 in Essay 24. The domain-specific recall gap is eliminated through strategy, not through a better model.

**F2h: Run stability is 24%.** Of 21 unique entities across 3 D-runs, only 5 appear in all three (adapter, epistemology, graph, plexus, provenance). Each run explores different vocabulary. This is the same low-stability pattern as Q0's relationship extraction (12%), confirming that multi-run union is a general strategy, not a subtask-specific one.

**F2i: Cross-condition union captures what single-condition union cannot.** The unprimed run (A) contributes "multi-phase processing", "sink", and descriptive terms that no primed run found. The primed runs contribute "enrichment", "fragment", "concept" — normalized domain terms the unprimed run missed. Neither approach alone reaches 85%; together they do.

**F2j: Reinforcement mechanics are the denoising strategy.** The 44-entity full union includes 3.4× overgeneration vs. gold. But per-adapter contribution tracking (Invariant 45) handles this: entities found by 3+ runs accumulate contributions (stable core at high weight), single-run entities stay at minimum weight. The graph's existing mechanics turn noisy extraction into calibrated confidence.

**Spike artifacts:** `scratch/spike-vocab-priming/` (experiment scripts, results JSON, analysis).

---

## Q2 Summary

The domain recall gap (85% general → 62% domain-specific) was a strategy gap, not a capability gap. The solution combines two interventions already validated by Q0:

1. **Glossary priming** — normalizes extraction toward domain vocabulary
2. **Multi-run union with mixed conditions** — 3× primed + 1× unprimed covers both normalized terms and verbatim component names

The Phase 3 (semantic, LLM) entity extraction architecture:
1. 3× glossary-primed + structured output runs (~51s sequential)
2. 1× unprimed + free run (~28s) for discovery and component names
3. Each run gets a distinct adapter ID (Invariant 45)
4. Reinforcement mechanics surface stable entities, suppress noise
5. Total: ~78s, ~85% recall on domain-specific text

Remaining misses: compound neologisms that look like ordinary English ("cross-dimensional traversal") and concepts not named in the source text ("dual obligation"). These are addressable by Phase 2 (heuristic, Rust) — CamelCase regex for component names, NLP noun phrase extraction for compound terms. Phase 2 + Phase 3 contributions accumulate independently per Invariant 45.

---

## Q1 Findings: Phase 2 Deterministic Extraction and Phase 2→3 Interaction

### Phase 2 Spike: Deterministic Relationship Extraction

Tested three deterministic approaches on Essay 12 using spaCy (`en_core_web_sm`) against the domain model's entity vocabulary (37 terms). No model inference required — runs in <1s on CPU.

**Approaches tested:**

| Approach | Output | Gold recall |
|----------|--------|-------------|
| Sentence co-occurrence | 209 `may_be_related` pairs | 6/8 (75%) |
| Dependency parsing (SVO) | 28 typed relationships | 4/8 (50%) |
| Verb pattern matching | 3 typed relationships | 2/8 (25%) |
| Combined (typed + co-occurrence) | 28 typed + 209 untyped | ~5.5/8 (69%) |

**F1a: Sentence co-occurrence alone captures most gold relationships.** 6 of 8 gold relationships have both entities co-occurring within the same sentence. The two misses are abstract (provenance→epistemological foundation, where "epistemology" appears in the title but not near "provenance" in a sentence) and structural (multi-phase→separate chains, which spans a paragraph). Co-occurrence is the strongest single deterministic signal.

**F1b: Dependency parsing adds relationship types but limited new coverage.** The 28 typed relationships include `adapter produces provenance`, `fragment produces provenance mark`, `enrichment create references` — all correct and matching gold standard vocabulary. But these are mostly subsets of what co-occurrence already found. Dependency parsing adds the verb (produces, creates, carries) but rarely finds entity pairs that co-occurrence missed.

**F1c: Verb pattern matching is too fragile for complex sentences.** Only 3 relationships found — the essay's argumentative prose breaks SVO extraction. Sentences like "Chains, marks, and links are not just a user annotation feature" have complex clause structure that simple regex patterns miss.

**F1d: Phase 2 is free and fast but cannot find abstract relationships.** All deterministic approaches miss Gold #1 (provenance provides epistemological foundation) because this relationship is an argumentative claim, not a syntactic structure. The essay argues for this relationship across paragraphs; no sentence-level pattern captures it. This is precisely what Phase 3 (LLM) should contribute.

### Phase 3 Comparison: LLM Only vs Phase 2 Primed

Tested whether feeding Phase 2's typed relationships to the LLM improves its relationship extraction. Condition F is LLM-only baseline; condition G/G2 primes the LLM with Phase 2's dependency parsing output.

| Condition | Rels | Time | Gold recall | Key characteristic |
|-----------|------|------|-------------|-------------------|
| F: LLM only | 37 | 108s | ~6.5/8 (81%) | Creative, finds abstract rels, noisy |
| G: Phase 2 primed (unlimited) | ~1000+ | 56min | N/A | Repetition loop — runaway generation |
| G2: Phase 2 primed (2048 tokens) | 16 | 41s | ~3/8 (38%) | Validated Phase 2, didn't extend |

**F1e: Phase 2 priming constrains the LLM rather than enriching it.** G2 produced 16 clean, well-formed relationships — but 14 of 16 are reformulations of the Phase 2 input. The model validated and rephrased what it was given (`adapter produces provenance` → `FragmentAdapter produces provenance`, `fragment get mark` → `Fragment contains mark`) instead of extending it. Novel relationships like Gold #1 (provenance provides epistemological foundation) and Gold #8 (TCB bridges dimensions) were not found.

**F1f: The unprimed LLM finds what Phase 2 cannot.** Condition F (LLM only) found `provenance component_of epistemology` and `provenance enables semantics` — the abstract argumentative relationship that Phase 2 missed entirely. It also found `TagConceptBridger creates references edges` and `multi-phase processing requires heterogeneous adapters`. These are the LLM's natural strength: recognizing claims and arguments in prose that no syntactic parser can identify.

**F1g: Phase 2 priming without token limit causes runaway generation.** Condition G (no `num_predict` limit) generated 81,920 tokens over 56 minutes, entering a repetition loop. The structured output schema constrains format but not length. The Phase 2 context (~600 tokens of typed relationships + entity co-occurrence summary) gave the model too much to validate, and without a token cap the validation spiral continued indefinitely. Token limits are mandatory for primed extraction.

**F1h: Phase 2 and Phase 3 should operate independently, not sequentially.** The experiment directly tested the Phase 2→3 priming hypothesis and found it counterproductive. Priming makes the LLM lazy — it validates what it's given instead of exploring. The better architecture is independent operation: Phase 2 contributes `may_be_related` edges and typed relationships from co-occurrence/dep parsing; Phase 3 contributes typed relationships from generative extraction. Both accumulate per Invariant 45's per-adapter contribution tracking. Relationships found by both phases get higher weight through reinforcement. This is additive, not sequential.

### Combined Phase 2 + Phase 3 Architecture (Independent)

| Phase | Method | Gold recall | Time | Cost |
|-------|--------|-------------|------|------|
| Phase 2 | Co-occurrence + dep parsing | 5.5/8 (69%) | <1s | CPU only |
| Phase 3 | LLM (Mistral:7b) unprimed | 6.5/8 (81%) | 108s | GPU |
| **Union** | **Both, independent** | **~7.5/8 (94%)** | **~109s** | **CPU + GPU** |

The union covers nearly all gold relationships: Phase 2 finds the structural relationships (chain→mark, adapter→provenance, fragment→mark) with zero model cost, and Phase 3 finds the abstract relationships (provenance→epistemology, TCB bridges dimensions) that require reasoning. The only potential miss is Gold #7 (multi-phase uses separate chains), which Phase 3 captures partially (multi-phase requires distinct input kinds).

**Spike artifacts:** `scratch/spike-vocab-priming/` (Phase 2 spike script, F/G/G2 results, phase2-context.txt).

---

## Q1 Summary

Phase 2 deterministic extraction is both viable and complementary to Phase 3 LLM extraction:

1. **Phase 2 (co-occurrence + dependency parsing)** achieves 69% gold recall on relationships at zero model cost in <1s. It reliably finds structural relationships where both entities co-occur in a sentence.

2. **Phase 3 (LLM, unprimed)** achieves 81% gold recall and finds abstract/argumentative relationships that no syntactic parser can detect.

3. **Phase 2→3 priming is counterproductive.** Feeding Phase 2 output to the LLM produced worse results (38%) than either approach alone. The LLM validated instead of extending.

4. **The right interaction is independent accumulation.** Phase 2 and Phase 3 contribute independently through separate adapter IDs. Invariant 45's per-adapter contribution tracking handles denoising: relationships found by both phases accumulate higher weight. The graph's existing mechanics compose the phases without either needing to know about the other.

5. **Union recall (~94%) exceeds either phase alone.** The phases have complementary strengths: Phase 2 for structural/syntactic relationships, Phase 3 for abstract/argumentative ones. Only one gold relationship (multi-phase uses separate chains) is borderline — it requires paragraph-level reasoning that neither sentence co-occurrence nor single-sentence LLM extraction fully captures.

---

## Research Plan

**Revised priority order:** ~~llm-orc `format` support~~ (done, 0.15.11) → ~~Q2~~ (done) → ~~Q1~~ (done) → Q3

Q0 established structured output as the key intervention for relationship extraction. Q2 established glossary priming + multi-run union as the solution for entity extraction on domain text. Q1 established that Phase 2 deterministic extraction and Phase 3 LLM extraction are complementary and should operate independently, not sequentially. Together, these findings define the extraction architecture:

**Phase 2 (heuristic, Rust/Python, <1s):**
- Sentence co-occurrence → `may_be_related` edges (209 pairs on Essay 12)
- Dependency parsing → typed relationships (28 on Essay 12)
- CamelCase regex → component entity names (from Q2 F2d)
- Each approach gets a distinct adapter ID

**Phase 3 (semantic, LLM via llm-orc, ~2-3 min):**
- **Entities:** 3× glossary-primed + 1× unprimed, Mistral:7b, structured output, ~78s
- **Relationships:** 3× Mistral:7b unprimed, structured output with relationship schema, ~150s
- **Themes:** already solved (Essay 24, 100% recall)
- Each run gets a distinct adapter ID

**Cross-phase interaction:** Independent accumulation via Invariant 45. No priming or data passing between phases. Reinforcement mechanics surface relationships confirmed by multiple sources.

**Q3 (GPU scheduling)** is simplified by llm-orc 0.15.11's max concurrency setting and the single-model sequential architecture (all runs use Mistral:7b, no multi-model GPU contention).

**Next:** Essay 25 — capture Q0, Q1, and Q2 findings. Then build the extraction ensemble.
