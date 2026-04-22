# ADR-042: Dimension Extensibility Guidance for Spec Authors

**Status:** Accepted

**Research:** PLAY field notes Finding 3 (`docs/essays/reflections/field-notes.md`, 2026-04-16); MODEL phase Amendment Log entry #9 (`docs/domain-model.md`, 2026-04-18)

**Product discovery:** Feed-forward item 10 (cycle-status §"routed to MODEL"); Product Debt row *""Minimum-viable spec" and "minimum-useful spec" are treated as the same thing in onboarding"*

**Domain model:** [domain-model.md](../domain-model.md) — **Dimension** (softened 2026-04-18); Open Question 15 (*"Guiding spec authors toward appropriate dimension choice"*, routed to DECIDE)

**Depends on:** ADR-025 (declarative adapter spec extensions — defines the `create_node` primitive and its `dimension` field)

**Resolves:** Open Question 15

---

## Context

PLAY Finding 3 observed two fragment nodes in the same context living in different dimensions — the content adapter placed its fragments in `dimension: structure`; a hand-authored minimal declarative adapter spec placed them in `dimension: semantic`. Both nodes coexisted but were invisible to each other's dimension-filtered queries and to enrichment behaviors scoped by dimension (Invariant 50). The field note flagged: "The dimension surface is not well-surfaced in the spec grammar; new consumers might pick a dimension without understanding the consequences."

The MODEL phase (2026-04-18) did not add an invariant constraining dimension. It **softened** the domain model's Dimension entry from enumeration-in-glossary (`"Known dimensions: structure, semantic, relational, temporal, provenance"`) to extensibility-aware framing: dimensions are string-valued and extensible — consumer-declared dimensions are valid graph data. Shipped-adapter conventions are documented alongside each adapter rather than inside the glossary. The MODEL pass explicitly staged three candidates for DECIDE:

- **(i) Warn-on-divergence from shipped conventions for Plexus-known node types only.** If a spec declares `node_type: fragment, dimension: semantic`, emit a validation warning — `fragment` is a Plexus-known structure-dimension type per the content adapter. If a spec declares `node_type: gesture_phrase, dimension: harmonic`, no warning — Plexus has no conventions for `gesture_phrase`; the consumer's choice is authoritative.
- **(ii) Documentation-only.** Keep the grammar permissive; make dimension intent prominent in spec-author documentation, quickstart examples, and the minimum-useful-spec pattern.
- **(iii) Syntactic validation only.** Reject empty strings or dimension strings containing reserved characters (colons, spaces); no semantic validation. Likely baseline regardless of which of (i) or (ii) is selected.

Two candidates were **ruled out** by the extensibility binding constraint established in MODEL: strict schema validation of dimension values (conflicts with extensibility) and mandatory node-type-to-dimension mapping (presumes Plexus knows the right dimension for every node type across all domains). These are not on the table.

The DECIDE task is to pick a shape within the range (i) / (ii) / (iii) or some combination, respecting the binding constraint that dimensions remain extensible.

## Decision

### Adopt (ii) + (iii) — documentation-only semantic guidance, plus syntactic validation as baseline

Plexus adopts:

- **(iii) Syntactic validation as baseline.** The declarative adapter spec validator rejects dimension values that are (a) empty strings, (b) strings containing whitespace, or (c) strings containing reserved characters (currently: colons `:` and null `\0`). These are not semantic constraints — they are well-formedness constraints that protect the on-wire format and prevent confusion with namespaced relationships (e.g., `lens:trellis:...`). Everything that passes syntactic validation is accepted.
- **(ii) Documentation-only semantic guidance.** The dimension choice is surfaced prominently in:
  - Spec-author documentation (a dedicated section on dimension choice, the shipped-adapter conventions, and when to depart from them).
  - The `create_node` primitive's documentation (field-level docs naming the dimension field as load-bearing for enrichments and queries).
  - Quickstart examples and the *minimum-useful-spec* pattern (see Product Debt row routed to interaction specs).
  - Interaction specs for the Consumer Application Developer's "write a declarative adapter spec" task (updated in this cycle).

### Do not adopt (i) — warn-on-divergence for Plexus-known node types

Option (i) was considered and rejected for this cycle. Reasons:

- **The "Plexus-known node types" list is itself a moving target.** The content adapter uses `node_type: fragment` in `dimension: structure`; the extraction coordinator uses `node_type: file` in `dimension: structure`; the declarative adapter spec primitive `for_each` can produce concept nodes typed by the spec author. Maintaining a canonical table of "node types Plexus knows" risks encoding into code a list that really belongs in documentation, and creates friction when a new shipped adapter introduces a new node type. The canonical-table-as-code path would drift quickly or require policy around when to add and when to warn.
- **The boundary between Plexus-known and consumer-novel is porous.** A consumer writing a spec for a creative-writing app produces `node_type: fragment` — the same string the content adapter uses. Whether the consumer's `fragment` is the same concept as Plexus's shipped `fragment` is a semantic question the type name alone cannot answer. Warning on name collision may actively mislead the author into thinking they must match the shipped convention, when in fact they may deliberately be authoring a different `fragment` concept with different dimension semantics.
- **Option (ii) covers the case at lower cost.** An author who reads the spec-author documentation and the minimum-useful-spec pattern encounters the shipped conventions there; the documentation does the guidance work without Plexus having to maintain code paths for convention policing.

Option (i) may be revisited in a future cycle if documentation-only guidance turns out to be insufficient in practice (i.e., if repeated spec authors make the same dimension-choice mistake because they skipped the docs). That evidence does not exist today.

### What "documentation-only" requires

The intent is for the "documentation" lever to do the guidance work that option (i)'s validator path would have done. Whether documentation-only is *actually* sufficient to prevent the PLAY Finding 3 failure mode (spec author silently picks a diverging dimension for a colliding node type) is empirical — this ADR cannot verify it at DECIDE time, and the claim "documentation is substantive, not nominal" is an intent claim with deferred verification. What this ADR *can* commit to is a concrete set of BUILD deliverables and a checkable landing criterion:

**Required BUILD deliverables for this ADR to land:**

1. **Spec-author documentation on dimension choice.** A section describing what dimensions are (extensible string facets declared on nodes), why the choice is load-bearing (enrichments per Invariant 50; query filtering per `find_nodes`), the shipped-adapter conventions (`structure` / `semantic` / `relational` / `temporal` / `provenance` with brief intent per the softened domain-model Dimension entry's pointer to adapter documentation), and guidance for consumers authoring novel dimensions (the dimension name is the consumer's vocabulary choice; Plexus will accept it; consumers should align with shipped conventions where the concept matches and depart where it does not).
2. **The `create_node` primitive's field-level documentation** naming `dimension` as load-bearing and linking to the dimension-choice section.
3. **The minimum-useful-spec pattern** in interaction specs — documented with a worked example that makes a deliberate dimension choice and names why. The pattern (see Product Debt routing) is the operational onboarding artifact that brings dimension guidance into the author's workflow.
4. **The shipped-adapter conventions documented alongside each adapter.** The content adapter's code documentation names that it places fragments in `structure`. The extraction coordinator's docs name its dimension choices. Consumers writing a spec that will coexist with a shipped adapter can check the adapter-level docs directly.

**Landing criterion (checkable at BUILD time):** a spec author authoring a declarative spec for a node type that collides with a shipped-adapter node type (e.g., `fragment`) must encounter the dimension-choice guidance *before* declaring the node's dimension. This is verified at BUILD by walking the onboarding path literally — opening the spec-author documentation, reaching the `create_node` primitive docs, and confirming the dimension-choice section is reachable in one navigation hop from the first spec-authoring reference. If the author has to search to find the guidance, the lever is nominal; if the guidance is on the path, the lever is substantive.

**Deferred empirical verification:** whether authors who skim rather than read catch the guidance in practice is an empirical question this ADR does not resolve. If repeated PLAY or user observation surfaces the Finding 3 failure mode again despite these deliverables, a future cycle should revisit and consider option (i) (warn-on-divergence) as escalation. The trigger for that future cycle is observed recurrence of the failure mode, not time passing.

### Syntactic validation is implemented in the spec validator

The syntactic validation (option iii) lives in the `DeclarativeAdapter::from_yaml()` / spec validator path. A spec declaring `dimension: ""` or `dimension: "semantic dimension"` or `dimension: "lens:trellis"` fails validation at `load_spec`, returning a clear error (Invariant 60). A spec declaring `dimension: "gesture"` or `dimension: "harmonic"` or `dimension: "movement-phrase"` is accepted — these are extensibility-valid consumer dimensions.

### The domain model's softened Dimension entry is the authoritative reference

The domain-model entry (softened 2026-04-18) is the authoritative reference for what a dimension is. Spec-author documentation points to the domain-model entry rather than duplicating it. Shipped-adapter conventions are named in adapter-level documentation rather than in the glossary, per the MODEL-phase decision.

## Consequences

**Positive:**

- Dimensions remain extensible. Consumers writing specs for novel domains (movement, audio, code analysis patterns not in shipped adapters) choose dimensions freely without fighting Plexus's validator.
- Syntactic validation closes the well-formedness holes (empty strings, embedded whitespace, collision with namespaced-relationship syntax) without encoding semantic policy.
- The documentation lever is explicitly a first-class implementation requirement. BUILD cannot treat docs as optional deliverables for this ADR.
- The "minimum-useful-spec" pattern in interaction specs (downstream of this ADR) gains a natural home for dimension guidance — one of several infrastructure preconditions named explicitly.

**Negative:**

- A spec author who does not read the documentation and picks a dimension that diverges from the shipped convention for a colliding node type will produce the same bug PLAY Finding 3 observed (two `fragment` nodes in different dimensions, invisible to each other's enrichments and queries). Mitigation is the strength of the documentation lever; the cost lands on spec authors who skip it.
- Plexus cannot detect a deliberate dimension choice that is also wrong. There is no tool that can say "you declared `fragment` in `semantic` and this is probably not what you meant" without the canonical table option (i) rejected.
- Documentation is harder to verify than validation. A future drift where documentation falls out of sync with shipped-adapter conventions is a foreseeable risk. The ORIENTATION document and system-design updates are the cross-check; the RDD methodology's `/rdd-conform` conformance audit (run on demand, not specified by an ADR) is the backstop mechanism for detecting drift between docs and code-level conventions.

**Neutral:**

- ADR-025 is unchanged. The `create_node` primitive's `dimension` field accepts any string that passes syntactic validation — the grammar is unchanged.
- Invariant 50 (enrichments are structure-aware, not type-aware) is unaffected — enrichments that filter by dimension filter on whatever string the spec author declared; semantic correctness is the author's responsibility.
- ADR-033 (lens declaration) and ADR-034 (composable query filters) are unaffected — both operate on relationship prefixes and node predicates, not on dimension-specific semantics.
- Future option (i) revisitation is preserved as an open path. If documentation-only proves insufficient, a future ADR can add warn-on-divergence for a curated set of shipped node types. The choice now is to start at the minimum and escalate if needed, not to start at the middle and discover it is too much.
- **Empirical escalation signal — BUILD-phase instrumentation opportunity.** The escalation trigger as originally stated ("if repeated PLAY or user observation surfaces the failure mode again") depends on observational signal that may not arrive — Plexus has few users, and builder-inhabitation PLAY has partial fidelity (cf. cycle-status §"Additional structural concern"). BUILD can optionally instrument a silent-idle detection path: when a spec loads declaring a `create_node` primitive whose `node_type` matches a shipped-adapter node type (e.g., `fragment`, `file`, `extraction-status`) but whose `dimension` diverges from the shipped convention, log the divergence at debug level — not warn, not error, just observable. A future cycle can mine logs or telemetry for occurrence rate and decide whether option (i) (warn-on-divergence) is warranted based on evidence rather than on observational chance. This is a BUILD-phase concern, not an ADR amendment — the ADR's decision shape does not change — but it is named here so the escalation path has a detectable signal, not only an observational one.

## Provenance

**Drivers:**
- PLAY field notes Finding 3 (`docs/essays/reflections/field-notes.md`) — grounded the problem as observed: two `fragment` nodes in different dimensions on the same context.
- MODEL phase Amendment Log entry #9 (`docs/domain-model.md`, 2026-04-18) — softened the Dimension entry to extensibility-aware framing and staged the three live candidates for DECIDE.
- Open Question 15 in the domain model — routed the guidance question to DECIDE with the extensibility binding constraint.
- Cycle-status MODEL-phase entry — named the three candidates and explicitly ruled out the stricter options (strict schema validation; mandatory node-type-to-dimension mapping) per the extensibility constraint.
- Product Debt row *""Minimum-viable spec" and "minimum-useful spec" are treated as the same thing in onboarding"* — the operational onboarding pattern that makes documentation-only guidance substantive rather than nominal.

The decision selects option (ii) + (iii) from the MODEL-phase staged candidates; no novel framings were introduced at drafting time. The "documentation lever is substantive, not nominal" stance is an ADR-level commitment to make the documentation requirement part of BUILD's deliverables — derived from the cycle-status guidance that documentation-only should not be a way to silently skip the work.
