# Spike Q0: Can Chunked Input Make the 14B Synthesizer Viable?

**Question:** Does chunking input into section-level segments allow Qwen3:14b to complete relationship extraction faster while maintaining quality?

**Answer:** No. But both questions were wrong. The key intervention is structured output (JSON schema constraint), not model size or chunking. Mistral:7b with structured output matches Qwen3:14b recall at one-third the time. Multi-run union with reinforcement mechanics handles noise.

**See `docs/research-log.md` Q0 section for complete findings (F0a–F0k).**

---

## Timing Results

| Run | Input chars | Prompt tokens | Gen tokens | Chars/token | Total time |
|-----|-------------|---------------|------------|-------------|------------|
| **Baseline (full text)** | 5,515 | 1,234 | 1,379 | 1.72 | **125.4s** |
| Section 1 (Problem) | 1,012 | 445 | 3,118 | 0.35 | 248.5s |
| Section 2 (Cognitive) | 976 | 422 | 3,522 | 0.28 | 282.9s |
| Section 3 (AI Acute) | 892 | 422 | 1,891 | 0.46 | 147.2s |
| Section 4 (Prompting) | 537 | 354 | 2,227 | 0.34 | 172.4s |
| Section 5 (Remedy) | 1,223 | 467 | 2,312 | 0.44 | 184.5s |
| Section 6 (Caveat) | 628 | 367 | 2,343 | 0.25 | 180.4s |
| **Section total** | — | — | — | — | **1,215.9s** |

Chunked approach is **9.7x slower** than baseline.

## Why Chunking Failed

The hypothesis assumed shorter input → less computation → faster completion. The reality:

1. **Shorter input triggers more thinking.** Qwen3 generates 2-3x more tokens per section than for the full essay. The chars/token ratio drops from 1.72 to 0.25-0.46, meaning 60-85% of generated tokens are invisible thinking tokens. The `/no_think` flag is unreliable on short inputs.

2. **More thinking, not better thinking.** The model compensates for less context by speculating more. The sections produce more false positives (wrong entity pairs, wrong relationship types) alongside the relationships they correctly capture.

3. **The bottleneck was never prompt evaluation.** Prompt eval for full text: 9.4s. For sections: 0.7-1.8s. Prompt processing is fast. The time cost is entirely in generation, and shorter input paradoxically increases generation.

## Recall Comparison

**Gold standard relationships (10):**
1. opacity problem --caused_by--> cognitive load
2. external structural representation --remedies--> opacity problem
3. vibe-coding --exemplifies--> opacity problem
4. material disengagement --describes--> vibe-coding
5. cognitive offloading --mechanism_of--> external structural representation
6. epistemic action --distinct_from--> pragmatic action
7. situation awareness --eroded_by--> opacity problem
8. AI-assisted composition --accelerates--> opacity problem
9. knowledge graph --instance_of--> external structural representation
10. computational offloading --component_of--> cognitive offloading

**Baseline recall (full text, 10 produced):**
- Clear matches: #2, #7, #9 (3/10)
- Partial: #1 (pair found, wrong type), #4 (pair found, wrong type)
- Missed: #3, #6, #8, #10

**Chunked recall (6 sections merged, 20 produced):**
- Clear matches: #1, #2, #7, #8, #9 (5/10)
- Partial: #4, #5, #6
- Missed: #3, #10
- Also: 5+ false positives (wrong entity pairs or entities not in list)

Chunked recall is modestly better (5 vs. 3 clear matches) because section-local context surfaces some relationships the full-text model misses (notably #8: AI-assisted composition accelerates opacity problem, which appears only in the "AI Makes It Acute" section). But the 10x time cost and higher false positive rate make this a bad tradeoff.

## The Actual Finding

**The 14B model is already viable.** The original rejection in Essay 24 was based on:
- "87% of system memory and 3+ minutes per run" (336s for the full synthesized pipeline)

But the 336s was the *whole pipeline* — three 8B specialists competing for GPU memory, then the 14B synthesizer. When the 14B model runs alone with entity context and the full essay text:
- **125.4s total** (9.4s prompt eval + 112.5s generation + 3.2s model load)
- Well within the 180s profile timeout
- No GPU memory contention (only one model loaded)

The viable architecture is sequential single-model execution:
1. Entity extraction with Qwen3:8b (~60s)
2. Entity extraction with Mistral:7b (~25s)
3. Merge entity lists (deterministic, <1s)
4. Relationship extraction with Qwen3:14b + entity context (~125s)
5. Theme extraction with Qwen3:8b (~50s, or parallel with step 4 if memory allows)

**Total: ~260s (~4.3 minutes), sequential on one GPU.**

This is within the "2-3 minutes per essay" budget Essay 24 established for the parallel entity-only pipeline, plus ~125s for the relationship extraction that was previously unsolved.

## Implications for Q1

Q1 asked: Can relationship extraction shift from generation to classification? This was motivated by the assumption that the 14B model was too expensive. With the 14B model viable at 125s:

- **Q1 is still worth investigating** as a potentially cheaper alternative (classification with 8B model could be faster)
- But it's no longer the *only* path to relationship extraction
- The comparison becomes: 14B generation (~125s, ~50% recall) vs. 8B classification (unknown time, unknown recall)
- If 8B classification can match 14B recall in less time, it's preferable. If not, the 14B model is the working solution.
