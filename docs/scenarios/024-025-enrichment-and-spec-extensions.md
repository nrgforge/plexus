# Behavior Scenarios: Core/External Enrichment Architecture and Declarative Spec Extensions

ADRs: 024 (core and external enrichment architecture), 025 (declarative adapter spec extensions)

---

## Feature: DiscoveryGapEnrichment (ADR-024)

> Build after embedding infrastructure provides `similar_to` edges. Build the enrichment now to validate the pattern; it becomes useful when embeddings arrive (OQ-14).

### Scenario: Discovery gap detected between latently similar but structurally unconnected nodes
**Given** a DiscoveryGapEnrichment configured with `trigger_relationship: "similar_to"` and `output_relationship: "discovery_gap"`
**And** the graph contains concept nodes A and B with no edges between them
**When** an embedding adapter emits a `similar_to` edge between A and B
**And** the enrichment loop runs
**Then** the enrichment emits a `discovery_gap` symmetric edge pair between A and B
**And** the contribution value equals the `similar_to` edge's contribution

### Scenario: No discovery gap when structural evidence already exists
**Given** a DiscoveryGapEnrichment configured with `trigger_relationship: "similar_to"` and `output_relationship: "discovery_gap"`
**And** the graph contains concept nodes A and B connected by a `may_be_related` edge
**When** an embedding adapter emits a `similar_to` edge between A and B
**And** the enrichment loop runs
**Then** the enrichment does NOT emit a `discovery_gap` edge between A and B

### Scenario: Discovery gap enrichment reaches quiescence
**Given** a DiscoveryGapEnrichment configured with `trigger_relationship: "similar_to"` and `output_relationship: "discovery_gap"`
**And** the enrichment has emitted `discovery_gap` edges for all unconnected `similar_to` pairs
**When** the enrichment loop runs a second round with the events from the first round
**Then** the enrichment returns `None` (quiescent)

### Scenario: DiscoveryGapEnrichment has unique stable ID
**Given** a DiscoveryGapEnrichment configured with `trigger_relationship: "similar_to"` and `output_relationship: "discovery_gap"`
**Then** its `id()` returns `"discovery_gap:similar_to:discovery_gap"`

---

## Feature: TemporalProximityEnrichment (ADR-024)

> Build alongside DiscoveryGapEnrichment. Critical for EDDI's reactive performance needs.

### Scenario: Temporal proximity detected between nodes within threshold
**Given** a TemporalProximityEnrichment configured with `timestamp_property: "gesture_time"`, `threshold_ms: 500`, and `output_relationship: "temporal_proximity"`
**And** the graph contains node A with property `gesture_time: 1000`
**When** node B is emitted with property `gesture_time: 1300`
**And** the enrichment loop runs
**Then** the enrichment emits a `temporal_proximity` symmetric edge pair between A and B

### Scenario: No temporal proximity when nodes exceed threshold
**Given** a TemporalProximityEnrichment configured with `timestamp_property: "gesture_time"`, `threshold_ms: 500`, and `output_relationship: "temporal_proximity"`
**And** the graph contains node A with property `gesture_time: 1000`
**When** node B is emitted with property `gesture_time: 2000`
**And** the enrichment loop runs
**Then** the enrichment does NOT emit a `temporal_proximity` edge between A and B

### Scenario: Nodes without timestamp property are skipped
**Given** a TemporalProximityEnrichment configured with `timestamp_property: "gesture_time"`, `threshold_ms: 500`, and `output_relationship: "temporal_proximity"`
**When** node C is emitted without a `gesture_time` property
**And** the enrichment loop runs
**Then** the enrichment skips node C and returns `None`

### Scenario: TemporalProximityEnrichment reaches quiescence
**Given** a TemporalProximityEnrichment that has emitted edges for all temporal pairs
**When** the enrichment loop runs a second round
**Then** the enrichment returns `None` (quiescent)

### Scenario: TemporalProximityEnrichment has unique stable ID
**Given** a TemporalProximityEnrichment configured with `timestamp_property: "gesture_time"`, `threshold_ms: 500`, and `output_relationship: "temporal_proximity"`
**Then** its `id()` returns `"temporal:gesture_time:500:temporal_proximity"`

---

## Feature: External enrichment emission trigger (ADR-024)

> Design-deferred implementation. These scenarios define the contract for when it is built.

### Scenario: Emission-triggered external enrichment runs in background
**Given** an adapter spec with `external_enrichments: [{ ensemble: "deep-analysis", trigger: emission }]`
**And** llm-orc is running
**When** the adapter processes input and the emission is committed
**Then** the `deep-analysis` ensemble is spawned as a background task
**And** the `ingest()` call returns immediately (does not wait for the external enrichment)

### Scenario: Emission-triggered external enrichment results re-enter via ingest
**Given** an emission-triggered external enrichment has completed
**When** its results are ready
**Then** the results enter the graph via `ingest()` with the external enrichment's adapter ID
**And** core enrichments fire on the new data (standard enrichment loop)

### Scenario: Emission-triggered external enrichment degrades gracefully
**Given** an adapter spec with `external_enrichments: [{ ensemble: "deep-analysis", trigger: emission }]`
**And** llm-orc is NOT running
**When** the adapter processes input and the emission is committed
**Then** the external enrichment is skipped (not spawned)
**And** no error is surfaced to the consumer

---

## Feature: Declarative adapter spec enrichment wiring (ADR-025)

> Build alongside or after the new core enrichments from ADR-024.

### Scenario: DeclarativeAdapter exposes enrichments from spec
**Given** a declarative adapter spec with:
  ```yaml
  enrichments:
    - type: co_occurrence
      source_relationship: exhibits
      output_relationship: co_exhibited
  ```
**When** `DeclarativeAdapter::from_yaml(yaml)` is called
**Then** `adapter.enrichments()` returns a vector containing one `CoOccurrenceEnrichment`
**And** the enrichment's `id()` is `"co_occurrence:exhibits:co_exhibited"`

### Scenario: DeclarativeAdapter enrichments are registered globally
**Given** a DeclarativeAdapter with enrichments
**When** `pipeline.register_integration(adapter, adapter.enrichments())` is called
**Then** the enrichments are available in the `EnrichmentRegistry`
**And** they fire after any adapter's emission (not just the declaring adapter)

### Scenario: Default enrichment parameters when omitted
**Given** a declarative adapter spec with:
  ```yaml
  enrichments:
    - type: tag_concept_bridger
  ```
**When** `DeclarativeAdapter::from_yaml(yaml)` is called
**Then** `adapter.enrichments()` returns a TagConceptBridger with relationship `"references"` (the default)

### Scenario: Unknown enrichment type is rejected
**Given** a declarative adapter spec with:
  ```yaml
  enrichments:
    - type: nonexistent_enrichment
  ```
**When** `DeclarativeAdapter::from_yaml(yaml)` is called
**Then** construction fails with an error naming the unknown enrichment type

---

## Feature: Declarative adapter spec ensemble field (ADR-025)

> Build when DeclarativeAdapter needs llm-orc integration (consumer onboarding).

### Scenario: DeclarativeAdapter invokes ensemble for Layer 1 extraction
**Given** a declarative adapter spec with `ensemble: "sketchbin-extraction"`
**And** llm-orc is running with the `sketchbin-extraction` ensemble available
**When** `ingest()` is called with input data
**Then** the adapter invokes `llm_orc_client.invoke("sketchbin-extraction", input_json)`
**And** the ensemble response is used as input for the `emit` primitives

### Scenario: DeclarativeAdapter without ensemble uses input directly
**Given** a declarative adapter spec with no `ensemble` field
**When** `ingest()` is called with input JSON
**Then** the `emit` primitives operate directly on the input JSON
**And** no llm-orc invocation occurs

### Scenario: DeclarativeAdapter with ensemble degrades gracefully
**Given** a declarative adapter spec with `ensemble: "sketchbin-extraction"`
**And** llm-orc is NOT running
**When** `ingest()` is called
**Then** the adapter returns `AdapterError::Skipped` (Invariant 47)
**And** no emission is produced

---

## Feature: Declarative adapter spec `update_properties` primitive (ADR-025)

> Build first — completes the primitive set from ADR-020.

### Scenario: update_properties merges into existing node
**Given** a declarative adapter spec with:
  ```yaml
  emit:
    - update_properties:
        node_id: "concept:{input.tag | lowercase}"
        properties:
          pagerank_score: "{input.score}"
  ```
**And** the graph contains `concept:travel` with property `community: 3`
**When** `ingest()` is called with `{ "tag": "travel", "score": "0.034" }`
**Then** `concept:travel` has both `pagerank_score: "0.034"` and `community: 3`

### Scenario: update_properties is no-op for absent node
**Given** a declarative adapter spec with an `update_properties` targeting `concept:nonexistent`
**When** `ingest()` is called
**Then** no error is produced
**And** no node is created

---

## Feature: DeclarativeAdapter YAML deserialization (ADR-025)

> Build first — prerequisite for all other spec extensions.

### Scenario: from_yaml deserializes a complete spec
**Given** a YAML string containing `adapter_id`, `input_kind`, `enrichments`, and `emit` sections
**When** `DeclarativeAdapter::from_yaml(yaml)` is called
**Then** a valid DeclarativeAdapter is returned
**And** `adapter.id()` matches the `adapter_id` in the YAML
**And** `adapter.input_kind()` matches the `input_kind` in the YAML

### Scenario: from_yaml validates dual obligation
**Given** a YAML string with `create_provenance` but no semantic `create_node`
**When** `DeclarativeAdapter::from_yaml(yaml)` is called
**Then** construction fails with a validation error citing the dual obligation (Invariant 7)

### Scenario: from_yaml rejects malformed YAML
**Given** a YAML string with invalid syntax
**When** `DeclarativeAdapter::from_yaml(yaml)` is called
**Then** construction fails with a deserialization error

---

## Conformance Debt

| ADR | Violation | Type | Location | Resolution |
|-----|-----------|------|----------|------------|
| ADR-024 | No `DiscoveryGapEnrichment` struct | missing | `src/adapter/` | Implement with `trigger_relationship`, `output_relationship`, parameterized constructor, idempotent `enrich()` |
| ADR-024 | No `TemporalProximityEnrichment` struct | missing | `src/adapter/` | Implement with `timestamp_property`, `threshold_ms`, `output_relationship`, parameterized constructor, idempotent `enrich()` |
| ADR-025 | `DeclarativeSpec` has no `enrichments` field | missing | `src/adapter/declarative.rs` | Add `enrichments: Vec<EnrichmentDeclaration>` field; add `enrichments()` method to `DeclarativeAdapter` |
| ADR-025 | `DeclarativeSpec` has no `ensemble` field | missing | `src/adapter/declarative.rs` | Add `ensemble: Option<String>` field; update `process()` to invoke llm-orc when present |
| ADR-025 | `Primitive` enum has no `UpdateProperties` variant | missing | `src/adapter/declarative.rs` | Add `UpdateProperties(UpdatePropertiesPrimitive)` variant with `node_id` and `properties` fields |
| ADR-025 | No `from_yaml()` constructor | missing | `src/adapter/declarative.rs` | Add `Deserialize` derives to all spec structs; implement `from_yaml(&str) -> Result<Self, AdapterError>` |
| ADR-025 | No `Deserialize` derives on spec structs | missing | `src/adapter/declarative.rs` | Add `#[derive(Deserialize)]` to `DeclarativeSpec`, `Primitive`, `CreateNodePrimitive`, `CreateEdgePrimitive`, `ForEachPrimitive`, `CreateProvenancePrimitive`, `IdStrategy`, `InputField` |
| ADR-022 | §Tier 1 says "design deferred" | exists (stale) | `docs/adr/022-parameterized-enrichments.md` | Add supersession note: Tier 1 cancelled by ADR-024 |
| ADR-023 | Title and body use "graph analysis" | exists (stale terminology) | `docs/adr/023-graph-analysis.md` | Add supersession note: terminology updated to "external enrichment" by ADR-024 |
