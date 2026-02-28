# Essay 25: Closing the Extraction Gap

*2026-02-27*

Essay 24 established parallel extraction — multiple 7-8B models extracting independently, composed through deterministic merge. Entity extraction worked (85% union recall on general text). Theme extraction was solved (100%). But three gaps remained: relationship extraction achieved only 31-35% recall, domain-specific entity recall dropped from 85% to 62%, and the only model that demonstrated strong relationship recall (Qwen3:14b) required 87% of system memory and 3+ minutes per run.

This essay reports a research cycle that closed all three gaps without new engine machinery, larger models, or changes to the extraction architecture's fundamental design. The interventions are: structured output (JSON schema constraint), glossary priming with multi-run union, and deterministic Phase 2 extraction operating independently alongside Phase 3 LLM extraction. Together they produce an extraction pipeline achieving ~85% entity recall and ~94% relationship recall on domain-specific text, running entirely on a 7B model.

---

## The wrong first question

The research started with what seemed obvious: if the 14B model produces 80-100% relationship recall but is too expensive, make it cheaper through chunking. Split the essay into sections, run the 14B model on each section independently, merge the results.

The spike tested Qwen3:14b on Essay 02, comparing full-text extraction (5,515 characters) against six section-level chunks. Chunking was 9.7x slower: 1,216 seconds versus 125 seconds for the baseline. The cause was hidden thinking tokens. Qwen3 generates 2-3x more internal reasoning tokens per chunk than for the full text, despite the `/no_think` flag. On shorter inputs, the model compensates for less context by speculating more. The chars/token ratio — a measure of how much visible output each generated token produces — dropped from 1.72 (full text) to 0.25-0.46 (chunks). Most generated tokens were invisible reasoning, not JSON output.

But the spike also revealed that Essay 24's "3+ minutes" timing was wrong. That measurement included three specialist agents competing for GPU memory alongside the 14B model. When the 14B model runs alone, it processes the full essay in 125 seconds — well within practical bounds. The chunking hypothesis was wrong. The timing premise that motivated it was also wrong.

More importantly, the 14B model's relationship recall was lower than Essay 24 claimed. Against the gold standard, the baseline achieved approximately 30% with 2 partial matches — better than isolated 8B extraction but well below "80-100%." The difference: Essay 24's synthesizer had received specialist outputs as additional context. The entity *list* alone is not enough; the specialist outputs' evidence and framing contributed to the higher recall. This meant the research question shifted from "make the 14B model cheaper" to "what actually makes relationship extraction work."

## Structured output is the intervention

Ollama's `format` parameter accepts a JSON schema and constrains the model to produce only tokens that conform to it. This is not prompt engineering — it is grammar-level constraint during inference. The model physically cannot generate tokens that violate the schema. No thinking tokens, no prose preamble, no markdown formatting. Every generated token is productive JSON.

The effect on relationship extraction was dramatic:

| Model | Mode | Total time | Relationships | Gold recall |
|-------|------|-----------|---------------|-------------|
| Qwen3:14b | unconstrained | 125.4s | 10 | 30% |
| Qwen3:14b | structured | 149.1s | 14 | 40-50% |
| Qwen3:8b | structured | 147.0s | 12 | 30-38% |
| Nemo:12b | structured | 30.0s | 7 | 25-31% |
| Mistral:7b | structured | 50.0s | 15 | 40-44% |

Mistral:7b with structured output matches or exceeds Qwen3:14b's recall at one-third the time and one-third the model size. It finds relationships the larger models miss — *vibe-coding exemplifies opacity*, *epistemic distinct_from pragmatic*. The JSON schema does more for extraction quality than doubling the model's parameters. This invalidates Essay 24's conclusion that "7-8B models may not be strong enough for abstract relationship identification." They are strong enough — they were just wasting most of their inference budget on invisible tokens.

Qwen3 models have approximately 100 seconds of unexplained overhead when using structured output — likely grammar compilation interacting with the thinking architecture. Mistral and Nemo have 3-5 seconds of overhead. This makes Mistral:7b the clear choice for schema-constrained extraction.

### Run-to-run variation as a resource

A single Mistral:7b structured extraction finds 14-16 relationships per run. Three runs at temperature 0.3 produce 32 unique relationship pairs in union — but only 4 appear in all three runs. Stability is 12%. Each run explores a different slice of the relationship space: Run 1 finds gold relationship #2 that Runs 2 and 3 miss. Run 3 finds gold #3 that Runs 1 and 2 miss.

This instability is a resource, not a problem. Three sequential runs at 50 seconds each produce more coverage than a single 14B model run at 149 seconds. The 32-pair union covers more gold relationships than any single run at any model size. The noise — wrong entity pairs, hallucinated relationships, entities not in the provided list — is handled by Plexus's existing contribution tracking (Invariant 45). Each run gets a distinct adapter ID. Relationships found by multiple runs accumulate contributions across slots. Single-run relationships stay at minimum weight. The stable core self-identifies through convergence, using the same Hebbian pattern from Essay 06.

## Domain vocabulary is a strategy problem

On Essay 02 (general concepts), multi-model entity extraction reaches 85% recall. On Essay 12 (domain-specific), it drops to 62%. The gap is entirely explained by domain neologisms — compound terms coined in the Plexus corpus that no language model has seen in training: *dual obligation*, *cross-dimensional traversal*, *deterministic ID*, *multi-phase processing*, *enrichment* in its Plexus-specific sense.

The hypothesis was that the domain model (`docs/domain-model.md`) already contains 30+ defined concepts. If the extraction prompt included this vocabulary, the model might recognize terms it would otherwise miss.

A 2x2 experiment tested two independent variables — glossary priming (domain concept names in the prompt) and structured output (JSON schema constraint) — against Essay 12's 13 gold-standard entities:

| Condition | Time | Gold recall | Character |
|-----------|------|-------------|-----------|
| A: Unprimed + unconstrained | 36.9s | ~54% | Noisy, 38 entities, 2.9x overgeneration |
| B: Unprimed + structured | 12.7s | ~31% | Fast but too abstract |
| C: Primed + unconstrained | 20.6s | ~58% | Clean normalized terms |
| D: Primed + structured | 15.6s | ~54% | Clean terms, unique finds |
| E: Fewshot + primed + structured | 36.1s | ~38% | Shifted focus, lost coverage |

Three findings emerged. First, glossary priming normalizes vocabulary: primed conditions extract clean terms like *provenance*, *adapter*, *fragment* instead of noisy paraphrases like "provenance marks", "fragment node", "adapter-shaped things." Priming shifts the model from describing to naming.

Second, structured output hurts entity extraction. Condition B (unprimed + structured) is the worst at 31%. This is the opposite of relationship extraction, where structured output is the key intervention. For entities, vocabulary context matters more than output format. For relationships, output format matters more than vocabulary context.

Third, few-shot examples shift focus rather than broadening it. Condition E recovered *TagConceptBridger* and *Hebbian contribution* but lost *provenance*, *epistemology*, *fragment*, and *multi-phase processing*. Net effect: worse recall than simple glossary priming.

### Closing the gap with multi-run union

The same multi-run strategy that works for relationships works for entities. Three glossary-primed + structured runs at temperature 0.3 union to 69% recall (up from 54% single-run). But the critical move is cross-condition union: adding one unprimed + unconstrained run pushes to approximately 85%.

| Strategy | Recall | Unique entities | Time |
|----------|--------|-----------------|------|
| Single D run | 54% | 15 | ~16s |
| D union (3 runs) | 69% | 21 | ~51s |
| Full union (3xD + 1xA) | ~85% | 44 | ~78s |

The unprimed run contributes terms that no primed run finds: *multi-phase processing*, *sink*, and descriptive terms from the essay's natural vocabulary. The primed runs contribute terms the unprimed run misses: *enrichment*, *fragment*, *concept* — normalized domain terms. Neither approach alone reaches 85%; together they do.

The 44-entity full union includes 3.4x overgeneration versus gold. But run stability within the primed condition is 24% — of 21 unique entities across 3 runs, only 5 appear in all three (adapter, epistemology, graph, plexus, provenance). Per-adapter contribution tracking handles this: entities confirmed by 3+ runs accumulate high weight; single-run entities stay at minimum weight. The graph's existing mechanics turn noisy extraction into calibrated confidence.

Three categories of missed entities require different interventions:

| Category | Examples | Solution |
|----------|----------|----------|
| Normalized domain terms | provenance, adapter, fragment | Glossary priming (solved) |
| Component/class names | TagConceptBridger, EngineSink | Unprimed extraction or CamelCase regex |
| Compound neologisms | dual obligation, cross-dimensional traversal | NLP phrase extraction or acceptance |

The first two categories are addressed by the multi-condition union: primed runs find normalized terms, unprimed runs find component names. The third category — compound neologisms that look like ordinary English — is a genuine ceiling for LLM extraction. These terms require either Phase 2 heuristic detection (NLP noun phrase extraction) or acceptance that some entities are concept *recognition* rather than extraction. "Dual obligation" is an idea named in the domain model but never used as a phrase in Essay 12. An extraction model can only extract what the text says.

## What the syntax parser sees

Essay 18 defined three extraction phases: Phase 1 (registration, instant), Phase 2 (heuristic analysis, Rust/Python, fast), Phase 3 (semantic extraction, LLM, slow). The Q2 findings established Phase 3's entity extraction architecture. But Phase 2 had not been tested. A spike ran three deterministic approaches on Essay 12 using spaCy's dependency parser and the domain model's entity vocabulary (37 terms):

| Approach | Output | Relationship gold recall |
|----------|--------|--------------------------|
| Sentence co-occurrence | 209 `may_be_related` pairs | 6/8 (75%) |
| Dependency parsing (SVO) | 28 typed relationships | 4/8 (50%) |
| Verb pattern matching | 3 typed relationships | 2/8 (25%) |

Sentence co-occurrence — entities co-occurring within the same sentence become `may_be_related` edges — captures 6 of 8 gold relationships at zero model cost in under one second. The two misses are abstract: *provenance provides epistemological foundation* (the key argument of the essay, stated across paragraphs rather than within a sentence) and *multi-phase processing uses separate chains per phase* (a structural relationship spanning multiple paragraphs). Co-occurrence reliably finds relationships where both entities appear together in the text. It cannot find relationships that are argued rather than stated.

Dependency parsing adds relationship types — `adapter produces provenance`, `fragment produces provenance mark`, `enrichment creates references edges` — but rarely finds entity pairs that co-occurrence missed. It names the verb connecting two known entities in a sentence, giving typed relationships instead of untyped `may_be_related` edges. Verb pattern matching is too fragile for complex sentences; the essay's argumentative prose breaks simple SVO extraction.

### The interaction between phases

The obvious architecture is sequential: Phase 2 feeds Phase 3. Phase 2's typed relationships prime the LLM prompt, giving it a starting vocabulary of known connections to validate and extend.

This was tested directly. Condition F ran the LLM with no Phase 2 context. Condition G2 ran the LLM with Phase 2's dependency parsing output included in the system prompt.

| Condition | Relationships | Time | Gold recall |
|-----------|--------------|------|-------------|
| F: LLM only | 37 | 108s | ~81% |
| G2: LLM + Phase 2 context | 16 | 41s | ~38% |

Phase 2 priming makes the LLM worse. G2 produced 16 well-formed relationships, but 14 of 16 are reformulations of the Phase 2 input. The model validated what it was given — `adapter produces provenance` became `FragmentAdapter produces provenance`, `fragment get mark` became `Fragment contains mark` — instead of extending it. The abstract relationships that are the LLM's unique contribution — *provenance provides epistemological foundation*, *TagConceptBridger bridges provenance to semantic dimension* — were not found. Priming made the model lazy.

Condition F, running without Phase 2 context, found `provenance component_of epistemology` and `provenance enables semantics`. These are the argumentative relationships that no syntactic parser can identify — the LLM's natural strength. It also found `TagConceptBridger creates references edges`, `multi-phase processing requires heterogeneous adapters`, and `independent verification creates higher-confidence knowledge`. More noise (37 relationships instead of 16), but also more signal.

An earlier uncapped run (Condition G, no token limit) demonstrated the extreme case: the primed LLM generated 81,920 tokens over 56 minutes, entering a repetition loop. The structured output schema constrains format but not length. Without a generation limit, the validation spiral continued indefinitely. Token limits are mandatory for any primed extraction.

The right interaction between phases is not sequential priming. It is independent accumulation. Phase 2 contributes its co-occurrence pairs and typed relationships through one set of adapter IDs. Phase 3 contributes its generative extraction through another. Both accumulate in the graph per Invariant 45. Relationships found by both phases get higher weight through reinforcement. Neither phase needs to know about the other.

| Phase | Method | Gold recall | Time | Cost |
|-------|--------|-------------|------|------|
| Phase 2 alone | Co-occurrence + dep parsing | ~69% | <1s | CPU only |
| Phase 3 alone | Mistral:7b unprimed | ~81% | 108s | GPU |
| Union (independent) | Both phases, separate adapter IDs | ~94% | ~109s | CPU + GPU |

The union covers nearly all gold relationships. Phase 2 finds the structural ones — *chain contains mark*, *adapter produces provenance*, *fragment has provenance mark* — with zero model cost. Phase 3 finds the abstract ones — *provenance provides epistemological foundation*, *TagConceptBridger bridges provenance to semantic dimension* — that require reasoning about the essay's argument. The only borderline miss is *multi-phase processing uses separate chains per phase*, which Phase 3 captures partially as *multi-phase processing requires distinct input kinds*.

## The architecture

```
Phase 2 (heuristic, <1s, CPU):
  ├── co-occurrence (209 may_be_related pairs)     ──┐
  ├── dependency parsing (28 typed relationships)   ──┤
  └── CamelCase regex (component entity names)      ──┤
                                                      ├── all contribute independently
Phase 3 (semantic, ~3.5 min, GPU):                    │   via Invariant 45
  ├── 3× entity (glossary-primed, structured, 0.3) ──┤
  ├── 1× entity (unprimed, unconstrained, 0.15)    ──┤
  ├── 3× relationship (unprimed, structured, 0.3)  ──┤
  └── 1× theme (structured)                        ──┘
```

Phase 1 runs synchronously at ingest time: file node, MIME type, basic metadata. Phase 2 runs as a background task in under a second: sentence co-occurrence for `may_be_related` edges, dependency parsing for typed relationships, regex patterns for component names. Phase 3 runs as sequential Mistral:7b calls through llm-orc: entity extraction (~78s), relationship extraction (~150s), theme extraction (~50s).

Each extraction run gets a distinct adapter ID — `extract-phase2:cooccurrence`, `extract-phase2:depparse`, `extract-phase3:entity:primed:1`, `extract-phase3:entity:primed:2`, `extract-phase3:entity:unprimed`, and so on. Contributions accumulate in separate slots. Scale normalization brings them to comparable range. Raw weight reflects independent confirmation across extraction methods. The graph's existing machinery — the same deterministic concept IDs, contribution tracking, and enrichment loop that handle multi-adapter evidence from Trellis and Carrel — handles multi-phase, multi-run extraction evidence.

All 8 Phase 3 runs use Mistral:7b. No multi-model GPU contention. No 14B model. Sequential runs on a single model avoid the memory competition that caused timeouts in Essay 24. The llm-orc orchestrator's max concurrency setting (added in 0.15.11) enforces this at the infrastructure level.

### Expected recall

| Subtask | Method | Expected recall |
|---------|--------|-----------------|
| Entities (domain text) | 3× primed + 1× unprimed, multi-run union | ~85% |
| Relationships | Phase 2 co-occurrence + Phase 3 3× union | ~94% |
| Themes | Single structured run | ~100% |

These numbers are from spikes on Essays 02 and 12 — two essays from a 24-essay corpus. The estimates will shift as more essays are tested. But the direction is clear: multi-run union with independent Phase 2 and Phase 3 contributions, denoised through the graph's contribution tracking, achieves recall that no single model run, at any model size, can match.

## What the research taught

Three findings surprised.

**Structured output matters more than model size.** Essay 24 concluded that 7-8B models "may not be strong enough" for relationship extraction. With JSON schema constraint, Mistral:7b at 50 seconds matches Qwen3:14b at 149 seconds. The format constraint eliminates wasted inference (thinking tokens, prose, formatting) and forces every generated token into the extraction schema. Going from no constraint to schema constraint improved relationship recall more than going from 7B to 14B parameters.

**Instability is a resource.** At temperature 0.3, Mistral:7b produces 12-24% stability across runs — each run finds different things. Multi-model union in Essay 24 exploited the fact that Qwen and Mistral have different extraction biases. Multi-run union exploits the fact that the same model has different outputs on repeated runs. The underlying mechanism is the same: independent variation composed through deterministic merge, denoised through contribution tracking. The graph doesn't care whether variation comes from different models or different runs.

**Sequential priming between phases is counterproductive.** The intuition that Phase 2 should feed Phase 3 was wrong. When the LLM receives Phase 2's typed relationships, it validates rather than extends — producing cleaner but less novel output, with worse gold recall (38%) than running unprimed (81%). The right interaction is independent accumulation: each phase contributes to the same graph through separate adapter IDs, and reinforcement mechanics surface relationships confirmed by multiple independent sources. This is Invariant 45 working exactly as designed.

## What remains

The extraction architecture is now defined by research rather than speculation. Three questions from the research log are resolved:

- **Q0 (chunking):** Rejected. Structured output is the intervention, not input chunking.
- **Q1 (classification vs generation):** Resolved differently than expected. Generative extraction with structured output achieves workable recall. Classification is unnecessary. But deterministic Phase 2 extraction — co-occurrence and dependency parsing — provides free coverage of structural relationships that complements the LLM's abstract reasoning.
- **Q2 (domain vocabulary):** Solved by glossary priming with multi-run cross-condition union. The domain recall gap was a strategy gap, not a capability gap.

**Q3 (GPU scheduling)** is simplified by the single-model architecture. All Phase 3 runs use Mistral:7b sequentially. No model swapping, no memory contention, no concurrent scheduling. The llm-orc max concurrency setting handles this at the infrastructure level.

The next step is building the Phase 3 extraction ensemble in llm-orc and running it against the full 24-essay corpus. The Phase 2 extractors need implementation as Rust-native adapters. Neither requires new engine design — the contribution tracking, scale normalization, enrichment loop, and deterministic concept IDs that the extraction pipeline relies on have been in place since Essay 06 (reinforcement mechanics) and Essay 12 (provenance infrastructure). The extraction layer is composed from infrastructure that was built for other reasons and works here without modification.
