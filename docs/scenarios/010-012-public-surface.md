# Behavior Scenarios: Public Surface (ADR-010 through ADR-012)

---

## Feature: Enrichment Trait (ADR-010)

### Scenario: Enrichment receives events and context snapshot after primary emission
**Given** a PlexusEngine with an existing context "provence-research"
**And** a registered enrichment "test-enrichment" that records its inputs and returns None
**And** an adapter emits a node into context "provence-research"
**When** the emission commits and fires graph events
**Then** the enrichment's `enrich()` is called with the NodesAdded event
**And** the enrichment receives a context snapshot containing the newly added node

### Scenario: Enrichment returning Some(Emission) causes mutations to be committed
**Given** a PlexusEngine with an existing context "provence-research"
**And** two concept nodes `concept:travel` and `concept:avignon` exist in the context
**And** a registered enrichment that emits a `may_be_related` edge between them
**When** the enrichment loop runs
**Then** the `may_be_related` edge exists in the context
**And** the edge has a contribution from the enrichment's `id()`

### Scenario: Enrichment's id() is used for contribution tracking
**Given** a registered enrichment with id "co-occurrence"
**When** the enrichment emits an edge with contribution value 0.8
**Then** the committed edge has `contributions["co-occurrence"] == 0.8`

### Scenario: Enrichment returning None means quiescent
**Given** a registered enrichment that always returns None
**When** the enrichment loop runs after a primary emission
**Then** the loop completes in one round
**And** no additional mutations are committed

---

## Feature: Enrichment Loop (ADR-010)

### Scenario: Loop runs multiple rounds until quiescence
**Given** a PlexusEngine with an existing context "provence-research"
**And** enrichment A that emits a node on round 0, then returns None on subsequent rounds
**And** enrichment B that emits an edge (referencing A's node) on round 1, then returns None
**When** the enrichment loop runs after a primary emission
**Then** enrichment A fires in round 0 (sees primary events)
**And** enrichment B fires in round 1 (sees A's NodesAdded event)
**And** the loop completes in round 2 (both return None)

### Scenario: Per-round events — enrichment sees only previous round's events
**Given** a registered enrichment that records the events it receives each round
**When** the primary emission produces events E0
**And** enrichment round 0 produces events E1
**And** enrichment round 1 produces events E2
**Then** round 0 received only E0
**And** round 1 received only E1
**And** round 2 received only E2

### Scenario: Multiple enrichments in the same round see the same snapshot
**Given** two registered enrichments A and B
**When** the enrichment loop runs round 0
**Then** both A and B receive the same context snapshot
**And** mutations from A in round 0 are not visible to B in round 0

### Scenario: Safety valve aborts after maximum rounds
**Given** a registered enrichment that always returns Some(Emission) (never quiesces)
**And** the enrichment loop maximum round count is set to 10
**When** the enrichment loop runs
**Then** the loop aborts after 10 rounds
**And** a warning is logged

### Scenario: Enrichment commits go through the same path as adapter emissions
**Given** a registered enrichment with id "tag-bridger"
**When** the enrichment emits an edge with contribution value 1.0
**Then** the edge receives scale normalization like any adapter emission
**And** graph events (EdgesAdded) fire for the committed edge
**And** the edge is persisted via the engine's GraphStore

### Scenario: Idempotent enrichment does not loop indefinitely
**Given** a CoOccurrenceEnrichment registered on the engine
**And** two fragments sharing a tag already have `may_be_related` edges between their co-occurring concepts
**When** a new fragment is processed that adds no new concept co-occurrences
**Then** the CoOccurrenceEnrichment checks context state, finds existing edges, and returns None
**And** the enrichment loop completes in one round

### Scenario: Enrichments shared across integrations are deduplicated
**Given** integration "trellis" registers enrichment with id "tag-bridger"
**And** integration "carrel" registers enrichment with id "tag-bridger"
**When** the enrichment loop runs
**Then** the enrichment with id "tag-bridger" runs exactly once per round, not twice

---

## Feature: TagConceptBridger Enrichment (ADR-009 + ADR-010)

### Scenario: New mark bridges to existing concept via enrichment
**Given** a context "provence-research" containing a concept node `concept:travel`
**And** a chain "reading-notes" in context "provence-research"
**And** a TagConceptBridger enrichment registered on the engine
**When** `add_mark` is called with tags `["#travel"]` in context "provence-research"
**And** the enrichment loop runs
**Then** a cross-dimensional `references` edge exists from the mark to `concept:travel`
**And** the edge has a contribution from "tag-bridger" with value 1.0

### Scenario: New concept retroactively bridges to existing mark
**Given** a context "provence-research" with no concept nodes
**And** a mark tagged `["#travel"]` already exists in context "provence-research"
**And** a TagConceptBridger enrichment registered on the engine
**When** a FragmentAdapter processes a fragment with tags `["travel"]`, creating `concept:travel`
**And** the enrichment loop runs
**Then** a cross-dimensional `references` edge exists from the existing mark to `concept:travel`

### Scenario: TagConceptBridger is idempotent
**Given** a mark with tag `["#travel"]` already has a `references` edge to `concept:travel`
**And** a TagConceptBridger enrichment registered on the engine
**When** the enrichment loop runs (triggered by an unrelated emission)
**Then** the TagConceptBridger checks context state, finds the existing edge, and returns None
**And** no duplicate `references` edge is created

---

## Feature: Bidirectional Adapter — Outbound Events (ADR-011)

### Scenario: Default transform_events returns empty vec
**Given** an adapter that does not override `transform_events()`
**When** `transform_events()` is called with graph events and a context snapshot
**Then** the result is an empty `Vec<OutboundEvent>`

### Scenario: Adapter translates graph events to domain-meaningful outbound events
**Given** a FragmentAdapter that overrides `transform_events()` to detect new concept nodes
**When** a fragment with tags `["travel", "avignon"]` is processed, creating two concept nodes
**And** `transform_events()` is called with the NodesAdded events
**Then** the result contains an outbound event with kind "concepts_detected" and detail listing "travel, avignon"

### Scenario: Outbound events include events from enrichment rounds
**Given** a FragmentAdapter that overrides `transform_events()` to detect `may_be_related` edges
**And** a CoOccurrenceEnrichment registered on the engine
**When** a fragment is processed, and the enrichment loop produces `may_be_related` edges
**And** `transform_events()` is called with all accumulated events (primary + enrichment rounds)
**Then** the result includes outbound events reflecting the co-occurrence relationships

### Scenario: Consumer receives outbound events, never raw graph events
**Given** a consumer calls `ingest()` with fragment data
**When** the full pipeline completes (process → enrichment loop → transform_events)
**Then** the return value is `Vec<OutboundEvent>`
**And** no `GraphEvent` values appear in the return type

---

## Feature: Unified Ingest Pipeline (ADR-012)

### Scenario: ingest routes to adapter by input_kind
**Given** a FragmentAdapter registered with input_kind "fragment"
**And** a PlexusEngine with an existing context "provence-research"
**When** `ingest("provence-research", "fragment", data)` is called
**Then** the FragmentAdapter's `process()` receives the data

### Scenario: ingest with unknown input_kind returns error
**Given** no adapter registered with input_kind "unknown"
**When** `ingest("provence-research", "unknown", data)` is called
**Then** the call returns an error indicating no matching adapter

### Scenario: Full ingest pipeline end-to-end
**Given** a FragmentAdapter registered with input_kind "fragment"
**And** a TagConceptBridger and CoOccurrenceEnrichment registered on the engine
**And** an existing context "provence-research" with prior fragments tagged "travel"
**When** `ingest("provence-research", "fragment", {text: "Walk in Avignon", tags: ["travel", "avignon"]})` is called
**Then** the pipeline executes in order:
  1. FragmentAdapter.process() creates fragment node, concept nodes, tagged_with edges
  2. Enrichment loop runs: TagConceptBridger and CoOccurrenceEnrichment fire until quiescence
  3. FragmentAdapter.transform_events() translates all accumulated events
**And** the return value is a `Vec<OutboundEvent>` containing domain-meaningful events

### Scenario: Fan-out — multiple adapters matching same input_kind
**Given** adapter A and adapter B both registered with input_kind "fragment"
**And** an existing context "provence-research"
**When** `ingest("provence-research", "fragment", data)` is called
**Then** both adapter A and adapter B receive the input and process independently
**And** the enrichment loop runs once after all primary emissions (not once per adapter)
**And** both adapters' `transform_events()` are called with the full accumulated event set
**And** the return value merges outbound events from both adapters

### Scenario: All writes go through ingest — no raw graph primitive API
**Given** the public API surface of Plexus
**Then** there is no public method to create individual nodes or edges directly
**And** all graph mutations are performed via `ingest(context_id, input_kind, data)`

### Scenario: Read queries bypass the adapter pipeline
**Given** a PlexusEngine with context "provence-research" containing nodes and edges
**When** `list_chains("provence-research")` is called
**Then** the result comes directly from the engine, not through any adapter
**And** no enrichment loop runs
**And** no outbound event transformation occurs

---

## Feature: Integration Registration (ADR-012)

### Scenario: Integration bundles adapter and enrichments
**Given** an integration registered as:
  ```
  register_integration("trellis",
      adapter: FragmentAdapter,
      enrichments: [TagConceptBridger, CoOccurrenceEnrichment],
  )
  ```
**When** `ingest()` is called with input_kind matching the FragmentAdapter
**Then** the FragmentAdapter processes the input
**And** both TagConceptBridger and CoOccurrenceEnrichment are available in the enrichment loop

### Scenario: Enrichments from multiple integrations are deduplicated
**Given** integration "trellis" registers enrichment with id "tag-bridger"
**And** integration "carrel" registers enrichment with id "tag-bridger"
**When** the enrichment loop runs
**Then** the "tag-bridger" enrichment executes exactly once per round

---

## Feature: Transport Independence (ADR-012)

### Scenario: Different transports produce identical results
**Given** the same adapter, enrichments, and context
**When** an MCP transport calls `ingest("provence-research", "fragment", data)`
**And** a gRPC transport calls `ingest("provence-research", "fragment", data)` with identical data
**Then** both calls produce identical graph mutations
**And** both calls return identical outbound events

### Scenario: Adding a transport requires no changes to adapters or enrichments
**Given** a working system with an MCP transport, FragmentAdapter, and CoOccurrenceEnrichment
**When** a new gRPC transport is added
**Then** the FragmentAdapter code is unchanged
**And** the CoOccurrenceEnrichment code is unchanged
**And** the engine code is unchanged
**And** the new transport calls the same `ingest()` and query endpoints

---

## Not Covered by These Scenarios

Behaviors deferred or out of scope for this decision cycle:

- **Event persistence and cursor-based delivery** (OQ8) — deferred. Outbound events are synchronous with ingest; async delivery requires event persistence.
- **Wire protocol schema** (OQ9) — deferred. The protobuf schema for gRPC ingest/query endpoints hasn't been designed.
- ~~**Emission removal variant** (OQ10)~~ — resolved. `Emission` now supports `edge_removals` and `removals`. All provenance operations route through the adapter pipeline.
- **Cross-pollination visibility** — an adapter surfacing events from another adapter's mutations requires event cursors (OQ8). Under the current synchronous model, a consumer only receives outbound events from its own ingestion.
- **Async enrichment / batching** — enrichments run synchronously after every emission. If enrichments become expensive during burst ingestion, pipelining is a future optimization.
- **Provenance operations through ingest** — provenance operations (create chain, add mark, link marks) routing through the adapter pipeline via a provenance input kind. This is described in ADR-012 but the provenance adapter design is not specified here.
