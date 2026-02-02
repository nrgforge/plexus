# PLEXUS-PAPER Stage 1 Findings: Literature Review + Citation Audit

**Date:** 2026-01-28
**Workflow:** Mode A (Full Pipeline)
**Paper:** docs/semantic/PLEXUS-PAPER.md

---

## Citation Errors (Must Fix)

### 1. [26] Author Mismatch
**Current:** "Arenas-Guerrero et al." in reference list
**Actual authors:** Van Assche, D., Rojas Melendez, J.A., De Meester, B., & Colpaert, P.
**Paper:** "IncRML: Incremental Knowledge Graph Construction from Heterogeneous Data Sources"
**Action:** Fix author attribution in reference list and any in-text mentions.

### 2. [27] Author Mismatch
**Current:** Listed as "Zhu et al." (implied by in-text usage)
**Actual first author:** Liu, J. (the reference list body correctly says "Liu, J." but the in-text may reference "Zhu")
**Paper:** "Towards Continual Knowledge Graph Embedding via Incremental Distillation" (AAAI 2024)
**Action:** Verify in-text attribution matches "Liu et al."

### 3. [34] In-Text / Reference Mismatch
**In §2.5:** "El Raheb et al. [34] survey ontology-based dance knowledge management comprehensively"
**But [34] is:** Paul, S., Das, P. P., & Rao, K. S. (2025). "Ontology in Dance Domain—A Survey"
**El Raheb is [33]**, not [34].
**Action:** Change §2.5 to "Paul et al. [34]" or rewrite to correctly attribute.

### 4. [42] Claim-Source Alignment (Overstated)
**In §2.3:** "Mark et al. [42] demonstrate that interrupted work carries significant recovery costs — after an interruption, workers require substantial time to resume their original task, and they compensate with faster (but more stressed) work."
**Actual finding:** The 2008 CHI paper found interrupted workers completed tasks *faster* but with more stress. The "substantial time to resume" / "23 minutes" finding comes from Mark's later work (not this paper).
**Action:** Either cite Mark's later work for the recovery-time claim, or reframe: "Mark et al. [42] demonstrate that interrupted work exacts cognitive costs — workers compensate by working faster but experience significantly more stress, frustration, and time pressure."

---

## Missing Literature (Must Add)

### A. Flow State Critique (Essential for §2.2 and §5.1)

**Farrokh, D., Stone, J. A., Davids, K., Strafford, B. W., & Rumbold, J. L. (2024).** "Why isn't flow flowing? Metatheoretical issues in explanations of flow." *Theory & Psychology*, 34(2).
- Challenges transient hypofrontality (Dietrich [20]) and argues flow research operates within an unchallenged cognitive metatheoretical framework.
- The paper cites Dietrich [20] uncritically. This counterpoint is needed.

**Wonders et al. (2025).** "Measuring Flow: Refining Research Protocols That Integrate Physiological and Psychological Approaches." *Human Behavior and Emerging Technologies*.
- Systematic review finding: 33/studies used non-validated flow instruments, 36 had ambiguous verification, 33 didn't match task difficulty to skill level.
- §5.1 proposes measuring flow. This paper is methodologically essential — it shows what can go wrong and how to design the study properly.

**Where to incorporate:** Add a paragraph to §2.2 acknowledging measurement challenges. Revise §5.1 to show awareness of these issues and describe methodological safeguards.

### B. Cognitive Offloading Risks (Essential for §2.1)

**Klein, G. & Klein, H. (2025).** "The Extended Hollowed Mind: Why Foundational Knowledge is Indispensable in the Age of AI." *PMC*.
- Introduces "hollowed mind" — cognitive atrophy from frictionless AI availability. EEG evidence shows reduced frontal theta power during LLM interaction.
- Direct counterpoint to Clark & Chalmers [18]. Plexus should position itself as avoiding this trap: externalizing *structure*, not *reasoning*.

**Gerlich, M. (2025).** "AI Tools in Society: Impacts on Cognitive Offloading and the Future of Critical Thinking." *Societies*, 15(1), 6.
- Empirical: significant negative correlation between frequent AI tool usage and critical thinking, mediated by cognitive offloading.
- Important counterweight to the paper's optimistic framing of external cognition.

**Where to incorporate:** Add 1-2 sentences to §2.1 after the Clark & Chalmers discussion. Distinguish Plexus's approach (externalizing structural awareness, not outsourcing reasoning) from the kind of offloading that produces cognitive atrophy.

### C. Vibe-Coding Formalization (Strengthens §2.3)

**The Vibe-Check Protocol (arXiv 2601.02410, Jan 2025).**
- Formalizes cognitive offloading in vibe coding via three metrics: Cold Start Refactor (skill retention via procedural decay), Hallucination Trap Detection (vigilance via Signal Detection Theory), Explainability Gap (metacognitive disconnect).
- Provides formal framework for the phenomenon §2.3 describes informally.

**Noda, A., Forsgren, N., Storey, M.-A., & Greiler, M. (2023).** "DevEx: What Actually Drives Productivity." *ACM Queue*.
- Establishes feedback loops + cognitive load + flow state as the three core dimensions of developer experience.
- Practically validates Plexus's design — Plexus addresses all three dimensions. Should be cited.

**Where to incorporate:** Add to §2.3, strengthening the empirical grounding.

### D. Trellis Theoretical Grounding (Fills §4.3 TODOs)

**Lee, M., Liang, P., & Yang, Q. (2024).** "Shaping Human-AI Collaboration: Varied Scaffolding Levels in Co-writing with Language Models." *CHI 2024*.
- Finding: paragraph-level AI scaffolding reduces user satisfaction and sense of ownership. Supports Trellis's non-generative design.

**Gero, K. I. et al. (2025).** "From Pen to Prompt: How Creative Writers Integrate AI into their Writing Practice." *arXiv 2411.03137*.
- 18 creative writers studied. Personal essayists most restrictive about AI generating text. Supports scaffolding-not-generation.

**Ramesh, V. et al. (2025).** "AI in the Writing Process" (Script&Shift). *arXiv 2506.20595*.
- Advocates process-oriented AI as "critical partner" not text generator. LLM-based tools that scaffold sub-processes without automating them.

**Where to incorporate:** §4.3 already has a TODO for "Vygotsky/ZPD, SDT, writing center pedagogy." These papers provide the empirical grounding for the scaffolding principle.

### E. Additional Strengthening Citations

**Rasmussen, P. (2025).** "Zep: A Temporal Knowledge Graph Architecture for Agent Memory." *arXiv 2501.13956*.
- The formal research paper for Graphiti/Zep. Currently only the GitHub repo is cited [25]. The paper reports benchmarks (94.8% on DMR) and describes the bi-temporal data model in detail.

**Matuschak, A. & Nielsen, M. (2019).** "How can we develop transformative tools for thought?" Essay.
- Foundational thinking on tools for thought. The mnemonic medium, creative prompts beyond recall, pro-serendipity. Relevant to the broader framing and to the distinction between recall-optimized (spaced repetition) and discovery-optimized (Plexus) systems.

**DASKEL (Eurographics 2023).** Interactive choreographic system with bidirectional skeleton-Labanotation conversion.
- Most directly comparable recent system for the movement domain. Should be cited in §2.5.

---

## How These Findings Reshape the Paper

### Two Critical Reframings Needed

**1. Flow state framing needs nuance (§2.2, §5.1)**
The paper currently treats flow as settled science with clear measurement. Recent literature shows:
- The neural model (transient hypofrontality) is contested
- Flow measurement instruments are largely non-validated
- Ecological validity of lab studies is questionable
- Flow-performance relationship is medium-sized (r=0.31) with unclear causal direction

The fix is NOT to abandon the flow framing — ambient structural feedback is a real design insight regardless of whether "flow" is the right theoretical lens. The fix is to:
- Acknowledge the measurement challenges in §2.2
- Frame the design principle as "ambient peripheral feedback that avoids interruption" rather than "flow-inducing"
- Redesign §5.1 with awareness of the methodological pitfalls (validated instruments, skill matching, ecological validity)

**2. Extended mind / cognitive offloading needs a counterweight (§2.1)**
The paper currently presents cognitive offloading as unambiguously beneficial. The "hollowed mind" literature shows it can erode the capacities it augments. But Plexus's design actually *avoids* this problem:
- Plexus externalizes *structural awareness* (what connects to what), not *reasoning* (what it means)
- It preserves the creator's interpretive agency (explicitly stated in §4.3's "non-interpretation" constraint)
- It makes structure *visible* rather than making decisions *for* the creator

This distinction should be made explicit. It's a strength of the design that the paper doesn't currently claim credit for.

### One Section Needs Filling

§4.3 (Trellis) and §4.4 (EDDI) have `<!-- TODO -->` and `<!-- PLACEHOLDER -->` markers. The scaffolding literature (CHI 2024, "From Pen to Prompt," Script&Shift) provides the theoretical grounding flagged as missing.

---

## Excluded Citations (With Rationale)

- **LuminAI (2025)** — Co-creative AI dance partner. Excluded per author's decision: prior work relationship with the lab; EDDI section already covers this design space.

---

## Revisions Applied (2026-01-28)

All Stage 1 findings have been incorporated into PLEXUS-PAPER.md:

- ✓ Fixed [26] in-text: "Arenas-Guerrero et al." → "Van Assche et al."
- ✓ Fixed [27] in-text: "Zhu et al." → "Liu et al."
- ✓ Fixed [34] in-text: "El Raheb et al. [34]" → "Paul et al. [34]"
- ✓ Fixed [42] claim: reframed to match actual finding (faster + more stress, not recovery time)
- ✓ Added [48]-[50]: cognitive offloading critique (Gerlich, Klein & Klein, Matuschak & Nielsen) to §2.1
- ✓ Added [51]-[52]: flow critique (Farrokh, Wonders) to §2.2, with design implications preserved
- ✓ Added [53]-[54]: Vibe-Check Protocol and DevEx framework to §2.3
- ✓ Added [55]: DASKEL to §2.5
- ✓ Added [56]-[58]: scaffolding literature (Lee, Gero, Ramesh) to §4.3 — filled TODO
- ✓ Added [59]-[60]: LuminAI (Trajkova et al. CHI 2024, C&C 2025) to §4.4
- ✓ Updated [25] to cite formal Zep paper (Rasmussen 2025) alongside GitHub repo
- ✓ Updated §5.1 with flow measurement caveats and methodological safeguards
- ✓ Updated §6 Discussion to reference new literature streams
- ✓ Decision: LuminAI cited neutrally for movement ontology context

## Next Steps

- Proceed to Stage 2 (argument audit + AI detect) on the revised paper
