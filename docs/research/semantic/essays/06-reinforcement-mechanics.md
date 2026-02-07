# Edge Reinforcement in Multi-Adapter Knowledge Graphs

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — February 2026*

---

## Why This Matters for Plexus

Plexus is a knowledge graph engine designed for live creative composition. Unlike batch-processing systems that index finished documents, Plexus integrates with creative environments and builds a semantic graph that evolves as content is composed. It receives data from domain-specific *adapters* — pluggable components that extract structure from code, documents, movement, and other domains — and emits events that drive ambient structural feedback. Three applications sit on the engine: Manza (AI-assisted code generation), Trellis (long-form writing), and EDDI (interactive movement performance).

The multi-adapter architecture is central to Plexus's design. In Manza, a SystemsArchitectureAdapter might map module dependencies while a CodeCoverageAdapter tracks which components are validated by tests. As a system is built out, edges should strengthen for parts of the architecture that are both structurally connected and well-tested — the convergence of two independent signals is evidence that a relationship is real and robust. In EDDI, a MovementAdapter captures gesture sequences and transition frequencies while a DocumentAdapter extracts concepts from Laban or Viewpoints literature. When a performer executes a "sudden, direct" gesture and the system has independently ingested text describing that quality, the cross-modal agreement — the same concept observed through body and through text — is strong evidence of a meaningful connection.

These adapter pairings create a concrete question that the existing codebase leaves unanswered: when two adapters independently emit the same edge, what happens to its weight?

## The Problem

The question is deceptively simple on the surface, but the answer depends on who is talking. Consider two adapters operating in the Manza environment. A CodeCoverageAdapter watches a test suite and emits edges between components that are validated by tests — its weight represents the number of distinct tests covering a relationship. When the suite re-runs with the same tests, nothing has changed. Repetition is noise; only net-new tests provide fresh evidence. A SystemsArchitectureAdapter, by contrast, might re-analyze module boundaries as code changes. If it re-confirms the same dependency after a refactor, that re-confirmation is mildly informative — the relationship survived structural change.

In EDDI the distinction is sharper. A MovementAdapter watches gesture sequences, and when a performer repeats the same gesture, that repetition *is* the signal — it indicates the gesture is becoming part of the performer's vocabulary for this piece. A DocumentAdapter extracting concepts from Laban literature, on the other hand, should not reinforce an edge just because the same paper is re-ingested. The concept "sudden" appears in the text exactly as many times as it appears; re-reading the paper adds no new information.

Same operation (re-emit an existing edge), opposite semantics. The engine cannot hardcode a single policy.

## What Existing Systems Do

The design space for multi-source weight management has been explored across knowledge graph systems, neuroscience-inspired architectures, and distributed systems.

**Knowledge graph systems** generally fall into two camps. Google's Knowledge Vault and CMU's NELL both use engine-controlled fusion: multiple extractors propose facts with confidence scores, and a central mechanism decides what to believe. NELL's key insight is that source diversity matters — a belief proposed by three independent subsystems is stronger than one proposed by the same subsystem three times. At the other end, Neo4j's MERGE clause puts the caller in full control: the ON MATCH handler can implement any update logic. This works for single-caller scenarios but produces inconsistent weights when multiple independent producers use different strategies on the same edges.

**Hebbian learning** in neural networks provides the theoretical grounding. The classical rule — "neurons that fire together wire together" — is additive and unbounded. Every successful system built on Hebbian principles adds a normalization mechanism: Oja's rule (unit-norm convergence), BCM theory (sliding threshold), or soft bounds (decaying learning rate near maximum). Miconi's differentiable plasticity separates fixed weights (stable baseline) from plastic traces (Hebbian accumulation with decay), letting the system learn per-connection how much plasticity to allow.

**CRDTs** offer a useful structural analogy. A G-Counter tracks increments per-replica, taking the maximum within each replica during merge and summing across replicas for the total. The key insight is the per-replica decomposition: each source gets its own slot, and the aggregate is computed from all slots. The merge function is a structural property of the data type — no replica can override it.

## Per-Adapter Contribution Tracking

The CRDT insight maps onto the adapter architecture:

- Each edge stores per-adapter contributions: `HashMap<AdapterId, f32>`
- When an adapter emits an edge that already exists, the engine replaces that adapter's contribution slot with the new value
- The edge's raw weight is the sum across all scale-normalized adapter contributions

The G-Counter pattern inspired this decomposition, but the merge rule diverges from a true G-Counter. A G-Counter uses `max` within each replica, which means values can only increase — a monotonicity constraint. This is actively harmful for Plexus: if tests are deleted, the CodeCoverageAdapter needs to emit a lower contribution and have the engine honor it. If evidence is retracted or reassessed, the adapter's slot should reflect the current state, not the historical peak. Latest-value-replace handles both increases and decreases while remaining naturally idempotent for unchanged values — emitting 3 twice still gives 3. The G-Counter's commutativity guarantee is unnecessary since adapters emit sequentially (each awaits `emit()`), so out-of-order writes cannot occur.

The CodeCoverageAdapter emits a contribution proportional to test coverage for a relationship. Re-running the same suite emits the same value; replace(3, 3) = 3, no change. Adding a test emits a higher value; replace(3, 4) = 4, update. Deleting a test emits a lower value; replace(4, 3) = 3, the edge correctly weakens. The MovementAdapter emits a contribution reflecting cumulative gesture importance. Each meaningful repetition produces a higher value in the adapter's internal accounting; the replace catches the update. Both adapters get the semantics they need from the same engine-level rule, because each adapter owns the computation of *what its contribution value means*.

In the cross-modal case — a DocumentAdapter has extracted the concept "sudden" from Laban literature and a MovementAdapter has observed sudden gestures in performance — each contributes independently to the same edge. The sum of their contributions reflects cross-modal agreement without either adapter knowing the other exists.

## The Scale Problem

The per-adapter contribution pattern has one critical failure mode: scale dominance. When adapters emit weights on fundamentally different scales, naive summation lets the highest-magnitude adapter drown out all others.

A numerical spike confirmed this. With a CodeCoverageAdapter emitting on a 1–20 scale (test counts) and a MovementAdapter emitting on a 1–500 scale (gesture repetitions), an edge with 2 tests and 400 gestures (raw sum: 402) outranks an edge with 18 tests and 300 gestures (raw sum: 318) — even though the second edge is stronger in *both* domains relative to its peers. The movement scale makes code coverage invisible.

The fix is per-adapter normalization before summing. The engine normalizes each adapter's contributions to a comparable range, then sums the normalized values. With normalization, the edge strong in both domains correctly ranks first.

This normalization can happen on either side — the adapter could self-normalize before emitting, or the engine could normalize per-adapter internally. But having the engine do it is the better choice. Adapters should not need to know what scale other adapters use. A MovementAdapter emitting gesture counts on 0–500, a MidiAdapter emitting note velocities on 0–127, and a SentimentAdapter emitting polarity on -1.0–1.0 should all just work. The engine already handles structural provenance context (adapter ID, timestamp, input summary); handling scale normalization is a natural extension of that responsibility. The initial normalization function — divide-by-range (`(value - min) / (max - min)`) — maps any adapter's contributions to [0, 1] regardless of whether its native scale is signed or unsigned.

This produces a three-layer weight model: contributions (stored per-adapter on each edge), raw weight (computed by the engine from scale-normalized contributions), and normalized weight (computed at query time by consumers per ADR-001 Decision 7). Scale normalization and query-time normalization serve different purposes — the former ensures fairness across sources, the latter provides an interpretive lens for relative importance per-node. Both are necessary.

## Source Diversity

One question the spike surfaced but did not fully resolve: should cross-adapter agreement count for more than single-adapter strength? If the SystemsArchitectureAdapter alone contributes 0.8 to an edge, and a different edge receives 0.4 from SystemsArchitecture and 0.4 from CodeCoverage, they tie under naive summation. But the second edge has independent corroboration — two different analytical lenses observed the same relationship.

NELL's research suggests this matters. The per-adapter HashMap makes a diversity bonus trivial to implement, but the right coefficient is an empirical question that can be deferred. The data structure supports it whenever the need becomes clear.

## Design Consequences

The per-adapter contribution model resolves ADR-001's Open Question 1 (reinforcement mechanics) with several structural consequences:

**Edge storage grows.** Each edge carries a HashMap instead of a single f32. This is the price of provenance. The payoff: full attribution (who contributed what), clean rollback (remove an adapter's contribution without recalculation), and natural WeightsChanged event firing (when any adapter's contribution changes).

**Contributions can decrease.** Unlike a G-Counter, latest-value-replace allows an adapter to lower its assessment. Deleted tests, retracted evidence, and reassessed relationships are first-class operations. The engine stores whatever the adapter says its current contribution is.

**raw_weight becomes a computed property.** It is no longer a stored f32 but the sum of scale-normalized per-adapter contributions. Query-time normalization (OutgoingDivisive, Softmax) operates on this computed value, unchanged from the current design.

**Adapters are fully independent.** No adapter needs to know about any other adapter's existence, scale, or update semantics. The engine mediates. This aligns with ADR-001 Decision 11 (cross-adapter dependency via graph state, not direct coupling).

**The engine's merge rule is universal.** Replace-per-adapter, scale-normalize-per-adapter, sum-across-adapters. No per-adapter configuration, no strategy registration, no merge hints. The rule is structural — part of the data type, not a runtime decision.
