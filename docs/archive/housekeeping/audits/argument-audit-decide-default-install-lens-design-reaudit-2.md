# Argument Audit Re-Audit Report (Pass 3)

**Audited documents:** `docs/decisions/038-release-binary-feature-profile.md`, `docs/decisions/041-lens-grammar-conventions.md`, `docs/decisions/042-dimension-extensibility-guidance.md` (revised); `docs/decisions/040-discovery-gap-trigger-sources.md` (spot-checked for cross-ADR consistency)
**Prior audits:** `docs/housekeeping/audits/argument-audit-decide-default-install-lens-design.md` (initial); `docs/housekeeping/audits/argument-audit-decide-default-install-lens-design-reaudit.md` (first re-audit — all 8 findings closed, 1 new P3 raised)
**Date:** 2026-04-20

---

## Section 1 — Verification of the Three Additions

### Addition 1 — ADR-041: Scope-condition paragraph (closing FI-2)

**Verdict: Clean. FI-2 is closed.**

The paragraph added under "What the analytical walk-through does NOT show" reads:

> The walk-through above uses `may_be_related` edges from co-occurrence over hand-tagged content as the illustrative source for both specs. Per PLAY field-note §"Crawl-step results and the tautology threshold," co-occurrence over user-supplied tags is below the tautology threshold — neither grammar can add discovery value to content the user already encoded in the tag overlap. The grammar distinction becomes practically consequential when the lens's source crosses the threshold (semantic extraction via an llm-orc ensemble, embedding-based similarity, or topological analysis over scale+time), where structure emerges that the spec author did not pre-encode. The convention endorsement below is meant to apply in that range; when the source is tautological, neither grammar changes the information content, and the choice reduces to stylistic preference.

The original FI-2 recommendation was "one sentence acknowledging the tautology scope condition." The revision added a full paragraph. No over-reach results. The paragraph does exactly what the finding required — it names the scope condition (tautological source), names the examples of non-tautological sources (semantic extraction, embedding, topological analysis), and characterizes what happens below the threshold (stylistic preference only). The full-paragraph treatment is appropriate here because the scope condition is load-bearing for the convention's practical relevance; a single sentence would have named the condition without anchoring it to the source material (field notes §"Crawl-step results") that established it. The paragraph adds no novel claims — it applies the field-note finding directly. No new logical gap introduced.

One observation worth noting but not flagging as a finding: the paragraph says the convention "is meant to apply in that range" (above the tautology threshold). The word "meant" is soft — the convention statement in the Decision section does not itself carry this scope condition, so a spec author reading the Decision in isolation will not see the threshold. The convention text reads "for jobs whose value proposition involves the user's interpretive work" without the tautology-threshold qualification. This is a documentation sequencing asymmetry (the scope condition lives in the analytical walk-through, not in the convention statement itself), but it does not create a logical error — the convention statement does not claim to apply below the threshold, it simply does not name the threshold. Given that the convention is explicitly "not a requirement" and the analytical walk-through is part of the same ADR, this asymmetry is tolerable. P3 if flagged; not flagged here because the first re-audit's R.3.1 already called out a similar sequencing issue in ADR-040 and the re-audit scope asked not to escalate minor P3s unless the new revisions introduced them.

### Addition 2 — ADR-041: Argument-grounds clarification subsection

**Verdict: Clean. The argument-grounds split is internally consistent and does not destabilize the convention statement.**

The subsection "Which argument carries which part of the decision" distinguishes:

- Composition-shape reasoning: load-bearing for the extensibility preference (structural predicates extend more naturally under future network-science additions).
- Phenomenology-of-discovery: load-bearing for the per-job split (structural for discovery-oriented, named for operational).

The subsection also states: "If a future cycle invalidates the phenomenology hypothesis, composition-shape reasoning survives but the convention's per-job phrasing needs revision — likely toward 'consider structural predicates when anticipating network-science extensions' without the job-type qualifier."

The argument-grounds split is logically sound. The two claims it makes are:

1. Composition-shape reasoning supports the extensibility preference independently of phenomenology. This is correct — the composition-shape analysis in the analytical walk-through shows that structural predicates produce more extensible vocabulary under network-science additions, and that observation holds regardless of whether phenomenology validates the per-job distinction.

2. Phenomenology is load-bearing for the per-job split specifically. This is also correct — the convention uses "discovery-oriented jobs" and "operational jobs" as the split criterion, and the rationale for that criterion traces to the hypothesis that interpretive jobs have different phenomenological requirements. Composition-shape reasoning alone would not produce a job-type qualifier.

No circular reasoning and no hidden premises. The subsection accurately represents what each argument does and what happens if the weaker argument (phenomenology) is later invalidated. The convention statement remains actionable — spec authors can apply it today using the per-job heuristic; the grounds-distinction is informational, not a condition on applicability.

The "Surface the grounds-distinction now so the future revision starts from visible structure rather than rediscovering which argument supports which part" justification is precise and appropriate.

### Addition 3 — ADR-038: Bundled-default-spec rejection paragraph

**Verdict: Clean. The rejection is internally consistent and correctly extends the honest-to-demo commitment.**

The paragraph added under Consequences Neutral reads:

> **Considered and rejected: bundling the worked-example spec as an install-time artifact.** [...] the bundled spec would produce no effect without llm-orc and a configured embedding provider, so shipping it would re-create the release-configuration truthfulness gap at a different layer — the user would see a "default spec" that appears to come with the binary but in fact does nothing unless separately-installed infrastructure is present.

The rejection logic holds. The core commitment of ADR-038 is honest-to-demo at the default-install layer — no artifact should create a default-behavior expectation that the binary cannot fulfill without separate infrastructure. A bundled spec that auto-loads at install time but silently does nothing (because llm-orc is not present) would violate this commitment in the same way the prior "four enrichments active" count violated it: the artifact appears to confer a capability that in fact depends on separately-installed infrastructure. The rejection is an application of the same principle the ADR was written to enforce, and it extends correctly to this new surface.

The alternative being rejected (bundled-at-default-load-path) is distinct from the deliverable the ADR commits to (`examples/specs/embedding-activation.yaml` as a documentation artifact). The paragraph makes this distinction explicit ("a documentation artifact referenced from onboarding material, not an install-time artifact auto-loaded on the user's behalf"). No ambiguity about what "worked-example spec" means remains after this paragraph.

The provenance is correctly attributed ("considered at the DECIDE gate (2026-04-21)"). Note the gate date (2026-04-21) is one day after the audit date (2026-04-20) — this is a minor temporal inconsistency in the provenance field, but it reflects gate-conversation timing and does not affect the ADR's reasoning. Not flagged as a finding.

---

## Section 2 — New Argument-Audit Issues

**No new P1 or P2 issues.**

**No new P3 issues** attributable to the three additions. The first re-audit's R.3.1 (ADR-040's "converge on one registration" phrasing is mechanically underspecified) remains open from the prior pass. The current revisions do not touch ADR-040 and do not interact with that finding.

---

## Section 3 — Cross-ADR Consistency Check

### ADR-038 and ADR-040

**Consistent.** The bundled-default-spec rejection paragraph in ADR-038 (new) does not interact with ADR-040's trigger-sources content. Both ADRs converge on the same account of the default-build enrichment state: CoOccurrence active, TemporalProximity active after ADR-039, DiscoveryGap registered-but-idle, EmbeddingSimilarity absent. ADR-040 §Decision still states "In the default Homebrew build, there is no built-in producer of `similar_to` — the lean baseline is CoOccurrence-only" — this phrasing is still slightly under-inclusive relative to ADR-038's two-enrichment count (TemporalProximity is also active after ADR-039), but as the first re-audit noted, this is a documentation-emphasis choice in ADR-040's DiscoveryGap-specific narrative, not a contradiction. ADR-040 was not touched in this pass; the first re-audit cleared it; it remains clear.

ADR-038's bundled-spec rejection does not imply any change to ADR-040's statement that "Solving it with a Plexus-side default (i.e., making a particular spec ship with the binary) is out of scope for this cycle." The two ADRs are consistent: ADR-040 defers the problem; ADR-038's new paragraph forecloses one specific resolution path (bundled install-time artifact) and names the reason.

### ADR-038 and the interaction specs

The addition does not conflict with anything in `docs/interaction-specs.md` that would have described the worked-example spec as an install-time artifact. ADR-038's positioning of the spec as a documentation artifact at `examples/specs/embedding-activation.yaml` is the only stated shape. No contradiction possible.

### ADR-041's convention statement after the argument-grounds clarification

The convention statement ("spec authors should consider structural-predicate output relationships... for jobs whose value proposition involves the user's interpretive work") is unchanged in the Decision section. The argument-grounds clarification lives in a subsection following the hypothesis-naming paragraph. The convention reads as actionable before and after the clarification. The clarification adds conditional stability information ("if phenomenology is invalidated, the per-job phrasing needs revision") without making the convention's current application conditional on the phenomenology hypothesis being confirmed. A spec author applying the convention today does not need to resolve the hypothesis first. The convention is stable.

---

## Summary

All three additions land cleanly:

- **ADR-041 scope-condition paragraph** — closes FI-2 as asked; does not over-reach.
- **ADR-041 argument-grounds clarification** — correctly splits the two arguments, is internally consistent, and leaves the convention actionable.
- **ADR-038 bundled-default-spec rejection** — correctly extends the honest-to-demo commitment to the install-time-artifact surface; introduces no new reasoning.

No new argument or framing issues at any priority level. The open item from the first re-audit (R.3.1 — ADR-040 "converge on one registration" phrasing) was not touched and remains as it was. FI-1 and FI-3 from the initial audit were not reopened by any of the three revisions.
