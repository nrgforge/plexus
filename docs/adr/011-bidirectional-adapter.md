# ADR-011: Bidirectional Adapter with Outbound Events

**Status:** Proposed

## Context

The adapter currently has one direction: inbound (domain data → graph mutations via `process()`). Consumers send data in but have no way to hear back what happened. Graph events are produced during emission and then discarded — no listener, no delivery.

Essay 09 found that every consumer needs events back: Trellis needs "concepts detected," EDDI needs "weights changed for gesture X," Manza needs "new code pattern linked to design concept." But consumers should never see raw graph events (`NodesAdded`, `EdgesAdded`). They need domain-meaningful outbound events translated into their own language.

The adapter is the natural place for this translation — it already knows the consumer's domain. Making the adapter bidirectional makes it the complete integration contract: the single artifact that defines a consumer's relationship with Plexus.

## Decision

Add an outbound method to the `Adapter` trait:

```rust
trait Adapter: Send + Sync {
    fn id(&self) -> &str;
    fn input_kind(&self) -> &str;

    // Inbound: domain data → graph mutations
    async fn process(&self, input: &AdapterInput, sink: &dyn AdapterSink)
        -> Result<(), AdapterError>;

    // Outbound: raw events → domain events for consumer
    fn transform_events(&self, events: &[GraphEvent], context: &Context)
        -> Vec<OutboundEvent> {
        vec![] // default: no outbound events
    }
}
```

### OutboundEvent

A new struct representing a domain-meaningful event for the consumer:

```rust
struct OutboundEvent {
    kind: String,
    detail: String,
}
```

Intentionally simple. The consumer defines what `kind` values it cares about. The adapter's `transform_events()` filters and translates.

### Event flow

After the enrichment loop (ADR-010) completes, the framework calls `adapter.transform_events()` with:
- All accumulated graph events from the primary emission and all enrichment rounds
- A context snapshot

The adapter filters what its consumer cares about. This enables cross-pollination visibility: if an enrichment creates a bridge relevant to this consumer, the adapter can surface it.

### Default is no-op

The default implementation returns an empty vec. Existing adapters (FragmentAdapter) don't break. CoOccurrenceEnrichment is unaffected — enrichments don't have outbound methods. Outbound is opt-in.

## Consequences

**Positive:**

- The adapter is the complete integration contract — data in, events out, consumer never sees graph internals
- Consumers receive domain-meaningful events without understanding graph primitives
- Cross-pollination visibility: an adapter can surface events from other adapters' or enrichments' mutations
- Backward compatible: default no-op means existing code doesn't break

**Negative:**

- Outbound events are synchronous with the ingest pipeline — the consumer receives events as part of the ingest response. For consumers that want async delivery (push notifications, event streams), this is insufficient. Mitigated by: event cursors (domain model OQ8) can provide async delivery later; the outbound events are still produced and can be persisted.
- Cross-pollination visibility (adapter A's consumer learning about adapter B's mutations) requires event cursors (OQ8). Under the current synchronous model, a consumer only receives outbound events from its own ingestion. The essay's cross-pollination promise is deferred until event persistence is built.

**Neutral:**

- `OutboundEvent` is deliberately unstructured (string kind + string detail). A more typed approach (enum variants per consumer) would provide compile-time safety but couple the framework to consumer-specific types. The current approach keeps the framework domain-agnostic.
