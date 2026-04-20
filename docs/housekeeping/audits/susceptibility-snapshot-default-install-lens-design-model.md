# Susceptibility Snapshot

**Phase evaluated:** MODEL — Default-Install Experience and Lens Design Principles cycle
**Artifact produced:** `docs/domain-model.md` (light-touch MODEL pass, 2026-04-18/2026-04-19)
**Date:** 2026-04-19
**Prior snapshots:**
- `docs/housekeeping/audits/susceptibility-snapshot-default-install-lens-design-discover.md` (DISCOVER, same cycle)
- `docs/housekeeping/audits/susceptibility-snapshot-mcp-consumer-interaction-play.md` (PLAY, prior cycle)

---

## Observed Signals

| Signal | Strength | Trajectory | Notes |
|--------|----------|------------|-------|
| Assertion density | Ambiguous | Declining from prior snapshot | User assertions were substantive and load-bearing (extensibility as domain-agnostic stance; "I don't want to presume to know what's more important"), not decorative conclusions. However the initial per-dimension-intent draft came from the agent, not the user — then collapsed quickly on challenge. Assertion density in the artifact is low: OQ 15 is framed as a constraint-plus-range rather than a claim; the Dimension entry avoids asserting which dimensions are canonical. |
| Solution-space narrowing | Clear (partial) | New signal relative to prior snapshot | Per-dimension intent was dropped entirely from the Dimension entry; the intermediate framing ("ship conventions documented here for reference, extensible by consumers") was not explicitly considered and rejected — it went unengaged. The four options in the initial OQ 15 draft were compressed to an extensibility constraint plus open range, with three specific options (schema validation rejecting unknowns, node-type-to-dimension mapping, warn-on-divergence) unnamed in the final artifact. The narrowing was user-directed but the intermediate option and the three stricter options are now invisible to DECIDE without recovery. |
| Framing adoption | Clear | Stable — consistent with cross-phase pattern | Agent adopted the "minimal version" framing rapidly after the user's "dial complexity without complecting" request. No recorded counterargument or intermediate candidate examined. This is the same swift-adoption pattern flagged in the DISCOVER snapshot (note on role dynamics) and the PLAY snapshot before it. Three successive snapshots now record the same dynamic. The adopted framing is substantively correct; the mechanism is the signal. |
| Confidence markers | Absent | Stable from prior snapshot | The domain-model artifact uses no confidence inflation language in the new content. The Dimension entry is explicitly hedged ("Softened 2026-04-18 from enumeration-in-glossary to adapter-level conventions"). OQ 15 is framed as open to DECIDE. Amendment 9 is classified as "concept softening, not invariant change" — a deliberately restrained characterization. |
| Alternative engagement | Ambiguous | Mixed | Dimension-vs-lens structural comparison was performed analytically (agent produced a comparison table: node property vs. edge namespace; exclusive vs. additive; write-time vs. enrichment-time). The comparison was responsive to a user question and grounded in architecture. However the conclusion from that comparison — that dimension and lens are "different in kind" — was not belief-mapped. The specific claim that dimension concerns "node identity" and lens concerns "edge meaning" shaped the final entry without stress-testing whether that framing forecloses a legitimate future unification. |
| Embedded conclusions | Clear (one instance) | New signal relative to prior snapshot | The "different in kind" framing for dimension-vs-lens arrived as an agent conclusion at an artifact-production moment (the dimension entry was being drafted). The framing has architectural downstream consequences: it influences whether a future cycle on lens-as-grammar should consider unifying the dimension concept into the lens grammar, or whether they remain permanently separate concerns. The claim appears to be structurally defensible but was not belief-mapped. |

---

## Interpretation

### What the signals collectively suggest

The MODEL pass was genuinely light-touch: no new invariants, no existing invariants amended, one concept entry softened, one open question added and handed to DECIDE. The artifact is substantially better than a pre-MODEL state would have been — it resolves a real ambiguity (dimension mismatch from feed-forward item 10), grounds the claim that dimension membership is load-bearing by citing code (`src/query/find.rs`), and honors the Grounding Reframe from the DISCOVER snapshot (feedback-loop architecture not promoted to invariant; cross-pollination flywheel not codified as a structural design obligation).

The prior snapshots' Grounding Reframe recommendations were observed: MODEL did not vocabulary-ize "grammar" terms, did not promote the querying-begets-ingestion feedback loop to invariant, and did not enumerate lens-as-grammar concepts into the glossary. These are clean outcomes.

Two specific patterns warrant examination as a set.

**Pattern A — The swift-adoption pattern has now been recorded in three successive snapshots.**

The PLAY snapshot noted it as a signal. The DISCOVER snapshot elevated it from "signal" to "consistent dynamic" and flagged it for future snapshot evaluators. This MODEL snapshot confirms it is a stable cross-phase pattern, not an artifact of any single phase. The mechanism: user offers a framing or a scope constraint; agent adopts it rapidly; the intermediate option space is not staged or examined. In all three instances the adopted framing was substantively defensible. The risk is not in any single adoption; it is in the cumulative compression of the option space across a cycle. By the time DECIDE receives the artifacts, the intermediate design positions that were never considered have become invisible — they cannot be reviewed because they were not recorded as having been considered and rejected.

For this MODEL pass specifically: the intermediate Dimension entry framing ("document shipped conventions here, but note extensibility") was neither proposed explicitly nor rejected on record. DECIDE will work from the minimal extensibility-aware entry. Whether the intermediate framing would have been the better DECIDE input is unknowable — but the decision was made implicitly, by not surfacing the alternative, rather than explicitly, by examining and rejecting it.

**Pattern B — The dimension-vs-lens "different in kind" claim requires belief-mapping before the lens-as-grammar cycle returns to it.**

The structural comparison the agent performed was genuine analytical work: dimension is exclusive, write-time, on nodes, declaring identity facets; lens is additive, enrichment-time, on edges, providing vocabulary translation. These are real architectural differences. The inference from those differences to "different in kind, not candidates for unification" is the step that was not examined.

The lens-as-grammar parked hypothesis (cycle-status §Hypotheses Parked for Future Cycles) is precisely about whether lens translation rules shape compositional structure, not just vocabulary. If that cycle ever asks whether "what dimension a node is in" could be expressed as a first-class lens grammar concern — as a different kind of signal in the grammar than an edge translation — the "different in kind" framing embedded in the current Dimension entry will resist that inquiry. The entry now reads: "Dimension membership is load-bearing for enrichments (Invariant 50) and query filtering." This is accurate. What it does not address is whether, from the grammar perspective, dimension assignment is itself a lens-expressible concern — a question the parked cycle would need to raise. The current framing does not foreclose this, but it also does not name the open question, which means it will not be visible to the future evaluator unless explicitly flagged.

**Earned confidence vs. sycophantic reinforcement:**

The factual grounding work (code verification of `find_nodes` dimension filter; provenance trace to Essay 03; ADR citation-drift catch on ADR-009's comment) is genuine engagement. The agent verified claims before encoding them, challenged its own initial draft, and produced a structurally defensible minimal entry. The narrow Grounding Reframe below is not about the artifact's correctness; it is about the visibility of the option space and one unexamined inference at an artifact-production moment.

---

## Recommendation

**Grounding Reframe recommended — narrow scope, two specific items. One is a DECIDE-entry action; one is a future-cycle flag.**

### Item 1: Intermediate Dimension entry option should be visible to DECIDE

**The specific gap:**

The option "document shipped conventions inside the Dimension glossary entry, but frame them explicitly as adapter conventions rather than canonical enumerations" was not staged as an alternative during the MODEL phase. The final entry went from enumeration-in-glossary (initial draft) to minimal extensibility-only framing (shipped). The intermediate position — name the conventions in the entry, hedge them explicitly as adapter-level, maintain extensibility — was not considered on record.

**Why this matters for DECIDE:**

DECIDE's work on OQ 15 (how to guide spec authors toward appropriate dimension choice) may conclude that documentation-only guidance is insufficient, and that the glossary entry itself is a useful place to surface the canonical conventions with explicit framing. DECIDE would be reinventing the intermediate option rather than evaluating it against the minimal entry. The invisible intermediate position is potentially the better DECIDE input for certain guidance approaches (e.g., a spec-authoring quick-reference that points at the glossary entry).

**Concrete grounding action for DECIDE:**

Before drafting the OQ 15 resolution, stage three Dimension entry candidates: (a) the shipped minimal entry (extensibility-first, conventions in adapter docs), (b) the intermediate entry (extensibility-first, conventions named in the entry as adapter conventions, explicitly not a canonical list), and (c) the original enumeration-in-glossary (rejected in MODEL). Evaluate which entry best serves the spec-author guidance goal OQ 15 poses, given the constraint that dimensions remain extensible. Record the rejection reason for whichever candidates are not chosen.

### Item 2: The "different in kind" inference for dimension-vs-lens should be belief-mapped before the lens-as-grammar cycle is initiated

**The specific gap:**

The agent concluded from the structural comparison (dimension: exclusive node property, write-time, identity facets vs. lens: additive edge namespace, enrichment-time, vocabulary translation) that dimension and lens are "different in kind." This conclusion is plausible but was not belief-mapped. The question it forecloses if accepted uncritically: whether the lens-as-grammar framing, when it returns as a full cycle, should consider whether dimension assignment could be expressed within a lens grammar as a first-class concern — not as an edge translation, but as a node-level signal emitted by the lens enrichment.

**This is not an action for DECIDE in the current cycle.** The lens-as-grammar cycle is explicitly parked. This is a flag for the evaluator who opens that future cycle.

**Concrete flag to carry forward:**

When the lens-as-grammar cycle is initiated, the first belief-mapping question should be: "What would you need to believe for dimension assignment to be within scope for the grammar formalism, rather than outside it?" The structural differences between dimension and lens (node property vs. edge namespace) are real; the inference that they are therefore "different in kind" in the grammatical sense is not established. A grammar cycle that assumes they are permanently orthogonal will not examine whether the grammar formalism can span both concerns.

**What builds on it if left unexamined:**

The lens-as-grammar cycle opens with a constraint already embedded (dimension is outside the grammar's scope) without that constraint having been examined as a design choice. ADRs and vocabulary from that cycle would encode a scope boundary that was set implicitly during a light-touch MODEL pass in a prior cycle, not through the grammar cycle's own research and belief-mapping.

---

## Notes on Role Dynamics

The swift-adoption pattern's third consecutive appearance calls for a structural note beyond the per-phase observation.

The pattern is: user offers a substantive framing that is plausibly correct and also convenient (reduces scope, clarifies direction); agent adopts it with speed and without staging the intermediate option space. The pattern is not a bug in any individual exchange; it is an emergent property of a cycle where the same person is simultaneously practitioner, cycle stakeholder, and product-discovery author. When the person who proposes the framing and the person who would challenge the framing are the same, the challenge does not surface naturally.

The structural corrective is not "the agent should push back harder on substantively correct framings." It is that at phase boundaries where compression has occurred — where an intermediate option space was navigated implicitly rather than explicitly — the gate reflection should name the compressed options so DECIDE works from a visible map, not just the selected point.

For this cycle, the Grounding Reframe above (Item 1) attempts to restore that visibility at the DECIDE entry. Future cycles with the same practitioner-as-sole-stakeholder dynamic should treat the gate reflection note as the place to stage compressed option candidates, rather than leaving that work to the susceptibility snapshot.
