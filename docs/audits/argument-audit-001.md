# Argument Audit: Essay 001 — Query Surface Design

**Audited document:** `/Users/nathangreen/Development/plexus/docs/essays/001-query-surface-design.md`
**Evidence trail:** `/Users/nathangreen/Development/plexus/docs/essays/research-logs/research-log.md`
**Date:** 2026-03-23
**Auditor:** Claude Sonnet 4.6

---

## Argument Map

The essay makes six major inferential moves. Each is traced here from evidence to conclusion.

### Chain 1: Write-heavy / query-light pattern is valid for Plexus

**Premise A (research log, Question 1, Finding 1):** Graphiti validates the pattern empirically — entity extraction, resolution, temporal annotation, embedding computation, and community detection at write time; three mechanical primitives at query time; no LLM at retrieval.

**Premise B (essay §Write-Heavy):** Plexus follows the same pattern — enrichments run reactively at write time and pre-load the graph with derived structure.

**Conclusion (essay §Write-Heavy):** The research question is not whether Plexus needs a sophisticated query language but what the thin surface must contain.

**Verdict:** Valid inference. The analogy is appropriately scoped — Graphiti validates the general pattern, not the specific requirements. The essay does not claim Graphiti's surface is sufficient for Plexus; it frames Graphiti as existence proof.

**Hidden assumption:** That Plexus's enrichments are actually "rich enough" to make mechanical search sufficient — as Graphiti's are. The essay asserts this but does not demonstrate it. The degree of enrichment richness is doing significant argumentative work here and is not measured.

---

### Chain 2: Five architectural differentiators create novel query requirements

**Premise (research log, Question 2, Finding 1):** Codebase analysis identifies five properties that distinguish Plexus from comparison systems — multi-consumer convergence, contribution-tracking provenance as a graph dimension, evidence diversity as a derived signal, three-phase independent accumulation, and declarative consumer extension.

**Conclusion (essay §Where Plexus Diverges):** These properties create four query requirements that have no analog in comparison systems.

**Verdict:** Valid for three of five. Two differentiators require qualification:

- The **declarative consumer extension** differentiator (essay §Declarative Consumer Extension) produces the question "can a consumer also declaratively define how it queries the graph?" This is a reasonable inference from architectural symmetry, but it is not a necessary consequence. Declarative write configuration does not logically entail declarative read configuration. The step from "write is declarative" to "therefore queries could also be declarative" is presented as a natural follow-on but is a design choice, not a logical implication.

- The **three-phase independent accumulation** differentiator (essay §Independent Contribution Accumulation) creates a subtle query surface requirement: evidence diversity queries must distinguish between "multiple phases of the same adapter pipeline" and "multiple independent consumers." This is correct and well-reasoned. However, the essay treats this as a distinct architectural differentiator from evidence diversity generally, when it is actually a sub-case of the evidence diversity concern already established in Chain 3. This creates mild structural duplication in the argument.

---

### Chain 3: Evidence diversity is a distinct ranking dimension

**Premise (domain model, Evidence diversity definition):** Evidence diversity is derived at query time from provenance entries — count distinct adapter IDs, source types, and contexts. It is not a stored field.

**Premise (research log, Question 2, Finding 2):** No comparison system computes corroboration breadth at query time.

**Conclusion (essay §Evidence Diversity):** The query surface must support evidence diversity as both a ranking and filtering criterion.

**Verdict:** Valid inference. The domain model and research log are in agreement. The essay correctly identifies that evidence diversity is derived, not stored, which means query-time computation is required rather than a simple field filter.

**Hidden assumption:** That consumers will reliably want to distinguish highly-corroborated connections from weakly-corroborated ones. This is asserted rather than demonstrated from consumer scenarios. It is a reasonable design assumption but should be flagged as one.

---

### Chain 4: Projection is a necessary fourth extension axis

**Premise (research log, Question 1, Finding 2):** Neo4j GDS projections — named, declarative, in-memory subgraphs — map to Plexus's multi-consumer scenario.

**Premise (research log, Question 1, Finding 3):** MV4PG (Xu et al., 2024) formalizes materialized views for property graphs, achieving 28-100x speedup via incremental maintenance.

**Premise (essay §Projection Layer):** Plexus's meta-context concept is "close to a projection but scoped to context union rather than subgraph selection."

**Conclusion (essay §Projection Layer):** Projection is a distinct concern from enrichment — potentially a new fourth extension axis.

**Verdict:** Partially valid; the framing oversimplifies the analogy. The MV4PG speedup figure (28-100x) is cited as support for the projection concept, but MV4PG speedups come specifically from incremental view maintenance and query rewriting — neither of which Plexus proposes to implement. The speedup claim is therefore misleading as motivating evidence; it supports pre-computed materialized views, not the lightweight declarative projection filtering the essay actually proposes.

The Neo4j GDS analogy is better — Neo4j projections are declarative subgraph selections, which is what the essay envisions. But Neo4j GDS projections are in-memory subgraphs used specifically for graph algorithm execution (PageRank, community detection), not for scoping consumer queries. The analogy transfers the concept but not the use case precisely.

The essay appropriately hedges on whether projection is a genuine fourth axis or a configuration concern within the existing declarative adapter spec — deferring to the DECIDE phase. This hedging is the correct epistemic move.

---

### Chain 5: Three-level discovery hierarchy requires cross-context operations as first-class

**Premise (research log, Question 3):** User conversation described the Sketchbin federated scenario — two users' contexts connected through shared graph structure, discovery emergent from enrichments.

**Premise (essay §Three Levels of Discovery):** The `shared_concepts` primitive already exists in the query module but is not MCP-exposed.

**Conclusion (essay §Three Levels):** `shared_concepts` must become architecturally central, not optional; cross-context operations must be first-class in the query surface.

**Verdict:** Valid inference for the promotion of `shared_concepts`. However, there is a scope gap: the entire Level 3 argument rests on a single user conversation (research log, Question 3: "Method: Conversation with user (design insight, not web research)"). This is thin evidentiary basis for an architectural claim. The argument is internally consistent but its foundation is a single design discussion, not empirical research or multiple converging sources.

**Specific logical gap:** The essay states that `shared_concepts` "already exists in Plexus's query module but is not exposed via MCP." This is factually confirmed by the codebase (the function is in `src/query/shared.rs` and is not in `src/mcp/mod.rs`). The essay then states: "Combined with provenance filtering and evidence diversity ranking, it enables queries like 'what concepts do these two contexts share, and which shared connections are most independently corroborated?'" This composition is asserted but not demonstrated — `shared_concepts` currently returns only concept node IDs (a `Vec<NodeId>`), not edges or provenance data. The composition the essay describes would require additional query steps not currently represented in the surface.

---

### Chain 6: The central tension resolves into a two-mode query model

**Premise (essay §Central Tension):** A projection-scoped query is too narrow (misses cross-domain discoveries); a fully open query is too noisy (drowns the consumer in irrelevant data).

**Conclusion (essay §Central Tension):** The resolution is a two-mode model: scoped (default) and discovery (opt-in).

**Verdict:** The problem is correctly identified. The proposed resolution is plausible but presented as more settled than the evidence warrants. The essay frames the two-mode model as "the resolution likely involves..." — the hedging is appropriate — but the subsequent paragraph treats it as established design: "the escape hatch from scope to discovery is the query surface's most important design problem." Moving from "likely involves" to "most important design problem" in two paragraphs overstates the solution's firmness.

No comparison system implements a two-mode scoped/discovery query model. The essay does not cite any prior art for this design. It is a novel design proposal that emerges from the analysis, which is legitimate — but it should be explicitly labeled as a proposed design rather than a research finding.

---

## P1 Issues (Must Fix)

### P1-A — MV4PG speedup figures are misleading evidence for the projection concept

**Location:** §The Projection Layer, paragraph 2

**Claim:** "MV4PG approach treats a materialized view as a pre-computed subgraph derived from a declarative view definition... achieving 28-100x query speedup."

**Evidence gap:** The MV4PG speedup is a product of incremental view maintenance and query rewriting over pre-computed subgraphs — neither of which the essay proposes to implement in Plexus. The Plexus projection concept as described is a declarative subgraph selection filter applied at query time, not a pre-computed materialized view that is incrementally maintained. Citing 28-100x speedup to motivate a design that cannot reproduce the mechanism behind that speedup is a false transfer of evidence.

The speedup is the result of a specific implementation technique (incremental maintenance with merge/delta operations). Using it to support a conceptually related but mechanically different design implies benefits that won't materialize.

**Recommendation:** Remove the speedup figure from the argument or clearly mark it as a property of the full MV4PG implementation that Plexus is not adopting. The MV4PG paper's relevant contribution is the formalization of declarative view definitions over property graphs — cite that. The performance numbers apply to a different design than what is proposed.

---

### P1-B — The `shared_concepts` composition claim overstates current capability

**Location:** §Three Levels of Discovery, paragraph about Level 3; §What the Query Surface Must Contain, item 5

**Claim:** `shared_concepts` "Combined with provenance filtering and evidence diversity ranking... enables queries like 'what concepts do these two contexts share, and which shared connections are most independently corroborated?'"

**Evidence gap:** The current `shared_concepts` function (`src/query/shared.rs`) returns `Vec<NodeId>` — concept node IDs that appear in both contexts. It returns no edges, no provenance data, and no weights. The composed query the essay describes requires: (1) `shared_concepts` for the node intersection, (2) a way to retrieve edges involving those nodes across both contexts, (3) provenance-dimensional filtering on those edges, and (4) evidence diversity computation over the filtered edges. Steps 2-4 do not currently exist as query primitives, and their composition is not described in the essay. The essay implies a capability that requires significant additional query surface design.

**Recommendation:** Replace "enables queries like" with "would enable queries like, once combined with provenance filtering and evidence diversity primitives not yet designed." This reframes the statement honestly: `shared_concepts` is a necessary but not sufficient component of federated discovery queries.

---

## P2 Issues (Should Address)

### P2-A — Level 3 discovery rests on a single design conversation

**Location:** §Three Levels of Discovery; research log, Question 3

**Claim:** Cross-person / federated discovery is the third level of the discovery hierarchy and `shared_concepts` is architecturally central to it.

**Evidence gap:** The entire federated discovery level is sourced from a single user conversation — the research log explicitly notes "Method: Conversation with user (design insight, not web research)." All other claims in the essay draw on web research plus codebase analysis. This claim draws on a single design discussion. That is not an error in the argument's logic, but it is a weak evidence base for an architectural conclusion ("moves from 'nice to have' to architecturally central").

**Recommendation:** Mark the Level 3 claim as a design hypothesis sourced from a single conversation, not a research finding. The architectural elevation of `shared_concepts` should be presented as a design preference to be validated through consumer scenarios, not a research-derived requirement.

---

### P2-B — "Write-heavy enrichments are rich enough" is asserted, not demonstrated

**Location:** §Write-Heavy / Query-Light Hypothesis, throughout

**Hidden assumption:** The core argument depends on Plexus's enrichments being rich enough to make mechanical query primitives sufficient, by analogy to Graphiti. But Graphiti's enrichment richness is demonstrated by the Rasmussen et al. paper — entity extraction, resolution, temporal annotation, embedding computation, and community detection all happen at write time. Plexus's current core enrichments (co-occurrence, embedding similarity, discovery gap, temporal proximity) are less comprehensive than Graphiti's write-time pipeline.

The essay does not address whether the current enrichment set is sufficient or merely directionally correct. If it is not yet sufficient, the "thin query surface" conclusion is premature.

**Recommendation:** Add a qualification: the thin-query-surface conclusion holds if and when the enrichment set is sufficiently rich. The current set is directionally aligned with the pattern, but the gap between Plexus's current enrichments and Graphiti's write-time intelligence should be acknowledged. This is not a fatal objection — it is a maturity gap — but the essay elides it.

---

### P2-C — The "declarative writes imply declarative queries" inference needs grounding

**Location:** §Declarative Consumer Extension, final paragraph; §The Projection Layer

**Claim:** "If a consumer can declaratively define how data enters the graph... can it also declaratively define how it queries the graph?"

**Evidence gap:** The parallel between declarative writes and declarative queries is rhetorically appealing but not logically entailed. Declarative write configuration (YAML adapter specs) is motivated by the need to lower the barrier to adding a new consumer adapter without writing Rust. Declarative query configuration (projections) is motivated by the need for consumer-specific views over shared graphs. These are different problems. The ADRs and domain model do not establish a principle that "declarative at write time" implies "declarative at read time." The inference is a design preference, not an architectural necessity.

**Recommendation:** Separate the motivation. The argument for projections should stand on the multi-consumer shared-graph problem (which is well-established) rather than on symmetry with the write surface. The symmetry observation can be noted as a design aesthetic, but the architectural case should not depend on it.

---

### P2-D — Invariant 28 tension is understated

**Location:** §Invariant Tensions

**Claim:** "This is not a contradiction — `shared_concepts` already performs cross-context operations — but it suggests Invariant 28's scope statement may need amendment to clarify that cross-context queries are supported even though cross-context aggregation is not automatic."

**Evidence gap:** Invariant 28 states: "`list_tags(context_id)` is scoped to a single context with 'no cross-context tag aggregation at the API layer.'" The essay reads Invariant 28 narrowly as applying only to `list_tags`. But the invariant's phrasing — "no cross-context tag aggregation at the API layer" — could be read as a general design principle, not just a constraint on one function. If the proposed query surface adds cross-context operations as first-class concerns (discovery mode, federated queries, `shared_concepts` MCP exposure), this is in material tension with the spirit of Invariant 28, not merely its letter.

The essay resolves this tension by noting the exception already exists — but the exception (`shared_concepts`) is currently not MCP-exposed and therefore not "at the API layer." Exposing it via MCP changes its status.

**Recommendation:** Acknowledge that elevating `shared_concepts` to MCP-exposed status and adding cross-context discovery as first-class query modes constitutes a substantive revision of the design principle in Invariant 28, not just a clarifying amendment. The ADR that adds cross-context MCP query tools should explicitly supersede or amend the relevant part of Invariant 28 with reasoning.

---

### P2-E — Two-mode query model presented with more resolution than warranted

**Location:** §The Central Tension: Scope Versus Serendipity

**Claim:** "The resolution likely involves a two-mode query model..."

**Evidence gap:** The two-mode model is a proposed design without prior art in the comparison systems surveyed. The essay does not cite any existing system that implements scoped/discovery mode switching for consumer queries. The transition from "likely involves" to treating the escape hatch problem as "the most important design problem" happens in two paragraphs and overstates the model's confidence level. The design question is well-framed; the proposed resolution is reasonable; but it should remain a hypothesis, not a premise for downstream design.

**Recommendation:** Explicitly mark the two-mode model as a design proposal requiring validation in the DECIDE phase. Downstream ADRs that operationalize the query surface should treat it as a hypothesis under test, not an established design decision. The essay can propose it strongly; it should not present it as resolved.

---

## P3 Issues (Nice to Have)

### P3-A — "Serendipitous" carries implicit value judgment

**Location:** §The Central Tension heading and throughout

The word "serendipitous" characterizes cross-boundary discovery as desirable-but-accidental. This framing subtly shapes the design — "serendipity" suggests the consumer stumbles upon connections, which biases toward the "escape hatch" metaphor. An alternative framing — "cross-boundary discovery is a deliberate operation with a specific trigger" — might produce a different design. The tension is real; the framing of one side as "serendipitous" is an unstated choice.

**Recommendation:** Consider replacing "serendipitous cross-boundary discovery" with "intentional cross-context discovery" to keep both modes as deliberate operations rather than one being the default and the other being accidental. This is a terminology choice with design implications.

---

### P3-B — Terminology consistency: "extraction phases" vs "adapter phases"

**Location:** §Independent Contribution Accumulation, throughout

The essay alternates between "three-phase extraction pipeline (registration, structural analysis, semantic extraction)" (essay language) and "Phase 1/2/3" (which the domain model explicitly prohibits — domain model entry for Extraction Phase: "Always use the descriptive name... not 'Phase 1/2/3'"). The essay does not use the forbidden numerals, but it does refer to "three-phase pipeline" without naming the phases, while the domain model requires using the descriptive names exclusively. This is minor — the essay is not using the prohibited shorthand — but tighter alignment with the domain model terminology would be cleaner.

**Recommendation:** When referencing the extraction pipeline, name the phases: "registration, structural analysis, semantic extraction." Remove "three-phase pipeline" as a standalone phrase unless immediately followed by the phase names.

---

### P3-C — The projection/enrichment/query layering would benefit from a tighter definition

**Location:** §The Projection Layer, bullet list

The three-layer model — enrichment (write-side), projection (read-side view), query (parameterized operation) — is the essay's most novel structural contribution. It is presented clearly, but the relationship between projection and meta-context is left as "close to a projection but scoped to context union rather than subgraph selection." This creates ambiguity: is meta-context a degenerate case of projection, or a separate concept? If projection subsumes meta-context, Invariant 28's meta-context definition may need updating. If they are distinct, the boundary should be stated.

**Recommendation:** Add a sentence explicitly stating whether the proposed projection concept subsumes meta-context or is parallel to it. "Meta-context is a special case of projection where the filter is 'include all nodes and edges from these contexts'" vs. "Meta-context and projection are distinct concerns operating at different levels." The choice affects whether the domain model needs a new concept or an amendment to an existing one.

---

### P3-D — Evidence diversity ranking: computed how, at what cost?

**Location:** §Evidence Diversity as a Derived Signal; §What the Query Surface Must Contain, item 3

The essay establishes that evidence diversity is "derived at query time from provenance entries" (domain model) and proposes it as a ranking dimension. It does not address the cost of this computation. For a query over a large traversal result, computing evidence diversity for each returned edge requires traversing provenance entries per edge — potentially an O(N × P) operation where N is traversal size and P is provenance depth. Graphiti's reranking (Reciprocal Rank Fusion, Maximal Marginal Relevance) is acknowledged as a write-time analog; evidence diversity is not pre-computed.

This is not a fatal objection, but it is a design concern that should be flagged before implementation. The essay's claim that the escape hatch "must be lightweight enough that it does not require an LLM at query time" acknowledges this class of concern — but does not apply it to evidence diversity computation itself.

**Recommendation:** Add a note acknowledging that evidence diversity ranking at query time has a computational cost that grows with traversal depth and provenance density. Flag this as a potential concern for the DECIDE phase rather than leaving it implicit.

---

## Summary

The essay is structurally sound. The core argument — that Plexus's write-heavy architecture justifies a thin query surface, but that Plexus's specific architectural properties create four novel query requirements not addressed by comparison systems — follows validly from the research log findings and codebase analysis.

The most consequential issue (P1-B) is a gap between the composition claim about `shared_concepts` and what the current implementation can actually deliver. This matters because it will mislead the DECIDE phase about how much new query surface work is required to enable federated discovery. The `shared_concepts` function is a starting point, not a near-complete primitive for the queries described.

The second consequential issue (P1-A) is a borrowed credibility problem: MV4PG's 28-100x speedup figures are cited to motivate the projection concept, but the mechanism behind those numbers does not transfer to the design being proposed. This should be corrected before the essay is used to motivate a projection layer ADR.

The P2 issues are real but navigable: the federated discovery argument rests on thin evidentiary ground (one conversation); the enrichment-richness prerequisite is not established; the declarative-writes-implies-declarative-queries inference needs grounding; and the Invariant 28 tension is understated. None of these invalidate the essay's conclusions, but they leave the argument more vulnerable to challenge than it needs to be.

The P3 issues are minor clarifications that would tighten the language without changing the conclusions.

**Argument chains mapped:** 6
**Issues found:** 10 (2 P1, 4 P2, 4 P3)

**Downstream risk:** If these issues are not resolved before the DECIDE phase, the highest-risk outcome is building a query surface that assumes `shared_concepts` composes naturally with provenance filtering (it does not yet), or citing the projection concept with a performance rationale (MV4PG speedup) that the implementation cannot reproduce. Both would create a gap between what was promised and what was delivered.
