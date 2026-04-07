# Reflections: Multi-Consumer Lens Interaction
*2026-04-01*

## The Single-Consumer Assumption

The query surface cycle (Essays 001–002, ADRs 033–035) designed lenses, composable filters, and event cursors from a single consumer's perspective. The implicit model: one consumer configures an adapter spec with a lens, ingests content, and queries through that lens using `relationship_prefix` filters. The design is internally consistent at this scope.

The assumption surfaced when attempting manual testing through the MCP transport. The MCP server exposes `set_context` (which graph) and `ingest` (push data), but has no mechanism for a consumer to configure its adapter spec or lens. The pipeline auto-discovers specs from `adapter-specs/` at server startup, but this is a deployment-time concern, not an interaction-time one. A consumer connecting via MCP cannot declare how it wants to interact with the graph.

## Contexts Are Multi-Consumer

A Plexus context is not scoped to a single consumer. Multiple consumers may ingest into the same context, each through their own declared adapter. Each consumer may encode its own lens — an enrichment that translates the graph's underlying relationships into that consumer's domain vocabulary. The graph accumulates vocabulary layers:

- Consumer A's lens creates `lens:trellis:thematic_connection` edges
- Consumer B's lens creates `lens:carrel:citation_link` edges
- Both translate the same underlying `may_be_related` edges

This is by design — Invariant 56 states lens output is public, visible to all consumers. But the query surface cycle didn't address the interaction model that follows from this: a new consumer connecting to a context with existing lenses needs to discover what vocabularies are already available, and may want to query through another consumer's lens rather than (or in addition to) its own.

## What's Missing from the Domain Model

The domain model defines lens as an enrichment (Invariant 57) and declares lens output is public (Invariant 56). It does not model:

- **Lens registration** as an API-level operation. Currently, lenses exist because `DeclarativeAdapter::lens()` returns an enrichment during pipeline construction. There is no way to register a lens after startup, and `register_specs_from_dir` doesn't wire the lens enrichment at all (a gap found during this session).
- **Lens discovery** — querying what lenses exist on a context. Since lenses produce edges with `lens:{consumer}:{relationship}` namespace, this is partially derivable from graph state, but there is no direct query for "what lenses are registered."
- **The consumer session model** — the parallel between `set_context` (which graph) and a hypothetical "set spec" (which adapter/lens). Together these define a consumer's relationship to the graph.
- **Vocabulary layering** as a first-class concept. Multiple lenses on one context create a vocabulary landscape. A consumer can query through its own lens, another consumer's lens, or the raw underlying relationships. The `relationship_prefix` filter is the mechanism, but the concept of navigating across vocabulary layers isn't named.

## Implications for API Design

This is not MCP-specific. The multi-consumer interaction model belongs at the `PlexusApi` level (Invariant 38 — transports are thin shells). The API needs:

1. A way to register a declarative adapter spec (including its lens) at runtime, not just at pipeline construction time
2. A way to discover what lenses/specs are active on a context
3. The query tools (already designed in ADR-036) with filter parameters that make vocabulary navigation possible

MCP tools would be thin wrappers over these API operations, as with every other MCP tool.

## Connection to Prior Reflections

Reflection 001 identified "core projections" as engine-provided query perspectives usable by any consumer without declaring their own lens. The multi-consumer model reinforces this: a consumer exploring an unfamiliar context benefits from both core projections (engine-provided views) and the ability to discover consumer-declared lenses (domain-specific views). These are complementary — core projections orient, consumer lenses specialize.

Reflection 001 also noted the gap between query primitives and product-level queries. The multi-consumer lens interaction is part of this gap: the primitives exist (`find_nodes`, `traverse` with `relationship_prefix` filter), but the composition layer — "show me this graph through Trellis's vocabulary" — requires knowing what vocabularies are available.

## Open Questions

- **OQ-24 (proposed):** Should spec/lens registration be session-scoped or persistent? If a consumer registers a lens via MCP, does it survive server restart? (File-based auto-discovery is persistent; runtime registration may not be.)
- **OQ-25 (proposed):** How does lens discovery interact with the event cursor? If a new lens is registered and enriches existing edges, do those enrichment events appear in the cursor stream? (Invariant 37 suggests yes — enrichment loop events are persisted.)
- **OQ-26 (proposed):** Is vocabulary layering a concept the domain model should name, or is it an emergent property of public lens output (Invariant 56) + composable filters (ADR-034)?
