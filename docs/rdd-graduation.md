# RDD Graduation Record

**Graduated:** 2026-07-07
**RDD plugin version(s) used:** 0.8.x series across the cycles; 0.8.7 at graduation
**Scope:** Whole project — the full RDD corpus, spanning the founding work
(essays 01–26, ADRs 000–032), the query-surface cycle (ADRs 033–035), the
MCP consumer-interaction cycle (ADRs 036–037), and the default-install /
lens-design cycle (ADRs 038–042, concluded with PLAY closure 2026-07-07).
**Cycle topic at graduation:** Default-Install Experience and Lens Design
Principles (started 2026-04-17, concluded 2026-07-07).

RDD served this project from inception through identity formation. The
system's identity is now settled; remaining work is ordinary engineering
toward the vision (`docs/vision.md`). Post-graduation findings route to
GitHub issues, not new cycles.

## What Was Migrated

| Knowledge | Source | Destination |
|-----------|--------|-------------|
| Vocabulary (ubiquitous language) + Invariants 1–62 + binding disambiguations | `docs/domain-model.md` | `docs/invariants.md` (numbering preserved verbatim — code cites "Invariant N") |
| Product vision, consumer mental model, multi-consumer interaction model, value tensions, parked hypotheses | `docs/product-discovery.md`, `docs/interaction-specs.md`, cycle-status hypothesis sections, 2026-07-07 alignment conversation | `docs/vision.md` |
| Engineering queue (7 items from PLAY findings) | `docs/cycle-status.md` §PLAY Closure, field notes | GitHub issues + `docs/vision.md` §Engineering queue |
| Documentation index | `docs/ORIENTATION.md` (RDD-artifact map) | `docs/ORIENTATION.md` (rewritten as native doc index) |

## What Was Archived

All at `docs/archive/` (joining the previously-archived first-generation corpus):

| Artifact | Location |
|----------|----------|
| Cycle status (final cycle, incl. PLAY closure + hypothesis parking) | `archive/cycle-status-default-install-lens-design.md` |
| Domain model (full: actions, relationships, amendment log, open questions) | `archive/domain-model.md` |
| Product discovery | `archive/product-discovery.md` |
| Interaction specs | `archive/interaction-specs.md` |
| Roadmap (final state; open decision points preserved) | `archive/roadmap.md` |
| Behavior scenarios (17 files; realized as the test suite) | `archive/scenarios/`, `archive/scenarios-index.md` |
| Housekeeping (gates, susceptibility snapshots, audits, spikes) | `archive/housekeeping/` |
| Audits (argument, citation, conformance — query-surface + MCP cycles) | `archive/audits/` |
| Essays 001–002, research logs, reflections, PLAY field notes | `archive/essays/` |

## What Was Kept As-Is

| Artifact | Reason |
|----------|--------|
| `docs/decisions/` (ADRs 000–042) | Industry-native decision format; code cites ADR numbers extensively; immutable historical record |
| `docs/system-design.md` | The living architecture document (module decomposition, responsibility matrix, amendment log) |
| `docs/references/` (field-guide, spec-author-guide, experiment data) | Native developer/consumer documentation |
| `docs/papers/` | Standalone papers |
| `docs/schemas/` | Wire/data schemas referenced by code |
| `README.md` | Native; rewritten during WP-D with lean-baseline framing |
| `tools/play-harness/` | PLAY instrument graduated into the vision's permanent validation harness |

## Reference notes

- Code references to `ADR-NNN` and `Invariant N` remain resolvable
  (ADRs kept; invariant numbering preserved in `docs/invariants.md`).
- ADRs' internal links to `docs/domain-model.md` etc. now point at moved
  files; ADRs are immutable historical records and were deliberately not
  edited. This record is the redirect map.
- Living docs (README, system-design, spec-author-guide, ORIENTATION)
  were updated to point at native or archived locations.
