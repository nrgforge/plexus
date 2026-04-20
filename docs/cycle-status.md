# Active RDD Cycle: Default-Install Experience and Lens Design Principles

**Started:** 2026-04-17
**Current phase:** DECIDE (next) — DISCOVER update complete 2026-04-17; MODEL light-touch pass complete 2026-04-20
**Artifact base:** ./docs/
**Scope:** Default-install consumer experience (what happens when a new consumer installs plexus via Homebrew and follows the advertised path) + lens design principles (named-relationship vs structural-predicate conventions, and whether the phenomenology-of-discovery constraint applies broadly or only to specific consumer types)

**Prior cycle:** MCP Consumer Interaction Surface (2026-04-01 → 2026-04-17) — concluded; archived at `docs/archive/cycle-status-mcp-consumer-interaction.md`. PLAY findings are the input to this new cycle.

---

## Phase Status

| Phase | Status | Artifact | Key Epistemic Response |
|-------|--------|----------|------------------------|
| Spike 2 (default-install intent) | ✅ Complete | `docs/housekeeping/spikes/spike-default-install-intent.md` | Feature-flag rationale is documented (build weight); release-distribution rationale is not. Homebrew inheritance of `default = []` is defect-by-omission at the release-configuration layer, not a deliberate staged-onboarding design. Two independent observations surfaced (DiscoveryGap trigger coupling; `created_at` property-contract bug). |
| Spike 1 (lens grammar comparison) | ⏸ Deferred to DECIDE | — | Not tractable as a live experiment in the current build — the structural predicates the hypothesis turns on (`bridges_communities`, etc.) require enrichments that don't exist yet. Carried to DECIDE as analytical work alongside lens-grammar ADR drafting. |
| DISCOVER (update) | ✅ Complete | `docs/product-discovery.md` (updated); gate reflection note at `docs/housekeeping/gates/default-install-lens-design-discover-gate.md`; susceptibility snapshot at `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-discover.md` | Updates reflect PLAY field-notes, Spike 2 resolution, and gate-surfaced refinements (two deployment classes for the embedding decision; parallel-code-paths constraint on the ADR; grounding-examples-not-build-targets stance; lens-as-grammar parked for future cycle with composition-shape awareness carried forward). Snapshot recommended one Grounding Reframe for MODEL entry (see below); mental-model section hedging applied per option A. |
| MODEL | ✅ Complete | `docs/domain-model.md` (Dimension entry softened; OQ 15 added then re-expanded with candidates; Amendment Log entry #9); gate reflection note at `docs/housekeeping/gates/default-install-lens-design-model-gate.md`; susceptibility snapshot at `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-model.md` | Light-touch pass complete. No invariant changes. Dimension entry reshaped from enumeration to extensibility-aware framing (three candidates staged; option (c) minimal selected per user's "dial complexity without complecting" guidance). OQ 15 routes spec-author-guidance question to DECIDE with extensibility as binding constraint and three live candidates explicitly staged (warn-on-divergence / documentation-only / syntactic-only). Grounding Reframe honored: no mental-model hypotheses promoted, no lens-as-grammar vocabulary added. Code documentation drift flagged (`src/graph/node.rs:10` cites a non-existent ADR title) — DECIDE/BUILD small cleanup task. Future-cycle belief-mapping question logged for lens-as-grammar entry. |
| DECIDE | ▶ Next | — | — |
| ARCHITECT | ☐ Pending | — | Light-touch pass planned. Principal work: update `system-design.md` to name both embedding backends as first-class (per DECIDE's shipping ADR); verify DiscoveryGap trigger broadening (if it happens) does not cross module boundaries; regenerate `docs/ORIENTATION.md` if drift is present. |
| BUILD | ☐ Pending | — | — |
| PLAY | ☐ Optional | — | Second PLAY by non-builder stakeholder is **not happening this cycle** (user is the sole tester at present; methodologically valuable but not realistic in current resourcing). Cycle continues without it. |
| SYNTHESIZE | ☐ Optional | — | — |

---

## Grounding Reframe Carried Forward (per 2026-04-17 susceptibility snapshot)

The PLAY phase of the prior cycle produced substantive field notes at `docs/essays/reflections/field-notes.md`. The susceptibility snapshot at `docs/housekeeping/audits/susceptibility-snapshot-mcp-consumer-interaction-play.md` flagged two uncertainties that should enter this new cycle **as hypotheses, not as settled findings**:

### Uncertainty 1 — Is the phenomenology-of-discovery constraint on lens grammar a Plexus design principle, or one stakeholder's design preference?

**Current field-note framing:** Lenses for consumers like Trellis must write signals that create conditions for discovery without asserting what's there; named relationships cancel the discovery phenomenology.

**Source of the claim:** The Trellis architecture paper §3.7 distinguishes "receiving information" from "having a discovery." The constraint was carried into the field notes as a Plexus design principle applicable beyond Trellis.

**Mid-PLAY refinement (field-notes §"Apps have multiple jobs"):** The hypothesis has already shifted once within the source material. Stepping into Carrel from the user perspective revealed Carrel is equally discovery-oriented — its publishing pipeline is downstream of thesis-finding, not the point of the app. The named-relationship vs structural-predicate question is therefore not a per-app choice; it is a **per-job choice within an app**. The same app may need named relationships for its publishing pipeline's internal routing AND structural predicates for its thesis-finding surface. This refinement is visible in field-notes but has not yet been reflected in product discovery or the ADR candidates.

**Why this is still not settled:** The per-job framing itself is derived from one practitioner's perspective-taking during PLAY — still one stakeholder's lens, now with finer grain. Whether real consumers build apps this way, and whether the structural-predicate constraint actually produces the phenomenology it claims, remains a hypothesis. Other consumer types (search-oriented, analytics, etc.) haven't been inhabited at all.

**Clarification from 2026-04-17 practitioner input — the two layers the spike differentiates:**

- **Value-proposition layer (settled):** Plexus adds value when it surfaces structure the user didn't encode. Latent discovery IS the value proposition. Anything that doesn't start from that premise IS tautological — and in that sense, the field notes' claim is correct.
- **Lens grammar layer (still hypothesis):** The claim that the lens MUST output structural predicates (not named relationships) to preserve discovery phenomenology. A publishing pipeline using `lens:carrel:cites` or `lens:carrel:references` as operational data is not tautological — the named edge is pipeline routing logic, not writer-facing discovery signal. The phenomenology constraint plausibly applies to the writer-facing subset of lens consumers, not all lens consumers.

**What the spike actually tests:** Whether the grammar-layer constraint is universal or consumer-dependent. Outcome possibilities:
- Constraint is universal (structural-only for all lens outputs) → single lens-design rule in the ADR.
- Constraint differentiates by consumer type (structural for writer-facing; named permitted for pipeline-facing) → richer guidance in the ADR that names the distinction.
- Constraint is subtler (e.g., named permitted when the consumer doesn't surface the edge to a human reader; structural required otherwise) → ADR articulates the triggering condition.

The value-proposition layer does not require spiking; it's the grammar layer that does.

**Concrete grounding action for this cycle:** During DECIDE, draft two alternative lens specs for Trellis — one using named relationships (`lens:trellis:thematic_connection`), one using structural predicates (`lens:trellis:bridges_communities`) — and walk through what each would emit and how the query output would differ. Run as analytical comparison (part of lens-grammar ADR drafting), not as a live DB experiment — the structural predicates the hypothesis turns on require external enrichments (graph-science scripts) that don't exist in the current build, so a live experiment would reduce to renaming the same edge and could not demonstrate the phenomenological claim.

**Why the spike shape changed (decision made 2026-04-17):** Initial plan was a live DB experiment before DISCOVER. Reconnaissance showed the live version is not tractable: the only structural signal available in the default build is `may_be_related` from CoOccurrence, so any two lens specs translating it reduce to single-hop edge renames. The comparison belongs with ADR drafting, where considering alternatives is already the work.

**What builds on this if left ungrounded:** Any DECIDE output (ADRs, interaction-specs guidance) that codifies "lenses MUST use structural predicates" as a design principle. Any SYNTHESIZE essay that names this as a Plexus-level finding rather than a Trellis-level design choice.

### Uncertainty 2 — Is the default-install enrichment gap a defect or a staged-onboarding pattern? (RESOLVED 2026-04-17)

**Resolution:** Neither framing is purely correct. Spike 2 (`docs/housekeeping/spikes/spike-default-install-intent.md`) found the feature-flag rationale is documented at the engine-architecture layer (commit `8d2ec7e`, ADR-026): `embeddings` is opt-in because ONNX Runtime is a non-trivial build dependency. The **release-distribution rationale is not documented** — `dist-workspace.toml` and `.github/workflows/release.yml` pass no feature flags, so the Homebrew binary inherits `default = []` by omission rather than by positive decision. Defect-by-omission framing is therefore substantively correct at the release-configuration boundary; staged-onboarding is a plausible **remediation shape**, not a plausible description of current state.

**Original field-note framing (preserved for audit trail):** Three of four default enrichments are silently inactive in the default Homebrew build (EmbeddingSimilarity feature-gated off; DiscoveryGap has no triggers without embeddings; TemporalProximity reads a non-existent property contract). Default-install doesn't deliver the advertised value proposition.

**Implications the spike surfaced:**
- The truthfulness gap is at the release-configuration layer, not the feature-flag level. DECIDE should write a Release-Binary Feature Profile ADR that explicitly decides what the distributed binary contains.
- DiscoveryGap's lack of triggers without embeddings is coupled to the release-binary decision; flag explicitly in the ADR rather than leaving the coupling implicit.
- TemporalProximity's `created_at` property bug is **independent** of the release-binary decision — separate property-contract ADR.
- BUILD should not modify `with_default_enrichments()` before the release-binary ADR is decided.

### Additional structural concern (from snapshot §Role Dynamics)

The PLAY session's practitioner-as-builder dynamic means the consumer-developer stakeholder was inhabited by the engineer who built the system. This is a partial-fidelity inhabitation — inhabitation confirms the builder's mental model rather than stress-testing it from outside. The corrective is not more gamemaster intervention in a future PLAY; it is genuine outside perspective. Either:

- A second PLAY session where a non-builder stakeholder (a real early-adopter developer; a colleague unfamiliar with Plexus internals) exercises the system — ideally after any default-install fixes this cycle produces, to validate the fixes work for someone without builder context.
- Or: accept the partial fidelity of the first PLAY session and weight its conclusions accordingly.

This isn't a Grounding Reframe action per se, but a note for cycle sequencing — a second PLAY session later in this cycle (or the next) would materially strengthen the BUILD outputs this cycle produces.

---

## Feed-Forward Signals (categorized routing of PLAY field notes)

### From PLAY — routed to DISCOVER (update mode)

1. **Assumption inversion:** "A consumer who `load_spec`s + ingests against the default pipeline gets Plexus's value proposition." PLAY showed this is false for the default Homebrew build. **Hypothesis to confirm or refute:** the default-install failure is a defect, not staged onboarding (see Uncertainty 2).
2. **Value tension:** Easy-to-demo (pre-tagged, cross-pollination-rigged) vs. honest-to-demo (untagged, real default pipeline). Product discovery currently lacks language for this tension.
3. **Stakeholder-model refinement (Consumer Application Developer):** apps are lenses on material, not containers of it; writing emerges between apps; separation of consumers is about capture ergonomics, not data-ownership. **Hold as characterization, not definition.**
4. **Potential new value tension:** phenomenology-of-discovery constraint on lens output (see Uncertainty 1). Some consumers need structural predicates; others need named relationships. Product discovery should name this tension rather than resolving it prematurely.
5. **Journey addition:** feedback loop — query begets ingestion; lens signals are prompts, not endpoints; the graph is partly authored by responses to its own surfaces. **Hold as working hypothesis pending validation with untagged content.**

### From PLAY — routed to DECIDE (candidate ADRs)

6. **ADR candidate: Default pipeline truthfulness.** Pending Uncertainty 2 grounding. Shape of the ADR depends on the intent determination.
7. **ADR candidate: `created_at` property contract.** TemporalProximity reads `node.properties["created_at"]`; no adapter writes it; the `created_at` the adapter sets is in metadata. Either formalize the property contract or change the enrichment to read metadata.
8. **ADR candidate: Homebrew/release feature activation.** Whether `embeddings` belongs in `default = []` or `default = ["embeddings"]`. Coupled with ADR-6.
9. **ADR candidate (deferred pending grounding):** Lens grammar conventions around interpretive vs structural predicates. Shape depends on Uncertainty 1 grounding.

### From PLAY — routed to MODEL (if invariants or concepts need updating)

10. **Dimension mismatch** between content adapter (`structure`) and declarative spec (`semantic`). Both produce fragments but in different dimensions. Dimension concept may need clearer definition or convention documentation. Possible domain-model amendment to clarify dimension semantics. Not a new invariant; a clarification of existing concepts.

### From PLAY — routed to interaction specs

11. **Minimum-viable-spec pattern for useful signal.** Interaction-specs currently show lens translation without noting that a minimum-viable spec + untagged prose + default enrichments = no structure. Add pattern for "minimum-useful spec" that emits tagged concept nodes OR calls out the infrastructure preconditions.

---

## Context for Resumption

**For the next session:**

1. Invoke `/rdd:rdd` or `/rdd:discover` — will detect the new cycle from this cycle-status.
2. Read `docs/essays/reflections/field-notes.md` (raw observations from PLAY) and `docs/housekeeping/audits/susceptibility-snapshot-mcp-consumer-interaction-play.md` (the snapshot's Grounding Reframe recommendations) as the primary inputs.
3. Begin DISCOVER in **update mode** against `docs/product-discovery.md`. The task is:
   - Section-by-section review of product-discovery against PLAY findings
   - Add the assumption inversion (default-install experience) as an open question, not a resolved finding (per Uncertainty 2)
   - Add the easy-vs-honest demo value tension
   - Refine the Consumer Application Developer stakeholder model (apps as lenses, not containers)
   - Add the feedback-loop architecture to the consumer journey
   - Hold the phenomenology-of-discovery constraint as a candidate lens-design value, not a settled principle (per Uncertainty 1)

**Grounding spikes to run alongside DISCOVER (or during DECIDE):**

- Spike 1: Draft alternative Trellis lens specs (named vs structural) and query against each. Resolves Uncertainty 1.
- Spike 2: Audit Cargo.toml + git history + prior ADRs for the rationale behind `default = []`. Resolves Uncertainty 2.

Both spikes are small; both can run before or during DECIDE to inform the ADRs that follow.

**Optional second PLAY later in this cycle:** Inhabited by a non-builder stakeholder to validate the fixes land. This is a methodological strength but not a blocker.

**MCP session state:** `./session/play.db` from the prior cycle contains the test graph with `writing-context-test`, loaded specs for `trellis-fragment` and `carrel-source`, and ingested content. Keep or reset at the practitioner's discretion. Resetting would restore the blank-slate condition for the next exploratory session.

---

## Hypotheses Parked for Future Cycles

### Lens-as-grammar

**Surfaced:** 2026-04-17, during DISCOVER update-mode gate conversation.

**Framing (provisional, stated here so a future cycle can return to it):** A lens may be more than a vocabulary layer — it may be a *grammar* the graph speaks in. Vocabulary gives you terms; grammar gives you composition rules, query-expectation contracts, and syntax for what sentences the consumer's app can naturally form against the graph. Structural-predicate lenses compose into topological queries; named-relationship lenses compose into routing queries; the two are different grammatical registers, not just different naming conventions. "The graph literally speaks the consumer's language" is the striking statement of this framing.

**Why scoped out of this cycle:** The grammar framing is a hypothesis large enough to warrant its own research cycle. Pulling it into the current cycle would stall DECIDE on the default-install direction and dilute the lens-grammar ADR that needs to be written about the narrower named-vs-structural question. Better to return to it with room to research properly.

**Simpler version carried into current cycle:** Composition-shape awareness — lens translation rules shape *query patterns*, not just *vocabulary*. This is captured in the `lens` vocabulary entry in `product-discovery.md` as an extension, and will inform DECIDE's lens-grammar ADR without requiring a full grammar theory. The current cycle's ADR will address the narrower question (named vs. structural predicates, per-job within an app); the future cycle can address the deeper framing (what does it mean for the graph to speak a grammar, and what does that imply for lens-spec design, declaration format, query surface, and consumer onboarding).

**Precondition for the future cycle:** This current cycle's BUILD (or a follow-up cycle) needs to cross the tautology threshold in practice — untagged content ingested with either Ollama-backed embeddings or llm-orc semantic extraction active, producing structure the user didn't pre-encode. Lens-as-grammar research will need real emergent graph content to study, not pre-tagged demonstrations. Until that's in hand, the grammar cycle would be speculative in the same way PLAY1 was — reasoning about what ought to be rather than observing what is.

**Recommended opening belief-mapping question for the future cycle (logged 2026-04-20 per MODEL-phase susceptibility snapshot):** *"What would you need to believe for dimension assignment to be within scope for the grammar formalism?"* This cycle's MODEL phase reached the conclusion that dimension and lens are "different in kind" (node identity vs. edge meaning) based on a structural comparison that was not belief-mapped. The lens-as-grammar cycle inherits that conclusion as a scoping constraint unless its opening belief-mapping explicitly surfaces and tests it. If the grammar formalism ends up subsuming dimension assignment (i.e., dimensions as one grammatical register the graph speaks in alongside lens relationships), the separation may be an artifact of the current architecture rather than a necessary distinction. The opening question preserves the option to reconcile dimension and lens under the grammar framework without presuming the answer either way.
