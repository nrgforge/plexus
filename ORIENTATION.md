# ORIENTATION

**Project:** Plexus — knowledge graph engine with provenance tracking
**Current phase:** BUILD complete (architectural consolidation)
**Last updated:** 2026-03-16

## What This Project Is

Plexus is a Rust library and MCP server for building knowledge graphs with provenance tracking. It transforms domain-specific input (text fragments, files, LLM extraction results, YAML specs) into a weighted graph with per-adapter contribution tracking and reactive enrichment. All knowledge carries both semantic content and provenance (Invariant 7).

## Where to Start

| You want to... | Start here |
|---|---|
| Understand the architecture | [`docs/system-design.md`](docs/system-design.md) |
| Find where something lives in code | [`docs/references/field-guide.md`](docs/references/field-guide.md) |
| Understand a design decision | [`docs/adr/`](docs/adr/) (29 ADRs, 000-029) |
| Read the domain vocabulary | [`docs/domain-model.md`](docs/domain-model.md) |
| See behavior scenarios | [`docs/scenarios.md`](docs/scenarios.md) |
| Understand the build roadmap | [`docs/roadmap.md`](docs/roadmap.md) |
| Read the research trail | [`docs/essays/`](docs/essays/) (26 essays) |

## Artifact Map

```
ORIENTATION.md          ← you are here
docs/
├── system-design.md    ← module decomposition, dependency graph, pipeline flow
├── domain-model.md     ← glossary, invariants, relationships
├── scenarios.md        ← behavior scenarios (Given/When/Then)
├── roadmap.md          ← work packages, dependencies, transition states
├── product-discovery.md ← stakeholder map, jobs, value tensions
├── adr/                ← 29 architecture decision records (000-029)
├── essays/             ← 26 research essays
├── references/
│   └── field-guide.md  ← module-to-code mapping, design rationale
└── logs/               ← archived research logs
```

## Current State

All five work packages from the architectural consolidation roadmap are complete:

- **WP-1:** adapter/ decomposed into 5 submodules (sink, enrichment, pipeline, adapters, enrichments)
- **WP-2:** PipelineBuilder extracted from MCP — transport-neutral pipeline construction
- **WP-3:** Enrichment removed from EngineSink (confirmed already done per ADR-029)
- **WP-4:** ADR-029 dead code cleanup (PhaseStatus, pulldown-cmark, parse_response, apply_mutation removed)
- **WP-5:** Domain model open questions resolved

373 library tests passing. No known regressions.

## Key Architectural Decisions

| ADR | Decision | Status |
|-----|----------|--------|
| ADR-001 | Sink-based progressive emission | Accepted, implemented |
| ADR-003 | Per-adapter contribution tracking with scale normalization | Accepted, implemented |
| ADR-006 | In-memory DashMap cache with optional persistence | Accepted, implemented |
| ADR-010 | Enrichment trait + reactive loop (max 10 rounds) | Accepted, implemented |
| ADR-012 | Single write path via IngestPipeline | Accepted, implemented |
| ADR-014 | Transport-independent PlexusApi facade | Accepted, implemented |
| ADR-028 | Universal MCP ingest + declarative adapter specs | Accepted, implemented |
| ADR-029 | Architectural consolidation decisions | Accepted, in progress |

## Open Questions

1. **SemanticAdapter / DeclarativeAdapter convergence** — ADR-028 planned convergence is deferred. Both exist as independent types.
2. **Phase 2 extraction** — No adapter exists since ADR-029 removed TextAnalysisAdapter. Rebuild or collapse to 2 phases?
3. **Batched normalization** — O(edges x adapters) per emission. Scaling concern for large graphs.
