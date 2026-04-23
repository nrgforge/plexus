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

**Default-install experience and lens design principles cycle — ARCHITECT complete (2026-04-22), BUILD pending.** DISCOVER (update), MODEL (light-touch), DECIDE, and ARCHITECT all complete. Five ADRs landed in DECIDE (all Accepted 2026-04-21):

- **ADR-038** — Release-binary feature profile. `default = []` stays; the Homebrew/CLI binary ships lean. No Rust code path to llm-orc for embedding. Consumers activate embedding via a declarative adapter spec declaring an llm-orc ensemble; library consumers under `features = ["embeddings"]` retain the in-process `FastEmbedEmbedder` path.
- **ADR-039** — `created_at` property contract. Authoritative on `node.properties["created_at"]` as ISO-8601 UTC string. Producer/consumer alignment is a coordinated four-site fix in BUILD (WP-A).
- **ADR-040** — DiscoveryGap trigger sources. Source-agnostic acceptance; no algorithm broadening; multiple parameterizations allowed. Graceful-idle by design when no producer is active.
- **ADR-041** — Lens grammar conventions. Structural predicates endorsed as convention (not requirement) for discovery-oriented jobs; per-job, not per-app. Phenomenology-of-discovery held as hypothesis; composition-shape reasoning is independently load-bearing.
- **ADR-042** — Dimension extensibility guidance. Documentation-only semantic guidance + syntactic validation at `load_spec`. Exclusive-allowlist behavior is conformance debt (WP-B).

**ARCHITECT delta (system-design v1.3, 2026-04-22):** architectural drivers reshaped to name both embedding backends as first-class per deployment class (in-process under feature flag; consumer-declared external via adapter spec). New "Embedding Backend Deployment Classes" subsection enumerates the three deployment shapes. DiscoveryGap trigger-source contract named explicitly in Core Enrichment Algorithms. New fitness criterion: default binary does not break without llm-orc or consumer-authored spec. No new modules, no new dependency edges.

**BUILD scope (see [roadmap.md](roadmap.md)):** five WPs (A–E), no hard dependencies between them, recommended order A → C → B → D. WP-D's documentation deliverables (README lean-baseline framing, worked-example spec at `examples/specs/embedding-activation.yaml` crossing the tautology threshold, spec-author documentation, lens grammar convention) are load-bearing for ADR-038's "positive decision, not defect-by-omission" reframing. Weak deliverables reassert the defect-by-omission framing.

**Prior cycle carried forward:** MCP consumer interaction surface cycle (2026-04-01 — 2026-04-17) is in the Completed Work Log. 17 MCP tools, runtime spec loading, persisted-spec rehydration at library construction time. Confirmed architectural follow-ups remain: background-phase + lens gap (T11 — semantic extraction output is not lens-translated; consumers wanting lens coverage over LLM-extracted structure use declarative `ensemble:` path), outbound event asymmetry on SemanticAdapter + GraphAnalysisAdapter, customizable outbound events in declarative specs, async event delivery for long-running ingest, MCP ingest response event shape.

**Totals:** 43 ADRs (000–042). 508 tests default-run (425 lib + 82 acceptance + 1 doc); 511 with `PLEXUS_INTEGRATION=1` (T6/T7/T8/T11 real-Ollama gated).

**To resume work:** invoke `/rdd-build` to enter BUILD on this cycle's WPs, `/rdd-play` for optional second-stakeholder experiential discovery (non-builder inhabitation of Consumer Application Developer), or `/rdd-graduate` to fold cycle knowledge into native docs when the cycle closes.
