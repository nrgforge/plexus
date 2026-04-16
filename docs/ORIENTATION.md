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
- [decisions/](decisions/) — 38 ADRs (000–037). Architectural decisions with context, rationale, and consequences.
- [scenarios/](scenarios/) — behavior scenarios grouped by ADR range. Acceptance criteria for each feature.
- [references/field-guide.md](references/field-guide.md) — module-to-code mapping. Where things live and why.
- [audits/](audits/) — citation audits, argument audits, conformance scans.

## Current State

**MCP consumer interaction cycle — BUILD complete (2026-04-01 — 2026-04-16).** WP-A through WP-H.2 shipped plus post-WP hardening. Runtime spec loading (ADR-037) is live; the MCP query surface (ADR-036) is wired; the two-consumer cross-pollination scenario is verified end-to-end through the compiled `plexus mcp` binary over raw JSON-RPC.

**Central new capability:** persisted lens enrichments rehydrate at library construction time via `PipelineBuilder::with_persisted_specs` — vocabulary layers are a durable property of the **context** rather than the **consumer process**. Cross-pollination between consumer domains happens automatically whenever any consumer holds the library against a shared context.

**Key builder evolution:** `PipelineBuilder::with_llm_client(client)` is the single method that wires SemanticAdapter onto the ExtractionCoordinator (so `extract-file` invokes llm-orc) AND stores the client on the pipeline so `load_spec` can propagate it to consumer declarative adapters with `ensemble:` fields. `default_pipeline` constructs a `SubprocessClient` by default.

**MCP surface:** 17 tools — 1 session (`set_context`), 1 ingest, 6 context management, 7 graph read (`evidence_trail`, `find_nodes`, `traverse`, `find_path`, `changes_since`, `list_tags`, `shared_concepts`), 2 spec lifecycle (`load_spec`, `unload_spec`). All thin wrappers over `PlexusApi`.

Cycle artifacts:
- ADRs 036 (MCP query surface), 037 (consumer spec loading; §4 superseded 2026-04-14 by WP-H.1)
- Domain model invariants 60 (upfront spec validation), 61 (consumer owns spec; narrowed 2026-04-14 to programmatic-only), 62 (durable vocabulary + lens registration)
- Domain-model terminology: extraction phases use descriptive names (registration / structural_analysis / semantic_extraction), not "Phase 1/2/3"
- 38 ADRs total. **508 tests default-run** (425 lib + 82 acceptance + 1 doc). **511 tests with `PLEXUS_INTEGRATION=1`** (T6/T7/T8/T11 against real Ollama).

**Confirmed architectural follow-ups** (see [cycle-status.md](cycle-status.md) § Follow-ups):
- Background-phase + lens gap (T11 pins current behavior): semantic extraction's output is not translated by registered lenses. Consumers wanting lens coverage over llm-orc-driven extraction must use a declarative adapter with `ensemble:` field (foreground path) instead.
- Outbound event asymmetry (SemanticAdapter + GraphAnalysisAdapter still don't override `transform_events`)
- Customizable outbound events in declarative specs
- Async event delivery for long-running ingest (cursors cover GraphEvents but not OutboundEvents)
- MCP ingest response should carry actual events, not just a count

**To resume work:** invoke `/rdd-play` for experiential discovery (recommended next phase — inhabit stakeholder roles, exercise the live system, produce field notes), `/rdd-synthesize` for publishable insight extraction, or `/rdd-graduate` to fold cycle knowledge into native docs.
