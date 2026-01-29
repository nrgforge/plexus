# Response Document: Plexus Paper

## How to Use This Document

This document presents each issue raised during peer review alongside relevant paper content and specific questions you must address. Work through each section, drafting your response. Your responses will form the basis of the revised manuscript and the formal response letter to reviewers.

**Review Summary:**
- 4 reviewers, all recommended MAJOR REVISIONS
- 10 major issues identified (convergent across reviewers)
- 14 minor issues identified
- Key strengths: theoretical synthesis, epistemic honesty, validation-over-access principle

---

## Issue 1: Abstract Misrepresents Paper Contribution

**Severity:** Major
**Raised by:** Reviewers 2, 4
**Category:** Writing / Positioning

### What the Reviewers Said

**Reviewer 2:** "The paper suffers from a fundamental mismatch between claims and evidence. It reads as a vision paper but presents itself as a systems paper... the paper needs either (a) to reposition as an explicitly speculative vision/design paper with appropriate hedging, or (b) to execute at least one of the proposed studies."

**Reviewer 4:** "The abstract presents Plexus as an operational system ('Plexus receives data... processes it... emits events') using present tense throughout, suggesting a completed, functioning system. Only in §6 does the paper acknowledge that 'the system is partially built'... Readers expecting a systems paper with implementation and evaluation will be misled."

### Relevant Paper Content

Current abstract (lines 12-17):
> "We present **Plexus**, a content-agnostic knowledge graph engine designed to evolve alongside creative composition. Plexus receives data from domain-specific clients, processes it at multiple frequencies with self-reinforcing edge dynamics inspired by Hebbian learning, and emits events that clients can use to provide ambient structural awareness..."

§6 acknowledgment (line 427):
> "The system is partially built: the Rust graph engine exists, the semantic extraction pipeline is experimentally validated [Paper 1], and the llm-orc and clawmarks integrations are operational. What remains is domain consumer development... the self-reinforcing edge dynamics, and — most importantly — empirical validation..."

### Questions to Address

1. Should the abstract be rewritten to frame this explicitly as a design/vision paper?
2. What tense should be used — present (for design intent) or conditional (for unbuilt components)?
3. Should "we present" become "we propose"?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change in the abstract]

---

## Issue 2: Content-Agnostic Claim Underspecified and Unvalidated

**Severity:** Major
**Raised by:** All 4 reviewers
**Category:** Architecture / Core Claim

### What the Reviewers Said

**Reviewer 1:** "The paper repeatedly claims that 'the same graph engine, edge dynamics, and update architecture serve all creative domains' (§1.3), but this is an architectural assertion, not an empirical finding... If domain-appropriate decay tuning requires substantially different parameters, or if validation mechanisms cannot be meaningfully abstracted, the 'content-agnostic' framing collapses into 'a family of related domain-specific systems that happen to share some infrastructure.'"

**Reviewer 2:** "The three domain consumers (Manza, Trellis, EDDI) all appear to be the author's own projects. The 'content-agnostic' claim requires demonstrating that the same engine serves genuinely different domains — but if all three were designed by the same person alongside the engine, the architecture may have been tacitly shaped to fit those specific use cases."

**Reviewer 3:** "The movement domain, in particular, relies on entirely different data sources (pose estimation, MHI/MEI), different validation mechanisms (Viewpoints repetition), and potentially different decay parameters..."

**Reviewer 4:** "If the claim reduces to 'the same database schema can store different data types,' it is trivial. If it means something stronger, that stronger claim needs defense."

### Relevant Paper Content

§1.3 claim:
> "Finally, the engine is content-agnostic: the same graph engine, edge dynamics, and update architecture serve all creative domains. Only the analyzers differ."

§3.4 acknowledgment (line 290):
> "Domain-appropriate decay tuning is an open design question — and if different domains require substantially different parameters, it complicates the content-agnostic claim (§5.3)."

### Questions to Address

1. What precisely is shared across domains? (Data model? Edge dynamics? Update architecture? All three?)
2. If validation mechanisms and decay parameters are domain-specific, what remains "content-agnostic"?
3. Should the claim be reframed as "designed for content-agnosticism" (hypothesis) rather than "demonstrated content-agnostic operation" (claim)?
4. Would third-party adoption be required to validate this claim? If so, is that feasible before publication?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 3: Validation Mechanisms Underspecified for Creative Domains

**Severity:** Major
**Raised by:** All 4 reviewers
**Category:** Architecture / Core Mechanism

### What the Reviewers Said

**Reviewer 1:** "The alternative — validation-based reinforcement — is well-defined for code (tests pass) but poorly defined for fiction and movement... how does the system distinguish deliberate repetition from accident? How does it detect compositional deepening rather than co-occurrence? These are hard computational problems, and the paper does not explain how Plexus solves them — it assumes they are solved."

**Reviewer 2:** "The fiction row states validation occurs through 'Recurrence + structural development (compositional deepening, not just co-occurrence)'... This is not a specification — it is an acknowledgment that the problem is unsolved."

**Reviewer 3:** "The validation mechanism for movement edges is 'Repetition in the Viewpoints sense — deliberate compositional recurrence, not accident.' But how does the system distinguish intentional from accidental?... Current pose estimation and movement classification cannot disambiguate these cases."

**Reviewer 4:** "The paper correctly identifies that validation-based reinforcement is superior to access-frequency reinforcement, but the validation mechanisms themselves remain vague."

### Relevant Paper Content

§3.4 validation table (lines 274-279):

| Domain | Validation mechanism |
|--------|---------------------|
| Code | Unit → integration → acceptance tests |
| Fiction | "Recurrence + structural development (compositional deepening, not just co-occurrence). Detection relies on the semantic and conceptual layers — the hardest domain case, as distinguishing deepening from co-occurrence may require LLM-level interpretation" |
| Movement | "Repetition in the Viewpoints sense — deliberate compositional recurrence, not accident" |

### Questions to Address

1. For fiction: What specific computational mechanism detects "compositional deepening"? What prompts or heuristics? What would distinguish deepening from co-occurrence in practice?
2. For movement: What signal distinguishes intentional repetition from habit, physical constraint, or fatigue?
3. Should you acknowledge that validation mechanism design is an open research problem for each creative domain?
4. If validation cannot be operationalized for creative domains, does the self-reinforcing model apply only to code?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 4: Self-Reinforcing Edge Dynamics Unimplemented and Unvalidated

**Severity:** Major
**Raised by:** Reviewers 1, 2, 3
**Category:** Architecture / Implementation

### What the Reviewers Said

**Reviewer 2:** "The paper devotes substantial space (§3.4) to the Hebbian-inspired edge model, presenting specific mechanisms... But §6 confirms this is 'what remains' — the dynamics are designed, not built... The self-reinforcing model is presented as a key differentiator from existing systems (§2.8 gap analysis), but it is currently vaporware."

**Reviewer 1:** "The argument that edges should strengthen through domain-appropriate validation rather than mere access frequency (§3.4) is the paper's most original contribution... [but] the distinction between structure/reasoning offloading distinction is... empirically undemonstrated."

### Relevant Paper Content

§2.8 gap analysis positions self-reinforcing edges as a key differentiator.

§6 (line 427):
> "What remains is domain consumer development... the self-reinforcing edge dynamics, and — most importantly — empirical validation..."

§3.4 specifies decay function:
> "`w(t) = w₀ × e^(-λt)` where λ corresponds to a configurable half-life (default: 1 week)"

### Questions to Address

1. Is any part of the self-reinforcing model implemented? (Decay function? Reinforcement hooks? Emergence detection?)
2. What evidence supports the 1-week decay half-life? Is this a hypothesis, pilot-data estimate, or placeholder?
3. Should §3.4 be labeled "Proposed Design" rather than appearing under "System Design"?
4. Should the gap analysis table note that self-reinforcement is unvalidated?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 5: EDDI Movement Classification Underspecified

**Severity:** Major
**Raised by:** Reviewers 1, 3
**Category:** Domain-Specific / Movement

### What the Reviewers Said

**Reviewer 3:** "The phrase 'still in development' is doing enormous work here. The entire gesture-to-graph pathway depends on movement classification, but no concrete method is specified. Viewpoints and Laban are mentioned as possibilities, but these are fundamentally different frameworks — Viewpoints is a rehearsal practice emphasizing spatial relationships and temporal awareness, while Laban is a codified notation system with discrete Effort/Shape categories."

**Reviewer 3:** "Viewpoints is invoked repeatedly — 'repetition in the Viewpoints sense,' 'deliberate compositional recurrence' — as if it provides a computational ontology. But Viewpoints is a training methodology developed by Mary Overlie and extended by Anne Bogart, not a classification system... This is a category error."

**Reviewer 1:** "The movement domain connects to a body of work on computational Laban Movement Analysis... but how does the system distinguish deliberate repetition from accident?"

### Relevant Paper Content

§4.3 (lines 370-371):
> "Movement qualities extracted via domain-specific methods (potentially including Viewpoints categories, mathematical heuristics, or other movement grammars — EDDI's classification approach is still in development)"

§3.4 movement validation:
> "Repetition in the Viewpoints sense — deliberate compositional recurrence, not accident"

### Questions to Address

1. What specific movement classification pipeline will EDDI implement? (Laban Effort qualities? Pose embeddings? Something else?)
2. Is Viewpoints being used as a conceptual frame (repetition creates meaning) or as a computational ontology (classifiable categories)? If the former, should you clarify this distinction?
3. Should EDDI be framed as future work rather than a third domain consumer on equal footing with Manza and Trellis?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 6: Non-Interpretation Constraint Boundary Unclear

**Severity:** Major
**Raised by:** Reviewers 1, 4
**Category:** Conceptual / Trellis

### What the Reviewers Said

**Reviewer 1:** "The paper distinguishes between structural observation ('these three fragments share the concept of isolation') and interpretation ('this character is struggling with loneliness'). But naming a concept is already interpretation. 'Isolation' is not a structural property of text — it is a semantic category that requires interpretive judgment to assign."

**Reviewer 4:** "The very act of suggesting that 'these three fragments share the concept of isolation' involves interpretation — deciding that the shared concept is 'isolation' rather than, say, 'solitude' or 'loneliness' is an interpretive act. How do you draw the line between structural observation and interpretation in practice?"

### Relevant Paper Content

§4.2 (lines 350-356):
> "The critical constraint is **non-interpretation**: Plexus reveals structure the writer has already created but does not impose interpretation. 'These three fragments share the concept of isolation' is structural observation. 'This character is struggling with loneliness' is interpretation — and is explicitly outside Plexus's scope."

§4.2 operationalization (lines 355-356):
> "Permitted behaviors include: juxtaposing fragments without comment, asking genuinely open questions ('anything here?'), inviting sorting... Prohibited behaviors include: naming themes ('these are about control'), claiming connections ('I noticed these relate'), and interpreting meaning..."

### Questions to Address

1. How do you reconcile "juxtaposing fragments that share 'isolation'" (permitted) with "naming themes" (prohibited)? Isn't identifying a shared concept naming a theme?
2. What criteria distinguish structural observation from interpretation? (One proposal from Reviewer 1: interpretation involves causal/intentional claims about author meaning; observation involves descriptive claims about textual properties)
3. How would you evaluate whether Trellis violated the constraint? What would a violation look like?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 7: Latency Targets Unvalidated

**Severity:** Major
**Raised by:** Reviewers 2, 3
**Category:** Engineering

### What the Reviewers Said

**Reviewer 2:** "The paper specifies target latencies (<100ms, <2s, 10-30s, minutes) but provides no measurements confirming these are achievable... If structural updates actually take 500ms, the design rationale changes significantly."

**Reviewer 3:** "The claim that structural updates occur at <100ms is offered without evidence... Interactive performance systems are notoriously latency-sensitive. Performers notice delays above ~50ms; delays above ~200ms break the sense of responsive coupling."

### Relevant Paper Content

§3.3 latency table:
| Layer | Target Latency | Method |
|-------|----------------|--------|
| Structural | <100ms | Deterministic parsing (tree-sitter, regex) |
| Relational | <2s | Lightweight text analysis |
| Semantic | 10-30s | LLM extraction |

### Questions to Address

1. Have you benchmarked structural-layer latency from the existing Rust implementation?
2. For EDDI specifically, what is the measured end-to-end latency from gesture to graph update?
3. Should these be labeled as "targets" or "specifications"?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 8: [Paper 1] Companion Paper Unavailable

**Severity:** Major
**Raised by:** Reviewers 2, 4
**Category:** Methodology

### What the Reviewers Said

**Reviewer 2:** "The paper heavily depends on findings from 'companion paper [Paper 1]' (file tree traversal, 0% hallucination, ~10s extraction floor, 2 worker limit). This paper cannot be evaluated without access to [Paper 1]."

**Reviewer 4:** "The paper repeatedly references 'Paper 1' and '[Paper 1]' for empirical validation... but the relationship between the two papers is not clearly stated. Are they meant to be read together? Is Paper 1 already published? Under review? A preprint?"

### Questions to Address

1. What is the status of Paper 1? (Published? Preprint? Working paper?)
2. Should Paper 1 be included as supplementary material or appendix?
3. Should key experimental findings be summarized in this paper with sufficient detail to stand alone?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 9: Flow Framework as Evaluation Target Questioned

**Severity:** Major
**Raised by:** Reviewers 1, 4
**Category:** Methodology

### What the Reviewers Said

**Reviewer 1:** "The paper appropriately cites methodological critiques of flow research... But the paper continues to use 'flow state' as a primary design motivation and evaluation target... If the flow framework is as methodologically troubled as the paper acknowledges, building the evaluation agenda around it risks producing results that are either uninterpretable or unfalsifiable."

**Reviewer 1 suggestion:** "Reframe the evaluation around more directly observable constructs: structural awareness accuracy (already proposed), task-switching frequency (already proposed), self-reported cognitive load, and attentional capture events."

### Relevant Paper Content

§2.2 acknowledges problems:
> "The flow-performance relationship, while positive, is moderate (r = 0.31) with unclear causal direction [52]."

§5.1 uses flow as primary measure:
> "**Claim**: A live knowledge graph whose events enable ambient structural feedback preserves creative engagement and structural awareness more effectively than traditional development tooling — with flow state as one operationalization of this effect."

### Questions to Address

1. Should flow be demoted from primary to secondary/exploratory measure?
2. What alternative primary measures would be more defensible? (Structural awareness accuracy? Cognitive load? Task-switching frequency?)
3. The paper already hedges flow as "one operationalization" — is this sufficient?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Issue 10: Implementation Status Unclear

**Severity:** Major
**Raised by:** Reviewers 1, 2, 4
**Category:** Writing / Clarity

### What the Reviewers Said

**Reviewer 2:** "The paper states 'the Rust graph engine exists' and lists components as 'operational' but provides no implementation details... The phrase 'partially built' is used once (§6) but the paper otherwise reads as if describing a working system."

**Reviewer 1:** "What exactly exists? Is the multi-frequency update model implemented? Is the self-reinforcing edge model implemented? The paper oscillates between describing what is built and what is designed without clearly distinguishing them."

**Reviewer 2 suggestion:** "Add a table specifying each component's implementation status: designed-only, prototype, alpha, production."

### Questions to Address

1. What is implemented and operational?
2. What is designed but not implemented?
3. What is planned/speculative?
4. Should you add an implementation status table?

### Author Response

[BLANK — draft your response here]

### Proposed Revision

[BLANK — describe what you will change]

---

## Minor Issues (Condensed)

### Issue 11: LuminAI Comparison Undersells Overlap
**Raised by:** R3 | **Location:** §4.3
The comparison frames LuminAI as "co-creative partner" vs. EDDI as "ambient awareness" but undersells LuminAI's memory and clustering. Clarify the genuine distinction (cross-session memory? Modality difference?).

### Issue 12: Missing PIM Literature
**Raised by:** R1 | **Location:** §2
Personal Information Management literature (Jones, Whittaker, Bergman) addresses the "keeping found things found" problem directly. Consider adding a paragraph.

### Issue 13: Decay Half-Life Lacks Justification
**Raised by:** R1, R2 | **Location:** §3.4
"Default: 1 week" is stated without rationale. Is this pilot data, literature-based, or placeholder?

### Issue 14: llm-orc/clawmarks Dependencies Confusing
**Raised by:** R1, R2 | **Location:** §3.1, §3.5-3.6
Described as "optional" but also providing core functions (extraction, provenance). Clarify minimal configuration.

### Issue 15: Mermaid Diagram in Manuscript
**Raised by:** R4 | **Location:** §3.1
Render the diagram as a figure or provide fallback description.

### Issue 16: Citation Style Inconsistent / [Paper 1] Placeholder
**Raised by:** R4 | **Location:** Throughout
Standardize citation format; add Paper 1 to References properly.

### Issue 17: Jargon Density in §2.3
**Raised by:** R4 | **Location:** §2.3
Rapid succession of technical terms may alienate readers from individual disciplines. Add brief glosses.

### Issue 18: Expertise Development Question Underdeveloped
**Raised by:** R1 | **Location:** §6
The question is raised and dropped. Either develop with competing hypotheses or move to future work.

### Issue 19: Evaluation Agenda Lacks Power Analysis
**Raised by:** R4 | **Location:** §5.1
Given moderate effect size (r=0.31), what sample size is needed? Is this feasible?

### Issue 20: Arousal-Theoretic Mapping Needs Detail
**Raised by:** R2, R3 | **Location:** §4.3
What is the mapping function from edge weight to environmental parameters?

### Issue 21: Missing MHI/MEI Citation
**Raised by:** R3 | **Location:** §4.3
Add citation to Bobick and Davis (2001).

### Issue 22: "Peripheral Vision for Knowledge Work" Overused
**Raised by:** R1 | **Location:** Throughout
The phrase becomes slogan rather than precise description. Unpack once, use sparingly.

### Issue 23: Paper Length Not Justified
**Raised by:** R4 | **Location:** §2, §4.2
Related Work could be compressed ~30% without loss.

### Issue 24: Self-Citation as External Validation
**Raised by:** R4 | **Location:** §4.2
"The Trellis paper [62]" is cited as if external support but is author's own work. Rephrase.

---

## Summary Checklist

| # | Issue | Response Drafted | Revision Planned | Revision Implemented |
|---|-------|-----------------|------------------|---------------------|
| 1 | Abstract misrepresents contribution | ☐ | ☐ | ☐ |
| 2 | Content-agnostic claim underspecified | ☐ | ☐ | ☐ |
| 3 | Validation mechanisms underspecified | ☐ | ☐ | ☐ |
| 4 | Self-reinforcing dynamics unvalidated | ☐ | ☐ | ☐ |
| 5 | EDDI movement classification underspecified | ☐ | ☐ | ☐ |
| 6 | Non-interpretation boundary unclear | ☐ | ☐ | ☐ |
| 7 | Latency targets unvalidated | ☐ | ☐ | ☐ |
| 8 | [Paper 1] unavailable | ☐ | ☐ | ☐ |
| 9 | Flow framework questioned | ☐ | ☐ | ☐ |
| 10 | Implementation status unclear | ☐ | ☐ | ☐ |
| 11-24 | Minor issues | ☐ | ☐ | ☐ |
