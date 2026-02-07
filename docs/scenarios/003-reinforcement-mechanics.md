# Behavior Scenarios: Reinforcement Mechanics

Derived from [ADR-003](../adr/003-reinforcement-mechanics.md) and the [domain model](../domain-model.md). Each scenario is refutable — it can be clearly true or false against running software. All terms follow the domain model vocabulary.

**Scope:** Per-adapter contribution tracking, scale normalization, and WeightsChanged events. For adapter layer infrastructure (emission validation, sinks, provenance, routing), see [001-semantic-adapter-layer.md](001-semantic-adapter-layer.md).

---

## Feature: Per-Adapter Contribution Tracking

Each edge stores per-adapter contributions as `HashMap<AdapterId, f32>`. When an adapter emits an edge that already exists, the engine replaces that adapter's contribution slot with the new value (latest-value-replace). See ADR-003 Decision 1.

### Scenario: First emission creates contribution slot
**Given** a graph containing node A and node B
**When** adapter "code-coverage" emits an emission containing edge A→B with contribution value 5.0
**Then** edge A→B exists in the graph
**And** edge A→B has contributions {"code-coverage": 5.0}

### Scenario: Same adapter re-emits same value — idempotent
**Given** edge A→B exists with contributions {"code-coverage": 5.0}
**When** adapter "code-coverage" emits an emission containing edge A→B with contribution value 5.0
**Then** edge A→B contributions are unchanged: {"code-coverage": 5.0}
**And** no `WeightsChanged` event fires

### Scenario: Same adapter emits higher value — contribution increases
**Given** edge A→B exists with contributions {"code-coverage": 5.0}
**When** adapter "code-coverage" emits an emission containing edge A→B with contribution value 8.0
**Then** edge A→B contributions are {"code-coverage": 8.0}
**And** a `WeightsChanged` event fires for edge A→B

### Scenario: Same adapter emits lower value — contribution decreases
**Given** edge A→B exists with contributions {"code-coverage": 8.0}
**When** adapter "code-coverage" emits an emission containing edge A→B with contribution value 3.0
**Then** edge A→B contributions are {"code-coverage": 3.0}
**And** a `WeightsChanged` event fires for edge A→B

### Scenario: Different adapter emits same edge — cross-source reinforcement
**Given** edge A→B exists with contributions {"code-coverage": 5.0}
**When** adapter "systems-architecture" emits an emission containing edge A→B with contribution value 0.7
**Then** edge A→B contributions are {"code-coverage": 5.0, "systems-architecture": 0.7}
**And** a `WeightsChanged` event fires for edge A→B

### Scenario: Re-processing with unchanged results is idempotent across all edges
**Given** adapter "code-coverage" has previously emitted edges A→B (5.0) and A→C (3.0)
**When** adapter "code-coverage" re-processes the same input and emits the same edges with the same contribution values
**Then** no contributions change
**And** no `WeightsChanged` events fire

### Scenario: Reflexive proposal then external confirmation — independent contribution slots
**Given** reflexive adapter "normalization-adapter" has proposed edge concept:sudden→concept:abrupt via ProposalSink with contribution value 0.2
**And** edge concept:sudden→concept:abrupt exists with contributions {"normalization-adapter": 0.2}
**When** adapter "document-adapter" independently emits edge concept:sudden→concept:abrupt with contribution value 0.85
**Then** edge contributions are {"normalization-adapter": 0.2, "document-adapter": 0.85}
**And** a `WeightsChanged` event fires
**And** the edge's raw weight reflects both contributions (stronger than either alone)

---

## Feature: Scale Normalization

The engine normalizes each adapter's contributions to a comparable scale before summing into raw weight. Scale normalization uses divide-by-range: `(value - min) / (max - min)` per adapter across all of that adapter's edges. See ADR-003 Decision 2.

### Scenario: Single adapter, single edge — degenerate case normalizes to 1.0
**Given** adapter "code-coverage" has emitted exactly one edge: A→B with contribution 5.0
**When** the engine computes the scale-normalized contribution
**Then** code-coverage min = 5.0, max = 5.0, range = 0.0
**And** the scale-normalized contribution for A→B is 1.0 (degenerate case)
**And** raw weight of A→B = 1.0

### Scenario: Single adapter, multiple edges — min maps to 0.0, max maps to 1.0
**Given** adapter "code-coverage" has emitted contributions: A→B = 2.0, A→C = 10.0, A→D = 18.0
**When** the engine computes scale-normalized contributions
**Then** code-coverage min = 2.0, max = 18.0, range = 16.0
**And** scale-normalized A→B = (2 - 2) / 16 = 0.0
**And** scale-normalized A→C = (10 - 2) / 16 = 0.5
**And** scale-normalized A→D = (18 - 2) / 16 = 1.0

### Scenario: Two adapters on different scales — normalization prevents scale dominance
**Given** adapter "code-coverage" has contributions: A→B = 2.0, A→C = 18.0, A→D = 14.0
**And** adapter "movement" has contributions: A→B = 400.0, A→C = 100.0, A→D = 350.0
**When** the engine applies divide-by-range scale normalization per adapter
**Then** code-coverage (min=2, max=18, range=16): A→B = 0.0, A→C = 1.0, A→D = 0.75
**And** movement (min=100, max=400, range=300): A→B = 1.0, A→C = 0.0, A→D = 0.833
**And** raw weight A→D = 0.75 + 0.833 = 1.583 (highest — strong in both domains)
**And** raw weight A→B = 0.0 + 1.0 = 1.0
**And** raw weight A→C = 1.0 + 0.0 = 1.0
**And** A→D ranks first despite not being the maximum in either adapter's native scale

### Scenario: Signed adapter range normalizes correctly
**Given** adapter "sentiment" has contributions: A→B = -0.8, A→C = 0.5, A→D = 1.0
**When** the engine computes scale-normalized contributions
**Then** sentiment min = -0.8, max = 1.0, range = 1.8
**And** scale-normalized A→B = (-0.8 - (-0.8)) / 1.8 = 0.0
**And** scale-normalized A→C = (0.5 - (-0.8)) / 1.8 = 0.722
**And** scale-normalized A→D = (1.0 - (-0.8)) / 1.8 = 1.0

### Scenario: New emission extending adapter's range shifts all that adapter's scale-normalized values
**Given** adapter "code-coverage" has contributions: A→B = 5.0, A→C = 15.0
**And** scale-normalized values are A→B = 0.0, A→C = 1.0 (min=5, max=15)
**When** adapter "code-coverage" emits edge A→D with contribution 25.0
**Then** code-coverage min = 5.0, max = 25.0, range = 20.0 (range extended)
**And** scale-normalized A→B = (5 - 5) / 20 = 0.0
**And** scale-normalized A→C = (15 - 5) / 20 = 0.5 (was 1.0 — shifted)
**And** scale-normalized A→D = (25 - 5) / 20 = 1.0
**And** raw weights for A→B and A→C change even though their contributions did not

---

## Not Covered (Open Questions)

The following scenarios cannot be written until their design questions are resolved:

- **Property merge on multi-source upsert:** When two adapters emit the same node with different properties, what merge semantics apply? See domain model open question (node property merge).
- **Evidence diversity bonus:** Should edges confirmed by more adapters rank higher than edges with equal total weight from fewer adapters? See ADR-003 Open Question 1.
- **Contribution removal:** ADR-003 mentions removing an adapter's contribution, but the mechanism (explicit API? emit with value 0? separate operation?) is not defined.
