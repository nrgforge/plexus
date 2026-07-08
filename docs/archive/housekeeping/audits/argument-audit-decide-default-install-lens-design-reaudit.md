# Argument Audit Re-Audit Report

**Audited documents:** `docs/decisions/038-release-binary-feature-profile.md`, `docs/decisions/039-created-at-property-contract.md`, `docs/decisions/040-discovery-gap-trigger-sources.md`, `docs/decisions/041-lens-grammar-conventions.md`, `docs/decisions/042-dimension-extensibility-guidance.md`
**Prior audit:** `docs/housekeeping/audits/argument-audit-decide-default-install-lens-design.md`
**Source material:** same as prior audit
**Date:** 2026-04-20

---

## Section 1 — Status of Prior Findings

### Issue 1.1 (P1) — ADR-038 enrichment-count contradiction

**Closed.**

The revision is thorough. ADR-038 now counts correctly in every location the prior audit flagged:

- The "Plexus does not break without llm-orc or a consumer-authored spec" section (the honest-lean-baseline enumeration at lines 82–88) names CoOccurrence and TemporalProximity as the two active enrichments, names DiscoveryGap as not firing, and names EmbeddingSimilarity as absent from the build.
- The Documentation section's first bullet reads "Two enrichments active by default" and says "DiscoveryGap is registered but idle."
- The closing summary sentence at the end of the Documentation section repeats the corrected count consistently: "two active by default in the distributed binary; DiscoveryGap registered but idle until a `similar_to` producer is active."
- Consequences Negative reads "two active by default (CoOccurrence always; TemporalProximity after ADR-039)" with DiscoveryGap and EmbeddingSimilarity correctly distinguished.

The correction is uniform across all previously-problematic locations, and no new miscounting was introduced.

---

### Issue 2.1 (P2) — ADR-038 parallel-code-paths rationale bullet ordering

**Closed.**

The revision inverted the order and conditionalized the parallel-code-paths framing. The rationale now leads with the capability-asymmetry argument ("The declarative path is strictly more capable") and derives the parallel-code-paths concern from it as a consequence: "Given the capability asymmetry above, a Rust `LlmOrcEmbedder` would be a second code path... for strictly less flexibility." The framing is now explicitly grounded in the preceding premise rather than asserted as a free-standing anti-pattern claim. The third bullet adds clean provenance (DISCOVER gate conversation) for where the constraint originated.

---

### Issue 2.2 (P2) — ADR-041 "pre-answered" language

**Closed.**

The "pre-answered" language has been replaced throughout the analytical walk-through. The revised text in "Query expression reach" reads: "A query against `lens:trellis:thematic_connection` names a semantic interpretation chosen by the lens author; the consumer's query-time language inherits that choice." This is structurally descriptive and hypothesis-neutral. The same paragraph continues: "A query against `lens:trellis:latent_pair` names a structural descriptor; the consumer's query-time language inherits a shape descriptor instead, and any semantic interpretation is supplied downstream... The second query shape is what the field-notes called the *shape-of-the-graph query*; whether it is better suited for interpretive jobs than the first shape is the hypothesis this ADR carries (see below), not a conclusion the analytical comparison establishes."

The revision does the work correctly — the phenomenological claim is cleanly deferred while the structural description of difference stands on its own.

---

### Issue 2.3 (P2) — ADR-042 documentation-lever verification

**Closed.**

The prior version claimed "documentation is substantive, not nominal" without a verification mechanism. The revised Decision section ("What 'documentation-only' requires") now commits to four named BUILD deliverables and adds a specific landing criterion:

> A spec author authoring a declarative spec for a node type that collides with a shipped-adapter node type (e.g., `fragment`) must encounter the dimension-choice guidance *before* declaring the node's dimension. This is verified at BUILD by walking the onboarding path literally — opening the spec-author documentation, reaching the `create_node` primitive docs, and confirming the dimension-choice section is reachable in one navigation hop from the first spec-authoring reference.

The ADR also now explicitly labels its empirical limitation: "the claim 'documentation is substantive, not nominal' is an intent claim with deferred verification." The revision distinguishes what the ADR can commit to at DECIDE time from what requires future observation. This is the correct epistemic posture and closes the hidden assumption.

---

### Issue 2.4 (P2) — ADR-040 DiscoveryGap ID format unspecified

**Closed.**

The revised "Multiple DiscoveryGap instances are allowed" section now states the ID convention explicitly: "Each parameterization produces a distinct `id()` per ADR-024's convention (`discovery_gap:{trigger}:{output}`)." The deduplication claim now follows from a stated premise rather than being asserted. The example instances (one with `trigger_relationship: "similar_to"`, one with `trigger_relationship: "embedding:mistral:similar_to"`) illustrate that different trigger values produce distinct IDs, making the Invariant-39 deduplication argument self-consistent.

---

### Issue 3.1 (P3) — ADR-038 worked-example spec buried in Consequences

**Closed.**

The worked-example spec is now a named deliverable inside the Documentation section's decision bullet ("How to activate embedding in the default build"), committed at `examples/specs/embedding-activation.yaml` with a specific quality bar (must cross the tautology threshold — must produce `similar_to` edges on content the author did not pre-encode with overlapping tags). It also appears in Consequences Negative as a BUILD obligation. The deliverable is now reachable from the Decision section; a BUILD implementer will find it without hunting through Consequences.

---

### Issue 3.2 (P3) — ADR-039 "graceful degradation" labeling silent-idle for spec authors

**Closed.**

The revised spec-author section now reads: "Failure mode for spec authors: a mismatch produces a **silent-idle enrichment**... Nothing in the runtime surfaces this to the author. Spec-author documentation must name this failure mode explicitly with that label ('silent-idle') and include a diagnostic checklist ('if your declared `TemporalProximityEnrichment` never produces edges, verify the nodes you emit carry the property key you declared')." The revision also explains what makes the "graceful degradation" label honest: it applies to the enrichment-loop contract (enrichments that cannot react emit nothing, not an error), while "silent-idle" is the label appropriate for the spec-authoring context where the cause is the author's omission. The tension between the two labels is resolved by distinguishing the framework-contract use from the spec-author-documentation use.

---

### Issue 3.3 (P3) — ADR-041 "if evidence accumulates" underspecified

**Closed.**

The Consequences Neutral section now states the precondition explicitly: "real emergent graph content produced by untagged-prose ingestion with active semantic extraction or embeddings — structure the writer did not pre-encode — ideally observed with a non-builder stakeholder inhabiting the Consumer Application Developer role during PLAY. Evidence short of that precondition (e.g., more practitioner-led analytical walk-throughs on co-occurrence over hand-tagged content) does not qualify, because the prior cycle's phenomenological observations were already from that shape of evidence." The future cycle has a legible opening condition; "if evidence accumulates" is no longer doing the work alone.

---

## Section 2 — New Argument-Audit Issues

No new P1 or P2 issues were introduced by the revisions.

One minor P3 observation:

### P3 — Consider

**Issue R.3.1**

- **Location:** ADR-040 §Multiple DiscoveryGap instances, the sentence beginning "two specs attempting to register the same parameterization..."
- **Claim:** "two specs attempting to register the same parameterization converge on one registration rather than colliding."
- **Note:** The ADR does not describe what "converge on one registration" means mechanically — whether the second `load_spec` call silently succeeds (skipping re-registration) or actively replaces the existing enrichment. This matters in the error-path case (first spec has been loaded, second spec with identical parameterization is loaded) but is not load-bearing for the default-install scenarios this ADR addresses. The prior P2.4 fix introduced this sentence as the deduplication-outcome description; the mechanism it implies (idempotent skip vs. last-writer-wins) is something Invariant 39 describes but the ADR does not cite at this resolution. Whether this warrants a BUILD clarification is low stakes, but if the Invariant 39 semantics are last-writer-wins rather than idempotent-skip, the word "converge" could mislead a spec author who is debugging duplicate-registration behavior.
- **Recommendation:** If Invariant 39's deduplication is idempotent (second registration of the same ID is a no-op), say "the second registration is a no-op." If it is last-writer-wins, say "the second registration replaces the first." Either way the current "converge on one registration" phrasing is under-specified for a spec author who encounters this case.

---

## Section 3 — ADR-038 / ADR-040 Consistency Check

**Fully consistent. The P1 contradiction is closed.**

The two ADRs now tell a single coherent story about default-build enrichment behavior:

**ADR-038** names the default-build enrichment state as: CoOccurrence (always active), TemporalProximity (active after ADR-039 property-contract fix), DiscoveryGap (registered but idle — no `similar_to` producer in default build), EmbeddingSimilarity (not registered in default build). The Documentation section states this as "two active by default." The activation path for DiscoveryGap is explicitly named as consumer-side: a declarative adapter spec declaring an llm-orc-backed embedding enrichment that emits `similar_to` edges.

**ADR-040** §Consequences Negative reads: "Without a consumer-authored spec that produces `similar_to` (and without the `features = ["embeddings"]` in-process path), DiscoveryGap does not fire in the default build." The Documentation sub-section in ADR-040 states: "In the default Homebrew build, there is no built-in producer of `similar_to` — the lean baseline is CoOccurrence-only." (Note: "CoOccurrence-only" is slightly under-inclusive relative to ADR-038's two-enrichment count, since TemporalProximity is also active after ADR-039, but this is a documentation-emphasis choice in ADR-040's DiscoveryGap-specific narrative rather than a contradiction. The full count is accurately given in ADR-038; ADR-040's context is specifically DiscoveryGap trigger availability.)

No count mismatches remain between the two ADRs on the question that generated the P1 finding: whether DiscoveryGap is meaningfully "active" in the default build. Both ADRs now agree it is registered but idle, and both name the same consumer-side activation path.
