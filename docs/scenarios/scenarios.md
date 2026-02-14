# Behavior Scenarios

Refutable behavior scenarios for ADRs 013–015. Each scenario can be verified against the running software. Domain vocabulary from [domain-model.md](../domain-model.md).

---

## Feature: StepQuery typed multi-hop traversal (ADR-013)

### Scenario: Single-step traversal follows relationship and direction
**Given** a context with concept node `concept:travel` and two marks with outgoing `references` edges to `concept:travel`
**When** `StepQuery::from("concept:travel").step(Incoming, "references")` is executed
**Then** the result contains exactly those two mark nodes at step 0, and the two `references` edges traversed

### Scenario: Multi-step traversal chains through frontiers
**Given** a context with concept `concept:travel`, mark `mark:1` referencing it, and chain `chain:provenance:research` containing `mark:1` via a `contains` edge
**When** `StepQuery::from("concept:travel").step(Incoming, "references").step(Incoming, "contains")` is executed
**Then** step 0 contains `mark:1`, step 1 contains `chain:provenance:research`, and all traversed edges are included

### Scenario: Step with no matching edges produces empty frontier
**Given** a context with concept `concept:travel` and no `tagged_with` edges pointing to it
**When** `StepQuery::from("concept:travel").step(Incoming, "tagged_with")` is executed
**Then** step 0 contains zero nodes and zero edges

### Scenario: Step filters by relationship type
**Given** a context with concept `concept:travel` that has both incoming `references` edges (from marks) and incoming `tagged_with` edges (from fragments)
**When** `StepQuery::from("concept:travel").step(Incoming, "references")` is executed
**Then** the result contains only the mark nodes, not the fragment nodes

### Scenario: StepQuery preserves per-step structure in results
**Given** a two-step query that discovers marks at step 0 and chains at step 1
**When** the query executes
**Then** the result distinguishes step 0 nodes from step 1 nodes — they are not flattened into a single list

### Scenario: StepQuery supports Outgoing direction
**Given** a context with chain `chain:provenance:research` that has outgoing `contains` edges to marks `mark:1` and `mark:2`
**When** `StepQuery::from("chain:provenance:research").step(Outgoing, "contains")` is executed
**Then** the result contains `mark:1` and `mark:2` at step 0

---

## Feature: Evidence trail query (ADR-013)

### Scenario: Evidence trail returns marks, fragments, and chains for a concept
**Given** a context where concept `concept:travel` has:
  - mark `mark:1` with a `references` edge to it (mark → concept)
  - fragment `fragment:abc` with a `tagged_with` edge to it (fragment → concept)
  - chain `chain:provenance:research` with a `contains` edge to `mark:1` (chain → mark)
**When** `evidence_trail("concept:travel")` is called
**Then** the result contains `mark:1` in marks, `fragment:abc` in fragments, `chain:provenance:research` in chains, and all traversed edges

### Scenario: Evidence trail with no evidence returns empty result
**Given** a context with concept `concept:obscure` that has no incoming `references`, `tagged_with`, or `contains` edges
**When** `evidence_trail("concept:obscure")` is called
**Then** the result contains zero marks, zero fragments, zero chains, and zero edges

### Scenario: Evidence trail composes two independent StepQuery branches
**Given** the evidence trail query
**When** it executes internally
**Then** it runs two separate StepQuery executions (branch 1: references → contains; branch 2: tagged_with) and merges their results

---

## Feature: PlexusApi as single entry point (ADR-014)

### Scenario: Transport calls PlexusApi for ingest
**Given** an MCP transport receiving an `ingest_fragment` tool call
**When** the transport processes the call
**Then** it calls `PlexusApi.ingest(context_id, "fragment", data)` — it does not call `IngestPipeline` or `PlexusEngine` directly

### Scenario: PlexusApi delegates provenance reads to ProvenanceApi
**Given** a `list_chains` request through any transport
**When** `PlexusApi.list_chains(context_id, status)` is called
**Then** it delegates to `ProvenanceApi` and returns `Vec<ChainView>`

### Scenario: PlexusApi delegates graph queries to the query system
**Given** a `find_nodes` request through any transport
**When** `PlexusApi.find_nodes(context_id, query)` is called
**Then** it delegates to the query system and returns `QueryResult`

### Scenario: list_tags is context-scoped
**Given** context "alpha" with marks tagged `#travel` and context "beta" with marks tagged `#cooking`
**When** `PlexusApi.list_tags("alpha")` is called
**Then** the result contains `travel` but not `cooking`

### Scenario: Non-ingest mutations route through ProvenanceApi
**Given** a mark `mark:1` in context "research"
**When** `PlexusApi.update_mark("research", "mark:1", changes)` is called
**Then** it routes directly to `ProvenanceApi`, not through the ingest pipeline

---

## Feature: Annotate workflow with auto-chain creation (ADR-015)

### Scenario: Annotate creates fragment, chain, and mark in one call
**Given** context "research" with no existing chain named "field notes"
**When** `annotate(context_id: "research", chain_name: "field notes", file: "src/main.rs", line: 42, annotation: "interesting pattern", tags: ["#refactor"])` is called
**Then** a fragment node is created with text "interesting pattern" and tags ["refactor"] (semantic content)
**And** concept `concept:refactor` is created (from tags)
**And** a chain node with ID `chain:provenance:field-notes` is created in the context
**And** a mark node is created in that chain at src/main.rs:42 (provenance)
**And** the annotation enters the graph as both semantic content and provenance

### Scenario: Annotate reuses existing chain
**Given** context "research" with an existing chain named "field notes" (ID `chain:provenance:field-notes`)
**When** `annotate(context_id: "research", chain_name: "field notes", file: "src/lib.rs", line: 10, annotation: "another note", tags: [])` is called
**Then** no new chain is created, and a mark is added to the existing chain

### Scenario: Chain name normalization produces deterministic IDs
**Given** two annotate calls with chain names "Field Notes" and "field notes"
**When** both are resolved to chain IDs
**Then** both produce the same ID: `chain:provenance:field-notes`

### Scenario: Chain name normalization handles special characters
**Given** an annotate call with chain name "research: phase 1/2"
**When** the chain name is resolved to a chain ID
**Then** the ID is `chain:provenance:research--phase-1-2` (colons and slashes replaced by hyphens, whitespace replaced by hyphens)

### Scenario: Annotate triggers enrichment loop
**Given** context "research" with existing concept `concept:refactor`
**When** `annotate(context_id: "research", chain_name: "notes", file: "src/main.rs", line: 1, annotation: "cleanup", tags: ["#refactor"])` is called
**Then** a fragment is created with the annotation text and tags
**And** the enrichment loop runs after the fragment and mark are created
**And** TagConceptBridger creates a `references` edge from the new mark to `concept:refactor`

### Scenario: create_chain is not exposed as a consumer-facing operation
**Given** the PlexusApi public surface
**When** a consumer inspects available operations
**Then** there is no standalone `create_chain` operation — chains are created via `annotate` or via adapter-produced provenance

### Scenario: Annotate returns merged outbound events
**Given** a successful annotate call that creates a chain and a mark (two ingest calls)
**When** the operation completes
**Then** the consumer receives a single merged list of outbound events from both ingest calls — chain creation events followed by mark creation events — not two separate response batches

### Scenario: Annotate rejects empty chain name
**Given** an annotate call with an empty string as chain_name
**When** the operation is invoked
**Then** it returns an error without creating any chain or mark
