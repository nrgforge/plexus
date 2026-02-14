# ADR-004: First Adapter Pair — Fragment and Co-Occurrence

**Status:** Accepted

**Date:** 2026-02-08

**Research:** [Essay 07](../essays/07-first-adapter-pair.md), [Research Log](../research/research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — Fragment adapter pair section

**Scenarios:** [004-first-adapter-pair.md](../scenarios/004-first-adapter-pair.md)

---

## Context

The adapter infrastructure (ADR-001, ADR-003) is built and tested — 177 tests covering emission validation, sinks, provenance, events, contribution tracking, and scale normalization. No real adapters exist. The infrastructure needs end-to-end validation with concrete adapters processing real-shaped input.

Trellis, a writing accumulation system, provides the simplest useful input: tagged fragments. A fragment carries text and tags — applied manually by a human or extracted by an LLM. The tags are the extraction; the adapter's job is structural mapping. This deliberately minimizes adapter complexity so the infrastructure is what gets tested.

A single external adapter produces graph structure but no graph refinement. Adding a reflexive adapter that detects co-occurrence between concepts exercises the ProposalSink path and validates the two-adapter interaction model.

---

## Decisions

### 1. FragmentAdapter: external adapter for tagged writing **UPDATED by Essay 12**

The FragmentAdapter is an external adapter with `input_kind: "fragment"`. It receives a `FragmentInput` containing text, tags, an optional source, and an optional date. It emits a single emission containing:

**Semantic output:**
- A **fragment node** — `ContentType::Document`, dimension `"structure"`, unique ID per fragment, with text and metadata as properties.
- A **concept node per tag** — `ContentType::Concept`, dimension `"semantic"`, with a deterministic concept ID: `concept:{lowercase_tag}`.
- An **edge per tag** — relationship `tagged_with`, from fragment node to concept node, contribution value 1.0.

**Provenance output** (added by Essay 12):
- A **chain node** — `ContentType::Provenance`, dimension `"provenance"`, with deterministic ID `chain:{adapter_id}:{source}`. Re-ingesting from the same source upserts the existing chain.
- A **mark node** — `ContentType::Provenance`, dimension `"provenance"`, carrying the fragment's text as annotation, source file reference, and tags.
- A **`contains` edge** — from chain to mark, within the provenance dimension.

The mark's tags trigger automatic tag-to-concept bridging via `TagConceptBridger`, creating `references` edges from the mark to matching concept nodes. This makes every fragment's origin graph-traversable without any additional enrichment code.

**Alternatives considered:**

- *Raw text input requiring NLP/LLM extraction.* Rejected for the first adapter: adds extraction complexity that would obscure infrastructure bugs. The first adapter should validate plumbing, not pioneer extraction.
- *Separate adapter types per source (ManualFragmentAdapter, LLMFragmentAdapter).* Rejected: same `process()` logic, different identities. One struct with configurable adapter ID serves all sources — ADR-003's per-adapter contribution tracking handles the rest.

### 2. Deterministic concept IDs for tag convergence

Concept nodes receive IDs derived from their tag label: `concept:{lowercase_tag}`. "Travel" and "travel" from different fragments produce the same node ID, triggering upsert. This ensures concept convergence without a separate merge step.

**Alternatives considered:**

- *UUID-based concept IDs.* Rejected: two fragments tagged "travel" would produce two unrelated concept nodes. Convergence would require a separate deduplication pass.
- *Content-hash IDs.* Rejected: equivalent to lowercase normalization for single-word tags, more complex for no benefit.

Tags are expected to be single words or short normalized phrases. Compound tags with spaces or punctuation are not addressed by this scheme.

### 3. ~~CoOccurrenceAdapter: reflexive adapter for concept co-occurrence~~ **UPDATED: CoOccurrenceEnrichment (Essay 09)**

> **Status:** Updated. The CoOccurrenceAdapter has migrated from a reflexive adapter to an **enrichment** — a reactive component that runs in the enrichment loop after each emission. The algorithm is unchanged; the trigger model changed from schedule-based to event-driven. The enrichment receives graph events and a context snapshot, self-selects based on whether relevant changes occurred (new `tagged_with` edges or concept nodes), and terminates via idempotency.

The CoOccurrenceEnrichment:

1. Builds a reverse index from `tagged_with` edges: fragment → concepts.
2. Counts shared fragments for each concept pair.
3. Computes co-occurrence scores: `count / max_count`.
4. Emits `may_be_related` symmetric edge pairs between co-occurring concepts, with the co-occurrence score as the contribution value.

The co-occurrence scoring metric (`count / max_count`) was chosen for simplicity. More discriminating metrics (PMI, Jaccard) can replace it without interface changes, since the enrichment's internal scoring is independent of its emission contract.

### 4. Context snapshot for enrichments

The framework clones the Context and provides it to enrichments during the enrichment loop. The enrichment reads the snapshot for a consistent, immutable view at enrichment time.

This maintains the abstraction boundary: the enrichment depends on the graph model (`Context`, `Node`, `Edge`), not on engine internals.

### 5. Symmetric edge pairs for may_be_related

The CoOccurrenceEnrichment emits both `A → B` and `B → A` for each co-occurring concept pair, with identical contribution values. This ensures query-time normalization (outgoing divisive: `w_ij / Σ_k w_ik`) sees the relationship from both endpoints.

**Alternatives considered:**

- *Single directed edge, arbitrary direction.* Rejected: the relationship is invisible from one endpoint during query-time normalization.
- *Undirected edge support.* Rejected: requires a change to the Edge model. Two directed edges with identical contributions is the standard directed-graph representation of symmetric relationships.

---

## Consequences

**Positive:**

- End-to-end validation of the adapter infrastructure with real input shapes
- Deterministic concept IDs ensure convergence without a deduplication pass
- One FragmentAdapter type scales to any number of evidence sources via configurable identity
- Context snapshot keeps the enrichment isolated from concurrent mutations
- Symmetric edge pairs make query-time normalization correct for symmetric relationships

**Negative:**

- Symmetric edge pairs double the edge count for co-occurrence proposals (2M edges for M concept pairs). Modest for the expected scale.
- Passing a full Context clone as payload is wasteful for large graphs. Acceptable for early use; can be optimized to a focused snapshot later without changing the adapter.
- The co-occurrence adapter's scoring (count / max_count) is simple. More sophisticated metrics (PMI, Jaccard) may be needed later. The adapter's internal scoring logic can change without affecting its interface.
- The enrichment's self-imposed contribution cap compresses the range before storage (e.g., scores [0.5, 1.0] become [0.5, 0.5] with cap 0.5), which changes the effective bounds for scale normalization. The normalization floor still works correctly — this is benign but worth noting.

**Neutral:**

- The first adapter pair does not exercise routing fan-out or node property merge. This is expected — those questions resolve inside the engine, not inside adapters. This analysis assumes the adapter interface (`Adapter` trait, `AdapterSink` trait, `AdapterInput` struct) remains stable. Changes to upsert semantics (node property merge, OQ1) could require adapter adjustments.
- The enrichment loop is not needed for testing co-occurrence detection in isolation. The test harness invokes the enrichment directly.
