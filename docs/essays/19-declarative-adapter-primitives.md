# Declarative Adapter Primitives: From Rust to YAML

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — February 2026*

---

## The Problem

Plexus is a knowledge graph engine where all knowledge enters through adapters — Rust structs that validate domain data, map it to graph nodes and edges, and enforce invariants. This works well when the adapter developer writes Rust. But Plexus aspires to serve external consumers — Sketchbin (visual art metadata), EDDI (movement analysis), Carrel (academic citation) — who shouldn't need a Rust toolchain to describe how their data maps to a graph.

ADR-020 proposed a solution: a YAML-based adapter specification language with seven primitives (`create_node`, `create_edge`, `for_each`, `id_template`, `hash_id`, `create_provenance`, `update_properties`). A Rust interpreter (`DeclarativeAdapter`) reads these specs at runtime and produces the same emissions that a hand-written Rust adapter would. The extractor (domain-specific, produces JSON) is separated from the mapper (domain-agnostic, maps JSON to graph).

The primitives are defined. Six of seven are implemented. But the consumer-facing surface — how the YAML actually looks, how it connects to extraction pipelines, how enrichments are declared — needed research before the build phase could proceed. This essay reports what was learned.

---

## Prior Art: How Others Map Data to Graphs

Five systems were surveyed for declarative graph-mapping patterns.

**YARRRML** is the most mature YAML-based graph mapping language, part of the RML (RDF Mapping Language) ecosystem. It maps structured data (CSV, JSON, XML) to RDF triples through declarative rules. Its key patterns: `$(field)` template interpolation for field access, subject templates for deterministic ID generation, `predicateobjects` for edge declarations, JSONPath iterators at the source level, and an extensible function system for transformations. YARRRML supports conditional triple generation — a feature Plexus intentionally omits.

**Nodestream** is the closest property graph precedent. It uses a pipeline architecture (extractor → transformer → interpreter) with YAML-defined interpretations. Interpretation types like `source_node` map directly to our `create_node`. JMESPath expressions extract field values. Node identity is a composite key object. Like YARRRML, it separates data access from graph mapping.

**LinkML** defines graph schemas (node types, edge types, properties) but doesn't map data sources to graph elements. Its property graph modeling — abstract `Node` and `Edge` base classes with domain-specific subclasses — validates treating edges as property-carrying entities.

**Koza** describes its approach as "turning knowledge graph ingests into a set of primitive operations" — the same language ADR-020 uses. But Koza's primitives are Python functions configured by YAML; the actual mapping logic is imperative. Plexus's primitives are fully declarative.

**Neo4j and TinkerPop** have no declarative YAML mapping languages. Neo4j uses Cypher; TinkerPop uses Gremlin. Both are procedural.

### What the survey confirms

The two-layer split (data extraction separate from graph mapping) is industry-standard. Template interpolation for field access and deterministic ID generation are universal patterns. The specific syntax (`$(field)` vs `!jmespath` vs `{input.field}`) is cosmetic — all systems solve the same problem.

Plexus's `create_provenance` primitive and dual-obligation enforcement (Invariant 7) have no precedent in any surveyed system. This is the differentiator that prevents adopting an existing language wholesale.

### No conditionals: a deliberate choice

YARRRML and Nodestream both support conditional logic — generating graph elements only when input data matches certain criteria. Plexus's declarative mapper has no `if`, `when`, or `match` construct. This is intentional, not a gap.

The reason is architectural. In systems like YARRRML, the mapping language operates directly on raw, heterogeneous data sources. Conditionals are necessary because the data hasn't been normalized. In Plexus, the Layer 1 extractor — typically an llm-orc ensemble — normalizes input into a uniform JSON shape before the mapper sees it. The extractor makes all decisions; the mapper just maps. This is already proven end-to-end: the `semantic-extraction` ensemble normalizes LLM output into `{concepts, relationships}`, and `SemanticAdapter` maps it without branching.

The `for_each` primitive over an empty array is the implicit conditional — if the extractor produces zero items, no graph elements are created. This covers the common case without adding branching logic to the spec language.

---

## The llm-orc Integration Pattern

Before this research, the Plexus → llm-orc → Plexus round trip was verified end-to-end for both external enrichment (PageRank, community detection) and semantic extraction (fan-out LLM pipeline). Three integration patterns emerged, all following the same shape:

1. **MCP `annotate`:** text + metadata → `FragmentAdapter` → graph
2. **External enrichment:** graph-export JSON → llm-orc scripts → analysis-result JSON → `GraphAnalysisAdapter` → graph
3. **Semantic extraction:** file path → llm-orc ensemble (extract + LLM + synthesize) → concepts JSON → `SemanticAdapter` → graph

Each pattern is: **structured JSON in → adapter interprets → emissions out.** This is exactly what `DeclarativeAdapter` formalizes. The llm-orc ensembles ARE Layer 1 extractors — domain-specific pipelines that produce structured JSON for a domain-agnostic mapper.

A concurrent MCP surface redesign reinforced the same principle. The Plexus MCP server was trimmed from 19 to 8 tools by removing all mark/chain/link management tools that bypassed the ingest pipeline. Marks, chains, and links are internal graph structures managed by the pipeline — consumers produce structured data, not graph primitives. This is the same boundary that declarative adapter specs enforce: the spec describes what data maps to what graph elements, and the pipeline handles the internal mechanics.

### The ensemble field

The adapter spec YAML should declare its Layer 1 extractor:

```yaml
adapter_id: sketchbin-metadata
input_kind: sketchbin.file
ensemble: sketchbin-semantic-extraction

emit:
  - create_node: ...
```

The `ensemble` field names the llm-orc ensemble that processes the input data. When `DeclarativeAdapter::process()` runs, it invokes the ensemble, receives structured JSON, then applies the `emit` primitives. The two-layer split — extraction (domain-specific) and mapping (domain-agnostic) — is explicit in a single artifact.

---

## Enrichment Architecture

### The wrong question

The original research plan asked: "What's the right execution model for declarative enrichments (match/find_nodes/guard/emit)?" This question assumed that the three-tier enrichment model from ADR-022 was correct: Tier 0 (parameterized Rust built-ins), Tier 1 (declarative YAML enrichments, deferred), Tier 2 (batch graph analysis via llm-orc).

The better question turned out to be: **what discovery affordances should the enrichment system provide?** This reframing, driven by considering EDDI's real-time temporal dynamics and the embedding possibilities from Open Question 14, produced a simpler and more powerful architecture.

### Core enrichments

Plexus's core enrichments provide reactive, graph-wide discovery after every emission. The existing enrichments — `TagConceptBridger` and `CoOccurrenceEnrichment` — are parameterizable general graph algorithms. Two more are needed:

**DiscoveryGapEnrichment.** When embedding-derived `similar_to` edges enter the graph, this enrichment checks whether each similar pair is also structurally connected. Pairs that are latently similar but structurally unconnected get a `discovery_gap` edge — surfacing unexplored territory. This is a negative structural query (checking for the ABSENCE of connections), which co-occurrence cannot express.

**TemporalProximityEnrichment.** Nodes with timestamps within a configurable threshold get `temporal_proximity` edges. For EDDI, this captures movement qualities that co-occur in the same time window. For Trellis, it captures edits made in the same session. The adapter attaches timestamps; the enrichment handles the temporal logic — no baked-in window granularity.

All four core enrichments share the same characteristics:

| Property | Value |
|----------|-------|
| Domain-specific? | No — general graph algorithms |
| Parameterizable? | Yes — relationship names, thresholds |
| Reactive? | Yes — fire per-emission in the enrichment loop |
| Performance | Native Rust, microseconds |
| LLM required? | No — pure graph structure queries |

These are not optional plugins — they are the engine's core discovery capabilities. Every domain benefits from them. They define what kind of knowledge graph engine Plexus is: one that automatically discovers structural co-occurrence, temporal proximity, tag-concept bridges, and latent-structural disagreement.

### Parameterized enrichments in adapter specs

The adapter spec YAML declares which enrichments its data benefits from:

```yaml
enrichments:
  - type: co_occurrence
    source_relationship: exhibits
    output_relationship: co_exhibited
  - type: temporal_proximity
    timestamp_property: gesture_time
    threshold_ms: 500
    output_relationship: temporal_co_occurrence
```

Enrichments are global — they fire after any adapter, not just the declaring one. But they are registered alongside the adapter via the existing `register_integration()` pattern. Deduplication by `id()` handles multiple adapters declaring the same enrichment with the same parameters.

### External enrichments: unifying the execution model

The original three-tier enrichment model (Tier 0 parameterized built-ins, Tier 1 declarative enrichments, Tier 2 graph analysis) described implementation details (where code runs), not an architectural distinction. The real distinction is simpler:

**Core enrichments** are general graph algorithms fundamental to Plexus. Fast, reactive, in Rust.

**External enrichments** are custom patterns implemented as llm-orc ensembles. The pipeline already exists end-to-end: ensemble YAML defines the computation, `invoke()` executes it, results re-enter via `ingest()`. What the "deferred Tier 1" was designing — a new `match`/`find_nodes`/`guard`/`emit` DSL — is unnecessary. The llm-orc ensemble YAML already serves this purpose.

The only missing piece is the trigger. Today, external enrichments run on demand (`plexus analyze`). The extension is an emission trigger: "also fire this ensemble when new data enters the graph." Same ensemble, same result path, different scheduling.

Emission-triggered external enrichments are always background — you cannot block the enrichment loop on an external subprocess. Results re-enter via `ingest()`, which triggers core enrichments on the new data. This creates a layered response:

1. **Immediate:** core enrichments fire synchronously (microseconds)
2. **Background:** emission-triggered external enrichment kicks off (seconds)
3. **Delayed:** external enrichment results arrive via `ingest()`, core enrichments fire again

For EDDI, the performer gets instant structural feedback (temporal proximity, co-occurrence). Richer semantic analysis arrives moments later and triggers another discovery round. The same pattern as phased extraction: Phase 1 is synchronous, Phase 3 (LLM) is background, results re-enter via `ingest()`.

---

## Invariant Tensions

### Disambiguation 13: Core vs External enrichments

The domain model previously stated: "Never call graph analysis an 'enrichment.'" This research found that the conceptual boundary is softer than that framing implies — both are enrichments, distinguished by where the computation happens. An emission-triggered external enrichment is conceptually reactive (it fires in response to graph changes) but does not participate in the enrichment loop (results re-enter via `ingest()`).

The updated disambiguation: both are enrichments. **Core enrichments** are Rust-native, reactive, and fire in the enrichment loop. **External enrichments** are llm-orc ensembles that run outside the enrichment loop, with results re-entering via `ingest()`. The technical distinction (enrichment loop participation, idempotency termination) remains — but the vocabulary now reflects the unity rather than enforcing a false dichotomy.

### Invariant 49: external enrichments via ingest, not enrichment loop

Invariant 49 says external enrichment results must enter through `ingest()`, not the enrichment loop. This is upheld — emission-triggered external enrichments are background tasks whose results re-enter via `ingest()`. The trigger is reactive; the result path is the standard ingest pipeline.

No other invariant tensions were found. Invariant 7 (dual obligation) is enforced by `DeclarativeAdapter` at registration time. Invariant 50 (structure-aware enrichments) is upheld by all four core enrichments. Invariant 48 (`create_provenance` enforces provenance half) is structurally guaranteed by the primitive.

---

## What Emerged

The research produced a simpler architecture than what was planned:

**Declarative adapter specs** need YAML parsing (add `Deserialize` derives), the `update_properties` primitive, an `ensemble` field for Layer 1 extractor declaration, and an `enrichments` section for parameterized enrichment wiring. The seven primitives are sound. The template language is appropriate. No conditionals needed.

**Core enrichments** expand from two to four: co-occurrence, tag bridging, discovery gap, and temporal proximity. All are general graph algorithms in Rust, parameterizable, reactive, and fast. They are the engine's built-in discovery capabilities.

**External enrichments** are not a new system. They are the existing llm-orc integration with an emission trigger option. The "deferred Tier 1 declarative enrichment DSL" (`match`/`find_nodes`/`guard`/`emit`) is unnecessary — llm-orc ensemble YAML already handles custom computation patterns.

The three-tier enrichment model collapses to two categories: core (Rust, fast, reactive, general) and external (llm-orc, background, custom). The difference is not a tier — it's where the computation happens and how fast it needs to be.

---

## References

- [YARRRML Specification](https://rml.io/yarrrml/spec/)
- [Nodestream](https://nodestream-proj.github.io/docs/docs/at-a-glance/)
- [LinkML Property Graph Modeling](https://linkml.io/linkml/howtos/model-property-graphs.html)
- [Koza](https://github.com/monarch-initiative/koza) — Monarch Initiative
- [Koza Application Note](https://arxiv.org/abs/2509.09096) — arXiv:2509.09096
- ADR-020: Declarative Adapter Specs and Two-Layer Extraction
- ADR-022: Parameterized Enrichments
- ADR-023: Graph Analysis via Ingest
- Essay 05: EDDI — Toward a Knowledge Graph for Interactive Performance
- Essay 18: Phased Extraction Architecture
