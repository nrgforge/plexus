# Interaction Specifications

**Derived from:** product-discovery.md (stakeholder models, 2026-04-02)
**Complements:** scenarios 036-037 (business-rule behavior)
**Scope:** MCP consumer interaction surface cycle

---

## Stakeholder: Consumer Application Developer

**Super-Objective:** Ingest domain-specific data into a knowledge graph, receive structural signals in domain vocabulary, and act on those signals — without learning graph internals.

### Task: Set up a working session

**Interaction mechanics:** The consumer connects via a transport (MCP, gRPC, embedded). The first operation is selecting a context — the bounded subgraph representing the consumer's project or workspace. Via MCP, this is `set_context` (which creates the context if it doesn't exist). The consumer must have a context before any other operation.

### Task: Declare consumer identity

**Interaction mechanics:** The consumer loads its declarative adapter spec onto the active context via `load_spec`. The spec YAML — authored and owned by the consumer — contains the adapter definition, lens translation rules, and declared enrichment config. The consumer sends the full spec content (not a file path reference). Plexus validates the spec upfront: if validation fails, the consumer receives a clear error and no graph work occurs. If validation succeeds, the consumer receives confirmation including the adapter ID, registered enrichments, lens namespace, and the count of vocabulary edges created by the initial lens run.

The consumer may load multiple specs in sequence (e.g., a primary adapter and a secondary one for a different data type). Each `load_spec` call is independent — there is no session-level "active spec" concept; all loaded adapters are available for routing based on input kind.

### Task: Ingest domain data

**Interaction mechanics:** The consumer calls `ingest` with domain-specific data. Plexus routes to the matching adapter by input kind (which was wired by the prior `load_spec`). Ingestion may be long-running — semantic extraction via llm-orc, declared enrichments that invoke external processes. The consumer does not block on all enrichments completing; the graph enriches incrementally.

After each ingest, the enrichment loop fires — including all lens enrichments registered on the context. The consumer's own lens translates new content into its vocabulary; other consumers' lenses translate it into theirs. Vocabulary layers grow with each ingest.

### Task: Query the graph

**Interaction mechanics:** The consumer queries on its own schedule (pull paradigm). Available query tools:

- `find_nodes` — search by type, dimension, properties. Use `relationship_prefix: "lens:trellis"` to scope to the consumer's vocabulary layer, or `"lens:"` to find all lens-translated edges.
- `traverse` — walk edges from a start node. Combine with `relationship_prefix` for vocabulary-scoped traversal. Use `rank_by: "corroboration"` to surface the most independently supported connections.
- `find_path` — shortest path between two nodes, optionally filtered by vocabulary layer.
- `evidence_trail` — trace a concept back to its source material.
- `changes_since` — pull-based event query. "What happened since I last looked?" The consumer manages its own cursor (sequence number). Events include lens-created edges from any consumer's ingestion.
- `list_tags` — concept discovery within the active context.
- `shared_concepts` — cross-context concept intersection.

The consumer navigates between vocabulary layers by changing the `relationship_prefix` filter. No special "switch lens" operation is needed — it's a query parameter.

### Task: Discover vocabulary layers

**Interaction mechanics:** A consumer arriving at a context with existing lenses discovers them by querying for edges with `relationship_prefix: "lens:"`. The distinct `lens:{consumer}:` prefixes in the results reveal what vocabulary layers exist. This is graph introspection — no dedicated discovery API is required.

The consumer may then explore a specific vocabulary layer by filtering with `relationship_prefix: "lens:carrel"` (another consumer's vocabulary). Cross-consumer vocabulary browsing is emergent from public lens output (Invariant 56) and composable filters (ADR-034).

### Task: Disconnect and return later

**Interaction mechanics:** The consumer disconnects from the transport. Its adapter wiring is transient — it no longer routes ingest calls. But:

- Vocabulary edges persist in the graph (Invariant 62).
- Lens translation rules persist in SQLite (ADR-037). Other consumers' ingests continue to trigger the disconnected consumer's lens, growing its vocabulary layer.
- On reconnection, the consumer calls `set_context` and (if needed) `load_spec` to re-establish its adapter for ingestion. The vocabulary layer is already there; only the adapter routing needs re-establishment.
- `changes_since` with the consumer's last cursor reveals everything that happened while it was away — including vocabulary edges created by its persisted lens reacting to other consumers' ingests.

---

## Stakeholder: Domain-Specific Extractor Author

**Super-Objective:** Process raw source material into structured concepts and relationships without needing to understand graph internals.

### Task: Write an extractor

**Interaction mechanics:** The extractor author writes domain-specific extraction logic (Python script, LLM prompt, Rust function) that produces structured JSON from raw source material. The extractor does not interact with Plexus directly — it is invoked by the declarative adapter spec or by llm-orc.

### Task: Write a declarative adapter spec

**Interaction mechanics:** The extractor author writes a YAML spec that maps their extractor's JSON output to graph structure using adapter spec primitives (`create_node`, `create_edge`, `create_provenance`, `for_each`, `update_properties`). The spec also declares enrichments and optionally a lens. The spec is validated when loaded via `load_spec` — the author receives clear errors if the spec is malformed.

### Task: Test the extractor pipeline

**Interaction mechanics:** The author loads their spec via `load_spec`, ingests test data, and queries the graph to verify that the extraction → adapter → enrichment → lens pipeline produces the expected graph structure. The `evidence_trail` tool traces concepts back to source material, confirming the provenance chain is intact.

---

## Stakeholder: Engine Developer

**Super-Objective:** Navigate the codebase quickly, trust that tests verify invariants, and understand why each architectural decision was made.

### Task: Add a new MCP tool

**Interaction mechanics:** The engine developer adds a `#[tool]` handler method to `PlexusMcpServer` (in `src/mcp/mod.rs`) that delegates to the corresponding `PlexusApi` method. The tool's parameters are flat optional fields (not nested objects). The developer adds a scenario to `docs/scenarios/036-mcp-query-surface.md` and an acceptance test. The transport is a thin shell — the developer never implements query logic in the MCP layer.

### Task: Verify spec loading lifecycle

**Interaction mechanics:** The engine developer runs the test suite. Spec loading scenarios (037) verify: validation rejects invalid specs before graph work, complete wiring (adapter + enrichments + lens), lens fires on other consumers' ingests, persistence survives restart, and unloading preserves vocabulary edges. Integration scenarios verify the end-to-end MCP flow.
