# Why Knowledge Graphs Should Learn: Validation-Based Reinforcement for Live Composition

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — January 2026*

---

## The Problem: Static Graphs in Dynamic Work

Knowledge graphs built by LLM extraction have a signal problem. Every relationship extracted carries equal weight — an import statement and a deep architectural dependency look the same. A co-occurrence between two concepts mentioned in the same paragraph is indistinguishable from a conceptual relationship that the author has deliberately developed across chapters. The graph doesn't know the difference between noise and structure.

This is tolerable for batch-processed, post-hoc graphs — the kind that index finished documents for retrieval (GraphRAG [1], LightRAG [2], iText2KG [22]). When the graph is a search index, precision can be traded against recall, and the user provides intent through queries. But for a *live* knowledge graph — one that evolves alongside creative composition and provides ambient structural feedback — the signal problem is acute. A graph full of equal-weight edges is not a structural reflection; it is a hairball. It shows everything and therefore reveals nothing.

The problem gets worse over time. As a composer works — writing code, drafting prose, accumulating research notes — the graph grows. New extractions add edges. Old edges persist. Nothing fades. The graph becomes denser but not more informative. The relationships the composer actually cares about are buried under the relationships that merely exist. This is the opposite of what a live structural tool should do: it should get *more useful* over time, not less.

Existing knowledge graph systems don't address this because they weren't designed for live composition. GraphRAG and its descendants assume documents are finished artifacts. Graphiti [25] builds graphs incrementally in real-time with a bi-temporal model, but targets AI agent memory — its notion of relevance is recency, not validated importance. Spaced repetition systems [29, 30] implement decay dynamics, but optimize for *recall* of known facts — a fundamentally different problem from *discovery* of emergent structure. InfraNodus [4] applies network science to knowledge management, revealing structural patterns through betweenness centrality and community detection, but does not dynamically re-weight edges based on how the user engages with the material.

What's missing is a graph that *learns* — that strengthens the relationships that prove important and lets the rest fade. Not through user ratings or manual curation (that defeats the ambient design), but through signals that naturally arise from the composition process itself.

## Why Access Frequency Is the Wrong Signal

The most obvious approach is the one web search pioneered: weight by usage. Edges the user traverses often must be important; edges never visited must not be. This is intuitive and wrong.

Consider a developer navigating a codebase. They repeatedly click through a tangled dependency chain — the same three files, the same confusing import path, the same unclear type relationship. Under access-frequency weighting, these edges strengthen. The graph learns that this dependency is important. But the developer isn't validating the relationship — they are *struggling with it*. The repetition signals confusion, not importance. A well-designed dependency, one the developer understands and rarely needs to trace, would be weighted low despite being architecturally central.

Access frequency conflates several distinct signals: importance, proximity, confusion, frustration, habit, and task assignment. A writer who re-reads the same chapter passage before every session is not validating those connections — they are warming up. A researcher who repeatedly navigates to the same paper is not confirming its relevance — they may be trying to understand why it keeps appearing. Frequency measures attention, not confirmation.

This is not a theoretical concern. Recommendation systems that optimize for engagement famously produce filter bubbles — reinforcing what users already see rather than surfacing what would be genuinely useful [cf. the extensive literature on algorithmic amplification]. The same dynamic would afflict a knowledge graph: popular edges get more popular, marginal edges disappear, and the graph converges on a distortion rather than a reflection.

## The Biological Analogy (and Its Limits)

A better model comes from neuroscience, though the analogy requires scoping carefully. The Hebbian principle — "neurons that fire together wire together" — is commonly cited but commonly misunderstood. The precise version is: synapses strengthen through *successful* co-activation. A synapse that fires and produces a downstream response strengthens (long-term potentiation, LTP). A synapse that fires without functional effect does not. The strengthening is contingent on *outcome*, not on mere co-occurrence.

This distinction matters. In biological learning, the mechanism that prevents every synapse from strengthening indiscriminately is precisely this outcome-dependence. If mere co-firing were sufficient, the brain would rapidly converge on noise — every coincidental co-activation would leave a trace. Instead, only *functional* co-activation — co-activation that produces a result — is reinforced.

The second relevant mechanism is decay. Unused synapses weaken over time (long-term depression, LTD), but this weakening is not permanent erasure. Bjork and Bjork [28] distinguish storage strength (relatively permanent) from retrieval strength (subject to decay). A memory that has decayed in retrieval strength but retains storage strength is *harder to access but not gone* — and critically, re-learning it after decay builds *higher* storage strength than maintaining continuous access would. This is the "desirable difficulty" principle that underlies spaced repetition: forgetting and re-learning is more effective than never forgetting.

These two mechanisms — outcome-dependent strengthening and productive decay — suggest a different approach to knowledge graph dynamics than access-frequency weighting. But the analogy has limits that must be stated. The mechanism we propose is, at its core, a weighted-graph update rule: edges gain weight through validation events and lose weight through time-based decay. We use the neuroscience terminology because the biological analogy motivates specific design choices — particularly the distinction between validation-based and frequency-based reinforcement — but the implementation is a graph algorithm, not a neural simulation. The analogy is inspirational, not isomorphic. Convergence predictions based on the analogy need independent justification, which is why we specify simulation experiments below.

## Proposed Dynamics: Validation, Decay, Emergence

We propose three mechanisms for a self-reinforcing knowledge graph:

### 1. Validation-Based Reinforcement

Edges strengthen through **domain-appropriate validation** — signals that confirm the relationship is real, not merely extracted.

What counts as validation depends on the domain. In code: a test passes that exercises the relationship between two components. An integration test confirms that a module boundary works as declared. A refactoring that preserves a dependency is an implicit validation — the developer chose to keep it. In text and research: a concept re-extracted from a modified document confirms existing edges. A writer who groups fragments together, moves them into a shared collection, or explicitly links them has validated the connection through intentional compositional action. A citation that appears in multiple papers validates the connection between the cited concepts.

The critical distinction from access-frequency: validation requires *outcome*, not just *attention*. A test passing is an outcome. A writer grouping fragments is an outcome. A developer staring at a confusing dependency is not.

Validation strength is also not binary. A unit test confirms a component's contract — a narrow validation. An integration test confirms the *relationship between components* — a broader validation. An acceptance test confirms the relationship holds in context — the strongest signal. The graph can weight these proportionally: narrower validation produces less reinforcement than broader validation.

### 2. Temporal Decay

Edges lose weight over time if not re-validated. The proposed function is exponential decay:

```
w(t) = w₀ × e^(-λt)
```

where λ corresponds to a configurable half-life. A 1-week default reflects typical development rhythms — most active projects see engagement multiple times per week — but this is a working hypothesis, not an empirically validated parameter. The optimal decay rate is an open question.

Decay serves two functions. First, it prevents the graph from growing monotonically dense — edges that are never re-validated gradually fade, keeping the active structure visible. Second, it enables the desirable-difficulty dynamic from memory research [28]: an edge that decays and is then re-validated receives stronger reinforcement than an edge that was maintained continuously. This rewards relationships that prove *durably* important rather than merely recent.

A design question: should decay rates vary by domain? A developer working daily has different rhythms than a novelist writing weekly. If optimal decay parameters differ substantially across domains (by an order of magnitude or more), this has implications for content-agnostic architecture — a question we flag for empirical investigation rather than resolving here.

### 3. Emergent Connections

When two concepts co-occur across multiple documents without an explicit edge, a new edge is created with initial weight proportional to co-occurrence frequency. This allows the graph to discover relationships that the extraction pipeline didn't explicitly identify — patterns that only become visible through accumulation.

Emergent edges start at low weight ("sketch" status) and require validation to strengthen. Co-occurrence alone is not validation — it is a *hypothesis* that the extraction pipeline surfaces. The dynamics then determine whether the hypothesis gains support.

## The Confidence Gradient

These three mechanisms produce a natural gradient of structural confidence. Consider a "vibe-coding" scenario: a developer prompts an AI to generate components in rapid succession. Each generated component creates nodes and edges in the graph — but these edges start at sketch weight, representing asserted but unverified structure.

As the developer works (or as background processes run):
- A unit test confirms a component's contract → that component's edges strengthen slightly
- An integration test confirms the relationship between two components → the connecting edge strengthens more
- A refactoring preserves a dependency → implicit validation; the edge strengthens
- A dependency goes untested and untraversed for a week → it begins to fade
- A concept appears in three files without an explicit edge → an emergent edge forms at sketch weight
- That emergent edge is later traversed during a refactoring → secondary validation; it strengthens

The graph evolves from a faint, uniform sketch to a differentiated structure where the strongest edges correspond to validated, important relationships and the weakest edges represent unconfirmed extractions. Over time, validated structures solidify, unvalidated sketches fade, and emergent co-occurrences surface as candidates for validation.

This is the hypothesis. Whether it actually works — whether the dynamics converge to useful structure rather than oscillating, over-reinforcing popular paths, or prematurely pruning valid relationships — is an empirical question that requires investigation.

## Active Invalidation

Decay handles neglect — edges that are never re-validated fade gradually. But some relationships are actively *contradicted*. A test that previously passed now fails. A writer deliberately separates fragments they had grouped. A research argument is refuted by new evidence.

In these cases, the edge should weaken immediately rather than waiting for passive decay. We propose an active invalidation signal that reduces edge weight proportionally to the strength of the contradiction. A failing integration test is a stronger invalidation signal than a failing unit test, just as integration testing is a stronger validation signal than unit testing. This symmetry — validation strengthens, invalidation weakens, both proportional to signal scope — ensures the graph can unlearn as well as learn.

## Open Questions and Research Agenda

We are explicit about what this essay proposes versus what it demonstrates: the dynamics described above are a design specification. They have not been implemented, simulated, or validated. Several questions must be answered before implementation.

### Does the system converge?

The most basic question. Given the proposed update rules (validation-based reinforcement, exponential decay, co-occurrence emergence), does the edge weight distribution stabilize over time, or does it oscillate? Under what conditions? A toy simulation — a synthetic graph with simulated validation events — would answer this and is the immediate next step.

Specific concerns:
- **Runaway reinforcement**: Do popular edges attract disproportionate validation, creating a rich-get-richer dynamic that drowns out genuinely important but less-trafficked relationships?
- **Premature pruning**: Does decay remove valid but infrequently-accessed edges before they can be re-validated?
- **Oscillation**: Do edges cycle between strengthening and decay without stabilizing?
- **Decay sensitivity**: How sensitive is convergence behavior to the decay parameter? Is there a "Goldilocks zone" or is the system robust across a range?

### Does validation-based reinforcement actually outperform access-frequency reinforcement?

The argument for validation over access frequency is conceptual — the frustration counter-example, the filter-bubble analogy. But the claim that validation produces better graph structure is testable: compare graphs built with validation-based reinforcement against graphs built with access-frequency reinforcement, using practitioner judgment as ground truth. Do the strongest edges in each graph correspond to the relationships the practitioner considers important?

### What is the optimal decay rate?

The 1-week default is a guess informed by development rhythms. The optimal rate likely depends on engagement patterns, domain characteristics, and individual work styles. A study that varies decay parameters and measures graph utility (practitioner-judged edge importance, structural awareness in downstream tasks) would establish reasonable defaults and ranges.

### Do the dynamics behave differently across domains?

If optimal parameters (decay rate, validation weight, emergence threshold) differ substantially across domains, this has architectural implications. A system whose behavior depends heavily on domain-tuned parameters is less "content-agnostic" than one whose defaults work broadly. This is a question about the proposed dynamics specifically — not about the broader architecture — but the answer informs architectural decisions.

### Is the desirable-difficulty effect real in this context?

The prediction that re-validated-after-decay edges should strengthen more than continuously-maintained edges is borrowed from memory research [28]. Whether this translates from human memory to knowledge graph dynamics is not obvious. A simulation that compares "re-validate after decay" against "continuous maintenance" for edge stability and practitioner-judged usefulness would test this directly.

## Implications for System Design

The dynamics described here inform the design of Plexus, a live knowledge graph engine for creative composition. Several design decisions follow:

**Edge weight is not a single number.** Following Bjork and Bjork's [28] dual-strength model, edges should track both *storage strength* (cumulative validation history) and *retrieval strength* (current accessibility, subject to decay). An edge with high storage strength but low retrieval strength is one that *was* important, has been dormant, and would benefit strongly from re-validation. This is richer than a single weight and enables the desirable-difficulty dynamic.

**Validation events must be typed and weighted.** Not all validations are equal. The system needs a validation event interface that allows different signal sources (tests, user actions, extraction re-confirmation) to contribute different amounts of reinforcement. This is an adapter-level concern: the graph engine processes weighted validation events; the domain-specific adapter decides what events to emit and how to weight them.

**Decay should be configurable but defaulted.** The graph engine should implement exponential decay with a configurable half-life, but provide reasonable defaults. If the simulation work reveals that different domains need substantially different defaults, this should be a semantic adapter configuration, not an engine-level fork.

**The graph should distinguish sketch, developing, and established edges.** Rather than a continuous weight that clients must interpret, the system should provide categorical confidence levels derived from the underlying dynamics. This simplifies client rendering: a client can show sketch edges faintly and established edges prominently without understanding the weight algebra.

**Cold start is intentional.** A new project's graph should be faint, not empty. Everything starts at sketch weight and earns structural confidence through validation. This is a feature: the graph honestly represents the state of knowledge (nothing confirmed yet) rather than pretending that extracted relationships are established facts.

## Conclusion

Static-weight knowledge graphs are indexes. They answer queries about what was extracted but don't distinguish important structure from noise, and they get less useful as they grow. A self-reinforcing graph — one that strengthens validated relationships, decays unconfirmed ones, and discovers emergent connections — could evolve into a structural reflection that gets more accurate over time.

The proposed dynamics (validation-based reinforcement, temporal decay, co-occurrence emergence) are a design specification, not an implementation. The immediate next step is simulation: a synthetic graph with the proposed update rules, run under various conditions, measuring convergence behavior and sensitivity to parameters. If the dynamics converge to useful structure, they're worth building. If they don't — if they oscillate, over-reinforce, or prematurely prune — the simulation will reveal what needs to change before any code is written.

The broader question — whether a live, learning knowledge graph improves creative practice — cannot be answered by simulation alone. It requires building the system, putting it in practitioners' hands, and measuring whether the structural feedback it provides is genuinely useful. The dynamics are one component of that system. Getting them right matters because they determine whether the graph converges on signal or noise — and a graph that amplifies noise is worse than no graph at all.

---

## References

[1] Edge, D., Trinh, H., Cheng, N., et al. (2024). From Local to Global: A Graph RAG Approach to Query-Focused Summarization. *arXiv preprint arXiv:2404.16130*.

[2] Guo, Z., Xia, L., Yu, Y., Ao, T., & Huang, C. (2025). LightRAG: Simple and Fast Retrieval-Augmented Generation. In *Findings of ACL: EMNLP 2025*, pp. 10746-10761.

[4] Paranyushkin, D. (2019). InfraNodus: Generating insight using text network analysis. In *Proc. WWW '19*, pp. 3584-3589.

[22] Lairgi, Y., Moncla, L., Cazabet, R., et al. (2024). iText2KG: Incremental Knowledge Graphs Construction Using Large Language Models. In *Proc. WISE 2024*. arXiv:2409.03284.

[25] Rasmussen, P. (2025). Zep: A Temporal Knowledge Graph Architecture for Agent Memory. *arXiv preprint arXiv:2501.13956*.

[28] Bjork, R.A. & Bjork, E.L. (1992). A New Theory of Disuse and an Old Theory of Stimulus Fluctuation. In *From Learning Processes to Cognitive Processes*, Erlbaum.

[29] Settles, B. & Meeder, B. (2016). A Trainable Spaced Repetition Model for Language Learning. In *Proc. ACL 2016*.

[30] Zaidi, A. et al. (2020). Adaptive Forgetting Curves for Spaced Repetition Language Learning. In *AIED 2020*, Springer LNCS 12164, pp. 358-363.
