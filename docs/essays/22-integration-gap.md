# The Integration Gap: From Architecture to Working Pipeline

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — February 2026*

---

## The Experiment We Tried to Run (and What Stopped Us)

Essay 21 proved the enrichment pipeline works at scale: 234 concepts, five enrichment types, 39 seconds. It also proved that extraction quality is the bottleneck — regex pulls out names but not meaning, while the LLM ensemble extracts genuine concepts but needs structured integration with the graph.

The natural next question: does the system produce genuine insight on literary text? We chose Macbeth as the test case — a work the researcher knows well, from a corpus already in the project (`test-corpora/shakespeare/text/macbeth_TXT_FolgerShakespeare.txt`).

The intended flow: feed Macbeth to Plexus → `ExtractionCoordinator` runs Phase 1 (file registration) → Phase 2 detects acts and scenes → Phase 3 invokes a `DeclarativeAdapter` with `ensemble: literary-themes` → the ensemble extracts themes, character roles, relationships → enrichment pipeline builds the graph → discovery gaps reveal literary connections.

This is the production architecture from ADR-019 through ADR-025: a `DeclarativeAdapter` with an `ensemble` field triggers a multi-phase extraction where Phase 3 calls llm-orc.

We couldn't run it. The *architecture* exists. The *components* exist. The *integration* between them has gaps.

---

## What Works (the Components)

Each component has been built and tested in isolation:

**llm-orc ensemble machinery.** Ensembles can be defined in YAML, invoked via MCP, and produce structured JSON. The `code-concepts` ensemble proved this in Essay 21 — `llama3:8b` analyzer → `gemma3:1b` tag producer, with output format validation and fan-out support.

**FragmentAdapter + enrichment pipeline.** 234 concepts, 39s, correct selectivity. The full enrichment chain — `TagConceptBridger` → `CoOccurrenceEnrichment` → `EmbeddingSimilarityEnrichment` → `DiscoveryGapEnrichment` — runs correctly. Proven in spike_06.

**DeclarativeAdapter primitives.** The YAML spec interpreter handles `create_node`, `create_edge`, `for_each`, `create_provenance`, `update_properties`, template rendering with filters, input validation, enrichment declarations, and dual-obligation enforcement (Invariant 7). All tested (`src/adapter/declarative.rs:924–1613`).

**ExtractionCoordinator framework.** Phase 1 runs synchronously (file node + frontmatter tags + extraction status). Phases 2–3 spawn as background tokio tasks with semaphore-controlled concurrency (4 for analysis, 2 for semantic). Phase 3 chains sequentially after Phase 2, with graceful degradation when llm-orc is unavailable. All tested with stub adapters (`src/adapter/extraction.rs:494–1363`).

**SemanticAdapter.** Invokes llm-orc, parses JSON responses (with fallback field names and fenced code block extraction), prefers the synthesizer agent in fan-out pipelines. Tested with `MockClient` and two `#[ignore]` live integration tests (`src/adapter/semantic.rs:378–1132`).

**LlmOrcClient.** `SubprocessClient` spawns `llm-orc m serve --transport stdio`, communicates via MCP JSON-RPC, lazily initializes the connection. `MockClient` returns preconfigured responses. Both tested (`src/llm_orc.rs:371–424`).

---

## The Six Integration Gaps

### Gap 1: DeclarativeAdapter doesn't invoke its ensemble

The `ensemble: literary-themes` field in the YAML spec is stored in `DeclarativeSpec` (`src/adapter/declarative.rs:329`) but `process()` never calls llm-orc. The method (`src/adapter/declarative.rs:638–665`) downcasts the JSON input, validates against the schema, renders templates, interprets primitives, and emits. It never checks whether `self.spec.ensemble` is `Some`, never constructs an `LlmOrcClient`, never invokes anything.

ADR-025 designed this as a two-layer architecture: the ensemble extracts structured JSON, then the spec's primitives map that JSON to graph mutations. But the bridge between "I have an ensemble name" and "I call that ensemble and get JSON back" doesn't exist in `process()`.

### Gap 2: ExtractionCoordinator → SemanticAdapter type mismatch

The coordinator passes `ExtractFileInput { file_path }` to Phase 3 (`src/adapter/extraction.rs:447–451`):

```rust
let phase3_input = AdapterInput::new(
    phase3.input_kind(),
    ExtractFileInput {
        file_path: file_path_bg.clone(),
    },
    &context_id_bg,
);
```

But `SemanticAdapter::process()` downcasts to `SemanticInput` (`src/adapter/semantic.rs:322–324`):

```rust
let semantic_input = input
    .downcast_data::<SemanticInput>()
    .ok_or(AdapterError::InvalidInput)?;
```

`ExtractFileInput` (`src/adapter/extraction.rs:28–31`) and `SemanticInput` (`src/adapter/semantic.rs:28–34`) are distinct types. The downcast fails at runtime with `InvalidInput`. The two components were designed in parallel and tested with stubs — never wired together.

### Gap 3: No Phase 2 adapters exist

The coordinator dispatches Phase 2 by MIME type prefix (`src/adapter/extraction.rs:111–116`), but no concrete heuristic adapters are implemented. The `register_phase2()` method exists; nobody calls it with a real adapter. For literary text, Phase 2 would detect section boundaries (acts, scenes), extract character names from formatting conventions, compute term frequency — the structural analysis that informs Phase 3. Without Phase 2, Phase 3 gets no section boundaries and operates on the whole file as a single unit.

### Gap 4: Background phases don't persist to SQLite

Background phases emit into `Arc<std::sync::Mutex<Context>>` (`src/adapter/extraction.rs:57`), not through `PlexusEngine`. The `EngineSink::new(ctx.clone())` call in the background task (`src/adapter/extraction.rs:405`) uses the in-memory context path. This means Phase 2 and Phase 3 emissions don't automatically write to SQLite. The graph exists in memory during the task but is lost unless explicitly flushed. The test path (`Arc<Mutex<Context>>`) and the production path (`PlexusEngine` → SQLite) diverge here.

### Gap 5: Chunking assumes file paths, not content

The `content-extractor` script reads files from disk by path. But in an MCP workflow, the user might provide text content directly (e.g., pasted into an `annotate` call). The extraction pipeline assumes filesystem access. This is a secondary gap — it doesn't block the Macbeth test case (which *is* a file on disk) but limits the architecture's generality.

### Gap 6: SemanticAdapter produces no provenance (Invariant 7 violation)

`SemanticAdapter::parse_response()` (`src/adapter/semantic.rs:207–304`) creates concept nodes and `tagged_with` edges, but no chain, no marks, no `contains` edges, no `references` edges. Phase 3 concepts float in the semantic dimension with no epistemological grounding — you can't answer "where did this concept come from?" or "which passage produced this theme?"

Compare with `FragmentAdapter` (`src/adapter/fragment.rs:163–231`), which creates the full provenance trail:

1. **Chain node** — `chain:{adapter_id}:{source}` in provenance dimension, with name and status properties
2. **Mark node** — `mark:{adapter_id}:{fragment_id}` with annotation, file, line, and tags
3. **Contains edge** — chain → mark in provenance dimension
4. **References edges** — mark → concept (created downstream by `TagConceptBridger` reading the mark's tags)

This is the dual obligation from Essay 12: adapters must produce BOTH semantic content AND provenance trails. For Macbeth, if Scene 1 produces 30 concept nodes, each of those 30 should trace back to Scene 1 through provenance. `SemanticAdapter` produces the 30 concepts but not the trace.

---

## Why the Gaps Exist (Process Reflection)

This is an instance of what Essay 16 (Invariant Propagation) warned about: when RDD artifacts accumulate faster than they're maintained, invariant violations propagate silently.

The architecture was designed in Essays 18–19 and codified in ADRs 019–025. The build cycles (Essays 19–20) implemented the *primitives* — DeclarativeAdapter, EmbeddingSimilarityEnrichment, ContributionRetraction — but not the *integration* between them. Each component was tested with its own stubs, not against the real adjacent components.

The self-ingestion spike (Essay 21) tested the enrichment pipeline end-to-end but bypassed the extraction architecture entirely — it used hand-written regex in a spike test, not the production extraction path. The llm-orc ensemble was tested via MCP tools, not through the adapter pipeline. Both worked individually. Neither was tested together.

The coordinator tests in `src/adapter/extraction.rs:494–1363` demonstrate this precisely: `RecordingAdapter`, `EmittingAdapter`, `FailingAdapter`, `SkippingAdapter`, `SequenceVerifyingAdapter` — five different stub adapters, all built to test the coordinator's orchestration logic, none of which exercise real downstream adapters. The stubs prove that Phase 2 runs before Phase 3, that failures don't cascade, that graceful degradation works. They don't prove that Phase 3 can accept what the coordinator sends it (Gap 2) or that the output persists beyond the test context (Gap 4).

---

## Macbeth as Validation Target

Why Macbeth is the right test case for closing these gaps:

**No foreknowledge required.** A real user feeds a text file to Plexus. The system should extract meaningful concepts without domain-specific rules. If the pipeline needs hand-holding for literary text, it's not general-purpose.

**Rich semantic content.** Themes (ambition, guilt, prophecy, fate) are inherently meaningful — they embed well, unlike code type names. If discovery gaps produce genuine insight on Macbeth, the core thesis from Essay 01 is validated on a second domain.

**Known ground truth.** The researcher knows the play well, so LLM extraction quality can be evaluated by someone who can spot both genuine insights and hallucinations. "The witches' prophecy drives Macbeth's ambition" is real. "Macbeth is a programming language" is not.

**Structural variety.** Acts, scenes, stage directions, verse, prose, soliloquies — tests whether chunking and extraction handle heterogeneous text structure. A Phase 2 adapter that can detect "Act I, Scene 2" boundaries in Folger Shakespeare formatting proves it can handle structured text generally.

---

## Integration as an RDD Optimization

Our RDD workflow validates components in isolation (spike per feature, tests with stubs), but integration testing happens late — or in this case, not until we tried to dogfood the full pipeline. This is a structural weakness in the workflow.

**Proposed optimization**: each build cycle should end with an integration spike that wires the new component into its real neighbors, not just stubs. The cost is one extra spike per cycle. The payoff is catching gaps like these before they accumulate across five ADRs.

This is the same lesson Essay 16 identified at the document level, now appearing at the code level: artifacts (components) that validate independently can violate integration invariants silently.

---

## The Transport Question — MCP Exercises the Full Pipeline

MCP is a transport, not a special case. Like any entry point into Plexus, it should exercise the full pipeline: ingestion → extraction → enrichment → provenance. There is nothing special about MCP vs. the Trellis transport vs. a direct Rust API call. They all route through the same `IngestPipeline`.

Today `annotate` is the primary MCP write path, and it creates provenance (marks + chains) but doesn't trigger LLM extraction. The gap: there's no MCP `ingest` tool that accepts a body of work (a file, a text blob) and routes it through the full phased extraction pipeline.

The Macbeth use case makes this concrete: a user should be able to `ingest("macbeth.txt")` through MCP and have the system chunk, extract, enrich, and build provenance for every concept. The subdivision of the work downstream of `ingest` — splitting into scenes or passages — should produce provenance from the extracted semantics. If Scene 1 yields 30 concept nodes, each of those 30 should have provenance that traces back to Scene 1: chain → mark → concept via `contains` and `references` edges.

This is not a new requirement. It's the dual obligation (Invariant 7, Essay 12) applied consistently: every adapter emission produces both semantic content and provenance trails. The gap is that `SemanticAdapter` doesn't do this (Gap 6), and the MCP server doesn't expose the extraction pipeline at all.

---

## Domain-Specific vs. General Extraction

The `code-concepts` ensemble knows it's analyzing code. A hypothetical `literary-themes` ensemble would know it's analyzing literature. But should the calling application need to know this?

**Option A: Domain-specific ensemble selection.** The adapter spec or the user specifies which ensemble to use. `adapter_id: code-adapter` → `ensemble: code-concepts`. `adapter_id: literary-adapter` → `ensemble: literary-themes`. Simple, explicit, but requires the consumer to know what kind of content it's feeding.

**Option B: General-purpose extraction with content detection.** A single `semantic-extraction` ensemble receives any text and adapts. The first agent does content classification (code? prose? technical docs? poetry?), then downstream agents use appropriate extraction strategies. More powerful, but harder to build and test.

**Option C: Phase 2 informs Phase 3.** The heuristic Phase 2 adapter detects content type (MIME type from Phase 1, structural analysis in Phase 2), and the coordinator selects the appropriate Phase 3 ensemble based on what Phase 2 found. The ensemble selection is an architectural decision, not a user decision.

Option C is the most aligned with the phased extraction architecture — each phase builds on the previous. But it requires Phase 2 to exist, which is Gap 3.

These are open questions the next ADR cycle should resolve.

---

## What the Next Build Cycle Should Deliver

Ordered by dependency (each gap unlocks the next):

1. **Fix the coordinator → SemanticAdapter type mismatch** (Gap 2). Small change: either make `SemanticAdapter` accept `ExtractFileInput` or make the coordinator construct `SemanticInput`. This wires Phase 3 into the coordinator for real.

2. **Make DeclarativeAdapter invoke its ensemble** (Gap 1). The `process()` method should: check if `self.spec.ensemble` is `Some` → call `LlmOrcClient::invoke()` → merge the ensemble response into the template context → proceed with `emit` primitives. This is the two-layer integration from ADR-025.

3. **Add persistence for background phases** (Gap 4). Background tasks should emit through `PlexusEngine` (the Engine path), not `Arc<Mutex<Context>>` (the test path). This may require passing `Arc<PlexusEngine>` into the background task instead of a cloned context.

4. **Add provenance to SemanticAdapter** (Gap 6). Phase 3 must produce chain + mark nodes for every extraction. Each chunk or passage that yields concepts should create a mark carrying those concept tags, with a chain scoped to the adapter run. This restores the dual obligation and makes every LLM-extracted concept traceable to its source passage.

5. **Build a Phase 2 text adapter** (Gap 3). For `text/plain`: detect section boundaries (blank lines, heading patterns, numbered sections, act/scene markers), extract proper nouns, compute term frequency. Feed section boundaries to Phase 3 so the ensemble can chunk intelligently.

6. **Decide the ensemble selection strategy** (Section 8). ADR needed: domain-specific vs. general-purpose vs. Phase 2-informed selection. Macbeth is the forcing function — it's clearly "literature" but the system shouldn't need to be told that.

7. **Build the literary extraction ensemble** (new). If Option A (domain-specific): a `literary-themes` ensemble. If Option B/C (general): extend `semantic-extraction` with content-type detection. Either way, test on Macbeth scenes.

8. **Integration spike** (Section 6). After items 1–5 are built, run the full pipeline end-to-end on Macbeth before writing the next essay. This is the new RDD integration checkpoint — the optimization identified in this essay.

After these items, the acceptance test is: `ingest("macbeth-context", "extract-file", ExtractFileInput { file_path: "macbeth.txt" })` produces a fully populated graph with LLM-extracted literary concepts, provenance tracing each concept to its source passage, enriched by co-occurrence, embedding similarity, and discovery gaps — all through a single `ingest` call on any transport.

---

## Key Files

| File | What it reveals |
|------|----------------|
| `src/adapter/declarative.rs:329, 638–665` | Gap 1: `ensemble` field stored, not invoked in `process()` |
| `src/adapter/semantic.rs:322–324` | Gap 2: expects `SemanticInput`, coordinator sends `ExtractFileInput` |
| `src/adapter/extraction.rs:28–31, 447–451` | Gap 2: `ExtractFileInput` definition and Phase 3 dispatch |
| `src/adapter/extraction.rs:57, 405` | Gap 4: background phases use `Arc<Mutex<Context>>`, not Engine |
| `src/adapter/semantic.rs:207–304` | Gap 6: `parse_response()` creates concepts but no provenance |
| `src/adapter/fragment.rs:163–231` | Comparison: FragmentAdapter's full provenance trail |
| `src/llm_orc.rs:98–115` | `LlmOrcClient` trait — the bridge Gap 1 needs |
| `.llm-orc/ensembles/code-concepts.yaml` | Template for the literary ensemble |
| `test-corpora/shakespeare/text/macbeth_TXT_FolgerShakespeare.txt` | Validation target |
| `docs/essays/16-invariant-propagation.md` | Process context for why gaps accumulate |

---

*This essay documents gaps honestly. The architecture is sound — ADRs 019–025 describe the right system. The components work — each is tested and functional. What's missing is the wiring between them. The next build cycle closes these gaps, with Macbeth as the forcing function and acceptance test.*
