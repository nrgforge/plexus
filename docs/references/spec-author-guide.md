# Spec Author Guide

How to write a declarative adapter spec for Plexus. The intended reader
is a consumer application developer extending Plexus to their domain
without writing Rust.

## When you want a spec

A declarative adapter spec is the right extension path when:

- Your application ingests domain-specific data (creative-writing
  fragments, gesture-movement recordings, citations, code annotations)
  and wants that data represented in the graph without writing a Rust
  adapter.
- You want a lens — a domain-specific vocabulary the graph exposes to
  your app's queries.
- You want embedding or semantic-extraction activation in the default
  Homebrew binary (see [ADR-038](../decisions/038-release-binary-feature-profile.md)).

If your domain requires tight control over the adapter's runtime
behavior (rate-limiting, retry policy, custom input routing, complex
state), write a Rust adapter implementing the `Adapter` trait. Specs
are the declarative path; Rust adapters are the imperative path. Both
coexist per Invariant 40.

## Anatomy of a spec

Every spec is a YAML document with these required and optional sections:

```yaml
adapter_id: my-app            # Required — stable identifier (Invariant 13)
input_kind: my-app.document   # Required — what ingest() route matches this adapter
input_schema:                 # Optional — field validation at load time
  - name: content
    type: string
    required: true
ensemble: my-ensemble-name    # Optional — invoke an llm-orc ensemble per ingest (ADR-025)
lens:                         # Optional — domain-vocabulary translation (ADR-033)
  consumer: my-app
  translations:
    - from: [may_be_related]
      to: latent_pair
enrichments:                  # Optional — parameterize core enrichments (ADR-024)
  - type: CoOccurrence
    ...
emit:                         # Required — what to produce per ingest
  - create_node: { ... }
  - create_edge: { ... }
  - create_provenance: { ... }
  - for_each: { ... }
  - update_properties: { ... }
```

Specs are delivered programmatically via `PlexusApi.load_spec(context_id, yaml)`
or the equivalent MCP tool. Plexus does **not** auto-discover specs from
disk — Invariant 61 (consumer owns spec delivery).

## Choosing a dimension

**Every `create_node` primitive declares a `dimension` string.** This
choice is load-bearing:

- **Enrichments filter by dimension** — Invariant 50. For example,
  `EmbeddingSimilarityEnrichment` operates on nodes in the `semantic`
  dimension. A node in `relational` wouldn't participate.
- **Query filters scope by dimension** — `find_nodes(dimension: "semantic")`
  returns only nodes in that dimension.
- **Lens translations inherit dimension constraints** from the edges they
  translate — a lens that translates edges between `semantic` nodes does
  not translate edges between `structure` nodes.

Plexus validates dimension values **syntactically** at `load_spec` time
(rejects empty strings, whitespace, reserved characters like `:`). Plexus
does **not** validate semantic appropriateness
([ADR-042](../decisions/042-dimension-extensibility-guidance.md) rejected
the warn-on-divergence option). The spec author's choice is authoritative
for any string that passes syntactic validation.

### Shipped-adapter conventions

When your spec's node type collides with a shipped-adapter node type, you
have two choices:

1. **Match the convention** — align your dimension with the shipped
   adapter's choice. Your nodes and the shipped adapter's nodes coexist
   in the same dimension; dimension-filtered queries and enrichments see
   them as one population.
2. **Depart deliberately** — choose a different dimension. Your nodes
   and the shipped adapter's nodes coexist in separate dimensions;
   dimension-filtered queries see them as separate populations. This is
   valid — see the ADR-042 scenario "Two adapters declaring different
   dimensions for the same node_type coexist" — but the author has to
   accept that downstream enrichments and queries treat the two
   populations independently.

Known shipped-adapter conventions (current as of this writing):

| Shipped adapter | Node types | Dimension |
|-----------------|------------|-----------|
| `ContentAdapter` (markdown fragments) | `fragment`, `concept` | `structure` (fragment); `semantic` (concept) |
| `ExtractionCoordinator` (registration phase) | `file`, `extraction-status` | `structure` |
| `SemanticAdapter` (llm-orc semantic extraction) | `concept`, `theme` | `semantic` |

The canonical list lives in the adapter source files (each adapter's
rustdoc names its dimension choices). Consult the source when the list
above is out of date.

### Novel dimensions for novel domains

If your spec's node type has no shipped-adapter convention (e.g.
`gesture_phrase`, `code_symbol`, `audio_event`), choose a dimension that
suits your domain. Plexus accepts any syntactically well-formed string.
Practical guidance:

- **Group node types that an enrichment will filter together into the
  same dimension.** If you want `EmbeddingSimilarity` to operate on
  `gesture_phrase` nodes, put them in `semantic` (or choose a domain-
  specific dimension and configure the enrichment to filter on it).
- **Use distinct dimensions for orthogonal facets of the same content.**
  A gesture-movement app might place body-position data in
  `spatial` and vocal-timing data in `temporal` even though both come
  from the same source event.

### Silent-idle failure mode

A spec that declares an enrichment reading a node property without the
spec's `create_node` primitives writing that property produces a
silent-idle enrichment — registered, called, but emitting nothing
because the read value is always absent. The canonical example:
[TemporalProximityEnrichment reads `node.properties["created_at"]`](../decisions/039-created-at-property-contract.md);
if your spec creates nodes without writing `created_at`, the enrichment
fires but never emits.

Diagnose by inspecting the graph for expected edges; when absent, verify
property writes match enrichment reads. Plexus does not surface this
silent-idle state as an error — it is indistinguishable at the
enrichment-loop level from "no pairs above threshold."

## Lens grammar conventions

When your spec declares a `lens:` section, each translation rule decides
the naming register of its `to` relationship
([ADR-041](../decisions/041-lens-grammar-conventions.md)).

### Named relationships

```yaml
lens:
  consumer: my-app
  translations:
    - from: [may_be_related]
      to: thematic_connection
```

The `to` name interprets the edge's meaning. Appropriate for
**operational jobs** within the app — publishing-pipeline routing,
search ranking, analytics aggregation — where the app's logic branches
on the relationship name. The consumer queries by meaning.

### Structural predicates

```yaml
lens:
  consumer: my-app
  translations:
    - from: [may_be_related]
      to: latent_pair
```

The `to` name describes the shape of the connection without interpreting
it. Appropriate for **discovery-oriented jobs** — creative-writing
scaffolding, thesis-finding, reflective discovery — where the value
proposition involves the end-user's interpretive work, and the app's
surface presents the connection as a prompt rather than an assertion.
The consumer queries by shape; meaning is supplied at the UI layer or
by the end-user reading the juxtaposition.

### Per-job, not per-app

An app may combine both registers within a single `lens:` section. A
consumer with both a user-facing discovery surface and an operational
publishing pipeline might declare:

```yaml
lens:
  consumer: trellis
  translations:
    - from: [may_be_related, similar_to]
      to: latent_pair           # structural predicate for discovery surface
    - from: [cites]
      to: ready_to_publish      # named relationship for publishing pipeline
```

Plexus accepts any syntactically well-formed `to` string — the
convention is documented here, not enforced at load time.

### Why structural predicates for discovery-oriented jobs?

Two arguments carry different weight:

- **Composition-shape (analytical, load-bearing):** Structural predicate
  vocabulary extends naturally under future network-science additions —
  a new translation `from: bridges_communities` → `to: bridges_communities`
  is one more predicate in the same register. Named-relationship
  vocabulary makes each extension a semantic commitment that reshapes
  the vocabulary's frame.
- **Phenomenology (hypothesis-level):** The claim that structural
  predicates preserve an "I noticed the connection myself" experience
  that named relationships cancel. This claim is held as hypothesis,
  not settled principle. A future research cycle with untagged-prose
  evidence and a non-builder stakeholder may promote it, revise it, or
  reject it.

If you adopt the structural-predicate convention expecting a phenomenological
outcome, know that the empirical evidence for that outcome is still
outstanding. The composition-shape reasoning holds independently.

## Ensemble integration (llm-orc)

When a spec declares `ensemble: my-ensemble-name`, Plexus invokes the
named llm-orc ensemble once per `ingest()` call. The ensemble's response
is parsed as JSON and made available to emit primitives via the
`{ensemble.<path>}` template accessor ([ADR-025](../decisions/025-semantic-driven-declarative-adapter.md)).

Typical shapes:

```yaml
ensemble: theme-extractor
emit:
  - create_node:
      id: "theme:{ensemble.theme}"     # uses ensemble's `theme` field
      type: theme
      dimension: semantic
```

For batch-pair work (embedding similarity, topic clustering), the
ensemble returns an array and the spec iterates with `for_each`:

```yaml
ensemble: embedding-similarity
emit:
  - for_each:
      collection: ensemble.pairs
      variable: pair
      emit:
        - create_edge:
            source: "{input.pair.source}"
            target: "{input.pair.target}"
            relationship: similar_to
            source_dimension: semantic
            target_dimension: semantic
```

The worked example at
[`examples/specs/embedding-activation.yaml`](../../examples/specs/embedding-activation.yaml)
demonstrates the full pattern: node creation from input, ensemble
invocation, edge materialization from ensemble output.

## Common patterns

### Minimum-viable vs. minimum-useful

A **minimum-viable** spec is any spec that passes `load_spec` validation
— adapter_id, input_kind, input_schema, and at least one `emit` primitive.
A minimum-viable spec is not necessarily useful: a spec that creates
isolated nodes on untagged input produces no structural signal.
CoOccurrence doesn't fire (no `tagged_with` edges); DiscoveryGap is
idle in the default build; no lens has material to translate.

A **minimum-useful** spec names the infrastructure preconditions that
make its emissions produce structural signal. At least one of:

- The spec's emit produces `tagged_with` edges to concept nodes (so
  CoOccurrence can detect shared-source patterns).
- The spec declares an external enrichment that produces `similar_to`
  edges (so DiscoveryGap and any downstream enrichment fire).
- The spec declares an `ensemble:` that performs semantic extraction,
  producing tagged concept nodes that CoOccurrence then operates on.
- The `features = ["embeddings"]` build is active and the spec operates
  on content whose nodes carry embeddable content.

Choose one infrastructure precondition, name it explicitly in your
spec or deployment instructions, and test that the resulting spec
produces edges — not just that it validates and loads. Running the
spec against representative input and verifying structure emerges is
the acceptance check.

See the interaction-specs document's Consumer Application Developer
section for the full decomposition: [`docs/interaction-specs.md`](../interaction-specs.md)
§"Choose a minimum-useful spec rather than a minimum-viable one".

## Where to read next

- [Worked example](../../examples/specs/embedding-activation.yaml) —
  embedding-activation spec with ensemble, for_each, and inline rationale
- [ADR-025](../decisions/025-semantic-driven-declarative-adapter.md) —
  declarative spec grammar (primitives, template language)
- [ADR-033](../decisions/033-lens-declaration.md) — lens mechanics
- [ADR-041](../decisions/041-lens-grammar-conventions.md) — lens grammar conventions
- [ADR-042](../decisions/042-dimension-extensibility-guidance.md) — dimension extensibility
- [`docs/interaction-specs.md`](../interaction-specs.md) — task-level
  workflows for the Consumer Application Developer
