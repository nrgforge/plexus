# Active RDD Cycle: Default-Install Experience and Lens Design Principles

**Started:** 2026-04-17
**Current phase:** BUILD (next) — DISCOVER update complete 2026-04-17; MODEL light-touch pass complete 2026-04-20; DECIDE complete 2026-04-21; ARCHITECT light-touch pass complete 2026-04-22
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
| DECIDE | ✅ Complete | ADRs 038–042 at `docs/decisions/`; scenarios at `docs/scenarios/038-042-default-install-lens-design.md`; interaction-specs updates in `docs/interaction-specs.md` (Consumer Application Developer stakeholder, four new tasks); argument audit reports at `docs/housekeeping/audits/argument-audit-decide-default-install-lens-design*.md`; conformance scan at `docs/housekeeping/audits/conformance-scan-decide-default-install-lens-design.md`; susceptibility snapshot at `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-decide.md`; gate reflection note at `docs/housekeeping/gates/default-install-lens-design-decide-gate.md` | Five ADRs: (038) release-binary feature profile — `default = []` stays, no Rust code path to llm-orc, consumer activates embedding via declarative adapter spec; (039) `created_at` property contract — `node.properties["created_at"]` authoritative, ISO-8601 UTC, adapters write + enrichment reads; (040) DiscoveryGap trigger sources — source-agnostic, no algorithm broadening, multiple parameterizations; (041) lens grammar conventions — structural-predicate endorsement as convention (not requirement) for discovery-oriented jobs, per-job not per-app, phenomenology held as hypothesis with argument-grounds split made visible; (042) dimension extensibility guidance — option (ii)+(iii) documentation + syntactic validation, option (i) rejected this cycle with empirical escalation signal named as BUILD opportunity. Grounding Reframe honored: bundled-default-spec option staged at gate, rejected on specific ground (would re-create truthfulness gap at a different layer because bundled spec produces no effect without llm-orc), rejection recorded in ADR-038. Three user-applied clarifying additions during gate: tautology-threshold scope condition (ADR-041), argument-grounds split (ADR-041), empirical escalation signal (ADR-042). No invariant changes — no backward propagation triggered. Conformance scan found 7 debt items (3 Structural, 2 Gap, 2 Drift) — D-01–D-04 a coordinated four-site fix for `created_at`; D-06 highest-consequence (`resolve_dimension` is an exclusive allowlist, needs conversion to syntactic validator); D-07 the known `graph/node.rs:10` doc drift. |
| ARCHITECT | ✅ Complete | `docs/system-design.md` v1.3 (Amendment 7); `docs/roadmap.md` regenerated (5 WPs, no hard deps); `docs/ORIENTATION.md` Current State refreshed; susceptibility snapshot at `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-architect.md`; gate reflection note at `docs/housekeeping/gates/default-install-lens-design-architect-gate.md` | Light-touch pass. Architectural drivers reshaped to name both embedding backends as first-class per deployment class (ADR-026 + ADR-038 co-cited); new "Embedding Backend Deployment Classes" subsection enumerates three deployment shapes (in-process under feature flag; default binary + consumer-declared external enrichment; neither-present graceful-idle baseline); DiscoveryGap trigger-source contract made explicit in Core Enrichment Algorithms per ADR-040; new fitness criterion: "Default binary does not break without llm-orc or consumer-authored spec." Confirmed: ADR-040 is naming/documentation concern, not structural — the enrichment's parameterization and the enrichment loop's re-entry semantics (Invariant 49) already support source-agnostic reception. No module boundaries changed, no dependency edges added. Roadmap resets to WP-A for new cycle with Completed Work Log preserving MCP cycle. ADR status discrepancy resolved: cycle-status.md bullet-4 note was stale — ADRs 038–042 are Accepted (status flipped at DECIDE gate close 2026-04-21). |
| BUILD | ▶ Next | — | Five WPs ready (see `docs/roadmap.md`). WP-A: `created_at` property contract coordinated four-site fix (ADR-039, D-01..D-04). WP-B: Dimension extensibility — `resolve_dimension` + `validate_spec` refactor (ADR-042, D-06). WP-C: Developer-facing docstring drift (D-05 + D-07). WP-D: ADR-038 onboarding deliverables (README + worked-example spec + spec-author documentation + lens grammar convention). WP-E (optional): silent-idle debug instrumentation for dimension divergence. No hard deps between WPs. Recommended order: A → C → B → D. |
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

## Feed-Forward Signals from DECIDE → ARCHITECT (2026-04-21)

1. **Documentation deliverables are load-bearing for ADR-038's reframing.** ADR-038's "positive decision, not defect-by-omission" stance holds only if BUILD produces the README updates, worked-example spec at `examples/specs/embedding-activation.yaml`, and onboarding material naming the lean baseline. ARCHITECT should weight these deliverables when scoping BUILD; if they are delayed or weak, the defect-by-omission framing reasserts.
2. **Worked-example spec quality bar.** The spec must cross the tautology threshold — demonstrate `similar_to` edges emerging over content the author did not pre-encode with overlapping tags. Embedding-over-untagged-prose is the target shape. A pre-tagged worked example would repeat the field-notes-flagged tautology failure mode. ARCHITECT should reference this requirement when sizing BUILD.
3. **Conformance debt D-01 through D-04 is a coordinated four-site fix, not four independent fixes.** Three producer sites + one consumer parser must move together as a single `fix:` commit. ARCHITECT does not need to do this work; the coupling should be called out in the ARCHITECT brief so BUILD treats it as atomic.
4. **Conformance debt D-06 is structural.** `resolve_dimension`'s exclusive-allowlist contradicts ADR-042's extensibility promise. Converting it to a syntactic well-formedness check + adding validation to `validate_spec()` for fail-fast behavior per Invariant 60 is medium-scope but bounded. ARCHITECT should confirm this does not introduce new module boundaries or trait changes.
5. **Optional BUILD-phase instrumentation:** silent-idle detection at spec load time. Named in ADR-042 as an empirical escalation signal opportunity. Not required; ARCHITECT surfaces if relevant to a logging/observability surface already being touched in architecture updates.
6. **ADR-040's "no algorithm broadening" is a narrowness commitment, not a structural change.** The enrichment stays the same shape; what changes is how its activation story is documented across the ADR chain. ARCHITECT should confirm no module-boundary crossing.
7. **Lens grammar ADR (041) does not require ARCHITECT action** — convention lives in product discovery, spec-author documentation, and interaction specs. No system-design touch required unless the lens module's documentation benefits from naming the convention.
8. **Carry "Candidates Considered" stylistic discipline forward.** Per susceptibility snapshot Advisory item 2, the MODEL-gate-introduced corrective for the swift-adoption pattern should continue into ARCHITECT. Stage compressed options explicitly.

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

**For the next session (ARCHITECT):**

1. Invoke `/rdd:rdd` or `/rdd:architect` — the orchestrator will detect the cycle state from this document and resume at ARCHITECT.
2. Primary inputs to ARCHITECT:
   - `docs/decisions/038-release-binary-feature-profile.md` through `042-dimension-extensibility-guidance.md` — the five accepted-pending decisions this phase extends.
   - `docs/housekeeping/gates/default-install-lens-design-decide-gate.md` — the gate reflection note, which captures settled premises, open questions, and specific commitments carried forward.
   - `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-decide.md` — the susceptibility snapshot at the DECIDE → ARCHITECT boundary, which recommends carrying "Candidates Considered" discipline forward and notes a partial-fidelity inhabitation concern.
   - `docs/housekeeping/audits/conformance-scan-decide-default-install-lens-design.md` — 7 debt items, with which map to BUILD scenarios vs. pure cleanup already labeled.
   - `docs/system-design.md` (v1.2) — the document ARCHITECT primarily edits.

3. ARCHITECT's planned scope (light-touch pass):
   - Name both embedding backends as first-class in system-design, per deployment class: Homebrew/CLI default = llm-orc-driven via consumer adapter spec; library-consumer with `features = ["embeddings"]` = in-process fastembed. Cite ADR-038.
   - Verify ADR-040's DiscoveryGap trigger-coupling story does not cross module boundaries — confirm it is a naming/documentation concern at the enrichment level, not a structural architectural change. Document the confirmation briefly in system-design.
   - Regenerate `docs/ORIENTATION.md` if any drift is detected after the system-design update.
   - Surface gate-carried open questions (empirical escalation signal for ADR-042; phenomenology hypothesis for ADR-041; documentation-deliverables contingency for ADR-038) in the ARCHITECT brief so BUILD inherits them visibly.

4. ADR status note for ARCHITECT: all five ADRs are currently **Proposed**. ARCHITECT does not flip them to Accepted — that happens when the user accepts the DECIDE output. The user may want to flip ADR statuses to Accepted as part of the ARCHITECT phase's opening action, or defer until ARCHITECT completes. Ask at ARCHITECT opening.

5. No invariant changes landed in DECIDE; no backward propagation required.

**Optional second PLAY later in this cycle:** Inhabited by a non-builder stakeholder to validate the fixes land. Carried from DISCOVER; still a methodological strength, still not a blocker.

**MCP session state:** `./session/play.db` from the prior PLAY session remains untouched. Not relevant to ARCHITECT; will re-become relevant if a second PLAY session runs after BUILD.

---

## Hypotheses Parked for Future Cycles

### Lens-as-grammar

**Surfaced:** 2026-04-17, during DISCOVER update-mode gate conversation.

**Framing (provisional, stated here so a future cycle can return to it):** A lens may be more than a vocabulary layer — it may be a *grammar* the graph speaks in. Vocabulary gives you terms; grammar gives you composition rules, query-expectation contracts, and syntax for what sentences the consumer's app can naturally form against the graph. Structural-predicate lenses compose into topological queries; named-relationship lenses compose into routing queries; the two are different grammatical registers, not just different naming conventions. "The graph literally speaks the consumer's language" is the striking statement of this framing.

**Why scoped out of this cycle:** The grammar framing is a hypothesis large enough to warrant its own research cycle. Pulling it into the current cycle would stall DECIDE on the default-install direction and dilute the lens-grammar ADR that needs to be written about the narrower named-vs-structural question. Better to return to it with room to research properly.

**Simpler version carried into current cycle:** Composition-shape awareness — lens translation rules shape *query patterns*, not just *vocabulary*. This is captured in the `lens` vocabulary entry in `product-discovery.md` as an extension, and will inform DECIDE's lens-grammar ADR without requiring a full grammar theory. The current cycle's ADR will address the narrower question (named vs. structural predicates, per-job within an app); the future cycle can address the deeper framing (what does it mean for the graph to speak a grammar, and what does that imply for lens-spec design, declaration format, query surface, and consumer onboarding).

**Precondition for the future cycle:** This current cycle's BUILD (or a follow-up cycle) needs to cross the tautology threshold in practice — untagged content ingested with either Ollama-backed embeddings or llm-orc semantic extraction active, producing structure the user didn't pre-encode. Lens-as-grammar research will need real emergent graph content to study, not pre-tagged demonstrations. Until that's in hand, the grammar cycle would be speculative in the same way PLAY1 was — reasoning about what ought to be rather than observing what is.

**Recommended opening belief-mapping question for the future cycle (logged 2026-04-20 per MODEL-phase susceptibility snapshot):** *"What would you need to believe for dimension assignment to be within scope for the grammar formalism?"* This cycle's MODEL phase reached the conclusion that dimension and lens are "different in kind" (node identity vs. edge meaning) based on a structural comparison that was not belief-mapped. The lens-as-grammar cycle inherits that conclusion as a scoping constraint unless its opening belief-mapping explicitly surfaces and tests it. If the grammar formalism ends up subsuming dimension assignment (i.e., dimensions as one grammatical register the graph speaks in alongside lens relationships), the separation may be an artifact of the current architecture rather than a necessary distinction. The opening question preserves the option to reconcile dimension and lens under the grammar framework without presuming the answer either way.
