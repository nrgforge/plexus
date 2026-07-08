# Scenarios 038-042: Default-Install Experience and Lens Design Principles

Refutable behavior scenarios for ADRs 038–042. Terms follow the [domain model](../domain-model.md) vocabulary.

---

## Feature: Release-Binary Feature Profile (ADR-038)

### Scenario: Default Homebrew build has no in-process embedding dependency
**Given** the `plexus` binary compiled with the default feature set (no `embeddings` feature)
**When** the binary is inspected for its compiled dependencies
**Then** neither `fastembed` nor `sqlite-vec` appear in the dependency tree, and the `FastEmbedEmbedder` type is not present in the compiled code.

### Scenario: `with_default_enrichments` does not register `EmbeddingSimilarityEnrichment` in the default build
**Given** a `PipelineBuilder` constructed with no feature flags
**When** `with_default_enrichments()` runs and the resulting pipeline is inspected for registered enrichments
**Then** the registry contains `CoOccurrenceEnrichment`, `DiscoveryGapEnrichment`, and `TemporalProximityEnrichment`, and does NOT contain any `EmbeddingSimilarityEnrichment` instance.

### Scenario: `with_default_enrichments` registers `EmbeddingSimilarityEnrichment` under `features = ["embeddings"]`
**Given** a `PipelineBuilder` constructed with the `embeddings` feature enabled
**When** `with_default_enrichments()` runs and the resulting pipeline is inspected
**Then** the registry contains `EmbeddingSimilarityEnrichment` wired with a `FastEmbedEmbedder`, in addition to CoOccurrence, DiscoveryGap, and TemporalProximity.

### Scenario: No Rust code path to llm-orc for embedding exists in the default build
**Given** the `plexus` binary compiled with the default feature set
**When** the codebase is scanned for types that directly invoke llm-orc for embedding purposes
**Then** no `LlmOrcEmbedder` type (or equivalent Rust-side llm-orc embedding embedder) is present. Llm-orc is reached only through the declarative adapter spec's `ensemble:` mechanism and the existing `SemanticAdapter`.

### Scenario: Default Homebrew build ingests content without an embedding provider
**Given** a default-feature `plexus` binary, no llm-orc installed, and no custom adapter spec loaded
**When** content is ingested into a context
**Then** the ingest succeeds, graph events fire, queries return results, and no panic or startup failure occurs. `EmbeddingSimilarityEnrichment` is not registered so no `similar_to` edges are produced.

### Scenario: Consumer activates embedding via adapter spec in the default build
**Given** a default-feature `plexus` binary and an installed llm-orc with an embedding provider configured
**When** a consumer loads a declarative adapter spec that declares an external enrichment invoking an llm-orc ensemble producing `similar_to` edges, and ingests content through the spec's adapter
**Then** the ensemble runs via the existing declarative path, emits `similar_to` edges through `ingest()`, those edges enter the core enrichment loop, and `DiscoveryGapEnrichment` (if its trigger is `similar_to`) fires on the new edges.

---

## Feature: `created_at` Property Contract (ADR-039)

### Scenario: `ContentAdapter` writes `created_at` to node properties
**Given** a `ContentAdapter` processing a fragment with tags
**When** the adapter emits fragment and concept nodes
**Then** each emitted node's `properties` map contains a `created_at` entry whose value is an ISO-8601 UTC string (e.g., `"2026-04-20T13:00:00Z"`) — distinct from any metadata-surface timestamp.

### Scenario: `ExtractionCoordinator` writes `created_at` on registration phase nodes
**Given** an `ExtractionCoordinator` processing an `extract-file` input
**When** registration emits the file node and the extraction-status node
**Then** both nodes carry `properties["created_at"]` as an ISO-8601 UTC string.

### Scenario: `DeclarativeAdapter` injects `created_at` on spec-created nodes when not explicitly set
**Given** a declarative adapter spec whose `create_node` primitive does not explicitly set `created_at` in its properties
**When** the spec emits a node
**Then** the emitted node's `properties["created_at"]` contains a `rfc3339` ISO-8601 UTC string, injected by `DeclarativeAdapter::interpret_create_node()`.

### Scenario: `DeclarativeAdapter` preserves spec-authored `created_at` when present
**Given** a declarative adapter spec whose `create_node` primitive explicitly sets `created_at` to the string `"2020-01-01T00:00:00Z"`
**When** the spec emits a node
**Then** the emitted node's `properties["created_at"]` is exactly `"2020-01-01T00:00:00Z"` — the spec-authored value wins over the adapter's injection.

### Scenario: `TemporalProximityEnrichment` parses ISO-8601 UTC strings
**Given** two fragment nodes in the same context, both carrying `properties["created_at"]` as ISO-8601 UTC strings within the 24-hour threshold window
**When** the enrichment loop runs after the nodes are committed
**Then** a `temporal_proximity` symmetric edge pair is emitted between the two nodes.

### Scenario: `TemporalProximityEnrichment` skips nodes missing `created_at`
**Given** two fragment nodes in the same context, one with a valid ISO-8601 `created_at` and one without the property
**When** the enrichment loop runs
**Then** no `temporal_proximity` edges are emitted between the two. The enrichment returns no mutations for the pair (no error to the caller; graceful skip).

### Scenario: `TemporalProximityEnrichment` skips nodes whose `created_at` is not parseable
**Given** two fragment nodes in the same context, both carrying `properties["created_at"]`, but one value is the malformed string `"not-a-date"`
**When** the enrichment loop runs
**Then** no `temporal_proximity` edge is emitted between the pair. The enrichment continues processing other pairs in the same round.

### Scenario: Integration — `ContentAdapter` output feeds `TemporalProximityEnrichment`
**Given** an `IngestPipeline` with the default enrichment set and a `ContentAdapter` registered (no stubs)
**When** two fragments are ingested within the 24-hour window
**Then** `TemporalProximityEnrichment` emits a `temporal_proximity` edge pair between the two fragment nodes, demonstrating end-to-end that the property-contract producer and consumer are wired consistently.

---

## Feature: DiscoveryGap Trigger Sources (ADR-040)

### Scenario: DiscoveryGap stays idle without a `similar_to` producer
**Given** a default-feature pipeline with `DiscoveryGapEnrichment` registered with `trigger_relationship: "similar_to"`, and no enrichment, adapter, or spec producing `similar_to` edges
**When** content is ingested and the enrichment loop runs
**Then** no `discovery_gap` edges are emitted. The enrichment returns no mutations across rounds.

### Scenario: DiscoveryGap fires on `similar_to` edges emitted by a declarative adapter spec
**Given** a default-feature pipeline with `DiscoveryGapEnrichment` registered (trigger `similar_to`) and a loaded adapter spec whose `emit` block creates a `similar_to` edge between two concept nodes with no other structural edge between them
**When** the spec's adapter processes input and emits the `similar_to` edge
**Then** `DiscoveryGapEnrichment` detects the pair and emits a `discovery_gap` symmetric edge pair between the same two nodes.

### Scenario: DiscoveryGap fires on `similar_to` re-entering via `ingest()` from an external enrichment
**Given** a default-feature pipeline with `DiscoveryGapEnrichment` registered, and an llm-orc ensemble configured to compute embeddings and re-enter `similar_to` edges via `ingest()`
**When** the ensemble runs and emits a `similar_to` edge via `ingest()` for a pair of nodes with no other structural edge between them
**Then** `DiscoveryGapEnrichment` detects the pair in the core enrichment loop that fires on the re-entered edges, and emits a `discovery_gap` symmetric edge pair.

### Scenario: DiscoveryGap respects its guard on duplicate triggers
**Given** a context in which a `similar_to` edge and a `discovery_gap` edge already exist between two nodes (from a prior enrichment round)
**When** a new `similar_to` emission arrives between the same two nodes
**Then** no additional `discovery_gap` edge is emitted — the guard "no output edge already exists between A and B" prevents re-emission.

### Scenario: Multiple DiscoveryGap parameterizations coexist
**Given** a pipeline with two `DiscoveryGapEnrichment` instances — one parameterized `{trigger: "similar_to", output: "discovery_gap"}` and one parameterized `{trigger: "embedding:mistral:similar_to", output: "discovery_gap_mistral"}`
**When** the pipeline is constructed and its enrichment registry is inspected
**Then** both enrichments are registered with distinct `id()` values (`discovery_gap:similar_to:discovery_gap` and `discovery_gap:embedding:mistral:similar_to:discovery_gap_mistral` respectively) and fire independently.

### Scenario: Deduplication of identical DiscoveryGap parameterizations
**Given** two declarative adapter specs loaded onto the same pipeline, each declaring a `DiscoveryGapEnrichment` with identical parameterization (`trigger: "similar_to"`, `output: "discovery_gap"`)
**When** the second spec is registered
**Then** the pipeline's enrichment registry contains exactly one instance at that `id()` — the second registration is a no-op (the existing registration stands; no replacement, no error).

---

## Feature: Lens Grammar Conventions (ADR-041)

### Scenario: Declarative adapter spec accepts named-relationship `to` value
**Given** a declarative adapter spec whose `lens:` translation rule specifies `to: thematic_connection`
**When** `load_spec` is invoked with the spec content
**Then** validation succeeds; the lens enrichment is registered; no warning or advisory is emitted about the naming convention.

### Scenario: Declarative adapter spec accepts structural-predicate `to` value
**Given** a declarative adapter spec whose `lens:` translation rule specifies `to: latent_pair`
**When** `load_spec` is invoked with the spec content
**Then** validation succeeds; the lens enrichment is registered; no warning or advisory is emitted about the naming convention.

### Scenario: Consumer spec mixes both grammar registers within a single lens
**Given** a declarative adapter spec whose `lens:` section declares two translation rules — one with `to: latent_pair` (for discovery surface) and one with `to: ready_to_publish` (for operational pipeline routing)
**When** `load_spec` is invoked and the lens runs on existing graph content
**Then** both translations execute; edges `lens:{consumer}:latent_pair` and `lens:{consumer}:ready_to_publish` are created side-by-side with no grammar-enforcement interference.

---

## Feature: Dimension Extensibility (ADR-042)

### Scenario: Spec declaring a novel dimension loads successfully
**Given** a declarative adapter spec whose `create_node` primitive declares `dimension: gesture` (not in the shipped-convention set of `structure`/`semantic`/`relational`/`temporal`/`provenance`/`default`)
**When** `load_spec` is invoked with the spec content
**Then** validation succeeds; the adapter wires; ingesting through the adapter produces nodes with `dimension: "gesture"` exactly as declared.

### Scenario: Spec declaring an empty dimension fails at load_spec
**Given** a declarative adapter spec whose `create_node` primitive declares `dimension: ""`
**When** `load_spec` is invoked with the spec content
**Then** validation fails fast at load time with a clear error naming the `create_node` primitive whose dimension is malformed. No adapter is wired; no enrichment registered; no graph work occurs (Invariant 60).

### Scenario: Spec declaring a dimension with whitespace fails at load_spec
**Given** a declarative adapter spec whose `create_node` primitive declares `dimension: "semantic dimension"`
**When** `load_spec` is invoked
**Then** validation fails fast with a clear error. The dimension string contains whitespace and is rejected at the syntactic well-formedness check.

### Scenario: Spec declaring a dimension with reserved characters fails at load_spec
**Given** a declarative adapter spec whose `create_node` primitive declares `dimension: "lens:trellis"`
**When** `load_spec` is invoked
**Then** validation fails fast with a clear error. The colon character is reserved (namespace-relationship syntax) and rejected at the syntactic well-formedness check.

### Scenario: Spec declaring a shipped-convention dimension loads without warning
**Given** a declarative adapter spec whose `create_node` primitive declares `dimension: semantic` for `node_type: fragment` (a naming pattern that collides with the content adapter's shipped convention of `dimension: structure` for `fragment`)
**When** `load_spec` is invoked
**Then** validation succeeds; no warning, no diagnostic is emitted. Plexus does not police dimension-for-node-type conventions at load time (option (i) was rejected by ADR-042).

### Scenario: Two adapters declaring different dimensions for the same node_type coexist
**Given** a shipped `ContentAdapter` placing fragments in `dimension: structure` and a loaded declarative adapter spec placing fragments in `dimension: semantic`
**When** both adapters process inputs producing `fragment` nodes in the same context
**Then** the context contains `fragment` nodes in both `structure` and `semantic` dimensions. Dimension-filtered queries for `structure` return the content-adapter fragments; queries for `semantic` return the spec-adapter fragments. This is the PLAY Finding 3 behavior preserved by ADR-042's acceptance of both.

### Scenario: Integration — dimension validation runs at `validate_spec`, not at `process`
**Given** a declarative adapter spec with an empty-string dimension value in a `create_node` primitive
**When** `DeclarativeAdapter::validate_spec()` runs (called from `from_yaml()` / `load_spec`)
**Then** validation returns an error before any `process()` call can execute. A spec author receives the error at load time, not after attempting to ingest content.
