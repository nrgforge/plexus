# Behavior Scenarios: First Adapter Pair

Derived from [ADR-004](../adr/004-first-adapter-pair.md), [ADR-005](../adr/005-normalization-floor.md), and the [domain model](../domain-model.md). Each scenario is refutable — it can be clearly true or false against running software. All terms follow the domain model vocabulary.

**Scope:** FragmentAdapter, CoOccurrenceAdapter, and normalization floor. For adapter infrastructure (emission validation, sinks, provenance, routing), see [001-semantic-adapter-layer.md](001-semantic-adapter-layer.md). For contribution tracking and scale normalization, see [003-reinforcement-mechanics.md](003-reinforcement-mechanics.md).

---

## Feature: FragmentAdapter Emits Graph Structure from Tagged Fragments

The FragmentAdapter is an external adapter that maps a fragment (text + tags) to fragment nodes, concept nodes, and `tagged_with` edges.

### Scenario: Single fragment with tags produces fragment node, concept nodes, and edges
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**And** an empty graph
**When** the adapter processes a fragment with text "Walked through Avignon" and tags ["travel", "avignon"]
**Then** the graph contains 3 nodes: 1 fragment node and 2 concept nodes
**And** the fragment node has content type Document and dimension "structure"
**And** concept node "concept:travel" has content type Concept and dimension "semantic"
**And** concept node "concept:avignon" has content type Concept and dimension "semantic"
**And** 2 `tagged_with` edges exist: fragment→concept:travel and fragment→concept:avignon
**And** each `tagged_with` edge has contribution {"manual-fragment": 1.0}

### Scenario: Two fragments sharing a tag converge on the same concept node
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**When** the adapter processes fragment F1 with tags ["travel", "avignon"]
**And** then processes fragment F2 with tags ["travel", "paris"]
**Then** the graph contains 2 fragment nodes (F1, F2)
**And** the graph contains 3 concept nodes: concept:travel, concept:avignon, concept:paris
**And** concept:travel was upserted (not duplicated) — node count is 5, not 6
**And** 4 `tagged_with` edges exist: F1→travel, F1→avignon, F2→travel, F2→paris

### Scenario: Tag case normalization ensures convergence
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**When** the adapter processes fragment F1 with tags ["Travel"]
**And** then processes fragment F2 with tags ["travel"]
**Then** both produce concept node with ID "concept:travel"
**And** the graph contains exactly 1 concept node (not 2)

### Scenario: Fragment with no tags produces only the fragment node
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**When** the adapter processes a fragment with text "A thought" and tags []
**Then** the graph contains 1 fragment node
**And** the graph contains 0 concept nodes
**And** the graph contains 0 edges

### Scenario: Fragment adapter emits all items in a single emission
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**When** the adapter processes a fragment with tags ["travel", "avignon"]
**Then** exactly 1 call to `sink.emit()` is made
**And** the emission contains the fragment node, concept nodes, and edges together
**And** the emission is fully committed (no rejections)

---

## Feature: Configurable Adapter Identity for Multi-Source Evidence

One FragmentAdapter type serves multiple evidence sources. Each instance has a different adapter ID, producing distinct contributions and provenance entries.

### Scenario: Two adapter instances produce separate contribution slots
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**And** a FragmentAdapter with adapter ID "llm-fragment"
**When** "manual-fragment" processes fragment F1 with tags ["travel"]
**And** "llm-fragment" processes fragment F2 with tags ["travel"]
**Then** edge F1→concept:travel has contributions {"manual-fragment": 1.0}
**And** edge F2→concept:travel has contributions {"llm-fragment": 1.0}

### Scenario: Same concept from different sources shows evidence diversity
**Given** "manual-fragment" and "llm-fragment" have both emitted concept:travel
**Then** concept:travel has provenance entries from two distinct adapter IDs
**And** the concept node exists exactly once (upserted by both adapters)

---

## Feature: CoOccurrenceEnrichment Proposes Relationships Between Co-Occurring Concepts

The CoOccurrenceEnrichment is an enrichment that detects co-occurrence (concepts sharing fragments) and emits `may_be_related` edges during the enrichment loop.

### Scenario: Two concepts sharing one fragment get a co-occurrence proposal
**Given** concept:travel and concept:avignon both appear on fragment F1 (via `tagged_with` edges)
**And** no other fragments exist
**When** the CoOccurrenceEnrichment processes a context snapshot
**Then** a `may_be_related` edge is emitted between concept:travel and concept:avignon
**And** the co-occurrence score is 1.0 (1 shared fragment / 1 max)

### Scenario: Two concepts sharing multiple fragments score higher than single-shared
**Given** concept:travel and concept:avignon co-occur on fragments F1 and F2
**And** concept:travel and concept:paris co-occur on fragment F2 only
**When** the CoOccurrenceEnrichment processes a context snapshot
**Then** concept:travel ↔ concept:avignon has co-occurrence score 1.0 (2/2 max)
**And** concept:travel ↔ concept:paris has co-occurrence score 0.5 (1/2 max)

### Scenario: Concepts with no shared fragments get no proposal
**Given** concept:travel appears only on fragment F1
**And** concept:morning appears only on fragment F3
**And** F1 and F3 share no tags
**When** the CoOccurrenceEnrichment processes a context snapshot
**Then** no `may_be_related` edge is emitted between concept:travel and concept:morning

### Scenario: Co-occurrence proposals are symmetric edge pairs
**Given** concept:travel and concept:avignon co-occur on fragment F1
**When** the CoOccurrenceEnrichment processes a context snapshot
**Then** two directed edges are emitted: concept:travel→concept:avignon and concept:avignon→concept:travel
**And** both edges have relationship `may_be_related`
**And** both edges have the same contribution value

### Scenario: Co-occurrence contribution is self-capped by enrichment
**Given** the CoOccurrenceEnrichment has a configured contribution cap of 0.5
**And** concept:travel and concept:avignon have co-occurrence score 1.0
**When** the enrichment emits the proposal
**Then** the `may_be_related` edge contribution is capped to 0.5

### Scenario: Empty graph produces no proposals
**Given** the graph contains no `tagged_with` edges
**When** the CoOccurrenceEnrichment processes a context snapshot
**Then** no emissions are made (the enrichment returns None)

### Scenario: CoOccurrenceEnrichment reads context snapshot, not live state
**Given** the framework creates a context snapshot of the current Context
**When** the CoOccurrenceEnrichment receives the snapshot during the enrichment loop
**Then** the enrichment reads `tagged_with` edges from the snapshot
**And** no mutations to the live graph affect the enrichment's view

---

## Feature: Normalization Floor (ADR-005)

Scale normalization uses dynamic epsilon to prevent the weakest real contribution from mapping to 0.0. The normalization floor is proportionally equal for all adapters.

### Scenario: Minimum contribution maps to floor, not zero
**Given** adapter "co-occurrence" has contributions on two edges: A→B = 0.5, A→C = 1.0
**And** floor coefficient α = 0.01
**When** the engine computes scale-normalized contributions
**Then** co-occurrence min = 0.5, max = 1.0, range = 0.5
**And** ε = 0.01 × 0.5 = 0.005
**And** scale-normalized A→B = (0.5 - 0.5 + 0.005) / (0.5 + 0.005) = 0.005 / 0.505 ≈ 0.0099
**And** scale-normalized A→C = (1.0 - 0.5 + 0.005) / 0.505 = 0.505 / 0.505 = 1.0
**And** A→B raw weight is non-zero (approximately 0.0099)

### Scenario: Floor is proportionally equal across adapters with different ranges
**Given** adapter "co-occurrence" has contributions spanning range 0.5 (min=0.5, max=1.0)
**And** adapter "code-coverage" has contributions spanning range 99 (min=1, max=100)
**And** floor coefficient α = 0.01
**When** the engine computes scale-normalized contributions for both adapters
**Then** co-occurrence minimum maps to α / (1 + α) ≈ 0.0099
**And** code-coverage minimum maps to α / (1 + α) ≈ 0.0099
**And** both adapters have the same proportional floor

### Scenario: Degenerate case unchanged — single value normalizes to 1.0
**Given** adapter "co-occurrence" has contributions on one edge only: A→B = 0.7
**And** floor coefficient α = 0.01
**When** the engine computes scale-normalized contributions
**Then** co-occurrence min = 0.7, max = 0.7, range = 0.0
**And** scale-normalized A→B = 1.0 (degenerate case, unchanged from ADR-003)

### Scenario: Normalization floor preserves relative ordering
**Given** adapter "co-occurrence" has contributions: A→B = 1.0, A→C = 3.0, A→D = 5.0
**And** floor coefficient α = 0.01
**When** the engine computes scale-normalized contributions
**Then** scale-normalized A→B < scale-normalized A→C < scale-normalized A→D
**And** scale-normalized A→D = 1.0
**And** scale-normalized A→B > 0.0

---

## Feature: End-to-End — Fragment Processing Through Co-Occurrence Detection

The full pipeline: fragments enter via FragmentAdapter, co-occurrence is detected by CoOccurrenceAdapter, and the graph contains both structural and propositional edges.

### Scenario: Three fragments produce tagged_with and may_be_related edges
**Given** a FragmentAdapter with adapter ID "manual-fragment"
**And** a CoOccurrenceAdapter with adapter ID "co-occurrence" and ProposalSink cap 1.0
**When** the FragmentAdapter processes:
  - Fragment F1 with tags ["travel", "avignon", "walking"]
  - Fragment F2 with tags ["travel", "avignon", "paris"]
  - Fragment F3 with tags ["walking", "nature"]
**And** the framework creates a graph state snapshot
**And** the CoOccurrenceAdapter processes the snapshot
**Then** the graph contains 3 fragment nodes and 5 concept nodes (travel, avignon, walking, paris, nature)
**And** 9 `tagged_with` edges exist (3 + 3 + 2 per fragment, one per tag)
**And** `may_be_related` symmetric edge pairs exist between all co-occurring concept pairs
**And** concept:travel ↔ concept:avignon has the highest co-occurrence score (2 shared fragments)
**And** all `tagged_with` edges have contributions from "manual-fragment"
**And** all `may_be_related` edges have contributions from "co-occurrence"

### Scenario: Re-running the CoOccurrenceAdapter with unchanged graph is idempotent
**Given** the CoOccurrenceAdapter has already processed a graph state snapshot and proposed `may_be_related` edges
**When** the CoOccurrenceEnrichment processes a new snapshot of the same graph (no new fragments)
**Then** all `may_be_related` edge contributions are replaced with the same values (latest-value-replace)
**And** no `WeightsChanged` events fire (contributions unchanged)

---

## Not Covered (Open Questions)

The following scenarios cannot be written until their design questions are resolved:

- **Node property merge on multi-source upsert:** When two different adapters emit the same concept node with different properties, what merge semantics apply? Not exercised by this pair (single FragmentAdapter type produces identical concept nodes). See domain model OQ1.
- **Enrichment loop integration:** The full pipeline (fragment ingestion → enrichment loop → co-occurrence detection → outbound events) is not exercised end-to-end. Enrichment loop scenarios will be written during the public surface build phase.
