# ADR-033: Lens Declaration in Declarative Adapter Spec

**Status:** Accepted

**Research:** [Essay 001](../essays/001-query-surface-design.md), [Essay 002](../essays/002-lens-storage-mechanism.md)

**Domain model:** [domain-model.md](../domain-model.md) — lens, enrichment, declarative adapter spec, adapter spec primitive, parameterized enrichment

**Depends on:** ADR-025 (declarative adapter spec extensions), ADR-024 (core and external enrichment architecture), ADR-010 (enrichment trait and loop)

**Resolves:** OQ-20 (lens-enrichment contract), OQ-21 (lens translation fidelity)

---

## Context

Essay 002 established that a lens is an enrichment producing first-class edges via the existing `Enrichment` trait — zero infrastructure changes required (Invariant 57). The domain model defines the lens as a consumer-scoped enrichment that translates cross-domain graph content into one consumer's domain vocabulary at write time (Invariant 56: lens output is public).

What remained unresolved was OQ-20: how does a consumer declare its lens translation rules? The lens needs a declarative format alongside the adapter spec (the adapter already knows the consumer's domain vocabulary). Three options surfaced:

- **(a)** Extend the adapter spec YAML with a `lens:` section
- **(b)** A separate lens spec file referenced by the adapter spec
- **(c)** A parameterized enrichment in the `enrichments:` section with translation-specific parameters

A secondary question (OQ-21) concerns untranslatable connections: cross-domain edges that resist translation into a specific consumer's vocabulary.

## Decision

### A `lens:` section in the adapter spec YAML

The declarative adapter spec gains a `lens:` section declaring translation rules:

```yaml
adapter_id: trellis-content
input_kind: trellis.fragment

lens:
  consumer: trellis
  translations:
    - from: [may_be_related, similar_to]
      to: thematic_connection
      min_weight: 0.2
    - from: tagged_with
      to: topic_link

enrichments:
  - type: co_occurrence
    source_relationship: tagged_with
    output_relationship: may_be_related

emit:
  - create_node: ...
```

Each translation rule specifies:

- **`from`**: one or more source relationship types to watch. The lens reacts to `EdgesAdded` events with matching relationships.
- **`to`**: the output relationship type in the consumer's vocabulary. Stored with the namespace prefix `lens:{consumer}:{to}` (e.g., `lens:trellis:thematic_connection`).
- **`min_weight`** (optional, default 0.0): minimum raw weight on the source edge to trigger translation. Filters noise — weak connections are not translated until they strengthen through further evidence.
- **`involving`** (optional): node predicate filtering which endpoints qualify. Supports `dimension`, `content_type`, and `node_type` fields. When absent, all edges matching `from` are translated.

Multiple `from` relationships mapping to the same `to` relationship is a many-to-one translation: the consumer sees a single `thematic_connection` regardless of whether the source evidence was co-occurrence or embedding similarity. The source evidence is preserved on the original edges — the lens does not replace them.

### Relationship namespace convention

Lens-created edges use the pattern `lens:{consumer}:{to_relationship}` for the edge `relationship` field. The contribution keys use the pattern `lens:{consumer}:{to_relationship}:{from_relationship}` — one slot per source relationship type.

Example: a Trellis lens translating `[may_be_related, similar_to]` into `thematic_connection` creates an edge with:
- `relationship = "lens:trellis:thematic_connection"`
- `contributions = { "lens:trellis:thematic_connection:may_be_related": 0.4, "lens:trellis:thematic_connection:similar_to": 0.6 }`

The per-source-relationship contribution keys preserve evidence diversity for many-to-one translations. Each source relationship type that triggered the translation gets its own contribution slot on the translated edge. This means `contributions.len()` on the translated edge reflects how many independent evidence types agree on the translation — the corroboration signal that ADR-034's `min_corroboration` filter uses. Without per-source keys, latest-value-replace (ADR-003) would overwrite earlier sources, losing evidence diversity.

This namespacing enables:
- Lens-scoped queries via relationship prefix filtering (`relationship_prefix: "lens:trellis:"`)
- Per-source-relationship provenance on translated edges
- Corroboration counting on translated edges (each source type is a distinct contributor)
- Deduplication across enrichment rounds (existing `add_edge` dedup)

### Lens enrichment construction

`DeclarativeAdapter` gains a method analogous to `enrichments()` (ADR-025):

```rust
impl DeclarativeAdapter {
    pub fn lens(&self) -> Option<Arc<dyn Enrichment>> { ... }
}
```

When a `lens:` section is present, `DeclarativeAdapter::lens()` returns a `LensEnrichment` implementing the `Enrichment` trait. Registration follows the existing pattern:

```rust
let adapter = DeclarativeAdapter::from_yaml(yaml_str)?;
let mut enrichments = adapter.enrichments();
if let Some(lens) = adapter.lens() {
    enrichments.push(lens);
}
pipeline.register_integration(Arc::new(adapter), enrichments);
```

### Untranslatable connections (OQ-21)

Translation rules are opt-in, not exhaustive. Connections that do not match any `from` + `involving` pattern remain in the graph as standard edges — accessible via traversal without a `lens:` prefix filter. A consumer that queries only `lens:trellis:*` relationships sees its translated world. A consumer that traverses without relationship filtering discovers untranslated cross-domain content.

This resolves the storage aspect of the scope-versus-serendipity tension (product discovery, 2026-03-25): no connections are hidden, omitted, or generically labeled. The lens translates what it can express in the consumer's vocabulary; everything else remains in the graph, accessible via standard traversal. The discovery mechanism — how consumers find untranslated cross-domain edges they did not know to look for — is not addressed by this ADR and remains an open question for a future query surface iteration.

### Why not option (b) or (c)

**Separate file (b):** Fragments the consumer's configuration. The adapter spec is already "the single artifact they need to understand" (product discovery). Splitting the lens into a separate file adds a second artifact to manage without meaningful benefit.

**Parameterized enrichment (c):** Conflates two different configuration shapes. Existing parameterized enrichments (co-occurrence, temporal proximity) specify pattern-matching parameters — source relationship, output relationship, threshold. A lens specifies *vocabulary translation* — many-to-one relationship mapping between domains. The configuration surface is different: enrichments produce graph intelligence; lenses produce domain vocabulary. Folding lenses into `enrichments:` obscures this distinction.

The `lens:` section is semantically distinct from `enrichments:` — it answers "how does my consumer see cross-domain content?" rather than "what graph patterns should the engine detect?"

## Consequences

**Positive:**

- One YAML artifact captures the full consumer integration: extraction (ensemble), mapping (emit), enrichment wiring (enrichments), and domain view (lens). Self-documenting.
- The lens is invisible to consumers who do not need it — adapters without a `lens:` section work identically to today.
- Lens output is automatically queryable by all existing operations (`traverse`, `find_path`, `step`, `evidence_trail`) without query module changes — the edges are standard first-class edges (Essay 002).
- The namespace convention (`lens:{consumer}:{relationship}`) enables simple prefix filtering at query time without introducing lens-specific concepts into the query module.

**Negative:**

- Each lens adds edges to the graph. Growth is bounded by the number of translatable connections (a fraction of total edges, not a multiple), but compounds with each consumer's lens. The linear edge scan in `add_edge()` (OQ-16) is the shared performance pressure — not lens-specific.
- The `from` field accepts a list of relationships for many-to-one translation. If the same source edge matches multiple translation rules, the lens creates multiple translated edges. Translation rules should be designed to avoid unintentional duplication.
- Lens retraction on source edge removal requires the lens to track which source edges produced which translated edges. The enrichment loop dispatches `EdgesRemoved` events, providing the trigger — but the retraction logic is new work per-lens. Existing enrichments face the same challenge and handle it through idempotency guards; the lens follows the same pattern.

**Neutral:**

- Rust-native adapters (ContentAdapter, SemanticAdapter) are unaffected. The `lens:` section is a declarative adapter spec extension. A Rust adapter that needs lens behavior implements `Enrichment` directly.
- The `LensEnrichment` type is internal to the adapter module. It is not a new public type — consumers interact with it through the adapter spec, not through Rust code.
