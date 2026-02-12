# ADR-010: Enrichment Trait and Enrichment Loop

**Status:** Proposed

## Context

Mutations in one dimension of the graph should trigger effects in other dimensions. Tag-to-concept bridging (ADR-009) is the first example: a mark tagged "travel" should automatically get a `references` edge to `concept:travel`. The codebase handled this inline in `ProvenanceApi::add_mark()`. Co-occurrence detection was a reflexive adapter with a `ProposalSink` wrapper. These are two patterns for the same kind of problem — reactive graph intelligence.

Essay 09 identified that both patterns (and any future cross-dimensional bridging) belong to a single mechanism: **enrichments** — reactive components that respond to graph events and produce additional mutations. The enrichment model supersedes the reflexive adapter and ProposalSink concepts (see domain model, resolved questions 2–3).

## Decision

### Enrichment trait

A new `Enrichment` trait, deliberately separate from `Adapter`:

```rust
trait Enrichment: Send + Sync {
    fn id(&self) -> &str;
    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission>;
}
```

- `id()` returns a stable identifier for contribution tracking and deduplication.
- `enrich()` receives the accumulated graph events from the previous round and a context snapshot. Returns `Some(Emission)` if there is work to do, `None` if quiescent.
- No `input_kind`. No domain data. No consumer. Enrichments bridge between dimensions within the graph.

### Enrichment loop

After each primary emission (adapter → sink → commit → events), the engine runs the enrichment loop:

1. Snapshot the context.
2. Pass the current round's events and snapshot to each registered enrichment. All enrichments in a round see the same snapshot.
3. Collect all returned emissions. Commit each via the engine (same commit path as adapter emissions — contribution tracking, scale normalization, event firing all apply). The enrichment's `id()` is the adapter ID for contribution tracking.
4. Collect the new events from step 3. These become the input events for the next round.
5. Repeat from step 1 with the new events until all enrichments return `None` — quiescence.

Events are per-round, not accumulated: round N sees only events from round N-1 (or from the primary emission, for round 0). This prevents enrichments from re-processing events they've already seen.

### Registration

Enrichments are registered globally on the engine, not per-adapter. They self-select based on events and context state. Enrichments shared across integrations are deduplicated by `id()`.

### Termination

The enrichment loop terminates via idempotency. Each enrichment checks context state before emitting — if the desired edges already exist, it returns `None`. The framework runs the loop; the enrichment implements the termination condition.

`EdgesAdded` fires for every committed edge including re-emissions. Enrichments must not rely on events alone to detect novelty — they must check context state.

### Safety valve

The enrichment loop enforces a maximum round count (default: 10). If quiescence is not reached within the limit, the loop aborts and logs a warning. This prevents a buggy enrichment from causing an infinite loop. The limit is configurable but its existence is a framework guarantee.

### Migration

- `CoOccurrenceAdapter` becomes `CoOccurrenceEnrichment`. Algorithm unchanged; trigger model changes from schedule-based to event-driven.
- `ProposalSink` is removed. The "propose, don't merge" principle survives as a design convention: `CoOccurrenceEnrichment` self-caps contributions and only emits `may_be_related` edges.
- Tag-to-concept bridging moves from inline code in `ProvenanceApi::add_mark()` to a `TagConceptBridger` enrichment.

## Consequences

**Positive:**

- One mechanism for all reactive graph intelligence (tag bridging, co-occurrence, future topology detection)
- Built-in termination via quiescence — resolves the former open question about reflexive adapter cycle convergence
- Simpler model: enrichment replaces reflexive adapter + ProposalSink + schedule monitor (three unbuilt or partially-built concepts)
- Enrichments compose: TagConceptBridger and CoOccurrenceEnrichment can both fire in the same loop without coordination

**Negative:**

- Enrichments run after every emission, not on a schedule. For expensive enrichments (full co-occurrence scan), this could be wasteful during burst ingestion. Mitigated by self-selection: the enrichment checks events for relevance before doing expensive work.
- No structural enforcement of constraints (ProposalSink is removed). An enrichment could emit any relationship type. Mitigated by enrichments being framework-level code, not user-provided plugins.

**Neutral:**

- The enrichment loop adds latency to each `emit()` call. For the current enrichments (tag bridging, co-occurrence), the cost is small — context snapshot + a few edge checks. If enrichments become expensive, batching or async enrichment can be added without changing the trait.
