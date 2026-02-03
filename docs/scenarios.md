# Behavior Scenarios: Semantic Adapter Layer

Derived from ADR-001 and the domain model. Each scenario is refutable — it can be clearly true or false against running software. All terms follow the domain model vocabulary.

**Scope:** Everything except reinforcement mechanics (blocked — needs spike).

---

## Feature: Emission Validation

The engine validates each item in an emission independently. Valid items commit; invalid items are rejected individually. `emit()` returns a result describing what was rejected.

### Scenario: Valid emission with nodes and edges commits successfully
**Given** an empty graph
**When** an adapter emits an emission containing node A, node B, and an edge A→B
**Then** the engine commits all three mutations
**And** node A and node B exist in the graph
**And** edge A→B exists in the graph with the emitted raw weight
**And** `emit()` returns a result with no rejections

### Scenario: Edge referencing a missing endpoint is rejected; valid items still commit
**Given** a graph containing node A (but not node C)
**When** an adapter emits an emission containing node B, edge A→B, and edge B→C
**Then** node B is committed (valid)
**And** edge A→B is committed (both endpoints exist)
**And** edge B→C is rejected (node C does not exist)
**And** `emit()` returns a result listing edge B→C as rejected with reason "missing endpoint C"

### Scenario: Edge endpoints satisfied within the same emission
**Given** an empty graph
**When** an adapter emits an emission containing node X, node Y, and edge X→Y
**Then** the engine commits all three
**And** edge X→Y exists in the graph

### Scenario: Edge endpoint exists in the graph from a prior emission
**Given** a graph containing node A (from a previous emission)
**When** an adapter emits an emission containing node B and edge A→B
**Then** the engine commits successfully
**And** edge A→B exists in the graph

### Scenario: Duplicate node ID causes upsert
**Given** a graph containing node A with property `name = "alpha"`
**When** an adapter emits an emission containing node A with property `name = "alpha-updated"`
**Then** the engine commits successfully
**And** node A exists in the graph with property `name = "alpha-updated"`
**And** no second node with ID A exists

### Scenario: Removal of a non-existent node is a no-op
**Given** an empty graph
**When** an adapter emits an emission containing a removal for node Z
**Then** the engine commits successfully (no error)
**And** the graph remains empty

### Scenario: Empty emission is a no-op
**Given** a graph in any state
**When** an adapter emits an emission with no nodes, no edges, and no removals
**Then** the engine commits successfully (no error)
**And** the graph state is unchanged
**And** no graph events fire

### Scenario: Self-referencing edge is allowed
**Given** a graph containing node A
**When** an adapter emits an emission containing edge A→A
**Then** the engine commits successfully
**And** edge A→A exists in the graph

### Scenario: Bad edge rejected individually; valid items in same emission commit
**Given** an empty graph
**When** an adapter emits an emission containing node A, node B, edge A→B, and edge A→Z (where Z does not exist in the graph or emission)
**Then** node A, node B, and edge A→B are committed
**And** edge A→Z is rejected (missing endpoint Z)
**And** `emit()` returns a result listing edge A→Z as rejected

### Scenario: Node removal cascades to connected edges
**Given** a graph containing node A, node B, and edge A→B
**When** an adapter emits an emission containing a removal for node A
**Then** the engine commits the removal
**And** node A does not exist in the graph
**And** edge A→B does not exist in the graph (cascade)

### Scenario: All edges in emission have missing endpoints; nodes still commit
**Given** an empty graph
**When** an adapter emits an emission containing node A and edge A→Z (Z not in graph or emission)
**Then** node A is committed
**And** edge A→Z is rejected
**And** `emit()` returns a result listing edge A→Z as rejected

---

## Feature: ProposalSink Constraints

The ProposalSink intercepts emissions from reflexive adapters before the engine sees them. It enforces the propose-don't-merge invariant structurally.

### Scenario: may_be_related edge passes through ProposalSink
**Given** a reflexive adapter with a ProposalSink (weight cap = 0.3)
**When** the adapter emits an emission containing edge A→B with relationship `may_be_related` and raw weight 0.2
**Then** the ProposalSink forwards the emission to the engine
**And** the engine commits edge A→B with raw weight 0.2

### Scenario: Non-may_be_related edge is rejected by ProposalSink
**Given** a reflexive adapter with a ProposalSink
**When** the adapter emits an emission containing edge A→B with relationship `related_to`
**Then** the ProposalSink rejects edge A→B
**And** the edge does not reach the engine
**And** `emit()` returns a result listing the edge as rejected with reason "invalid relationship type"

### Scenario: Edge raw weight exceeding cap is clamped
**Given** a reflexive adapter with a ProposalSink (weight cap = 0.3)
**When** the adapter emits an emission containing edge A→B with relationship `may_be_related` and raw weight 0.8
**Then** the ProposalSink clamps the raw weight to 0.3
**And** the engine commits edge A→B with raw weight 0.3

### Scenario: Node removal is rejected by ProposalSink
**Given** a reflexive adapter with a ProposalSink
**When** the adapter emits an emission containing a removal for node A
**Then** the ProposalSink rejects the removal
**And** node A remains in the graph
**And** `emit()` returns a result listing the removal as rejected

### Scenario: Node emission is allowed through ProposalSink
**Given** a reflexive adapter with a ProposalSink
**When** the adapter emits an emission containing a new node M (topology metadata)
**Then** the ProposalSink forwards the emission to the engine
**And** the engine commits node M

### Scenario: Annotation on node passes through ProposalSink
**Given** a reflexive adapter with a ProposalSink
**When** the adapter emits an emission containing node M with an annotation (confidence = 0.7, method = "near-miss-detection")
**Then** the ProposalSink forwards the emission to the engine
**And** the engine constructs a provenance entry with the annotation

### Scenario: Mixed emission with valid nodes and invalid edge type
**Given** a reflexive adapter with a ProposalSink
**And** node A exists in the graph
**When** the adapter emits an emission containing node M and edge M→A with relationship `contains`
**Then** the ProposalSink rejects edge M→A (invalid relationship type)
**And** node M is forwarded to the engine and committed
**And** `emit()` returns a result listing edge M→A as rejected

---

## Feature: Provenance Construction

The engine constructs provenance entries by combining adapter-provided annotations with framework context. Adapters never build provenance entries directly.

### Scenario: Annotated node receives full provenance entry
**Given** adapter "document-adapter" processing input with context ID "manza-session-1"
**When** the adapter emits an emission containing node A with annotation (confidence = 0.85, method = "llm-extraction", source_location = "file.md:87")
**Then** the engine constructs a provenance entry for node A containing:
  - adapter_id = "document-adapter"
  - timestamp = (current time)
  - context_id = "manza-session-1"
  - input_summary = (summary of the input that triggered this adapter)
  - annotation confidence = 0.85
  - annotation method = "llm-extraction"
  - annotation source_location = "file.md:87"

### Scenario: Node without annotation receives structural provenance only
**Given** adapter "document-adapter" processing input with context ID "manza-session-1"
**When** the adapter emits an emission containing node B with no annotation
**Then** the engine constructs a provenance entry for node B containing:
  - adapter_id = "document-adapter"
  - timestamp = (current time)
  - context_id = "manza-session-1"
  - input_summary = (summary of the input)
  - annotation = none

### Scenario: Each emission gets its own timestamp
**Given** adapter "document-adapter" emits emission E1 at time T1
**And** the same adapter emits emission E2 at time T2 (T2 > T1)
**When** the engine constructs provenance entries
**Then** provenance entries from E1 have timestamp T1
**And** provenance entries from E2 have timestamp T2

### Scenario: Multiple nodes in one emission share framework context
**Given** adapter "document-adapter" processing input with context ID "manza-session-1"
**When** the adapter emits an emission containing node A (annotation confidence = 0.9) and node B (annotation confidence = 0.6)
**Then** both provenance entries share adapter_id, timestamp, context_id, and input_summary
**And** the annotation confidence differs: 0.9 for node A, 0.6 for node B

---

## Feature: Query-Time Normalization

Raw weights are stored. Normalized weights are computed at query time via a pluggable normalization strategy. The graph stores ground truth; normalization is an interpretive lens.

### Scenario: Default per-node outgoing divisive normalization
**Given** node A with outgoing edges: A→B (raw weight 3.0), A→C (raw weight 1.0), A→D (raw weight 1.0)
**When** a consumer queries normalized weights using the default strategy
**Then** normalized weight A→B = 3.0 / 5.0 = 0.6
**And** normalized weight A→C = 1.0 / 5.0 = 0.2
**And** normalized weight A→D = 1.0 / 5.0 = 0.2

### Scenario: Adding an edge weakens existing edges in normalized view without mutation
**Given** node A with outgoing edges: A→B (raw weight 3.0), A→C (raw weight 2.0)
**And** normalized weights are A→B = 0.6, A→C = 0.4
**When** a new edge A→D is committed with raw weight 5.0
**Then** raw weights are unchanged: A→B = 3.0, A→C = 2.0
**And** normalized weight A→B = 3.0 / 10.0 = 0.3 (was 0.6)
**And** normalized weight A→C = 2.0 / 10.0 = 0.2 (was 0.4)
**And** normalized weight A→D = 5.0 / 10.0 = 0.5

### Scenario: Quiet graph stays stable
**Given** node A with outgoing edges: A→B (raw weight 3.0), A→C (raw weight 2.0)
**When** no new emissions are committed (time passes, nothing happens)
**Then** raw weights remain: A→B = 3.0, A→C = 2.0
**And** normalized weights remain: A→B = 0.6, A→C = 0.4

### Scenario: Different normalization strategies produce different results
**Given** node A with outgoing edges: A→B (raw weight 3.0), A→C (raw weight 1.0)
**When** consumer 1 queries with outgoing divisive normalization
**And** consumer 2 queries with softmax normalization
**Then** consumer 1 sees A→B = 0.75, A→C = 0.25
**And** consumer 2 sees different values (exp(3)/[exp(3)+exp(1)], exp(1)/[exp(3)+exp(1)])
**And** raw weights are the same for both: A→B = 3.0, A→C = 1.0

### Scenario: Node with single outgoing edge normalizes to 1.0
**Given** node A with one outgoing edge: A→B (raw weight 7.0)
**When** a consumer queries normalized weights using the default strategy
**Then** normalized weight A→B = 1.0

### Scenario: Normalization is per-node, not global
**Given** node A with outgoing edges: A→B (raw weight 100.0)
**And** node C with outgoing edges: C→D (raw weight 1.0)
**When** a consumer queries normalized weights using the default strategy
**Then** normalized weight A→B = 1.0
**And** normalized weight C→D = 1.0
**And** the high raw weight on A→B does not suppress C→D

---

## Feature: Input Routing

The input router directs incoming input to all adapters whose input kind matches. Routing is fan-out. The framework never inspects the opaque data payload.

### Scenario: Input routed to matching adapter
**Given** adapter "document-adapter" declares input kind "file_content"
**And** adapter "movement-adapter" declares input kind "gesture_encoding"
**When** the input router receives input with kind "file_content"
**Then** "document-adapter" receives the input
**And** "movement-adapter" does not receive the input

### Scenario: Fan-out to multiple adapters with same input kind
**Given** adapter "normalization-adapter" declares input kind "graph_state"
**And** adapter "topology-adapter" declares input kind "graph_state"
**And** adapter "coherence-adapter" declares input kind "graph_state"
**When** the input router receives input with kind "graph_state"
**Then** all three adapters receive the input
**And** each is spawned independently with its own sink and cancellation token

### Scenario: No matching adapter for input kind
**Given** no adapter declares input kind "unknown_kind"
**When** the input router receives input with kind "unknown_kind"
**Then** no adapter is invoked
**And** the framework logs a warning (or no-ops — not an error)

### Scenario: Opaque data downcast failure
**Given** adapter "document-adapter" declares input kind "file_content" and expects `FileContent` data
**When** the input router delivers input with kind "file_content" but data payload is `GestureEncoding`
**Then** the adapter's `process()` returns `Err(AdapterError::InvalidInput)`
**And** the framework logs the error
**And** other adapters matching "file_content" (if any) are not affected

### Scenario: Independent adapters don't see each other's emissions
**Given** adapter A and adapter B both receive the same input (fan-out)
**When** adapter A emits node X in its first emission
**Then** adapter B's processing is not affected by node X
**And** adapter B does not receive node X as input

---

## Feature: Cancellation

Cancellation is cooperative. The framework signals via a cancellation token. Already-committed emissions remain valid.

### Scenario: Adapter checks cancellation between emissions
**Given** an adapter has emitted emission E1 (committed successfully)
**When** the framework sets the cancellation token
**And** the adapter checks the token before its next emission
**Then** the adapter stops processing (returns Ok or a cancellation-specific result)
**And** emission E1 remains committed in the graph
**And** no further emissions from this adapter are attempted

### Scenario: Committed emissions survive cancellation
**Given** an adapter has emitted emissions E1 and E2 (both committed)
**When** the framework cancels the adapter before emission E3
**Then** nodes and edges from E1 and E2 remain in the graph
**And** E3 is never emitted

### Scenario: Cancellation during emission has no effect until next check
**Given** an adapter is in the middle of building emission E2
**When** the framework sets the cancellation token while E2 is being constructed
**Then** the adapter may still call `sink.emit()` with E2 (cancellation is checked between emissions, not during)
**And** if E2 is emitted and valid, it is committed

---

## Feature: Progressive Emission

Adapters emit progressively — cheap results first, expensive results later. Each emission commits independently. The graph is always partially built.

### Scenario: Multiple emissions from one adapter, each commits independently
**Given** an adapter processing a document
**When** the adapter emits emission E1 (structural nodes: file, sections)
**And** later emits emission E2 (semantic nodes: concepts, edges)
**Then** after E1 commits, structural nodes exist in the graph
**And** after E2 commits, semantic nodes and edges also exist
**And** E1 and E2 are independent — E2 failing does not roll back E1

### Scenario: Graph events fire per emission
**Given** an adapter processing a document
**When** the adapter emits emission E1 containing 3 nodes
**And** later emits emission E2 containing 2 edges
**Then** a `NodesAdded` event fires after E1 commits (containing the 3 node IDs)
**And** an `EdgesAdded` event fires after E2 commits (containing the 2 edge IDs)

### Scenario: Early emissions are visible to graph queries before later emissions
**Given** an adapter processing a document
**When** the adapter emits emission E1 containing node A
**And** E1 is committed
**And** the adapter is still processing (E2 not yet emitted)
**Then** a graph query returns node A
**And** concepts from E2 are not yet visible (they haven't been emitted)

---

## Feature: Cross-Modal Bridging

All domains contribute concepts to a shared semantic namespace. Content type disambiguates origin. Labels are the bridge.

### Scenario: Two adapters emit the same concept label — single node with accumulated provenance
**Given** adapter "document-adapter" emits node `concept:sudden` with annotation (method = "llm-extraction")
**When** adapter "movement-adapter" later emits node `concept:sudden` with annotation (method = "label-mapping")
**Then** the engine upserts — a single node `concept:sudden` exists
**And** the node has provenance entries from both adapters
**And** property merge semantics for the node are TBD (see reinforcement spike — open question 1)

### Scenario: Same concept from different adapters produces multiple provenance entries
**Given** adapter "document-adapter" has emitted concept `concept:sudden` with annotation (method = "llm-extraction", confidence = 0.85)
**When** adapter "movement-adapter" emits concept `concept:sudden` with annotation (method = "label-mapping", confidence = 0.95)
**Then** the node has two provenance entries:
  - One from "document-adapter" with method "llm-extraction"
  - One from "movement-adapter" with method "label-mapping"

### Scenario: Different labels for related concepts remain separate
**Given** adapter "document-adapter" emits concept `concept:sudden`
**And** adapter "movement-adapter" emits concept `concept:abrupt`
**Then** two separate nodes exist: `concept:sudden` and `concept:abrupt`
**And** no automatic bridging occurs (a reflexive adapter may later propose `may_be_related`)

---

## Feature: Graph Events

The engine fires low-level graph events per mutation type when an emission is committed.

### Scenario: NodesAdded event on successful emission
**Given** an empty graph
**When** an adapter emits an emission containing nodes A and B
**Then** the engine fires a `NodesAdded` event
**And** the event payload contains node IDs [A, B], the adapter ID, and the context ID

### Scenario: EdgesAdded event on successful emission
**Given** a graph containing nodes A and B
**When** an adapter emits an emission containing edge A→B
**Then** the engine fires an `EdgesAdded` event
**And** the event payload contains edge ID(s) for A→B, the adapter ID, and the context ID

### Scenario: NodesRemoved event on removal
**Given** a graph containing node A
**When** an adapter emits an emission containing a removal for node A
**Then** the engine fires a `NodesRemoved` event with node ID A

### Scenario: EdgesRemoved event on cascade from node removal
**Given** a graph containing node A, node B, and edge A→B
**When** an adapter emits an emission containing a removal for node A
**Then** the engine fires a `NodesRemoved` event for node A
**And** the engine fires an `EdgesRemoved` event for edge A→B with reason "cascade"

### Scenario: No events fire for rejected items; events fire for committed items
**Given** an empty graph
**When** an adapter emits an emission containing node A, edge A→B (B missing), and edge A→C (C missing)
**Then** node A is committed; edges A→B and A→C are rejected
**And** a `NodesAdded` event fires for node A
**And** no `EdgesAdded` event fires (no edges committed)

### Scenario: Emission with only invalid edges produces no edge events
**Given** an empty graph
**When** an adapter emits an emission containing only edge X→Y (both endpoints missing, no nodes in emission)
**Then** edge X→Y is rejected
**And** no graph events fire

### Scenario: No events fire on empty emission
**Given** a graph in any state
**When** an adapter emits an empty emission
**Then** no graph events fire

### Scenario: Events include both nodes and edges from a mixed emission
**Given** an empty graph
**When** an adapter emits an emission containing node A, node B, and edge A→B
**Then** the engine fires a `NodesAdded` event (node IDs [A, B])
**And** the engine fires an `EdgesAdded` event (edge A→B)
**And** the order of events is: `NodesAdded` before `EdgesAdded`

---

## Feature: Schedule Monitor

The schedule monitor evaluates trigger conditions and fires reflexive adapters.

### Scenario: Mutation threshold triggers reflexive adapter
**Given** a reflexive adapter "normalization-adapter" with schedule `MutationThreshold(count = 10)`
**When** 10 mutations have been committed since the last time "normalization-adapter" ran
**Then** the schedule monitor triggers "normalization-adapter"
**And** the adapter receives a ProposalSink (not a full AdapterSink)

### Scenario: Periodic schedule triggers reflexive adapter
**Given** a reflexive adapter "topology-adapter" with schedule `Periodic(interval_secs = 60)`
**When** 60 seconds have elapsed since the last run
**Then** the schedule monitor triggers "topology-adapter"

### Scenario: Condition schedule evaluated against graph state
**Given** a reflexive adapter "coherence-adapter" with a condition schedule that checks for nodes with conflicting provenance entries
**When** the schedule monitor evaluates the condition and it returns true
**Then** the schedule monitor triggers "coherence-adapter"

---

## Not Covered (Blocked by Reinforcement Spike)

The following scenarios cannot be written until reinforcement and multi-source convergence mechanics are defined:

- **WeightsChanged event:** When does it fire? What triggers raw weight changes?
- **Duplicate edge emission:** When adapter emits an edge that already exists, does the raw weight change? How?
- **Re-processing behavior:** Same adapter re-emits the same edge (e.g., file re-saved). Upsert? Reinforce? No-op?
- **Reflexive confirmation:** A reflexive adapter proposes `may_be_related`. Later, actual co-occurrence "confirms" it. What changes?
- **Evidence diversity computation:** Derived from provenance entries, but the query semantics depend on whether re-emission creates new provenance entries or updates existing ones.
- **Property merge on multi-source upsert:** When two adapters emit the same concept with different properties, what merge semantics apply? Last-writer-wins? Union? Thin nodes with detail in provenance only?

These will be added as a second batch of scenarios after the reinforcement spike.
