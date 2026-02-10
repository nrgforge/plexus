# Behavior Scenarios: Runtime Architecture (ADR-006 through ADR-009)

---

## Feature: Adapter-to-Engine Wiring (ADR-006)

### Scenario: Emission through engine-backed sink reaches storage
**Given** a PlexusEngine with a SqliteStore and an existing context "provence-research"
**And** an EngineSink created via `EngineSink::for_engine(engine, "provence-research")`
**When** an adapter emits a single node through the sink
**Then** the node exists in the engine's in-memory context "provence-research"
**And** after restarting the engine (hydrate from storage), the node still exists in "provence-research"

### Scenario: Emission through engine-backed sink persists edges with contributions
**Given** a PlexusEngine with a SqliteStore and an existing context "provence-research"
**And** an EngineSink created via `EngineSink::for_engine(engine, "provence-research")`
**And** two nodes already exist in the context
**When** an adapter emits an edge between the two nodes with contribution value 0.75
**Then** after restarting the engine, the edge exists with `contributions["adapter-id"] == 0.75`
**And** the raw weight is recomputed correctly from the persisted contributions

### Scenario: Emission to a non-existent context returns an error
**Given** a PlexusEngine with a SqliteStore
**And** no context named "does-not-exist"
**And** an EngineSink created via `EngineSink::for_engine(engine, "does-not-exist")`
**When** an adapter emits a node through the sink
**Then** the emit returns an error indicating context not found
**And** no data is persisted

### Scenario: Persist-per-emission writes once per emit call
**Given** a PlexusEngine with a SqliteStore and an existing context "provence-research"
**And** an EngineSink created via `EngineSink::for_engine(engine, "provence-research")`
**When** an adapter emits an emission containing 3 nodes and 2 edges
**Then** all 3 nodes and 2 edges exist in the in-memory context
**And** all 3 nodes and 2 edges survive a restart (single persist, not per-item)

### Scenario: Existing Mutex-based sink still works for tests
**Given** an EngineSink created via the existing `Arc<Mutex<Context>>` constructor
**When** an adapter emits a node through the sink
**Then** the node exists in the context
**And** no persistence occurs (no GraphStore involved)

---

## Feature: Contribution Persistence (ADR-007)

### Scenario: Contributions survive save and load
**Given** a context with an edge carrying contributions `{"fragment-manual": 1.0, "co-occurrence": 0.75}`
**When** the context is saved to SqliteStore and then loaded back
**Then** the loaded edge has contributions `{"fragment-manual": 1.0, "co-occurrence": 0.75}`

### Scenario: Existing edges without contributions load with empty map
**Given** a SqliteStore database created before the contributions_json migration
**When** the migration runs and existing edges are loaded
**Then** each existing edge has an empty contributions map `{}`
**And** existing raw_weight values are preserved unchanged

### Scenario: Scale normalization works after reload
**Given** a context with two edges from different adapters:
  - edge A: contributions `{"adapter-1": 5.0}`
  - edge B: contributions `{"adapter-1": 10.0}`
**When** the context is saved, loaded, and `recompute_raw_weights()` is called
**Then** the raw weights reflect correct scale normalization from the persisted contributions
**And** the results match what the in-memory computation would have produced

---

## Feature: Project-Scoped Provenance (ADR-008)

### Scenario: Mark is created in a project context
**Given** a PlexusEngine with an existing context "provence-research"
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with context "provence-research", chain "reading-notes", file "notes.md", line 42, annotation "walking through Avignon"
**Then** a mark node exists in context "provence-research" with dimension "provenance"
**And** a "contains" edge connects chain "reading-notes" to the new mark

### Scenario: Mark creation without a context fails
**Given** a PlexusEngine with no contexts
**When** `add_mark` is called without a context parameter
**Then** the operation fails with an error indicating a context is required

### Scenario: No __provenance__ context is auto-created
**Given** a freshly initialized PlexusEngine
**When** listing all contexts
**Then** no context named "__provenance__" exists

### Scenario: list_tags returns tags from all contexts
**Given** a PlexusEngine with two contexts:
  - "provence-research" containing marks tagged `#travel` and `#avignon`
  - "desk" containing marks tagged `#travel` and `#writing`
**When** `list_tags()` is called
**Then** the result contains `["travel", "avignon", "writing"]` (deduplicated, from all contexts)

### Scenario: Chains are scoped to their context
**Given** a chain "reading-notes" in context "provence-research"
**And** a chain "desk-notes" in context "desk"
**When** listing chains in context "provence-research"
**Then** only "reading-notes" is returned

---

## Feature: Tag-to-Concept Bridging (ADR-009)

### Scenario: Mark with matching concept gets references edge
**Given** a context "provence-research" containing a concept node `concept:travel`
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with tags `["#travel"]` in context "provence-research"
**Then** a cross-dimensional `references` edge exists from the new mark (provenance dimension) to `concept:travel` (semantic dimension)

### Scenario: Mark with multiple matching concepts gets multiple references edges
**Given** a context "provence-research" containing concept nodes `concept:travel` and `concept:avignon`
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with tags `["#travel", "#avignon"]` in context "provence-research"
**Then** two cross-dimensional `references` edges exist: mark → `concept:travel` and mark → `concept:avignon`

### Scenario: Mark with non-matching tag gets no references edge
**Given** a context "provence-research" containing a concept node `concept:travel`
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with tags `["#walking"]` in context "provence-research"
**Then** no `references` edge is created (no concept node `concept:walking` exists)
**And** the mark is still created successfully

### Scenario: Tag format normalization strips # and lowercases
**Given** a context "provence-research" containing a concept node `concept:travel`
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with tags `["#Travel"]` in context "provence-research"
**Then** a `references` edge exists from the mark to `concept:travel` (normalization: strip `#`, lowercase)

### Scenario: Mark created before concept is not bridged (known limitation)
**Given** a context "provence-research" with no concept nodes
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with tags `["#travel"]` in context "provence-research"
**Then** the mark is created successfully with no `references` edges
**And** when a concept node `concept:travel` is later added to the context, no retroactive bridging occurs

### Scenario: Mark with no tags gets no references edges
**Given** a context "provence-research" containing concept nodes
**And** a chain "reading-notes" in context "provence-research"
**When** `add_mark` is called with no tags in context "provence-research"
**Then** the mark is created successfully with no `references` edges

---

## Feature: End-to-End — Provence Travel Research

### Scenario: Full workflow from ingestion through marking to query
**Given** a PlexusEngine with SqliteStore and an existing context "provence-research"
**And** a FragmentAdapter with EngineSink wired to the engine for context "provence-research"
**When** the adapter processes a fragment with text "Morning walk in Avignon" and tags `["travel", "avignon"]`
**Then** context "provence-research" contains:
  - a fragment node (structure dimension)
  - concept nodes `concept:travel` and `concept:avignon` (semantic dimension)
  - `tagged_with` edges from fragment to each concept
**When** a chain "reading-notes" is created in context "provence-research"
**And** `add_mark` is called with file "notes.md", line 10, annotation "walking through Avignon", tags `["#travel", "#avignon"]`
**Then** the mark has `references` edges to both `concept:travel` and `concept:avignon`
**And** traversing from `concept:avignon` via incoming `references` edges reaches the mark
**And** after restarting the engine, all nodes, edges, and contributions are intact
