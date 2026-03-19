# Behavior Scenarios: Structural Module System

ADRs: 030 (structural module trait), 031 (structural output and handoff), 032 (first structural module — markdown)

---

## Feature: Structural Module Trait and Dispatch (ADR-030)

### Scenario: Structural module registers on extraction coordinator
**Given** an `ExtractionCoordinator` with no modules registered
**When** `register_module(Arc::new(MarkdownStructureModule::new()))` is called
**Then** the coordinator's module registry contains one module
**And** calling `matching_modules("text/markdown")` returns that module

### Scenario: MIME dispatch is fan-out — all matching modules execute
**Given** an extraction coordinator with two modules registered:
  - Module A with MIME affinity `text/` (matches all text types)
  - Module B with MIME affinity `text/markdown` (matches only markdown)
**When** a `text/markdown` file is processed
**Then** both Module A and Module B execute (fan-out)
**And** both modules' outputs are included in the merged structural output

### Scenario: MIME dispatch — no match produces empty structural output
**Given** an extraction coordinator with one module registered for MIME affinity `text/`
**When** an `audio/mpeg` file is processed
**Then** no modules match
**And** structural analysis produces `StructuralOutput::default()` (empty)
**And** semantic extraction still runs with no sections and no vocabulary

### Scenario: Empty module registry is a passthrough
**Given** an extraction coordinator with no modules registered
**When** any file is processed
**Then** structural analysis is skipped entirely
**And** semantic extraction receives an empty `SemanticInput` (no sections, no vocabulary)
**And** semantic extraction chunks the whole file using its default strategy

### Scenario: Structural module uses MIME dispatch, not input-kind routing
**Given** a type implementing `StructuralModule`
**Then** it is dispatched by the extraction coordinator via MIME type affinity
**And** it is not registered on `IngestPipeline` for input-kind routing
**And** it is invoked via `analyze(file_path, content)` and returns `StructuralOutput`
**And** whether it also implements `Adapter` is a BUILD-time implementation decision

### Scenario: Module receives file content from coordinator — no self-reads
**Given** a structural module registered on the coordinator
**When** a file is processed through extraction
**Then** the coordinator reads the file content once
**And** passes the content string to `module.analyze(file_path, content)`
**And** the module does not perform any file I/O

### Scenario: Coordinator emits on behalf of module with module's adapter ID
**Given** a structural module with `id()` returning `"extract-analysis-text-headings"`
**And** the module's `analyze()` returns a `StructuralOutput` with one `ModuleEmission` containing a concept node
**When** the coordinator processes the structural analysis output
**Then** the concept node is emitted to the graph with adapter ID `"extract-analysis-text-headings"`
**And** any contribution tracking uses the module's ID, not the coordinator's ID

---

## Feature: Structural Output Merge and Handoff (ADR-031)

### Scenario: Single module output passes through without merge
**Given** one structural module matches the file
**When** the module returns sections `[("## Intro", 1, 10), ("## Body", 11, 30)]` and vocabulary `["plexus", "graph"]`
**Then** the merged `StructuralOutput` contains those same sections and vocabulary unchanged

### Scenario: Multiple module outputs are merged
**Given** Module A returns sections `[("## Intro", 1, 10)]` and vocabulary `["plexus"]`
**And** Module B returns sections `[("fn main", 15, 25)]` and vocabulary `["Plexus", "graph"]`
**When** outputs are merged
**Then** sections are `[("## Intro", 1, 10), ("fn main", 15, 25)]` (sorted by start_line)
**And** vocabulary is `["plexus", "graph"]` (deduplicated case-insensitively, 2 terms not 3)

### Scenario: Empty module output merges cleanly
**Given** Module A returns an empty `StructuralOutput` (no sections, no vocabulary)
**And** Module B returns sections and vocabulary
**When** outputs are merged
**Then** the merged output contains only Module B's contributions
**And** no error or warning from Module A's empty output

### Scenario: All modules return empty — merged output is default
**Given** two modules match the file
**And** both return `StructuralOutput::default()`
**When** outputs are merged
**Then** the merged output is `StructuralOutput::default()`
**And** semantic extraction runs with its default chunking strategy, no vocabulary hints

### Scenario: Structural output hands off vocabulary to semantic extraction
**Given** structural analysis produces merged output with vocabulary `["plexus", "trellis"]` and sections `[("## Architecture", 1, 50)]`
**When** the coordinator constructs `SemanticInput` for semantic extraction
**Then** `SemanticInput.sections` contains one `SectionBoundary { label: "## Architecture", start_line: 1, end_line: 50 }`
**And** `SemanticInput.vocabulary` contains `["plexus", "trellis"]`

### Scenario: Semantic extraction chunks with default strategy when no sections
**Given** structural analysis produces merged output with vocabulary `["plexus"]` but empty sections
**When** semantic extraction processes the file
**Then** semantic extraction uses its default chunking strategy
**And** each chunk is processed independently
**And** concepts found in multiple chunks reinforce through contribution tracking
**And** where chunking happens (Rust or llm-orc script agent) is a BUILD-time decision

### Scenario: Vocabulary bootstrap does NOT include relationships
**Given** a structural module discovers that "plexus" and "trellis" appear in the same heading
**When** the module produces output
**Then** the vocabulary contains `["plexus", "trellis"]` (entity names)
**And** the vocabulary does NOT contain relationship information (e.g., no "plexus-related-to-trellis")
**And** any relationship the module found is emitted to the graph (via `ModuleEmission`), not forwarded to semantic extraction

### Scenario: SemanticInput serializes vocabulary to llm-orc JSON
**Given** a `SemanticInput` with vocabulary `["plexus", "trellis"]` and one section
**When** `build_input()` serializes the input for llm-orc
**Then** the JSON payload contains a `"vocabulary"` field with `["plexus", "trellis"]`
**And** the JSON payload contains a `"sections"` field with the section boundary

---

## Feature: Markdown Structure Module (ADR-032)

### Scenario: Markdown heading extraction produces sections
**Given** a markdown file with content:
  ```markdown
  # Title
  Introduction paragraph.
  ## Architecture
  Architecture details.
  ## Testing
  Test details.
  ```
**When** `MarkdownStructureModule.analyze(path, content)` is called
**Then** the output contains three sections:
  - `("# Title", 1, 2)` — from `# Title` to before `## Architecture`
  - `("## Architecture", 3, 4)` — from `## Architecture` to before `## Testing`
  - `("## Testing", 5, 6)` — from `## Testing` to end of file

### Scenario: Markdown link extraction produces vocabulary
**Given** a markdown file containing `[Plexus](https://example.com)` and `[knowledge graph](./docs/kg.md)`
**When** `MarkdownStructureModule.analyze(path, content)` is called
**Then** the vocabulary contains `"plexus"` and `"knowledge graph"` (link display text, lowercased)

### Scenario: Markdown heading text contributes to vocabulary
**Given** a markdown file with heading `## Extraction Architecture`
**When** `MarkdownStructureModule.analyze(path, content)` is called
**Then** the vocabulary contains `"extraction architecture"` (heading text, lowercased, formatting stripped)

### Scenario: Markdown module MIME affinity matches markdown only
**Given** a `MarkdownStructureModule`
**Then** `mime_affinity()` returns `"text/markdown"`
**And** a file with MIME type `text/markdown` matches
**And** a file with MIME type `text/plain` does NOT match

### Scenario: Markdown module may produce graph emissions
**Given** a markdown file with headings and links
**When** `MarkdownStructureModule.analyze(path, content)` is called
**Then** `output.emissions` contains whatever the module determined to be graph-worthy (may be empty, may contain concept nodes from headings or link targets)
**And** if emissions are present, each has a `module_id` matching the module's `id()`

### Scenario: Markdown module handles file with no structure gracefully
**Given** a markdown file with no headings and no links (just paragraphs of text)
**When** `MarkdownStructureModule.analyze(path, content)` is called
**Then** the output has zero sections
**And** the vocabulary may contain terms from paragraph text (or be empty)
**And** no error is raised

### Scenario: Non-markdown file is not processed by markdown module
**Given** an extraction coordinator with `MarkdownStructureModule` registered
**When** a `.rs` file with MIME type `text/x-rust` is processed
**Then** `MarkdownStructureModule` does not execute (MIME affinity `text/markdown` does not match `text/x-rust`)
**And** structural analysis produces empty output for this file

---

## Feature: Integration — Full Pipeline (ADR-030 + 031 + 032)

### Scenario: End-to-end extraction with structural analysis
**Given** a `PlexusEngine` with `IngestPipeline` wired with:
  - `ExtractionCoordinator` with `MarkdownStructureModule` registered
  - `SemanticAdapter` registered for semantic extraction (with mock llm-orc)
  - `CoOccurrenceEnrichment` registered
**When** `ingest("extract-file", { "file_path": "docs/example.md" })` is called on a markdown file with headings and links
**Then** registration completes synchronously (file node + frontmatter concepts)
**And** structural analysis runs as a background task:
  - `MarkdownStructureModule` produces sections from headings and vocabulary from links
  - Merged structural output feeds `SemanticInput` with sections and vocabulary
**And** semantic extraction runs after structural analysis:
  - `SemanticAdapter` receives `SemanticInput` with populated sections and vocabulary
  - LLM agents receive vocabulary hints in their input
**And** enrichments fire incrementally after each phase's emission

### Scenario: PipelineBuilder wires default structural module
**Given** a `PipelineBuilder` with default adapters
**When** the pipeline is constructed
**Then** the `ExtractionCoordinator` has `MarkdownStructureModule` registered
**And** markdown files trigger structural analysis
**And** non-markdown files pass through to semantic extraction without structural context

### Scenario: Structural analysis failure does not block semantic extraction
**Given** an extraction coordinator with a structural module that panics on certain input
**When** the module panics during `analyze()`
**Then** structural analysis for that module is treated as returning empty output
**And** other matching modules (if any) still run
**And** semantic extraction proceeds with whatever structural output was collected
**And** registration results are unaffected (already persisted)

---

## Conformance Debt

| # | ADR | What exists | What ADR prescribes | Type | Location | Resolution |
|---|-----|-------------|---------------------|------|----------|------------|
| 1 | 030 | `Phase2Registration` struct | Removed | exists | extraction.rs:49-54 | Delete struct, replace usages |
| 2 | 030 | `phase2_adapters: Vec<Phase2Registration>` | `modules: Vec<Arc<dyn StructuralModule>>` | exists | extraction.rs:59 | Replace field |
| 3 | 030 | `register_phase2(mime_prefix, adapter)` | `register_module(module)` | exists | extraction.rs:117-126 | Replace method |
| 4 | 030 | `find_phase2_adapter()` — find-first | `matching_modules()` — fan-out | wrong-structure | extraction.rs:137-142 | Rewrite as filter/collect |
| 5 | 030 | No `StructuralModule` trait | Async trait with `id`, `mime_affinity`, `analyze` | missing | — | Create in adapter module |
| 6 | 030 | Phase 2 dispatch via `adapter.process()` | Coordinator reads file, calls `module.analyze()` | wrong-structure | extraction.rs:452-494 | Rewrite dispatch |
| 7 | 031 | `SemanticInput` lacks `vocabulary` field | Add `vocabulary: Vec<String>` | wrong-structure | semantic.rs:29-34 | Add field |
| 8 | 031 | No `with_structural_context()` constructor | Constructor taking sections + vocabulary | missing | semantic.rs | Add constructor |
| 9 | 031 | `SemanticInput::for_file()` with no context | `with_structural_context(path, sections, vocabulary)` | wrong-structure | extraction.rs:523 | Replace call |
| 10 | 031 | `StructuralOutput`, `ModuleEmission` types absent | Both types defined | missing | — | Create in adapter module |
| 11 | 032 | No `pulldown-cmark` dependency | Add to Cargo.toml | missing | Cargo.toml | Add dependency |
| 12 | 032 | No `MarkdownStructureModule` | Full module implementation | missing | — | Create module |
| 13 | 032 | No `with_structural_module()` on PipelineBuilder | Registration method + default wiring | missing | builder.rs | Add method |
