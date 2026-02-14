# ADR-001: Semantic Adapter Architecture

**Status:** Accepted

**Date:** 2026-02-02

**Design documents:** [adapters.md](../design/adapters.md) (overview), [adapter-architecture.md](../design/adapter-architecture.md) (system design)

---

## Context

Plexus needs to ingest input from multiple domains — text documents, writing fragments, gesture data — and build a shared multi-dimensional knowledge graph. Each domain has different data shapes, processing costs, and extraction strategies. The system must support three applications with different timescales: real-time editing (Manza), long-term accumulation (Trellis), and live performance (EDDI).

We need an abstraction that:

- Handles diverse input types through a uniform interface
- Supports both instant and long-running extraction (milliseconds to minutes)
- Emits results progressively as they become available
- Tracks how every piece of knowledge entered the graph (provenance)
- Allows the graph to refine itself over time (reflexive processing)
- Keeps the framework decoupled from domain-specific logic

---

## Decisions

### 1. Coarse-grained, self-organizing adapters

An adapter owns its entire processing pipeline. Internal phase ordering, file-type branching, LLM delegation (via llm-orc), and sub-phase events are the adapter's business. The framework sees only what comes out of the sink.

**Alternatives considered:**

- *Fine-grained pipeline stages* — framework orchestrates individual phases (parse, chunk, extract). Rejected: phase ordering varies by domain and file type. A PDF and a markdown file need entirely different phase sequences. Externalizing this creates a combinatorial configuration problem.
- *Tier-based scheduling* — adapters declare a tier (instant/fast/moderate/slow) and the framework schedules by tier. Rejected: tier assignment is file-type-dependent within a single adapter (markdown structure extraction is instant; PDF structure extraction requires LLM). Tiers belong inside the adapter, not in the framework.

### 2. Sink-based progressive emission (async)

Adapters receive an `AdapterSink` and `await sink.emit()` whenever they have results. Each emission commits immediately to the graph and fires events. The graph is always partially built — that is correct, not an error state.

`emit()` is async: the adapter awaits each emission and receives validation feedback (e.g., rejected edges with missing endpoints). The adapter layer spawns `process()` as a background task — async emit does not block other adapters. Adapters wrapping synchronous sources (e.g., a C library callback) can bridge internally via a channel.

**Alternatives considered:**

- *Return a complete result* — `process()` returns a single `AdapterOutput` when done. Rejected: blocks the UI until the slowest phase completes. A DocumentAdapter doing LLM extraction would produce no graph mutations for seconds or minutes.
- *Return a stream* — `process()` returns `Stream<AdapterOutput>`. Considered viable but less ergonomic for adapters that delegate to llm-orc ensembles with their own async event flows. The sink model lets the adapter push from any internal context.
- *Fire-and-forget emit* — `emit()` sends into a channel and returns immediately. Rejected: loses validation feedback (adapter never learns an emission was rejected) and backpressure (fast adapters can flood the engine). Since `process()` is already async and adapters wrapping llm-orc are already awaiting MCP calls, async emit adds no complexity.

### 3. Two trigger modes

Input-triggered adapters run when matching input arrives (`schedule() = None`). Scheduled adapters run on timer, mutation threshold, or graph condition (`schedule() = Some(Schedule)`). Both use the same `process(input, sink, cancel)` interface.

**Alternatives considered:**

- *Separate traits for input-triggered and scheduled adapters.* Rejected: unnecessary divergence. The processing model is identical — only the trigger differs.

### 4. Opaque adapter data

The framework routes input to adapters by matching `input_kind()` strings. The data payload is `Box<dyn Any + Send + Sync>` — the adapter downcasts internally. The framework never inspects the payload.

If the downcast fails (wrong type for the matched adapter), the adapter returns `Err(AdapterError::InvalidInput)`. The framework logs the error and continues — one adapter's type mismatch does not affect others. This is a programming error (router bug or input producer bug), not a runtime condition.

**Alternatives considered:**

- *Typed enum* (`AdapterData::FileContent { path, content }`, etc.) — Rejected: couples the framework to every domain's input shape. Adding a new domain means modifying the enum. The framework doesn't need to know what's inside.

### 5. Dual-obligation provenance **UPDATED by Essay 12, strengthened: bidirectional**

> **Updated.** The original "two-layer provenance" design (adapters annotate, engine wraps) remains valid for operational metadata. But Essay 12 ("Provenance as Epistemological Infrastructure") established a stronger requirement: adapters must produce epistemological provenance — chain and mark nodes in the provenance dimension — alongside their semantic output. Only the adapter understands its source material well enough to produce meaningful provenance.
>
> **Strengthened.** The dual obligation is bidirectional. Semantic adapters must produce provenance, AND provenance operations must produce semantic content. There is no consumer-facing path that creates provenance without semantic content. An annotation's text IS a fragment — it carries semantic meaning. Provenance without semantic content is bookkeeping; semantic content without provenance is ungrounded. Both halves are required.

Adapters have a dual obligation: **semantic contribution** (concepts, relationships) and **provenance contribution** (chains, marks, source evidence). Each adapter produces:

- **Semantic output:** domain-meaningful nodes and edges (e.g., fragment node, concept nodes, `tagged_with` edges)
- **Provenance output:** a chain node (deterministic ID: `chain:{adapter_id}:{source}`), mark nodes (with annotation text, source file, and tags), and `contains` edges (chain → mark)

The mark's tags trigger automatic tag-to-concept bridging via `TagConceptBridger` enrichment, creating cross-dimensional `references` edges from provenance to semantic dimension. This makes every concept's origin graph-traversable: concept ← `references` ← mark ← `contains` ← chain → source.

Adapters also continue to annotate nodes and edges with extraction metadata (confidence, method, source location). The engine continues to add framework context (adapter ID, timestamp, context ID). These two layers provide operational provenance. The chain/mark structure provides epistemological provenance — where knowledge came from, not just how it was processed.

**Alternatives considered (historical):**

- *Pipeline-level provenance* — the ingest pipeline wraps adapter output in provenance after processing. Rejected (Essay 12): the pipeline doesn't know what the nodes represent. Marks without domain-meaningful annotations are operational bookkeeping, not evidence.
- *Sink-level provenance* — EngineSink auto-generates provenance on commit. Rejected (Essay 12): same knowledge gap as pipeline-level. The sink validates and commits but doesn't understand domain semantics.
- *Adapter-level provenance* — the adapter that understands the source material produces provenance alongside semantics. **Accepted (Essay 12):** provenance without domain knowledge is empty.

### 6. Atomic sink emissions with validation

Each `sink.emit()` call validates and commits as a unit.

| Condition | Behavior |
|---|---|
| Edge references missing endpoint | **Reject** — must exist in graph or same emission |
| Duplicate node ID | **Upsert** — update properties |
| Removal of non-existent node | **No-op** |
| Empty emission | **No-op** |
| Self-referencing edge | **Allow** |

**Adapter authors must emit nodes before or in the same emission as edges that reference them.** This is a consequence of the rejection rule: an edge pointing to a node that doesn't exist yet is invalid. Progressive emission (Decision 2) naturally produces this ordering — structural nodes first, relationship edges later — but adapters that build complex subgraphs in a single emission must include all referenced endpoints.

Cancellation is checked between `emit()` calls, not during. Each emission is atomic.

### 7. No temporal decay — Hebbian normalization

Edge weights weaken through normalization as the graph grows, not through clock-based half-lives. A quiet graph stays stable — silence is not evidence against previous observations.

Per-adapter contributions are stored on each edge. Raw weight is computed from contributions via scale normalization (see ADR-003). Normalized weights are computed at query time via a pluggable `NormalizationStrategy`. Different consumers can interpret the same graph differently.

Default strategy: per-node outgoing divisive normalization.

```
w_normalized(i→j) = w_raw(i→j) / Σ_k w_raw(i→k)
```

Hebbian weakening emerges naturally: when a new edge from node A is reinforced, every other outgoing edge from A becomes relatively weaker in the normalized view without mutating those edges.

**Alternatives considered:**

- *Temporal half-life per context* — edges decay on a clock (weekly for Manza, monthly for Trellis). Rejected: normalization already handles relative weakening. A half-life is a second mechanism doing the same job. In a quiet graph, nothing should change — there's no new information to justify mutation.
- *Global normalization* — normalize across all edges in the graph. Rejected: a single high-traffic node would suppress weights across unrelated parts of the graph. Per-node normalization localizes the effect.

### 8. Shared semantic dimension with label-based bridging

All domains contribute concept nodes to the same semantic namespace. `ContentType` (a field on `Node`, defined in the Plexus core spec) disambiguates origin. Each adapter uses whatever vocabulary its domain produces — a MovementAdapter uses Laban terms, a DocumentAdapter uses whatever the LLM extracts. Adapters do not coordinate vocabulary.

When independent adapters happen to produce the same label (e.g., both emit `concept:sudden`), the system sees automatic cross-modal agreement — strong evidence. When they use different labels for related concepts (e.g., "abrupt" vs "sudden"), the NormalizationAdapter's near-miss detection proposes `may_be_related` edges. Graph dynamics determine which proposals are real.

No special unification logic. Labels are the bridge where vocabulary overlaps; the NormalizationAdapter bridges where it doesn't.

### 9. ~~Reflexive adapters propose, don't merge — enforced by ProposalSink~~ **SUPERSEDED by enrichment model (Essay 09)**

> **Status:** Superseded. The reflexive adapter and ProposalSink concepts have been replaced by **enrichments** — reactive components registered globally on the engine that respond to graph events and produce additional mutations. The "propose, don't merge" principle survives as a design convention: enrichments like CoOccurrenceEnrichment emit weak `may_be_related` edges; Hebbian reinforcement from actual evidence validates them. The structural enforcement (ProposalSink) is no longer needed — enrichments are framework-level code with built-in termination via idempotency. See domain model §10 (Adapter ≠ Enrichment) and invariants 35–40.

### 10. Five low-level graph events

Every `sink.emit()` produces mutations. The engine fires events per mutation type:

| Event | Trigger |
|---|---|
| `NodesAdded` | Nodes committed from emission |
| `EdgesAdded` | Edges committed (after validation) |
| `NodesRemoved` | Removals committed from emission |
| `EdgesRemoved` | Cascade from node removal, or cleanup |
| `WeightsChanged` | Reinforcement applied |

Higher-level events (topology shifts, cross-modal bridges) are modeled as nodes and edges emitted by enrichments — not as special event types. One event mechanism for everything.

### 11. Cross-adapter dependency via graph state

Adapters are independent — they don't know about each other. Enrichments depend on accumulated graph state and events, not on specific adapter outputs. This avoids ordering constraints and coupling between adapters.

---

## Consequences

**Positive:**

- Adapters are independently developable and testable — each is a self-contained unit with a clear contract
- Progressive emission enables responsive UIs across all application timescales
- Dual-obligation provenance (semantic + epistemological) makes every concept's origin graph-traversable
- Query-time normalization means the stored graph is ground truth — no information lost to decay
- The framework is domain-agnostic — new input types require a new adapter, not framework changes
- Enrichments enable graph self-improvement without destructive operations

**Negative:**

- Opaque adapter data (`Box<dyn Any>`) trades compile-time type safety for decoupling. A misrouted input causes a runtime downcast failure, not a compile error.
- Per-node normalization requires graph traversal at query time. For high-degree nodes this may need caching.
- Coarse-grained adapters may accumulate internal complexity. The framework can't help decompose them — that's the adapter author's responsibility.

---

## Open Questions

### Needs spike (blocks adapter implementation)

1. **Reinforcement mechanics.** What happens to `edge.weight` when a second adapter emits an edge that already exists? When a reflexive adapter confirms a `may_be_related` edge? When the same adapter re-emits an edge on re-processing? The documents describe Hebbian reinforcement as a property of the system but never define the operation. This blocks adapter development — you can't write an adapter that emits edges without knowing the contract. Candidates to explore in a spike: additive increment, multiplicative boost, source-diversity-weighted increment, or some combination. The spike should also clarify whether reinforcement is the sink's responsibility (implicit on duplicate edge) or an explicit adapter action.

### Deferred to future ADRs or implementation discovery

2. **AdapterSnapshot design.** Incremental state is likely adapter-specific (file: chunk hashes, movement: cluster centroids, graph: timestamp of last run).

3. **Chunking as graph nodes.** Should chunks be structure-dimension nodes (queryable, referenceable) or adapter-internal state?

4. **Canonical pointers vs pure emergence.** When `may_be_related` strengthens to high weight — designate one node as canonical, or keep both with a strong equivalence edge?

5. **Edge cleanup strategy.** Simple weight threshold, distribution-aware cutoff (power-law tail), or both?

6. **Session boundaries (EDDI).** Separate session contexts, temporal windowing, or session metadata on nodes/edges?
