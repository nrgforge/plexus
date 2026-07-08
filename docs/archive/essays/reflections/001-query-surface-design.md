# Reflections: Query Surface Design
*2026-03-25*

## Core Projections as the Read-Side Analog of Core Enrichments

The essay framed projections as consumer-specific declarations — each app defines its own lens. The epistemic gate conversation surfaced an extension: there should be **core projections** alongside declarative ones, just as there are core enrichments alongside declarative enrichment configs.

Core enrichments (co-occurrence, embedding similarity, discovery gap) are generalized graph intelligence any consumer benefits from. Core projections would be generalized *views* any consumer could use without declaring its own:

- **Emerging themes** — nodes with high degree growth or recent contribution density
- **Cross-domain bridges** — nodes/edges connecting clusters from different adapters
- **Discovery gaps** — the read-side complement to DiscoveryGapEnrichment
- **Most corroborated** — edges ranked by evidence diversity

These are query *perspectives*, not query *operations*. An ad hoc LLM client exploring via MCP could use "show me cross-domain bridges" without knowing the graph's internal structure. This is significant because it means the projection concept is not just a consumer configuration concern — it is also a product-level feature of the engine itself.

## The Gap Between Primitives and Product Queries

The essay identifies five query primitives (find, traverse, path, shared_concepts, evidence_trail). The user identified Trellis's actual needs: cross-domain connections, latent discoverables, emerging themes, outline formalization aids. These are product-level query shapes that don't yet have clear compositions from the existing primitives.

This suggests the DISCOVER and DECIDE phases need to work through concrete Trellis query scenarios — "given a user working on an essay in Trellis, what questions does Trellis ask Plexus, and what sequence of primitives answers each one?" The answer may reveal that the five existing primitives are sufficient (with projections as the missing composition layer), or that new higher-level query operations are needed.

## Projections as Bridging Concept Between Consumer and Engine

The user noted that a domain "specifying what it cares about" and "getting that encoded into the graph in ways that link to the generalized KG" has potential but unclear shape. This suggests the projection concept might be more than a read-side filter — it might inform *how enrichments run*. If a consumer's projection declares "I care about thematic connections," the engine could prioritize enrichments that surface thematic structure.

This is speculative but worth holding: projections as a bidirectional concept — informing both read-side filtering AND write-side enrichment prioritization.

## LLM Clients Need Projections Too

Even an LLM exploring via MCP benefits from projections to scope its exploration. Without a projection, an LLM client would need to understand the full graph schema to compose useful queries. With a core projection like "cross-domain bridges," the LLM has a pre-composed entry point. This reinforces that projections are not just a consumer configuration concern but a query ergonomics concern for any client.

## Open Questions for Domain Model

- **OQ-20 (proposed):** What are the core projections, and how do they relate to core enrichments? Is each core enrichment paired with a core projection that surfaces its results?
- **OQ-21 (proposed):** How does a consumer's projection interact with the enrichment loop? Does declaring "I care about X" influence which enrichments run or how they're parameterized?
- **OQ-22 (proposed):** What do concrete Trellis and Carrel query scenarios look like as compositions of primitives + projections?
