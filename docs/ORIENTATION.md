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
6. **Vocabulary layers are durable graph data; lens enrichments are durably registered on the context** (Invariant 62) — the specs table is the context's lens registry, so any library instance against a context transiently runs those lenses on behalf of the context, making cross-pollination between consumer domains automatic.

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
- [decisions/](decisions/) — 43 ADRs (000–042). Architectural decisions with context, rationale, and consequences.
- [scenarios/](scenarios/) — behavior scenarios grouped by ADR range. Acceptance criteria for each feature.
- [references/field-guide.md](references/field-guide.md) — module-to-code mapping. Where things live and why.
- [audits/](audits/) — citation audits, argument audits, conformance scans.

## Current State

**Default-install experience and lens design principles cycle — BUILD complete (2026-04-24); PLAY pending.** DISCOVER (update), MODEL (light-touch), DECIDE, ARCHITECT, and BUILD all complete. WP-A/B/C/D/E landed. User has elected practitioner-as-builder PLAY as the next phase. Five ADRs landed in DECIDE (all Accepted 2026-04-21):

- **ADR-038** — Release-binary feature profile. `default = []` stays; the Homebrew/CLI binary ships lean. No Rust code path to llm-orc for embedding. Consumers activate embedding via a declarative adapter spec declaring an llm-orc ensemble; library consumers under `features = ["embeddings"]` retain the in-process `FastEmbedEmbedder` path. **The "positive decision, not defect-by-omission" reframing holds** — WP-D's documentation deliverables validated it empirically.
- **ADR-039** — `created_at` property contract. Authoritative on `node.properties["created_at"]` as ISO-8601 UTC string. Producer/consumer alignment shipped in WP-A (`f82bd76`).
- **ADR-040** — DiscoveryGap trigger sources. Source-agnostic acceptance; no algorithm broadening; multiple parameterizations allowed. Graceful-idle by design when no producer is active.
- **ADR-041** — Lens grammar conventions. Structural predicates endorsed as convention (not requirement) for discovery-oriented jobs; per-job, not per-app. Phenomenology-of-discovery held as hypothesis; composition-shape reasoning is independently load-bearing. Convention documented in `docs/references/spec-author-guide.md`.
- **ADR-042** — Dimension extensibility guidance. Documentation-only semantic guidance + syntactic validation at `load_spec`. `validate_dimension_syntax` shipped in WP-B (`2cc25ee`); shipped-adapter convention notes inline with ContentAdapter + ExtractionCoordinator module docs.

**BUILD delta (WP-A/B/C/D, 2026-04-22 / 2026-04-23):**
- **WP-A** `created_at` property contract: four-site `fix:` aligning ContentAdapter / ExtractionCoordinator / DeclarativeAdapter writers with `TemporalProximityEnrichment` reader (ISO-8601 UTC, RFC-3339 parse with graceful degradation).
- **WP-B** Dimension extensibility: `validate_dimension_syntax` replaces the 6-element allowlist in `resolve_dimension`; `validate_spec` walks all CreateNode/CreateEdge/ForEach primitives at load time per Invariant 60.
- **WP-C** Docstring drift: DiscoveryGap trigger-source contract spelled out in `discovery_gap.rs`; stale ADR-009 reference dropped from `src/graph/node.rs`.
- **WP-D** ADR-038 onboarding: worked-example spec at `examples/specs/embedding-activation.yaml` with companion llm-orc ensemble + Python script; two fixture corpora at `test-corpora/collective-intelligence/` (8 arXiv abstracts) and `test-corpora/public-domain-stories/` (6 PG short stories) with falsifiability-inviting CURATION docs; README rewrite (honest lean baseline + capability-loss transparency + 17-tool MCP surface); spec-author guide at `docs/references/spec-author-guide.md` covering dimension choice, lens grammar conventions, ensemble integration, minimum-useful-spec; inline `CreateNodePrimitive` field docs + shipped-adapter convention tables on `ContentAdapter` / `ExtractionCoordinator`. Tautology threshold crossed empirically (within-corpus 0.72-0.90, cross-corpus all below). T12 acceptance test pins the end-to-end worked-example behavior.

**Prior cycle carried forward:** MCP consumer interaction surface cycle (2026-04-01 — 2026-04-17) is in the Completed Work Log. 17 MCP tools, runtime spec loading, persisted-spec rehydration at library construction time. Confirmed architectural follow-ups remain: background-phase + lens gap (T11 — semantic extraction output is not lens-translated; consumers wanting lens coverage over LLM-extracted structure use declarative `ensemble:` path), outbound event asymmetry on SemanticAdapter + GraphAnalysisAdapter, customizable outbound events in declarative specs, async event delivery for long-running ingest, MCP ingest response event shape.

**Totals:** 43 ADRs (000–042). 535 tests default-run (448 lib + 86 acceptance + 1 doc); PLEXUS_INTEGRATION=1 adds T6/T7/T8/T11/T12 real-Ollama gated tests.

**To resume work:** invoke `/rdd-play` to inhabit the Consumer Application Developer role against the now-complete cycle (interrogates onboarding-path traversability, dimension-choice navigation hop, tautology-felt-vs-measured, lens grammar per-job usability — phenomenology hypothesis remains parked for a future non-builder PLAY); `/rdd-synthesize` to extract publishable insight (lean-baseline-as-honest-demo + composition-shape-vs-phenomenology argument-grounds split); or `/rdd-graduate` to fold cycle knowledge into native docs and archive the cycle-status.
