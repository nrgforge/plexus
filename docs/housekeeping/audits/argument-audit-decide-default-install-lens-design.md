# Argument Audit Report

**Audited document:** Five ADRs: `docs/decisions/038-release-binary-feature-profile.md`, `docs/decisions/039-created-at-property-contract.md`, `docs/decisions/040-discovery-gap-trigger-sources.md`, `docs/decisions/041-lens-grammar-conventions.md`, `docs/decisions/042-dimension-extensibility-guidance.md`
**Source material:** `docs/essays/reflections/field-notes.md`, `docs/housekeeping/spikes/spike-default-install-intent.md`, `docs/product-discovery.md`, `docs/domain-model.md`, `docs/cycle-status.md`, `docs/decisions/024-core-and-external-enrichment-architecture.md`, `docs/decisions/025-declarative-adapter-spec-extensions.md`, `docs/decisions/026-embedding-enrichment.md`, `docs/decisions/033-lens-declaration.md`, `docs/decisions/034-composable-query-filters.md`
**Date:** 2026-04-20

---

## Section 1: Argument Audit

### Summary

- **Argument chains mapped:** 14 (across five ADRs)
- **Issues found:** 8 (1 P1, 4 P2, 3 P3)

---

### P1 — Must Fix

**Issue 1.1**

- **Location:** ADR-038 §Context, final paragraph of Context; §Consequences Negative, first bullet. Also §Decision "Documentation per deployment class," bullet 1.
- **Claim:** ADR-038 Context states "three active (CoOccurrence + TemporalProximity after ADR-039 + DiscoveryGap when triggered)." The Consequences Negative bullet restates: "A user who reads about four core enrichments and installs via Homebrew without authoring a spec sees three active (CoOccurrence + TemporalProximity after ADR-039 + DiscoveryGap when triggered)." The Documentation section uses the same phrasing.
- **Evidence gap:** This counting is inconsistent with ADR-040's own conclusion. ADR-040 §Consequences Negative explicitly states: "Without a consumer-authored spec that produces `similar_to` (and without the `features = ["embeddings"]` in-process path), DiscoveryGap does not fire in the default build." DiscoveryGap is idle without a trigger producer, which does not exist in the default Homebrew binary by ADR-038's own decision. Calling DiscoveryGap "active (when triggered)" in the count of three active enrichments overstates the default-build experience: the trigger is never present in the default build unless the consumer authors a spec. The default build truthfully delivers two active enrichments (CoOccurrence always, TemporalProximity after ADR-039), not three. Counting DiscoveryGap as the third "active" enrichment misleads documentation readers and undermines ADR-038's stated goal of honesty about the lean baseline.
- **Recommendation:** Replace "three active" with "two active by default (CoOccurrence + TemporalProximity after ADR-039); DiscoveryGap registered but idle until a consumer-authored spec provides a `similar_to` producer." The Documentation section's deployment-class bullet should match. This is a must-fix because documentation language is ADR-038's primary deliverable and the miscounting directly contradicts the ADR-040 dependency.

---

### P2 — Should Fix

**Issue 2.1**

- **Location:** ADR-038 §Decision "No Rust code path to llm-orc for embedding," rationale bullet 1.
- **Claim:** "A Rust `LlmOrcEmbedder` would be a second code path (the first being the declarative spec's ensemble invocation) doing the same infrastructure work. Two code paths to the same external dependency is the parallel-code-paths anti-pattern..."
- **Evidence gap:** The parallel-code-paths argument is stated as settled, but it is asserted without demonstrating that the two paths are actually equivalent in capability. The rationale then adds in bullet 2 that "the declarative path is strictly more capable" — which is the stronger, self-sufficient argument. The parallel-code-paths framing as an anti-pattern is an unstated assumption: it treats "two code paths to the same dependency" as categorically bad, but many systems have exactly this (e.g., a library that can be called directly or through a higher-level abstraction). The real argument is the second bullet (consumer parameterization, independent versioning) — that one is grounded and sufficient. The parallel-code-paths premise as stated requires the implicit claim that capability equivalence holds, which is not shown. A reader who doubts the anti-pattern framing would not be persuaded by bullet 1 alone.
- **Recommendation:** Reorder the rationale bullets or soften bullet 1 to note that the anti-pattern claim holds only given the capability argument in bullet 2: "A `LlmOrcEmbedder` would add a second code path to llm-orc for a capability already available — and more flexibly — through the declarative path (bullet 2)." This keeps the reasoning accurate without depending on the contested framing.

**Issue 2.2**

- **Location:** ADR-041 §Analytical comparison — "What the analytical walk-through does NOT show" and §Decision "The phenomenology-of-discovery claim is named as hypothesis."
- **Claim:** The ADR correctly notes that the analytical comparison does not demonstrate that Spec B produces a discovery where Spec A does not, and that the phenomenological claim remains a hypothesis. However, in the preceding walk-through section — specifically "What differs between A and B" — the language under "Query expression reach" moves beyond structural description into implied phenomenological advantage: "A query against `lens:trellis:thematic_connection` asks 'what is thematically connected?' — a question pre-answered by the lens author's naming." The word "pre-answered" carries an implicit value judgment (pre-answering is worse than not answering), which is precisely the phenomenological claim the ADR elsewhere characterizes as hypothesis.
- **Evidence gap:** The analytical walk-through is supposed to show *structural differences* that are hypothesis-neutral; "pre-answered" is not structurally neutral. The cycle-status Uncertainty 1 explicitly distinguishes the value-proposition layer (settled) from the lens-grammar layer (hypothesis). The "pre-answered" framing conflates them. The ADR's own disclaimer that "the analytical comparison does not demonstrate that Spec B produces a discovery where Spec A does not" is undercut by the walk-through using language that implies exactly this.
- **Recommendation:** Replace "pre-answered" with neutral language: "A query against `lens:trellis:thematic_connection` names a semantic interpretation chosen by the lens author; the consumer's query-time language inherits that choice." This is descriptively accurate and hypothesis-neutral. The convention endorsement can then rely on the composition-shape argument (extension vocabulary) where the ADR's support is genuinely non-hypothetical.

**Issue 2.3**

- **Location:** ADR-042 §Decision "What 'documentation-only' requires," and §Consequences Negative.
- **Claim:** "The 'documentation' lever is substantive, not nominal." The ADR lists four explicit documentation deliverables and states "BUILD should not consider the ADR landed without them."
- **Evidence gap:** The ADR does not name any mechanism by which BUILD can verify the documentation lever is substantive after the fact. The Consequences Negative section acknowledges: "Documentation is harder to verify than validation. A future drift where documentation falls out of sync with shipped-adapter conventions is a foreseeable risk." The ADR names ORIENTATION and system-design updates as cross-checks and "periodic conformance-audit runs (ADR-?)" as backstop — but no ADR number exists, the conformance-audit backstop is an open reference to a non-existent decision, and no concrete verification step is described for the BUILD deliverable gate itself.

  This is a hidden assumption: the claim that "documentation-only" is substantive-not-nominal depends on a verification process that is not yet defined. As written, the ADR can land with documentation deliverables committed but not verified against their stated purpose (whether they actually guide spec authors who don't read carefully). The "substantive" commitment is an intent claim, not a verifiable constraint.
- **Recommendation:** Either (a) add a concrete verification criterion to the BUILD deliverables list (e.g., "the minimum-useful-spec worked example must be verified by loading it via `load_spec` and confirming the dimension guidance is visible at the spec-author's first decision point") or (b) acknowledge explicitly that verification of the documentation lever's effectiveness is deferred to a future cycle that can observe real spec-author behavior. The current "substantive, not nominal" framing oversells what the ADR can actually commit to at DECIDE time.

**Issue 2.4**

- **Location:** ADR-040 §Decision "Multiple DiscoveryGap instances are allowed," referencing "Invariant 39 (enrichment deduplication by `id()`)."
- **Claim:** "Invariant 39 ensures each parameterization is registered once per pipeline."
- **Evidence gap:** ADR-040 does not state how parameterization affects `id()` uniqueness. The domain model (Invariant 39) says enrichments are deduplicated by `id()`. ADR-024's `DiscoveryGapEnrichment` struct has a dedicated `id: String` field. If `id()` is derived from `trigger_relationship`, two instances parameterized with the same trigger would share an ID and be deduplicated to one. If `id()` is derived from the full parameter set, instances with the same trigger but different output relationships would be distinct — which is the desired behavior. The ADR doesn't specify which of these holds and simply asserts the invariant "ensures each parameterization is registered once." This is a logical gap: the invariant guarantees uniqueness for a given ID, but whether different parameterizations produce different IDs depends on the ID-construction scheme, which is not stated here.
- **Recommendation:** Add one sentence specifying the ID convention for parameterized DiscoveryGap instances. ADR-024's ID format is `discovery_gap:{trigger}:{output}` — if that's the convention, stating it here closes the gap and the deduplication claim follows correctly.

---

### P3 — Consider

**Issue 3.1**

- **Location:** ADR-038 §Consequences Negative, third bullet.
- **Claim:** "BUILD must produce a worked-example spec (or reference implementation) for the documented activation path."
- **Note:** This is stated as a BUILD requirement inside the Consequences section, but it is not listed as an explicit deliverable in the Documentation section that precedes it. The Documentation section names four deployment-class descriptions but does not mention the worked-example spec as a named deliverable with a clear artifact location. A BUILD implementer reading only the Decision section would see the documentation framing but might not find the worked-example requirement. Consider making the worked-example spec a named artifact in the Documentation decision bullet ("point to a worked example spec at `[path]`") rather than leaving it in Consequences.

**Issue 3.2**

- **Location:** ADR-039 §Decision "Spec authors using `TemporalProximityEnrichment` must write the declared property."
- **Claim:** "The spec validator (Invariant 60) does not statically verify this coupling — there is no cross-primitive validation that a `create_node` emits a property that a declared enrichment reads. This is a known limitation."
- **Note:** The ADR acknowledges this as a known limitation and names documentation as the mitigation. It also says "a future ADR could add cross-primitive validation." This is appropriate scoping, but the ADR does not name what a failure looks like for spec authors — they get a silently-idle enrichment with no error. The ADR calls this "graceful degradation" elsewhere, but calling a silently dead enrichment "graceful" in a spec-authoring context may mislabel it. The naming is consistent with the general enrichment-loop contract (an enrichment that cannot fire emits nothing, not an error), but its application here — where the failure mode is entirely the spec author's omission — might deserve a stronger label in the documentation deliverable ("silent-idle state; check your property contract").

**Issue 3.3**

- **Location:** ADR-041 §Consequences Neutral, final bullet.
- **Claim:** "A future cycle on lens-as-grammar may amend this ADR's 'convention, not requirement' stance if evidence accumulates."
- **Note:** The ADR does not name what evidence would constitute a sufficient basis for promoting the convention to a requirement. The cycle-status (Hypotheses Parked for Future Cycles §Lens-as-grammar) gives a precondition: real emergent graph content from untagged prose with active semantic extraction or embeddings, not pre-tagged demonstrations. The ADR could carry this precondition forward explicitly so the future cycle has a legible basis for its opening framing. As written, "if evidence accumulates" is underspecified — any evidence could be argued to satisfy or not satisfy the condition.

---

## Section 2: Framing Audit

The framing audit compares what the source material made available against what the ADRs chose to foreground.

---

### Question 1: What alternative framings did the evidence support?

**Alternative A — ADR-038: "Honest default baseline" framing vs. "Lean default plus documented upgrade path" framing**

ADR-038 chose to frame its decision as producing a "positive decision, not a defect-by-omission" by explicitly endorsing the lean baseline. The Spike 2 evidence supported an alternative framing: the lean baseline could remain a defect-by-omission, remediated by a staged-onboarding design that names the progression explicitly as an onboarding *product feature* (Homebrew lite → llm-orc-adjacent → features=embeddings → library). Spike 2 itself says: "staged-onboarding framing is a plausible *remediation shape*, not a plausible description of current state. A team could *decide* to adopt staged onboarding, document it [...] and the state would then match the framing."

What would a reader need to believe for this alternative framing to be right? They would need to believe that the staged-onboarding pattern is a genuine product affordance (not just a workaround), and that naming the progression paths as "lite" / "full" / "library" is a meaningful product differentiation rather than a documentation convenience. Under this framing, the ADR's deliverable would be a product-tier design, not just documentation — which is a larger commitment.

ADR-038 explicitly declined this framing in favor of the honest-baseline framing. The evidence from Spike 2 supports the ADR's choice, but the alternative is not engaged directly. The rejection is implicit (the word "staged-onboarding" does not appear in ADR-038 at all).

**Alternative B — ADR-041: "Named vs. structural predicates" framing vs. "When does vocabulary register matter at all" framing**

The field notes §"Crawl-step results and the tautology threshold" contains a finding that the ADR does not use as a framing lens: the tautology threshold observation. The note identifies that the first crawl demonstrated mechanism but not value — any lens output from a tautological source (user-supplied tags reflected back) delivers nothing Plexus didn't start with. Under this alternative framing, the prior question to "which naming convention" would be "whether the lens has a non-tautological source at all." A structural predicate applied to co-occurrence over user-supplied tags is as tautological as a named relationship applied to the same input. The framing question becomes "when does vocabulary register add value beyond zero?" rather than "which register adds more value?"

Under this framing, the ADR's endorsement of structural predicates would be conditional on the underlying source having crossed the tautology threshold — semantic extraction or embedding, not just co-occurrence over tags. The ADR's composition-shape argument (structural predicates extend vocabulary better) would remain valid, but the scope condition for "discovery-oriented jobs" would include a prerequisite about source quality that the ADR does not name.

What would a reader need to believe for this framing to be right? They would need to believe that the named-vs-structural distinction is second-order relative to source-quality, and that the convention is only meaningfully actionable after the tautology threshold is crossed. This is a more demanding but arguably more honest framing for a convention targeting discovery value.

**Alternative C — ADR-042: "Dimension guidance for spec authors" framing vs. "Dimension as first interaction with Plexus's architecture" framing**

The PLAY field notes Finding 3 observed that the dimension mismatch was not discovered until live inspection of query behavior. An alternative framing would treat dimension choice not as a spec-authoring detail to guide through documentation, but as the first architectural orientation moment a new spec author has — the choice where they encounter Plexus's core concept that the graph is multidimensional and queries are dimension-scoped. Under this framing, the decision would be about the onboarding *entry point* to Plexus's dimensional model, not about validation strictness. The ADR as written gives a documentation-only answer; the alternative framing might have foregrounded the quickstart experience design as the resolution rather than the spec validator.

---

### Question 2: What truths were available but not featured?

**Finding A: The tautology threshold and its implications for worked examples**

The field notes §"Crawl-step results and the tautology threshold" contains the most consequential practical finding of the PLAY session: the minimum-useful Plexus setup (for any real value) requires crossing the tautology threshold, which is not crossed by the default Homebrew build with user-supplied tags. The finding has three explicit implications listed in the field notes (§"Implications"):

1. The onboarding path needs to make the tautology threshold visible.
2. The minimum value-demonstrating setup is not the minimum mechanism setup.
3. The "simplest thing that works" for Plexus is not a good demo.

None of these three implications appear in any of the five ADRs. ADR-038's §Consequences Negative mentions "the easy-to-demo vs honest-to-demo tension resolves toward honest-to-demo," but this is a passing reference to a value tension in product discovery, not an engagement with the tautology threshold as a structural constraint on what the worked-example spec must demonstrate.

ADR-038 specifies that BUILD must produce "a worked example spec" as part of the activation path for embedding. Whether that worked example must also cross the tautology threshold — produce structure the user did not supply — is not stated. A worked example that uses pre-tagged content to demonstrate the embedding activation path would repeat the tautology the field notes explicitly warned against.

Why this matters: the worked-example spec is the primary consumer-facing artifact this ADR produces. If it demonstrates mechanism rather than value, it perpetuates the exact documentation-actuality gap the cycle is trying to close.

**Finding B: The feedback loop as an evaluative criterion for the query surface**

The field notes §"Indeterminacy, delayed recognition, and the writer's journey" ends with a concrete evaluative criterion: "can a consumer compose a query that distinguishes 'things that were related when I added them' from 'things that became related after I added them'?" This is an open design question about the current query surface that none of the five ADRs address.

This finding is tagged in the field notes as "something to notice as the specs get authored and tested" — it was explicitly not resolved. It appears in product-discovery as one of the hypothesis-level mental model extensions. But it has direct bearing on ADR-039 (the TemporalProximity fix), which is specifically about temporal structure in the graph. ADR-039 fixes the property-contract bug and correctly describes the enrichment's graceful-degradation contract, but does not note the temporal query dimension that the field notes identified as potentially load-bearing for the value proposition.

This is an underrepresentation, not an error — ADR-039 is scoped to the property-contract fix, not the temporal query surface design. But the framing omission means the ADR treats TemporalProximity as a structural-availability fix without naming its connection to a load-bearing query affordance.

**Finding C: The "apps as lenses, not containers" architectural implication for consumer onboarding**

The field notes §"Writing emerges between applications" contains the design posture observation: "consumer apps are lenses on shared material, not containers for their own material." This has direct bearing on ADR-042 (dimension guidance), because dimension choice is precisely where the "app as container" vs "app as lens" distinction becomes visible in spec-authoring practice. A spec author who thinks their app owns its dimension (e.g., always putting fragments in `semantic` because "that's my app's dimension") is operating with the container mental model. The dimension guidance in ADR-042 addresses the technical consequence (mismatch behavior) but does not address the underlying mental model that produces the mismatch.

The field notes' observation appeared in the cycle routing as a "stakeholder-model refinement — hold as characterization, not definition." Product discovery carries it as a characterization but hedges it appropriately. ADR-042 could have used it as a framing lens for *why* dimension mismatches are a predictable onboarding failure mode (container mental model applied to lens-oriented architecture), making the documentation guidance more diagnostic. Instead the ADR frames the issue as spec-author carelessness rather than a predictable mental-model collision.

---

### Question 3: What would change if the dominant framing were inverted?

**ADR-038 dominant framing:** The lean default binary is now a positive decision, not a defect-by-omission.

**Inverted framing:** The lean default binary remains a defect, and the documentation commitment is the mitigation, not the resolution.

Under the inverted framing:
- The claim "this is now a positive decision" would need to be replaced with "this is an acknowledged limitation with documented mitigation." The architecture documentation would name the gap between what the binary delivers and what the engine supports rather than presenting the lean baseline as intentional design.
- The worked-example spec would become a defect mitigation artifact (here is how to make the binary usable) rather than an onboarding feature (here is how to extend the lean binary's capabilities).
- The Consequences Negative section would be the primary narrative, not the subordinate one.
- The claim of "honest-to-demo at the default-install layer" becomes contested: the honest representation of the default binary might be "this binary currently demonstrates one working enrichment; everything else requires consumer-side setup" — which is closer to the PLAY field notes' Finding 4 framing than to the ADR's framing.

What the inverted framing reveals: ADR-038 resolves the defect-by-omission label by reframing the lack as intent. This is legitimate, but it works only if the documentation commitment is strong enough to make the lean baseline genuinely discoverable before a consumer experiences the gap. The ADR's Consequences section relies on documentation it does not yet produce (the onboarding material, the worked example, the README updates). If those deliverables are weak or delayed, the reframing does not hold — the defect label reasserts itself.

**ADR-041 dominant framing:** Structural predicates are the endorsed convention for discovery-oriented jobs.

**Inverted framing:** Named relationships are the natural default; structural predicates require justification.

Under the inverted framing:
- The burden of proof shifts. Rather than "spec authors should consider structural predicates for discovery-oriented jobs," the convention would read "spec authors should consider named relationships as the natural default; structural predicates are appropriate when the consuming app explicitly does not surface the relationship name to human readers."
- The composition-shape argument (structural predicates extend more naturally) becomes weaker against the inverted framing: named relationships extend just as naturally once the vocabulary is established; the composition-shape difference only matters on the first extension, not in a mature spec.
- The phenomenological claim (structural predicates preserve the discovery experience) becomes the only differentiating argument — and it is explicitly a hypothesis. Under the inverted framing, the hypothesis is more exposed: the convention would need the phenomenological claim to hold, but the ADR has declined to validate it.
- ADR-041's choice to ground the endorsement in composition-shape reasoning rather than the phenomenological claim looks more precarious under the inverted framing: if composition-shape is the actual argument, the convention should be stated as "structural predicates produce more extensible lens vocabularies" rather than as a convention for "discovery-oriented jobs."

---

### Framing Issues

**P2 — Underrepresented alternatives**

**FI-1**

- **Location:** ADR-038 §Decision "Documentation per deployment class."
- **Claim:** The lean-baseline decision is honest-to-demo once documentation is in place.
- **Framing gap:** The staged-onboarding alternative framing — where the lean/full distinction is a product-tier design decision, not just a documentation stance — is visible in Spike 2 but is not named or rejected in ADR-038. The alternative is not necessarily better (it would require a product-tier design decision outside this cycle's scope), but dismissing it by omission means the ADR's framing choice is invisible. The ADR reads as if there were no alternative framing available, when Spike 2 explicitly named one.
- **Recommendation:** Add a brief "alternatives considered" note in ADR-038 §Consequences Neutral acknowledging that a product-tier (lite/full) framing was available and naming why this cycle chose documentation-only over that path.

**FI-2**

- **Location:** ADR-041 §Analytical comparison, both spec examples.
- **Claim:** The walk-through demonstrates the structural differences between named-relationship and structural-predicate grammars.
- **Framing gap:** Both examples use `may_be_related` (co-occurrence over presumably hand-tagged overlaps) as the source. This is precisely the tautological source the field notes §"Crawl-step results" identified as demonstrating mechanism but not value. A walk-through using a tautological source cannot demonstrate that either grammar produces meaningful discovery signal — it can only show how they differ structurally. The walk-through is honest about this ("does not demonstrate that Spec B 'produces a discovery'") but does not name that the source choice (may_be_related from co-occurrence) means neither grammar is in the value-delivering range for a writer-facing discovery app. The analytical comparison thus demonstrates the grammar distinction against a source that doesn't cross the tautology threshold for either grammar, leaving the practical relevance of the comparison underexplored for the primary use case (creative-writing discovery).
- **Recommendation:** Add a note in the walk-through acknowledging that the illustrative case uses co-occurrence-derived edges; the grammar distinction becomes more consequential when the source crosses the tautology threshold (semantic extraction, embedding similarity) because the latent connections are ones the user did not supply. This does not require adding a second example; one sentence anchoring the practical scope condition is sufficient.

**P3 — Minor framing choices**

**FI-3**

- **Location:** ADR-040 §Consequences Negative, second paragraph.
- **Claim:** "Users expecting DiscoveryGap to detect structural-absence patterns broadly (not only latent-vs-structural disagreement) will not find that here."
- **Framing gap:** The rejection of algorithm broadening is presented as a one-sentence observation in Consequences, not as an engaged counterargument in the Decision section. ADR-040's Decision §"No algorithm broadening" gives three rationale bullets, but the Consequences section adds this without returning to the decision structure. The counterargument against algorithm broadening — that co-occurrence-based gap detection is a different algorithm using the same terminology — is load-bearing for ADR-024 alignment, and the ADR gives it appropriate weight in the Decision section. The Consequences section repeating the rejection as a negative without the reasoning may leave readers who read Consequences before Decision with an unexplained limitation.
- **Recommendation:** Minor: cross-reference "No algorithm broadening" from this Consequences bullet ("see Decision §'No algorithm broadening' for the full rationale"). This is cosmetic but improves navigability.
