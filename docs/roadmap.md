# Roadmap: Plexus

**Last updated:** 2026-03-17
**Derived from:** System Design v1.0, ADR-029, Essay 26, conformance audit

## Current Work Packages

### WP-6: End-to-end pipeline integration testing

**Objective:** Verify the full pipeline works end-to-end through the MCP transport layer — not just at the pipeline level. Close the MCP-layer test gaps identified in the conformance audit.

**Changes:**
- Test MCP `ingest` tool handler directly (transport exposes ingest, no `annotate`)
- Test explicit `input_kind` routing through MCP boundary
- Test classification dispatch (no `input_kind` → classifier → pipeline)
- Test error propagation from classifier through MCP response
- Test Phase 3 provenance chains from file extraction (structural provenance)
- Verify `EmbeddingSimilarityEnrichment` integration behind `embeddings` feature flag

**Scenarios covered:**
- Transport exposes ingest tool (scenarios.md)
- Ingest with explicit input_kind routes directly
- Ingest without input_kind triggers classification
- Ingest returns error on unrecognized input shape
- Pipeline-derived structural provenance (file extraction)

**Dependencies:** None — all infrastructure exists

**Open questions:**
- MCP server testability: does `rmcp` support in-process testing, or do tests need to exercise `PlexusApi` at the transport boundary?
- ExtractionCoordinator Phase 2/3 test strategy: mock llm-orc subprocess or test with real local model?

---

## Open Decision Points

- **SemanticAdapter / DeclarativeAdapter convergence** — ADR-028 says they merge. They haven't. Keep dual existence? Execute convergence? Rename to clarify?
- **Phase 2 extraction** — No adapter exists post-ADR-029. Rebuild? Collapse to 2 phases? Defer until concrete use case?
- **Pipeline construction location** — Currently in `PipelineBuilder`. Consider extracting to transport-neutral binary entry point.
- **Batched normalization / persist** — O(edges × adapters) per emission scaling concern. Requires profiling data to justify.
- **Context field encapsulation** — `Context.nodes` and `Context.edges` are public fields. Enable future `Vec<Edge>` → `HashMap` migration.

---

## Completed Work Log

### Cycle: Architectural Consolidation (2026-03-16 — 2026-03-17)

**Derived from:** ADR-029, Essay 26

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-1 | Decompose adapter/ into submodules | `b94bbc0`, `c466f33`, `ef261c8` | Done |
| WP-2 | Extract pipeline builder from MCP | `37d789e` | Done |
| WP-3 | Remove enrichment from EngineSink (ADR-029 D2) | `19214ba` | Done |
| WP-4 | Remaining ADR-029 cleanup | (prior session commits) | Done |
| WP-5 | Note open questions in domain model | (prior session commits) | Done |

**Post-build:**
- TagConceptBridger removed entirely (`feb6499`) — tag bridging is domain-specific
- Documentation drift fixed across 36+ files (`fc69209`, `d6586b7`)
- `get_links` returns `Vec<MarkView>`, empty chain name validation added (`3e69a16`)
- 2 new integration tests (`a8f5cf5`)

**Transition state achieved:** TS-3 (Fully consolidated) — ADR-029 fully implemented, EngineSink is purely commit+persist, PipelineBuilder owns construction, MCP is a thin shell.

**Final state:** 364 lib tests, clippy clean, all conformance drift addressed.

#### Original Dependency Graph

```
WP-5 ─────────────────────────── (open choice, independent)

WP-1 ◄──── WP-2 (implied logic)
  ▲
  │
  └──── WP-3 (implied logic)
           │
           └──── WP-4 (hard dependency)
```
