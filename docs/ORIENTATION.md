# Orientation: Plexus

A content-agnostic knowledge graph engine that derives structure from unstructured input. Consumer applications send domain-specific data (creative writing fragments, research citations, code files, movement encodings) through adapters; Plexus tracks per-source contributions, detects cross-domain connections via enrichment algorithms, and returns structural signals. The graph accumulates evidence from independent sources — connections strengthen through reinforcement, not authority. Consumers decide what to do with the signals: surface insights, build outlines, trigger responses, or something else entirely.

## Who It Serves

**Consumer application developers** — build apps that ingest domain data and act on graph signals. They write an adapter (Rust or YAML spec), optionally define a lens for domain vocabulary translation, and query the graph on their own schedule.
- Reading path: [product-discovery.md](product-discovery.md) → [system-design.md](system-design.md) §Pipeline Flow → [field-guide.md](references/field-guide.md) §adapter

**Domain-specific extractor authors** — write extraction logic (scripts, LLM prompts) that produces structured JSON. The declarative adapter spec maps their output to graph structure.
- Reading path: [product-discovery.md](product-discovery.md) §Extractor Author → ADR-028 → [field-guide.md](references/field-guide.md) §adapter/adapters

**Engine developers** — maintain and extend Plexus itself.
- Reading path: this document → [system-design.md](system-design.md) → [field-guide.md](references/field-guide.md) → [domain-model.md](domain-model.md)

## Key Constraints

1. **All writes go through `ingest()`** (Invariant 34) — no public API for raw graph primitives.
2. **All knowledge carries semantic content + provenance** (Invariant 7) — the dual obligation.
3. **Adapters, enrichments, transports are independent extension axes** (Invariant 40) — changes in one don't affect the others.
4. **Transports are thin shells** (Invariant 38) — adding a transport never touches adapters, enrichments, or the engine.
5. **Event cursors preserve the library rule for reads** (Invariant 58) — consumers write, walk away, come back, query "changes since N."

## How the Artifacts Fit Together

**Tier 1 — Entry point (start here):**
- [ORIENTATION.md](ORIENTATION.md) — this document. What the system is, who it serves, where to go next.

**Tier 2 — Primary readables:**
- [product-discovery.md](product-discovery.md) — stakeholder maps, jobs, value tensions, assumption inversions. The "why" behind design choices.
- [system-design.md](system-design.md) — module decomposition, pipeline flow, responsibility allocation, provenance chains. The compiled rollup of all upstream artifacts.
- [roadmap.md](roadmap.md) — work package sequencing, completed work log, open decision points.

**Tier 3 — Supporting material:**
- [domain-model.md](domain-model.md) — ubiquitous language (concepts, actions, relationships, invariants). The naming authority.
- [essays/](essays/) — research essays with citation and argument audits.
- [decisions/](decisions/) — 36 ADRs (000–035). Architectural decisions with context, rationale, and consequences.
- [scenarios/](scenarios/) — behavior scenarios grouped by ADR range. Acceptance criteria for each feature.
- [references/field-guide.md](references/field-guide.md) — module-to-code mapping. Where things live and why.
- [audits/](audits/) — citation audits, argument audits, conformance scans.

## Current State

The query surface cycle (2026-03-26 — 2026-04-01) is complete. Three work packages delivered:

- **WP-A: Event cursor persistence** — pull-based "changes since N" queries via SQLite event log.
- **WP-B: Lens declaration and translation** — consumer-scoped enrichment that translates cross-domain edges into domain vocabulary at write time.
- **WP-C: Composable query filters** — `QueryFilter` (contributor_ids, relationship_prefix, min_corroboration) composable with all query primitives; `RankBy` corroboration ranking.

461 tests (403 lib + 58 acceptance), all passing. 36 ADRs. No active cycle.
