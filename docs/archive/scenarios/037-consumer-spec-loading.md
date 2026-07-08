# Behavior Scenarios: Consumer Spec Loading

**ADR:** 037 (consumer spec loading)

**Domain vocabulary:** load spec, declarative adapter spec, lens, vocabulary layer, enrichment, ingest, context, adapter, consumer, spec validation, unload spec

**Note:** These scenarios verify the spec loading lifecycle: validation, wiring, persistence, lens enrichment execution, and multi-consumer interaction. Query-level filter and lens translation semantics are tested by scenarios 033-035. MCP transport wiring for `load_spec` is tested by scenarios 036.

---

## Feature: Spec Validation (ADR-037 §1, Invariant 60)

### Scenario: valid spec loads successfully
**Given** a context "test" exists
**And** a valid declarative adapter spec YAML with adapter ID "trellis", input kind "content", and a lens section
**When** `load_spec` is called with context "test" and the spec YAML
**Then** the result contains `adapter_id: "trellis"`
**And** the result contains the lens namespace
**And** the adapter is available for ingest routing on context "test"

### Scenario: invalid spec YAML fails before any graph work
**Given** a context "test" exists
**And** a malformed YAML string (not valid YAML)
**When** `load_spec` is called with context "test" and the malformed YAML
**Then** the result is an error indicating validation failure
**And** no adapter is registered on the pipeline
**And** no enrichments are registered
**And** no edges are added to the graph

### Scenario: spec with invalid lens rules fails validation
**Given** a context "test" exists
**And** a spec YAML with a valid adapter section but an invalid lens section (e.g., translation rule referencing a non-existent relationship type format)
**When** `load_spec` is called with context "test" and the spec YAML
**Then** the result is an error indicating lens validation failure
**And** no adapter is registered on the pipeline

### Scenario: spec with duplicate adapter ID replaces existing spec
**Given** a context "test" with a previously loaded spec for adapter ID "trellis"
**When** `load_spec` is called with context "test" and a new spec YAML with adapter ID "trellis"
**Then** the new spec replaces the old one
**And** the new adapter definition is used for subsequent ingest routing
**And** the new lens rules are used for subsequent enrichment

---

## Feature: Complete Spec Wiring (ADR-037 §§1,4)

### Scenario: load_spec registers adapter, enrichments, and lens atomically
**Given** a context "test" exists
**And** a spec YAML with adapter ID "trellis", a co_occurrence enrichment declaration, and a lens section
**When** `load_spec` is called with context "test" and the spec YAML
**Then** the adapter is registered for ingest routing
**And** the co_occurrence enrichment is registered in the enrichment registry
**And** the lens enrichment is registered in the enrichment registry

> **Note (2026-04-14):** A "register_specs_from_dir wires enrichments and lens" scenario was previously recorded here. It was removed when file-based spec auto-discovery was deleted from the pipeline (MCP cycle WP-H.1, ADR-037 §4 supersession). The remaining wiring scenario — `load_spec wires adapter, enrichments, and lens from a valid spec` — covers the intentional path.

---

## Feature: Lens Enrichment Execution (ADR-037 §§1,3)

### Scenario: lens runs immediately on existing graph content
**Given** a context "test" with existing concept nodes and `may_be_related` edges
**When** `load_spec` is called with a spec containing a lens that translates `may_be_related` edges to `lens:trellis:thematic_connection`
**Then** the result reports the number of vocabulary edges created
**And** the graph contains `lens:trellis:thematic_connection` edges for each translated pair

### Scenario: lens fires on subsequent ingest by another consumer
**Given** a context "test" with a loaded spec for "trellis" containing a lens
**And** a loaded spec for "carrel" with a different adapter
**When** content is ingested via the "carrel" adapter that produces new `may_be_related` edges
**Then** the enrichment loop fires the "trellis" lens enrichment
**And** new `lens:trellis:thematic_connection` edges are created for the newly ingested content
**And** these enrichment events are persisted to the event log

### Scenario: multiple lenses on same context compound vocabulary layers
**Given** a context "test" with concept nodes and `may_be_related` edges
**When** a spec for "trellis" is loaded with a lens translating to `lens:trellis:thematic_connection`
**And** a spec for "carrel" is loaded with a lens translating to `lens:carrel:citation_link`
**Then** the graph contains both `lens:trellis:thematic_connection` and `lens:carrel:citation_link` edges
**And** both vocabulary layers are queryable via `relationship_prefix` filters

---

## Feature: Spec Persistence (ADR-037 §§2,3)

### Scenario: loaded spec persists in SQLite
**Given** a context "test" exists with SQLite persistence
**When** `load_spec` is called with a valid spec YAML
**Then** the `specs` table contains a row with the context ID, adapter ID, and spec YAML

### Scenario: persisted specs re-register on startup
**Given** a SQLite database with a persisted spec for adapter "trellis" on context "test"
**When** `PlexusEngine` starts and loads contexts
**Then** the "trellis" adapter is registered for ingest routing
**And** the "trellis" lens enrichment is registered in the enrichment registry
**And** subsequent ingests by other adapters trigger the "trellis" lens

### Scenario: lens enrichment fires correctly after restart
**Given** a persisted spec for "trellis" with a lens, and a context "test" with existing vocabulary edges
**When** the engine restarts and re-registers the spec
**And** new content is ingested via a different adapter
**Then** the "trellis" lens fires in the enrichment loop
**And** new vocabulary edges are created for the new content

---

## Feature: Spec Unloading (ADR-037 §6)

### Scenario: unload_spec removes adapter routing and enrichment registration
**Given** a context "test" with a loaded spec for "trellis"
**When** `unload_spec` is called with context "test" and adapter ID "trellis"
**Then** the "trellis" adapter is no longer available for ingest routing
**And** the "trellis" lens enrichment is no longer registered
**And** the spec is removed from the `specs` table

### Scenario: unload_spec preserves vocabulary edges
**Given** a context "test" with a loaded spec for "trellis" and existing `lens:trellis:thematic_connection` edges
**When** `unload_spec` is called with context "test" and adapter ID "trellis"
**Then** the `lens:trellis:thematic_connection` edges remain in the graph
**And** those edges are still queryable via `relationship_prefix: "lens:trellis"`

---

## Feature: Vocabulary Layer Discovery

### Scenario: consumer discovers vocabulary layers via relationship_prefix query
**Given** a context "test" with vocabulary layers from "trellis" (`lens:trellis:*`) and "carrel" (`lens:carrel:*`)
**When** `find_nodes` is called with `relationship_prefix: "lens:"`
**Then** the result includes nodes connected by any lens-created edges
**And** the distinct relationship prefixes in the results reveal the available vocabulary layers

---

## Feature: Integration — End-to-End Consumer Workflow

### Scenario: full e2e flow through load_spec, ingest, and query
**Given** a fresh context "integration-test"
**And** a valid spec YAML for adapter "trellis" with a lens
**When** the spec is loaded via `load_spec`
**And** content is ingested via the "trellis" adapter
**And** the enrichment loop completes (including lens translation)
**Then** `find_nodes` returns concept nodes from the ingested content
**And** `traverse` from a concept node returns both raw and lens-translated edges
**And** `changes_since` with cursor 0 returns events including lens-created edges

### Scenario: second consumer adds vocabulary layer to existing context
**Given** a context "integration-test" with loaded spec "trellis" and ingested content
**When** a second spec for "carrel" (with its own lens) is loaded via `load_spec`
**And** content is ingested via the "carrel" adapter
**Then** `traverse` from a shared concept returns edges from both vocabulary layers
**And** `find_nodes` with `relationship_prefix: "lens:trellis"` returns only trellis-relevant nodes
**And** `find_nodes` with `relationship_prefix: "lens:carrel"` returns only carrel-relevant nodes
