# Gate Reflection: Default-Install and Lens Design Principles — MODEL → DECIDE

**Date:** 2026-04-19
**Phase boundary:** MODEL → DECIDE
**Cycle:** Default-Install Experience and Lens Design Principles

## Belief-mapping / warrant-elicitation questions composed for this gate

Primary (warrant elicitation, composed by agent):

> "The Dimension entry now says placement is consequential because enrichments and queries filter by dimension. What's your warrant for that claim — what evidence in the codebase actually establishes it? The existing Invariant 50 says enrichments are structure-aware, but the specific 'queries filter by dimension' part — I wrote that synthetically. Is it actually true of current query implementations, or have I overstated?"

User response produced the agent's grounding action: verified in `src/query/find.rs` that `FindQuery` exposes `dimension: Option<String>` as a filter with exact-match implementation. The "queries filter by dimension" claim became grounded rather than aspirational.

Secondary (epistemic challenge from user, composed mid-gate):

> "Is it necessary to explicitly name dimensions?"

This question — surfaced by the user, not the agent — reshaped the phase's outcome. The agent had drafted a Dimension entry with per-dimension-intent enumeration; the user's question triggered reconsideration of whether enumeration belongs in the glossary at all. The agent produced an analysis of dimension-as-concept vs. dimension-as-closed-enumeration and concluded that the concept earns its place but the enumeration does not.

## User's response

Primary response to warrant-elicitation (verbatim):

> "Make the case for me for filtering by dimension."

Asked agent to articulate the affirmative case. Agent produced grounded claim citing `src/query/find.rs:14-15` and `:140-142`, `src/graph/edge.rs:74-79`, and internal code (`src/provenance/api.rs`) treating dimension as an organizing index.

Response on extensibility (verbatim):

> "I think we have core dimensions that perhaps we know from research (temporal, structural, etc) are what we want. However I don't want to contrain too much here. Again, the idea with Plexus is the handle unstructured knowledge. So if we have oen dimensional paradigm from one domain and another dimensional paradigm from a different domain, I don't want to presume to know what's more important. Does that track?"

Epistemic challenge (verbatim):

> "Is it necessary to explicitly name dimensions?"

Probe on provenance (verbatim):

> "Tell me more about the provenance of the dimension idea and how it differs from any other lens encoding"

Led the agent to trace origin (Essay 03, archived; four dimensions originally with "conceptual" rather than "provenance"; no dedicated ADR), identify `src/graph/node.rs:10` citation drift (ADR-009 cited by code is actually "Automatic Tag-to-Concept Bridging," superseded), and provide structural comparison of dimension vs. lens (node property vs. edge namespace; exclusive vs. additive; write-time vs. enrichment-time; taxonomic vs. translational).

Complexity guidance (verbatim):

> "That makes more sense. I want to dial the right level of complexity here without complecting how all this works."

Triggered agent to propose minimal Dimension entry and minimal OQ 15. User approved with "Apply and proceed please."

## Pedagogical move selected

Warrant elicitation (primary, on "queries filter by dimension"). Commitment gating (terminal, on settled premises and open questions for DECIDE). User-initiated epistemic challenge reshaped phase outcome mid-gate.

## Commitment gating outputs

**Settled premises (building on these going into DECIDE):**

- Dimension is a named facet declared on nodes, used as an organizing/filtering layer. It's load-bearing via Invariant 50 (enrichments) and `find_nodes` query filter.
- Dimensions are string-valued and extensible — consumer-declared dimensions are valid graph data. Plexus does not presume which dimensional paradigms matter for consumer domains it doesn't understand.
- Shipped-adapter dimension conventions (`structure`, `semantic`, `relational`, `temporal`, `provenance`) live documented alongside each adapter, not enumerated as a closed set in the glossary.
- No invariant amendment. No existing invariant reversed.
- Dimension and lens are distinct concerns in this cycle's treatment — node identity vs. edge meaning. Future lens-as-grammar cycle may revisit this separation.

**Open questions (held open going into DECIDE):**

- OQ 15: how (if at all) should Plexus guide spec authors toward dimension choice, within the extensibility constraint (no rejection of unknowns; no forced canonical mapping)? Documentation-only, validation warnings, or some combination — DECIDE decides shape.
- Code-level documentation drift: `src/graph/node.rs:10` cites "ADR-009: Multi-Dimensional Knowledge Graph Architecture" but ADR-009 is actually a superseded tag-concept-bridging ADR. Flagged during provenance tracing; DECIDE/BUILD should address as a small documentation fix.
- Whether the dimension-vs-lens distinction (node identity vs. edge meaning) is truly load-bearing or collapsible under a unified framework — parked for the lens-as-grammar cycle.

**Specific commitments carried forward to DECIDE:**

- Product-discovery's "default-lean install vs. full capability" tension stands; DECIDE's Release-Binary / EmbeddingSimilarity shipping ADR remains the principal work item.
- The three ADR candidates flagged in cycle-status (Release-Binary, DiscoveryGap trigger broadening, TemporalProximity property contract) carry forward.
- Lens-grammar ADR (per-job framing, composition-shape awareness — not full grammar theory) carries forward.
- OQ 15 enters DECIDE as a named question alongside the ADRs; DECIDE decides whether to resolve inside an ADR, via interaction-specs updates, or via documentation revisions.
- Extensibility constraint on dimensions is binding: any DECIDE output that touches spec validation must honor it.

## Candidates Considered

*(Section added 2026-04-20 per MODEL-phase susceptibility snapshot recommendation. Staging compressed option space explicitly rather than leaving recovery to the susceptibility evaluator — addresses the cross-phase pattern named in the snapshot's Role Dynamics section.)*

### Dimension glossary entry shape — three candidates, one selected

- *(a) Enumeration-in-glossary with per-dimension intent paragraphs* — initial agent draft. **Rejected on extensibility grounds during gate.** Intent paragraphs elevate the five shipped dimensions (structure, semantic, relational, temporal, provenance) to canonical status, which conflicts with Plexus's domain-extensible stance. User explicitly articulated the extensibility concern: *"I don't want to presume to know what's more important."*
- *(b) Enumeration-in-glossary as "shipped conventions" with explicit extensibility note* — intermediate middle-path option. **Not explicitly staged during gate; would have preserved per-dimension naming inside the glossary entry while marking them as adapter conventions, not closed-set constitutional vocabulary.** Viable alternative; adds length to the glossary entry for information that already lives at the adapter level.
- *(c) Minimal extensibility-only entry* — **selected.** Shipped conventions named briefly inline, per-dimension intent documented alongside each adapter, extensibility is primary framing. Trade-off accepted: a reader of the glossary in isolation must consult adapter documentation for per-dimension intent.

### OQ 15 option space — two ruled out, three live

Candidates ruled out by the extensibility constraint:
- *Strict schema validation* (reject unknown dimension strings). Out on extensibility.
- *Mandatory node-type-to-dimension mapping* (inferred against Plexus-canonical table). Out on extensibility.

Candidates live going into DECIDE:
- *Warn-on-divergence from shipped conventions for Plexus-known node types only* — middle path surfacing the constraint where Plexus has conventions, silent where it doesn't.
- *Documentation-only* — lightest touch; relies on author diligence.
- *Syntactic validation only* — non-empty strings, no reserved characters; likely baseline under any shape.

Candidates can combine. See OQ 15 in the domain model for the fully-staged framing.

### Future-cycle flag — lens-as-grammar scoping

Dimension-vs-lens "different in kind" conclusion (node identity vs. edge meaning) reached during gate was structurally defensible but not belief-mapped. Recommended belief-mapping question for the lens-as-grammar cycle's entry (logged in cycle-status `§Hypotheses Parked for Future Cycles`): *"What would you need to believe for dimension assignment to be within scope for the grammar formalism?"*

### Methodological note

Cross-phase pattern flagged by the MODEL susceptibility snapshot: three successive phases (PLAY, DISCOVER, MODEL) have recorded the same dynamic — user offers substantive framing; agent adopts with speed; intermediate option space goes unexamined. Framings adopted were all defensible; cumulative effect is that downstream phases inherit compressed option space. This Candidates Considered section is the structural corrective: gate reflection notes stage compressed candidates rather than leaving recovery to the susceptibility evaluator. Precedent established here for future MODEL/DECIDE/ARCHITECT gate notes in this cycle and beyond.
