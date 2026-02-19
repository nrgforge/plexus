# Behavior Scenarios — Essay 22 Items 1–5

**Date:** 2026-02-19
**Source:** Essay 22 ("The Integration Gap"), items 1–5 from "What the Next Build Cycle Should Deliver"
**ADRs:** 019 (phased extraction), 020 (declarative adapter), 021 (Phase 3 llm-orc), 025 (spec extensions)

These are conformance gaps — the decisions already exist. No new ADRs are needed; the code doesn't match the architecture.

---

## Conformance Debt Table (Step 3.5)

| # | ADR | Violation | Type | Location | Resolution |
|---|-----|-----------|------|----------|------------|
| 1 | **019/021** | Coordinator sends `ExtractFileInput` to Phase 3; `SemanticAdapter` expects `SemanticInput`. Phase 3 always fails with `InvalidInput` when dispatched by the coordinator. | wrong-structure | `extraction.rs:447-451`, `semantic.rs:322-324` | Coordinator constructs `SemanticInput` from Phase 2 output (file path, section boundaries). |
| 2 | **025** | `DeclarativeAdapter::process()` stores `spec.ensemble` but never invokes it. The two-layer architecture (ensemble extractor → declarative mapper) is structurally absent. | missing | `declarative.rs:329` (field), `declarative.rs:638-665` (process, no invocation) | Add ensemble invocation: check `self.spec.ensemble` → call `LlmOrcClient::invoke()` → merge response into template context → proceed with emit primitives. |
| 3 | **019** (Inv 30, 46) | Background phases use `EngineSink::new(Arc<Mutex<Context>>)` — in-memory test path. Emissions accumulate in a Mutex-held context clone that is never persisted. Persist-per-emission (Invariant 30) violated. | wrong-structure | `extraction.rs:57` (field), `extraction.rs:405, 439` (construction) | Pass `Arc<PlexusEngine>` + `ContextId` to background tasks. Use `EngineSink::for_engine()`. Remove `shared_context` field. |
| 4 | **019** (Inv 5, 7) | `SemanticAdapter::process()` emits concept nodes and relationship edges but no provenance trail. No chain, mark, or contains edges. Dual obligation (Invariant 7) violated. | missing | `semantic.rs:317-375` (process), `semantic.rs:207-304` (parse_response — concepts only) | Add provenance: chain scoped to adapter run, mark per passage with concept tags, contains edges from chain to marks. |
| 5 | **019** | No concrete Phase 2 adapter for `text/*` MIME types. Coordinator's `register_phase2()` API works; tests use stubs. No real text analysis adapter exists. | missing | `extraction.rs:91-98` (registration API only) | Build `TextAnalysisAdapter`: detect section boundaries, extract proper nouns, compute term frequency. Output feeds Phase 3 via `SemanticInput.sections`. |

---

## Feature: Coordinator-to-Phase 3 Type Alignment (Item 1, Gap 2)

Addresses: ADR-019/021 — coordinator must construct `SemanticInput` for Phase 3, not forward `ExtractFileInput`.

### Scenario: Coordinator constructs SemanticInput for Phase 3
**Given** an ExtractionCoordinator with a SemanticAdapter registered as Phase 3
**When** the coordinator processes an `extract-file` input for a text file
**Then** the SemanticAdapter receives a `SemanticInput` (not `ExtractFileInput`)
**And** the `SemanticInput.file_path` matches the original `ExtractFileInput.file_path`

### Scenario: Phase 2 section boundaries propagate to Phase 3
**Given** an ExtractionCoordinator with a Phase 2 text adapter that detects section boundaries and a Phase 3 SemanticAdapter
**When** the coordinator processes a text file and Phase 2 produces section boundaries
**Then** the `SemanticInput` received by Phase 3 contains the `sections` from Phase 2's output
**And** each `SectionBoundary` has a non-empty `label`, `start_line`, and `end_line`

### Scenario: Phase 3 runs without section boundaries when Phase 2 produces none
**Given** an ExtractionCoordinator with a Phase 2 adapter that emits no section boundaries and a Phase 3 SemanticAdapter
**When** the coordinator processes a file
**Then** the `SemanticInput` received by Phase 3 has an empty `sections` vec
**And** Phase 3 processes the whole file (no chunking)

### Scenario: Coordinator dispatches to real SemanticAdapter (integration)
**Given** an ExtractionCoordinator with a real SemanticAdapter (not a stub) backed by a MockClient
**And** the coordinator uses `EngineSink::for_engine()` (not the Mutex path)
**When** the coordinator processes an `extract-file` input end-to-end
**Then** SemanticAdapter receives input it can downcast to `SemanticInput` without error
**And** the ensemble is invoked with file content
**And** concept nodes from the ensemble response are persisted in the engine's context

---

## Feature: DeclarativeAdapter Ensemble Invocation (Item 2, Gap 1)

Addresses: ADR-025 — `DeclarativeAdapter` must invoke its `ensemble` via `LlmOrcClient` before interpreting emit primitives.

### Scenario: DeclarativeAdapter invokes ensemble when spec declares one
**Given** a DeclarativeAdapter with `spec.ensemble = Some("code-concepts")`
**And** a registered `LlmOrcClient`
**When** `process()` is called with valid JSON input
**Then** `LlmOrcClient::invoke()` is called with ensemble name `"code-concepts"`
**And** the ensemble response is merged into the template context

### Scenario: Emit primitives access ensemble response fields
**Given** a DeclarativeAdapter with `spec.ensemble = Some("test-ensemble")`
**And** emit primitives that use template expressions referencing `ensemble.concepts`
**When** `process()` is called and the ensemble returns `{"concepts": [{"label": "X"}]}`
**Then** the emit primitives resolve `ensemble.concepts` from the response
**And** `for_each` over `ensemble.concepts` iterates the returned array

### Scenario: DeclarativeAdapter degrades gracefully when llm-orc unavailable
**Given** a DeclarativeAdapter with `spec.ensemble = Some("test-ensemble")`
**And** llm-orc is not running
**When** `process()` is called
**Then** the adapter returns `AdapterError::Skipped` (Invariant 47)
**And** no emissions are produced

### Scenario: DeclarativeAdapter without ensemble works unchanged
**Given** a DeclarativeAdapter with `spec.ensemble = None`
**When** `process()` is called with valid JSON input
**Then** the adapter interprets emit primitives directly (existing behavior)
**And** no `LlmOrcClient` call is made

### Scenario: DeclarativeAdapter with real LlmOrcClient and EngineSink (integration)
**Given** a DeclarativeAdapter loaded from a YAML spec file with `ensemble: "test-ensemble"`
**And** a real `LlmOrcClient` (backed by MockClient for test isolation)
**And** a real `EngineSink` backed by `PlexusEngine`
**When** `process()` is called
**Then** the ensemble is invoked
**And** emissions (nodes, edges) are persisted to the engine's context
**And** the context survives engine reload

---

## Feature: Background Phase Persistence (Item 3, Gap 4)

Addresses: ADR-019, Invariant 30 (persist-per-emission), Invariant 46 (background phases are independent adapter runs).

### Scenario: Phase 2 emissions persist through the engine
**Given** an ExtractionCoordinator with a Phase 2 adapter
**And** the coordinator holds an `Arc<PlexusEngine>` and `ContextId` (not `Arc<Mutex<Context>>`)
**When** the coordinator processes a file and Phase 2 emits concept nodes
**Then** the concept nodes are persisted via `PlexusEngine` (each `emit()` calls `save_context()`)
**And** the nodes exist in the context after engine reload from store

### Scenario: Phase 3 emissions persist through the engine
**Given** an ExtractionCoordinator with Phase 2 and Phase 3 adapters and an `Arc<PlexusEngine>`
**When** Phase 2 and Phase 3 both complete
**Then** Phase 3 emissions are persisted via `PlexusEngine`
**And** concept nodes from both phases exist in the context after reload

### Scenario: Phase 2 failure does not affect Phase 1 persistence
**Given** an ExtractionCoordinator with a failing Phase 2 adapter and an `Arc<PlexusEngine>`
**When** the coordinator processes a file
**Then** Phase 1 nodes (file node, YAML metadata) are already persisted (committed via the primary sink before background spawn)
**And** Phase 2's failure does not roll back Phase 1 results

### Scenario: Background phase persistence round-trip through SqliteStore (integration)
**Given** an ExtractionCoordinator backed by `PlexusEngine` with `SqliteStore`
**And** a Phase 2 adapter that emits a concept node
**When** the coordinator processes a file and background tasks complete
**Then** a new `PlexusEngine` loaded from the same `SqliteStore` contains the Phase 2 concept node
**And** the extraction status node shows `phase2: "complete"`

---

## Feature: SemanticAdapter Provenance Trail (Item 4, Gap 6)

Addresses: ADR-019, Invariant 5/7 (dual obligation — semantic content AND provenance from every adapter).

### Scenario: SemanticAdapter produces chain node
**Given** a SemanticAdapter processing a file at path `"test.txt"`
**When** the adapter emits concept nodes
**Then** the emission also contains a chain node with ID `chain:extract-semantic:test.txt`
**And** the chain node is in the provenance dimension

### Scenario: SemanticAdapter produces mark per extracted passage
**Given** a SemanticAdapter processing a file that yields concepts from two distinct passages
**When** the adapter emits
**Then** the emission contains two mark nodes (one per passage)
**And** each mark carries the concept labels from its passage as tags
**And** each mark has the file_path and passage location as properties

### Scenario: SemanticAdapter produces contains edges
**Given** a SemanticAdapter producing a chain and marks
**When** the emission is committed
**Then** `contains` edges connect the chain node to each mark node
**And** each `contains` edge has a contribution of 1.0 from `extract-semantic`

### Scenario: SemanticAdapter provenance triggers tag-to-concept bridging (integration)
**Given** a SemanticAdapter with real `EngineSink::for_engine()` and a `TagConceptBridger` enrichment registered on the engine
**When** the adapter emits concepts and marks with matching tags
**Then** the enrichment loop creates `references` edges from marks to concept nodes
**And** the `references` edges are cross-dimensional (provenance → semantic)

---

## Feature: Phase 2 Text Analysis Adapter (Item 5, Gap 3)

Addresses: ADR-019 — modality-dispatched Phase 2 adapters. No concrete `text/*` adapter exists.

### Scenario: TextAnalysisAdapter detects section boundaries in plain text
**Given** a text file with sections separated by blank lines and heading patterns (e.g., lines in ALL CAPS or lines starting with `#`)
**When** the `TextAnalysisAdapter` processes the file
**Then** the emission contains section boundary metadata as properties on the file node
**And** each section has `label`, `start_line`, and `end_line`

### Scenario: TextAnalysisAdapter detects act/scene markers in dramatic text
**Given** a text file containing "ACT I" and "SCENE 1" markers (Macbeth format)
**When** the `TextAnalysisAdapter` processes the file
**Then** section boundaries identify each act and scene with start/end positions
**And** act boundaries are hierarchically above scene boundaries

### Scenario: TextAnalysisAdapter extracts proper nouns as concepts
**Given** a text file containing "Macbeth" and "Scotland" as capitalized proper nouns
**When** the `TextAnalysisAdapter` processes the file
**Then** concept nodes are emitted with IDs `concept:macbeth` and `concept:scotland`
**And** edges connect the file node to each concept with relationship `mentions`

### Scenario: TextAnalysisAdapter produces provenance trail
**Given** a `TextAnalysisAdapter` processing a file
**When** the adapter emits
**Then** the emission includes a chain node (`chain:extract-analysis-text:{file_path}`)
**And** mark nodes annotating the sections where concepts were found
**And** `contains` edges from chain to marks (Invariant 7 — dual obligation)

### Scenario: TextAnalysisAdapter output consumed by real SemanticAdapter (integration)
**Given** a real `TextAnalysisAdapter` registered as Phase 2 for `"text/"`
**And** a real `SemanticAdapter` registered as Phase 3
**And** a real `ExtractionCoordinator` backed by `PlexusEngine`
**When** the coordinator processes a text file end-to-end
**Then** Phase 3 receives `SemanticInput` with `sections` populated from Phase 2's analysis
**And** Phase 3 produces concepts scoped to those sections
**And** both phases' emissions are persisted independently (Invariant 46)
