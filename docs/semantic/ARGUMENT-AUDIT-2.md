# Argument Audit: Plexus Paper (Second Pass)

**Paper:** "Plexus: A Content-Agnostic Self-Reinforcing Knowledge Graph for Live Creative Composition"
**Date:** January 2026
**Audit scope:** Full argument structure, internal consistency, logical validity, evidence alignment, gaps

---

## Argument Map

### Thesis / Central Claim

Plexus — a content-agnostic, self-reinforcing knowledge graph engine with multi-frequency updates and event emission — would, if fully implemented, address the "opacity problem" (structural awareness loss during creative composition) across multiple creative domains by enabling clients to provide ambient structural feedback.

### Supporting Argument 1: The Opacity Problem Is Real and Significant

**Claim:** Creators lose structural awareness of their own work as artifacts grow, and this is a significant cognitive problem.

**Premises:**
- P1: Working memory is sharply limited (Sweller [14]), so information density outpacing comprehension produces extraneous cognitive load
- P2: AI-assisted composition accelerates production while adding interruption costs (Mark et al. [42], Subramonyam et al. [43])
- P3: "Vibe-coding" and similar practices produce artifacts whose structure the creator didn't consciously design (Cito & Bork [10], Qiao et al. [11])
- P4: The problem is domain-general — it applies to code, writing, research, and movement composition

**Inference type:** Inductive (from multiple domain examples and cognitive science literature to a general claim)

**Evidence cited:** [10], [11], [12], [13] (industry survey, qualified), [14], [42], [43], [44], [53] (preprint, qualified), [54]

**Depends on:** None — this is foundational

### Supporting Argument 2: External Structural Representations Are the Remedy

**Claim:** Externalizing structural relationships in a live representation reclaims the cognitive capacity lost to opacity.

**Premises:**
- P1: External representations reduce cognitive burden through computational offloading, re-representation, and graphical constraining (Kirsh & Maglio [15], Scaife & Rogers [17])
- P2: Distributed cognition theory shows tools enable qualitatively different cognitive processes, not just amplified ones (Hutchins [16])
- P3: Diagrams replace memory retrieval with perceptual processing (Larkin & Simon [46])
- P4: External representations expand cognitive system boundaries (Kirsh [47])
- P5 (unstated): A knowledge graph specifically is an appropriate form of external representation for this problem

**Inference type:** Analogical (from established external cognition research to the specific case of a live knowledge graph)

**Evidence cited:** [15], [16], [17], [18], [46], [47], [64], [65]

**Depends on:** Argument 1

### Supporting Argument 3: Ambient Feedback Preserves Creative Process Better Than On-Demand Querying

**Claim:** Ambient, peripheral structural feedback is preferable to explicit-query feedback because it avoids interrupting the primary task.

**Premises:**
- P1: Interrupted work exacts significant cognitive costs (Mark et al. [42])
- P2: Flow involves transient hypofrontality, requiring feedback to be peripheral (Dietrich [20])
- P3: Ambient information can maintain awareness without attentional capture (Matthews et al. [21])
- P4: DAWs, 3D modeling tools, and game engines already provide this kind of live structural feedback successfully
- P5 (qualified): Flow theory provides supporting context but the claim stands on interruption-cost research alone

**Inference type:** Inductive (from interruption research + design precedent to the design principle)

**Evidence cited:** [19], [20], [21], [42], [51], [52]

**Depends on:** Argument 1, Argument 2

### Supporting Argument 4: Structural Offloading Avoids the "Hollowed Mind" Problem

**Claim:** Plexus externalizes structural *detection* but not *interpretation*, thereby avoiding the cognitive erosion that tools outsourcing reasoning cause.

**Premises:**
- P1: Tools that outsource reasoning degrade critical thinking (Gerlich [48], Klein & Klein [49])
- P2: EEG evidence shows reduced working memory engagement during LLM interaction [49]
- P3: Plexus shows "what connects" but not "what it means"
- P4: The distinction between structural offloading and reasoning offloading is stable and implementable

**Inference type:** Deductive (If the problem is reasoning offloading, and Plexus doesn't offload reasoning, then Plexus avoids the problem)

**Evidence cited:** [48], [49], [50]

**Depends on:** Argument 2

### Supporting Argument 5: Self-Reinforcing Edge Dynamics Produce Useful Graph Convergence

**Claim:** Validation-based reinforcement with temporal decay will cause the graph to converge on the relationships that matter to the practitioner.

**Premises:**
- P1: Hebbian learning (successful co-activation, not mere co-occurrence) is the right analogy for knowledge graph edge strengthening
- P2: Validation is a stronger signal than access frequency (access conflates importance with proximity/frustration)
- P3: Memory research (Bjork & Bjork [28]) shows desirable difficulties improve storage strength
- P4: Domain-appropriate validation mechanisms exist or can be designed for each domain

**Inference type:** Analogical (from neuroscience/memory research to graph algorithms) + Hypothetical (convergence is predicted, not observed)

**Evidence cited:** [28], [29], [30]

**Depends on:** None directly, but the system's value depends on this

### Supporting Argument 6: Content-Agnosticism — Same Engine, Different Adapters

**Claim:** A single content-agnostic graph engine can serve code, writing, and movement domains with only the semantic adapters differing.

**Premises:**
- P1: All composition produces structure regardless of medium
- P2: The graph data model (nodes, edges, weights, decay) is general enough for all domains
- P3: Domain knowledge is isolated in semantic adapters (extraction + provenance)
- P4: The adapter interface is expressive enough for all domains without engine-level modifications

**Inference type:** Deductive from architecture + Hypothetical (untested)

**Evidence cited:** Three domain examples (Manza, Trellis, EDDI)

**Depends on:** Argument 5 (convergence must work similarly across domains)

### Supporting Argument 7: Multi-Frequency Updates Solve the Latency Problem

**Claim:** Tiered update frequencies allow a live graph to stay responsive despite expensive LLM extraction.

**Premises:**
- P1: LLM extraction takes ~10s per document [66]
- P2: Different semantic layers have fundamentally different computational costs
- P3: Precedent exists in Lambda/Kappa architectures, CEP, and multi-layer monitoring [37]–[41]

**Inference type:** Deductive (from computational constraints to architectural solution)

**Evidence cited:** [37], [38], [39], [40], [41], [66]

**Depends on:** None

### Argument Dependencies (Critical Path)

```
Arg 1 (Opacity is real)
  → Arg 2 (External representations help)
    → Arg 3 (Ambient > on-demand)
    → Arg 4 (Structural offloading ≠ reasoning offloading)

Arg 5 (Self-reinforcing convergence) — independent but critical
Arg 6 (Content-agnosticism) — depends on Arg 5
Arg 7 (Multi-frequency) — independent, architectural
```

The central thesis requires ALL of these to hold. Arguments 1, 2, and 3 form the motivational chain. Arguments 5 and 6 form the architectural chain. Argument 7 is the engineering solution. Argument 4 is the defense against the strongest objection.

---

## Internal Consistency Report

### Contradictions Found

| # | Claim A (Location) | Claim B (Location) | Nature of Contradiction |
|---|---|---|---|
| 1 | "Plexus behaves identically in all cases" (§1.2, line 50) | Decay parameters, validation mechanisms may need to differ by domain; content-agnostic claim may reduce to "shared infrastructure" (§3.4, line 317; §1.3, line 66) | Direct tension: "identically" vs. acknowledged need for domain-specific parameters. The paper qualifies this tension in §1.3 and §3.4, but line 50 still asserts identity without qualification. |
| 2 | "The graph engine does not know or care whether a concept was extracted by an LLM" (§3.1, line 232) | Node types include domain-specific types: "concept, function, character, pose, fragment" (§3.2, line 245) | If the data model includes domain-specific node types, the engine *does* encode domain knowledge in its schema, even if it doesn't interpret it. The paper never addresses whether node typing is part of the engine or the adapter. |
| 3 | Non-interpretation constraint: Plexus shows "what connects" not "what it means" (§2.1, line 106; §4.2, line 405) | Semantic layer extracts "themes, narrative arcs, character development trajectories" (§3.3, line 274) and LLMs detect "compositional deepening" (§3.4, line 302) | Extracting themes and detecting compositional deepening *is* interpretation. The paper draws a line between infrastructure-level pattern detection and user-facing communication, but this distinction is unstable — the system internally interprets and then decides what to withhold. |
| 4 | "ambient structural feedback" with "cognitive overhead literally zero" for EDDI (§4.3, line 419) | "An open question is whether Plexus's cognitive overhead... exceeds the cognitive savings" (§6, line 497) | The EDDI claim of zero cognitive overhead is made for the specific case where the performer never sees the graph, but the §6 discussion doesn't reconcile this — it implies overhead is universal. Minor inconsistency. |
| 5 | Abstract: "Core components (the graph engine, semantic extraction, event emission) are operational" (line 14) | §1.5: "the coordination model is incomplete" for multi-frequency updates (line 80) | The abstract lists "event emission" as operational but the multi-frequency event coordination that drives useful event emission is incomplete. What's "operational" is narrower than the abstract implies. |

### Terminology Shifts

| Term | Meaning in [Section] | Meaning in [Section] | Problematic? |
|---|---|---|---|
| "content-agnostic" | Engine architecture that serves all domains with same data model and dynamics (§1.3, abstract) | Architectural hypothesis that may be falsified; may reduce to "shared infrastructure" (§1.3 line 66, §5.3) | Yes — the title and framing treat it as a property, while the body treats it as a hypothesis. The paper now acknowledges this tension (line 66), but the title still asserts it as a property. |
| "validate" / "validation" | Domain-appropriate confirmation that a relationship holds (§3.4) | Also used for empirical validation of the system itself (§5), and for companion paper's experimental validation (§3.7) | Minor — context disambiguates, but the triple use in a paper about validation-based reinforcement creates occasional confusion. |
| "ambient" | Peripheral, non-attentional (§2.2, §3) | Embodied in the environment for EDDI (§4.3) | Minor — the EDDI case is a specific form of ambient, but it's a different *kind* of ambient than a peripheral display. The paper could be clearer that these are two distinct design patterns under the same label. |
| "self-reinforcing" | Edges strengthen through validation (§3.4) | Also used in title and throughout as a system-level property | Yes — the title says "Self-Reinforcing Knowledge Graph" but the self-reinforcing dynamics are a design specification, not implemented. The title claims a property the system doesn't yet have. |

### Broken Dependency Chains

| Conclusion | Depends On | But Paper Says | Location |
|---|---|---|---|
| Content-agnosticism is the core architectural claim | Validation mechanisms expressible through same API across domains | Fiction validation (compositional deepening) is "open research" and may require LLM-level pattern detection that is domain-specific | §3.4, line 306 |
| Self-reinforcing dynamics produce useful convergence | Validation mechanisms that distinguish meaningful from spurious relationships | Movement domain cannot reliably distinguish intentional from accidental repetition | §3.4, line 310 |
| The graph converges on "relationships that matter to the practitioner" | Some ground truth for what "matters" | No mechanism proposed for establishing ground truth except post-hoc practitioner judgment | §5.2 |
| Plexus addresses the opacity problem | Full system operational | 2/3 domain consumers not operational; self-reinforcing dynamics not implemented; multi-frequency coordination incomplete | §1.5, §6 Limitations |

### Abstract–Body Mismatches

| Abstract Claim | Body Treatment | Gap |
|---|---|---|
| "self-reinforcing edge dynamics inspired by Hebbian learning" — presented as a system feature | §3.4 explicitly says this is "a design specification" not yet implemented | The abstract's "is designed to" language is better than before but still reads as describing a system that does this, not one that would. |
| "proposed multi-frequency model targets structural updates in <100ms" | §3.3 note: only structural layer achieves this; "full multi-frequency coordination model... is not yet implemented" | The abstract's "targets" language is appropriate, but the juxtaposition with "Core components... are operational" creates ambiguity about what's working. |
| "Three domain consumers illustrate the content-agnostic intent" | Only Manza is operational; Trellis is prototype; EDDI is design specification | The abstract now includes status markers (operational/prototype/design stage) — this is well-handled. |

---

## Logical Audit

### Validity of Inference

| Argument | Inference Type | Valid? | Issue (if any) |
|----------|---------------|--------|----------------|
| 1 (Opacity problem) | Inductive | Yes | Well-supported by multiple independent sources. Domain-generality claim is the weakest link — evidence is strongest for code, weaker for movement. |
| 2 (External representations) | Analogical | Partial | The external cognition literature supports externalization generally. The specific claim that a *knowledge graph* is the right representation is assumed, not argued. Why not a hierarchical view? A timeline? A simple dependency list? |
| 3 (Ambient > on-demand) | Inductive | Partial | The interruption-cost evidence is strong. The leap from "interruptions are costly" to "continuous ambient display is the solution" skips the alternative: *better-timed* on-demand queries (predictive, context-triggered). The paper frames this as a binary (ambient vs. explicit query) when the design space is a continuum. |
| 4 (Structural ≠ reasoning offloading) | Deductive | Partial | The distinction is drawn clearly but **its stability is questionable**. The system uses LLMs internally to extract themes and detect compositional deepening (interpretation), then claims it only surfaces structure. The line between "the system interpreted internally" and "the system told the user its interpretation" is a UX decision, not an architectural guarantee. |
| 5 (Self-reinforcing convergence) | Analogical + Hypothetical | Partial | The analogy from neuroscience is explicitly scoped as inspirational, not isomorphic (good). But the convergence prediction is entirely hypothetical — no simulation, no toy model, no formal analysis of the dynamics. The paper acknowledges this. |
| 6 (Content-agnosticism) | Deductive + Hypothetical | Weak | This is the weakest argument. It asserts that domain knowledge can be cleanly isolated in adapters, but the body reveals multiple points where domain knowledge leaks into the engine: node types, decay parameters, validation mechanisms. The paper provides falsification criteria (§5.3) but no positive evidence. |
| 7 (Multi-frequency) | Deductive | Yes | Well-grounded in precedent. The architectural logic is sound. Implementation details remain, but the design reasoning is valid. |

### Premise Evaluation

**Most dangerous unstated premises:**

1. **Graph representation is universally appropriate.** The paper now mentions this in the Limitations section (point 6), but never engages with it substantively. For movement, a *temporal* representation (timeline, sequence) might better capture choreographic structure than a graph. For fiction, a *hierarchical* representation (outline, tree) might better capture narrative structure. The paper assumes graphs are the right tool for all domains without arguing for this.

2. **Practitioners *want* continuous structural awareness.** The paper argues that opacity is a problem, and that externalization is the remedy — but jumps over the question of whether practitioners actually desire continuous structural feedback during composition. Many writers deliberately avoid structural awareness while drafting. Many developers prefer to think about structure *after* a coding session, not during. The paper mentions this briefly (§6, line 497) but treats it as an open question rather than a potential refutation.

3. **The non-interpretation constraint is stable.** The paper's claim that Plexus shows "what connects" but not "what it means" depends on a clean distinction between structural observation and interpretation. But the semantic layer extracts themes, narrative arcs, and character development trajectories. These *are* interpretations. The constraint is really about what gets communicated to the user, not about what the system does — and that's a prompt engineering / UX design decision, not an architectural property.

4. **"Composition" is the right frame for all target domains.** The paper treats code, fiction, research, and movement as instances of "composition." But code writing involves debugging, refactoring, and maintenance — activities that are not well-described as "composition." Research involves critical evaluation and synthesis. Movement involves improvisation and physical training. The "composition" frame fits fiction best and stretches increasingly thin for the other domains.

### Evidence-Claim Alignment

| Claim | Evidence | Alignment Issue |
|-------|----------|----------------|
| "vibe-coding" causes structural awareness loss | Cito & Bork [10], Qiao et al. [11] | These papers describe related phenomena but the specific claim about structural awareness loss is the paper's own synthesis, not directly measured by any cited study. |
| Ambient feedback preserves flow | Flow theory [19], [20] + Matthews et al. [21] on ambient displays | The paper correctly notes flow measurement problems. The evidence for *ambient structural feedback specifically* (not just ambient information generally) is thin — no direct study. |
| Validation-based reinforcement is superior to access-frequency reinforcement | Argument from the frustration example (developer navigating tangled dependency) | This is a thought experiment, not evidence. The claim is plausible but not empirically grounded. No citation compares the two approaches. |
| DAWs/3D tools provide precedent for live structural feedback | Stated as fact (§2.2, line 116) | This is an analogy, not evidence. DAWs provide feedback on the *same modality* (audio→waveform). Plexus provides feedback on a *different modality* (code→graph). The cognitive transfer is not equivalent. |

---

## Argument Gaps

### Unsupported Claims

- **"all composition — regardless of medium — produces structure" (§1.2, line 36)** — This is asserted as "the core insight" but never argued. Free improvisation, stream-of-consciousness writing, and exploratory play may not produce structure in the graph-representable sense. The claim needs scoping.

- **"the graph is hypothesized to converge on the relationships that actually matter to the practitioner" (§1.3, line 60)** — No formal or even informal analysis of convergence properties. Not even a toy simulation. The dynamics could produce pathological behavior (oscillation, runaway reinforcement, premature convergence) and there's no analysis showing they won't.

- **"A tree-sitter parser and a pose tracker are structurally equivalent from Plexus's perspective" (§3.3, line 279)** — This is the key content-agnosticism claim stated as architectural fact. But it's precisely what §5.3 says needs testing. Stating it as fact in §3 while acknowledging it's a hypothesis in §5 is a tense mismatch.

### Hidden Assumptions

- **Graph topology captures what matters about structure.** A graph captures pairwise relationships. But some structural properties (narrative arc, temporal progression, hierarchical containment) are poorly represented by flat graphs. The paper uses a layered model (structural/relational/semantic/conceptual) to partially address this, but the layers are *also* graph layers — they don't escape the fundamental representation.

- **Extraction quality is sufficient.** The entire system's value depends on LLM extraction producing accurate semantic structure. [66] validates this for text domains, but the paper assumes this capability scales to all the semantic/conceptual layer content described (themes, narrative arcs, architectural patterns). The gap between "we can extract concepts from text" and "we can extract character development trajectories" is substantial.

- **Ambient information is processable during creative work.** The external cognition literature shows that external representations help, but most studies involve *deliberate consultation* of the representation. The claim that *ambient* (peripheral, non-attended) information provides structural awareness is much less studied. Matthews et al. [21] is about glanceable displays for simple information (weather, email count), not semantically rich graph structures.

### Missing Counterarguments

- **"Just use an LLM to explain the structure."** The paper's opacity problem has a simpler solution: ask an LLM to summarize the codebase structure, explain dependencies, or identify themes. This is on-demand, but it addresses the same problem without requiring a persistent graph engine. The paper never engages with this alternative. **Severity: Moderate** — this is the pragmatic competitor that many practitioners already use.

- **"Structure awareness is overrated."** Many successful creative practitioners deliberately suppress structural awareness during composition — writers who don't outline, developers who spike solutions before architecting, choreographers who improvise. The paper assumes structural awareness is always beneficial but doesn't engage with creative traditions that value structural ignorance during the generative phase. **Severity: Moderate** — would narrow the target audience claim.

- **"The graph itself will become a distraction."** Even with ambient rendering, a constantly-updating visualization competes for visual attention with the primary creative artifact (code, text). The paper addresses this briefly (§6, line 497) but doesn't engage with the inattentional blindness literature or the cognitive cost of *ignoring* a changing visual stimulus. **Severity: Minor** — the paper acknowledges the question without engaging deeply.

- **"N=1 domains don't prove content-agnosticism."** The paper has one operational domain consumer (Manza), one prototype (Trellis), and one design specification (EDDI). Content-agnosticism requires at least two fully operational consumers to test. The paper acknowledges this in the Limitations section but the gap between the claim's ambition and its current evidentiary basis is large. **Severity: High** for the content-agnostic claim specifically.

### Scope Overreach

- **Title: "A Content-Agnostic Self-Reinforcing Knowledge Graph"** — The system is neither content-agnostic (only one domain operational, agnosticism is a hypothesis) nor self-reinforcing (the dynamics are a design specification). The title describes the aspiration, not the artifact. This is acknowledged in the body but the title still overreaches.

- **The movement/EDDI domain.** §4.3 runs to ~600 words of detailed specification (skel-mhi, MEI streams, arousal mapping, specific transfer functions) for a system that doesn't exist. The level of specification detail creates an impression of engineering completion that contradicts the "design specification" status. The specificity is intellectually interesting but misrepresents readiness.

### Inferential Leaps

- **From "external representations help cognition" → "a self-reinforcing knowledge graph with multi-frequency updates is the right external representation."** The external cognition literature supports externalization generally. The specific architectural choices (graph not tree, self-reinforcing not static, multi-frequency not single-pass) are design decisions, not implications of the theory. The paper presents them as following from the theory when they're independent choices.

- **From "interruptions are costly" → "continuous ambient display is the solution."** The design space between "explicit query requiring mode-switch" and "continuous peripheral display" includes: context-triggered notifications, predictive queries, session-boundary summaries, and other patterns that reduce interruption without requiring continuous display. The paper treats this as a binary.

- **From "Hebbian learning works for neurons" → "validation-based reinforcement will work for knowledge graphs."** The paper scopes this as "inspirational, not isomorphic" (good), but then proceeds to make predictions about convergence behavior based on the analogy. If the analogy is only inspirational, convergence predictions need independent justification.

---

## Argument Strengths

1. **Exceptional intellectual honesty about implementation status.** §1.5 is a model of how design papers should present themselves. The built/designed/planned separation is clear and consistently maintained through most of the paper. The Limitations subsection consolidates this well.

2. **Sophisticated engagement with counterarguments.** The paper proactively addresses the hollowed-mind critique (§2.1), flow measurement problems (§2.2), fiction-domain circularity (§5.2), and content-agnostic falsification criteria (§5.3). This is unusually thorough for a design paper.

3. **The validation vs. access-frequency distinction.** Even without empirical evidence, the argument that edges should strengthen through domain-appropriate validation rather than click frequency is well-reasoned. The frustration counter-example is effective. This is a genuine design insight.

4. **Clear falsification criteria.** §5.3 specifies what would falsify the content-agnostic claim. §3.4 specifies what would reduce it to "shared infrastructure." These are concrete and honest.

5. **The non-interpretation constraint** (§4.2) is well-specified with concrete permitted/prohibited behaviors and an implementation strategy. This is one of the most carefully argued design decisions in the paper.

6. **The IDE distinction** (§6, line 493) directly engages the most obvious practical objection and correctly identifies the difference between on-demand queries and ambient evolution.

## Strongest Links

- Argument 1 (opacity problem) → Argument 2 (external representations): This chain is well-supported by established literature and the synthesis is original.
- Argument 7 (multi-frequency architecture): Sound engineering reasoning with clear precedent.
- The interruption-cost basis for ambient design (§2.2–2.3): Stands independently of flow theory, as the paper now correctly positions it.

## Weakest Links

1. **Content-agnosticism (Argument 6):** The paper's central architectural claim, but also its least supported. One operational domain, domain-specific node types in the data model, acknowledgment that validation mechanisms and decay parameters may need domain-specific tuning. If this fails, the paper becomes a description of a code-domain knowledge graph with speculative cross-domain applicability.

2. **Self-reinforcing convergence (Argument 5):** Entirely hypothetical. No simulation, no toy model, no formal analysis. The paper hangs significant architectural weight on dynamics that might not converge, might converge to useless states, or might require domain-specific tuning that breaks content-agnosticism.

3. **Non-interpretation stability (Argument 4):** The distinction between structural observation and interpretation is a design aspiration implemented through prompt engineering, not an architectural property. The system *does* interpret internally (semantic extraction of themes, arcs, deepening). The constraint is about what gets communicated. This is more fragile than the paper acknowledges.

---

## Argument Audit Summary

**Overall logical coherence:** Moderate-to-Strong

The paper's motivational arguments (opacity problem, external cognition, ambient feedback) are well-constructed and well-supported. Its architectural arguments (content-agnosticism, self-reinforcing convergence) are well-articulated but essentially hypothetical. The paper knows this and says so — which is a strength. The remaining logical problems are mostly about the gap between how confidently the paper *frames* its claims (title, abstract, section headers) and how carefully it *qualifies* them (body text, limitations).

**Critical vulnerabilities:** 2

1. Content-agnosticism asserted in the title and framing but functioning as an untested hypothesis with evidence of domain knowledge leaking into the engine (node types, parameter tuning)
2. Self-reinforcing convergence entirely hypothetical — no analysis of the dynamics' behavior, even in simplified form

**Moderate issues:** 4

1. Non-interpretation constraint is a UX/prompt-engineering decision, not an architectural guarantee, despite being positioned as a design principle
2. Graph representation assumed universally appropriate — now acknowledged in Limitations but never engaged with substantively
3. "Composition" frame stretches thin across domains (debugging is not composition; improvisation is not composition in the same sense)
4. The inferential leap from "external representations help" to "this specific architecture is the right one" is unsupported by the cited theory

**Minor issues:** 4

1. "Plexus behaves identically in all cases" (§1.2) contradicts acknowledged domain-specific parameter needs
2. Node types in data model (function, character, pose) encode domain knowledge in the "agnostic" engine
3. DAW/3D tool analogy is same-modality feedback; Plexus is cross-modality — not equivalent
4. Title asserts properties (content-agnostic, self-reinforcing) the system doesn't yet have

### The Strongest Version of This Argument

Plexus is a knowledge graph engine for live creative composition, currently operational in the code domain. Its motivating insight is sound: creative composition produces structure faster than creators can track, and externalizing that structure as ambient feedback could reclaim cognitive capacity without disrupting the creative process. The architecture proposes a clean separation between a domain-agnostic graph engine and domain-specific semantic adapters — a hypothesis worth testing. The self-reinforcing edge dynamics, inspired by but not isomorphic to Hebbian learning, predict that validation-based reinforcement will cause the graph to converge on meaningful relationships — a prediction that requires empirical testing. Whether the architecture is truly content-agnostic, whether the self-reinforcing dynamics converge usefully, and whether ambient structural feedback improves creative practice are empirical questions with clear falsification criteria specified in the evaluation agenda.

### What Must Be Fixed

**Priority 1 — Critical:**

1. **Title–content mismatch.** The title asserts "Content-Agnostic Self-Reinforcing" as properties of the system. The body repeatedly acknowledges these are hypotheses. Options: (a) change the title to signal design intent ("Toward a Content-Agnostic..." or "Designing a Self-Reinforcing..."), or (b) keep the title and add a subtitle explicitly signaling design-paper status (the "Working Paper" label partially does this but doesn't address the property assertion).

2. **Convergence analysis gap.** The paper predicts convergence from an analogy. Even a toy simulation — a synthetic graph with the proposed update rules run for N iterations — would move the self-reinforcing argument from "we predict this" to "we've observed this in simplified conditions." Without *any* analysis of the dynamics, the convergence claim is a conjecture, not a hypothesis.

**Priority 2 — Moderate:**

3. **Node type leakage.** Address whether typed nodes (function, character, pose) are part of the engine or the adapter. If the engine schema includes domain-specific types, acknowledge this as a point where domain knowledge enters the "agnostic" engine and discuss the implications.

4. **"Behaves identically" in §1.2 line 50.** This directly contradicts the qualified content-agnosticism in §1.3, §3.4, and §5.3. Either qualify the claim ("is designed to behave identically") or remove it.

5. **Engage with alternative representations.** The Limitations section mentions this (point 6) but never discusses it. Add 2-3 sentences somewhere (§6 or §3.2) explaining why graphs specifically — as opposed to hierarchies, timelines, or spatial maps — are the chosen representation, and acknowledging that the choice is a design bet.

6. **Non-interpretation as UX vs. architecture.** Reframe the non-interpretation constraint honestly: the system *does* interpret internally (it extracts themes, detects deepening); the constraint is about what gets *communicated* to the user. This is a legitimate design principle but it's a prompt-engineering / output-filtering decision, not an architectural property. Currently it's positioned as more fundamental than it is.

**Priority 3 — Minor:**

7. **"all composition produces structure" (§1.2, line 36).** Scope this: "all *sustained* composition produces structure" or "composition processes that generate artifacts produce structure." Free improvisation and pure exploration may not.

8. **EDDI specification detail.** The level of detail in §4.3 (specific formulas, skel-mhi architecture, energy dissipation analysis) is disproportionate to implementation status. Consider signaling this disproportion more clearly — the "toy example" framing for the formulas is good, but the skel-mhi description reads like systems documentation for a system that doesn't exist.

9. **The "just ask an LLM" counterargument.** The IDE distinction (§6) addresses on-demand structural tools but not the simpler competitor: asking an LLM to explain structure on demand. This is worth a sentence.

10. **Abstract "Core components... are operational" (line 14).** This sits awkwardly next to §1.5's acknowledgment that the multi-frequency coordination model is incomplete. Tighten the abstract to specify *which* core components are operational rather than using the umbrella term.
