# Behavior Scenarios

Refutable behavior scenarios for Plexus. Each scenario can be verified against the running software. All terms follow the [domain model](domain-model.md) vocabulary.

Scenarios are organized by ADR group in [`docs/scenarios/`](scenarios/).

| File | ADRs | Scope |
|------|------|-------|
| [001-semantic-adapter-layer](scenarios/001-semantic-adapter-layer.md) | ADR-001 | Emission validation, sinks, provenance, routing, events |
| [003-reinforcement-mechanics](scenarios/003-reinforcement-mechanics.md) | ADR-003 | Per-adapter contributions, scale normalization, WeightsChanged |
| [004-first-adapter-pair](scenarios/004-first-adapter-pair.md) | ADR-004, 005 | ContentAdapter, CoOccurrenceEnrichment, normalization floor |
| [006-009-runtime-architecture](scenarios/006-009-runtime-architecture.md) | ADR-006–009 | Engine wiring, contribution persistence, provenance, tag bridging |
| [010-012-public-surface](scenarios/010-012-public-surface.md) | ADR-010–012 | Enrichment loop, bidirectional adapter, unified ingest |
| [013-015-public-surface-redesign](scenarios/013-015-public-surface-redesign.md) | ADR-013–015 | StepQuery, evidence trail, PlexusApi, annotate workflow |
| [016-017-storage-architecture](scenarios/016-017-storage-architecture.md) | ADR-016–017 | XDG storage, shared DB, multi-app context |
| [019-023-phased-extraction](scenarios/019-023-phased-extraction.md) | ADR-019–023 | Phased extraction, declarative adapters, graph analysis |
| [022-integration-gap](scenarios/022-integration-gap.md) | ADR-019–021, 025, 028 | Essay 22 conformance gaps — coordinator alignment, ensemble invocation, provenance |
| [024-025-enrichment-and-spec-extensions](scenarios/024-025-enrichment-and-spec-extensions.md) | ADR-024–025 | Core/external enrichment, declarative spec extensions |
| [026-027-embedding-and-retraction](scenarios/026-027-embedding-and-retraction.md) | ADR-026–027 | Embedding enrichment, contribution retraction |
| [030-032-structural-module-system](scenarios/030-032-structural-module-system.md) | ADR-030–032 | Structural module trait, output, first module |
| [033-035-query-surface](scenarios/033-035-query-surface.md) | ADR-033–035 | Lens declaration, composable query filters, event cursor persistence |
| [036-mcp-query-surface](scenarios/036-mcp-query-surface.md) | ADR-036 | MCP query surface tools |
| [037-consumer-spec-loading](scenarios/037-consumer-spec-loading.md) | ADR-037 | `load_spec`, `unload_spec`, spec persistence, lens lifecycle |
| [038-042-default-install-lens-design](scenarios/038-042-default-install-lens-design.md) | ADR-038–042 | Release-binary feature profile, `created_at` property contract, DiscoveryGap trigger sources, lens grammar conventions, dimension extensibility |
