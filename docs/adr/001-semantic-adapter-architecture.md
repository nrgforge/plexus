# ADR-001: Semantic Adapter Architecture

**Status:** Accepted

**Date:** 2026-02-02

**Design documents:** [semantic-adapters.md](../semantic-adapters.md) (overview), [semantic-adapters-design.md](../semantic-adapters-design.md) (system design)

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

### 2. Sink-based progressive emission

Adapters receive an `AdapterSink` and call `sink.emit()` whenever they have results. Each emission commits immediately to the graph and fires events. The graph is always partially built — that is correct, not an error state.

**Alternatives considered:**

- *Return a complete result* — `process()` returns a single `AdapterOutput` when done. Rejected: blocks the UI until the slowest phase completes. A DocumentAdapter doing LLM extraction would produce no graph mutations for seconds or minutes.
- *Return a stream* — `process()` returns `Stream<AdapterOutput>`. Considered viable but less ergonomic for adapters that delegate to llm-orc ensembles with their own async event flows. The sink model lets the adapter push from any internal context.

### 3. Two trigger modes

Input-triggered adapters run when matching input arrives (`schedule() = None`). Scheduled adapters run on timer, mutation threshold, or graph condition (`schedule() = Some(Schedule)`). Both use the same `process(input, sink, cancel)` interface.

**Alternatives considered:**

- *Separate traits for input-triggered and scheduled adapters.* Rejected: unnecessary divergence. The processing model is identical — only the trigger differs.

### 4. Opaque adapter data

The framework routes input to adapters by matching `input_kind()` strings. The data payload is `Box<dyn Any + Send + Sync>` — the adapter downcasts internally. The framework never inspects the payload.

**Alternatives considered:**

- *Typed enum* (`AdapterData::FileContent { path, content }`, etc.) — Rejected: couples the framework to every domain's input shape. Adding a new domain means modifying the enum. The framework doesn't need to know what's inside.

### 5. Two-layer provenance

Adapters annotate nodes and edges with what they know about the extraction: confidence, method, source location. The engine wraps each emission with structural context: adapter ID, timestamp, input summary, context ID. The engine constructs `ProvenanceEntry` records by combining both layers.

Adapters never build provenance entries directly. Nodes emitted without annotation still receive provenance marks (structural context only).

**Alternatives considered:**

- *Adapter builds full provenance* — adapter constructs `ProvenanceEntry` objects with all fields. Rejected: duplicates framework-known information (adapter ID, timestamp, context) and requires every adapter author to get it right. Inconsistent provenance formats across adapters.
- *Framework handles all provenance* — no adapter input, framework auto-generates everything. Rejected: the framework doesn't know epistemological detail — extraction method, confidence, source location within the input. Only the adapter knows *how* it came to know something.

### 6. Atomic sink emissions with validation

Each `sink.emit()` call validates and commits as a unit.

| Condition | Behavior |
|---|---|
| Edge references missing endpoint | **Reject** — must exist in graph or same emission |
| Duplicate node ID | **Upsert** — update properties |
| Removal of non-existent node | **No-op** |
| Empty emission | **No-op** |
| Self-referencing edge | **Allow** |

Cancellation is checked between `emit()` calls, not during. Each emission is atomic.

### 7. No temporal decay — Hebbian normalization

Edge weights weaken through normalization as the graph grows, not through clock-based half-lives. A quiet graph stays stable — silence is not evidence against previous observations.

Raw reinforcement weights are stored. Normalized weights are computed at query time via a pluggable `NormalizationStrategy`. Different consumers can interpret the same graph differently.

Default strategy: per-node outgoing divisive normalization.

```
w_normalized(i→j) = w_raw(i→j) / Σ_k w_raw(i→k)
```

Hebbian weakening emerges naturally: when a new edge from node A is reinforced, every other outgoing edge from A becomes relatively weaker in the normalized view without mutating those edges.

**Alternatives considered:**

- *Temporal half-life per context* — edges decay on a clock (weekly for Manza, monthly for Trellis). Rejected: normalization already handles relative weakening. A half-life is a second mechanism doing the same job. In a quiet graph, nothing should change — there's no new information to justify mutation.
- *Global normalization* — normalize across all edges in the graph. Rejected: a single high-traffic node would suppress weights across unrelated parts of the graph. Per-node normalization localizes the effect.

### 8. Shared semantic dimension with label-based bridging

All domains contribute concept nodes to the same semantic namespace. `ContentType` disambiguates origin. Cross-modal bridging happens through shared vocabulary — when a MovementAdapter produces `concept:sudden` and a DocumentAdapter already created that concept from text, the system sees independent agreement across modalities.

No special unification logic. Labels are the bridge.

### 9. Reflexive adapters propose, don't merge

Scheduled adapters that examine the graph (NormalizationAdapter, TopologyAdapter, CoherenceAdapter) emit weak `may_be_related` edges. They never merge nodes or overwrite data. Graph dynamics — Hebbian reinforcement and normalization — determine which proposed connections are real.

**Alternatives considered:**

- *Auto-merge on high confidence* — when a `may_be_related` edge exceeds a threshold, merge the two nodes. Rejected: two similarly-labeled concepts may be entirely different in context. Merging is destructive and irreversible. Strong equivalence edges preserve both nodes and their distinct provenance.

### 10. Five low-level graph events

Every `sink.emit()` produces mutations. The engine fires events per mutation type:

| Event | Trigger |
|---|---|
| `NodesAdded` | Nodes committed from emission |
| `EdgesAdded` | Edges committed (after validation) |
| `NodesRemoved` | Removals committed from emission |
| `EdgesRemoved` | Cascade from node removal, or cleanup |
| `WeightsChanged` | Reinforcement applied |

Higher-level events (topology shifts, cross-modal bridges) are modeled as nodes and edges emitted by reflexive adapters — not as special event types. One event mechanism for everything.

### 11. Cross-adapter dependency via graph state

External adapters are independent — they don't know about each other. Reflexive adapters depend on accumulated graph state, not on specific adapter outputs. This avoids ordering constraints and coupling between adapters.

---

## Consequences

**Positive:**

- Adapters are independently developable and testable — each is a self-contained unit with a clear contract
- Progressive emission enables responsive UIs across all application timescales
- Two-layer provenance is consistent without burdening adapter authors
- Query-time normalization means the stored graph is ground truth — no information lost to decay
- The framework is domain-agnostic — new input types require a new adapter, not framework changes
- Reflexive adapters enable graph self-improvement without destructive operations

**Negative:**

- Opaque adapter data (`Box<dyn Any>`) trades compile-time type safety for decoupling. A misrouted input causes a runtime downcast failure, not a compile error.
- Per-node normalization requires graph traversal at query time. For high-degree nodes this may need caching.
- Coarse-grained adapters may accumulate internal complexity. The framework can't help decompose them — that's the adapter author's responsibility.

---

## Open Questions

These are deferred to future ADRs or implementation discovery:

1. **AdapterSnapshot design.** Incremental state is likely adapter-specific (file: chunk hashes, movement: cluster centroids, graph: timestamp of last run).

2. **Chunking as graph nodes.** Should chunks be structure-dimension nodes (queryable, referenceable) or adapter-internal state?

3. **Canonical pointers vs pure emergence.** When `may_be_related` strengthens to high confidence — designate one node as canonical, or keep both with a strong equivalence edge?

4. **Edge cleanup strategy.** Simple weight threshold, distribution-aware cutoff (power-law tail), or both?

5. **Session boundaries (EDDI).** Separate session contexts, temporal windowing, or session metadata on nodes/edges?
