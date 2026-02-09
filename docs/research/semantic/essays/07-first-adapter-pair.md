# The First Adapter Pair: From Infrastructure to Evidence

The Plexus adapter layer is a machine for turning domain-specific observations into shared knowledge. ADR-001 defined its architecture: self-organizing adapters, sink-based progressive emission, two-layer provenance, Hebbian normalization. ADR-003 added the reinforcement mechanics: per-adapter contributions, scale normalization, latest-value-replace. Fifty-seven tests prove the infrastructure works. But infrastructure without adapters is plumbing without water. This essay describes the design of the first concrete adapter pair — a fragment adapter and a co-occurrence adapter — that will validate the plumbing end-to-end.

## The simplest useful input

The target application is Trellis, a system for accumulating writing fragments over time. Fragments arrive from many sources: SMS messages, email, OCR'd journal scans, markdown notes. Each fragment carries text and tags — sometimes applied manually ("travel", "avignon", "2007"), sometimes by an LLM extracting concepts from the text.

A tagged fragment is the simplest input that produces interesting graph structure. The human (or LLM) has already done the extraction work. The adapter's job is structural mapping: turn `{text, tags}` into graph mutations. No NLP, no LLM calls, no chunking strategy. Just nodes and edges.

This is deliberately boring. The first adapter should validate infrastructure, not pioneer extraction techniques. If the plumbing leaks, we want to know it's the plumbing, not the adapter's complexity.

## FragmentAdapter: external, structural

The FragmentAdapter declares `input_kind: "fragment"` and receives a `FragmentInput` containing text, tags, an optional source identifier, and an optional date. Its `process()` method does three things:

1. Creates a **fragment node** — a unique document node in the structure dimension, carrying the text and metadata as properties.
2. Creates a **concept node per tag** — in the semantic dimension, with a deterministic ID derived from the lowercase tag label. `concept:travel` is the same node regardless of which fragment produced it.
3. Creates **edges from fragment to concept** — relationship `tagged_with`, contribution value 1.0 (binary: this tag was applied).

All three go into a single emission. Edges reference nodes in the same emission, satisfying the endpoint validation rule.

The deterministic concept IDs are the key design choice. When a second fragment is also tagged "travel", it emits the same `concept:travel` node. The engine upserts — same ID, same properties, no information lost. The `tagged_with` edge from the new fragment to the existing concept is a new edge (different source node), adding to the graph's evidence that "travel" is a meaningful concept.

### One adapter, many sources

Manual tagging and LLM tagging don't need separate adapter types. They need separate adapter *identities*. The FragmentAdapter is instantiated with a configurable ID: `"manual-fragment"` for human-tagged input, `"llm-fragment"` for machine-tagged input. ADR-003's per-adapter contribution tracking does the rest — each source gets its own slot on every edge, provenance records which source produced which tag, and evidence diversity counts both as independent observations.

This pattern scales: `"ocr-fragment"`, `"voice-transcription-fragment"`, `"email-fragment"`. Same `process()` logic, different identities. The system sees independent evidence sources without any adapter code changes.

## CoOccurrenceAdapter: reflexive, propositional

Once fragments populate the graph with concepts and tagged_with edges, patterns emerge. Concepts that are frequently tagged together across fragments — "travel" and "avignon", "walking" and "nature" — are probably related. The CoOccurrenceAdapter detects these patterns and proposes relationships.

It declares `input_kind: "graph_state"` and receives a cloned Context as its input payload. Its `process()` method:

1. Finds all `tagged_with` edges and builds a reverse index: fragment → concepts.
2. For each pair of concepts that share at least one fragment, counts the shared fragments.
3. Normalizes counts relative to the maximum (strongest pair = 1.0).
4. Emits `may_be_related` edges between co-occurring concepts via the ProposalSink, with the normalized score as the contribution value.

The ProposalSink enforces the propose-don't-merge invariant: only `may_be_related` edges, contribution values clamped to a cap, no removals. The co-occurrence adapter doesn't need to know about these constraints — it just calls `sink.emit()` and the ProposalSink intercepts.

### Symmetric edges in a directed graph

`may_be_related` is semantically symmetric — if avignon may be related to travel, then travel may be related to avignon. But Plexus edges are directed. This matters because query-time normalization is outgoing-divisive: it only considers edges leaving a node. A one-directional `may_be_related` edge would be visible from one endpoint and invisible from the other.

The co-occurrence adapter emits both directions: `avignon → travel` and `travel → avignon`, with identical contribution values. This is the standard representation of undirected relationships in a directed graph. The storage cost is modest (2M edges for M concept pairs), and normalization works correctly from either endpoint.

### Reading graph state

The reflexive adapter needs to read the graph, but the AdapterSink is write-only. The architecture's answer: the framework (or schedule monitor, when it exists) clones the Context and passes it as the opaque payload in AdapterInput. The adapter downcasts to `&Context` and reads.

This maintains the abstraction boundary — the adapter depends on the graph model (`Context`, `Node`, `Edge`), not on engine internals. And the snapshot gives the adapter a consistent view, unaffected by concurrent mutations from other adapters.

For testing without a schedule monitor, the test harness takes the snapshot and triggers the adapter directly. The schedule monitor is infrastructure for *when* to trigger; the adapter's logic is independent of the trigger mechanism.

## Scale normalization: the epsilon fix

ADR-003's scale normalization uses divide-by-range: `(value - min) / (max - min)`. This maps each adapter's contributions to [0, 1], preventing high-magnitude adapters from dominating. But it has a known issue: the minimum contribution maps to exactly 0.0. Real evidence — a concept pair that co-occurs once — becomes invisible.

A static epsilon (adding a small constant) partially fixes this but introduces unfairness: the floor is proportionally larger for narrow-range adapters than wide-range adapters.

A dynamic epsilon scales with the adapter's range: `ε = α × (max - min)` for a small constant α (0.01). The normalized formula becomes:

```
(value - min + α·range) / ((1 + α)·range)
```

At the minimum: `α / (1 + α)` ≈ α — the same proportional floor for every adapter, regardless of range. At the maximum: exactly 1.0. The degenerate case (single value, range = 0) retains the existing behavior: normalize to 1.0.

With α = 0.01, the weakest real evidence from any adapter maps to ~1% of that adapter's strongest evidence. This is a principled position: the minimum is small but non-zero, the relative ordering is preserved, and every adapter is treated fairly.

## What the first pair doesn't test

The domain model identifies four open questions. This adapter pair exercises none of them directly — and that's fine.

**Node property merge** only matters when two *different* external adapters emit the same node with different properties. A single FragmentAdapter emitting the same concept node is idempotent.

**Reflexive cycle convergence** requires a schedule monitor that triggers on mutations. Without the monitor, there's no loop.

**ProposalSink metadata edges** are needed by topology adapters (community membership), not by co-occurrence adapters (pairwise similarity).

**Routing fan-out semantics** matter when two adapters share an input kind. This pair uses different kinds.

All four open questions resolve inside the engine, the sink, the router, or the monitor — never inside the adapter. The adapter interface acts as the isolation boundary. Code written now is unlikely to need rework — the adapter interface isolates adapter code from engine-internal resolution of these questions.

## What needs building

The build phase produces five artifacts:

1. **FragmentInput** — a struct with text, tags, source, and date fields.
2. **FragmentAdapter** — implements the Adapter trait. Approximately 50 lines of `process()` logic.
3. **CoOccurrenceAdapter** — implements the Adapter trait. Approximately 60 lines of `process()` logic.
4. **Dynamic epsilon** — a small change to `Context.recompute_raw_weights()`.
5. **Integration tests** — wiring both adapters together with real emissions and verifying end-to-end behavior.

No new traits, no new infrastructure modules, no engine changes beyond the epsilon fix. The adapter layer's design proves itself here: the first real adapters require only the adapter layer's public API.
