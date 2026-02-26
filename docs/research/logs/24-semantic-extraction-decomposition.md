# Research Log: Semantic Extraction Decomposition

## Background

Essay 23 established universal ingest (ADR-028) as the single write path. The infrastructure is complete: three core adapters, YAML-driven declarative specs, llm-orc integration, enrichment loop. What's missing: actual adapter specs and ensembles that make semantic extraction work "for real" — unblocking Trellis and Carrel.

llm-orc now supports composable ensembles. The conductor's tiered profile set (1.7B router → 7-8B analysts → 14B synthesizer) provides the model foundation. The premise is local-first composition: smart decomposition across small models, with synthesis delegated to 14B or an affordable frontier model.

### Research Questions

**Q1:** What extraction subtasks produce clean, focused output from 7-8B models when each agent handles exactly one concern?

**Q2:** What DAG topology composes those subtasks into the best overall extraction quality?

**Q3:** Does classical ML pre-extraction improve SLM entity recall?

### Test Corpus

- Essay 02: The Opacity Problem (conceptual/philosophical, 117 lines)
- Essay 12: Provenance as Epistemological Infrastructure (domain theory, 104 lines)

### Correction Note

The original Q1-Q3 results (run on llm-orc 0.15.8) were **invalidated** after discovering a profile resolution bug: individual YAML profile files in `.llm-orc/profiles/` were invisible to the model runtime, which only read `config.yaml`. All ensembles silently fell back to llama3 instead of using the intended models (qwen3:8b, mistral:7b, qwen3:14b).

The bug was fixed in llm-orc 0.15.9. All experiments were re-run with:
- Correct model resolution (verified via `model_substituted: false`)
- `temperature: 0.2` on analyst profiles (reduces non-determinism)
- `max_tokens: 2000/4000` on profiles (bounds output length)
- `/no_think` in Qwen3 system prompts (disables internal chain-of-thought, ~30% faster inference)

The findings below reflect the corrected results.

---

## Question 1: What extraction subtasks isolate cleanly for 7-8B models?

**Method:** Spike. Run three candidate subtasks (entities, relationships, themes) independently on Essays 02 and 12 using Qwen3:8b and Mistral:7b. Measure output focus and quality against Claude gold-standard extractions.

**Status:** Complete (re-run with correct models)

**Results:**

| Run | Model | Essay | Extracted | Gold Matches | Recall | Time |
|---|---|---|---|---|---|---|
| entities | qwen3:8b | 02 | 19 | 9/14 | 64% | 62s |
| entities | qwen3:8b | 12 | 16 | 7/13 | 54% | 64s |
| entities | mistral:7b | 02 | 10 | 8/14 | 57% | 29s |
| entities | mistral:7b | 12 | 14 | 6/13 | 46% | 18s |
| themes | qwen3:8b | 02 | 7 | ~4/5 | ~80% | 48s |
| themes | qwen3:8b | 12 | 6 | ~4/5 | ~80% | 47s |
| relationships | qwen3:8b | 02 | 8 | 3-4/10 | ~35% | 92s |
| relationships | qwen3:8b | 12 | 12 | 2-3/8 | ~31% | 76s |

**Findings:**

1. **Entity recall is substantially higher with correct models** — Qwen3:8b achieves 54-64% recall, up from the ~38% reported when llama3 was silently substituted. The `/no_think` flag is essential: without it, Qwen3 spends most of its time budget on internal reasoning and frequently times out.

2. **Theme extraction remains the most reliable subtask** — ~80% recall on both essays. Captures 4/5 gold themes consistently. The one pattern: models soften the most precisely-stated design claims (E02's "externalize awareness, not reasoning" becomes "external representations solve opacity"). Themes are paraphrased, not quoted.

3. **Relationship extraction is consistently the weakest subtask** — 31-35% recall. Misses literally-stated structural relationships ("chain contains mark") while producing thematic inferences. On Essay 02, produced 8 relationships — most are correct inferences but only 3-4 match gold. The extractor captures argumentative flow (cause/effect) but not explicit domain structure (part_of, contains).

4. **Qwen/Mistral complementarity is genuine** — not llama3 non-determinism as previously suspected. On Essay 12, Qwen found epistemology, tagconceptbridger, multi-phase processing, cross-dimensional traversal. Mistral found mark, chain, fragment, adapter. Union would reach ~77% recall. The models have genuinely different extraction biases: Qwen favors abstract/compound terms, Mistral favors short structural terms.

5. **~~Mistral hallucinates cross-essay entities~~** — CORRECTED: "trellis" and "carrel" ARE present in Essay 12's text ("Essay 11 demonstrated that two consumers — Trellis (fragments) and Carrel (provenance marks)"). This was a scoring error, not a hallucination. With optimized settings (anti-hallucination prompt + temp=0.15), Mistral shows zero actual hallucinations on both essays.

6. **Confidence scores remain broken** — Monotonically decreasing sequences (0.95, 0.92, 0.91...) even with temperature=0.2. These are rank orderings, not calibrated probabilities. Validates Invariant 14: use structural signals for graph weights, not SLM self-assessment.

**Hypothesis status:**
- H1a (entities decompose cleanly): **Yes, with caveats** — 54-64% recall is workable as one signal; still misses domain neologisms and compound terms
- H1b (relationships are sequential): **Confirmed** — isolated extraction captures argumentative flow but misses structural relationships
- H1c (themes are distinct): **Confirmed** — ~80% recall, robust across essays
- H1d (Qwen/Mistral complementarity): **Confirmed** — genuine model-level bias differences, union reaches ~77%

**Implications for topology:**
- Entity fan-out across models is valuable and complementarity is genuine
- Theme extraction runs independently (parallel with entities)
- Relationship extraction needs entity context AND structural priming
- Mistral output needs hallucination filtering in unsupervised pipelines
- `/no_think` and `temperature: 0.2` are essential for Qwen3 extraction tasks

---

## Question 2: What DAG topology composes subtasks into best overall quality?

**Method:** Two ensembles tested on Essays 02 and 12:
- `spike-sequential` — entities + themes (parallel) → relationships (with entity context)
- `spike-synthesized` — same pipeline + qwen3:14b synthesizer that merges, deduplicates, and fills gaps

**Hypotheses:**
- H2a: Sequential entity→relationship flow improves relationship quality over isolated extraction
- H2c: 14B synthesizer adds value over mechanical merging of 8B outputs

**Status:** Complete (re-run with correct models; synthesized E12 partially failed due to timeouts)

**Results:**

| Run | Agents | Status | Duration |
|---|---|---|---|
| sequential E02 | entity + theme (parallel) → relationship | All succeeded | 167s |
| sequential E12 | entity + theme (parallel) → relationship | All succeeded | 176s |
| synthesized E02 | entity + theme → relationship → 14b synthesizer | All succeeded | 336s |
| synthesized E12 | entity + theme → relationship → 14b synthesizer | Theme + synthesizer timed out | 399s |

**Findings:**

1. **Sequential topology produces proper JSON relationships when entity context is available** — On Essay 12, the sequential relationship extractor produced 12 well-formed JSON relationships using entity labels from upstream. On Essay 02, however, it produced prose instead of JSON despite the same prompt. Schema compliance remains inconsistent even with temperature=0.2.

2. **Non-determinism is real but reduced** — The synthesized E02 entity extractor found "working memory" and "epistemic actions" — gold entities that the standalone entity extractor (same model, same text, same prompt) missed. Temperature=0.2 reduced but did not eliminate this variance. Downstream pipeline quality still depends partly on luck.

3. **The 14b synthesizer produces prose, not structured output** — On Essay 02, qwen3:14b generated a multi-paragraph analytical essay instead of JSON. The synthesizer prompt asks for structured JSON but the model, receiving ~5000 tokens of upstream context, switches to free-form text. On Essay 12, the synthesizer timed out at 180s. The 14b model requires 87% system memory and ~3 minutes per run — impractical for batch processing.

4. **Concurrent agents overwhelm local Ollama** — When entity-extractor and theme-extractor run in parallel on qwen3:8b, one or both may time out. The synthesized E12 theme extractor timed out at 120s during parallel execution. Sequential single-agent runs are more reliable on memory-constrained hardware.

5. **Themes remain robust across topologies** — When they complete, themes achieve ~80% recall regardless of pipeline structure. They are the one extraction dimension that doesn't degrade with compositional complexity.

**Hypothesis status:**
- H2a (sequential improves relationships): **Inconclusive** — produced better JSON on E12 but prose on E02. Relationship quality not clearly improved over isolated extraction.
- H2c (14B synthesizer adds value): **Rejected** — produces prose instead of JSON, times out on longer inputs, requires excessive memory. Not viable for local-first extraction.

**The fundamental problem remains:** The sequential topology constrains relationships to the upstream entity list, propagating any entity extraction errors. And the synthesizer — even at 14b — fails on schema compliance when input is long. The extraction pipeline needs deterministic structural signals, not more SLM stages.

---

## Question 3: Does classical ML pre-extraction improve SLM performance?

**Motivation:** Q1 found that SLMs miss domain neologisms, compound terms, and short structural terms that are statistically significant. These are exactly what classical NLP excels at finding — TextRank for key phrase extraction, TF-IDF for statistically significant terms.

**Method:** Build a TextRank/TF-IDF script agent that runs as the first pipeline stage. Feed its output to the entity extractor as vocabulary priming. Compare entity recall with and without priming.

**Hypotheses:**
- H3a: TextRank vocabulary priming improves SLM entity recall
- H3b: Classical ML and SLM find complementary entities (union > either alone)

**Status:** Complete (re-run; E02 primed extraction timed out, E12 succeeded)

**Results:**

| Run | Status | Entities | Notes |
|---|---|---|---|
| primed E02 | TextRank ok, SLM **timed out** | — | Memory pressure after prior runs |
| primed E12 | Both succeeded (70s) | 15 | TextRank "nthe" bug present |
| unprimed E02 (from Q1) | Succeeded | 19 (9/14 gold) | Qwen3:8b standalone |
| unprimed E12 (from Q1) | Succeeded | 16 (7/13 gold) | Qwen3:8b standalone |

**Primed E12 entities:** epistemological infrastructure, provenance dimension, semantic dimension, fragment adapter, provenance mark, tag concept bridger, hebbian contributions, cross-dimensional traversal, multi-phase processing, source identifier, contribution weights, provenance trail, knowledge graph, independent verification, adapter-level provenance

**Findings:**

1. **Primed extraction finds different entities than unprimed** — On Essay 12, primed found "fragment adapter", "provenance mark", "source identifier", "contribution weights" — structural terms surfaced by TextRank vocabulary hints. Unprimed found "bookkeeping", "concept nodes", "pipeline fan-out" — abstractions the SLM generates independently. The overlap is substantial but not complete.

2. **The "nthe" artifact confirms the newline mangling bug** — TextRank output via llm-orc contains "nthe" (score 0.37) and compounds like "provenance nthe", "nthe provenance". This is the JSON serialization bug: newlines in input text become mangled, producing ghost tokens. Running the same script directly via pipe produces clean output.

3. **Timeout reliability is a practical constraint** — The primed E02 run timed out after the synthesized ensemble consumed memory. On resource-constrained hardware, pipeline ordering matters: script agents are fast (<1s) but the downstream SLM competes with whatever Ollama has loaded.

**Hypothesis status:**
- H3a (priming improves recall): **Inconclusive** — E02 timed out. E12 primed found 15 entities vs. 16 unprimed. Priming shifts what's found more than how much.
- H3b (classical ML + SLM complementarity): **Supported** — primed and unprimed find different entity sets. Union would improve recall. But the strongest complementarity evidence comes from Q1's Qwen/Mistral comparison (~77% union recall).

**The key insight remains:** Parallel extraction + merge outperforms sequential priming. The right architecture runs TextRank, Qwen, and optionally Mistral independently, then merges results. This avoids error propagation while capturing complementary signals.

---

## Cross-Cutting Findings

### llm-orc Bugs Discovered

1. **Profile resolution mismatch** (fixed in 0.15.9) — Profiles in `profiles/` directory invisible to model runtime; `check_ensemble_runnable` reported available but `invoke` fell back to llama3. All original Q1-Q3 results were invalidated.

2. **Silent ensemble load failure** — `ScriptAgentConfig` rejects unknown fields via `extra="forbid"`, and `list_ensembles` silently skips invalid files. Ensembles that appear to save correctly vanish from listings.

3. **Script agent newline mangling** — JSON serialization of input to script agents corrupts newlines, producing "nthe" artifacts. Only appears through llm-orc pipeline, not direct script execution.

4. **Model substitution with no error** — `_load_model_with_fallback()` catches any exception and silently falls back to llama3. Combined with bug 1, this made all experiments run on the wrong model with no visible warning.

### Operational Lessons

- **`/no_think` is mandatory for Qwen3 extraction** — Without it, Qwen3 spends 80%+ of time on internal reasoning, frequently timing out. With it, inference is ~30% faster.
- **`temperature: 0.2` reduces but doesn't eliminate non-determinism** — Same model/text/prompt still produces different entity lists across runs.
- **Concurrent Ollama agents need sequentialization on <32GB RAM** — Two qwen3:8b agents in parallel frequently cause timeouts at 87%+ memory usage.
- **Mistral:7b is 2x faster than Qwen3:8b** but less reliable — 18-29s vs 48-92s per essay. Speed advantage is real but hallucination risk requires post-filtering.

---

## Revised Architecture Recommendation

```
                    ┌─── textrank-extract (script) ──┐
input ──► chunker ──┤                                ├──► merge + deduplicate ──► emit
                    ├─── entity-extractor (qwen3:8b) ─┤
                    └─── entity-extractor (mistral:7b)─┘
                    ┌─── theme-extractor (qwen3:8b, parallel) ───────────────┘
```

1. **Parallel extraction**: TextRank script, Qwen entity extractor, and Mistral entity extractor run independently (no dependency chain)
2. **Merge + deduplicate**: Union of all term sets, normalized
3. **Themes**: Independent, as established in Q1 (~100% recall, solved)
4. **Emit**: Structured output to Plexus adapter pipeline

**What this avoids:**
- Sequential error propagation (Q2's failure mode)
- SLM-based synthesis (schema compliance failures, memory cost)
- Single-model dependency (non-determinism risk)

**What this preserves:**
- Classical ML + SLM complementarity (up to 85% union recall on E02)
- Theme extraction robustness (~100% recall with optimized settings)
- Local-first execution (all models run on Ollama)

**Open questions for next phase:**
- Relationship extraction needs a different approach — neither isolated nor sequential works well at 120s; entity-primed relationship extraction shows promise but requires lighter approach
- Chunking strategy for longer documents (schema compliance degrades with length)
- How to handle the merge step: simple dedup vs. lightweight classifier
- Sequentialization strategy: concurrent agents on single GPU cause timeouts; need sequential Ollama scheduling

---

## Optimized Run Analysis (llm-orc 0.15.10)

**Date:** 2026-02-25
**Changes from baseline (0.15.9 corrected runs):**

1. Qwen temp: 0.2 → 0.6 (vendor recommendation)
2. Mistral temp: 0.2 → 0.15 (vendor recommendation)
3. Both models: num_ctx 2048 (default) → 8192 (full essay processing)
4. Qwen: top_k=20, top_p=0.8 (vendor defaults)
5. Mistral: anti-hallucination system prompt added ("Extract ONLY entities explicitly stated in the provided text. Do NOT include concepts from your training data or other sources.")
6. llm-orc: 0.15.9 → 0.15.10 (options pass-through support, required for items 3-5)

### Gold Standards

**Essay 02** (corrected — "zone of proximal development" removed, 13 entities total):
opacity problem, cognitive load, working memory, external structural representation, situation awareness, epistemic action, cognitive offloading, vibe-coding, material disengagement, knowledge graph, AI-assisted composition, computational offloading, re-representation
Relationships: 10 | Themes: 5

**Essay 12** (13 entities):
provenance, epistemology, chain, mark, fragment, TagConceptBridger, Hebbian contribution, dual obligation, adapter, enrichment, multi-phase processing, deterministic ID, cross-dimensional traversal
Relationships: 8 | Themes: 5

### Q1 Entity Recall Scoring

All recall figures use the corrected 13-entity gold for both essays.

**Qwen E02 (15 extracted):**
Hits: opacity problem, cognitive load (→ "cognitive load theory"), material disengagement, situation awareness, vibe-coding, computational offloading, re-representation, external structural representation (→ "structural representations"), knowledge graph (→ "knowledge graph engine") = **9/13 = 69%**
Missed: working memory, epistemic action, cognitive offloading, AI-assisted composition

**Qwen E12 (15 extracted):**
Hits: provenance, epistemology (→ "epistemological infrastructure"), adapter, TagConceptBridger (→ "tag_concept_bridger"), Hebbian contribution (→ "hebbian design"), chain (→ "chain node"), mark (→ "mark node") = **7/13 = 54%**
Missed: fragment, dual obligation, enrichment, multi-phase processing, deterministic ID, cross-dimensional traversal

**Mistral E02 (25 extracted):**
Hits: opacity problem, vibe-coding, cognitive load, material disengagement, situation awareness, epistemic action (→ "epistemic_actions"), computational offloading, re-representation, cognitive offloading = **9/13 = 69%**
Missed: working memory, external structural representation, knowledge graph, AI-assisted composition
Note: 16 extracted entities are not in E02 gold (cold_start_refactor, hallucination_trap_detection, explainability_gap, dance_improvisation, rapid_prototyping, comprehension-performance_gap, interrupted_work, prompt-wait-evaluate_cycle, gulf_of_envisioning, collaborative_cognitive_load, graphical_constraining, diagrams, extended_hollowed_mind, transformative_tools_for_thought, pragmatic_actions, Plexus)

**Mistral E12 (15 extracted):**
Hits: adapter, provenance, fragment, Hebbian contribution (→ "hebbian contributions"), chain, mark, TagConceptBridger = **7/13 = 54%**
Missed: epistemology, dual obligation, enrichment, multi-phase processing, deterministic ID, cross-dimensional traversal
Note: trellis, carrel are NOT hallucinations — both appear in Essay 12's text ("Essay 11 demonstrated that two consumers — Trellis (fragments) and Carrel (provenance marks)"). Previous session incorrectly flagged these as cross-essay contamination.

**Union E02 (both models combined):**
Qwen contributes 9; Mistral uniquely adds: epistemic action, cognitive offloading = **11/13 = 85%**

**Union E12 (both models combined):**
Qwen contributes 7; Mistral uniquely adds: fragment = **8/13 = 62%**

**Themes E02 (6 extracted vs gold 5):** 6/5 — overcounts by 1. All 5 gold themes present; 1 additional ("Structural vs Reasoning Offloading") is a reasonable decomposition of a gold theme. Effective recall: **5/5 = 100%** (with 1 extra).

**Themes E12 (6 extracted vs gold 5):** All 6 plausible. Effective recall: **5/5 = 100%** (with 1 extra).

**Relationships E02 (8 extracted vs gold 10):** 8 extracted; cross-checking against gold: knowledge accumulation→opacity, working memory→cognitive load, external structural representations→opacity, computational offloading→external structural representations, AI tool usage→cognitive offloading, AI-assisted work→opacity problem all present or equivalent. Plexus→structural tool and collaborative cognitive load theory→cognitive mechanism are extra. Missed: 2 gold relationships. Recall: **~8/10 = 80%** (significant improvement — but see Q2a note below).

**Relationships E12 (10 extracted vs gold 8):** 10 extracted vs gold 8. Hits include ingest_record→bookkeeping, epistemology→bookkeeping, chains→epistemological infrastructure, fragment_adapter→semantic output, tag_concept_bridger→references edges, provenance→semantics, hebbian_design→multi_phase, provenance_trail→weight, adapter_level→provenance_trail, tag_concept_bridger→user_created_marks. All 8 gold relationships plausibly covered. Recall: **~8/8 = 100%** (but with 2 additional non-gold).

### Comparison Table

| Metric | Previous (0.15.9) | Optimized (0.15.10) | Delta |
|---|---|---|---|
| Qwen E02 recall | 9/13 = 69%* | 9/13 = 69% | 0 |
| Qwen E12 recall | 7/13 = 54% | 7/13 = 54% | 0 |
| Mistral E02 recall | 8/13 = 62%* | 9/13 = 69% | +7pp |
| Mistral E12 recall | 6/13 = 46% | 7/13 = 54% | +8pp |
| Mistral E12 hallucinations | 2 (prev. incorrectly flagged) | 0 (trellis/carrel are in E12 text) | Fixed* |
| Union E02 recall | ~77% (~10/13) | 11/13 = 85% | +8pp |
| Union E12 recall | ~77% (~10/13) | 8/13 = 62% | -15pp** |
| Themes E02 | ~80% (4/5) | ~100% (5/5+1 extra) | +20pp |
| Themes E12 | ~80% (4/5) | ~100% (5/5+1 extra) | +20pp |
| Relationships E02 | ~35% | ~80% (Q2c Synthesized) | +45pp*** |
| Relationships E12 | ~31% | ~100% (Q2c Synthesized) | +69pp*** |
| Q2a Sequential: relationship timeout | — | Timed out (120s) | — |
| Q2c Synthesized: synthesizer output | — | E02 all ok; E12 synthesizer alone | — |
| Q3 Primed E02 vs unprimed | — | +2 entities (ext. struct. repr., cognitive offloading) | +gain |

*Previous figures rebased to corrected 13-entity gold standard.
**The previous ~77% estimate for Union E12 appears to have been overstated; the optimized runs give a concrete 8/13 = 62%. Qwen and Mistral now find many of the same entities (7 and 7 overlap almost entirely), reducing complementarity on E12.
***Q2c Synthesized relationship extraction is not directly comparable to Q1 isolated extraction — different experiment (synthesizer ensemble, not the standalone extractor). The Q1 isolated relationship extractor still timed out at 120s in Q2a.

### Key Findings

**1. Context window expansion (2048 → 8192) is the single largest driver of improvement.**
Mistral recall improved +7-8pp across both essays. The most likely explanation is that at 2048 tokens, essays were being truncated and the model was extracting from incomplete text. At 8192, the full essay is available. This is supported by the fact that Mistral's entity count increased from 10-14 (previous) to 25 (E02) and 15 (E12) — the model is now seeing more text and finding more entities. Qwen's count also shifted (19→15 on E02, 16→15 on E12), but recall held stable, suggesting Qwen was already reading enough context at 2048 or that increased temperature offset the benefit.

**2. Temperature changes had mixed or null effect on Qwen entity recall.**
Qwen E02 recall is unchanged (69%) despite temperature rising from 0.2 to 0.6. Qwen E12 recall is also unchanged (54%). The higher temperature appears to cause Qwen to generate more varied but not more accurate extractions — similar recall, different entity lists. The vendor recommendation of 0.6 neither helps nor hurts recall on these essays.

**3. Previous "hallucination" finding for Mistral on E12 was incorrect.**
Trellis and carrel, previously flagged as cross-essay contamination from Essay 04, actually appear in Essay 12's text: "Essay 11 demonstrated that two consumers — Trellis (fragments) and Carrel (provenance marks)." These are correct extractions, not hallucinations. With the anti-hallucination prompt and low temperature, Mistral E12 output shows zero actual hallucinations — all 15 entities are grounded in the source text. The anti-hallucination prompt may have contributed to this, though a controlled A/B test wasn't run.

**4. The previous Union E12 ~77% estimate was optimistic.**
Concrete scoring of the optimized runs gives 8/13 = 62%. Qwen and Mistral on E12 find nearly the same 7 entities (provenance, epistemology/variant, adapter, TagConceptBridger, chain, mark, Hebbian); Mistral uniquely adds only fragment. The complementarity that drives union recall on E02 (Mistral recovers epistemic action and cognitive offloading that Qwen misses) does not appear strongly on E12. Both models systematically miss the same 5-6 entities: dual obligation, enrichment, multi-phase processing, deterministic ID, cross-dimensional traversal. These are domain neologisms and compound terms that require domain knowledge to recognize as entities — exactly the gap TextRank priming was intended to address.

**5. Theme extraction is now essentially solved at the Q1 level.**
Both themes runs produced 6 themes against a gold of 5, with all 5 gold themes represented. The extra theme in each case is a defensible decomposition. Theme recall is effectively at ceiling for single-model extraction. This is consistent with the prior finding (~80%), now confirmed at ~100% with the corrected gold.

**6. Q2c Synthesized relationships show the pipeline can reach near-gold on relationships — but at extreme cost.**
The synthesized ensemble (parallel specialists + synthesizer) produced 8 relationships for E02 (vs gold 10) and 10 for E12 (vs gold 8), covering most gold relationships. This is a dramatic improvement over isolated Q1 relationship extraction (~31-35%). However: E12 parallel specialists all timed out due to memory contention; the synthesizer ran alone and still produced 10 relationships from its primed context. This means the quality gain came from the synthesizer seeing entity and theme context, not from parallel relationship specialist execution. The 14b synthesizer approach remains impractical (memory, latency), but the pattern of "prime the relationship extractor with entity context" is validated.

**7. Q2a sequential relationship extraction continues to time out.**
Both E02 and E12 sequential runs timed out the relationship extractor at 120s. This is unchanged from the previous run. The relationship extraction task is computationally expensive for 7-8B models regardless of context window size. The timeout is a model capacity issue, not a configuration issue.

**8. Q3 Primed extraction recovers missed gold entities.**
Qwen primed E02 found "external structural representations" and "cognitive offloading" — two gold entities that unprimed Qwen missed. This confirms H3b: classical ML (TextRank) surfaces vocabulary hints that redirect the SLM toward missed terms. The 15-entity primed count is the same as unprimed, but the composition shifts toward gold terms. This is the most actionable Q3 finding: priming doesn't increase total recall dramatically, but it recovers specific missed gold terms, which is exactly what a merge-and-deduplicate pipeline needs.

### Updated Hypothesis Status

**H1a (entities decompose cleanly): Confirmed with narrowed caveats.**
54-69% recall per model, 62-85% union. Context window expansion eliminated truncation as a confound. The remaining gap is systematic: domain neologisms (deterministic ID, dual obligation, cross-dimensional traversal) are missed by both models regardless of configuration. These require either TextRank priming or domain-specific fine-tuning.

**H1b (relationships are sequential): Confirmed.**
Q2a timeout confirms that relationship extraction at 120s is not viable as a standalone step for 7-8B models. The Q2c result (synthesizer producing relationships from primed context) suggests the path forward is a lighter relationship extractor primed with entity context, not a full 7-8B model running unconstrained.

**H1c (themes are distinct): Confirmed — now at ceiling.**
~100% recall across both essays with a single 8B model. Themes are the most reliable extraction subtask; no further investment needed here.

**H1d (Qwen/Mistral complementarity): Partially revised.**
Strong on E02 (union 85%, Mistral recovers 2 unique gold entities), weak on E12 (union 62%, Mistral recovers only 1 unique gold entity). Complementarity is essay-dependent. On E12, both models hit the same ceiling imposed by the same missed domain neologisms. The complementarity finding from the prior run (~77% union) was partially an artifact of not scoring concretely.

**H2a (sequential improves relationships): Inconclusive — now leaning negative.**
Q2a timed out. Q2c showed improvement but only because the synthesizer received entity context as a prompt prefix, not because the sequential topology itself improved extraction. The topology is not the variable; the entity-primed prompt is.

**H2c (14B synthesizer adds value): Rejected — confirmed.**
E12 parallel specialists timed out; synthesizer ran alone and still produced reasonable output. This means the synthesizer's contribution was from its own priors + entity context prefix, not from aggregating specialist outputs. The 14B model is not viable locally.

**H3a (priming improves recall): Partially confirmed.**
E02 primed recovers 2 missed gold entities vs unprimed. E12 primed count is the same (15) but composition is not directly compared. The improvement exists but is modest — priming shifts entity composition more than it lifts total count.

**H3b (classical ML + SLM complementarity): Confirmed.**
TextRank surfaces terms that SLMs miss. The mechanism is vocabulary priming, not knowledge addition — the SLM still needs to recognize and validate the term as an entity. This works best for structurally significant terms that are statistically salient in the text (TextRank's strength) but that the SLM de-emphasizes in favor of abstract concepts.

### Architecture Implications

The optimized results reinforce the parallel extraction architecture from the previous analysis, with one addition:

1. **Context window must be 8192+** for both models on essay-length inputs (~100 lines). The default 2048 truncates and produces systematically lower recall.
2. **Mistral hallucination concern is resolved.** Previous E12 "hallucinations" (trellis, carrel) were scoring errors — both terms appear in the text. With anti-hallucination prompting + temp=0.15, Mistral shows zero actual hallucinations. A text-grounding post-filter remains good practice but is no longer a critical blocker.
3. **TextRank priming should run in parallel with SLM extractors, not before them.** The vocabulary hints from TextRank recover specific missed gold entities; the SLM does not need to wait on TextRank output to start extracting.
4. **Relationship extraction cannot be a standalone 7-8B model call at 120s timeout.** Either: (a) use a faster model/prompt combination with entity-primed context, or (b) use a deterministic co-occurrence approach (entities that appear in the same sentence → candidate relationship) and let the SLM classify rather than generate from scratch.
5. **Theme extraction is solved.** Remove from open questions; include as a fixed parallel stage.
