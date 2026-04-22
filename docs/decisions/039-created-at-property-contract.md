# ADR-039: `created_at` Property Contract for Timestamp-Based Enrichments

**Status:** Accepted

**Research:** Spike 2 Observation B (`docs/housekeeping/spikes/spike-default-install-intent.md`, 2026-04-17); PLAY field notes Finding 1 (`docs/essays/reflections/field-notes.md`, 2026-04-16)

**Product discovery:** Product Debt row *"TemporalProximity enrichment reads a node property adapters don't write"*

**Domain model:** [domain-model.md](../domain-model.md) — node (carries `properties`), core enrichment, TemporalProximityEnrichment, adapter, context

**Depends on:** ADR-024 (core enrichment architecture — TemporalProximityEnrichment is one of the four core enrichments)

---

## Context

`TemporalProximityEnrichment` is a core enrichment (ADR-024) parameterized with a timestamp property key (default `"created_at"`) and a time threshold. Its algorithm reads `node.properties[timestamp_property]` from each node and emits a symmetric edge pair for any pair of nodes whose timestamps fall within the threshold window.

PLAY Finding 1 and Spike 2 Observation B identified the bug: `with_default_enrichments()` registers `TemporalProximityEnrichment` reading `node.properties["created_at"]`, but no built-in adapter writes that property. The `created_at` value the adapters set goes to **node metadata**, not **node properties**. The enrichment runs on every emission, iterates nodes, finds no `created_at` in properties, and silently emits nothing.

The bug is independent of ADR-038's release-binary decision and of ADR-040's DiscoveryGap trigger decoupling. It is a property-contract inconsistency between the producers of timestamp data (adapters) and the consumers of it (the enrichment). Two surfaces exist for time information on a node — metadata and properties — and the built-in adapters populate the former while the built-in enrichment reads the latter.

The decision this ADR makes is which surface is authoritative for timestamp-based enrichments, and how both sides are brought into alignment.

## Decision

### `node.properties["created_at"]` is the authoritative surface for timestamp-based enrichments

Node properties (the `HashMap<String, serde_json::Value>` on `Node`) are the declared surface for data that enrichments and queries operate on. Node metadata is framework bookkeeping — identifiers, timestamps, bookkeeping fields — used by the engine and the persistence layer, not by enrichments. This ADR affirms that separation and places timestamp data that is **semantically meaningful to enrichments or queries** on `properties`.

Timestamp-based enrichments read `node.properties["created_at"]` as an ISO-8601 string (e.g., `"2026-04-16T12:34:56Z"`). Other enrichments that operate on temporally-qualified data may declare their own property keys in their parameterization (e.g., `gesture_time` for EDDI). The convention is: the property key is declared in the enrichment's parameterization, and the adapter that produces the nodes the enrichment operates on is responsible for populating that key.

### Built-in adapters write `node.properties["created_at"]`

Built-in adapters that create nodes that `TemporalProximityEnrichment` operates on populate `node.properties["created_at"]` at node creation time with the current ingest timestamp in ISO-8601 UTC format. Specifically:

- `ContentAdapter` — writes `created_at` on fragment nodes and on concept nodes created from fragment tags.
- `ExtractionCoordinator` (registration phase) — writes `created_at` on file nodes and on the `extraction_status` node.
- `DeclarativeAdapter` — when a spec declares a node-producing primitive (`create_node`, `for_each`), the adapter writes `created_at` on each created node unless the spec's emit explicitly sets the property (in which case the spec-authored value wins).

Metadata timestamps (if any) remain as framework bookkeeping and are not reshaped by this ADR. The two surfaces can coexist: metadata for bookkeeping, properties for semantic use.

### `TemporalProximityEnrichment` reads from properties, not metadata

The enrichment's code reads `node.properties[timestamp_property]`. It does not fall back to metadata. If the property is missing, the node is not considered by the enrichment — this preserves the graceful-degradation contract (a node without a declared timestamp simply does not participate in temporal proximity, and the enrichment emits nothing for it).

### Spec authors using `TemporalProximityEnrichment` must write the declared property

A declarative adapter spec that parameterizes `TemporalProximityEnrichment` on a non-default property key is responsible for writing that key on the nodes it creates. The spec validator (Invariant 60) **does not** statically verify this coupling — there is no cross-primitive validation that a `create_node` emits a property that a declared enrichment reads. This is a known limitation; a runtime absent-property observation falls into the graceful-degradation path.

**Failure mode for spec authors:** a mismatch produces a **silent-idle enrichment** — the enrichment is registered, the enrichment loop calls it, and it returns no mutations every round because no node it sees has the property it reads. Nothing in the runtime surfaces this to the author. Spec-author documentation must name this failure mode explicitly with that label ("silent-idle") and include a diagnostic checklist ("if your declared `TemporalProximityEnrichment` never produces edges, verify the nodes you emit carry the property key you declared"). This is the BUILD deliverable that makes the "graceful degradation" label honest in the spec-authoring context — where the degradation is the author's omission, not a framework capability gap.

### ISO-8601 UTC string as the on-wire format

`created_at` is stored as an ISO-8601 UTC string (not a Unix timestamp, not a `chrono::DateTime` serialization variant). Rationale: node properties are `serde_json::Value`, which represents datetimes most portably as strings; ISO-8601 is the widest-compatibility format across JSON consumers and across the MCP surface. The enrichment parses the string to `chrono::DateTime<Utc>` at comparison time. If parsing fails, the node is skipped for that enrichment run (graceful degradation).

## Consequences

**Positive:**

- `TemporalProximityEnrichment` actually fires in the default build once `with_default_enrichments()`'s adapter side is updated — the silent-dead enrichment observed in PLAY becomes live.
- The property-surface / metadata-surface distinction is now explicit: enrichments and queries operate on properties; metadata is framework bookkeeping. Future enrichments and query additions follow this boundary without needing to rediscover it.
- ISO-8601 UTC strings are portable across the MCP surface, SQLite storage, and JSON-based declarative specs.
- Graceful degradation (nodes without the property are skipped, not errored) maintains the enrichment-loop contract — an enrichment that cannot react emits nothing, not an error.

**Negative:**

- All built-in adapters that create nodes that may participate in temporal proximity must be touched to write `created_at`. This is a mechanical change and is covered by scenarios.
- The spec-author coupling (spec declares an enrichment that reads property X → spec's `create_node` primitives must write property X) is not statically validated. A spec author can declare `TemporalProximityEnrichment` without writing timestamps and get a silently-dead enrichment. Documentation is the mitigation; a future ADR could add cross-primitive validation if this becomes a recurring footgun.
- Metadata and properties now overlap for `created_at` (framework metadata may also carry the timestamp). This is redundancy, not inconsistency — both surfaces can coexist. A future pass could eliminate the metadata copy if it proves unused.

**Neutral:**

- ADR-024 is unaffected — `TemporalProximityEnrichment` remains a core enrichment with the same algorithm; only the read surface and the producer side change.
- ADR-038 is unaffected — the `created_at` bug is independent of the release-binary decision; the fix lands in both Homebrew and `features = ["embeddings"]` builds.
- ADR-040 is unaffected — DiscoveryGap's trigger-sources decoupling is independent of timestamp handling.

## Provenance

**Drivers:**
- Spike 2 Observation B (`docs/housekeeping/spikes/spike-default-install-intent.md`) — identified the bug as a property-contract inconsistency independent of the embedding/release-binary decision; named the two resolution options (adapters write the property; enrichment reads metadata) and concluded BUILD should fix it in a `fix:` commit rather than an architectural ADR would demand.
- PLAY field notes Finding 1 (`docs/essays/reflections/field-notes.md`) — grounded the problem as observed in a real Homebrew build.
- Product Debt row on TemporalProximity property contract — named the gap in product discovery and routed it to DECIDE.

This ADR chose "adapters write `created_at` to properties" (spike option 1) over "enrichment reads metadata" (spike option 2) because properties are already the declared surface for enrichment/query data and metadata is framework bookkeeping; keeping the read side on properties aligns with the long-term direction and means future enrichments do not need to learn a metadata-read pathway.
