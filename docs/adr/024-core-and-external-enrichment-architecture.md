# ADR-024: Core and External Enrichment Architecture

**Status:** Accepted

**Research:** [Essay 19](../essays/19-declarative-adapter-primitives.md)

**Domain model:** [domain-model.md](../domain-model.md) — core enrichment, external enrichment, discovery gap, latent evidence, structural evidence

**Amends:** ADR-022 (parameterized enrichments — adds two core enrichments, cancels Tier 1 deferral), ADR-023 (graph analysis — renames to external enrichment, adds emission trigger)

**Depends on:** ADR-010 (enrichment trait and loop), ADR-012 (unified ingest pipeline), ADR-021 (llm-orc integration)

---

## Context

ADR-022 defined a three-tier enrichment model: Tier 0 (parameterized Rust built-ins), Tier 1 (declarative YAML enrichments, deferred), Tier 2 (batch graph analysis via llm-orc). ADR-023 defined graph analysis as a concept distinct from enrichment.

Essay 19 research reframed the enrichment question from "what tiers do we need?" to "what discovery affordances should the enrichment system provide?" This reframing, driven by EDDI's real-time performance requirements and Open Question 14's embedding possibilities, produced two findings:

1. **Two general graph algorithms are missing.** Discovery gap (latent-structural disagreement) and temporal proximity (timestamp-based co-occurrence) are domain-agnostic patterns that every consumer benefits from. Both require reactive, sub-millisecond performance — ruling out anything outside native Rust. Discovery gap cannot be expressed as co-occurrence because it checks for the *absence* of connections (a negative structural query).

2. **The three-tier model described implementation details, not architecture.** The deferred "Tier 1 declarative enrichment DSL" (`match`/`find_nodes`/`guard`/`emit`) was designing a new language for something the llm-orc ensemble YAML already handles. A "declarative enrichment" and a "graph analysis flow" are the same thing with different triggers. The real distinction is simpler: core (Rust, fast, reactive) vs external (llm-orc, background, custom).

## Decision

### Two categories, not three tiers

The enrichment architecture has two categories:

**Core enrichments** are Rust-native general graph algorithms. Reactive (fire in the enrichment loop per Invariant 36), fast (microseconds), parameterizable, and domain-agnostic. Four core enrichments:

| Enrichment | Pattern | Affordance | ID format |
|------------|---------|------------|-----------|
| TagConceptBridger | tag matching | "this mark is about this concept" | `tag_bridger:{relationship}` |
| CoOccurrenceEnrichment | shared sources | "these concepts appear together" | `co_occurrence:{source}:{output}` |
| DiscoveryGapEnrichment | latent-structural delta | "these should be connected but aren't" | `discovery_gap:{trigger}:{output}` |
| TemporalProximityEnrichment | timestamp proximity | "these happened together" | `temporal:{property}:{threshold}:{output}` |

**External enrichments** are llm-orc ensembles. Run outside the enrichment loop (Invariant 49), results re-enter via `ingest()`. Two trigger modes:

- **On-demand:** `plexus analyze` or MCP tool invocation. Existing behavior (ADR-023).
- **Emission-triggered:** background task spawned when new data enters the graph. Results re-enter via `ingest()`, triggering core enrichments on the new data.

Emission-triggered external enrichments are always background — they cannot block the enrichment loop. This creates a layered response: immediate (core, microseconds) → background (external, seconds) → delayed (core again on new data).

### DiscoveryGapEnrichment

```rust
pub struct DiscoveryGapEnrichment {
    trigger_relationship: String,    // e.g., "similar_to"
    output_relationship: String,     // e.g., "discovery_gap"
    id: String,
}
```

Algorithm in `enrich()`:
1. Filter events for `EdgesAdded` where any edge has `relationship == trigger_relationship`
2. For each new trigger edge (A, B): check if any other edge exists between A and B in the context
3. Guard: no `output_relationship` edge already exists between A and B
4. Emit: `output_relationship` symmetric edge pair with the trigger edge's contribution as weight

Parameterizable in adapter spec YAML:
```yaml
enrichments:
  - type: discovery_gap
    trigger_relationship: similar_to
    output_relationship: discovery_gap
```

### TemporalProximityEnrichment

```rust
pub struct TemporalProximityEnrichment {
    timestamp_property: String,      // e.g., "created_at", "gesture_time"
    threshold_ms: u64,
    output_relationship: String,     // e.g., "temporal_proximity"
    id: String,
}
```

Algorithm in `enrich()`:
1. Filter events for `NodesAdded`
2. For each new node: read `timestamp_property` from its properties
3. Scan context for other nodes with `timestamp_property` within `threshold_ms`
4. Guard: no `output_relationship` edge already exists between the pair
5. Emit: `output_relationship` symmetric edge pair

Parameterizable in adapter spec YAML:
```yaml
enrichments:
  - type: temporal_proximity
    timestamp_property: gesture_time
    threshold_ms: 500
    output_relationship: temporal_co_occurrence
```

### Tier 1 declarative enrichment DSL: cancelled

The `match`/`find_nodes`/`guard`/`emit` DSL described in ADR-022 §Tier 1 is unnecessary. The llm-orc ensemble YAML already serves this purpose. Any computation pattern that requires more than the four core enrichments is expressed as an external enrichment (llm-orc ensemble) with an on-demand or emission trigger.

### Emission trigger for external enrichments

The adapter spec YAML can declare external enrichments with an emission trigger:

```yaml
external_enrichments:
  - ensemble: deep-semantic-analysis
    trigger: emission
```

When an emission is committed for this adapter, the named ensemble is spawned as a background task. Its results re-enter via `ingest()`. This is the same pipeline as on-demand external enrichments (ADR-023), with different scheduling.

The emission trigger mechanism is design-deferred — the decision here is that the trigger is architecturally valid and follows the existing `ingest()` result path. The implementation details (debouncing, concurrency limits, context serialization for the background task) are build-phase concerns.

## Consequences

**Positive:**

- Four core enrichments cover the known discovery affordance landscape without LLM dependency — fast enough for EDDI's real-time performance needs
- The architecture is simpler: two categories (core/external) instead of three tiers
- No new DSL to design, implement, or document — external enrichments reuse the proven llm-orc ensemble infrastructure
- Emission-triggered external enrichments enable the layered response pattern (immediate → background → delayed) without blocking the enrichment loop

**Negative:**

- Each new general graph algorithm requires Rust code. The trade-off is performance — core enrichments must be sub-millisecond for reactive use cases like EDDI
- DiscoveryGapEnrichment only fires when `similar_to` edges enter the graph. Until embedding infrastructure exists (OQ-14), it has no trigger data. Building it now validates the pattern; it becomes useful when embeddings arrive
- TemporalProximityEnrichment scans the full context for timestamp matches on each new node. For contexts with many timestamped nodes, this is O(n) per new node. Acceptable for EDDI's session-sized contexts; may need indexing for large contexts

**Neutral:**

- ADR-023's "graph analysis" concept is now "external enrichment (on-demand)." The pipeline, data contracts, and llm-orc integration are unchanged — only the vocabulary and the addition of emission triggers
- OQ-13 (declarative enrichment termination guarantees) remains resolved — its constraints now apply to core enrichments generally, not a deferred Tier 1 DSL
