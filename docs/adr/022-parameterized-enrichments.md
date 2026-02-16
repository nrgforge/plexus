# ADR-022: Parameterized Enrichments

**Status:** Proposed

**Research:** [Essay 18](../essays/18-phased-extraction-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — parameterized enrichment, declarative enrichment, co-occurrence

**Depends on:** ADR-010 (enrichment trait and loop), ADR-012 (unified ingest pipeline — integration registration model)

---

## Context

The two existing enrichments — TagConceptBridger and CoOccurrenceEnrichment — are generic graph patterns that happen to be hardcoded to specific relationships. CoOccurrenceEnrichment's algorithm is: "find target nodes that share a source via relationship X → bridge them with relationship Y." Currently hardcoded to `tagged_with` → Concept → `may_be_related`.

Essay 18 found that domain-specific enrichments (EDDI's "MovementBridger," Carrel's citation co-occurrence) are not new algorithms — they are the same co-occurrence algorithm configured with different relationships:

| Application | Source relationship | Target type | Output relationship |
|---|---|---|---|
| Fragment (current) | `tagged_with` | Concept | `may_be_related` |
| EDDI | `exhibits` | movement-quality | `co_exhibited` |
| Carrel | `cited_by` | Paper | `co_cited` |

No new enrichment code is needed for these cases.

## Decision

### Tier 0: Parameterized built-ins

CoOccurrenceEnrichment and TagConceptBridger accept configuration parameters instead of hardcoding relationship types. A parameterized instance is declared in the adapter spec:

```yaml
enrichments:
  - type: co_occurrence
    params:
      edge_relationship: "exhibits"
      output_relationship: "co_exhibited"
```

Each parameterized instance gets a unique enrichment ID (e.g., `co_occurrence:exhibits:co_exhibited`) for deduplication (Invariant 39). The instance runs in the enrichment loop with full graph-wide reactivity — scanning the full context snapshot on every emission round.

### Structure-aware, not type-aware

Enrichments fire based on graph structure (edge relationships, dimension membership) rather than node content type. CoOccurrenceEnrichment fires for any pair of nodes connected via the configured relationship, regardless of which adapter produced the source nodes. (Invariant 50.)

This means Sketchbin doesn't need a custom enrichment at all — if its adapter uses `tagged_with` edges, the existing CoOccurrenceEnrichment fires automatically.

### Tier 1: Declarative enrichments (design deferred)

For bridging patterns not covered by parameterized built-ins, a YAML spec with `match`, `find_nodes`, `guard`, and `emit` primitives. Still graph-wide and reactive. Implementation deferred — the parameterized built-ins cover the known use cases. The design is sketched (Essay 18 §Three tiers) but not detailed enough for an ADR. Deferred pending resolution of Open Question 13 (declarative enrichment termination guarantees) in the domain model.

## Consequences

**Positive:**

- Domain-specific enrichments without writing Rust — configure existing algorithms with new parameters
- Structure-aware firing means new domains automatically benefit from existing enrichments when they use standard relationship patterns
- Adapter spec YAML declares which enrichments to register — self-documenting integration

**Negative:**

- Broader trigger scope (any source node, not just fragments) must be verified for quiescence. Open question 12 tracks this.
- Parameterized enrichment IDs must be unique and stable. Two adapter specs registering the same parameterized enrichment with the same parameters should deduplicate; different parameters should produce distinct instances.

**Neutral:**

- The existing hardcoded enrichments become the default parameterization. CoOccurrenceEnrichment with no explicit config uses `tagged_with` / `may_be_related` — backward compatible.
- Tier 1 (declarative enrichments) is explicitly deferred. This ADR only decides Tier 0 (parameterized built-ins).
