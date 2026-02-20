# Essay 23: Universal Ingest — One Tool, One Ensemble, One Graph

## The problem

Plexus has a structural bottleneck at its MCP transport layer. The internal API has `PlexusApi.ingest()` — a universal write endpoint that routes by `input_kind` to matching adapters, runs enrichments, and returns outbound events. But the MCP server exposes only `annotate`, a narrow tool that creates a text fragment, a provenance chain, and a mark. File extraction is impossible from MCP. Semantic analysis is impossible from MCP. The eight tools registered in the MCP server (one write, six context management, one graph read) reflect the first thing Plexus was built to do (annotations for Trellis), not what it's become.

Meanwhile, the extraction pipeline has proliferated. Essay 18 introduced phased extraction: Phase 1 (registration, blocking), Phase 2 (heuristic analysis, background), Phase 3 (semantic extraction via llm-orc, background). Essay 19 introduced DeclarativeAdapter, a YAML-driven spec interpreter that maps JSON to graph primitives. Essay 20 introduced embedding infrastructure. Essay 22 closed five conformance gaps. And two draft essays — one on codebase semantic intelligence, one on LoRA training for extraction models — proposed domain-specific ensembles: one for code, one for literature, one for documentation, each with specialized prompts and model profiles.

This essay asks whether that complexity is necessary. Three questions drove the research:

1. What should the MCP write surface look like?
2. Can a single general-purpose ensemble extract meaningfully from any content type?
3. If extraction lives in llm-orc ensembles, what remains for Plexus adapters?

## One tool: `ingest`

The domain model already has the right abstraction. Invariant 34 states: "All writes go through ingest(). There is no separate public API for raw graph primitives." `PlexusApi.ingest(context_id, input_kind, data)` is the universal write endpoint. The `input_kind` string routes to matching adapters; the `data` payload is type-erased so any adapter can accept any input type.

The MCP surface should reflect this. One write tool:

```
ingest(input_kind: string, data: object)
```

The MCP server deserializes `data` as JSON and passes it to `PlexusApi.ingest()`. The adapter registered for that `input_kind` receives the JSON and interprets it. `input_kind="extract-file"` triggers the extraction pipeline. `input_kind="annotate"` creates a fragment with provenance. `input_kind="semantic"` runs llm-orc semantic extraction. Any future input kind works without changing the MCP server — add an adapter, register it in the pipeline, and `ingest` routes to it.

`annotate` goes away as a separate MCP tool. It's one kind of ingest, not a distinct operation. The current `annotate` tool's behavior (create fragment, create chain, create mark) is what happens when the pipeline processes `input_kind="annotate"`. No special treatment at the transport layer.

The deeper shift is in who specifies provenance. The current `annotate` tool has the caller orchestrating provenance: they provide `chain_name`, `file`, `line`, `annotation`. This is backwards. Semantic extraction should derive and report its own provenance. The caller provides input data — a file path, a text fragment, structured metadata. The extraction pipeline determines what's significant and creates its own provenance trail. The caller receives back what was found.

For manual annotations, the caller still provides the annotation text and location — but the pipeline creates the provenance structures, not the caller. For file extraction, the caller says "ingest this file" and the pipeline creates chains, marks, concept nodes, relationship edges, and provenance from the extraction results. The MCP transport is a thin shell over the API. This is Invariant 38: "Transports are thin shells."

## One ensemble: convergence over classification

The two draft essays proposed domain-specific ensembles. `codebase-intelligence` would have 11 agents across three tiers: a classifier routing to specialized extractors (AST analyzer, dependency mapper, documentation linker, pattern recognizer, relationship extractor) plus LLM agents for semantic analysis, with a final synthesizer. Each content type gets its own pipeline.

This inverts the question. The essays assume we need to know what we're extracting before we extract it. But what actually is semantic knowledge? Given a scene from Macbeth versus an API module, a generic semantic extractor can pull concepts and relationships from both. What's interesting is where they overlap in the graph. Domain-specific routing prevents exactly those connections from forming.

To test this, we ran a spike: the same ensemble (`plexus-semantic`, one agent, generic prompt) on Macbeth Act 1, Scene 7 and `src/adapter/ingest.rs`, using both llama3 8B and mistral 7B.

From Macbeth, both models extracted: macbeth, assassination, justice, virtue, ambition, lady macbeth, duncan. Thematic concepts alongside characters. Relationships: macbeth considers assassination, justice opposes assassination, ambition drives macbeth.

From the pipeline code, both models extracted: ingest pipeline, engine sink, enrichment registry, adapter error, outbound event. Architectural concepts alongside implementation types. Relationships: pipeline uses engine sink, pipeline describes enrichment registry.

No direct concept overlap between Macbeth and the pipeline code. This is correct and expected. The interesting finding is what didn't happen: no domain routing was needed. The same generic prompt — "identify key concepts and relationships" — produced valid, meaningful extractions from both Shakespeare and Rust. The model's training already includes knowledge of literature, code, science. Domain-specific vocabulary emerges from the content, not from the prompt.

Convergence is a graph-level phenomenon, not an extraction-level one. Individual extractions from different domains won't share concepts (usually). Convergence emerges when the enrichment loop — co-occurrence, embedding similarity, discovery gaps — operates on a context containing diverse sources. The ensemble's job is to extract meaningful concepts from each source. The graph's job is to find the connections.

Domain-specificity belongs at the structural layer: a script that reads an AST (code) vs. one that detects section boundaries (prose) vs. one that parses YAML (config). These are structural preprocessors — the content-extractor stage in the `semantic-extraction` ensemble. The semantic extraction that follows is the same regardless of input type. One ensemble: structural preprocessing (script, content-type-aware) → semantic extraction (LLM, universal) → synthesis (LLM, deduplication).

Model quality matters more than prompt specificity. The difference between llama3 8B and mistral 7B was marginal — both produced valid extractions with similar quality issues (linear confidence scores, pipe-separated relationship types, occasional wrong relationships). The difference between 7B and 70B would be substantial. Investment should go to model capability, not prompt engineering per domain.

## One graph: what adapters become

If extraction lives in ensembles, what's left for Plexus adapters?

Today, SemanticAdapter does five things: calls llm-orc, extracts JSON from the LLM response, maps JSON to graph primitives, creates provenance, and handles graceful degradation. DeclarativeAdapter already handles the third responsibility — it maps JSON to graph primitives via YAML specs, with template expressions that interpolate input fields. SemanticAdapter's 80 lines of `parse_response()` Rust code is a hardcoded version of what DeclarativeAdapter does declaratively.

When extraction moves fully to ensembles:

**Orchestration** (calling llm-orc) becomes a thin wrapper — a generic `EnsembleAdapter` that takes an ensemble name and invokes it. No custom Rust per extraction type.

**Response parsing** (extracting JSON from LLM prose) becomes unnecessary as models reliably produce structured output. The robustness logic (`extract_json()` with its markdown fence stripping and brace matching) is a workaround, not architecture.

**Graph mapping** (JSON → nodes, edges, provenance) moves to DeclarativeAdapter YAML specs. A 20-line spec replaces 80 lines of Rust.

**Provenance** is derived from extraction results by the declarative mapper. The ensemble reports what it found; the mapper creates chains, marks, and tags.

**Graceful degradation** stays — if llm-orc is unavailable, the adapter degrades. This is a pipeline concern.

The new landscape:

| Layer | Responsibility | Lives in |
|-------|---------------|----------|
| Ensemble | Read content, extract concepts, synthesize | llm-orc YAML + model profiles |
| Declarative mapper | JSON → graph primitives + provenance | Plexus YAML specs (DeclarativeAdapter) |
| Pipeline | Routing, enrichment loop, event merging | Plexus Rust (IngestPipeline) |
| Enrichments | Co-occurrence, embedding, discovery gaps, tag bridging | Plexus Rust (reactive) |

Adding a new extraction type means writing two YAML files: an ensemble spec (for llm-orc) and a mapper spec (for DeclarativeAdapter). No Rust code. This is the power of the integration.

What stays in Rust: IngestPipeline (the routing and enrichment engine), EngineSink (the emit/validate/persist bridge), all enrichments (reactive graph transformations that must be fast), DeclarativeAdapter (the YAML interpreter), LlmOrcClient (the subprocess bridge), FragmentAdapter and ProvenanceAdapter (thin, stable, for direct fragment and provenance operations).

## What this means for items 6–8

Essay 22's remaining roadmap was:
- Item 6: ensemble selection strategy ADR
- Item 7: literary extraction ensemble
- Item 8: Macbeth integration spike

This research reframes all three:

**Item 6 collapses.** There is no ensemble selection strategy because there is one ensemble. The decision isn't "which ensemble for which content" — it's "one universal extraction ensemble with structural preprocessing." The ADR becomes simpler: prescribe the architecture (ensemble + declarative mapper + pipeline), not a routing strategy.

**Item 7 is unnecessary.** A literary extraction ensemble is redundant when the universal ensemble handles literature and code equally well. What may be needed is a content-extractor script that handles literary text well (scene detection, dialogue parsing, stage direction separation) — but that's structural preprocessing, not a separate ensemble.

**Item 8 remains, transformed.** The Macbeth integration spike is still the forcing function, but the question changes from "does a literary ensemble produce good extractions?" to "does a universal ensemble, with the full enrichment pipeline (co-occurrence, embeddings, discovery gaps), produce convergence in the graph when diverse sources share a context?" This requires the MCP `ingest` tool to exist, the full pipeline registered, and enrichments wired. The spike becomes an integration test of the whole system, not just ensemble quality.

## Ensemble composition: the deeper question

The spike showed that a single generic ensemble works. But the domain-specific ensembles proposed in the codebase-intelligence essay aren't wrong — they describe expertise that matters. A code-tuned LoRA model will extract architectural patterns that a generic 7B model misses. A literary-tuned model will find thematic resonance that a general extractor overlooks.

The reconciliation: **domain expertise is an llm-orc concern, not a Plexus routing concern.** From Plexus's perspective, it's still one tool (`ingest`), one ensemble name. But inside llm-orc, that ensemble could compose others:

```
universal-extraction (what Plexus invokes)
  ├── classifier agent (routes by content type)
  ├── → code-extraction ensemble (AST scripts, code-tuned LoRA)
  ├── → literary-extraction ensemble (section detection, interpretive LoRA)
  ├── → documentation-extraction ensemble (markdown parsing, instruct model)
  └── synthesis ensemble (merges results from whichever sub-ensembles ran)
```

llm-orc does not yet support nested ensemble invocation — an agent that calls another ensemble rather than a model. But the dependency graph already handles fan-out and sequencing. Nested ensembles are a natural extension: an agent whose `type` is `ensemble` rather than `llm` or `script`, configured with the sub-ensemble name. The sub-ensemble runs, its results flow into the parent's dependency graph, and the synthesis agent merges everything.

This means:
- The LoRA training guide's domain-specific models become **model profiles within sub-ensembles** — a code-extraction ensemble uses `model_profile: code-qwen-7b-lora`, a literary ensemble uses `model_profile: literary-mistral-7b-lora`
- The codebase-intelligence essay's 11-agent design becomes **a sub-ensemble** that the universal extractor delegates to when it encounters code
- The universal extractor's classifier agent decides which sub-ensembles to invoke based on content type — but this routing is invisible to Plexus
- New domain expertise means adding a sub-ensemble and a LoRA model, not changing any Plexus code or YAML

The architecture is fractal: ensembles all the way down, YAML all the way across, and Plexus sees exactly one entry point.

## The path forward

1. **Expose `ingest` at MCP level.** Replace `annotate` with `ingest(input_kind, data)`. Register the full adapter pipeline in the MCP server.
2. **Write a declarative mapper spec** for the universal extraction ensemble's JSON output. Replace SemanticAdapter's hardcoded `parse_response()` with a YAML spec.
3. **Run the Macbeth spike through the full pipeline.** Ingest Macbeth and a Plexus source file into the same context through `ingest`. Observe whether the enrichment loop produces cross-source connections.
4. **Add ensemble composition to llm-orc.** An agent type that invokes a sub-ensemble. This unlocks the hierarchical extraction architecture without changing the Plexus integration.
5. **Iterate on model quality.** As LoRA fine-tuning produces domain-adapted extractors, they slot into sub-ensembles as model profiles. Extraction quality improves without pipeline changes.

The architecture is: one tool (`ingest`), one ensemble (universal extraction, potentially composing sub-ensembles), one graph (convergence through enrichment). Everything else is YAML.
