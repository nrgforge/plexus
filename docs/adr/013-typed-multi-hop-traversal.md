# ADR-013: Typed Multi-Hop Traversal

**Status:** Accepted

**Research:** [Essay 14](../essays/14-public-surface-redesign.md), [Research Log Q1](../research/research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — cross-dimensional edge, references, tagged_with, contains

**Depends on:** ADR-009 (tag-to-concept bridging), ADR-010 (enrichment loop)

---

## Context

Multiple consumers (Trellis, Carrel, Manza, Sketchbin among them) converge on the same cross-dimensional query: "What is the evidence trail for this concept?" This query follows a specific shape — each hop follows a different relationship type through different dimensions:

1. concept ← marks (via `references`, incoming — marks have outgoing `references` edges to the concept)
2. concept ← fragments (via `tagged_with`, incoming — fragments have outgoing `tagged_with` edges to the concept)
3. marks ← chains (via `contains`, incoming — chains have outgoing `contains` edges to marks)

Steps 1 and 2 are independent branches from the same origin. Step 3 follows from step 1's results.

The existing query primitives cannot express this. `TraverseQuery` accepts one optional relationship filter and has no dimension awareness — depth-2 with direction Both returns everything within 2 hops, requiring the consumer to post-filter. `FindQuery` can't follow edges. `PathQuery` requires both endpoints known.

The key traversals identified in Essay 13 (and analyzed in Essay 14) — the most important queries in the system — bypass the query system entirely, using raw `ctx.edges().filter(|e| ...)` with hand-written predicates.

## Decision

Add a `StepQuery` to the query system: a typed multi-hop traversal where each step specifies its own relationship filter and direction.

```
StepQuery::from(origin_id)
    .step(direction, relationship)   // hop 1: from origin
    .step(direction, relationship)   // hop 2: from hop 1 results
    .execute(&context)
```

Each step:
- Follows edges matching the specified relationship type and direction from the current frontier
- Collects the nodes reached
- Uses those nodes as the frontier for the next step

The result contains the nodes discovered at each step level and the edges traversed, similar to `TraversalResult` but with per-step relationship context preserved.

`StepQuery` is a sequential chain — each step operates on the previous step's results. It cannot express branching patterns (multiple independent traversals from the same origin) in a single query.

`evidence_trail(node_id)` is a convenience function that composes multiple `StepQuery` executions internally:

```
// Branch 1: concept ← marks ← chains
StepQuery::from(node_id)
    .step(Incoming, "references")    // → marks
    .step(Incoming, "contains")      // → chains

// Branch 2: concept ← fragments
StepQuery::from(node_id)
    .step(Incoming, "tagged_with")   // → fragments
```

The results are merged into an `EvidenceTrailResult` containing marks, fragments, chains, and all traversed edges. `evidence_trail()` is not a separate query primitive — it is a PlexusApi-level convenience that composes `StepQuery` calls.

### Alternatives considered

- **Extend `TraverseQuery` with per-depth relationship filters.** Rejected: `TraverseQuery` is BFS with uniform behavior per depth. Adding per-depth configuration changes its semantics and complicates the existing interface for a use case that's structurally different (typed hops vs. uniform exploration).

- **Compose multiple `TraverseQuery` calls.** This works but requires the consumer to know the graph schema, execute N round-trips, and manually wire frontiers between queries. Every consumer repeating this pattern is a sign the abstraction is missing.

- **Graph query language (Cypher-like).** Overengineered for the current need. `StepQuery` is a builder pattern, not a language. If a query language becomes necessary later, `StepQuery` can be one compilation target.

## Consequences

**Positive:**

- The evidence trail query becomes a single call instead of 3 separate traversals with post-filtering
- All consumers get the query they need, expressed in Plexus core (not transport-specific code)
- Existing query primitives (`FindQuery`, `TraverseQuery`, `PathQuery`) are unaffected
- `StepQuery` is general-purpose — any typed traversal pattern can be expressed, not just evidence trails

**Negative:**

- A new query primitive adds surface area to the query system
- `StepQuery` results include nodes from multiple dimensions and relationship types in a single result — consumers must interpret the step-level structure, not just a flat node list

**Neutral:**

- `evidence_trail()` is a convenience, not a primitive. If the graph schema changes (new relationship types, new dimensions), the convenience function updates but `StepQuery` doesn't
