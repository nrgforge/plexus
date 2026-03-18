# ORIENTATION

**Project:** Plexus — knowledge graph engine with self-reinforcing edges
**Current phase:** Operationalization — Track B complete, Track A pending
**Last updated:** 2026-03-18

## What This Project Is

Plexus is a knowledge graph engine that derives structure from domain-specific input, strengthens relationships through repeated co-occurrence, and tracks provenance for all knowledge. Consumer applications — creative writing scaffolding, interactive performance, research coordination, code analysis — plug in via adapters and Plexus handles graph mechanics: contribution tracking, enrichment, normalization, and persistence.

The core value proposition is cross-domain convergence. Multiple applications can write into a shared context, and because concept identity is deterministic, the graph surfaces connections that would remain latent within any single application. A concept that appears in both a research tool and a writing tool gets reinforced by both — the structure that emerges from the intersection is genuinely novel.

It ships as an embeddable Rust library and an MCP server.

## Where to Start

| You want to... | Start here |
|---|---|
| Understand the architecture | [`docs/system-design.md`](docs/system-design.md) |
| Find where something lives in code | [`docs/references/field-guide.md`](docs/references/field-guide.md) |
| Understand a design decision | [`docs/decisions/`](docs/decisions/) (30 ADRs, 000-029) |
| Read the domain vocabulary | [`docs/domain-model.md`](docs/domain-model.md) |
| See behavior scenarios | [`docs/scenarios.md`](docs/scenarios.md) |
| Understand the build roadmap | [`docs/roadmap.md`](docs/roadmap.md) |
| Read the research trail | [`docs/archive/essays/`](docs/archive/essays/) (26 essays) |

## Artifact Map

```
ORIENTATION.md          ← you are here
docs/
├── system-design.md    ← module decomposition, dependency graph, pipeline flow
├── domain-model.md     ← glossary, invariants, relationships
├── scenarios.md        ← table of contents for scenario suites
├── roadmap.md          ← work packages, dependencies, transition states
├── product-discovery.md ← stakeholder map, jobs, value tensions
├── decisions/          ← 30 architecture decision records (000-029)
├── archive/
│   └── essays/         ← 26 research essays (graduated)
│       └── research-logs/ ← per-essay research process logs (21 logs)
├── papers/             ← publication drafts (plexus-design, semantic-extraction)
├── schemas/            ← JSON schemas (phase2-output, phase3-result, graph-export, analysis-result)
├── references/
│   ├── field-guide.md  ← module-to-code mapping, design rationale
│   ├── experiment-data/ ← raw evidence trail for paper claims
│   └── gold-standards/  ← measurement baselines for extraction evaluation
├── superpowers/        ← design specs and implementation plans
│   ├── specs/          ← design documents
│   └── plans/          ← implementation plans
└── scenarios/          ← 11 scenario suite files (Given/When/Then)
tests/
├── acceptance.rs       ← acceptance test entry point (25 tests, 8 contract areas)
├── acceptance/         ← contract tests by area (ingest, extraction, enrichment, etc.)
└── fixtures/           ← test fixture files
```

## Current State

All five work packages from the architectural consolidation roadmap are complete:

- **WP-1:** adapter/ decomposed into 5 submodules (sink, enrichment, pipeline, adapters, enrichments)
- **WP-2:** PipelineBuilder extracted from MCP — transport-neutral pipeline construction
- **WP-3:** Enrichment removed from EngineSink (confirmed already done per ADR-029)
- **WP-4:** ADR-029 dead code cleanup (PhaseStatus, pulldown-cmark, parse_response, apply_mutation removed)
- **WP-5:** Domain model open questions resolved

364 library tests + 25 acceptance tests passing. No known regressions.

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
| ADR-029 | Architectural consolidation decisions | Accepted, implemented |

## Open Questions

1. **SemanticAdapter / DeclarativeAdapter convergence** — ADR-028 planned convergence is deferred. Both exist as independent types.
2. **Phase 2 extraction** — No adapter exists since ADR-029 removed TextAnalysisAdapter. Rebuild or collapse to 2 phases?
3. **Batched normalization** — O(edges x adapters) per emission. Scaling concern for large graphs.
