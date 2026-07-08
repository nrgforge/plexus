# Susceptibility Snapshot

**Phase evaluated:** ARCHITECT (light-touch pass) — Default-Install Experience and Lens Design Principles cycle
**Artifact produced:** `docs/system-design.md` v1.3 (Amendment 7); `docs/roadmap.md` (regenerated); `docs/ORIENTATION.md` (Current State refreshed); `docs/cycle-status.md` (ARCHITECT row updated)
**Date:** 2026-04-22
**Prior snapshots:**
- `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-decide.md` (DECIDE, same cycle)
- `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-model.md` (MODEL, same cycle)
- `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-discover.md` (DISCOVER, same cycle)

---

## Observed Signals

| Signal | Strength | Trajectory | Notes |
|--------|----------|------------|-------|
| Assertion density | Ambiguous | Stable from prior snapshot | Agent-side assertions were high-density throughout — Amendment 7 content, roadmap WP decomposition, and ORIENTATION.md Current State rewrite produced without mid-pass checkpoints or staged options. User assertions were minimal: three-word phase selection, single-exchange scope confirmation, implied debt-triage confirmation. |
| Solution-space narrowing | Clear (scoped) | Stable | Three candidate scopes were explicitly staged at phase open (a Minimal / b Targeted / c Full). User selected (b) without rationale or interrogation of alternatives. Within (b), all sub-choices — amendment wording, WP decomposition shape, fitness criterion phrasing — were made unilaterally by the agent. No sub-options were staged after the top-level scope choice. |
| Framing adoption | Clear | Stable — consistent with cross-phase pattern | The "deployment class" framing from ADR-038 and the "source-agnostic trigger" framing from ADR-040 were inherited into system-design.md's architectural drivers and enrichment narrative without re-examination. The adoption is mechanically traceable and the inherited framings are internally consistent. The mechanism — inheritance without examination of alternative architectural axes — is the signal. Fifth consecutive phase recording this dynamic. |
| Confidence markers | Ambiguous | Stable | Amendment log entry states "No module boundaries changed, no new dependency edges, no responsibility matrix changes" in declarative form. The retrofit reconnaissance (file reads of `discovery_gap.rs` and `builder.rs`, grep across enrichment loop) supports this claim, but the claim is a negative (what does NOT need to change) that reconnaissance can partially, not fully, verify. The fitness criterion "verified by acceptance tests under `default = []`" is phrased as present-tense verified; the acceptance tests were not run during the ARCHITECT pass. |
| Alternative engagement | Clear (partial) | Declining from prior snapshot | ARCHITECT skill steps 4–7 (Module Decomposition, Responsibility Allocation, Dependency Graph, Integration Contracts) were compressed to near-zero by the "light-touch" scope decision. This is the correct scope decision, not a short-circuit — but it means none of those rubric axes received any engagement. Additionally: within scope item (1), the agent produced one wording for the architectural drivers table without staging alternatives. The DECIDE-snapshot's "Candidates Considered" discipline note (Advisory item 2) did not propagate to within-amendment sub-choices. |
| Embedded conclusions | Clear (one specific instance) | Present, moderate severity | The fitness criterion "Default binary does not break without llm-orc or consumer-authored spec" is presented as a testable condition verified by acceptance tests. The test coverage claim is aspirational: no check was performed during ARCHITECT to verify that the three deployment shapes named in the new subsection (in-process, external-consumer-spec, graceful-idle baseline) each have acceptance-test coverage. The new subsection describes behaviors that may be partially covered by existing tests but were not confirmed to be fully covered during this pass. The criterion reads as grounded at artifact-production time; it is actually asserted pending BUILD's test-writing. |

---

## Interpretation

### What the signals collectively suggest

This ARCHITECT pass was scoped explicitly as light-touch from the beginning. The scope decision was the user's and it was correct given the artifact state: the module decomposition, responsibility allocation, and dependency graph from prior cycles are accurate and did not need re-examination. The four scope items (deployment-class naming, DiscoveryGap trigger story, ORIENTATION.md refresh, BUILD gate-surfacing) are well-matched to what the system-design actually needed at this boundary.

Given that scope, most of the declining-engagement signals are not susceptibility artifacts — they are the intended consequence of choosing a targeted amendment pass over a full architectural re-examination. The evaluator's primary job at this boundary is to distinguish signals that are scope-appropriate from signals that encode unexamined assumptions downstream phases will inherit.

Two patterns merit evaluation in that frame.

**Pattern A — The DECIDE snapshot's Grounding Reframe was honored: the bundled-default-spec option was staged and rejected on record.**

The DECIDE snapshot's primary Grounding Reframe recommendation was that the bundled-default-spec option (placing a reference spec at an install-time path so consumers encounter a working activation path without authoring a spec) should be staged and rejected on record before ARCHITECT. The cycle-status records this was actioned: the option was staged at the DECIDE gate, rejected on the specific ground that a bundled spec producing no effect without llm-orc would re-create the truthfulness gap at a different layer, and the rejection is recorded in ADR-038. This is a clean outcome — the option is no longer invisible.

**Pattern B — Framing adoption from DECIDE is structural, not incidental.**

The system-design now uses "deployment class" as the primary architectural axis for the embedding backend section. This framing was derived from ADR-038. ARCHITECT did not examine whether "deployment class" is the right primary axis for the architectural drivers table — alternatives (e.g., "activation mechanism," "consumer authoring surface," "runtime dependency") were not staged. The adoption is substantively defensible: deployment class maps clearly to the two consumer populations the cycle identified. The concern is not that the framing is wrong; it is that it arrived from DECIDE by inheritance rather than by independent architectural examination.

For system-design.md, framings in the architectural drivers table propagate into BUILD's understanding of what the system is optimizing for. A future cycle reading the v1.3 drivers will encounter "deployment class" as the load-bearing concept without evidence that the concept was examined at the architecture layer versus inherited from ADR-naming conventions.

This is the same pattern the MODEL snapshot named for "different in kind" (dimension-vs-lens inference) and the DISCOVER snapshot named for "grounding examples, not build targets." In all three cases, the framing was substantively defensible and adopted without examination. In all three cases the adoption was correctly flagged as a pattern rather than a per-instance correction. This is the fifth consecutive phase where the pattern appears.

**Pattern C — The fitness criterion's verification gap is the one concrete unresolved item.**

The new fitness criterion ("Default binary does not break without llm-orc or consumer-authored spec") is scoped to acceptance tests under `default = []`. The ARCHITECT pass did not confirm those tests cover the three deployment shapes in the new subsection. The three shapes are: (i) in-process `features = ["embeddings"]`; (ii) default binary with consumer-declared external enrichment loaded via `load_spec`; (iii) neither present — graceful-idle baseline. Shape (iii) is the primary claim (the lean binary should not break). Shapes (i) and (ii) are activation paths, not baseline assertions.

The existing acceptance test suite almost certainly covers shape (iii) — ingest succeeds, CoOccurrence fires, queries return expected results, DiscoveryGap and EmbeddingSimilarity produce no edges. These behaviors were tested before the ARCHITECT pass. The fitness criterion is therefore likely accurate for shape (iii). Shapes (i) and (ii) are covered by WP-A and WP-D respectively in the BUILD roadmap. The criterion as written applies to all three shapes; coverage is complete for one, pending for two.

This is a scoped gap, not a broad framing failure. BUILD will encounter it naturally when writing WP-A and WP-D test coverage.

**Earned confidence vs. sycophantic reinforcement:**

The retrofit reconnaissance (file reads and grep) was genuine verification work. The claim that ADR-040's trigger-coupling story does not cross module boundaries was confirmed against code (`src/adapter/enrichments/discovery_gap.rs`, `src/adapter/pipeline/builder.rs`), not asserted. The deployment-class framing accurately maps to the ADR-038 decision and to the code's actual behavior. The roadmap's WP decomposition is well-grounded in the conformance scan's seven debt items, with risks named per WP (low/medium/moderate) and the highest-risk quality bar (worked-example spec crossing the tautology threshold) explicitly named in WP-D.

The susceptibility concern at this boundary is not about the artifact's correctness. It is about: (1) the framing inheritance pattern's fifth consecutive appearance; (2) the fitness criterion's aspirational test-coverage claim; and (3) the "Candidates Considered" discipline not propagating below the top-level scope choice.

---

## Recommendation

**No Grounding Reframe warranted at the ARCHITECT → BUILD boundary.** Two feed-forward items for BUILD entry that do not require blocking.

### Reasoning for no Grounding Reframe

The three criteria for a Grounding Reframe are: (a) multiple signals converging on a narrowing pattern without earned confidence; (b) phase position in the sycophancy gradient amplifying risk; (c) the artifact encoding decisions that downstream phases will inherit.

Criterion (b) is attenuated at ARCHITECT. BUILD is the most resistant phase in the sycophancy gradient — it produces code that either passes tests or does not, and the conformance scan has already surfaced the specific debt items BUILD must address. The framing inheritance (deployment class, source-agnostic trigger) is encoded in system-design.md, which BUILD will read but not substantially amend. The fitness criterion's verification gap will surface naturally in BUILD's test-writing pass.

Criterion (a) is partially present — framing adoption is the fifth consecutive occurrence and alternative engagement within scope was near-zero. But the ARCHITECT scope was correctly constrained, and within that scope the agent verified claims rather than asserting them. This is consistent with earned confidence at a late phase where prior cycle work did the substantive design examination.

The DECIDE snapshot's Grounding Reframe was honored — the bundled-spec option was staged, rejected, and recorded. That specific gap is closed.

---

### Feed-Forward Item 1: Fitness criterion test coverage should be confirmed at BUILD entry (not a blocker)

The fitness criterion "Default binary does not break without llm-orc or consumer-authored spec" is aspirational for deployment shapes (i) and (ii). BUILD should confirm at the start of the WP-A and WP-D passes that coverage exists or name it as a gap to close. This does not require a new ADR or architectural change — a single sentence in the BUILD gate reflection or a test audit is sufficient. The risk if left unconfirmed: the criterion remains in system-design.md as a testable condition that BUILD has implicitly claimed is covered, which may create false confidence during a future conformance scan.

### Feed-Forward Item 2: The "Candidates Considered" discipline did not propagate below the top-level scope choice (process note)

The MODEL-gate-introduced corrective for the swift-adoption pattern — staging compressed options explicitly at phase boundaries — was applied at the top level of ARCHITECT (three scope candidates staged). It was not applied at the sub-choice level (amendment driver wording, fitness criterion phrasing, roadmap WP decomposition shape). This is consistent with the pattern observed in PLAY, DISCOVER, MODEL, and DECIDE: the discipline is applied at scope-selection moments but not at artifact-production sub-choices within a scope.

For BUILD, this means the roadmap's WP descriptions represent the agent's unilateral decomposition of the DECIDE artifacts into implementation units. BUILD may decompose differently — particularly WP-D's documentation deliverables, where the ordering (README → worked-example spec → spec-author docs) and the "implied logic" dependency framing are judgment calls. BUILD should treat the WP descriptions as a starting map, not a binding decomposition, and verify the ordering at BUILD entry against the actual code state.

This is a process note, not a blocking concern. No artifact produced in this ARCHITECT pass encodes a decision BUILD cannot locally revise.

---

## Notes on Role Dynamics

### The fifth-consecutive-phase framing adoption

Five consecutive phases (PLAY, DISCOVER, MODEL, DECIDE, ARCHITECT) have recorded the same dynamic: the user or a prior agent offers a substantive framing that is plausibly correct; the current agent adopts it rapidly; intermediate options go unexamined. Per the MODEL snapshot's structural note, the corrective is not harder pushback on substantively correct framings — it is explicit staging of compressed options at phase-gate moments.

At this ARCHITECT boundary, the framing inherited from DECIDE ("deployment class" as the primary architectural axis) is defensible and internally consistent. The risk the five-phase accumulation poses is that a future reader of system-design.md v1.3 encounters the deployment-class framing as architectural doctrine without the examination record that would let them evaluate it critically. The remedy at this phase is documentation, not revision: Amendment 7's provenance cites ADR-038 and ADR-040, which carry their own argument-audit records. A future evaluator has a clear trace to the framing's origin.

### Practitioner-as-sole-stakeholder dynamic persists

The partial-fidelity inhabitation limitation from prior snapshots continues to apply to WP-D's deliverables. ADR-038's worked-example spec and onboarding material need to serve a consumer who cannot author a spec fluently — a perspective the cycle's sole stakeholder cannot fully inhabit. The DECIDE snapshot named a second PLAY session with a non-builder stakeholder as an optional strengthening action; this note confirms its continued relevance. WP-D's quality bar ("a reader can trace the activation path from README to worked example to ingest-and-query in one pass") is a correct criterion; whether the actual deliverables meet it is easier to assess if someone who hasn't authored the spec attempts the path.
