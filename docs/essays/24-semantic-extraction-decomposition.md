# Essay 24: Semantic Extraction Decomposition

*2026-02-25*

How small language models extract knowledge from text when each model handles exactly one concern, and what happens when you compose them.

---

## The question

Essay 23 established universal ingest and a single general-purpose llm-orc ensemble as Plexus's semantic extraction path. The ensemble works: a generic prompt pulls valid concepts from Shakespeare and Rust alike. But "works" at what quality? Essay 23 tested with llama3 8B and mistral 7B on two texts and observed valid-looking output. It did not measure recall against a reference, test whether decomposing extraction into subtasks improves quality, or ask whether multiple models finding different things could be composed into something better than either alone.

This essay asks three questions about the Phase 4 semantic extraction layer:

1. What extraction subtasks produce clean, focused output when each 7-8B model handles exactly one concern?
2. What DAG topology composes those subtasks into the best overall extraction?
3. Does classical ML pre-extraction (TextRank, TF-IDF) improve what language models find?

## Method

Two essays from the Plexus corpus served as test material: Essay 02 (*The Opacity Problem*, conceptual/philosophical, 117 lines) and Essay 12 (*Provenance as Epistemological Infrastructure*, domain theory, 104 lines). Claude-produced gold-standard extractions established the reference: 13 entities, 10 relationships, and 5 themes for Essay 02; 13 entities, 8 relationships, and 5 themes for Essay 12.

Three extraction subtasks were isolated into single-agent llm-orc ensembles: entity extraction, relationship extraction, and theme extraction. Two models were tested independently: Qwen3:8b and Mistral:7b, both running locally on Ollama. Each ensemble had one agent, one concern, one prompt, one model. The prompts specified JSON output schemas and instructed the model to extract only its assigned dimension.

An initial run on llm-orc 0.15.8 produced results that appeared reasonable but were invalidated when a profile resolution bug was discovered: all ensembles had silently fallen back to llama3 regardless of the profile specified. The `_load_model_with_fallback()` function caught the profile resolution exception and substituted the default model with no warning. Every experiment was rerun after the fix.

A second optimization pass applied evidence-based parameter tuning: Qwen3's vendor-recommended temperature of 0.6 (we had used 0.2), Mistral's vendor-recommended 0.15, and critically, expanding Ollama's context window from the default 2048 tokens to 8192. The essays are 2500-3800 tokens. At 2048, the models were reading truncated text.

## Results

### Isolated subtasks (Q1)

Each subtask ran as a single-agent ensemble with one model and one concern. Entity extraction was tested with both models independently.

**Entity recall against gold standard:**

| Model | Essay 02 (13 gold) | Essay 12 (13 gold) |
|---|---|---|
| Qwen3:8b | 9/13 (69%) | 7/13 (54%) |
| Mistral:7b | 9/13 (69%) | 7/13 (54%) |
| Union (both models) | 11/13 (85%) | 8/13 (62%) |

**Theme recall:** Qwen3:8b captured all 5 gold themes on both essays (5/5, 100%), producing 6 themes each time — the extra always a defensible decomposition of a gold theme.

**Relationship recall (isolated):** Qwen3:8b achieved approximately 35% on Essay 02 (3-4 of 10 gold) and 31% on Essay 12 (2-3 of 8 gold).

### Composition topologies (Q2)

| Topology | Essay 02 | Essay 12 |
|---|---|---|
| Parallel (independent merge) | All agents completed; merge is deterministic | All agents completed |
| Sequential (entities → relationships) | Relationship extractor timed out (120s) | Relationship extractor timed out (120s) |
| Synthesized (specialists + 14B) | All agents completed (336s); relationships ~80% recall | Specialists timed out; synthesizer alone produced ~100% relationship recall |

The synthesized topology's relationship quality came from the 14B model seeing entity context, not from composing specialist outputs. On Essay 12, all three 8B specialists timed out due to GPU memory contention; the synthesizer ran alone against the raw text.

### Classical ML priming (Q3)

| Configuration | Essay 02 entities | Essay 12 entities |
|---|---|---|
| Qwen3:8b unprimed | 9/13 gold (69%) | 7/13 gold (54%) |
| Qwen3:8b with TextRank priming | +2 gold entities recovered, −2 others lost | Similar total, different composition |
| Mistral:7b unprimed | 9/13 gold (69%) | 7/13 gold (54%) |
| Three-way union (TextRank + Qwen + Mistral) | Up to 85% | Up to 62% |

Priming redirected entity extraction rather than expanding it. The strongest complementarity came from multi-model union, not from TextRank vocabulary hints.

## What decomposes cleanly

**Entities** decompose. Both models produce valid JSON entity lists when asked for nothing else. Qwen3:8b achieves 54-69% recall against the gold standard depending on the essay; Mistral:7b achieves 54-69% after parameter optimization. Neither model alone reaches 70%. But they miss different things.

Qwen favors abstract and compound terms: *collaborative cognitive load theory*, *epistemological infrastructure*, *extended hollowed mind*. Mistral favors short structural terms: *fragment*, *chain*, *mark*, *vibe-coding*. On Essay 02, the union of both models' entity lists reaches 85% recall (11 of 13 gold entities). Qwen finds 9; Mistral adds *epistemic action* and *cognitive offloading* that Qwen missed. The complementarity is genuine — these are different extraction biases, not random variation.

On Essay 12, complementarity is weaker. Both models find largely the same 7 entities and miss the same 6: *dual obligation*, *enrichment*, *multi-phase processing*, *deterministic ID*, *cross-dimensional traversal*. These are domain neologisms — compound terms coined in the Plexus essays that have no training-data presence. Neither model recognizes them as entities regardless of temperature, context window, or prompting. The union reaches only 62%. Complementarity is strong when the missed entities are general concepts one model deprioritizes; it breaks down when both models hit the same ceiling imposed by domain vocabulary.

**Themes** decompose trivially. A single Qwen3:8b agent captures all 5 gold themes on both essays, producing 6 themes each time (the extra is always a defensible decomposition of a gold theme). Theme extraction is the most reliable subtask at every configuration tested. This makes sense: themes are the kind of abstract pattern recognition that language models do well — paraphrase the argument, identify the tensions, name the principles. No domain vocabulary required.

**Relationships** do not decompose cleanly in isolation. An isolated relationship extractor achieves only 31-35% recall: it captures argumentative flow (*A causes B*, *X accelerates Y*) but misses explicit structural relationships (*chain contains mark*, *adapter produces provenance*). The model generates plausible inferences rather than extracting stated connections. When given entity context from an upstream extraction — the entity list as a vocabulary constraint — relationship quality improves dramatically (80-100% recall in the synthesized pipeline). The variable is not topology; it is whether the relationship extractor has a vocabulary to work with.

## What composition reveals

Three DAG topologies were tested:

**Parallel independent agents.** Entity extractor, relationship extractor, and theme extractor run simultaneously with no dependencies. Each sees only the source text. This is the Q1 baseline — the composition is mechanical merge after independent extraction.

**Sequential: entities then relationships.** The entity extractor runs first; its output is prepended to the relationship extractor's input as vocabulary context. The relationship extractor is constrained to use only concepts from the upstream entity list. Theme extraction runs in parallel with both.

**Synthesized: specialists plus a 14B synthesizer.** All three specialists run (entities and themes in parallel, relationships after entities), then a Qwen3:14b model receives all outputs and produces a deduplicated, reconciled extraction.

The sequential topology's relationship extractor consistently timed out at 120 seconds. The combined input — source text plus entity list plus extraction instructions — exceeds what an 8B model can process within the time budget on consumer hardware. When it did complete (in earlier runs before optimization), it produced better JSON than the isolated extractor but inconsistently — sometimes prose instead of structured output.

The synthesized topology's three specialist agents competed for GPU memory when running concurrently. On Essay 12, all three timed out. The 14B synthesizer then ran alone — receiving empty specialist outputs but retaining the original text in its context — and produced 10 concepts, 6 relationships, and 3 themes. Reasonable output from a model that was supposed to be aggregating, not extracting. This is the wrong lesson: the synthesizer's quality came from its own capability on the raw text, not from composing specialist outputs. The 14B model requires 87% of system memory and takes 3+ minutes per run. It is not a viable composition strategy for local-first extraction.

The parallel topology is the only one that reliably completes on consumer hardware. Concurrent agents on a single GPU cause memory contention and timeouts. The practical architecture sequences agents through the GPU: TextRank (CPU, <1 second), then Qwen entity extraction (~60 seconds), then Mistral entity extraction (~25 seconds), then theme extraction (~50 seconds). Total wall time under 3 minutes for a full extraction. The merge step is deterministic and instant.

## What classical ML adds

TextRank is a graph-based ranking algorithm that scores terms by their co-occurrence relationships in the text. It knows nothing about semantics — it identifies statistically significant phrases. TF-IDF weights terms by their distinctiveness. Together they surface vocabulary that is structurally prominent in the text but that language models might deprioritize in favor of more abstract concepts.

A TextRank/TF-IDF script agent runs as the first pipeline stage. Its output — a ranked list of candidate terms — is passed to the entity extractor as "vocabulary hints." The prompt tells the model to use the candidates as a starting point but add abstract concepts the statistical analysis missed.

The effect is compositional, not additive. On Essay 02, primed extraction found *external structural representations* and *cognitive offloading* — two gold entities that unprimed Qwen missed — while dropping two entities the unprimed version had found. Total count stayed at 15. Priming redirects the model's attention toward statistically prominent terms, recovering specific vocabulary at the cost of other discoveries.

TextRank's candidate list also contains artifacts. The llm-orc script agent's JSON serialization corrupts newlines in the input text, producing ghost tokens like "nthe" that rank highly (score 0.30-0.60) and contaminate compound terms: "nthe provenance", "source nthe". The statistical extraction works correctly when run directly; the corruption is in the orchestration layer's serialization.

The strongest complementarity signal comes not from TextRank priming but from multi-model extraction. Qwen and Mistral, each extracting independently, produce a union that exceeds either alone by 15-30 percentage points on Essay 02. TextRank priming adds 2 specific entities. The right architecture runs all three — TextRank, Qwen, Mistral — independently and merges, rather than chaining TextRank as a prerequisite.

## What configuration teaches

The single largest improvement came from expanding Ollama's context window from 2048 to 8192 tokens. Mistral's recall improved 7-8 percentage points across both essays after this change. The essays are 2500-3800 tokens; at the default 2048, the model was extracting from truncated text. This was invisible — Ollama silently truncates input without warning, and the model still produces plausible-looking output from whatever it sees.

Temperature adjustments followed vendor recommendations: Qwen3 at 0.6 (their default, packaged in the Modelfile), Mistral at 0.15 (from Mistral's instruct configuration). Qwen's recall was unchanged at 0.6 versus 0.2 — the higher temperature produces different entity lists but the same hit rate. Mistral's improvement is confounded with the context window change; the two were applied simultaneously.

Qwen3's `/no_think` flag is essential for extraction tasks. Without it, the model generates internal chain-of-thought reasoning that consumes 80% of the time budget, frequently causing timeouts. With it, inference is approximately 30% faster. The vendor recommends placing `/no_think` in the user message for non-thinking mode; placing it in the system prompt also works for extraction.

SLM-generated confidence scores are not calibrated probabilities. Every model produces monotonically decreasing sequences (0.95, 0.92, 0.91, 0.88...) that are rank orderings, not confidence measures. This validates the domain model's design: graph weights come from structural signals — per-adapter contribution tracking, scale normalization, evidence diversity across adapters — not from model self-assessment (Invariant 8).

## The architecture that emerges

```
                    ┌─── textrank-extract (script, CPU) ──┐
input ──► chunker ──┤                                     ├──► merge + deduplicate ──► emit
                    ├─── entity-extractor (qwen3:8b) ─────┤
                    └─── entity-extractor (mistral:7b) ────┘
                    ┌─── theme-extractor (qwen3:8b) ───────────────────────────────┘
```

The architecture is parallel extraction with deterministic merge. No sequential dependencies, no synthesis model, no 14B aggregator. Each extractor runs independently; the merge step unions and deduplicates entity lists by normalized label. Themes run independently. All outputs emit through the adapter pipeline as separate contributions with distinct adapter IDs — exactly what the contribution tracking system was designed for (Invariant 45).

This maps directly to Plexus's existing infrastructure. Each extractor is a Phase 3 adapter with its own adapter ID: `extract-semantic:qwen3`, `extract-semantic:mistral`, `extract-keywords:textrank`. When two extractors discover the same concept — say, both find *provenance* — the deterministic concept ID (`concept:provenance`, Invariant 19) ensures they converge on the same node. Each adapter's contribution accumulates in its own slot. Scale normalization brings the contributions to comparable range. The raw weight reflects independent confirmation from multiple extraction methods.

This is the Hebbian pattern from Essay 06, applied to extraction: independent evidence sources strengthening the same connections. A concept found by TextRank (statistical salience), Qwen (abstract reasoning), and Mistral (structural parsing) has three contribution slots — evidence diversity across extraction methods, visible in the graph and traceable through provenance.

The merge step does not require a language model. It is string normalization: lowercase, deduplicate by edit distance, resolve near-synonyms ("cognitive load" and "cognitive load theory" → keep the more specific form). A deterministic algorithm, not a model call. This is critical for local-first operation: the expensive part (LLM extraction) runs once per model; the composition is free.

## What remains open

**Relationship extraction needs a different approach.** Neither isolated nor entity-primed extraction works reliably at 7-8B scale within practical time budgets. The synthesized pipeline showed that when a model has entity context, relationship quality improves dramatically — but the 14B model that demonstrated this is too expensive locally. Two paths forward: a lightweight relationship classifier that takes entity pairs and classifies their relationship type from a fixed vocabulary (faster than generating relationships from scratch), or a deterministic co-occurrence approach where entities appearing in the same sentence become candidate relationships that the model validates rather than discovers.

**Domain neologisms are systematically missed.** Both models fail on compound terms coined in the corpus: *dual obligation*, *cross-dimensional traversal*, *deterministic ID*. These have no training-data presence. TextRank priming helps for statistically prominent terms but not for terms that appear only once or twice. Fine-tuning on domain vocabulary or few-shot examples in the extraction prompt are the likely remedies.

**Concurrent GPU scheduling is an operational constraint.** Two 8B models cannot run simultaneously on consumer hardware (16-32GB unified memory) without memory contention causing timeouts. The practical architecture sequences models through the GPU. This means total extraction time scales linearly with model count, not constant. For a three-extractor pipeline, wall time is approximately 2-3 minutes per essay. Acceptable for background Phase 3 extraction; too slow for interactive use.

## What this means for Plexus

The extraction layer is not one model doing everything. It is multiple weak extractors — each finding part of the picture — composed through the graph's existing convergence mechanisms. The same deterministic concept IDs, contribution tracking, and scale normalization that handle multi-adapter evidence from Trellis and Carrel also handle multi-model evidence from Qwen and Mistral. No new engine machinery is needed.

The parallel extraction architecture is the local-first answer to the quality gap between small and large language models. A single 8B model achieves 54-69% entity recall. Two 8B models composed through deterministic merge achieve up to 85%. The composition is free — it uses infrastructure that already exists for a different purpose. The investment is in extraction prompts and model configuration, not engine changes.

Theme extraction is solved. Entity extraction is workable and improvable. Relationship extraction needs a different approach. The next spike should test lightweight relationship classification: given a pair of entities and the sentence containing both, classify the relationship type from the domain model's vocabulary. This moves from generation (expensive, unreliable) to classification (fast, constrained) — the same shift that made entity extraction work when vocabulary priming redirected the model from generating to recognizing.
