# Behavior Scenarios: Public Surface Redesign (ADR-013 through ADR-015, ADR-008)

---

## Feature: Typed Multi-Hop Traversal — StepQuery (ADR-013)

### Scenario: StepQuery follows a single typed step from an origin
**Given** a context containing concept `concept:travel` and mark `mark:1`
**And** a `references` edge from `mark:1` to `concept:travel`
**When** `StepQuery::from("concept:travel").step(Incoming, "references")` executes
**Then** the result contains `mark:1` at step 0
**And** only edges matching the `references` relationship are traversed

### Scenario: StepQuery chains multiple typed steps
**Given** a context with `concept:travel`, `mark:1`, and `chain:provenance:research`
**And** a `references` edge from `mark:1` to `concept:travel`
**And** a `contains` edge from `chain:provenance:research` to `mark:1`
**When** `StepQuery::from("concept:travel").step(Incoming, "references").step(Incoming, "contains")` executes
**Then** step 0 contains `mark:1`
**And** step 1 contains `chain:provenance:research`
**And** the steps are not flattened — per-step structure is preserved

### Scenario: StepQuery with no matching edges returns empty result
**Given** a context containing concept `concept:obscure` with no edges
**When** `StepQuery::from("concept:obscure").step(Incoming, "references")` executes
**Then** the result has zero nodes at step 0
**And** no edges are traversed

### Scenario: StepQuery uses each step's frontier as the next step's origin
**Given** a context with `concept:travel`, two marks, and one chain containing both marks
**When** a two-step query executes (Incoming "references" then Incoming "contains")
**Then** step 1 discovers the chain via the marks found in step 0
**And** step 1 does not re-traverse from the original origin

### Scenario: StepQuery supports Outgoing direction
**Given** a chain `chain:provenance:research` with two `contains` edges to marks
**When** `StepQuery::from("chain:provenance:research").step(Outgoing, "contains")` executes
**Then** step 0 contains both marks

---

## Feature: Evidence Trail — Composite Query (ADR-013)

### Scenario: Evidence trail returns marks, fragments, and chains for a concept
**Given** a context with:
  - `concept:travel` (semantic dimension)
  - `mark:1` with a `references` edge to `concept:travel` (provenance → semantic)
  - `fragment:abc` with a `tagged_with` edge to `concept:travel` (structure → semantic)
  - `chain:provenance:research` with a `contains` edge to `mark:1`
**When** `evidence_trail("concept:travel")` executes
**Then** the result contains `mark:1` in marks
**And** `fragment:abc` in fragments
**And** `chain:provenance:research` in chains
**And** all three traversed edges

### Scenario: Evidence trail for an isolated concept returns empty
**Given** a context with `concept:obscure` and no edges
**When** `evidence_trail("concept:obscure")` executes
**Then** marks, fragments, chains, and edges are all empty

### Scenario: Evidence trail composes two independent StepQuery branches
**Given** the same context as the first evidence trail scenario
**When** `evidence_trail("concept:travel")` executes
**Then** branch 1 (Incoming "references" → Incoming "contains") populates marks and chains
**And** branch 2 (Incoming "tagged_with") populates fragments
**And** results are merged — all three categories present in a single result

### Scenario: Evidence trail is a PlexusApi convenience, not a query primitive
**Given** `evidence_trail` is implemented in `PlexusApi`
**Then** it composes `StepQuery` calls internally
**And** `StepQuery` is the only new query primitive added to the query system

---

## Feature: Transport-Independent API Layer (ADR-014)

### Scenario: PlexusApi delegates provenance reads to ProvenanceApi
**Given** a context "research" with a chain "field-notes"
**When** `api.list_chains("research", None)` is called
**Then** the result contains the chain from ProvenanceApi
**And** the caller never interacts with ProvenanceApi directly

### Scenario: PlexusApi delegates graph queries to the query system
**Given** a context "research" with a concept node `concept:travel`
**When** `api.find_nodes("research", FindQuery::new().with_node_type("concept"))` is called
**Then** the result contains `concept:travel`
**And** the caller never calls the query system directly

### Scenario: PlexusApi delegates ingest to IngestPipeline
**Given** a PlexusApi with no adapter registered for input_kind "unknown"
**When** `api.ingest("research", "unknown", data)` is called
**Then** the call returns an error from IngestPipeline (proving delegation)
**And** no graph mutations occur

### Scenario: Non-ingest mutations route through ProvenanceApi
**Given** a context "research" with a mark `mark:1` annotated "original"
**When** `api.update_mark("research", "mark:1", Some("updated"), ...)` is called
**Then** the mark's annotation is changed to "updated"
**And** the mutation bypasses the ingest pipeline (read-modify-write pattern)

### Scenario: list_tags is context-scoped (ADR-014 §list_tags)
**Given** context "alpha" with a mark tagged "travel"
**And** context "beta" with a mark tagged "cooking"
**When** `api.list_tags("alpha")` is called
**Then** the result contains "travel"
**And** does not contain "cooking"

### Scenario: context_create rejects duplicate names
**Given** a context "research" already exists
**When** `api.context_create("research")` is called
**Then** the call returns an error indicating the context already exists

### Scenario: context_rename rejects name collisions
**Given** contexts "alpha" and "beta" both exist
**When** `api.context_rename("alpha", "beta")` is called
**Then** the call returns an error indicating "beta" already exists

---

## Feature: Workflow-Oriented Write Surface (ADR-015)

### Scenario: Annotate creates chain and mark in one call
**Given** a context "research" with no chains
**When** `api.annotate("research", "field notes", "src/main.rs", 42, "interesting pattern", ...)` is called
**Then** a chain `chain:provenance:field-notes` is created
**And** a mark is created with annotation "interesting pattern" at src/main.rs:42
**And** the mark is contained in the chain

### Scenario: Annotate reuses an existing chain
**Given** a context "research" where a chain "field notes" already exists
**When** `api.annotate("research", "field notes", "src/lib.rs", 10, "second", ...)` is called
**Then** no new chain is created (still one chain)
**And** a second mark is added to the existing chain

### Scenario: Chain name normalization produces deterministic IDs
**Given** chain names "Field Notes" and "field notes"
**When** both are normalized
**Then** both produce `chain:provenance:field-notes`

### Scenario: Chain name normalization handles special characters
**Given** a chain name "research: phase 1/2"
**When** normalized
**Then** the result is `chain:provenance:research--phase-1-2`
**And** colons and slashes (ID format separators) are replaced with hyphens

### Scenario: Annotate rejects empty chain name
**Given** a context "research"
**When** `api.annotate("research", "", ...)` is called
**Then** the call returns an error
**And** no chain or mark is created

### Scenario: Annotate rejects whitespace-only chain name
**Given** a context "research"
**When** `api.annotate("research", "   ", ...)` is called
**Then** the call returns an error
**And** no chain or mark is created

### Scenario: Annotate triggers the enrichment loop
**Given** a context "research" containing concept `concept:refactor`
**And** a TagConceptBridger enrichment registered
**When** `api.annotate("research", "notes", "src/main.rs", 1, "cleanup", tags: ["refactor"])` is called
**Then** the enrichment loop runs
**And** a cross-dimensional `references` edge is created from the mark to `concept:refactor`

### Scenario: Annotate returns merged outbound events
**Given** a context "research" with no chains
**When** `api.annotate("research", "notes", "src/main.rs", 1, "note", ...)` is called
**Then** the return value is a single `Vec<OutboundEvent>` merging events from both chain creation and mark creation

### Scenario: create_chain is not a consumer-facing operation
**Given** the PlexusApi public surface
**Then** there is no `create_chain` method
**And** chains are created implicitly through `annotate` or adapter-produced provenance

### Scenario: delete_mark routes through ingest pipeline
**Given** a context "research" with a mark `mark:1`
**When** `api.delete_mark("research", "mark:1")` is called
**Then** the deletion routes through `ProvenanceInput::DeleteMark` via `ingest()`
**And** enrichments and outbound events fire

### Scenario: delete_chain cascades to contained marks
**Given** a context "research" with chain "notes" containing marks `mark:1` and `mark:2`
**When** `api.delete_chain("research", "chain:provenance:notes")` is called
**Then** the chain and both marks are removed
**And** the deletion routes through `ProvenanceInput::DeleteChain` via `ingest()`

### Scenario: link_marks validates both endpoints exist
**Given** a context "research" with mark `mark:1` but no mark `mark:999`
**When** `api.link_marks("research", "mark:1", "mark:999")` is called
**Then** the call returns a `LinkError::MarkNotFound("mark:999")` error
**And** no edge is created

### Scenario: unlink_marks routes through ingest pipeline
**Given** a context "research" with marks `mark:1` and `mark:2` linked
**When** `api.unlink_marks("research", "mark:1", "mark:2")` is called
**Then** the unlink routes through `ProvenanceInput::UnlinkMarks` via `ingest()`

---

## Feature: Session Context — MCP Transport (ADR-008)

### Scenario: No __provenance__ context is auto-created
**Given** a freshly initialized PlexusMcpServer
**Then** no context named "__provenance__" exists
**And** no default context is set

### Scenario: set_context activates a context for the session
**Given** a PlexusMcpServer with no active context
**When** `set_context({ name: "research" })` is called
**Then** the active context is set to "research"
**And** the response confirms "active context set to 'research'"

### Scenario: set_context auto-creates the context if it doesn't exist
**Given** no context named "research" exists
**When** `set_context({ name: "research" })` is called
**Then** the context "research" is created
**And** the active context is set to "research"

### Scenario: set_context activates an existing context without duplication
**Given** a context "research" already exists
**When** `set_context({ name: "research" })` is called
**Then** the active context is set to "research"
**And** no duplicate context is created

### Scenario: Tools error if no context is set
**Given** a PlexusMcpServer with no active context (set_context not called)
**When** any provenance tool is called (e.g., `list_chains`)
**Then** the call returns an MCP error with code INVALID_REQUEST
**And** the error message says "no context set — call set_context first"

### Scenario: Tools use the active context after set_context
**Given** `set_context({ name: "research" })` has been called
**When** `list_chains({})` is called
**Then** the tool queries the "research" context
**And** the caller does not specify a context — it uses the session state

---

## Not Covered by These Scenarios

Behaviors deferred or out of scope for this decision cycle:

- **Event streaming** (OQ8) — deferred. Outbound events are synchronous with ingest.
- **Wire protocol schema** (OQ9) — deferred. MCP is the only transport.
- **ingest_fragment MCP tool** — described in ADR-015 but not yet built. The FragmentAdapter exists; the MCP tool wrapping it does not.
- **update_chain** — mentioned in ADR-014 as a provenance mutation but not yet implemented.
- **context_add_sources / context_remove_sources** — removed from MCP surface (commit 05e2d41). Remain as PlexusApi methods.
- **find_nodes / traverse / find_path MCP tools** — PlexusApi exposes these but the MCP server does not surface them yet.
