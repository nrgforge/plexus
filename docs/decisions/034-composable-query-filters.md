# ADR-034: Composable Query Filters

**Status:** Accepted

**Research:** [Essay 001](../essays/001-query-surface-design.md)

**Domain model:** [domain-model.md](../domain-model.md) — query, evidence diversity (corroboration), provenance-scoped filtering, lens, normalized weight, raw weight

**Depends on:** ADR-013 (typed multi-hop traversal), ADR-033 (lens declaration)

**Resolves:** OQ-23 (provenance-scoped query implementation)

---

## Context

Invariant 59 requires provenance-scoped filtering composable with any query primitive (`find_nodes`, `traverse`, `find_path`, `evidence_trail`). Essay 001 identifies evidence diversity — user-facing term: **corroboration** — as a ranking dimension alongside raw weight and normalized weight, derived at query time from contributions.

The current query structs (`FindQuery`, `TraverseQuery`, `PathQuery`, `StepQuery`) accept structural filters (node type, dimension, relationship, min weight) but have no provenance filter or ranking parameters. The query module operates on in-memory `Context` data — `HashMap<NodeId, Node>` + `Vec<Edge>` — building temporary edge indexes at query time.

Two design axes need decisions:

1. **Filtering**: how provenance constraints compose with existing query primitives
2. **Ranking**: how corroboration participates in result ordering

## Decision

### QueryFilter: an optional composable filter struct

A new `QueryFilter` struct composes with any query primitive:

```rust
pub struct QueryFilter {
    /// Only include edges with a contribution from at least one of these IDs
    pub contributor_ids: Option<Vec<String>>,
    /// Only include edges whose relationship starts with this prefix
    pub relationship_prefix: Option<String>,
    /// Minimum corroboration: edges must have at least this many distinct contributors
    pub min_corroboration: Option<usize>,
}
```

Each query struct gains an optional `filter` field:

```rust
pub struct TraverseQuery {
    pub origin: NodeId,
    pub max_depth: usize,
    pub direction: Direction,
    pub relationship: Option<String>,  // existing: exact match
    pub min_weight: Option<f32>,       // existing
    pub filter: Option<QueryFilter>,   // new: composable filter
}
```

When `filter` is `None`, queries behave identically to today — zero cost when unused. When present, edges are filtered before traversal, path computation, or result collection.

### Filter semantics

- **`contributor_ids`**: an edge passes if `edge.contributions.keys()` intersects with the provided IDs. This is provenance-scoped filtering — "show me only edges that this adapter or enrichment contributed to." For lens-scoped queries: `contributor_ids: ["lens:trellis:thematic_connection:may_be_related"]`. Note: `contributor_ids` matches on the contributions map (which adapter contributed), while `relationship_prefix` matches on the edge's relationship type field (what the edge is called). For lens-created edges, these overlap but diverge: an edge with `relationship = "lens:trellis:thematic_connection"` might have contributions from both the lens and another adapter that independently strengthened the same edge. `relationship_prefix` returns it regardless of contributor; `contributor_ids` returns it only if the specified contributor is present.
- **`relationship_prefix`**: an edge passes if `edge.relationship.starts_with(prefix)`. For lens-scoped queries: `relationship_prefix: "lens:trellis:"`. Catches all of a consumer's lens translations without enumerating each relationship type.
- **`min_corroboration`**: an edge passes if `edge.contributions.len() >= min_corroboration`. This is corroboration filtering — "show me only connections supported by at least N independent sources." A `min_corroboration` of 3 means three distinct adapters, enrichments, or lenses contributed to the edge.

`min_weight` remains on individual query structs (`TraverseQuery`, `FindQuery`, etc.) where it already exists. `QueryFilter` owns only the provenance-dimensional and corroboration filters that are genuinely new capabilities. This avoids a dual-presence ambiguity where two fields on the same query mean "minimum edge weight" with unclear precedence.

All filter fields are AND-composed: an edge must pass all non-`None` predicates.

### Corroboration as a ranking dimension

Query results support ranking by a `RankBy` enum:

```rust
pub enum RankBy {
    /// Raw weight (sum of contributions after scale normalization)
    RawWeight,
    /// Number of distinct contributors to the edge
    Corroboration,
    /// Normalized weight via the given strategy
    NormalizedWeight(Box<dyn NormalizationStrategy>),
}
```

`RankBy` is an optional parameter on query result types, not on the query structs themselves. This keeps the query execution path simple — queries return unranked results, and ranking is applied as a post-processing step. This separation means ranking does not affect traversal order (which is BFS by depth), only the final result ordering within each depth level or result set.

### Why not a decorator or post-query filter

Two alternatives were considered:

**(a) Decorator pattern**: a `FilteredQuery<T>` wrapper that accepts any query and applies filters. Elegant in theory, but Rust's trait system makes this cumbersome — each query type has a different `execute()` signature and return type. The optional field approach is simpler and avoids a new generic abstraction for a composable filter.

**(b) Post-query filtering**: run the query unfiltered, then filter results. This works for `find_nodes` but fails for traversal — an unfiltered traversal may visit nodes via edges that should have been excluded, producing paths that don't exist within the provenance scope. Filtering must happen during traversal, not after.

## Consequences

**Positive:**

- Invariant 59 is operationalized: provenance-scoped filtering composes with all query primitives through a single struct.
- Corroboration (evidence diversity) becomes a first-class ranking dimension without storing new data — derived from existing `contributions` on each edge.
- Lens-scoped queries are a natural composition: `relationship_prefix: "lens:trellis:"` scopes results to one consumer's translated view. No lens-specific query API needed.
- Zero overhead when unused — `filter: None` skips all predicate checks.

**Negative:**

- Every query struct gains a new optional field. The query module's API surface grows, though the field is ignorable.
- Corroboration ranking applies to the result set produced by the query — it does not reorder traversal to prioritize highly corroborated paths. For `traverse` queries, `max_depth` limits which nodes are eligible for ranking. Consumers who want globally top-corroborated connections regardless of graph distance should use `find_nodes` with `min_corroboration` rather than `traverse` with `RankBy::Corroboration`.
- Corroboration counts all contributors equally — a content adapter and a co-occurrence enrichment and a lens each count as one. Whether all contributions represent equally independent evidence is a domain question, not a query question. Consumers who need finer-grained provenance analysis use `evidence_trail`.
- `min_corroboration` filtering during traversal may prune paths that would have led to highly corroborated edges further out. This is inherent to pre-filtering — the same tradeoff exists with `min_weight` today.

**Neutral:**

- The existing `relationship` (exact match) and `min_weight` fields on `TraverseQuery` are not removed. `QueryFilter` provides strictly more capability. Migration from the existing fields to `QueryFilter` equivalents is optional.
- `StepQuery` already specifies relationship per-step. The `QueryFilter` applies as an additional edge predicate within each step — it does not override the step's relationship filter. When both a step's `relationship` and `QueryFilter.relationship_prefix` are set, edges must satisfy both: the step's exact match AND the prefix match. If a step specifies `relationship: "tagged_with"` and the filter specifies `relationship_prefix: "lens:trellis:"`, no edge satisfies both predicates and the traversal terminates at that step. This is the intended behavior — a lens-scoped `StepQuery` must use lens-prefixed relationship types in its steps. Consumers composing `StepQuery` with `relationship_prefix` should ensure every step's relationship matches the prefix.
