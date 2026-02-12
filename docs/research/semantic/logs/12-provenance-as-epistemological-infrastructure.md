# Research Log: Provenance as Epistemological Infrastructure

## Question 1: What does it look like for adapters to produce source-level provenance (chains/marks/links) alongside semantic contributions?

**Method:** Code analysis + design exploration

**Context:** The reverted Essay 12 spike answered the wrong question — "how should the pipeline add operational metadata?" The real question is: how should adapters record WHERE knowledge came from, using Plexus's chain/mark/link vocabulary, so that every concept in the graph has a traversable evidential chain back to its source material?

Chains, marks, and links are not just a user annotation feature. They are Plexus's epistemological infrastructure — the mechanism by which the graph knows why it knows things. Every adapter that introduces knowledge should produce provenance alongside semantics.

**Constraint:** Must be compatible with (and ideally strengthen) the Hebbian reinforcement design (ADR-003/005).

**Setup:** Examine how ProvenanceAdapter currently produces chains/marks, then design what it would look like for FragmentAdapter to produce the same vocabulary alongside its semantic output. Assess Hebbian compatibility.

**Findings:**

Three architectural options were identified:

1. **Adapter-level** — each adapter produces chain/mark/contains alongside its semantic nodes. Adapters know their source material and can create marks with appropriate tags, annotation text, and file references.
2. **Sink-level** — EngineSink auto-wraps emissions in provenance. The sink doesn't know what the nodes represent, so marks would lack meaningful annotations and tags.
3. **Pipeline-level** — the ingest pipeline adds provenance after adapter processing. Same knowledge gap as sink-level, plus it was the approach in the reverted spike.

**Conclusion:** Adapter-level is the only option that produces meaningful provenance. Only the adapter knows the source material, the annotation text, and the relevant tags. Provenance alongside semantics — from the same code that understands the domain.

**Implications:** This means adapters have a dual obligation: semantic contribution AND provenance trail. This is not a violation of separation of concerns — it IS the concern. A knowledge graph that can't explain its knowledge isn't trustworthy.

## Question 2: How does multi-phase document processing interact with provenance chains?

**Method:** Design exploration

**Context:** Real document adapters process in phases: L1 (metadata), L2 (structure), L3 (heuristic), L4 (LLM extraction). Same source, different resolutions, different timescales.

**Findings:**

- Each adapter instance gets its own chain per source: `chain:<adapter_id>:<source>`
- Deterministic chain IDs ensure idempotent upsert — re-processing doesn't duplicate chains
- Marks accumulate across phases: manual tagging produces marks with broad tags, LLM extraction produces marks with richer tags
- TagConceptBridger bridges ALL marks to concepts — both manual and LLM-extracted
- Hebbian contribution tracking naturally separates: each adapter's `tagged_with` edges carry its own contribution slot
- The result: you can see not just WHAT the graph knows, but which phase contributed each piece

**Implications:** Multi-phase processing strengthens the Hebbian design. Weights become explainable — if concept:distributed-ai has high confidence, you can traverse provenance to see that both a human tagger and an LLM extractor independently confirmed it from the same source.

## Question 3 (Spike): Can FragmentAdapter produce provenance marks alongside semantic output, with TagConceptBridger auto-bridging and Hebbian accumulation?

**Method:** Spike — code in production files (not scratch), validated by tests

**Spike question:** "Can FragmentAdapter produce chain + mark + contains edge alongside its semantic output, and does TagConceptBridger automatically bridge the marks to concepts?"

**Changes:**

1. `src/adapter/fragment.rs` — FragmentAdapter.process() now emits chain node, mark node, and contains edge alongside its existing fragment/concept/tagged_with output
2. Updated 5 existing unit tests for new node/edge counts
3. Updated 3 assertions in the two-consumer integration test (contains: 4→10, references: 8→23)
4. Added 2 spike tests:
   - `spike_fragment_adapter_produces_traversable_provenance` — single ingest, verifies chain/mark creation, TagConceptBridger bridging, cross-dimensional traversal from concept→mark→chain→source
   - `spike_multi_phase_hebbian_provenance` — two adapters processing same source at different phases, verifies separate chains, separate Hebbian contributions, progressive concept discovery, and traversal to origin

**Results:** 248 tests pass (246 original + 2 new). Zero failures.

**Key observations:**

1. **TagConceptBridger works unchanged.** No modifications to the enrichment. Fragment marks with tags are automatically bridged to concept nodes — the existing design handles this perfectly.
2. **Cross-dimensional traversal works.** From `concept:federated-learning`, you can traverse: concept ← references ← mark ← contains ← chain → source. The evidential chain is graph-traversable.
3. **Hebbian contributions accumulate correctly.** Manual and LLM adapters produce separate `tagged_with` edges with distinct contribution slots. Each contribution is explainable via its corresponding provenance mark.
4. **Idempotent chain creation.** Deterministic chain IDs (`chain:<adapter>:<source>`) mean re-ingesting the same source upserts rather than duplicates.
5. **Two-consumer test expanded naturally.** The existing Essay 11 test now has 23 references edges (up from 8) — fragment marks bridge to the same concept space as provenance marks, creating richer cross-dimensional connectivity.
6. **Pipeline fan-out gotcha.** Two adapters with the same `input_kind` both process every ingest call. Multi-phase adapters should use separate pipelines or distinct input kinds.

**What the graph now knows:** For any concept, you can ask "where did this come from?" and traverse to specific source evidence. For any source, you can ask "what knowledge did this produce?" and traverse to the semantic concepts it contributed. The graph is both ontological (what exists) and epistemological (why we believe it).
