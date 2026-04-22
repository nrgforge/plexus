# ADR-041: Lens Grammar Conventions — Named vs Structural Predicates

**Status:** Accepted

**Research:** PLAY field notes (2026-04-16), specifically §"Phenomenology of discovery constrains lens output language," §"Apps have multiple jobs," §"Three complementary layers for Trellis's lens"; DISCOVER update (product-discovery.md, 2026-04-17)

**Product discovery:** Value tension *"Interpretive vs. structural lens predicates (per-job, not per-app)"*; Assumption inversion *"Apps can be classified by their most visible surface"* (validated as hypothesis, surface-classification obscures the discovery core); vocabulary entry for **lens** (extended 2026-04-17 with composition-shape awareness)

**Domain model:** [domain-model.md](../domain-model.md) — lens, vocabulary layer, consumer, translation rule, declarative adapter spec

**Depends on:** ADR-033 (lens declaration in declarative adapter spec), ADR-034 (composable query filters — the `relationship_prefix` filter is the navigation mechanism across lens vocabularies)

**Does not depend on:** ADR-038, ADR-039, ADR-040 (the lens-grammar decision is independent of the release-binary and default-enrichment ADRs in this cycle)

---

## Context

ADR-033 established the mechanics of lens declaration: a `lens:` section in the declarative adapter spec maps source relationships (`may_be_related`, `similar_to`, `tagged_with`, etc.) to output relationships namespaced as `lens:{consumer}:{relationship}`. ADR-033 left the **semantics** of the output relationship open — what a consumer chooses to name its translation target was treated as an opaque design decision.

The PLAY session surfaced a substantive question ADR-033 does not answer: does the naming convention of the `to` relationship matter for Plexus's value proposition, or is it purely the consumer's cosmetic choice? Two candidate grammars appeared in the field notes:

- **Named-relationship lenses** — the `to` relationship interprets the connection semantically: `lens:trellis:thematic_connection`, `lens:carrel:cites`, `lens:trellis:draft_about_theme`. The consumer queries for the interpretation directly.
- **Structural-predicate lenses** — the `to` relationship describes the *shape* of the connection without interpreting it: `lens:trellis:latent_pair`, `lens:trellis:bridges_communities`, `lens:trellis:has_N_semantic_neighbors`, `lens:trellis:density_shift`. The consumer queries for the shape; meaning is supplied by the consuming app's UI or by the end-user's interpretation.

The field notes proposed that for jobs supporting interpretive work (creative writing coaching, thesis-finding), structural predicates preserve the phenomenology of discovery — "noticing the connection yourself" rather than "being told the connection exists" (Trellis architecture paper §3.7). The field notes further proposed that for operational jobs within the same app (publishing-pipeline routing, search ranking), named relationships are appropriate because the app routes logic on them programmatically. This is the **per-job-not-per-app** framing.

The DISCOVER gate conversation (2026-04-17) refined this further. Two layers must be distinguished:

- **Value-proposition layer (settled):** Plexus adds value when it surfaces structure the user did not encode. Latent discovery IS the value proposition. A lens that reports only pre-encoded tags is tautological with respect to Plexus's value proposition.
- **Lens grammar layer (hypothesis):** Whether the lens output language *must* be structural to preserve discovery phenomenology is distinct from the value-proposition layer. A publishing pipeline using `lens:carrel:cites` for operational data is not tautological — the named edge is pipeline routing logic, not writer-facing discovery signal. The phenomenology constraint plausibly applies to the writer-facing subset of lens consumers, not all lens consumers.

The cycle-status at start-of-DECIDE specifies that the grammar-layer question is **analytical work within this ADR**, not a live DB experiment — the structural predicates the hypothesis turns on require external enrichments (graph-science scripts) that do not exist in the current build, so a live comparison would reduce to renaming the same edge and could not demonstrate the phenomenological claim.

This ADR is the analytical comparison plus the decision about what grammar conventions Plexus endorses.

## Analytical comparison (the in-ADR spike)

Two alternative lens specs for a Trellis-like consumer, walked through against an illustrative graph of four fragments with extracted concept overlaps:

### Spec A — named-relationship lens

```yaml
lens:
  consumer: trellis
  translations:
    - from: [may_be_related, similar_to]
      to: thematic_connection
      min_weight: 0.2
```

For a pair of fragments `F1` and `F2` whose extracted concepts overlap (a `may_be_related` edge with raw weight 0.4), the lens emits:

```
F1 --[lens:trellis:thematic_connection]--> F2
F2 --[lens:trellis:thematic_connection]--> F1
```

**What the consumer queries:** `find_nodes` with `relationship_prefix: "lens:trellis:thematic_connection"`, or `traverse` from `F1` with the same prefix filter. The consumer's query composer receives edges named `thematic_connection` — a semantic interpretation encoded directly in the edge type.

**What the consumer's UI has to do:** surface the edge as an interpreted signal. "F1 and F2 are thematically connected." The interpretation is already baked in — the UI either displays it verbatim or discards information to avoid asserting it.

### Spec B — structural-predicate lens

```yaml
lens:
  consumer: trellis
  translations:
    - from: [may_be_related, similar_to]
      to: latent_pair
      min_weight: 0.2
```

For the same `F1`/`F2` pair, the lens emits:

```
F1 --[lens:trellis:latent_pair]--> F2
F2 --[lens:trellis:latent_pair]--> F1
```

**What the consumer queries:** identical mechanics — `find_nodes` / `traverse` with `relationship_prefix: "lens:trellis:latent_pair"`. The consumer's query composer receives edges named `latent_pair` — a structural descriptor that says "these two fragments share something latent that may be worth examining" without naming what.

**What the consumer's UI has to do:** surface the edge as a prompt for the writer's interpretation. "Here are two fragments that share something. What do you notice?" The UI does not assert the nature of the connection — it creates the conditions for the writer to notice it.

### What's identical between A and B

The plumbing. Both specs use the same ADR-033 machinery. Both produce symmetric edge pairs. Both are namespace-scoped and discoverable via `relationship_prefix: "lens:"` introspection (ADR-034). Both carry the same corroboration (per-source contribution keys, ADR-033 §Relationship namespace convention). Both support `min_weight` filtering and `involving` predicate scoping.

### What differs between A and B

**The interpretive work location.** In Spec A, the interpretation happens at lens-write time — the name `thematic_connection` encodes Plexus's (or the lens author's) claim about what the connection means. In Spec B, the interpretation happens later — at the consumer's UI layer (how it surfaces the edge to the user) or at the end-user's reading layer (the writer reading the juxtaposed fragments and deciding what connects them).

**Query expression reach.** A query against `lens:trellis:thematic_connection` names a semantic interpretation chosen by the lens author; the consumer's query-time language inherits that choice. A query against `lens:trellis:latent_pair` names a structural descriptor; the consumer's query-time language inherits a shape descriptor instead, and any semantic interpretation is supplied downstream (at the UI layer or by the end-user). The second query shape is what the field-notes called the *shape-of-the-graph query*; whether it is better suited for interpretive jobs than the first shape is the hypothesis this ADR carries (see below), not a conclusion the analytical comparison establishes.

**Composition-shape on extension.** If a consumer later adds network-science enrichments via llm-orc ensembles (bridge nodes, community membership, centrality), structural-predicate vocabulary extends naturally — a new translation `from: bridges_communities` → `to: bridges_communities` is one more predicate in the same register. Named-relationship vocabulary extends less naturally — the new rule has to decide whether `bridges_communities` is `thematic_connection` (semantic overlap), `structural_bridge` (graph topology), or a new interpretive category. Each named extension forces a semantic commitment; each structural extension adds a predicate without reshaping the vocabulary's frame.

### What the analytical walk-through does NOT show

The analytical comparison does not demonstrate that Spec B "produces a discovery" where Spec A does not. Whether the phenomenological difference (receiving an observation vs having a discovery) actually obtains when a real writer interacts with a Trellis-like UI is an empirical question not answerable by comparing edge names in a graph. The analytical comparison shows what the two grammars structurally differ on — interpretive location, query shape, composition-shape on extension — and shows that the differences plausibly matter for jobs whose value proposition involves the user's interpretive work.

The phenomenological claim remains a hypothesis. This ADR does not promote it to settled principle.

**Scope condition on the illustrative case.** The walk-through above uses `may_be_related` edges from co-occurrence over hand-tagged content as the illustrative source for both specs. Per PLAY field-note §"Crawl-step results and the tautology threshold," co-occurrence over user-supplied tags is below the tautology threshold — neither grammar can add discovery value to content the user already encoded in the tag overlap. The grammar distinction becomes practically consequential when the lens's source crosses the threshold (semantic extraction via an llm-orc ensemble, embedding-based similarity, or topological analysis over scale+time), where structure emerges that the spec author did not pre-encode. The convention endorsement below is meant to apply in that range; when the source is tautological, neither grammar changes the information content, and the choice reduces to stylistic preference.

## Decision

### Plexus endorses structural-predicate lens grammar for discovery-oriented jobs as a convention, not a requirement

The declarative adapter spec grammar (ADR-033) does not enforce any naming constraint on the `to` relationship. Lens authors are free to choose named or structural predicates. This ADR adds a documented **convention** with an explicit scope condition:

> For jobs whose value proposition involves the user's interpretive work — creative-writing scaffolding, thesis-finding, reflective discovery — lens authors should consider structural-predicate output relationships (`latent_pair`, `bridges_communities`, `density_shift`, `has_N_semantic_neighbors`, `dormant_since_T`, `member_of_candidate_cluster`) over named-relationship output relationships (`thematic_connection`, `related_by_theme`).
>
> For operational jobs within any app — publishing-pipeline routing, search-result ranking, analytics aggregation — named-relationship output relationships are appropriate and expected. The two registers coexist within a single consumer spec.

This is a **per-job**, not **per-app**, distinction. The same consumer spec may declare a translation rule producing `lens:trellis:latent_pair` (for the discovery UI surface) and another producing `lens:trellis:ready_to_publish` (for a publishing pipeline). Both are valid. Whether either is appropriate is a judgment the lens author makes based on the job that will consume the translated edge.

### The convention is documented in product discovery, spec-author documentation, and interaction specs — not enforced in code

Plexus's declarative adapter spec grammar will **not** reject named-relationship `to` values, will **not** flag them as warnings, and will **not** maintain a canonical list of structural predicates. The rationale aligns with the dimension decision in ADR-042: Plexus does not presume to know the right grammar for every consumer's job. The convention is surfaced where spec authors look (documentation) and where jobs are decomposed (product discovery value tension; interaction specs derivation), not encoded as validation.

### The phenomenology-of-discovery claim is named as hypothesis, not principle

The convention is grounded in the composition-shape reasoning from the analytical walk-through above (what query shapes the grammar enables; how the vocabulary extends under network-science additions), not in an asserted phenomenological claim. The phenomenological framing — that interpretive writing apps need structural predicates to preserve the writer's experience of "having a discovery" rather than "receiving an observation" — is explicitly carried as hypothesis-level material in product discovery (value tension *"Interpretive vs. structural lens predicates"*, assumption inversion *"Apps can be classified by their most visible surface"*) and is not promoted to settled principle by this ADR.

This matters because the phenomenological claim originated from one practitioner's mid-PLAY perspective-taking (cycle-status Uncertainty 1) and has not been validated by observing real writers interacting with either grammar. A future cycle with grounded untagged-content evidence and ideally a non-builder stakeholder doing PLAY may promote the hypothesis to principle, revise it, or reject it.

**Which argument carries which part of the decision.** The convention as stated combines two arguments that rest on different grounds, and close reading shows they do different work:

- **Composition-shape reasoning (analytical, load-bearing for the extensibility preference).** Structural predicates extend more naturally under future network-science additions than named relationships do. This argument endorses structural predicates for any spec anticipating such extensions, independent of what job the spec serves.
- **Phenomenology-of-discovery (hypothesis-level, load-bearing for the per-job split).** The per-job distinction in the convention — structural for discovery-oriented jobs, named for operational jobs — rests on the hypothesis that different jobs have different phenomenological requirements (user-interprets vs. app-routes-logic). Composition-shape reasoning alone would not produce the per-job split; a publishing pipeline that may later add network-science analytics would benefit from structural predicates under composition-shape too.

The convention combines both: composition-shape gives the extensibility preference, phenomenology gives the per-job shape. If a future cycle invalidates the phenomenology hypothesis, composition-shape reasoning survives but the convention's per-job phrasing needs revision — likely toward "consider structural predicates when anticipating network-science extensions" without the job-type qualifier. Surface the grounds-distinction now so the future revision starts from visible structure rather than rediscovering which argument supports which part.

### Lens-as-grammar is scoped out; composition-shape awareness is carried in

The richer theoretical framing — lens-as-grammar, where the lens is not just a vocabulary layer but a grammar the graph speaks in (composition rules, query-expectation contracts, syntactic register) — is **scoped out of this cycle** per the cycle-status (Hypotheses Parked for Future Cycles §Lens-as-grammar). What this ADR carries in is the simpler concept: **composition-shape awareness** — lens translation rules shape the *query patterns* the consumer's app can naturally compose against the graph, not just the *vocabulary* those queries return. This is the concept documented in the extended **lens** vocabulary entry in product discovery.

Composition-shape awareness is what this ADR's convention rests on. Lens-as-grammar will need room for its own research cycle — it is not load-bearing for this ADR.

### The `from` field and existing ADR-033 mechanics are unchanged

No mechanical changes to the declarative adapter spec grammar. The `from:` / `to:` / `min_weight:` / `involving:` fields work exactly as ADR-033 defined. The `lens:{consumer}:{to}` namespacing, per-source-relationship contribution keys, and `LensEnrichment` construction are unchanged. This ADR adds vocabulary convention; it does not amend the spec grammar.

## Consequences

**Positive:**

- Lens authors for discovery-oriented jobs have an explicit convention pointing toward structural predicates, backed by composition-shape reasoning they can evaluate for their own context.
- The convention is scope-conditional (per-job, not per-app), which matches the PLAY finding about apps having multiple jobs and avoids over-generalizing from one role inhabitation.
- Plexus's grammar remains permissive — no consumer is forced into a naming pattern they judge inappropriate. This preserves extensibility and consumer sovereignty over spec content (Invariant 61).
- Composition-shape awareness enters spec-author documentation as a working concept, enabling lens authors to think about what their app will query against the lens, not just what vocabulary it returns.
- The lens-as-grammar hypothesis is visibly parked rather than implicitly abandoned, and the conditions under which a future cycle can pick it up (real emergent graph content to study, not pre-tagged demonstrations) are recorded.

**Negative:**

- Documentation-only conventions are softer than grammar-enforced constraints. A lens author who does not read the convention will not be flagged. Product discovery and interaction specs are the mitigation; the cost is that authors who skip them may produce lenses that underserve discovery-oriented jobs.
- "Per-job" requires the consumer to decompose their app's jobs before authoring lens rules. This is additional design work compared to "pick any naming convention and go." The cost is paid in spec-author work; the benefit lands in the user's experience.
- The phenomenological claim is explicitly unresolved. Consumers who adopt the structural-predicate convention expecting it to deliver a named experiential outcome may find the outcome empirically varies. Clear hypothesis-framing in product discovery is the mitigation.

**Neutral:**

- ADR-033 is unchanged. The `lens:` YAML grammar is unchanged.
- ADR-034's composable filters are what make the structural-predicate query pattern work (`relationship_prefix` for lens navigation). This ADR does not require any filter extension.
- Scenarios for the lens grammar question are light — they exercise the existing ADR-033 mechanism with both named and structural `to` values and verify both are accepted without diagnostics. The substantive validation is in product-discovery framing and interaction-specs guidance, not in code behavior.
- A future cycle on lens-as-grammar may amend this ADR's "convention, not requirement" stance if evidence accumulates. The cycle-status (Hypotheses Parked for Future Cycles §Lens-as-grammar) names the precondition that would make such evidence legible: real emergent graph content produced by untagged-prose ingestion with active semantic extraction or embeddings — structure the writer did not pre-encode — ideally observed with a non-builder stakeholder inhabiting the Consumer Application Developer role during PLAY. Evidence short of that precondition (e.g., more practitioner-led analytical walk-throughs on co-occurrence over hand-tagged content) does not qualify, because the prior cycle's phenomenological observations were already from that shape of evidence. Until a cycle meeting the precondition runs, this ADR's "convention, not requirement" stance holds.

## Provenance

**Drivers:**
- PLAY field notes §"Phenomenology of discovery constrains lens output language" and §"Apps have multiple jobs" (`docs/essays/reflections/field-notes.md`) — grounded the per-job-not-per-app framing and surfaced the phenomenology-of-discovery hypothesis.
- Product discovery value tension *"Interpretive vs. structural lens predicates (per-job, not per-app)"* — routed the question to DECIDE with the analytical comparison as the grounding action.
- DISCOVER gate conversation (`docs/housekeeping/gates/default-install-lens-design-discover-gate.md`, 2026-04-17) — differentiated the value-proposition layer (settled) from the lens-grammar layer (hypothesis).
- Cycle-status Uncertainty 1 — specified that the grammar-layer question is analytical work within this ADR, not a live DB experiment, and named the three outcome possibilities (universal constraint / consumer-dependent / subtler triggering condition).
- ADR-033's mechanics — the analytical walk-through rests on ADR-033's namespace convention and translation rule semantics.

**Note on provenance check:** The framings in this ADR trace to named drivers above — the analytical walk-through is composed from ADR-033's mechanics against a Trellis-like illustrative case; the per-job decomposition is from product discovery; the hypothesis framing is from the DISCOVER gate conversation and the cycle-status Uncertainty. The "composition-shape awareness" concept is the documented extension to the **lens** vocabulary entry added 2026-04-17 during the DISCOVER gate, not a drafting-time synthesis. The choice to scope lens-as-grammar out is from cycle-status's Hypotheses Parked for Future Cycles. No novel framings introduced here lack driver provenance.
