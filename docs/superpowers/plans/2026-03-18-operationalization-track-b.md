# Track B: Operationalization Implementation Plan

**Goal:** Make Plexus production-ready for Trellis integration: clean .llm-orc to production essentials, establish contract-based acceptance tests, and graduate the research corpus.

**Architecture:** Three sequential work packages (WP-B1 → WP-B2 → WP-B3). Tests move from the monolithic `src/adapter/integration_tests.rs` (4500 lines) into focused test modules under `tests/acceptance/`. All necessary types are publicly exported — tests use the crate's public API via `plexus::*`.

**Execution methodology:** `/rdd-build` — behavior scenarios drive acceptance tests via TDD (red/green/refactor). Structure changes separated from behavior changes. Small reversible commits.

**Design spec:** `docs/superpowers/specs/2026-03-17-operationalization-design.md`

**Scope:** Track B only. Track A (Phase 2 RDD) runs separately via rdd-model/decide/architect/build. WP-B4 (Tier 2 tests) is planned after Track A completes.

---

## File Structure

### WP-B1: .llm-orc cleanup

```
.llm-orc/
├── ensembles/
│   ├── extract-semantic.yaml          (KEEP — canonical production ensemble)
│   └── archive/                       (CREATE — move non-production ensembles here)
│       ├── plexus-semantic.yaml
│       ├── code-concepts.yaml
│       ├── graph-analysis.yaml
│       └── research/                  (MOVE from ensembles/research/)
├── profiles/
│   ├── analyst-mistral.yaml           (KEEP — used by extract-semantic)
│   └── archive/                       (CREATE — move unused profiles here)
│       ├── analyst-qwen.yaml
│       ├── ollama-gemma3-1b.yaml
│       ├── ollama-llama3.yaml
│       └── synthesizer-local.yaml
├── scripts/
│   └── extraction/
│       ├── spacy-extract.py           (KEEP — used by extract-semantic)
│       └── archive/                   (CREATE — move non-production scripts here)
│           └── textrank-extract.py
└── artifacts/                         (DELETE contents — transient execution history)
```

Non-production scripts (`scripts/chunker.sh`, `scripts/specialized/`) move to archive within their parent directory.

### WP-B2: Acceptance tests

```
tests/
├── acceptance.rs                      (CREATE — integration test entry point)
├── acceptance/
│   ├── mod.rs                         (CREATE — declares submodules)
│   ├── helpers.rs                     (CREATE — shared test utilities)
│   ├── ingest.rs                      (CREATE — ingest contract tests)
│   ├── extraction.rs                  (CREATE — extraction pipeline tests)
│   ├── enrichment.rs                  (CREATE — enrichment loop tests)
│   ├── provenance.rs                  (CREATE — chain/mark lifecycle tests)
│   ├── contribution.rs                (CREATE — per-adapter tracking tests)
│   ├── persistence.rs                 (CREATE — save/load roundtrip tests)
│   ├── degradation.rs                 (CREATE — graceful skip tests)
│   └── query.rs                       (CREATE — find/traverse/path tests)
└── fixtures/
    ├── simple.md                      (CREATE — plain markdown, no frontmatter)
    ├── frontmatter.md                 (CREATE — markdown with tags/title frontmatter)
    ├── code_sample.rs                 (CREATE — Rust source file for code extraction)
    └── plain.txt                      (CREATE — plain text, no structure)

src/adapter/mod.rs                     (MODIFY — remove integration_tests module)
src/adapter/integration_tests.rs       (DELETE — after migration complete)
```

---

## WP-B1: .llm-orc Cleanup

### Task 1: Archive non-production ensembles

**Files:**
- Create: `.llm-orc/ensembles/archive/`
- Move: `plexus-semantic.yaml`, `code-concepts.yaml`, `graph-analysis.yaml` → archive/
- Move: `ensembles/research/` → `ensembles/archive/research/`

Steps:
- [ ] Create archive directory and move non-production ensembles
- [ ] Verify `extract-semantic.yaml` is the sole production ensemble
- [ ] Commit: `chore: archive non-production ensembles, extract-semantic is canonical`

### Task 2: Archive non-production profiles and scripts

**Files:**
- Create: `.llm-orc/profiles/archive/`, `.llm-orc/scripts/archive/`
- Move: unused profiles (`analyst-qwen`, `ollama-gemma3-1b`, `ollama-llama3`, `synthesizer-local`) to archive
- Move: `textrank-extract.py`, `chunker.sh`, `specialized/` to archive

Steps:
- [ ] Archive unused profiles (extract-semantic only uses `analyst-mistral`)
- [ ] Archive non-production scripts (only `spacy-extract.py` is production)
- [ ] Verify production-only structure: one ensemble, one profile, one script
- [ ] Commit: `chore: archive non-production profiles and scripts`

### Task 3: Delete transient artifacts

**Files:**
- Delete: `.llm-orc/artifacts/` contents
- Create: `.llm-orc/artifacts/.gitkeep`

Steps:
- [ ] Delete all artifact execution history (transient llm-orc outputs)
- [ ] Add `.gitkeep` to preserve directory
- [ ] Commit: `chore: delete transient llm-orc artifacts`

---

## WP-B2: Tier 1 Acceptance Tests

### Task 4: Create acceptance test infrastructure

**Files:**
- Create: `tests/acceptance.rs`, `tests/acceptance/mod.rs`, `tests/acceptance/helpers.rs`
- Create: `tests/fixtures/simple.md`, `tests/fixtures/frontmatter.md`, `tests/fixtures/code_sample.rs`, `tests/fixtures/plain.txt`

**Key types available from public API:**
- `plexus::PlexusApi`, `plexus::PlexusEngine`, `plexus::Context`, `plexus::ContextId`
- `plexus::adapter::{PipelineBuilder, EngineSink, FrameworkContext, IngestPipeline}`
- `plexus::adapter::{ContentAdapter, ExtractionCoordinator, ProvenanceAdapter}`
- `plexus::adapter::{CoOccurrenceEnrichment, EnrichmentRegistry}`
- `plexus::llm_orc::MockClient`
- `plexus::{SqliteStore, OpenStore}`

**Helpers needed:**
- `TestEnv` struct: wires `PlexusEngine` + `IngestPipeline` + `PlexusApi` with in-memory SQLite and mock llm-orc
- `TestEnv::new()` — default env with `MockClient::unavailable()` (Phase 3 skips gracefully)
- `TestEnv::with_mock_client(client)` — env with custom mock responses
- `TestEnv::fixture(name)` — path to a fixture file
- `TestEnv::fixture_content(name)` — read fixture as string

Steps:
- [ ] Create fixture documents (markdown with/without frontmatter, code sample, plain text)
- [ ] Create test entry point (`tests/acceptance.rs`) and module structure
- [ ] Create shared test helpers (`TestEnv`)
- [ ] Verify infrastructure compiles: `cargo test --test acceptance --no-run`
- [ ] Commit: `test: acceptance test infrastructure and fixtures`

### Task 5: Ingest contract tests

**Scenarios (from domain invariants):**
- Ingesting text content routes to ContentAdapter and produces concept nodes
- Ingesting with explicit input_kind routes directly to matching adapter
- Ingesting frontmatter-bearing markdown produces tagged_with edges from file to concepts
- Ingest returns outbound events describing graph mutations

**Files:** `tests/acceptance/ingest.rs`

Steps:
- [ ] Write failing tests for each scenario
- [ ] Run tests, verify they fail for the right reason (red)
- [ ] Fix any compilation/wiring issues in helpers.rs (green)
- [ ] Commit: `test: ingest contract acceptance tests`

### Task 6: Extraction contract tests

**Scenarios:**
- Phase 1 file extraction creates file node with MIME type and size
- Phase 1 creates extraction status node tracking phase completion
- Phase 3 with mock ensemble produces concept nodes and relationships
- Multi-agent ensemble results all parsed (multi-run union, Inv 45)

**Files:** `tests/acceptance/extraction.rs`

Steps:
- [ ] Write failing tests — Phase 3 tests need `TestEnv::with_mock_client` wiring SemanticAdapter
- [ ] Run tests, iterate on wiring (may need to extend helpers for ExtractionCoordinator + SemanticAdapter registration)
- [ ] Commit: `test: extraction contract acceptance tests`

### Task 7: Enrichment contract tests

**Scenarios:**
- Co-occurrence enrichment creates may_be_related edges between concepts sharing a source
- Enrichment loop quiesces (does not run forever — implicit timeout assertion)

**Files:** `tests/acceptance/enrichment.rs`

Steps:
- [ ] Write failing tests
- [ ] Run and iterate
- [ ] Commit: `test: enrichment contract acceptance tests`

### Task 8: Provenance contract tests

**Scenarios (from domain model — chain/mark lifecycle):**
- Creating a chain via API produces a queryable chain
- Adding a mark to a chain creates the mark and a contains edge
- Linking two marks creates bidirectional reference edges

**Files:** `tests/acceptance/provenance.rs`

Steps:
- [ ] Write failing tests — verify ProvenanceAdapter JSON input format against `src/adapter/adapters/provenance_adapter.rs`
- [ ] Run and iterate
- [ ] Commit: `test: provenance contract acceptance tests`

### Task 9: Contribution contract tests

**Scenarios (Inv 45, ADR-003):**
- Edges carry per-adapter contribution keys after ingest
- Retracting an adapter's contributions removes its edges

**Files:** `tests/acceptance/contribution.rs`

Steps:
- [ ] Write failing tests
- [ ] Run and iterate
- [ ] Commit: `test: contribution contract acceptance tests`

### Task 10: Persistence contract tests

**Scenarios (Inv 30 — persist-per-emission):**
- Nodes and edges survive engine reload from SQLite store
- Edge weights and contributions survive reload

**Files:** `tests/acceptance/persistence.rs`

Steps:
- [ ] Write failing tests — use two PlexusEngine instances sharing one SqliteStore
- [ ] Run and iterate
- [ ] Commit: `test: persistence contract acceptance tests`

### Task 11: Degradation contract tests

**Scenarios (Inv 47 — graceful degradation):**
- File extraction succeeds even when llm-orc is unavailable (Phase 1 completes, Phase 3 skipped)
- Text ingest succeeds without llm-orc

**Files:** `tests/acceptance/degradation.rs`

Steps:
- [ ] Write failing tests — default TestEnv uses MockClient::unavailable()
- [ ] Run and iterate
- [ ] Commit: `test: degradation contract acceptance tests`

### Task 12: Query contract tests

**Scenarios:**
- FindQuery returns nodes matching type filter
- TraverseQuery follows edges from a starting node
- Evidence trail returns provenance for a concept

**Files:** `tests/acceptance/query.rs`

Steps:
- [ ] Write failing tests — verify FindQuery/TraverseQuery constructor API against `src/query/`
- [ ] Run and iterate
- [ ] Commit: `test: query contract acceptance tests`

### Task 13: Migrate and remove integration_tests.rs

**Files:**
- Modify: `src/adapter/mod.rs` — remove `#[cfg(test)] mod integration_tests;`
- Delete: `src/adapter/integration_tests.rs`

Steps:
- [ ] Verify all acceptance tests pass: `cargo test --test acceptance`
- [ ] Audit `integration_tests.rs` for scenarios not covered by acceptance suite
- [ ] Move any uncovered contract-level tests to appropriate acceptance module
- [ ] Move any internal-behavior tests to unit test blocks within their source files
- [ ] Remove `integration_tests` module declaration from `src/adapter/mod.rs`
- [ ] Delete `src/adapter/integration_tests.rs`
- [ ] Verify all tests pass: `cargo test`
- [ ] Commit: `refactor: remove monolithic integration_tests, replaced by acceptance suite`

---

## WP-B3: Research Graduation

### Task 14: Run rdd-conform and fix drift

Steps:
- [ ] Invoke `/rdd-conform` against operational docs (ADRs, system design, domain model, field guide, scenarios)
- [ ] Document drift findings
- [ ] Fix each drift item — commit separately with `docs:` prefix

### Task 15: Archive research corpus

**Files:**
- Move: `docs/essays/` → `docs/archive/essays/`
- Modify: `ORIENTATION.md`

Steps:
- [ ] Create `docs/archive/` and move essays and research logs
- [ ] Update ORIENTATION.md to reflect post-archive structure
- [ ] Verify no broken cross-references in non-archived docs
- [ ] Commit: `docs: graduate research corpus to archive, verify operational docs`

### Task 16: Update roadmap

**Files:** `docs/roadmap.md`

Steps:
- [ ] Update WP-B1 through WP-B3 status to Done with commit references
- [ ] Note WP-B4 awaits Track A completion
- [ ] Commit: `docs: update roadmap with Track B completion status`
