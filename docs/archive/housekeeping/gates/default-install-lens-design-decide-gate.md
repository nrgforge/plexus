# Gate Reflection: Default-Install Experience and Lens Design Principles — DECIDE → ARCHITECT

**Date:** 2026-04-21
**Phase boundary:** DECIDE → ARCHITECT
**Cycle:** Default-Install Experience and Lens Design Principles

## Belief-mapping question composed for this gate

Primary belief-mapping question (from the Grounding Reframe carried out of the susceptibility snapshot):

> *What would you need to believe for the bundled-default-spec option — shipping `examples/specs/embedding-activation.yaml` as an install-time artifact rather than only as onboarding documentation — to be the right choice over pure consumer-authored activation?*

Separated into two sub-questions to surface the underlying principle:

1. Is Invariant 61 (consumer owns spec) the settled ground that rules out Plexus bundling a reference spec? Or does "consumer owns spec" mean "consumer owns *their own* spec" — which is orthogonal to whether Plexus ships examples?
2. If Invariant 61 doesn't rule it out, is there any other architectural reason to treat the example as copy-fodder only, rather than as an install-time artifact?

Two follow-up questions on close-call decisions were offered to test whether the argument chain held under interrogation:

- **Q1 (ADR-042):** Given the rejected option (i) (warn-on-divergence) and the commitment to documentation-only, is the evidence base more risky than the ADR treats it as?
- **Q2 (ADR-041):** Is composition-shape reasoning alone sufficient to endorse the convention, or does phenomenology-of-discovery do hidden work in the per-job split?

## User's response

**On the bundled-default-spec option (Grounding Reframe):**

> "The bundled default spec option seems great but for the fact that it relies on llm-orc which then in turn relies on Ollama. So without it all being somehow self-contained, that's a non-starter for me. In practice to get real value from plexus we'll need to leverage llm-orc. But the default version of it shouldn't break without it."

The user rejected the bundled-default-spec option on a specific ground: the spec would produce no effect without llm-orc and a configured embedding provider, so bundling it would re-create the same kind of truthfulness gap ADR-038 was written to close — the user would see a "default spec" shipping with the binary that in fact does nothing without separately-installed infrastructure. The rejection is not grounded in Invariant 61 but in ADR-038's own honest-to-demo principle extended to a second layer.

**On the follow-up tradeoff questions (Q1 and Q2):**

> "Those are both deep questions that I'm not sure how to cleanly resolve off the top of my head. I think they could be left as open questions to investigate in the course of architect and build, but I'm also curious if you can analyze the tradeoffs here and what might shift our thinking."

The user asked for tradeoff analysis before deciding whether to act on or defer either question. The agent produced analyses naming what would shift thinking in each case:

- **Q1 (ADR-042):** The rejection of option (i) is defensible, but the escalation trigger ("if repeated PLAY or user observation surfaces the failure mode") depends on observational signal that may not arrive. BUILD-phase silent-idle-detection instrumentation would convert the trigger from observational to detectable. Named as a BUILD-phase concern worth surfacing in the ADR.
- **Q2 (ADR-041):** The convention's per-job split rests on phenomenology-of-discovery (hypothesis), while composition-shape (analytical) carries the extensibility preference. The ADR framed composition-shape as load-bearing and phenomenology as a held-at-arms-length hypothesis, but close reading showed phenomenology was actually load-bearing for the per-job distinction. One clarifying paragraph could make the argument-grounds split visible without changing the decision.

> "Very well, let's apply both please."

The user chose to apply both clarifications rather than defer them.

## Pedagogical move selected

**Challenge** (rebuttal-elicitation + tradeoff analysis).

The gate opened with a belief-mapping question on a specific alternative that had been invisible during drafting. The user engaged substantively with the alternative, named a concrete rejection reason tied to the ADR's own purpose, and resolved the Grounding Reframe within one response. On the two follow-up close-call questions, the user declined to resolve from memory and requested tradeoff analysis — which led to the three clarifying additions being applied.

The three gate-conversation-driven additions (ADR-038 bundled-spec rejection, ADR-041 FI-2 + argument-grounds clarification, ADR-042 empirical escalation signal) were audited in a third pass and all landed cleanly with no new issues.

## Commitment gating outputs

**Settled premises (the user is building on these going into ARCHITECT):**

- Plexus's Rust core stays narrow. No Rust code path to llm-orc for embedding; consumer activation via declarative adapter spec + external enrichment is the single accepted mechanism for default-build embedding activation.
- The worked-example spec at `examples/specs/embedding-activation.yaml` is a documentation artifact, not an install-time auto-loaded artifact. Honest-to-demo applies at every layer where a user might form a default-behavior expectation.
- The default Homebrew binary must not break without llm-orc. Graceful degradation (enrichment silent-idle, not error) is a hard contract.
- `features = ["embeddings"]` remains first-class for library-consumer deployments that cannot depend on llm-orc at all. Both paths are peers with different deployment suitability.
- Dimensions remain extensible. Syntactic validation (empty, whitespace, reserved characters) fails at `load_spec`; semantic guidance is documentation-only. Option (i) warn-on-divergence was rejected for this cycle with the escalation path preserved.
- The lens grammar convention endorses structural predicates for discovery-oriented jobs as convention, not requirement. Composition-shape reasoning is load-bearing for the extensibility preference; phenomenology-of-discovery is load-bearing for the per-job split and is held as hypothesis, not principle.
- The `created_at` property contract places authoritative timestamp data on `node.properties`, with ISO-8601 UTC string format. Adapters write; enrichment reads. Graceful degradation on missing/unparseable values.
- DiscoveryGap's trigger relationship is source-agnostic. The enrichment stays narrow (no algorithm broadening); the declarative/external path is the extension surface.

**Open questions (the user is holding these open going into ARCHITECT):**

- **Empirical escalation signal for ADR-042.** Whether BUILD should instrument silent-idle detection (logging dimension divergences at debug level) is a BUILD-phase concern; the ADR names it as an opportunity but does not require it. ARCHITECT can surface this as a system-design concern if the logging surface is touched during architecture updates.
- **Phenomenology-of-discovery hypothesis (ADR-041).** The per-job split rests on the hypothesis; a future cycle with untagged-content evidence and a non-builder PLAY stakeholder may promote, revise, or reject it. Neither ARCHITECT nor BUILD is expected to resolve this; the convention is actionable as hypothesis-grounded guidance.
- **ADR-038 reframing is contingent on documentation deliverables.** "Positive decision, not defect-by-omission" holds only if BUILD lands the README updates, worked-example spec, and onboarding material. If those deliverables are weak or delayed, the defect-by-omission framing reasserts. ARCHITECT should weight the documentation deliverables when scoping BUILD.
- **Second PLAY session (optional).** A non-builder stakeholder PLAY session was named in cycle-status as "methodologically valuable but not blocker" and declined for this cycle due to resourcing. Carried as an optional strengthening action after BUILD lands.

**Specific commitments carried forward to ARCHITECT:**

- Update `docs/system-design.md` to name both embedding backends as first-class per deployment class (Homebrew/CLI default = llm-orc-driven via consumer spec; library-consumer with `features = ["embeddings"]` = in-process fastembed).
- Verify that ADR-040's DiscoveryGap trigger-coupling story does not cross module boundaries. The decoupling is a naming/documentation concern at the enrichment level, not a structural architectural change — ARCHITECT should confirm.
- Regenerate `docs/ORIENTATION.md` if architectural drift is detected after DECIDE's additions land.
- Surface (in the ARCHITECT brief) the three open-question items above, plus the conformance-scan debt (7 items, all routed to scenarios or BUILD).
- Carry forward "Candidates Considered" stylistic discipline from MODEL into the ARCHITECT gate — stage compressed options explicitly at phase boundaries (per the susceptibility snapshot's Advisory item 2).

**Framing audit findings not applied (left for ARCHITECT or future cycle):**

- **FI-1** — the staged-onboarding alternative framing for ADR-038 (lite/full product tiers) was not added as an "alternatives considered" note. Judgment call; not load-bearing.
- **FI-3** — cosmetic cross-reference addition in ADR-040's Consequences section. Not applied; cosmetic.

Both are recorded here so a future reader can see the option space was visible and declined, not invisible.

**Partial-fidelity inhabitation note (carried from susceptibility snapshot):**

The DECIDE phase's user interventions came from the system's architect, who is the same stakeholder as the PLAY session's inhabitant. The ADR-038 correction is architecturally sound, but the magnitude of the friction cost for a non-builder consumer needing embedding out-of-the-box was not assessed from outside the designer's perspective. A second PLAY session with a non-builder stakeholder would strengthen BUILD's outputs on this specific axis. Noted; not blocking.
