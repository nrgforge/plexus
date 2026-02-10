# ADR-009: Automatic Tag-to-Concept Bridging

**Status:** Proposed

**Date:** 2026-02-10

**Research:** [Essay 08](../research/semantic/essays/08-runtime-architecture.md), [Research Log Q3](../research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — Invariants 28–29, 31

**Depends on:** ADR-008 (project-scoped provenance), ADR-004 (deterministic concept IDs)

---

## Context

With marks living in project contexts (ADR-008), marks and adapter-produced concept nodes share a context. A mark tagged `#travel` and a concept node `concept:travel` are in the same graph. But nothing connects them — they exist in different dimensions (provenance and semantic) with no edge between them.

The dimension system supports cross-dimensional edges within a context. The connection between what a user annotates (marks) and what the adapter layer discovered (concepts) is the core value proposition of a knowledge graph with provenance. This connection should be automatic, not manual.

Tags are the shared vocabulary between provenance and semantics — a tag string on a mark and a concept node ID express the same semantic idea. Tag format normalization provides the mapping: strip `#` prefix, prepend `concept:` to match deterministic concept IDs (ADR-004).

---

## Decisions

### 1. Automatic `references` edges at mark creation time

When `add_mark` is called with tags in a project context, ProvenanceApi checks for concept nodes with matching IDs (after tag format normalization). For each match, a cross-dimensional `references` edge is created from the mark (provenance dimension) to the concept node (semantic dimension).

This happens inline in `ProvenanceApi.add_mark()` — no separate pass, no deferred processing.

**Alternatives considered:**

- *Reflexive adapter scanning for tag-concept matches.* More principled — the adapter pipeline handles all graph refinement. Rejected for now: requires schedule monitor infrastructure that doesn't exist. The inline approach can be replaced by a reflexive adapter later without changing external behavior.
- *Explicit user action (manual linking).* Rejected: defeats the purpose. The user already expressed the semantic connection by choosing the tag. The system should close the loop automatically.

### 2. Tag format normalization: `#travel` → `concept:travel`

The convention: strip the `#` prefix from mark tags, prepend `concept:` to construct the candidate concept node ID. This matches the deterministic concept ID scheme from ADR-004 (`concept:{lowercase_tag}`).

The normalization also lowercases the tag to match ADR-004's convention: `#Travel` → `concept:travel`.

This convention must be consistent across all mark creation and concept node creation paths.

### 3. Bridging is creation-time only (known limitation)

If a mark is created before the matching concept node exists, no `references` edge is created. The mark exists unbridged until a future mechanism closes the gap. Two future options:

- A reflexive adapter that scans for unbridged marks
- A hook on concept node creation that scans for marks with matching tags

Neither is implemented now. This is a documented limitation, not a defect. The common workflow — ingest fragments first, then mark passages — produces concepts before marks, so bridging works in the expected order.

### 4. `references` as the relationship type

Cross-dimensional edges from marks to concepts use the relationship type `references`. This is a new relationship type, distinct from `tagged_with` (fragment → concept), `may_be_related` (concept ↔ concept), `links_to` (mark → mark), and `contains` (chain → mark).

---

## Consequences

**Positive:**

- Marks automatically connect to the concepts they reference — the core provenance-semantic bridge
- Query "what evidence supports concept:avignon?" traverses from concept to marks via `references` edges
- The connection is automatic — no manual work beyond tagging, which the user is already doing
- Tag format normalization is a simple, consistent convention

**Negative:**

- Order-dependent: marks created before concepts are not bridged. This affects users who mark passages before ingesting fragments. The limitation is documented; a future reflexive adapter can address it.
- Exact-match only: `#walking` does not bridge to `concept:walk`. Fuzzy matching could be added later but introduces ambiguity. Exact match is the right starting point.
- Adds processing to every `add_mark` call: for each tag, look up a concept node ID. This is a HashMap lookup in the in-memory context — negligible cost.

**Neutral:**

- The `references` edges are regular cross-dimensional edges. They participate in query-time normalization like any other edge. Their raw weight is 1.0 (binary: the tag was applied to the mark), matching `tagged_with` contribution semantics.
- Bridging is inline in ProvenanceApi, not through the adapter pipeline. Bridge edges are created directly via `Context.add_edge()`, so they may not fire graph events or track contributions. If this becomes important (e.g., for provenance of the bridge itself), the reflexive adapter approach would be the right migration path.
