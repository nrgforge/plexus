# ADR-020: Declarative Adapter Specs and Two-Layer Extraction

**Status:** Accepted

**Research:** [Essay 18](../essays/18-phased-extraction-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — declarative adapter spec, DeclarativeAdapter, adapter spec primitive, extractor, declarative mapper, template expression

**Depends on:** ADR-001 (adapter architecture), ADR-004 (fragment adapter), ADR-012 (unified ingest pipeline)

---

## Context

Writing a Rust adapter for every application and file type doesn't scale. Essay 18 analyzed the existing FragmentAdapter and found its logic is a structured transformation: take a field, make a node; for each tag, make a concept; hash fields for a deterministic ID. No arbitrary control flow, no complex algorithms.

Consumers like Sketchbin (creative media) and EDDI (movement analysis) need domain-specific extraction (audio analysis, movement algorithms) but their graph mapping follows the same patterns. The domain expertise lives in extractors (Python, Bash, Rust); the graph mapping is mechanical.

## Decision

### Declarative adapter spec

A YAML specification describes an adapter's behavior using seven primitives:

| Primitive | Purpose |
|---|---|
| `create_node` | Node with type, dimension, content_type, properties |
| `create_edge` | Same or cross-dimensional, with relationship and contribution value |
| `for_each` | Iterate over array fields |
| `id_template` | Deterministic ID from string interpolation |
| `hash_id` | Deterministic ID from content hash (UUID v5) |
| `create_provenance` | Composite: chain + mark + contains edge (enforces Invariant 7) |
| `update_properties` | Add properties to existing node via upsert |

The `create_provenance` primitive enforces the provenance half of the dual obligation (Invariant 7): it always produces chain + mark + contains edge. The semantic half (concept nodes, `tagged_with` edges) depends on the spec also containing `create_node` and `create_edge` directives. DeclarativeAdapter validates at registration time that any spec using `create_provenance` also produces at least one semantic node — enforcing both halves of Invariant 7 through validation rather than primitive structure alone.

### DeclarativeAdapter

A Rust struct implementing the Adapter trait that interprets a declarative adapter spec at runtime. All input is JSON (from any transport). The spec's `input_schema` validates it. Each DeclarativeAdapter instance has its own adapter ID and input kind from the spec.

Existing Rust adapters (FragmentAdapter, ProvenanceAdapter) remain valid. The declarative path is for external consumers who can't or don't want to write Rust.

### Template expressions

The interpolation language in specs is intentionally limited: field access (`{input.tags}`), filters (`{tag | lowercase}`, `sort`, `join`, `default`), and context variables (`{adapter_id}`, `{context_id}`). Complex transformations belong in extractors or Rust, not in templates.

### Two-layer architecture: extractor + mapper

For non-trivial extraction (audio, movement, code, LLM-assisted), the architecture separates into two layers:

- **Layer 1 — Extractor:** Domain-specific code that produces structured JSON. Lives outside Plexus — in script agents (Python/Bash via llm-orc), standalone processes, or Rust functions. This is where domain expertise lives.
- **Layer 2 — Declarative mapper:** The YAML spec that maps extractor-produced JSON to graph nodes and edges. Domain-agnostic — same primitives regardless of input domain.

The mapper doesn't understand audio codecs or movement notation. The extractor doesn't understand graph structure. Neither Sketchbin nor EDDI needs a custom Rust adapter — both use DeclarativeAdapter with domain-appropriate YAML specs.

### FragmentAdapter expressible declaratively

Essay 18 demonstrated that the existing FragmentAdapter can be fully expressed as a declarative spec. This validates the primitive set. FragmentAdapter remains as Rust (it's already built and tested), but its expressibility confirms the primitives are sufficient.

## Consequences

**Positive:**

- External consumers write YAML instead of Rust — lower barrier to entry
- The `create_provenance` primitive enforces Invariant 7 structurally — pit of success
- Two-layer separation keeps domain expertise in the language best suited to it (Python for audio, custom algorithms for movement)
- Cross-domain compatibility: if Sketchbin and EDDI both tag `concept:improvisation`, enrichments fire automatically

**Negative:**

- Runtime interpretation of YAML specs is slower than compiled Rust adapters. For the use case (file extraction where LLM calls dominate), this overhead is negligible.
- The template language is deliberately limited. Consumers needing complex transformations must use extractors (Layer 1) rather than expressing logic in templates. This is intentional but means the spec can't handle all cases alone.
- The primitive set has no conditional (`if`/`match`) construct. Extractors (Layer 1) must normalize output to a uniform shape that the mapper handles without branching. Conditional logic belongs in the extractor, not the mapper.
- DeclarativeAdapter validation errors (malformed YAML, missing required fields, type mismatches) surface at runtime, not compile time.

**Neutral:**

- The seven primitives are designed to cover FragmentAdapter and the three extraction phases based on analysis of their graph mapping patterns. Future adapters may reveal the need for additional primitives. The set is extensible.
- Input validation via `input_schema` is standard JSON Schema. The spec declares what shape it expects; the framework validates before the primitives execute.
