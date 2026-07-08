# Interaction Specifications

**Derived from:** product-discovery.md (stakeholder models, 2026-04-02; updated 2026-04-17)
**Complements:** scenarios 036-037 (MCP consumer interaction surface), scenarios 038-042 (default-install and lens-grammar)
**Scope:** MCP consumer interaction surface cycle + Default-install and lens-design cycle (additions 2026-04-20)

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

### Task: Understand the default-build baseline before authoring a spec *(added 2026-04-20, ADR-038, ADR-040)*

**Interaction mechanics:** A consumer installing the default Homebrew/CLI binary encounters a **lean baseline**: CoOccurrence runs on `tagged_with` edges, TemporalProximity runs on nodes carrying `created_at`, DiscoveryGap is registered but idle (no `similar_to` producer in the default build), EmbeddingSimilarity is not registered. Before authoring a spec, the consumer reads the onboarding documentation that names this baseline explicitly.

The consumer chooses one of two activation paths when embedding-based discovery is needed:

- **llm-orc path (default build):** install llm-orc, configure it with an embedding provider (Ollama locally, OpenAI-compatible endpoint, or any other provider llm-orc supports), author (or adopt) a declarative adapter spec that declares an external enrichment invoking an llm-orc ensemble to emit `similar_to` edges. This is the path the worked-example spec (shipped at `examples/specs/embedding-activation.yaml` or equivalent) demonstrates.
- **In-process path (library consumers):** rebuild with `plexus = { features = ["embeddings"] }`. Registers `EmbeddingSimilarityEnrichment` with `FastEmbedEmbedder`; no llm-orc required.

Consumers whose content flow does not require embedding-based discovery skip activation entirely. The lean baseline is a valid end-state, not a deferred feature.

### Task: Choose a minimum-useful spec rather than a minimum-viable one *(added 2026-04-20, ADR-042 + Product Debt routing)*

**Interaction mechanics:** A **minimum-viable spec** is any spec that passes `load_spec` validation — declaring an adapter ID, input kind, input schema, and at least one `emit` primitive. A minimum-viable spec is not necessarily useful: a spec declaring a single `create_node` emission on untagged input produces isolated nodes with no structural signal. CoOccurrence does not fire (no `tagged_with` edges); TemporalProximity runs but produces only time-proximity edges between arbitrary nodes; DiscoveryGap is idle in the default build. The consumer sees mechanism but not value.

A **minimum-useful spec** names the infrastructure preconditions that make its emissions produce structural signal. At least one of the following must hold:

- The spec's emit produces `tagged_with` edges to concept nodes (so CoOccurrence can detect shared-source patterns).
- The spec declares an external enrichment that produces `similar_to` edges (so DiscoveryGap and any downstream enrichment fire).
- The spec declares an ensemble (via the `ensemble:` field) that performs semantic extraction over prose content, producing tagged concept nodes that CoOccurrence then operates on.
- The `features = ["embeddings"]` build is active and the spec operates on content whose nodes carry embeddable content (so `EmbeddingSimilarityEnrichment` produces `similar_to`).

The consumer chooses one of these infrastructure preconditions, names it explicitly in their spec or in their deployment instructions, and tests that the resulting spec produces edges — not only validates and loads. Testing the loaded spec against untagged prose and verifying that structure emerges is the acceptance check.

### Task: Choose a dimension for my spec's node types *(added 2026-04-20, ADR-042)*

**Interaction mechanics:** Every `create_node` primitive in a declarative adapter spec declares a `dimension` string — the named facet the node lives in. The choice is load-bearing: enrichments filter by dimension (Invariant 50) and `find_nodes` queries can scope by dimension.

The consumer chooses a dimension per node-type by consulting:

- **Shipped-adapter conventions.** If the consumer's node type collides with a shipped-adapter node type, the author consults the shipped adapter's documentation. The content adapter places fragments in `structure`; the extraction coordinator places file and `extraction-status` nodes in `structure`. A consumer whose `fragment` node is conceptually the same as the content adapter's `fragment` should match the convention (`structure`). A consumer whose `fragment` is conceptually different should depart deliberately and accept that dimension-scoped queries and enrichments will see the two node populations separately.
- **Extension to novel domains.** If the consumer's node type has no shipped-adapter convention (e.g., `gesture_phrase`, `code_symbol`, `audio_event`), the consumer chooses a dimension name that suits the domain. Plexus does not prescribe. Common patterns: group node types that an enrichment will filter together into the same dimension; use distinct dimensions for orthogonal facets of the same content.

Plexus validates dimension values syntactically at `load_spec` time (rejects empty strings, whitespace, reserved characters like `:`). Plexus does **not** validate semantic appropriateness — no warn-on-divergence, no canonical node-type-to-dimension table. The author's choice is authoritative for any string that passes syntactic validation.

**Silent-idle failure mode to avoid:** a spec that declares an enrichment reading a property (e.g., `TemporalProximityEnrichment` reading `created_at`) without the spec's `create_node` primitives writing that property produces a silent-idle enrichment — registered, called, but emitting nothing because the read value is always absent. This is not surfaced as an error; the author diagnoses by inspecting the graph for expected edges and, when absent, checking property writes match enrichment reads.

### Task: Choose named-relationship or structural-predicate output relationships in the lens *(added 2026-04-20, ADR-041)*

**Interaction mechanics:** When authoring the `lens:` section of the adapter spec, the author decides the naming register of each translation's `to` relationship. Two registers are available:

- **Named relationships** (`thematic_connection`, `cites`, `draft_about_theme`): the `to` name interprets the edge's meaning. Appropriate for operational jobs within the app — publishing-pipeline routing, search ranking, analytics aggregation — where the app's logic branches on the relationship name.
- **Structural predicates** (`latent_pair`, `bridges_communities`, `density_shift`, `dormant_since_T`, `member_of_candidate_cluster`): the `to` name describes the shape of the connection without interpreting it. Appropriate for discovery-oriented jobs — creative-writing scaffolding, thesis-finding, reflective discovery — where the value proposition involves the end-user's interpretive work, and the app's surface presents the connection as a prompt rather than an assertion.

The choice is **per-job within an app**, not per-app. A consumer whose app supports both a user-facing discovery surface and an operational publishing pipeline may declare translation rules of both registers within a single `lens:` section. The structural-predicate recommendation for discovery-oriented jobs is a **convention** (documented in product discovery and here), not a grammar-enforced constraint. Plexus accepts any syntactically well-formed `to` string.

**Hypothesis-level framing:** the claim that structural predicates preserve a phenomenology-of-discovery experience that named relationships cancel is held as hypothesis in product discovery, not as settled principle. Consumers authoring discovery-oriented lenses should know the convention rests on composition-shape reasoning (structural predicates extend more naturally under network-science enrichments; query patterns differ in expressive reach) rather than on validated phenomenological evidence. A future cycle may promote the hypothesis; until then, the convention is guidance, not requirement.

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
