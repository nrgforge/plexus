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
| 2 | **025** | `SemanticAdapter::process()` stores `spec.ensemble` but never invokes it. The two-layer architecture (ensemble extractor → declarative mapper) is structurally absent. | missing | `declarative.rs:329` (field), `declarative.rs:638-665` (process, no invocation) | Add ensemble invocation: check `self.spec.ensemble` → call `LlmOrcClient::invoke()` → merge response into template context → proceed with emit primitives. |
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

## Feature: SemanticAdapter Ensemble Invocation (Item 2, Gap 1)

Addresses: ADR-025 — `SemanticAdapter` must invoke its `ensemble` via `LlmOrcClient` before interpreting emit primitives.

### Scenario: SemanticAdapter invokes ensemble when spec declares one
**Given** a SemanticAdapter with `spec.ensemble = Some("code-concepts")`
**And** a registered `LlmOrcClient`
**When** `process()` is called with valid JSON input
**Then** `LlmOrcClient::invoke()` is called with ensemble name `"code-concepts"`
**And** the ensemble response is merged into the template context

### Scenario: Emit primitives access ensemble response fields
**Given** a SemanticAdapter with `spec.ensemble = Some("test-ensemble")`
**And** emit primitives that use template expressions referencing `ensemble.concepts`
**When** `process()` is called and the ensemble returns `{"concepts": [{"label": "X"}]}`
**Then** the emit primitives resolve `ensemble.concepts` from the response
**And** `for_each` over `ensemble.concepts` iterates the returned array

### Scenario: SemanticAdapter degrades gracefully when llm-orc unavailable
**Given** a SemanticAdapter with `spec.ensemble = Some("test-ensemble")`
**And** llm-orc is not running
**When** `process()` is called
**Then** the adapter returns `AdapterError::Skipped` (Invariant 47)
**And** no emissions are produced

### Scenario: SemanticAdapter without ensemble works unchanged
**Given** a SemanticAdapter with `spec.ensemble = None`
**When** `process()` is called with valid JSON input
**Then** the adapter interprets emit primitives directly (existing behavior)
**And** no `LlmOrcClient` call is made

### Scenario: SemanticAdapter with real LlmOrcClient and EngineSink (integration)
**Given** a SemanticAdapter loaded from a YAML spec file with `ensemble: "test-ensemble"`
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

---
---

# Behavior Scenarios — ADR-028: Universal MCP Ingest Tool

**Date:** 2026-02-19
**Source:** Essay 23 ("Universal Ingest — One Tool, One Ensemble, One Graph"), ADR-028
**ADRs:** 028 (universal MCP ingest), supersedes 015 (workflow-oriented write surface)
**Depends on:** ADR-012 (unified ingest pipeline), ADR-020 (declarative adapter specs)

---

## Conformance Debt Table (Step 3.5)

| # | ADR | Violation | Type | Location | Resolution |
|---|-----|-----------|------|----------|------------|
| 1 | **028** | MCP server exposes `annotate` tool instead of `ingest` | exists | `mcp/mod.rs:106-134` | Replace with `ingest(data, input_kind?)` accepting JSON |
| 2 | **028** | `PlexusApi.annotate()` still exists (86-line composite method) | exists | `api.rs:48-133` | Remove; migrate composite logic into ContentAdapter |
| 3 | **028** | Pipeline registers only FragmentAdapter + ProvenanceAdapter | wrong-structure | `mcp/mod.rs:55-63` | Register three core adapters (ContentAdapter, ExtractionCoordinator, SemanticAdapter) + all core enrichments |
| 4 | **028** | No input classifier component exists | missing | — | Build input classifier for `input_kind` detection from JSON |
| 5 | **028** | MCP params are domain-specific structs (`AnnotateParams`) | wrong-structure | `mcp/params.rs` | Replace with generic `serde_json::Value` input |

---

## Feature: Ingest Tool (ADR-028, Gap 1)

Addresses: ADR-028 — replace `annotate` with `ingest(data, input_kind?)` on all transports.

### Scenario: Transport exposes ingest tool
**Given** the Plexus MCP server is started
**When** a client lists available tools
**Then** the tool list contains `ingest` with parameters `data` (required, object) and `input_kind` (optional, string)
**And** the tool list does not contain `annotate`

### Scenario: Ingest with explicit input_kind routes directly
**Given** the server with the full adapter pipeline registered
**When** a client calls `ingest(data: {"text": "hello", "tags": ["test"], "source": "trellis"}, input_kind: "content")`
**Then** the pipeline routes to ContentAdapter
**And** a fragment node is created with text "hello"
**And** a chain and mark are created (content is always fragment + provenance)
**And** the response contains outbound events

### Scenario: Ingest without input_kind triggers classification
**Given** the server with the full adapter pipeline and input classifier
**When** a client calls `ingest(data: {"text": "important pattern", "file": "src/main.rs", "line": 42, "tags": ["refactor"]})`
**Then** the input classifier resolves `input_kind` to `"content"`
**And** ContentAdapter processes the input
**And** a fragment, chain, and mark are created

### Scenario: Ingest returns error on unrecognized input shape
**Given** the server with the input classifier
**When** a client calls `ingest(data: {"unknown_field": true})` without `input_kind`
**Then** the response is an error indicating unrecognized input shape
**And** the error includes guidance on valid input shapes

---

## Feature: Input Classifier (ADR-028, Gap 4)

Addresses: ADR-028 — resolve `input_kind` from JSON structure when caller omits it.

### Scenario: Classifier detects content-shaped input
**Given** an input classifier
**When** it receives JSON `{"text": "some thought", "tags": ["idea"], "source": "trellis"}`
**Then** it returns `input_kind = "content"`

### Scenario: Classifier detects content with location
**Given** an input classifier
**When** it receives JSON `{"text": "pattern here", "line": 10, "file": "foo.rs"}`
**Then** it returns `input_kind = "content"`

### Scenario: Classifier detects file-extraction-shaped input
**Given** an input classifier
**When** it receives JSON `{"file_path": "/path/to/file.txt"}`
**Then** it returns `input_kind = "extract-file"`

### Scenario: Explicit input_kind bypasses classifier
**Given** an input classifier
**When** `input_kind = "content"` is provided by the caller alongside data
**Then** the classifier is not invoked
**And** the pipeline routes directly by the provided `input_kind`

---

## Feature: ContentAdapter — Fragment + Provenance (ADR-028, Gap 2)

Addresses: ADR-028 — content is always fragment + provenance. ContentAdapter unifies the old FragmentAdapter and `PlexusApi.annotate()` composite. All content entering the graph produces both semantic content and provenance structures (Invariant 7).

### Scenario: Content with location creates fragment, chain, and mark
**Given** the full adapter pipeline with ContentAdapter
**When** `ingest` is called with `data: {"text": "interesting pattern", "line": 15, "file": "src/lib.rs", "tags": ["architecture"], "chain_name": "field-notes"}`
**Then** a fragment node is created with text "interesting pattern" and tag "architecture"
**And** a chain node `chain:provenance:field-notes` is created (or reused if it exists)
**And** a mark node is created with file_path "src/lib.rs" and line 15
**And** a `contains` edge connects the chain to the mark

### Scenario: Content without location creates fragment with source-level provenance
**Given** the full adapter pipeline with ContentAdapter
**When** `ingest` is called with `data: {"text": "The interplay of structure and meaning", "tags": ["architecture", "semantics"], "source": "trellis"}`
**Then** a fragment node is created with the text
**And** a chain node is created (auto-generated from source)
**And** a mark node is created with source "trellis" as origin provenance
**And** a `contains` edge connects the chain to the mark
**And** TagConceptBridger creates concept nodes for "architecture" and "semantics"

### Scenario: Chain name normalization
**Given** the full adapter pipeline
**When** `ingest` is called with `data: {"text": "test", "line": 1, "file": "f.rs", "chain_name": "Field Notes"}`
**Then** the chain ID is `chain:provenance:field-notes` (lowercased, spaces to hyphens)
**And** a second call with `chain_name: "field notes"` reuses the same chain

### Scenario: Content ingest triggers enrichment loop
**Given** the full adapter pipeline with TagConceptBridger registered
**When** `ingest` is called with content data containing tags
**Then** TagConceptBridger fires after the emission
**And** concept nodes are created for each tag
**And** `tagged_with` edges connect the fragment to concepts

---

## Feature: Core Pipeline Registration (ADR-028, Gap 3)

Addresses: ADR-028 — server registers three core adapters and all core enrichments.

### Scenario: Pipeline includes three core adapters
**Given** the server is initialized
**When** the adapter pipeline is constructed
**Then** the pipeline contains ContentAdapter (for `"content"`)
**And** ExtractionCoordinator (for `"extract-file"`)
**And** SemanticAdapter instances for each registered semantic spec

### Scenario: Pipeline includes all core enrichments
**Given** the server is initialized
**When** the enrichment registry is constructed
**Then** the registry contains TagConceptBridger
**And** CoOccurrenceEnrichment
**And** EmbeddingSimilarityEnrichment
**And** DiscoveryGapEnrichment
**And** TemporalProximityEnrichment

### Scenario: File extraction reachable from transport
**Given** the server with the full pipeline
**When** a client calls `ingest(data: {"file_path": "test.txt"}, input_kind: "extract-file")`
**Then** ExtractionCoordinator processes the input
**And** Phase 1 creates a file node with metadata
**And** outbound events are returned to the client

### Scenario: Full pipeline round-trip (integration)
**Given** the server with full pipeline, backed by `PlexusEngine` with `SqliteStore`
**When** a client calls `ingest` with content data (text + tags + source)
**Then** ContentAdapter processes it producing fragment + provenance
**And** enrichments fire (TagConceptBridger at minimum)
**And** all emissions are persisted to the store
**And** a new engine loaded from the same store contains the fragment, chain, mark, and concept nodes

---

## Feature: JSON Wire Format (ADR-028, Gap 5)

Addresses: ADR-028 — all transport-facing input arrives as `serde_json::Value`.

### Scenario: Ingest accepts raw JSON
**Given** the server
**When** a client calls `ingest` with a JSON object in the `data` field
**Then** the data is deserialized as `serde_json::Value`
**And** the input classifier or explicit `input_kind` determines routing

### Scenario: ContentAdapter accepts JSON input
**Given** a ContentAdapter in the pipeline
**When** it receives a `serde_json::Value` containing `{"text": "hello", "tags": ["a", "b"], "source": "trellis"}`
**Then** it constructs typed input from the JSON fields
**And** produces fragment + provenance (chain + mark + contains)

### Scenario: ExtractionCoordinator accepts JSON input
**Given** an ExtractionCoordinator in the pipeline
**When** it receives a `serde_json::Value` containing `{"file_path": "/path/to/file.txt"}`
**Then** it constructs an `ExtractFileInput` from the JSON fields
**And** proceeds with Phase 1 extraction

---

## Feature: Layered Provenance (ADR-028)

Addresses: ADR-028 — caller always provides origin provenance; pipeline adds structural provenance. Content is always fragment + provenance.

### Scenario: Location-specific provenance (Carrel annotation)
**Given** the full adapter pipeline
**When** `ingest` is called with `data: {"text": "pattern here", "line": 42, "file": "src/main.rs", "chain_name": "review"}`
**Then** origin provenance: file "src/main.rs" (caller-provided)
**And** location provenance: mark at line 42 (caller-provided)
**And** chain `chain:provenance:review` contains the mark
**And** fragment + provenance both created (Invariant 7)

### Scenario: Source-level provenance (Trellis text)
**Given** the full adapter pipeline
**When** `ingest` is called with `data: {"text": "a thought about code", "tags": ["reflection"], "source": "trellis"}`
**Then** origin provenance: source "trellis" (caller-provided)
**And** a chain and mark are created with source-level granularity (no file/line)
**And** fragment + provenance both created (Invariant 7)

### Scenario: Pipeline-derived structural provenance (file extraction)
**Given** the full adapter pipeline with ExtractionCoordinator and SemanticAdapter
**When** `ingest` is called with `data: {"file_path": "macbeth.txt"}, input_kind: "extract-file"`
**And** Phase 3 SemanticAdapter discovers concepts at specific locations
**Then** Phase 1 creates a file node as the structural origin record (Layer 1)
**And** Phase 3 creates provenance-dimension chains and marks tracing where concepts were found (Layer 2, pipeline-derived)
**And** the caller provided only the file path; the pipeline derived all internal structure

---

## Feature: SemanticAdapter Spec Registration (ADR-028)

Addresses: ADR-028 — SemanticAdapter specs are registered YAML files declaring ensemble + response mapping.

### Scenario: Registered spec declares input_kind and ensemble
**Given** a YAML semantic adapter spec with `input_kind: "sketchbin-asset"` and `ensemble: "sketchbin-extraction"`
**When** the spec is registered with Plexus
**Then** the pipeline contains a SemanticAdapter instance for `input_kind = "sketchbin-asset"`
**And** the spec was validated at registration time

### Scenario: Input classifier routes to registered spec
**Given** a registered semantic adapter spec with `input_kind: "sketchbin-asset"`
**When** a client calls `ingest(data: {"asset_id": "123", "content": "..."})` without `input_kind`
**And** the JSON matches the spec's expected input shape
**Then** the input classifier routes to the `"sketchbin-asset"` SemanticAdapter instance

### Scenario: Registration rejects malformed spec
**Given** a YAML semantic adapter spec with an invalid template expression `"{input.}"`
**When** the spec is registered with Plexus
**Then** registration fails with a validation error describing the template syntax issue
**And** no SemanticAdapter instance is created

### Scenario: Registered spec with ensemble integration (integration)
**Given** a registered semantic adapter spec for a domain (e.g., Sketchbin assets)
**And** the spec declares an llm-orc ensemble for extraction
**When** `ingest` is called with domain-specific JSON matching the spec's input shape
**Then** SemanticAdapter processes the input using the registered spec
**And** the ensemble runs for semantic extraction
**And** origin provenance is captured from the caller's input
**And** structural provenance is derived from extraction results
