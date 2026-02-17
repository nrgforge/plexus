# ADR-027: Contribution Retraction

**Status:** Proposed

**Research:** [Essay 20](../essays/20-embedding-infrastructure.md)

**Domain model:** [domain-model.md](../domain-model.md) — contribution, raw weight, adapter ID, scale normalization

**Extends:** ADR-003 (reinforcement mechanics — adds a removal primitive to the contribution lifecycle)

**Depends on:** ADR-003 (per-adapter contribution tracking), ADR-012 (unified ingest pipeline)

---

## Context

ADR-003 defines per-adapter contributions on edges: each adapter/enrichment occupies a named slot (`HashMap<AdapterId, f32>`), and the engine combines them via scale normalization. Contributions can increase or decrease via re-emission (latest-value-replace). But there is no mechanism to *remove* a contribution entirely — to say "this adapter/enrichment no longer has an opinion about this edge."

Essay 20 surfaced this gap in the context of model-aware provenance. When enrichment and adapter IDs encode model identity (e.g., `embedding:nomic-embed-text-v1.5`, `extract-semantic:mistral-7b`), users need the ability to:

1. **Replace a model:** retract old model's contributions, re-run with new model
2. **Compare models:** run both, inspect contributions, then retract the one that performed worse
3. **Clean up experiments:** remove a model's influence after A/B testing

ADR-003 Decision 1 notes that "each adapter writes only to its own slot" and that contributions can decrease. But decreasing to zero is not the same as removing — a zero-valued contribution still occupies a slot, participates in scale normalization (as the adapter's minimum), and counts toward evidence diversity queries. True removal requires deleting the slot.

## Decision

### ContributionRetraction emission type

A new variant in `Emission`:

```rust
pub struct ContributionRetraction {
    pub adapter_id: AdapterId,
}
```

Semantics: remove the named adapter/enrichment's contribution slot from every edge in the context. This is a batch operation — it affects all edges, not a single edge.

**Per-edge retraction considered and rejected.** An alternative design would allow retracting a specific adapter's contribution from a single edge. This adds API complexity for a use case that has a simpler workaround: re-emit the edge with a near-zero contribution value. The batch operation covers the primary use cases (model replacement, experiment cleanup) where you want to remove *all* influence of a specific adapter/model. Per-edge granularity can be revisited if a use case emerges.

### Processing in emit_inner

ContributionRetraction is an engine operation, not consumer data. It does not enter through `ingest()` — it has no `input_kind`, no adapter, and no domain data to transform. Instead, retraction is exposed as a dedicated engine API method: `retract_contributions(context_id, adapter_id)`. This is consistent with the design principle that `ingest()` is for domain data from consumers (Invariant 34), while retraction is an administrative operation on the graph's internal accounting.

The MCP server and CLI expose retraction as a tool/command; transports can expose it alongside `ingest()` as a separate endpoint. The operation is context-scoped (consistent with Invariant 33's context-scoped design principle).

ContributionRetraction is processed as a new phase before the existing emission phases:

1. **Phase 0: Retractions.** For each retraction, iterate all edges in the context and remove the named adapter's contribution slot from the contributions map.
2. **Existing phases proceed as before** (nodes, edges, property updates).
3. **recompute_raw_weights()** at the end recalculates all raw weights from the remaining contributions.

### Edge pruning

After retraction and recomputation, edges whose contributions map is empty have a raw weight of 0.0. These edges are pruned — removed from the context. An edge with no contributions is an edge with no evidence.

Pruning fires `EdgesRemoved` events, which trigger the enrichment loop. Note on cascade behavior: retracting an embedding model's contributions (e.g., `embedding:nomic-embed-text-v1.5`) removes `similar_to` edges that model produced. However, `discovery_gap` edges produced by DiscoveryGapEnrichment are attributed to a *different* enrichment ID (e.g., `discovery_gap:similar_to:discovery_gap`), so they are **not** directly retracted. Instead, `EdgesRemoved` fires and DiscoveryGapEnrichment re-evaluates — pairs that no longer have latent evidence lose their discovery gap edges through the enrichment's own idempotency check (the `similar_to` edge is gone, so the gap condition no longer holds, and the enrichment does not re-emit). This is indirect cleanup via the enrichment loop, not direct cascade from retraction.

### Graph event

A new graph event type: `ContributionsRetracted { adapter_id: AdapterId, edges_affected: usize }`. This allows enrichments and outbound event transformers to react to retraction.

### Model replacement workflow

```
1. Retract:  retract_contributions(context, "embedding:nomic-embed-text-v1.5")
             → removes old contribution slots from all edges
             → prunes zero-evidence edges
             → EdgesRemoved fires → enrichments react

2. Re-embed: ingest(new similar_to edges from "embedding:nomic-embed-text-v2.0")
             → new contribution slots created
             → EdgesAdded fires → DiscoveryGapEnrichment reacts

3. Result:   graph reflects new model's understanding
```

This applies identically to LLM extraction model replacement (`extract-semantic:mistral-7b` → `extract-semantic:claude-sonnet-4-5`).

### Model-aware adapter and enrichment IDs

Adapter and enrichment IDs that involve a model should encode the model name:

| Component | ID format | Example |
|-----------|-----------|---------|
| Embedding enrichment | `embedding:{model_name}` | `embedding:nomic-embed-text-v1.5` |
| Semantic extraction adapter | `extract-semantic:{model_name}` | `extract-semantic:claude-sonnet-4-5` |
| Graph analysis (no model) | `graph-analysis:{algorithm}` | `graph-analysis:pagerank` |
| Core enrichment (no model) | `{type}:{params}` | `co_occurrence:tagged_with:may_be_related` |

This is a convention, not a structural requirement. The contribution tracking machinery doesn't parse adapter IDs — it treats them as opaque strings. The convention ensures that different models produce different contribution slots automatically.

Invariant 13 (stable adapter IDs) is satisfied: the same model name produces the same ID across sessions. A new model version uses a new name, producing a new ID — which is the intended behavior (new contribution slot, old slot retractable).

## Consequences

**Positive:**

- Model replacement becomes a first-class operation — retract, re-run, done
- Multi-model A/B testing: run both, compare, retract the loser
- Edge pruning after retraction keeps the graph clean — no ghost edges with zero evidence
- The mechanism is adapter/enrichment-agnostic — works for embeddings, LLM extraction, or any future model-based computation

**Negative:**

- Retraction iterates all edges in the context — O(E) where E is edge count. For large contexts this may be slow. Mitigation: an index from adapter ID to edge set (deferred optimization)
- Edge pruning after retraction triggers enrichment re-evaluation: removing `similar_to` edges causes DiscoveryGapEnrichment to re-check affected pairs. Discovery gap edges are cleaned up indirectly (enrichment idempotency, not direct cascade), but the end result is the same — discovery gaps that depended on retracted latent evidence are removed
- No undo for retraction. Once an adapter's contributions are removed, the only way to restore them is to re-run the adapter/enrichment. This is acceptable — retraction is an intentional operation

**Neutral:**

- ADR-003's contribution lifecycle is now: add (emit) → update (re-emit, latest-value-replace) → remove (retract). The three operations cover the full lifecycle
- ContributionRetraction is context-scoped, consistent with the context-scoped design principle (Invariant 33: context-scoped EngineSink)
- The retraction primitive could also serve federated contexts (ADR-018): when a peer's adapter is removed from a shared context, retract its contributions. This is a future consideration, not a current requirement
