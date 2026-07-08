# Susceptibility Snapshot

**Phase evaluated:** DISCOVER (update mode) — Default-Install Experience and Lens Design Principles cycle
**Artifact produced:** `docs/product-discovery.md` (updated 2026-04-17)
**Date:** 2026-04-17
**Prior snapshot:** `docs/housekeeping/audits/susceptibility-snapshot-mcp-consumer-interaction-play.md` (PLAY phase, MCP Consumer Interaction cycle)

---

## Observed Signals

| Signal | Strength | Trajectory | Notes |
|--------|----------|------------|-------|
| Assertion density | Ambiguous | Declining relative to prior snapshot | Prior snapshot flagged three field-notes assertions as stated-as-conclusions. In this update, seven of those assertions are explicitly tagged "hold as working hypothesis" or "hold as characterization rather than definition." Agent hedging increased — but inconsistently (see notes). |
| Solution-space narrowing | Absent | Stable | Value tensions are framed as spectra or per-job distinctions rather than resolved choices. Three new tensions added; none collapsed prematurely. The embedding-backend decision was explicitly held open for DECIDE rather than resolved in product-discovery language. |
| Framing adoption | Ambiguous | Stable | Two specific instances worth examining: (1) "grounding examples, not build targets" reframe adopted quickly and integrated as a clarifying paragraph without recorded counterargument. (2) Lens-as-grammar extension engaged substantively by the agent; agent moved to scope-discipline mode without testing whether the grammar framing is correct. Both adoptions are substantively defensible, but the speed is a signal. |
| Confidence markers | Ambiguous | Declining relative to prior snapshot | Prior snapshot: multiple high-certainty unmarked assertions in practitioner voice. This update: "Validated as wrong," "hold as working hypothesis," "hold as characterization," "hypothesis pending DECIDE's grounding spike" — hedging language is more present. However, "Validated as wrong" applied to the default-install assumption inversion overstates resolution: Spike 2 established defect-by-omission at the release-configuration boundary; it did not validate the full default-install failure story as wrong across all deployment classes. |
| Alternative engagement | Clear (partial) | Improving relative to prior snapshot | Three of the four Phase-specific brief patterns are well-handled: per-job framing on interpretive vs structural predicates; two deployment classes making the embedding tension "structural, not just operational"; scope discipline on lens-as-grammar. One pattern that received no counterargument examination: whether "grounding examples, not build targets" forecloses a future cycle where Trellis or Carrel artifacts become real deliverables. The reframe is plausibly correct for this cycle, but the gate conversation record shows no examination of when the stance would not hold. |
| Embedded conclusions | Ambiguous | Declining relative to prior snapshot | Product-debt section contains several cells where the "Resolution" column resolves toward a specific build direction (e.g., "favor building the llm-orc/Ollama external embedding path already sanctioned by ADR-026 but unimplemented"). These pre-figure DECIDE outcomes. This is expected at a DISCOVER → MODEL boundary — the tensions surface, resolution directions appear — but three of the five new product-debt rows carry specific implementation direction rather than named options. |

---

## Interpretation

### What the signals collectively suggest

The prior snapshot's Grounding Reframe on two uncertainties was explicitly actioned. Uncertainty 2 was resolved by Spike 2 before the DISCOVER update began — and that resolution is correctly reflected in the artifact as an examined-and-grounded assumption inversion. Uncertainty 1 was not resolved (the spike was not tractable as a live experiment) and is correctly carried as a hypothesis with per-job nuance preserved.

This is a meaningful improvement over the prior state. The field notes' assertion-form language has been substantially attenuated in this update — the hedging language ("hold as working hypothesis," "hold as characterization," "hypothesis pending DECIDE's grounding spike") appears where the prior snapshot said it should. The per-job framing from field-notes §"Apps have multiple jobs" was preserved correctly — product-discovery specifically says "the same app may contain both job types" rather than collapsing to a per-app binary.

Three patterns from the Phase-specific brief require examination:

**Pattern (a) — Research-phase framings inherited without testing against user voice.**

The delayed-recognition quality and the querying-begets-ingestion loop are both tagged "hold as working hypothesis, derived from one practitioner's inhabitation" in the product-discovery update. This is the correct posture per the prior snapshot's recommendation. However, the "Apps are containers for their own material" assumption inversion — the "lenses on material, not containers" framing — is tagged "hold as characterization rather than definition" but the associated product-vocabulary and stakeholder-model language is not hedged: the Consumer Application Developer mental model reads the cross-pollination flywheel and "thin surfaces over the shared graph" framing as settled description, not as working characterization. The hedging is present in the assumption inversions section but has not fully propagated to the sections that inherit from it.

Specifically: the Mental Model paragraph at line 62 reads as settled architecture ("The lens processes a graph it partly authored... the cross-pollination flywheel") without any hypothesis marker. A reader who reaches the consumer mental model section without reading the assumption inversions section encounters these as descriptions of what Plexus does, not hypotheses about what a real consumer workflow looks like.

**Pattern (b) — Value tensions that surfaced as spectra but may have collapsed into binary framings.**

The examination shows the three new value tensions are handled well on this criterion. "Interpretive vs structural lens predicates" is explicitly scoped per-job, not per-app. "Easy-to-demo vs honest-to-demo" is framed as a genuine tension with a question at the end rather than a resolved position. "Default-lean vs full-capability" is explicitly resolved toward "both paths first-class" with the constraint named — but this is appropriate for a value tension whose resolution is load-bearing for DECIDE, and the resolution is grounded in Spike 2 evidence.

**Pattern (c) — Prior-snapshot assertions picked up as settled findings.**

Largely handled. The main case where the prior snapshot's assertion-form language was NOT adequately attenuated is the assumption inversion "Consumer apps are containers for their own material" — as noted above, the "lenses on material, not containers" framing became settled architectural description in the stakeholder model without the full hypothesis hedging that appears in the assumption inversions section.

**Pattern (d) — Hypotheses correctly parked vs. smuggled into product-discovery as settled findings.**

Lens-as-grammar was correctly parked (cycle-status.md) and the product-discovery `lens` vocabulary entry explicitly flags the theoretical framing as "scoped out of this cycle" and carries only "composition-shape awareness" forward. This is clean scope discipline.

**One signal not in the Phase-specific brief worth naming: the "Validated as wrong" framing.**

The default-install assumption inversion is marked "**Validated as wrong** at the release-configuration boundary." Spike 2 established that the Homebrew binary's silence on the embedding question is defect-by-omission, not staged onboarding. This is a grounded determination. However, "validated as wrong" as a label suggests more confidence than Spike 2's determination supports: Spike 2 concludes that no documented deliberate intent exists for the staged-onboarding framing; it does not establish that the full value proposition fails for all default-install consumers. Consumers who install Homebrew plus Ollama (the common developer-tool context) and follow the existing llm-orc-backed extraction path may actually be getting meaningful value — the "advertised value proposition" varies by which claim is being evaluated. The labeled assumption is too wide for the evidence.

### Earned confidence vs. sycophantic reinforcement

The improvement over the prior snapshot is genuine. The hypothesis hedging, the per-job framing preservation, the scope discipline on lens-as-grammar — these are responsive to the prior snapshot's recommendations and are correctly handled. The artifact is not in a state of sycophantic reinforcement: multiple named uncertainties remain explicitly open, and three new value tensions carry real tension rather than being resolved prematurely.

The residual risk is language-layer inconsistency rather than structural narrowing: the hypothesis language in the assumption-inversions section has not fully propagated to the consumer mental-model section, which means a reader who stops at the mental-model section receives the cross-pollination flywheel architecture as settled description. At the DISCOVER → MODEL boundary this matters because the domain model will import from the stakeholder-model section, not from the assumption-inversions section.

---

## Recommendation

**Grounding Reframe recommended — one specific uncertainty, narrow scope. Two language-consistency notes that do not require blocking.**

### Grounding Reframe: Hypothesis hedging is inconsistent between the assumption-inversions section and the stakeholder-model section

**The specific uncertainty:**

The "Apps are lenses on material, not containers" characterization is correctly hedged in the assumption-inversions section ("hold as characterization rather than definition — surfaced during one practitioner's inhabitation; real-consumer behavior may reveal additional forces"). But the consumer-developer mental model (product-discovery §"Jobs and Mental Models → Consumer Application Developer") presents the querying-begets-ingestion feedback loop and the "thin surfaces over the shared graph" posture as settled description. The hedging is present in one section and absent from the adjacent section that inherits from it.

**Where it lives in the artifact:**

`docs/product-discovery.md`, the paragraph beginning "Query begets ingestion — a consumer/Plexus feedback loop" (line 62) and the final sentence of the two-paragraph block beginning "In a shared context this is the cross-pollination flywheel." Both are marked "hold as working hypothesis" in the assumption-inversions section but not where they appear in the mental-model section.

**Concrete grounding action for MODEL:**

MODEL can resolve this without blocking: when the domain modeler reads the consumer-developer mental-model section, treat the cross-pollination flywheel architecture and "thin surfaces" posture as hypothesis-level input to the model rather than settled description. Do not promote "the graph is partly authored by responses to its own surfaces" to an invariant or a domain model obligation in this pass. Flag it for a future cycle with real untagged-content evidence. The domain-model amendment this cycle should cover dimension semantics (feed-forward item 10) and any new invariants the default-install resolution demands — not a new invariant codifying the feedback-loop architecture.

**What builds on it if left ungrounded:**

The domain model would inherit "query begets ingestion" as a structural design obligation, potentially prompting new invariants around feedback-loop support that have no observed-behavior validation behind them. The MODEL artifact would then carry those into ADR design, where DECIDE would be working to satisfy an invariant derived from one practitioner's inhabitation rather than from observed user behavior.

---

### Language-consistency note 1 (no blocking action required)

The "**Validated as wrong**" label on the default-install assumption inversion is slightly overconfident. Spike 2 established defect-by-omission at the release-configuration layer, not a universal failure of the advertised value proposition across all deployment classes. The label is directionally correct but would be more accurate as "**Grounded: defect-by-omission at the release-configuration layer — not a deliberate staged-onboarding design**." DECIDE will produce the ADR framing the resolution; the label can be revisited at SYNTHESIZE if the team wants sharper language.

This is a cosmetic note for accuracy — it does not affect DECIDE's direction.

### Language-consistency note 2 (no blocking action required)

The five new product-debt rows have Resolution cells that carry specific build directions rather than named options for DECIDE. This is appropriate for three of the five (TemporalProximity property-contract bug — singular fix; EmbeddingSimilarity shipping direction — Spike 2 grounded this toward Ollama-backed path; onboarding demo revision — directionally clear). For DiscoveryGap trigger coupling and minimum-useful-spec, the resolution cells state directions as settled conclusions that are better framed as options pending DECIDE. Not a blocking concern — DECIDE will revise these — but a downstream phase reader picking up product-discovery in isolation would see them as settled.

---

## Notes on Role Dynamics

The practitioner-as-builder dynamic from the prior cycle persisted into this DISCOVER update, as it must — the user is both cycle stakeholder and product-discovery author. The structurally clean observations in this update are:

- The "grounding examples, not build targets" reframe was accepted without counterargument. Its adoption speed is noted; the substantive correctness is not in doubt. Whether the reframe's scope extends to all future cycles (i.e., whether a future cycle would explicitly build Trellis artifacts as deliverables) was not examined during the gate. This is a parked question, not a current concern.
- Lens-as-grammar was extended by the user as a substantive hypothesis and then subjected to scope discipline by the user. The agent moved with the user toward scope-discipline mode without independently stress-testing whether the grammar framing is internally consistent. Given that the framing was parked rather than adopted, this is low-risk. If lens-as-grammar returns in a future cycle, the initial belief-mapping should happen then.
- Both of these are examples of the persistent pattern from the prior snapshot: swift adoption of the user's substantive framings. The prior snapshot flagged this as a signal worth watching; this snapshot confirms it is a consistent dynamic rather than a phase-specific artifact. The risk remains low so long as the adopted framings are held explicitly as hypotheses (which they are), but the pattern's persistence warrants naming for future snapshot evaluators.
