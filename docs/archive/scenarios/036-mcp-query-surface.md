# Behavior Scenarios: MCP Query Surface

**ADR:** 036 (MCP query surface)

**Domain vocabulary:** query filter, composable filter, corroboration, event cursor, changes since, traverse, find path, lens, provenance-scoped filtering, thin shell, transport

**Note:** Query logic (filter semantics, ranking, cursor persistence) is tested at the API level by scenarios 033–035. Spec loading lifecycle (validation, wiring, persistence, lens execution) is tested at the API level by scenarios 037. These scenarios verify the MCP transport layer: parameter serialization, result marshalling, and end-to-end flow through MCP tools. Given clauses describe graph state abstractly — how nodes arrived in the graph is not the concern of transport-level scenarios.

---

## Feature: Spec Loading Tool (ADR-036 §1, ADR-037)

### Scenario: load_spec via MCP wires spec onto active context
**Given** a context "test" set as active via `set_context`
**When** `load_spec` is called with `spec_yaml` containing a valid adapter spec with lens
**Then** the result contains the adapter ID and lens namespace
**And** subsequent `ingest` calls can route through the loaded adapter

### Scenario: load_spec via MCP fails on invalid spec
**Given** a context "test" set as active via `set_context`
**When** `load_spec` is called with `spec_yaml` containing malformed YAML
**Then** the result is an error indicating validation failure

---

## Feature: Graph Query Tools (ADR-036 §1)

### Scenario: find_nodes returns matching nodes from active context
**Given** a context "test" set as active via `set_context`
**And** the context contains nodes A, B, and C
**When** `find_nodes` is called with no filter parameters
**Then** the result contains all three nodes
**And** the result includes a `total_count`

### Scenario: find_nodes passes filter parameters to API
**Given** a context "test" with nodes connected by edges from two adapters: "adapter-a" and "adapter-b"
**When** `find_nodes` is called with `contributor_ids: ["adapter-a"]`
**Then** only nodes with at least one incident edge contributed by "adapter-a" appear in results

### Scenario: traverse explores graph from origin with depth and direction
**Given** a context "test" with nodes A → B → C (outgoing edges)
**When** `traverse` is called with `origin: "A"`, `max_depth: 2`, `direction: "outgoing"`
**Then** the result contains levels: [A], [B], [C]
**And** the result contains the two traversed edges

### Scenario: traverse accepts rank_by parameter
**Given** a context "test" with node A connected to B (2 contributors) and C (1 contributor)
**When** `traverse` is called with `origin: "A"`, `max_depth: 1`, `rank_by: "corroboration"`
**Then** edges in the result are ordered by corroboration count descending

### Scenario: find_path discovers connection between two nodes
**Given** a context "test" with path A → B → C → D
**When** `find_path` is called with `source: "A"`, `target: "D"`, `max_length: 5`
**Then** the result contains `found: true`
**And** the path includes nodes [A, B, C, D] and three edges
**And** `length` equals 3

### Scenario: find_path with filter restricts traversable edges
**Given** a context "test" with path A → B → C where A→B has relationship "tagged_with" and B→C has relationship "lens:trellis:thematic_connection"
**When** `find_path` is called with `source: "A"`, `target: "C"`, `relationship_prefix: "lens:trellis"`
**Then** the result contains `found: false` (the A→B edge is filtered out, breaking the path)

---

## Feature: Event Cursor Tool (ADR-036 §1, §4)

### Scenario: changes_since returns events after cursor with latest_sequence
**Given** a context "test" with 5 persisted graph events (sequences 1–5)
**When** `changes_since` is called with `cursor: 3`
**Then** the result contains events with sequences 4 and 5
**And** the result contains `latest_sequence: 5`

### Scenario: changes_since accepts cursor filter parameters
**Given** a context "test" with events from two adapters: "content" and "enrichment"
**When** `changes_since` is called with `cursor: 0`, `adapter_id: "content"`
**Then** only events produced by adapter "content" appear in the result

---

## Feature: Discovery Tools (ADR-036 §1)

### Scenario: list_tags returns all tags in active context
**Given** a context "test" containing nodes with associated tags
**When** `list_tags` is called
**Then** the result contains the tag strings present in the context

### Scenario: shared_concepts returns concept overlap between two contexts
**Given** context "project-a" and context "project-b" that share some concept nodes
**When** `shared_concepts` is called with `context_a: "project-a"`, `context_b: "project-b"`
**Then** the result contains the node IDs present in both contexts

---

## Feature: Evidence Trail Filter Conformance (ADR-036 §5, Invariant 59)

### Scenario: evidence_trail accepts optional filter parameter
**Given** a context "test" with a concept node referenced by marks from adapters "adapter-a" and "adapter-b"
**When** `evidence_trail` is called with `node_id` set to that concept, `contributor_ids: ["adapter-a"]`
**Then** only marks and chains from "adapter-a" appear in the trail

---

## Feature: Error Handling

### Scenario: query tool called without active context returns error
**Given** no context has been set via `set_context`
**When** `find_nodes` is called
**Then** the result is an error indicating no active context

---

## Feature: End-to-End Integration

### Scenario: ingest then query flow through MCP
**Given** a fresh context "integration-test" set via `set_context`
**When** a file is ingested via `ingest` with `{"file_path": "/path/to/test-document.md"}`
**And** then `find_nodes` is called (no filter)
**Then** the result contains nodes created by the extraction pipeline
**When** `traverse` is called with `origin` set to one of the returned node IDs, `max_depth: 1`
**Then** the result contains connected nodes and edges
**When** `changes_since` is called with `cursor: 0`
**Then** events include NodesAdded and EdgesAdded from the ingest
**And** `latest_sequence` is greater than 0
