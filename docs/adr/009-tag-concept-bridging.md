# ADR-009: Automatic Tag-to-Concept Bridging

**Status:** Accepted

**Date:** 2026-02-10

**Research:** [Essay 08](../essays/08-runtime-architecture.md), [Research Log Q3](../research/research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — Invariants 27, 29

**Depends on:** ADR-008 (project-scoped provenance), ADR-004 (deterministic concept IDs)

---

## Context

With marks living in project contexts (ADR-008), marks and adapter-produced concept nodes share a context. A mark tagged `#travel` and a concept node `concept:travel` are in the same graph. But nothing connects them — they exist in different dimensions (provenance and semantic) with no edge between them.

The dimension system supports cross-dimensional edges within a context. The connection between what a user annotates (marks) and what the adapter layer discovered (concepts) is the core value proposition of a knowledge graph with provenance. This connection should be automatic, not manual.

> **Note (Essay 12):** Semantic adapters (FragmentAdapter and others) produce marks as part of their dual obligation — automatic evidence trails alongside semantic output. Tag-to-concept bridging works identically for all marks regardless of origin — TagConceptBridger enrichment required zero modifications to handle adapter-created marks. This is the mechanism that makes every concept's origin graph-traversable.
>
> **Note (bidirectional dual obligation):** All consumer-facing paths produce both semantic content and provenance. The `annotate` workflow creates a fragment (semantic) alongside a mark (provenance). There is no path that creates marks without accompanying semantic content.

Tags are the shared vocabulary between provenance and semantics — a tag string on a mark and a concept node ID express the same semantic idea. Tag format normalization provides the mapping: strip `#` prefix, prepend `concept:` to match deterministic concept IDs (ADR-004).

---

## Decisions

### 1. Automatic `references` edges at mark creation time

When `add_mark` is called with tags in a project context, ProvenanceApi checks for concept nodes with matching IDs (after tag format normalization). For each match, a cross-dimensional `references` edge is created from the mark (provenance dimension) to the concept node (semantic dimension).

This happens inline in `ProvenanceApi.add_mark()` — no separate pass, no deferred processing.

**Alternatives considered:**

- *Enrichment scanning for tag-concept matches.* More principled — the enrichment loop handles all graph refinement. **Update (Essay 09):** this is now the chosen approach. A `TagConceptBridger` enrichment runs in the enrichment loop, bridging bidirectionally (new marks to existing concepts, new concepts to existing marks). The inline approach in `ProvenanceApi.add_mark()` is superseded.
- *Explicit user action (manual linking).* Rejected: defeats the purpose. The user already expressed the semantic connection by choosing the tag. The system should close the loop automatically.

### 2. Tag format normalization: `#travel` → `concept:travel`

The convention: strip the `#` prefix if present, lowercase, then prepend `concept:` to construct the candidate concept node ID. This matches the deterministic concept ID scheme from ADR-004 (`concept:{lowercase_tag}`).

The normalization also lowercases the tag to match ADR-004's convention: `#Travel` → `concept:travel`.

This convention must be consistent across all mark creation and concept node creation paths.

### 3. ~~Bridging is creation-time only~~ **RESOLVED by ADR-010 (enrichment model)**

> **Status:** Resolved. The `TagConceptBridger` enrichment runs in the enrichment loop after every emission, bridging bidirectionally: new marks to existing concepts, and new concepts to existing marks. The creation-time-only limitation no longer applies.

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

- ~~Order-dependent: marks created before concepts are not bridged.~~ Resolved by ADR-010: the `TagConceptBridger` enrichment bridges bidirectionally.
- Exact-match only: `#walking` does not bridge to `concept:walk`. Fuzzy matching could be added later but introduces ambiguity. Exact match is the right starting point.
- Adds processing to every `add_mark` call: for each tag, look up a concept node ID. This is a HashMap lookup in the in-memory context — negligible cost.

**Neutral:**

- The `references` edges are regular cross-dimensional edges. They participate in query-time normalization like any other edge.
- Under the enrichment model (ADR-010), `references` edges are created via `Emission` from the `TagConceptBridger` enrichment. They go through the standard commit path with contribution tracking (contribution value 1.0 from the enrichment, matching `tagged_with` binary semantics) and fire graph events. This replaces the previous `Context.add_edge()` bypass.
