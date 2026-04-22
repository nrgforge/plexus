# Susceptibility Snapshot

**Phase evaluated:** DECIDE — Default-Install Experience and Lens Design Principles cycle
**Artifact produced:** ADRs 038–042, scenarios 038–042, interaction-specs updates (2026-04-20)
**Date:** 2026-04-20
**Prior snapshots:**
- `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-model.md` (MODEL, same cycle)
- `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-discover.md` (DISCOVER, same cycle)

---

## Observed Signals

| Signal | Strength | Trajectory | Notes |
|--------|----------|------------|-------|
| Assertion density | Ambiguous | Declining from prior snapshot | One high-confidence user assertion reshaped the ADR set (the LlmOrcEmbedder correction on ADR-038). Outside that event, user assertions were sparse — "Yes" approvals after audit, no interrogation of individual ADR framings. Artifact-level assertion density is moderate and documented rather than asserted as conclusions. |
| Solution-space narrowing | Clear (one specific instance) | Stable to new-signal | The user's correction collapsed ADR-038 from two possible shapes (Rust-side embedder vs. consumer-declared spec) to one. This is appropriate narrowing grounded in architectural principle. However, the intermediate possibility within that narrowing — whether a Plexus-bundled default spec (shipped with the binary to activate embedding without user authoring) could be the activation mechanism — was not staged or examined. Both the added-Rust-path and the consumer-must-author-spec framings appeared; the bundled-default-spec option did not surface. |
| Framing adoption | Clear | Stable — consistent with cross-phase pattern | The user's ADR-038 correction framing ("llm-orc should not be enshrined in Rust beyond the adapter spec; the embedding strategy can be declared like anything else in the adapter spec") was adopted immediately and propagated to ADR-040 without independent re-examination at ADR-040's own driver level. The same swift-adoption pattern recorded in PLAY, DISCOVER, and MODEL recurs here as the fourth consecutive instance. |
| Confidence markers | Ambiguous | Stable from prior snapshot | User's ADR-038 correction was stated without hedging. This may be earned confidence (user designed the system). The post-correction ADR draft accepted the framing and added supporting rationale without testing whether the framing is internally consistent at the boundary it creates (DiscoveryGap's idle state in the default build is named as "expected behavior, not a bug," which is a confident reframing of what PLAY called a silent trap). |
| Alternative engagement | Ambiguous | Slightly declined from prior snapshot | The argument audit performed by the in-context agent was substantive and produced genuine corrections (P1 count error, P2 language neutralization, P2 rationale ordering). Framing issues (FI-1, FI-2, FI-3) were surfaced but not applied — they are being routed to the gate per skill protocol. The ADR-041 analytical walk-through used co-occurrence edges as the illustrative source; FI-2 specifically flagged this as tautological for the primary use case. The user did not raise this during drafting; the audit raised it; it was not addressed in the revision pass. |
| Embedded conclusions | Clear (two instances) | New instances relative to prior snapshot | (1) ADR-040's framing ("consumer-declared external enrichment is the activation path for default-build DiscoveryGap triggers") arrived via mechanical propagation from ADR-038's corrected framing, not from independent examination against ADR-040's own drivers (Spike 2 Observation A, ADR-024). (2) ADR-038's "this is now a positive decision, not a defect-by-omission" reframing is a confident conclusion embedded at artifact-production time that the framing audit's inversion analysis (argument-audit §"What would change if the dominant framing were inverted?") identifies as contingent on documentation deliverables that do not yet exist. |

---

## Interpretation

### What the signals collectively suggest

DECIDE produced a five-ADR set that is internally consistent, has clean provenance sections, and survived a rigorous argument audit with all P1 and P2 issues corrected. The re-audit found no new issues. This is a meaningfully better artifact quality than the pre-audit draft represented. The compound defense (argument audit + conformance scan) functioned as designed: it caught the enrichment-count contradiction that would have propagated into onboarding documentation as the primary consumer-facing claim.

Two patterns warrant careful evaluation at this gate boundary.

**Pattern 1 — Cross-ADR mechanical propagation without independent driver examination (ADR-040).**

ADR-038's corrected framing ("no Rust code path to llm-orc; consumer-declared activation via spec") propagated to ADR-040 during the same drafting session, by the same agent. ADR-040's provenance section lists its own drivers (Spike 2 Observation A, PLAY Finding 1, ADR-024) and the decision framings trace to those drivers independently. On close reading, the drivers do support the propagated framing: Spike 2 Observation A explicitly names the trigger-coupling problem and the consumer-spec activation path; ADR-024's "four core enrichments, each a crisp algorithm" posture independently supports the "no algorithm broadening" decision. The propagation is mechanically traceable and the downstream ADR's drivers are adequate on their own.

The susceptibility concern is narrower: ADR-040 did not examine whether the "consumer-declared external enrichment is the activation path" framing is the *only* resolution consistent with its own driver chain. A plausible alternative — bundling a default spec with the Homebrew binary that provides the llm-orc ensemble activation without requiring users to author one — is consistent with ADR-024's external-enrichment path and with ADR-040's "source-agnostic trigger" principle. That alternative would change the friction profile (user installs llm-orc and configures a provider, but does not have to author a spec) without changing the activation mechanism. ADR-040's text does not name this as a considered and rejected option; it does not appear in ADR-038's Consequences either. The option was invisible in both ADRs, not rejected on record.

Whether this gap is consequential depends on whether ARCHITECT or BUILD will encounter it as a decision point. If "what ships with the binary" is scoped to the current five ADRs, the bundled-spec option remains invisible to BUILD and the gap persists. If BUILD naturally asks "should we ship a reference spec alongside the worked-example spec," the gap will be noticed then — but without the option having been staged and rejected in DECIDE, the BUILD-time decision will happen without the benefit of the driver chain.

**Pattern 2 — "Positive decision, not a defect-by-omission" reframing is load-bearing but unverified at artifact-production time (ADR-038).**

ADR-038's dominant framing — that the lean default binary is "now a positive decision, not a defect-by-omission" — is explicitly contingent on documentation deliverables (README updates, worked-example spec, onboarding material) that do not exist at DECIDE time. The argument audit's inversion analysis (§"What would change if the dominant framing were inverted?") makes this explicit: under the inverted framing, the lean binary remains a defect, and documentation is a mitigation, not a resolution. The ADR reads as if the reframing is completed at DECIDE time; it is actually completed only when BUILD lands the documentation deliverables. If those deliverables are weak, delayed, or scoped out, the defect-by-omission label reasserts itself and ADR-038's core claim becomes false.

This is not a framing error — the ADR explicitly names this risk in Consequences Negative. The susceptibility concern is that the reframing is foregrounded as the ADR's conclusion while the condition on which it depends (strong documentation deliverables) is backgrounded in Consequences. An ARCHITECT or BUILD reader who accepts the conclusion at face value may not weight the documentation deliverables with the urgency the condition requires.

**Pattern 3 — FI-2 (ADR-041 tautological source) was not addressed in the revision pass.**

The argument audit's framing finding FI-2 identified that ADR-041's analytical walk-through uses co-occurrence edges (from hand-tagged content) as the illustrative source for comparing named-relationship and structural-predicate grammars. The field notes §"Crawl-step results and the tautology threshold" explicitly established that co-occurrence over user-supplied tags is below the tautology threshold — neither grammar adds value beyond what the user encoded. The audit recommended adding one sentence acknowledging the scope condition (the grammar distinction becomes consequential after the tautology threshold is crossed, i.e., with semantic extraction or embedding as the source).

This finding was correctly surfaced by the audit but was not applied in the revision pass. The framing issues are routed to the gate per skill protocol, not silently absorbed into the ADR. That routing is correct process. The implication for ARCHITECT is that ADR-041's worked example in product-discovery and interaction-specs guidance does not carry the tautology scope condition. The analytical comparison's core claim — that structural predicates better preserve composition-shape extensibility — is valid. But the endorsement's practical scope condition (applies meaningfully only above the tautology threshold) is absent from the convention as stated.

**Earned confidence vs. sycophantic reinforcement:**

The user's single substantive intervention (ADR-038 LlmOrcEmbedder correction) is the clearest earned-confidence marker in this phase. The user articulated a specific architectural principle ("llm-orc should not be enshrined in Rust beyond the adapter spec"), named its scope ("the embedding strategy can be declared like anything else"), and the resulting ADR is more consistent with the prior ADR chain (ADR-024, ADR-025, Invariant 61) than the first draft. This is earned confidence from the system designer.

The four "Yes" approval moments after the audit produce a different signal. Approving a clean re-audit is not sycophantic reinforcement — the re-audit passed cleanly on argument grounds. The absence of counterargument at approval time is only a signal worth noting if the framing findings (which were explicitly not applied) contained substantive alternatives that the user would have interrogated had they been foregrounded. The argument audit routed framing findings to the gate, which is the correct place for the user to engage them.

The swift-adoption pattern (fourth consecutive instance) is the accumulation concern. Three prior snapshots recorded it; this snapshot confirms it persists at a DECIDE-phase artifact-production moment. The individual adoptions remain defensible; the cumulative invisible option space is the structural risk.

---

## Recommendation

**Grounding Reframe recommended — one specific and actionable item for ARCHITECT entry. Two advisory items that do not require blocking.**

### Grounding Reframe: The bundled-default-spec option should be staged and rejected on record before ARCHITECT

**The specific gap:**

ADR-040 and ADR-038 together establish that DiscoveryGap's activation in the default build requires a consumer-authored declarative adapter spec that produces `similar_to` edges via an llm-orc ensemble. The alternative — a reference spec bundled with the Homebrew binary (or placed at a well-known install path) that provides this activation without requiring user authorship — is consistent with both ADRs' mechanisms and has not been named as considered and rejected. It was invisible at drafting time, invisible during the audit, and invisible during the user's correction. It is not the same as the rejected "Rust LlmOrcEmbedder" option; it is an application of the already-accepted "consumer-declared external enrichment" mechanism at distribution rather than authoring time.

**Why this matters for ARCHITECT:**

ARCHITECT's scope for this cycle is described in cycle-status as "update system-design.md to name both embedding backends as first-class; verify DiscoveryGap trigger broadening does not cross module boundaries; regenerate ORIENTATION.md." The bundled-spec option does not fit cleanly into that scope. But if the option surfaces during BUILD — as a natural question when the worked-example spec is being committed at `examples/specs/embedding-activation.yaml` (per ADR-038) — BUILD will face a decision about whether that path should be positioned as "this is what you copy" or "this is what ships." BUILD making that determination without a DECIDE-level rejection reason means the option is still making its first appearance two phases late.

**Concrete grounding action for ARCHITECT entry:**

Before the ARCHITECT artifact is produced, stage the following question explicitly in the gate reflection: "Is there any scenario in which Plexus bundles a reference spec with the binary — placing `embedding-activation.yaml` (or equivalent) at an install-time `examples/` or `specs/` path — such that a consumer installing via Homebrew encounters a working activation path without authoring a spec themselves?" If yes, determine whether this violates Invariant 61 (consumer owns spec), whether it changes the distribution footprint (llm-orc provider configuration would still be required), and whether it would be scoped to BUILD or to a future cycle. If no, record the rejection reason so BUILD's worked-example scoping question is already answered.

This is a small staging question, not a blocking determination. The ARCHITECT phase can surface it as a scoped bullet in the ARCHITECT brief rather than requiring a new ADR. If the answer is "no — Invariant 61 means the spec is the consumer's artifact, not Plexus's" then the answer is available from existing architecture and can be recorded in a single sentence. The risk of leaving it unstaged is that BUILD makes the determination without the ADR-chain context.

**What builds on it if left unstaged:**

BUILD commits the worked-example spec at `examples/specs/embedding-activation.yaml` and makes a local scoping decision about whether consumers are expected to copy it or reference it. That scoping decision propagates into onboarding documentation (README, install guide). If the bundled-spec option would have been rejected on Invariant 61 grounds, the README will inadvertently position the example as a starting-point-to-copy rather than as a "copy this into your own spec" resource — the distinction matters for consumers who read documentation literally. The gap is small but the documentation consequence is persistent.

---

### Advisory item 1: FI-2 tautology scope condition should propagate to ADR-041's convention statement (not a blocker)

The framing finding FI-2 was surfaced by the argument audit and correctly routed to the gate. Its recommendation is a single sentence in ADR-041's analytical walk-through acknowledging that the illustrative source (co-occurrence over tagged content) is below the tautology threshold, and that the grammar distinction becomes consequential when the source crosses that threshold (semantic extraction, embedding similarity). The user should decide at the gate whether to apply this sentence.

If applied, the convention endorsement for structural-predicate grammars gains a visible scope condition: "structural predicates for discovery-oriented jobs — and specifically for jobs where the source crosses the tautology threshold." Without the scope condition, the convention reads as applicable to any discovery-oriented job, including those operating over pure co-occurrence-over-tags, where neither grammar adds value.

This does not require ARCHITECT action if the user decides not to apply FI-2. The convention is stated as a convention, not a requirement, and the phenomenological claim is already explicitly held as hypothesis. The scope condition would strengthen the analytical walk-through's practical grounding; the convention's core claim (composition-shape extensibility favors structural predicates) stands without it.

---

### Advisory item 2: The four-snapshot cross-phase swift-adoption pattern warrants a structural note for cycle continuation

Four consecutive phases (PLAY, DISCOVER, MODEL, DECIDE) have recorded the same dynamic: user offers a substantive framing that is plausibly correct and also convenient; agent adopts with speed; the intermediate option space goes unexamined. In each case the adopted framing was defensible. The cumulative effect is that the artifact set encodes compressed option space at each phase boundary.

The MODEL-phase gate reflection introduced the "Candidates Considered" section as a structural corrective — staging compressed options explicitly at phase boundaries rather than leaving recovery to the susceptibility evaluator. This ARCHITECT → BUILD gate should carry that precedent forward: if the ARCHITECT pass compresses an option during system-design.md updating (e.g., whether to document the embedding backend as "one path available, consumer-activates" vs. "two paths available, one bundled"), the gate reflection should stage the compressed option so BUILD works from a visible map.

This is a process recommendation, not a concern about the current ADR set's correctness.

---

## Notes on Role Dynamics

The partial-fidelity inhabitation limitation noted in prior snapshots remains structurally present. The user who corrected ADR-038's first draft is the architect of the system the ADR describes. The correction was substantively correct and the evidence in the Provenance section is clean. The limitation is not that the correction was wrong; it is that the correction was not stress-tested by someone asking "what would a consumer who needs embedding out-of-the-box, and cannot author a spec, experience under this decision?" That is precisely the perspective the PLAY session inhabitation could not fully occupy (builder inhabiting their own design). The partial-fidelity note from prior snapshots applies to the framing embedded at ADR-038's correction moment: the "consumer-declared activation is the right mechanism" principle is correct from the architectural layer; its friction implications for a non-builder consumer were not examined because the correcting stakeholder is the same person who designed the mechanism.

This is not a recommendation to revisit the correction — the correction is architecturally sound. It is a framing note: ADR-038's Consequences Negative ("Consumers who want embedding in the default build must do spec-authoring work") acknowledges the friction but does not assess its magnitude from a non-builder's perspective. The magnitude question is the kind of thing a second PLAY session with a non-builder stakeholder would surface. Cycle-status already names this as an optional strengthening action; this note confirms its relevance to the ADR-038 decision specifically.
