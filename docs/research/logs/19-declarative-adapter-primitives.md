# Research Log: Declarative Adapter Primitives (ADR-020 Build Phase)

## Background

ADR-020 is accepted. The seven primitives are defined. A Rust interpreter (`DeclarativeAdapter`) implements 6 of 7 primitives with template engine, input validation, and Invariant 7 enforcement. All tests pass.

The remaining work is about the **consumer-facing surface**: YAML format design, enrichment wiring, and the loading/validation mechanism. The core question: *how should the YAML spec language look and behave so that external consumers (Sketchbin, EDDI) can define adapters without writing Rust?*

### What we know
- Seven primitives: create_node, create_edge, for_each, id_template, hash_id, create_provenance, update_properties
- Two-layer split: extractor (domain-specific JSON) + declarative mapper (domain-agnostic YAML)
- Template language: field access, 4 filters (lowercase, sort, join, default), context variables
- No conditionals by design — extractors normalize
- create_provenance enforces Invariant 7 structurally; registration-time validation enforces the semantic half
- FragmentAdapter expressibility validates primitive sufficiency

### Verified integration patterns (from llm-orc test drive)

The llm-orc integration verified three input patterns that all follow the same shape — **structured JSON in → adapter interprets → emissions out**:

1. **MCP `annotate`:** text + metadata → `FragmentAdapter` → graph
2. **llm-orc external enrichment:** graph-export JSON → scripts → analysis-result JSON → `GraphAnalysisAdapter` → graph
3. **llm-orc semantic extraction:** file path → ensemble (extract + LLM + synthesize) → concepts JSON → `SemanticAdapter` → graph

Key lessons from integration testing:
- **Consumers produce structured data, not graph primitives.** The MCP surface was trimmed to 8 tools (commit `fda27ff`) specifically because mark/chain/link management tools bypassed the ingest pipeline. Declarative specs must follow the same principle.
- **The adapter is the boundary, not the transport.** Whether JSON arrives via MCP, llm-orc subprocess, or Rust API, the adapter's job is the same. The YAML spec language should be transport-agnostic.
- **llm-orc ensembles ARE Layer 1 extractors.** The `semantic-extraction` ensemble (extract_content.py → concept-extractor → synthesizer) is exactly a domain-specific extractor that produces structured JSON for `SemanticAdapter`. Declarative adapter specs formalize the Layer 2 mapping that adapters like `SemanticAdapter` currently do in Rust.

### What we don't know
1. Are there existing declarative graph-mapping languages we should learn from? (format ergonomics, iteration patterns, ID generation conventions)
2. How should parameterized enrichments declared in spec YAML get wired into the engine's enrichment registry?
3. What are the property merge semantics for update_properties?
4. What's the right execution model for declarative enrichments (match/guard/emit)?
5. Should the YAML spec also declare what llm-orc ensemble to use as its Layer 1 extractor, or is that wiring external?

---

## Question 1: What existing declarative graph-mapping languages exist, and what patterns do they use?

**Method:** Web search

**Motivation:** Before finalizing YAML format, we should check whether established mapping languages (R2RML, RML, YARRRML, ShExMap, etc.) have solved similar problems. Specifically:
- How do they express node/edge creation from structured data?
- How do they handle iteration over collections?
- How do they handle deterministic ID generation?
- Do they separate data extraction from graph mapping (like our two-layer split)?

The llm-orc integration adds a concrete constraint: the YAML format must work naturally with the kind of structured JSON that llm-orc ensembles produce (concepts with labels, relationships with weights, nested arrays of chunks). If prior art exists that handles this well, we should learn from it.

**Search terms:**
- "declarative graph mapping language YAML"
- "RML YARRRML knowledge graph mapping"
- "JSON to knowledge graph declarative mapping"
- "declarative ETL knowledge graph specification"

**What would change our approach:** If we find a widely-adopted YAML convention for graph mapping, we should align with it where possible (lower learning curve for consumers). If nothing relevant exists, our design is novel and we proceed with confidence.

**Findings:**

### Systems surveyed

**1. YARRRML / RML** (rml.io) — The most mature YAML-based declarative graph mapping language. Targets RDF (triples), not property graphs, but the mapping patterns are directly relevant.

Core structure:
```yaml
mappings:
  person:
    sources:
      - [data.json~jsonpath, $.people[*]]     # source + iterator
    s: http://example.com/person/$(id)         # subject (= our id_template)
    po:                                         # predicate-object pairs (= our create_edge targets)
      - [foaf:name, $(firstname)]
      - [foaf:knows, $(colleague)~iri]
      - [schema:birthDate, $(dob), xsd:date]
```

Key patterns:
- **Template interpolation:** `$(field)` for field access from source data. Supports string concatenation with constants. (We use `{input.field}`.)
- **Iteration:** `iterator` on the source definition (JSONPath `$.items[*]`). Iteration happens at the data source level, not inside the mapping. (We use `for_each` inside the mapping.)
- **ID generation:** Subject templates with interpolation. Deterministic — same input produces same IRI. (Same as our `id_template`.)
- **Functions:** `function: ex:toLowerCase` with parameters. More expressive than our filters but more complex syntax. (We use `| lowercase` pipe filters.)
- **Conditions:** Conditional triple generation via `condition:` blocks. (We intentionally exclude conditionals.)
- **Cross-source joins:** Link records across different data sources using join conditions. (We don't have this — each adapter processes one input.)
- **Two-layer separation:** Sources (where data comes from) are separate from mappings (how data maps to graph). This validates our extractor/mapper split.

**2. Nodestream** (nodestream-proj.github.io) — Declarative YAML framework for property graph construction. Closest to our use case since it targets property graphs, not RDF.

Core structure:
```yaml
- implementation: nodestream.pipeline:Extractor
  factory: range
  arguments:
    stop: 100000
- implementation: nodestream.interpreting:Interpreter
  arguments:
    interpretations:
      - type: source_node
        node_type: Number
        key:
          number: !jmespath index
```

Key patterns:
- **Pipeline = extractor → transformer → interpreter.** Similar to our extractor → mapper. Extractors produce records (dicts), interpreters map them to graph operations.
- **Interpretation types:** `source_node` (= our `create_node`), `relationship` (= our `create_edge`). Each interpretation type has its own fields.
- **ID via `key`:** Node identity is a composite key object. Fields extracted via JMESPath. (Similar to our `id_template` and `hash_id`.)
- **JMESPath for field access:** Uses YAML tags (`!jmespath path.to.field`) rather than template strings. More powerful but less readable for simple cases.
- **No provenance concept.** No equivalent of `create_provenance` or dual obligation.

**3. LinkML** (linkml.io) — YAML-based schema language for data models. Defines node types, edge types, and property types, but does NOT map data sources to graph elements. It's a schema definition language, not a mapping language. Generates JSON Schema, GraphQL, OWL, etc. from a single YAML model.

Relevant insight: LinkML models property graphs with abstract `Node` and `Edge` base classes, then domain-specific subclasses (`Person is_a Node`, `ActedIn is_a Edge`). Properties on edges are first-class. This validates our approach of treating edges as property-carrying entities.

**4. Koza** (monarch-initiative/koza) — Semi-declarative Python framework for biomedical KG construction. YAML configures data sources and metadata, but Python code does the actual mapping. More imperative than we want — the YAML configures but doesn't define the mapping logic itself.

Key insight: Koza describes its approach as "turning knowledge graph ingests into a set of **primitive operations**" — the same language we use. Their primitives are Python functions; ours are YAML directives. Same concept, different expression.

**5. Neo4j / TinkerPop** — No declarative YAML mapping languages. Neo4j uses Cypher queries or APOC procedures for import. TinkerPop uses Gremlin (procedural). Neither has a declarative spec format for data-to-graph mapping.

### Synthesis

**Our design is novel but has clear precedent.** YARRRML solves a similar problem for RDF. Nodestream solves a similar problem for property graphs. Neither is a perfect match, but both validate our core patterns:

| Concern | YARRRML | Nodestream | Plexus (ADR-020) |
|---------|---------|------------|------------------|
| Target graph | RDF triples | Property graph | Property graph (with dimensions) |
| Spec format | YAML | YAML | YAML |
| Template syntax | `$(field)` | `!jmespath path` | `{input.field}` |
| ID generation | Subject templates | Composite key objects | `id_template` / `hash_id` |
| Iteration | At source level (JSONPath iterator) | At extractor level | Inside mapping (`for_each`) |
| Conditionals | Yes (conditions) | Yes (filters/transformers) | No (intentionally; pushed to Layer 1) |
| Provenance | No | No | Yes (`create_provenance`, Invariant 7) |
| Two-layer split | Yes (source / mapping) | Yes (extractor / interpreter) | Yes (extractor / mapper) |
| Functions/transforms | Yes (FnO functions) | Limited (JMESPath) | Limited (4 filters) |

**Key takeaways:**

1. **Two-layer split is industry-standard.** All three YAML-based systems separate data access from graph mapping. Our extractor/mapper architecture is validated.

2. **Template syntax is a cosmetic choice.** `$(field)` vs `!jmespath` vs `{input.field}` are all doing the same thing. Our `{input.field | lowercase}` pipe syntax is readable and sufficient. No need to change.

3. **No conditionals is the right choice for our architecture.** YARRRML and Nodestream support conditions/filters because they handle raw, heterogeneous data sources directly. We don't — our Layer 1 extractors (including llm-orc ensembles) normalize input into a uniform JSON shape before the mapper sees it. The extractor makes all decisions; the mapper just maps. This is already proven end-to-end: the `semantic-extraction` ensemble normalizes LLM output into `{concepts, relationships}`, and `SemanticAdapter` maps it without branching. Consumers should understand that conditional logic belongs in their extractor, not in their spec.

4. **Provenance is unique to Plexus.** No other system has anything like `create_provenance` or dual obligation enforcement. This is our differentiator and the reason we can't simply adopt an existing language.

5. **`for_each` inside the mapping (vs. at source level) is a design choice.** YARRRML iterates at the source level (JSONPath `$.items[*]`). We iterate inside the mapping (`for_each: collection`). Our approach is more explicit but means the mapping must handle iteration that YARRRML's source layer handles transparently. For our use case (JSON from llm-orc ensembles, not raw data sources) this is fine — the extractor already produces a flat JSON structure.

6. **The YAML format we have is reasonable.** None of the surveyed systems have a format that's clearly better for our use case. We should proceed with our current design, borrowing readability patterns from YARRRML (short keys, inline notation for simple cases) where they improve ergonomics.

**What would NOT change our approach:** Nothing found that would cause us to fundamentally redesign. The seven primitives are sound. The template language is appropriate. The two-layer split is validated.

**What MIGHT improve our approach:**
- YARRRML's shorthand notation (`po: [predicate, $(value)]`) is worth considering for compact edge declarations.
- Nodestream's `key` object pattern (composite keys as a dict) is arguably cleaner than our `id_template` string interpolation for complex IDs.
- All systems document their spec format thoroughly with a reference guide and examples. We need this too.

### Implications

The research confirms we should proceed with the existing design. No fundamental changes needed. The YAML format should be finalized with attention to:
1. Readability (YARRRML's brevity is worth emulating)
2. Documentation (spec reference + worked examples per consumer)
3. Clear guidance on where conditionals go (Layer 1, not Layer 2)

---

## Question 2: How should parameterized enrichments declared in spec YAML get wired into the engine's enrichment registry?

**Method:** Code exploration (enrichment infrastructure) + design analysis

**Motivation:** The domain model says parameterized enrichments are "declared in the adapter spec YAML but fire globally in the enrichment loop." The enrichment infrastructure exists and works. The question is: what's the YAML syntax, and how does `DeclarativeAdapter` expose declared enrichments for pipeline registration?

### Current enrichment infrastructure

**`Enrichment` trait:** `id() -> &str` + `enrich(events, context) -> Option<Emission>`. Enrichments are self-terminating via idempotency checks.

**Registration:** `pipeline.register_integration(adapter, enrichments)` — enrichments are merged into a global `EnrichmentRegistry` with deduplication by `id()`. They fire after ANY adapter, not just the one they're registered with.

**Existing parameterizable enrichments:**

| Enrichment | Default | Parameterized | ID format |
|-----------|---------|---------------|-----------|
| `TagConceptBridger` | `new()` → `references` | `with_relationship("exhibits")` | `tag_bridger:{relationship}` |
| `CoOccurrenceEnrichment` | `new()` → `tagged_with` / `may_be_related` | `with_relationships("exhibits", "co_exhibited")` | `co_occurrence:{source}:{output}` |

**Enrichment loop:** runs after all adapters finish. Each round: snapshot context → run all enrichments → commit emissions → collect events → next round. Terminates on quiescence (all return `None`) or max 10 rounds.

### Design: YAML syntax

Enrichments are declared in the spec YAML alongside `emit`:

```yaml
adapter_id: eddi-gesture
input_kind: eddi.gesture_session

enrichments:
  - type: tag_concept_bridger
    relationship: exhibits          # default: "references"
  - type: co_occurrence
    source_relationship: exhibits   # default: "tagged_with"
    output_relationship: co_exhibited  # default: "may_be_related"

emit:
  - create_node: ...
```

Parameters are flat key-value pairs at the same level as `type`. This matches the enrichment constructors directly: `TagConceptBridger::with_relationship(rel)` and `CoOccurrenceEnrichment::with_relationships(src, out)`.

Default values are the same as the Rust defaults — if you omit a parameter, you get the standard behavior. An enrichment with all defaults:

```yaml
enrichments:
  - type: tag_concept_bridger    # uses "references"
  - type: co_occurrence          # uses "tagged_with" / "may_be_related"
```

### Design: Rust wiring

`DeclarativeAdapter` gets a new method:

```rust
impl DeclarativeAdapter {
    pub fn enrichments(&self) -> Vec<Arc<dyn Enrichment>> {
        // Instantiate enrichments from spec.enrichments declarations
    }
}
```

Registration at the call site:

```rust
let adapter = DeclarativeAdapter::from_yaml(yaml_str)?;
let enrichments = adapter.enrichments();
pipeline.register_integration(Arc::new(adapter), enrichments);
```

This is identical to how `ProvenanceAdapter` is registered today — no new infrastructure needed. The adapter declares what enrichments it needs, the pipeline registers them globally.

### Design: enrichment type registry

With only 2 enrichment types, a simple `match` on the `type` string is sufficient:

```rust
match decl.enrichment_type.as_str() {
    "tag_concept_bridger" => TagConceptBridger::with_relationship(&rel),
    "co_occurrence" => CoOccurrenceEnrichment::with_relationships(&src, &out),
    _ => return Err(...)
}
```

If we add more enrichment types later, we add more match arms. No plugin system needed — enrichments are engine-internal, not consumer-extensible.

### Key insight: enrichments are global, declarations are local

The YAML spec declares "this adapter's data will benefit from these enrichments." But the enrichments fire globally — `CoOccurrenceEnrichment` parameterized on `exhibits` will fire on ALL `exhibits` edges in the context, not just ones from this adapter. This is correct behavior (Invariant 50: structure-aware, not type-aware), but consumers should understand that their enrichment declaration affects the whole graph.

Deduplication by `id()` handles the case where multiple adapters declare the same enrichment with the same parameters — it runs once.

### Findings

No research needed — the infrastructure fully determines the design:

1. **YAML syntax:** `enrichments:` list with `type` and flat parameters. Clean, matches constructors.
2. **Wiring:** `adapter.enrichments()` → `pipeline.register_integration(adapter, enrichments)`. Existing pattern.
3. **Type registry:** Simple `match`. Two types now, add arms as needed.
4. **No new infrastructure.** The enrichment registry, deduplication, and global firing all work as-is.

### Implications

This is a build task, not a research question. The design falls directly out of existing infrastructure. Add to the build plan:
- Add `enrichments` field to `DeclarativeSpec`
- Add `enrichments()` method to `DeclarativeAdapter`
- Add `EnrichmentDeclaration` struct (type + params HashMap)
- Add serde deserialization for enrichment declarations

---

## Question 3: What are the property merge semantics for `update_properties`?

**Method:** Code exploration

**Motivation:** `update_properties` is the 7th primitive, not yet implemented. It should "add properties to existing node via upsert." What happens when two adapters set the same property key? What happens if the node doesn't exist yet?

### Findings: already solved by ADR-023

The infrastructure already has `PropertyUpdate` (ADR-023), committed in `emit_inner` Phase 2.5:

```rust
pub struct PropertyUpdate {
    pub node_id: NodeId,
    pub properties: HashMap<String, PropertyValue>,
}
```

Behavior:
- **Per-key merge** into existing node's properties (`HashMap::insert` per key)
- **Preserves other keys** — only specified properties are touched
- **Last-write-wins per key** — if two adapters set the same key, the later one wins
- **No-op if node absent** — silently skipped, not rejected

This is distinct from the full node upsert path (`Emission.nodes` via `AnnotatedNode`), which replaces the entire node including all properties.

### Design for the primitive

`update_properties` maps directly to `PropertyUpdate`:

```yaml
emit:
  - update_properties:
      node_id: "concept:{input.tag | lowercase}"
      properties:
        pagerank_score: "{input.score}"
        last_analyzed: "{input.timestamp}"
```

The interpreter renders templates in `node_id` and property values, then pushes a `PropertyUpdate` onto `emission.property_updates`. No new infrastructure — the emit path already handles it.

**Use case:** `GraphAnalysisAdapter` results (PageRank scores, community IDs) are property updates on existing concept nodes. Today this is done in Rust; with `update_properties`, a declarative spec could express the same mapping.

### Implications

Another build task, not a research question. The semantic question ("what happens on conflict?") is already answered: per-key last-write-wins, which is the right default for property annotations like scores and timestamps. No need for configurable merge strategies at the primitive level.

---

## Question 4: What's the right execution model for declarative enrichments (match/guard/emit)?

**Method:** ADR review + domain model analysis

**Motivation:** The domain model defines declarative enrichments with `match`, `find_nodes`, `guard`, and `emit` primitives. ADR-022 sketches but defers them. Is this something we need to design now?

### What's already decided

**ADR-022 (Accepted)** defines two tiers:

- **Tier 0 — Parameterized built-ins:** CoOccurrenceEnrichment and TagConceptBridger with configurable relationships. Covers all known use cases (Fragment, EDDI, Carrel). **This is what we build now.**
- **Tier 1 — Declarative enrichments:** YAML spec with `match`/`find_nodes`/`guard`/`emit`. Explicitly deferred: *"Implementation deferred — the parameterized built-ins cover the known use cases."*

**Essay 18** sketches Tier 1 as: "YAML spec with `match` (filter events), `find_nodes` (query context), `guard` (idempotency check), `emit` (create edges). Still graph-wide and reactive."

**OQ-13 (Resolved)** establishes three termination constraints that the YAML schema must enforce at validation time:

1. **No self-triggering output:** a declarative enrichment must NOT create edges of a type it matches on. Prevents self-triggering cycles.
2. **Mandatory guard:** every spec must include a `guard` clause checking output existence before emitting. Idempotency mechanism.
3. **Edge-only emission:** no new node creation. Enrichments bridge existing structure. Bounds the output space to existing node pairs.

Combined with the existing safety valve (`max_rounds`, default 10), these make non-termination structurally impossible for well-formed specs and bounded for malformed ones.

### Do we need to design this now?

**No.** The deferral is correct:

- All known consumers (Sketchbin, EDDI, Carrel) are covered by Tier 0 parameterized built-ins.
- The termination constraints are resolved, so the design can proceed when needed.
- The execution model is clear: declarative enrichments implement the `Enrichment` trait via a YAML-driven interpreter (analogous to how `DeclarativeAdapter` implements `Adapter`). They receive events + context, evaluate `match` against events, run `find_nodes` queries against the context snapshot, check `guard` conditions, and emit edges.
- The YAML syntax (`match`/`find_nodes`/`guard`/`emit`) is sketched but not detailed. Detailing it now would be speculative — we should wait for a concrete use case that parameterized built-ins can't handle.

### What would trigger Tier 1 work

A consumer needs a bridging pattern that isn't expressible as co-occurrence or tag bridging. Examples of patterns that WOULD need declarative enrichments:

- **Temporal proximity:** nodes created within a time window get a `temporal_proximity` edge. (Different algorithm — based on metadata, not shared sources.)
- **Property similarity:** nodes with similar property values get linked. (Different matching — based on property comparison, not edge traversal.)
- **Transitive closure:** if A→B and B→C via relationship X, create A→C. (Different pattern — path traversal, not co-occurrence.)

None of these are needed by current consumers.

### Prior art (noted for future reference)

When Tier 1 is needed, these systems are worth studying:
- **SHACL Rules** — W3C standard for RDF inference rules, declarative graph pattern matching
- **Datalog** — declarative logic programming for graph inference (used in Datomic, LogicBlox)
- **SPIN** — SPARQL Inferencing Notation, declarative rules over RDF graphs

### Findings

Declarative enrichments are correctly deferred. The termination constraints are resolved. No research needed now. When a concrete use case arises, the design space is understood well enough to proceed.

### Implications

~~Remove Q4 from the build plan for this phase. Tier 0 (parameterized built-ins declared in adapter spec YAML) is the scope.~~

**Revised assessment:** The deferral assumption was too narrow. See Q4 addendum below.

---

## Question 4 Addendum: Enrichment affordances and the discovery gap

**Triggered by:** Considering EDDI's temporal dynamics and OQ-14's embedding/latent evidence layer.

### The wrong frame

Q4 originally asked "what's the right execution model for declarative enrichments?" and concluded "correctly deferred." This was wrong because it only asked "do known consumers need Tier 1?" The right question is: **what discovery affordances should the enrichment system provide?**

### Enrichment affordance landscape

| Enrichment | Affordance | Expressible as Tier 0? | Reactive? |
|-----------|------------|----------------------|-----------|
| Tag bridging | "this mark is about this concept" | Yes (built) | Yes |
| Co-occurrence | "these concepts appear together" | Yes (built) | Yes |
| Latent co-occurrence | "second-order similarity" | Yes (parameterize on `similar_to`) | Yes |
| **Discovery gap** | "latently similar but structurally unconnected" | **No** — requires negative structural query | **Yes** |
| **Temporal co-occurrence** | "these happened in the same time window" | Partially — adapter can encode time windows as structure, then co-occurrence fires | **Yes** (EDDI is reactive) |
| **Cross-modal bridge** | "this movement relates to this text concept" | **No** — special case of discovery gap across domains | **Yes** |

### Why discovery gap can't be Tier 0

Co-occurrence finds nodes that SHARE a connection: "A and B are both connected to C via relationship X." Discovery gap checks for ABSENT connections: "A and B are connected via `similar_to` but NOT via any structural edge." This is a negative structural query — fundamentally different from co-occurrence.

Discovery gap logic in the match/find_nodes/guard/emit pattern:
- **Match:** `EdgesAdded` where relationship = `similar_to`
- **Find:** for each new `similar_to` edge (A, B), check if ANY structural edge exists between A and B
- **Guard:** no `discovery_gap` edge exists between A and B
- **Emit:** `discovery_gap` edge with the similarity score as weight

This is reactive — it should fire in the enrichment loop when embeddings produce new `similar_to` edges (which enter via `ingest()` from the external enrichment path).

### Why EDDI forces reactivity

EDDI is a real-time performance system. The performer moves → gesture data enters the graph → enrichments fire → environmental parameters shift. This loop needs to complete within the enrichment loop's execution, not as a batch job. External enrichments are too slow for environmental response.

If temporal co-occurrence is modeled as adapter-encoded time windows + Tier 0 co-occurrence, the reactive requirement is met. But discovery gap and cross-modal bridge cannot be expressed this way.

### Design decision: Tier 0 built-in vs. Tier 1 declarative

**Option A: New Rust built-in** (`DiscoveryGapEnrichment`). Simple to implement, no new infrastructure. But each new enrichment pattern requires Rust code.

**Option B: Build Tier 1 declarative enrichments.** The match/find_nodes/guard/emit DSL. Discovery gap is the first consumer. Future patterns (temporal, cross-modal) can be expressed without Rust. Consistent with the declarative adapter philosophy — if adapters can be YAML, enrichments should be too.

**Option C: Build discovery gap as a Tier 0 built-in NOW, design Tier 1 LATER with discovery gap as the validation case.** Pragmatic: unblocks the immediate need, validates the pattern, informs the declarative design.

### Resolution: Option A — Rust built-in, Tier 0

**Key insight:** Discovery gap is a general-purpose graph pattern, not a domain-specific one. It's parameterizable the same way co-occurrence is. And EDDI's latency requirements rule out anything slower than native Rust — no llm-orc, no YAML interpretation, no subprocess calls.

No LLM involvement needed. Discovery gap is pure graph structure: "check for edge of type X, check for absence of edge of type Y." This is the same kind of abstract pattern as co-occurrence — a different algorithm, but equally domain-agnostic.

**`DiscoveryGapEnrichment` as Tier 0 built-in:**

```rust
pub struct DiscoveryGapEnrichment {
    trigger_relationship: String,    // e.g., "similar_to"
    output_relationship: String,     // e.g., "discovery_gap"
    id: String,                      // "discovery_gap:{trigger}:{output}"
}
```

Parameterizable in adapter spec YAML:
```yaml
enrichments:
  - type: discovery_gap
    trigger_relationship: similar_to
    output_relationship: discovery_gap
```

Algorithm in `enrich()`:
1. Filter events for `EdgesAdded` where relationship = `trigger_relationship`
2. For each new trigger edge (A, B): check if ANY other edge exists between A and B in the context
3. Guard: no `output_relationship` edge already exists between A and B
4. Emit: `output_relationship` edge with the trigger edge's weight

**This strengthens the Tier 1 deferral.** The enrichment tier model becomes:

| Category | Purpose | Performance | Examples |
|----------|---------|-------------|----------|
| Core | General graph algorithms, parameterized | Native Rust, microseconds | Co-occurrence, tag bridging, discovery gap, temporal proximity |
| External | Custom computation, LLM/script, batch or emission-triggered | llm-orc subprocess, seconds | PageRank, community detection, embeddings, semantic analysis |

The decision criterion: if an enrichment is a **generalizable graph algorithm** → core enrichment. If it needs **LLM/script computation** → external enrichment.

### Implications for this build phase

1. **Add `DiscoveryGapEnrichment` to the build plan** — new Tier 0 built-in alongside co-occurrence and tag bridging.
2. **Tier 1 declarative enrichments remain correctly deferred** — the deferral is now better justified by the performance argument and the recognition that discovery gap is general enough for Tier 0.
3. **Update ADR-022** — add discovery gap as a third Tier 0 built-in. Note the performance rationale for keeping general patterns in Rust.
4. **Add `TemporalProximityEnrichment` to the build plan** — new Tier 0 built-in. Reacts to new nodes with timestamp properties, finds other nodes within a configurable time threshold, emits `temporal_proximity` edges. Parameterizable on timestamp property name, threshold, and output relationship. Better than the adapter-encoded time-window trick because: the adapter just attaches timestamps (no baked-in granularity), different consumers parameterize different thresholds (EDDI: 500ms, Trellis: 5 minutes), and the temporal logic is reusable across domains.

**Four Tier 0 built-ins (general graph algorithms):**

| Built-in | Pattern | Affordance |
|----------|---------|------------|
| TagConceptBridger | tag matching | "this mark is about this concept" |
| CoOccurrenceEnrichment | shared sources | "these appear together" |
| DiscoveryGapEnrichment | latent-structural delta | "these should be connected but aren't" |
| TemporalProximityEnrichment | timestamp proximity | "these happened together" |

All parameterizable, all reactive, all fast enough for EDDI.

### Architectural simplification: two categories, not three tiers

The three-tier model (Tier 0 parameterized built-ins, Tier 1 declarative enrichments, Tier 2 graph analysis) described implementation details (where code runs), not an architectural distinction. The real distinction is:

**Core enrichments (Rust).** General graph algorithms fundamental to what Plexus IS as a knowledge graph engine. Every domain benefits from them. Fast, reactive, always on. These are not optional — they're the engine's discovery capabilities:
- Co-occurrence (shared-source patterns)
- Tag bridging (provenance → semantic bridging)
- Discovery gap (latent-structural disagreement)
- Temporal proximity (timestamp-based co-occurrence)

**External enrichments (llm-orc).** Custom patterns implemented as llm-orc ensembles, where the specification is the same but the trigger mode varies:
- `trigger: emission` → emission-triggered, background execution, results via ingest()
- `trigger: on_demand` → batch, run when requested, results via ingest()

The difference between a "declarative enrichment" and a "graph analysis flow" collapses — a discovery gap sweep over the whole graph and a per-emission discovery gap check are the same algorithm with different scheduling. The Rust built-in handles the reactive case because it's core. An external enrichment handles the batch/LLM/script case.

This unification means:
1. The deferred "Tier 1 declarative enrichments" and the existing "Tier 2 graph analysis" are the same thing with different triggers — both are external enrichments.
2. When we build external enrichments, the YAML spec should support both `trigger: emission` and `trigger: on_demand` — same vocabulary, different scheduling.
3. Core enrichments stay in Rust because they're general, fast, and fundamental — not because "Tier 0" is a separate category.

### Emission-triggered external enrichments are always background

An external enrichment with `trigger: emission` delegates to llm-orc and can't block the enrichment loop — it spawns in the background. Results re-enter via `ingest()`, which triggers the core enrichments again on the new data.

This creates a layered response:
1. **Immediate:** core enrichments fire synchronously (structural discovery in microseconds)
2. **Background:** emission-triggered external enrichment kicks off (LLM/script computation)
3. **Delayed:** external enrichment results arrive → `ingest()` → core enrichments fire again on new data

For EDDI: the performer gets instant structural feedback (temporal proximity, co-occurrence). Richer semantic analysis (LLM-derived movement quality) arrives moments later and triggers another discovery round.

This is the same pattern as phased extraction — Phase 1 is synchronous, Phase 3 (LLM) is background, results re-enter via `ingest()`. Applied to enrichments: core enrichments are synchronous, external enrichments are background, results re-enter and trigger more core enrichments.

The execution model options for external enrichments are:
- `trigger: emission` — emission-triggered, background execution, results via ingest()
- `trigger: on_demand` — batch, run when requested, results via ingest()

Both use the same result path (`ingest()`). Both trigger core enrichments on their results. The difference is only when the flow starts.

### Key realization: external enrichments = existing llm-orc integration + emission trigger

The external enrichment pipeline already exists end-to-end: llm-orc ensembles defined in YAML, invoked via `invoke()`, results back through `ingest()`. This IS the external enrichment infrastructure.

External enrichments are not a new system. They are the existing llm-orc integration with an emission trigger wired up. Today: `plexus analyze` runs manually. The extension: also fire a configured ensemble when an emission happens. Same ensemble YAML, same result path, different trigger.

This means:
- **No new declarative enrichment DSL needed.** The `match/find_nodes/guard/emit` pattern from the domain model was designing a DSL for something the llm-orc ensemble YAML already handles.
- **The "deferred Tier 1" work is just wiring:** listen for emissions, dispatch the configured ensemble to the background, let results re-enter via `ingest()`.
- **The genuinely new work is the core enrichments** (discovery gap, temporal proximity) — these are new algorithms that need to be fast and reactive.

---

## Question 5: Should the YAML spec declare its Layer 1 extractor?

**Findings:**

Yes. The adapter spec YAML should declare which llm-orc ensemble serves as its Layer 1 extractor via an `ensemble` field:

```yaml
adapter_id: sketchbin-metadata
input_kind: sketchbin.file
ensemble: sketchbin-semantic-extraction

enrichments:
  - type: co_occurrence
    source_relationship: tagged_with
    output_relationship: may_be_related

emit:
  - create_node: ...
```

This makes the two-layer split explicit in a single artifact:
- `ensemble` — Layer 1 (domain-specific extraction, produces structured JSON)
- `emit` — Layer 2 (domain-agnostic mapping, maps JSON to graph primitives)

When `DeclarativeAdapter::process()` runs, it invokes the named ensemble via llm-orc, receives structured JSON, then applies the `emit` primitives.

This also connects to the external enrichment design: the same `ensemble` field could appear in an external enrichment spec with a trigger parameter, unifying adapter extraction and emission-triggered analysis under one vocabulary.
