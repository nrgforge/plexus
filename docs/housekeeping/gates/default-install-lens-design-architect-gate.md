# Gate Reflection: Default-Install Experience and Lens Design Principles — ARCHITECT → BUILD

**Date:** 2026-04-22
**Phase boundary:** ARCHITECT → BUILD
**Cycle:** Default-Install Experience and Lens Design Principles

## Belief-mapping question composed for this gate

The gate opened with a pre-mortem probe on the highest-stake ARCHITECT commitment (ADR-038's "positive decision, not defect-by-omission" reframing, which is contingent on WP-D's documentation deliverables crossing the tautology threshold):

> If BUILD discovers that the worked-example spec cannot cross the tautology threshold with the provider choice made — say, the embedding model produces no discriminating signal on the chosen prose, or the llm-orc ensemble flakes on the chosen provider — what's the right response? Push through with a thinner example? Pause and change the provider choice? Amend ADR-038 to acknowledge the reframing didn't land?

The probe was anchored in the ADR-038 text's own pause-and-escalate language, asking the practitioner to specify "escalate to whom, under what signal?" — load-bearing practical guidance the roadmap framed only abstractly.

Following the probe, five commitment-gating items were offered covering the roadmap's WP decomposition, the worked-example provider choice, the WP-B owned-string migration scope, the fitness criterion's test-coverage status, and the phenomenology-of-discovery hypothesis scope.

## User's response

**R1 (tautology pre-mortem):**

> "Yeah -- the question becomes how do we rise above what we're experiencing to get away from tautology. That would be escalating to me and we could then go into another research cycle if necessary."

The practitioner reframed the question's central concern with more precision than the probe carried. "Rise above what we're experiencing" names the recursive risk — that our demonstration of escape from tautology can itself be tautological if we carry pre-encoded assumptions about what emergent structure should look like into the worked example's prose selection or the ensemble's parameters. The escalation path: to the practitioner. The response shape: potentially a new research cycle, not a WP-D patch.

**R2 (commitment gating):**

> "Yes to WP. I will use Ollama models via llm-orc.. we should figure out embedding strategy via llm-orc. owned-string migration... your call. We should test it doesn't break without llm-orc, but we should be transparent about what functionality we don't get with that. PoD hypothesis -- I think so, but what prevents it from being this cycle?"

The practitioner confirmed the WP decomposition, committed to Ollama via llm-orc for empirical validation, named embedding strategy within llm-orc (model, parameters, threshold) as a BUILD exploration rather than a pre-decided input, deferred the owned-string migration scope call to the agent, named capability-loss transparency as a first-class WP-D deliverable alongside capability-present framing, and challenged the deferral of phenomenology-of-discovery to a future cycle.

**R3 (phenomenology split-treatment choice):**

> "OK ii it is"

The practitioner selected option (ii) from three offered after the agent engaged the challenge directly: phenomenology-of-discovery splits into composition-shape reasoning (analytical, partially validatable in-cycle if WP-D crosses tautology) and phenomenology proper (experiential, still requires non-builder stakeholder and remains future-cycle). Option (ii) captures the split in the roadmap without over-committing to an in-cycle test that the worked example's tautology-threshold outcome gates.

## Pedagogical move selected

**Probe** (warrant-elicitation / pre-mortem hybrid on the ADR-038 documentation-deliverables contingency), followed by **Commitment gating** (five items), followed by direct engagement with a practitioner-initiated challenge on the phenomenology scoping.

The initial probe referenced specific ADR content (ADR-038 Consequences Negative's pause-and-escalate language) and asked for the missing specificity. The commitment gating used the standard phase-boundary question form. The phenomenology challenge was practitioner-initiated — the agent responded by presenting the two-argument split in ADR-041 and staging three in-cycle options (defer both / split with in-cycle opportunity / formalize as new WP) rather than restating the "future cycle" position. The practitioner selected the middle option, which matches the ADR's own provenance structure without expanding scope.

## Commitment gating outputs

**Settled premises (the practitioner is building on these going into BUILD):**

- Five-WP decomposition (A/B/C/D + optional E), recommended order A → C → B → D.
- **Ollama via llm-orc** is the provider for empirical tautology-threshold verification of the worked-example spec. Onboarding prose names OpenAI-compatible endpoints as the other common shape (llm-orc handles provider routing; Plexus is provider-indifferent).
- **Embedding strategy within llm-orc is a BUILD exploration** — model choice (nomic-embed-text, mxbai-embed-large, others via Ollama), similarity threshold, batch size, ensemble shape, and output relationship name are BUILD-phase choices to try empirically against the tautology-threshold bar. Not pre-decided in ADRs.
- **WP-B owned-string migration scope:** grep all call sites of the dimension-typed field first; if `&'static str` is entrenched deeper than expected (e.g., on-wire or in contribution keys), pause and escalate to the practitioner for ADR-amendment judgment. Don't push through a larger refactor unilaterally.
- **Capability-loss transparency is a first-class WP-D deliverable, not an afterthought.** The README must name what functionality is absent without llm-orc (DiscoveryGap idle; EmbeddingSimilarity producing no signal; lens translation of LLM-extracted structure absent). Direct framing, not apologetic or overclaiming.
- **Fitness criterion "default binary does not break without llm-orc" will be actively tested**, not inherited as verified from existing test coverage. Test coverage for the three enumerated deployment shapes to be confirmed at WP-A and WP-D entry.
- **Tautology escalation path:** BUILD escalates to the practitioner when the worked example cannot rise above tautology. Response shape may be a new research cycle on emergent-structure demonstration, not a WP-D patch. Shipping a thin worked example is worse than shipping no worked example — creates appearance of complete documentation lever while delivering tautological output.
- **Phenomenology hypothesis (ADR-041) split by argument:** composition-shape reasoning is partially validatable in-cycle IF WP-D crosses tautology (run both grammar conventions over emergent content and observe whether analytical walk-through's claims obtain); phenomenology proper remains future-cycle pending non-builder stakeholder PLAY. The in-cycle composition-shape observation is a stewardship checkpoint at WP-D close, contingent on WP-D's tautology-threshold outcome.

**Open questions (the practitioner is holding these open going into BUILD):**

- **Whether WP-D's worked example will cross the tautology threshold with Ollama embedding models on realistic untagged prose.** Empirical, unknowable at ARCHITECT close. Multiple model/parameter combinations may need testing before landing on one that produces discriminating signal. BUILD documents the final choice and rejected alternatives.
- **Whether composition-shape observation at WP-D close will ground or revise ADR-041's analytical walk-through.** Only triggered if WP-D crosses tautology. If the analytical claims hold, the convention's extensibility argument is empirically grounded; if they don't hold, the convention's composition-shape argument needs revision and the convention as-stated may need amendment.
- **Non-builder PLAY session remains deferred.** Phenomenology validation carries forward as future-cycle concern. The partial-fidelity inhabitation concern (builder inhabiting their own design) recurs if the practitioner self-tests.
- **Fitness-criterion test coverage for two of three deployment shapes** (in-process, consumer-spec) is aspirational until BUILD confirms. Graceful-idle baseline (shape iii) is accurately covered by existing acceptance tests.

**Specific commitments carried forward to BUILD:**

- Execute WP-A as a single `fix:` commit (four-site coordinated change; partial landing worse than no landing).
- Execute WP-B by opening with a call-site grep for the dimension-typed field; pause-and-escalate if `&'static str` is load-bearing in consumers not visible from the `resolve_dimension` locality.
- Execute WP-C as a `docs:` commit (D-05 + D-07, two single-file edits).
- Execute WP-D with the strengthened deliverable list: README capability-present + capability-loss framing; worked-example spec at `examples/specs/embedding-activation.yaml` using Ollama via llm-orc, targeting emergent `similar_to` edges over untagged prose; spec-author documentation reachable in one navigation hop; ADR-042 dimension-choice guidance; ADR-041 lens grammar convention documentation; "minimum-useful-spec" pattern in interaction specs.
- At WP-D close, if the worked example crosses tautology: run both grammar conventions (`lens:{consumer}:thematic_connection` vs `lens:{consumer}:latent_pair`) over the emergent content and observe whether ADR-041's analytical walk-through claims obtain. Record as a reflection.
- At WP-D close, if the worked example does not cross tautology: escalate to the practitioner. Do not ship a thin worked example. A new research cycle on emergent-structure demonstration is the legitimate response.
- At WP-A entry and WP-D entry: confirm existing acceptance test coverage for the three enumerated deployment shapes (in-process, consumer-spec, graceful-idle baseline); add test coverage where the fitness criterion is aspirational rather than verified.
- WP-E (silent-idle debug instrumentation) is optional — include if BUILD is already in the validator path for WP-B, defer otherwise.

**Framing audit items not addressed (left for BUILD or future cycle):**

None from this phase. The DECIDE gate's unaddressed framing items (FI-1 staged-onboarding alternative for ADR-038, FI-3 cosmetic cross-reference) were judgment calls recorded at the DECIDE gate and do not resurface at ARCHITECT.

**Partial-fidelity inhabitation note (carried from prior snapshots):**

The partial-fidelity concern persists. The practitioner is the sole tester and the ARCHITECT pass ratified the system design against the practitioner's architectural model. A second PLAY session with a non-builder stakeholder remains the canonical corrective — declined this cycle by resourcing constraint, carried forward as a recommended follow-up. The phenomenology-of-discovery split-treatment in this gate's outputs preserves the in-cycle opportunity that is empirically testable (composition-shape) while acknowledging the non-builder-dependent claim (phenomenology) remains genuinely unresolved.
