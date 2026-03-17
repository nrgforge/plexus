# Research Log: First Adapter Pair

## Question 1: Can a Trellis fragment adapter exercise the full pipeline, and which open questions does it surface?

**Method:** Code trace analysis — walked the adapter design through every infrastructure component.

**Findings:**

A FragmentAdapter that takes text + tags and emits fragment nodes, concept nodes, and tagged_with edges exercises the entire adapter pipeline: Adapter trait, AdapterInput, EngineSink, ProposalSink, Emission, AnnotatedNode/Edge, Annotation, provenance, graph events, contribution tracking, scale normalization, and InputRouter.

Traced the design against the four open questions from the domain model:

1. **Node property merge (OQ1):** Not a practical problem. Concept nodes from the same adapter are idempotent — same tag produces same node structure. Full replacement on upsert is correct. Only bites when two different external adapters emit the same node with different properties, which doesn't happen with this pair.
2. **Reflexive cycle convergence (OQ2):** Schedule monitor doesn't exist. Reflexive adapter can be tested by direct invocation. Deferrable.
3. **ProposalSink metadata edges (OQ3):** Co-occurrence adapter only needs `may_be_related` edges. Fits within ProposalSink constraints. Deferrable.
4. **Routing fan-out (OQ4):** Different input kinds (`"fragment"` vs `"graph_state"`). No fan-out conflict. Not exercised.

**Key design decision:** Reflexive adapter reads graph state via a cloned Context passed as the AdapterInput payload. Context is `Clone + Send + Sync + 'static`, so `Box::new(context.clone())` works. No new infrastructure needed.

**Implications:** All four open questions are deferrable. None block the first adapter pair. The abstraction boundary (Adapter trait + AdapterSink trait) isolates adapter code from engine internals, so resolving OQ1–4 later won't require adapter changes.

## Question 2: Co-occurrence adapter design and scoring

**Method:** Design trace + worked scenario with three fragments.

**Findings:**

The CoOccurrenceAdapter scans graph state for concept nodes that share fragments (via `tagged_with` edges), counts shared fragments per concept pair, and proposes `may_be_related` edges with contribution = normalized co-occurrence score.

Worked scenario with 3 fragments, 8 concepts, 10 tagged_with edges produced 11 co-occurrence proposals. Strongest pair (2 shared fragments) gets score 1.0; single-co-occurrence pairs get score 0.5.

**Issue found:** Scale normalization zeros out the minimum contribution. After normalize: strongest = 1.0, all single-co-occurrence = 0.0. This is the ADR-003 known issue.

**Resolution:** Dynamic epsilon in scale normalization. Formula: `(value - min + ε) / (max - min + ε)` where `ε = α × (max - min)`, α = 0.01. This gives every adapter the same proportional floor (~1%) regardless of range. Degenerate case (range = 0) still returns 1.0. Fair, simple, principled.

**Additional design decisions:**
- **Manual vs LLM tagging:** One FragmentAdapter struct with configurable identity. Different instances get different adapter IDs → separate contribution slots, independent provenance, evidence diversity counts both sources.
- **Edge directionality:** Emit both directions for symmetric `may_be_related` relationships. Ensures query-time normalization (outgoing divisive) sees the relationship from both endpoints.
- **Test harness:** Direct adapter invocation, no schedule monitor needed. Process fragments → clone Context → pass to co-occurrence adapter.

**Implications:** No new infrastructure needed. The build phase produces: FragmentInput struct, FragmentAdapter, CoOccurrenceAdapter, integration tests, and the dynamic epsilon tweak to recompute_raw_weights().
