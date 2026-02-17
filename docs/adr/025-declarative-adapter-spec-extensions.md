# ADR-025: Declarative Adapter Spec Extensions

**Status:** Accepted

**Research:** [Essay 19](../essays/19-declarative-adapter-primitives.md)

**Domain model:** [domain-model.md](../domain-model.md) — declarative adapter spec, DeclarativeAdapter, adapter spec primitive, extractor, declarative mapper, parameterized enrichment, external enrichment

**Amends:** ADR-020 (declarative adapter specs — adds `ensemble` field, `enrichments` section, `update_properties` primitive)

**Depends on:** ADR-020 (declarative adapter specs), ADR-021 (llm-orc integration), ADR-022 (parameterized enrichments), ADR-024 (core and external enrichment architecture)

---

## Context

ADR-020 defined the declarative adapter spec language: seven primitives, a template engine, and the two-layer extraction architecture (extractor + mapper). Six of seven primitives are implemented. Two pieces of the consumer-facing surface remained unresolved:

1. **How does a spec declare its Layer 1 extractor?** The two-layer architecture separates extraction (domain-specific, produces JSON) from mapping (domain-agnostic, maps JSON to graph). The llm-orc integration (ADR-021) verified that ensembles ARE Layer 1 extractors. But the spec had no way to name which ensemble to invoke.

2. **How does a spec declare which enrichments its data benefits from?** ADR-022 established parameterized enrichments and the `register_integration()` wiring pattern. But the YAML spec had no `enrichments` section — enrichment wiring required Rust code at the registration call site.

Essay 19's prior art survey confirmed that the two-layer split and template syntax are industry-standard (YARRRML, Nodestream, Koza all follow the same pattern). No existing system has anything like `create_provenance` or enrichment wiring — these are Plexus-specific concerns that the spec language must handle.

## Decision

### The `ensemble` field

The adapter spec YAML declares its Layer 1 extractor via an `ensemble` field:

```yaml
adapter_id: sketchbin-metadata
input_kind: sketchbin.file
ensemble: sketchbin-semantic-extraction

emit:
  - create_node: ...
```

When `DeclarativeAdapter::process()` runs:
1. If `ensemble` is present, invoke the named llm-orc ensemble via `LlmOrcClient::invoke()`, following the same pattern as `SemanticAdapter`: check `is_available()` first, return `AdapterError::Skipped` if unavailable (Invariant 47).
2. The ensemble's response JSON becomes the input for the `emit` primitives. The original `input` data is available as `{input.*}`; the ensemble response is available as `{result.*}`.
3. If `ensemble` is absent, the `emit` primitives operate directly on `{input.*}` — the adapter is a pure mapper with no extraction step.

This makes the two-layer split explicit in a single artifact: `ensemble` is Layer 1 (domain-specific extraction), `emit` is Layer 2 (domain-agnostic mapping).

### The `enrichments` section

The adapter spec YAML declares which core enrichments its data benefits from:

```yaml
enrichments:
  - type: co_occurrence
    source_relationship: exhibits
    output_relationship: co_exhibited
  - type: temporal_proximity
    timestamp_property: gesture_time
    threshold_ms: 500
    output_relationship: temporal_co_occurrence
```

`DeclarativeAdapter` exposes a new method:

```rust
impl DeclarativeAdapter {
    pub fn enrichments(&self) -> Vec<Arc<dyn Enrichment>> { ... }
}
```

Registration at the call site uses the existing pattern:

```rust
let adapter = DeclarativeAdapter::from_yaml(yaml_str)?;
let enrichments = adapter.enrichments();
pipeline.register_integration(Arc::new(adapter), enrichments);
```

Enrichment type resolution is a simple match on the `type` string:

| `type` | Constructor | Parameters |
|--------|-------------|------------|
| `tag_concept_bridger` | `TagConceptBridger::with_relationship()` | `relationship` (default: `"references"`) |
| `co_occurrence` | `CoOccurrenceEnrichment::with_relationships()` | `source_relationship`, `output_relationship` |
| `discovery_gap` | `DiscoveryGapEnrichment::new()` | `trigger_relationship`, `output_relationship` |
| `temporal_proximity` | `TemporalProximityEnrichment::new()` | `timestamp_property`, `threshold_ms`, `output_relationship` |

Enrichments declared in the spec are global — they fire after any adapter, not just the declaring one (Invariant 35). Deduplication by `id()` handles multiple specs declaring the same enrichment with the same parameters (Invariant 39).

### The `update_properties` primitive

The seventh primitive, completing the set from ADR-020:

```yaml
emit:
  - update_properties:
      node_id: "concept:{input.tag | lowercase}"
      properties:
        pagerank_score: "{input.score}"
        last_analyzed: "{input.timestamp}"
```

Maps directly to the existing `PropertyUpdate` mechanism (ADR-023): per-key merge into existing node properties, preserving other keys, no-op if node absent. The interpreter renders templates in `node_id` and property values, then pushes a `PropertyUpdate` onto `emission.property_updates`.

### Full spec format

A complete declarative adapter spec:

```yaml
adapter_id: eddi-gesture
input_kind: eddi.gesture_session
ensemble: eddi-extraction        # optional: Layer 1 extractor

input_schema:                     # optional: JSON Schema validation
  - name: gestures
    type: array
    required: true

enrichments:                      # optional: core enrichment wiring
  - type: co_occurrence
    source_relationship: exhibits
    output_relationship: co_exhibited
  - type: temporal_proximity
    timestamp_property: gesture_time
    threshold_ms: 500
    output_relationship: temporal_co_occurrence

emit:                             # required: Layer 2 mapping
  - for_each:
      collection: "{input.gestures}"
      emit:
        - create_node:
            id: "concept:{item.quality | lowercase}"
            type: concept
            dimension: semantic
        - create_edge:
            source: "{source_node_id}"
            target: "concept:{item.quality | lowercase}"
            relationship: exhibits
            contribution: "{item.confidence}"
  - create_provenance:
      chain_id: "chain:{adapter_id}:{input.session_id}"
      mark_annotation: "{input.session_label}"
      mark_file: "{input.source_file}"
      mark_line: 1
      tags: "{input.qualities}"
```

### `DeclarativeAdapter::from_yaml()`

A new constructor that deserializes a YAML string into a `DeclarativeSpec` and validates it:

```rust
impl DeclarativeAdapter {
    pub fn from_yaml(yaml: &str) -> Result<Self, AdapterError> { ... }
}
```

This requires adding `serde::Deserialize` derives to `DeclarativeSpec`, `Primitive`, and all primitive structs. The existing `DeclarativeAdapter::new(spec)` constructor handles validation (Invariant 7 dual obligation check); `from_yaml` handles deserialization then delegates to `new`.

## Consequences

**Positive:**

- A single YAML artifact captures the full adapter integration: extraction (ensemble), mapping (emit), and enrichment wiring (enrichments). Self-documenting.
- External consumers write YAML, not Rust. The `ensemble` field connects to their existing llm-orc extraction pipeline; the `enrichments` section connects to Plexus's discovery capabilities.
- `from_yaml()` enables runtime loading of adapter specs from files or MCP tool parameters — no recompilation needed to add a new adapter.
- `update_properties` completes the primitive set, enabling declarative expression of external enrichment result application.

**Negative:**

- `DeclarativeAdapter` gains llm-orc as an optional dependency. When `ensemble` is specified, `LlmOrcClient` must be injected at construction time. This means `from_yaml` alone is insufficient — a builder pattern or factory that accepts the client is needed.
- The `enrichments` type registry is a hardcoded match. Adding a new core enrichment type requires updating the match arms. This is intentional (enrichments are engine-internal, not consumer-extensible) but means the registry is not self-extending.

**Neutral:**

- The existing Rust adapters (FragmentAdapter, SemanticAdapter, GraphAnalysisAdapter) are unaffected. DeclarativeAdapter is an additional path, not a replacement.
- The `input_schema` validation remains optional. Specs without a schema accept any JSON input. This is consistent with ADR-020.
