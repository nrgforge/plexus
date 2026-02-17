# Behavior Scenarios: Phased Extraction Architecture

ADRs: 019 (phased extraction), 020 (declarative adapter specs), 021 (Phase 3 llm-orc integration), 022 (parameterized enrichments), 023 (graph analysis)

---

## Feature: Parameterized Enrichments (ADR-022)

> Build first — this is a refactor of existing code, prerequisites for nothing, and validates the enrichment generalization (Invariant 50).

### Scenario: CoOccurrenceEnrichment accepts relationship parameters
**Given** a CoOccurrenceEnrichment configured with `source_relationship: "exhibits"` and `output_relationship: "co_exhibited"`
**When** two source nodes share edges with relationship `"exhibits"` to the same concept node
**Then** the enrichment emits `co_exhibited` symmetric edge pairs between the co-occurring concepts
**And** the contribution value is the normalized co-occurrence score (`count / max_count`)

### Scenario: Default CoOccurrenceEnrichment backward compatible
**Given** a CoOccurrenceEnrichment with no explicit configuration
**When** two fragment nodes share `tagged_with` edges to the same concept node
**Then** the enrichment emits `may_be_related` symmetric edge pairs (existing behavior unchanged)

### Scenario: Parameterized enrichment has unique stable ID
**Given** a CoOccurrenceEnrichment configured with `source_relationship: "exhibits"` and `output_relationship: "co_exhibited"`
**Then** its `id()` returns `"co_occurrence:exhibits:co_exhibited"`
**And** a second instance with `source_relationship: "tagged_with"` and `output_relationship: "may_be_related"` returns `"co_occurrence:tagged_with:may_be_related"`
**And** the two instances are distinct in the EnrichmentRegistry (not deduplicated)

### Scenario: Structure-aware enrichment fires for any source node type
**Given** a CoOccurrenceEnrichment configured with `source_relationship: "tagged_with"` and `output_relationship: "may_be_related"`
**And** the graph contains a fragment node with `tagged_with` edges to concepts A and B
**And** the graph contains an artifact node (from a declarative adapter) with `tagged_with` edges to concepts B and C
**When** the enrichment runs on the context snapshot
**Then** it detects co-occurrence between A and B (via the fragment), B and C (via the artifact), and emits `may_be_related` edges for both pairs
**And** the enrichment does not filter source nodes by content type

### Scenario: TagConceptBridger accepts relationship parameter
**Given** a TagConceptBridger configured with `relationship: "references"` (default)
**And** a second TagConceptBridger configured with `relationship: "categorized_by"`
**When** a mark with tag `"travel"` is added to a context containing `concept:travel`
**Then** the first bridger creates a `references` edge from the mark to `concept:travel`
**And** the second bridger creates a `categorized_by` edge from the mark to `concept:travel`

---

## Feature: Declarative Adapter Spec Primitives (ADR-020)

> Build second — the DeclarativeAdapter provides the foundation for extraction phases and external consumers.

### Scenario: DeclarativeAdapter interprets create_node primitive
**Given** a declarative adapter spec with:
  ```yaml
  emit:
    - create_node:
        id: "artifact:{input.file_path}"
        type: "artifact"
        dimension: structure
        properties:
          mime_type: "{input.mime_type}"
  ```
**When** `ingest()` is called with input `{ "file_path": "song.mp3", "mime_type": "audio/mpeg" }`
**Then** a node with ID `"artifact:song.mp3"` exists in the context
**And** the node has type `"artifact"`, dimension `"structure"`, and property `mime_type: "audio/mpeg"`

### Scenario: DeclarativeAdapter interprets for_each with create_node and create_edge
**Given** a declarative adapter spec with a `for_each` over `{input.tags}` creating concept nodes and `tagged_with` edges
**When** `ingest()` is called with input `{ "tags": ["jazz", "improv"] }`
**Then** concept nodes `concept:jazz` and `concept:improv` exist in the semantic dimension
**And** `tagged_with` edges connect the source node to each concept with contribution 1.0

### Scenario: DeclarativeAdapter interprets hash_id for deterministic node IDs
**Given** a declarative adapter spec using `hash_id` from `["{adapter_id}", "{input.file_path}"]`
**When** `ingest()` is called twice with the same adapter ID and file path
**Then** both calls produce the same node ID (UUID v5 hash)
**And** the second call upserts the node rather than creating a duplicate

### Scenario: DeclarativeAdapter interprets create_provenance primitive
**Given** a declarative adapter spec with:
  ```yaml
  emit:
    - create_node:
        id: "concept:{input.topic | lowercase}"
        type: "concept"
        dimension: semantic
    - create_provenance:
        chain_id: "chain:{adapter_id}:{input.source}"
        mark_annotation: "{input.title}"
        tags: "{input.tags}"
  ```
**When** `ingest()` is called
**Then** the emission contains both semantic output (concept node) and provenance output (chain + mark + contains edge)
**And** TagConceptBridger creates `references` edges from the mark to matching concepts

### Scenario: DeclarativeAdapter validates input against schema
**Given** a declarative adapter spec with `input_schema` requiring fields `file_path` (string) and `tags` (array)
**When** `ingest()` is called with input missing the `file_path` field
**Then** the adapter returns an error
**And** no emission is produced

### Scenario: DeclarativeAdapter validates dual obligation at registration
**Given** a declarative adapter spec that uses `create_provenance` but has no `create_node` with semantic content
**When** the spec is registered with DeclarativeAdapter
**Then** registration fails with a validation error citing the dual obligation (Invariant 7)

### Scenario: Template expressions apply filters
**Given** a declarative adapter spec using `{input.name | lowercase}` and `{input.tags | sort | join:","}`
**When** `ingest()` is called with `{ "name": "My Project", "tags": ["beta", "alpha"] }`
**Then** the lowercase filter produces `"my project"`
**And** the sort+join filters produce `"alpha,beta"`

---

## Feature: Phased Extraction Coordinator (ADR-019)

> Build third — depends on DeclarativeAdapter for phase adapters.

### Scenario: Extraction coordinator runs Phase 1 synchronously
**Given** an extraction coordinator registered for input kind `extract-file`
**And** Phase 1 (registration) adapter registered
**When** `ingest("extract-file", { "file_path": "docs/example.md" })` is called
**Then** the call returns outbound events from Phase 1
**And** a file node exists in the structure dimension with MIME type and file size
**And** concept nodes exist for any YAML frontmatter tags

### Scenario: Phase 1 metadata failure does not prevent file registration
**Given** an extraction coordinator registered for input kind `extract-file`
**When** `ingest("extract-file", { "file_path": "docs/corrupt-frontmatter.md" })` is called
**And** YAML frontmatter parsing fails
**Then** a file node still exists in the structure dimension with MIME type and file size
**And** no concept nodes are created from metadata
**And** the extraction status node shows Phase 1 complete with a metadata warning

### Scenario: Extraction coordinator spawns Phase 2 as background task
**Given** an extraction coordinator registered for input kind `extract-file`
**And** Phase 2 (analysis) adapter registered for MIME type `text/markdown`
**When** `ingest("extract-file", { "file_path": "docs/example.md" })` is called
**Then** the call returns immediately after Phase 1 completes
**And** Phase 2 runs as a background task that calls `ingest()` with its own input kind
**And** Phase 2's emission is persisted independently when it completes

### Scenario: Phase 2 dispatches by MIME type
**Given** an extraction coordinator with text and audio Phase 2 adapters registered
**When** `ingest("extract-file", { "file_path": "song.mp3" })` is called
**Then** the coordinator dispatches to the audio analysis adapter (not the text adapter)
**And** the Phase 2 adapter ID reflects the modality (e.g., `extract-analysis-audio`)

### Scenario: Phase 3 spawns only after Phase 2 completes
**Given** an extraction coordinator with Phases 2 and 3 enabled
**When** `ingest("extract-file", { "file_path": "docs/example.md" })` is called
**Then** Phase 3 does not begin until Phase 2's `ingest()` call returns
**And** Phase 3 receives Phase 2's structural output (sections, extracted terms) as input

### Scenario: Extraction status tracks phase completion
**Given** an extraction coordinator processing a file
**When** Phase 1 completes
**Then** the extraction status node shows Phase 1 complete, Phases 2-3 pending
**When** Phase 2 completes
**Then** the extraction status node shows Phases 1-2 complete, Phase 3 pending

### Scenario: Background phase failure does not affect earlier phases
**Given** an extraction coordinator that has completed Phases 1-2
**When** Phase 3 fails (e.g., llm-orc unavailable)
**Then** Phases 1-2 results remain persisted in the graph
**And** the extraction status node shows Phases 1-2 complete, Phase 3 failed
**And** no rollback occurs

### Scenario: Each phase has a distinct adapter ID
**Given** an extraction coordinator processing a file
**When** Phase 1 and Phase 2 both discover `concept:xdg-open`
**Then** the `tagged_with` edge from the file node to `concept:xdg-open` has two contribution slots
**And** the contributions are keyed by distinct adapter IDs (e.g., `extract-registration`, `extract-analysis-text`)

### Scenario: Enrichments fire incrementally after each phase
**Given** TagConceptBridger and CoOccurrenceEnrichment registered
**When** Phase 1 adds concept nodes from YAML frontmatter
**Then** TagConceptBridger creates `references` edges for marks matching those concepts
**When** Phase 2 later adds more concept nodes from link extraction
**Then** CoOccurrenceEnrichment detects new co-occurrence pairs including cross-phase concepts

### Scenario: Concurrency control limits background phases
**Given** an analysis phase semaphore with capacity 4
**And** 10 files submitted for extraction simultaneously
**Then** at most 4 Phase 2 (analysis) tasks run concurrently
**And** the remaining tasks wait for semaphore permits

---

## Feature: Phase 3 llm-orc Integration (ADR-021)

> Build fourth — depends on phased extraction coordinator and external llm-orc service.

### Scenario: Phase 3 delegates to llm-orc
**Given** llm-orc running as a persistent service
**And** Phase 2 completed for a file, producing sections and extracted terms
**When** Phase 3 starts
**Then** Phase 3 serializes Phase 2 output to JSON
**And** calls llm-orc's `invoke` endpoint with the extraction ensemble
**And** deserializes the response into an emission with concept nodes and edges

### Scenario: Phase 3 graceful degradation
**Given** llm-orc is NOT running
**When** `ingest("extract-file", { "file_path": "docs/example.md" })` is called
**Then** Phases 1-2 complete normally
**And** Phase 3 is skipped
**And** the extraction status node shows Phase 3 as skipped (not failed)
**And** no error is surfaced to the original caller

### Scenario: Long document chunking via Phase 2 boundaries
**Given** Phase 2 identified structural boundaries (e.g., acts and scenes in a Shakespeare play)
**When** Phase 3 processes the file
**Then** llm-orc chunks the document along Phase 2's structural boundaries
**And** runs parallel semantic extraction per chunk via fan-out
**And** each chunk emission persists independently

---

## Feature: External Enrichment — On-Demand (ADR-023, vocabulary updated by ADR-024)

> Build last — depends on llm-orc integration and a mature graph to analyze.

### Scenario: External enrichment results enter via ingest
**Given** an external enrichment ensemble completes (e.g., PageRank)
**When** results are returned to Plexus
**Then** a `GraphAnalysisAdapter` with stable ID (e.g., `graph-analysis:pagerank`) ingests the results via `ingest()`
**And** existing nodes receive property updates (`pagerank_score: 0.034`)
**And** the enrichment loop fires after the ingest (standard pipeline)

### Scenario: External enrichment does not run in enrichment loop
**Given** a PageRank external enrichment ensemble
**When** a new fragment is ingested via the normal pipeline
**Then** the enrichment loop runs (TagConceptBridger, CoOccurrenceEnrichment)
**And** PageRank does NOT run — it is not registered as a core enrichment

### Scenario: On-demand external enrichment
**Given** a context with 200 concept nodes and 500 edges
**When** `plexus analyze my-context` is invoked
**Then** the graph is exported to a format consumable by llm-orc script agents
**And** the llm-orc `graph-analysis` ensemble runs (PageRank, community detection in parallel)
**And** results are applied back via `ingest()`

---

## Conformance Debt

All items resolved. Evidence listed per row.

| ADR | Violation | Type | Location | Resolution | Status |
|-----|-----------|------|----------|------------|--------|
| ADR-022 | CoOccurrenceEnrichment hardcodes `"tagged_with"` and `"may_be_related"` | wrong-structure | `src/adapter/cooccurrence.rs` | Refactored: `with_relationships()` constructor accepts parameters; `new()` defaults to original values | **Resolved** — tests: `accepts_relationship_parameters`, `default_backward_compatible` in `cooccurrence.rs` |
| ADR-022 | TagConceptBridger hardcodes `"references"` | wrong-structure | `src/adapter/tag_bridger.rs` | Refactored: `with_relationship()` constructor accepts parameter; `new()` defaults to `"references"` | **Resolved** — tests: `accepts_relationship_parameter` in `tag_bridger.rs` |
| ADR-022 | CoOccurrenceEnrichment `id()` returns static `"co-occurrence"` | wrong-structure | `src/adapter/cooccurrence.rs` | ID generated from parameters: `co_occurrence:{source}:{output}` | **Resolved** — test: `parameterized_id_from_relationships` in `cooccurrence.rs` |
| ADR-022 | Enrichment assumes fragment source nodes (not structure-aware) | wrong-structure | `src/adapter/cooccurrence.rs` | Fires based on relationship structure, not node content type (Invariant 50) | **Resolved** — test: `structure_aware_fires_for_any_source_node_type` in `cooccurrence.rs` |
