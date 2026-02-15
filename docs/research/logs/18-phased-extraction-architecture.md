# Research Log: Essay 18 — Phased Extraction Architecture

## Research Questions

### Q1: Target graph shape — what does a fully-extracted file look like?
Given a file that passes through all four phases (file info, metadata, heuristic, semantic), what nodes exist in what dimensions, what edges connect them, and how do contributions from different phases compose? Where does reinforcement happen vs. complementary evidence accumulation?

### Q2: Declarative adapter primitives — what building blocks cover 80% of use cases?
What's the minimal set of declarative primitives (create_node, create_edge, for_each, id_template, etc.) that can express FragmentAdapter and most custom adapters without Rust code? Can the existing FragmentAdapter be fully expressed declaratively?

### Q3: Phase execution model — how do non-blocking phases schedule and report?
Phases 1-2 are blocking (caller gets results immediately). Phases 3-4 are background (progressive enrichment). What's the execution model? How does a background phase signal completion? How does the caller know the graph is "fully enriched"?

### Q4: Phase contribution interaction — how do heuristic and semantic evidence compose?
If Phase 3 (heuristic: word count, structural similarity) and Phase 4 (semantic: LLM-extracted themes) both propose edges between the same concepts, how do their contributions interact? Same adapter ID (merge) or different (accumulate)? What does scale normalization do with heuristic vs. semantic confidence?

### Q5: llm-orc integration architecture — use as-is, port, or hybrid?
llm-orc is Python with a mature DAG model, script agents, and MCP surface. Plexus is Rust. Options: (a) invoke llm-orc as external process/MCP service, (b) port DAG concepts to Rust, (c) hybrid. What are the tradeoffs in latency, deployment complexity, and capability? Note: designing specific ensembles and choosing models is a SEPARATE research cycle — this question is about the structural integration.

### Q6: What do other systems do for progressive/phased extraction?
Tika, Elasticsearch ingest pipelines, Apple Spotlight/mdimporter, Docparser, etc. What patterns exist for multi-phase file processing where later phases are more expensive?

### Q7: Adapter spec format — how does a declarative adapter relate to the Adapter trait?
Is the declarative spec interpreted by a generic "DeclarativeAdapter" that implements the Adapter trait? Or is it a separate concept? How does it compose with the existing pipeline?

### Q8: Test corpora adequacy — do we have the right test data?
The test-corpora submodule has 4 corpora (PKM webdev, PKM datascience, Arch Wiki, Shakespeare). Are these sufficient for testing phased extraction across all four phases? What's missing?

---

## Q1: Target graph shape after phased extraction

**Method:** Design walkthrough — manually trace what each phase produces for `test-corpora/arch-wiki/wiki/Default_applications.md` (247 lines, section hierarchy, related articles metadata, markdown links, rich technical content about MIME types and XDG).

### Phase outputs

**Phase 1 — File info (instant, blocking).** Adapter ID: `file-info`. Produces a `file` node (structure dimension) with path, size, extension, mime_type, modified timestamp. Edge: file → context (`belongs_to`). Contribution: 1.0 (binary). Immediately useful for queries by path/type.

**Phase 2 — Metadata (fast, blocking).** Adapter ID: `metadata`. Parses format-specific metadata without deep content analysis. For this file: extracts title ("Default applications"), "Related articles" (Desktop environment, Window manager), and "Category" (Desktop environments). Produces concept nodes in the semantic dimension + cross-dimensional edges (file → concept). Contribution: 1.0 (author-declared, high confidence).

**Phase 3 — Heuristic (moderate, background).** Adapter ID: `heuristic`. Algorithmic analysis, no LLM. Section structure parsing → section nodes (structure dimension) with heading, level, word_count, has_code_blocks. Link/term extraction → concept nodes for referenced articles (xdg-open, mime-type, thunar, etc.). Statistical properties on the file node (word_count, section_count, code_block_count). Contributions: 1.0 for structural relationships, 1.0 for link-extracted concepts.

**Phase 4 — Semantic (slow, background, LLM).** Adapter ID: `semantic-llm`. Discovers abstract concepts not present in the text literally: `file-association`, `default-application-configuration`. Also discovers concepts already found by Phase 3 (xdg-open, mime-type) — deterministic IDs mean upsert, not duplicate; contributions accumulate in separate adapter slots. Unique Phase 4 value: **concept-to-concept edges** (xdg-open `uses` mime-type, desktop-entry `defines` mime-type) that heuristics can't infer. Contributions: 0.7–0.9 (LLM confidence).

### Where reinforcement happens

Concepts discovered by multiple phases get stronger raw weights:

| Concept | Phase 2 | Phase 3 | Phase 4 | Total contribution slots |
|---|---|---|---|---|
| `concept:xdg-open` | — | 1.0 (heuristic) | 0.8 (semantic-llm) | 2 |
| `concept:mime-type` | — | 1.0 (heuristic) | 0.85 (semantic-llm) | 2 |
| `concept:desktop-environment` | 1.0 (metadata) | — | 0.9 (semantic-llm) | 2 |
| `concept:file-association` | — | — | 0.85 (semantic-llm) | 1 |

Evidence diversity across extraction phases maps directly to higher raw weight via contribution accumulation. The self-reinforcing property works across phases without any special coordination.

### Findings

1. **Each phase MUST have a distinct adapter ID** so contributions accumulate in separate slots rather than overwriting (LWW).

2. **Deterministic concept IDs are essential** — they're what makes cross-phase convergence work. Phase 3's `concept:xdg-open` and Phase 4's `concept:xdg-open` are the same node.

3. **Phase 4's unique value is abstract concepts and concept-to-concept edges** — things heuristics can't find. This is where the LLM investment pays off.

4. **Phases 2-3 are surprisingly valuable** — they produce high-confidence edges cheaply. Phase 4 confirms and extends, but doesn't replace.

5. **Enrichments must be structure-aware, not type-aware.** CoOccurrenceEnrichment currently triggers only on fragment nodes. It needs to generalize to any node with `tagged_with` edges. The enrichment should care about graph topology (which concepts co-occur on the same source node), not node type labels. TagConceptBridger already works this way. This generalization means enrichments fire after every phase emission regardless of what node types the phase produces.

6. **Enrichments fire after each phase emission** (persist-per-emission, Invariant 30). TagConceptBridger and CoOccurrence progressively build bridges as phases complete — the graph enriches incrementally.

### Implications for Q4 (contribution interaction)

Partially answered: phases use distinct adapter IDs, so their contributions are independent slots on the same edge. Scale normalization brings heuristic (always 1.0) and semantic (0.7–0.9) values to comparable ranges before summation. No special coordination needed — the existing contribution tracking system handles multi-phase evidence naturally.

### Implication for Q2 (declarative primitives)

The walkthrough reveals the primitive set: `create_node` (with type, dimension, properties from fields), `create_edge` (same/cross-dimensional, with relationship type), `for_each` (iterate over extracted lists — tags, links, sections), `id_template` (deterministic IDs from input fields), `update_properties` (add properties to existing node on upsert). Phase 3 additionally needs `parse_sections` and `extract_links` as higher-level primitives.

### Long document case (Shakespeare)

A Shakespeare play (3000+ lines, no frontmatter, no links, no consistent structure) stresses Phase 4. The play won't fit in a single LLM context at quality, requiring **chunking + fan-out + synthesis**:

1. **Chunk** — split by acts/scenes or fixed windows (256-512 tokens, ~20% overlap per RAG literature)
2. **Fan-out** — parallel semantic extraction per chunk (llm-orc native `fan_out: true`)
3. **Synthesize** — downstream agent merges chunk-level concepts into file-level themes

Each chunk emission persists independently (Invariant 30). If Phase 4 fails on chunk 7 of 12, chunks 1-6 are already in the graph. Progressive and resilient.

This produces chunk-level concepts (`concept:jealousy` from Act 3), file-level themes (`concept:tragic-jealousy` from synthesis), and concept-to-concept edges from the synthesis step. The DAG model (chunk → fan-out → synthesize) is a strong argument for llm-orc's architecture — this pattern is nontrivial to reimplement.

---

## Q6: What do other systems do for progressive/phased extraction?

**Method:** Web research — survey existing systems for multi-phase document processing patterns.

### Systems surveyed

**Elasticsearch ingest pipelines.** Sequential processor chain. Each processor is a modular step: parsing, extracting, enriching, dropping fields. Processors chain: output of one feeds the next. Lightweight processors (date, geoip, script) minimize latency; heavier processors (grok with LLM connector, enrichment lookups) are later in the chain. Key pattern: **linear pipeline, cheap-first ordering**. No native background/async phases — each document blocks until the full chain completes. Organizations report ~40% reduction in downstream transformation costs by preprocessing in the ingest pipeline.

**Apache Tika.** Two-phase: (1) type detection (fast — MIME sniffing), (2) content extraction (parser-specific — delegates to format-specific parsers). Metadata extraction is unified across formats via a common metadata interface (title, description, keywords, creator). Key pattern: **auto-detect + dispatch to specialized parser**. Single-phase extraction — no progressive enrichment. All extraction happens synchronously.

**Apple Spotlight (mdimporter).** Plugin architecture: each file type has an `.mdimporter` plugin. When files change, `mdworker` processes extract metadata using the appropriate plugin. Metadata and content indexed separately. Key pattern: **plugin-per-type, triggered on file change**. No multi-phase — each plugin does one extraction pass. But the trigger-on-change model is relevant for re-extraction when files update.

**Unstructured.io.** Explicitly two-stage: (1) **Partition** — detects document structure, produces typed Element objects (titles, narrative text, tables, images) using either "rule-based" (fast) or "model-based" (slower, higher resolution) strategies. (2) **Chunk** — post-processes elements into size-optimized chunks for RAG, preserving semantic boundaries from partitioning. Chunking strategies: basic, by_page, by_similarity, by_title. Key insight: **partitioning preserves semantic units; chunking only text-splits when a single element exceeds max size**. Original elements preserved in metadata for recovery. Key pattern: **partition-then-chunk with metadata preservation**.

**RAG chunking literature (2025).** Semantic chunking improves retrieval quality over naive splitting. Optimal chunk sizes: 256-512 tokens with 10-20% overlap. Adaptive chunking tailors strategy to content structure. Context-enriched methods attach metadata/summaries to each chunk. Key finding: wrong chunking strategy creates up to 9% recall gap.

**LlamaIndex.** Multi-stage with metadata management: parse → chunk → embed → index. LlamaParse preserves structure (headings, lists, tables) during parsing. Metadata tagged on nodes (source, section, page, created_at) for query-time filtering. Hierarchical retrieval enables multi-doc auto-retrieval. Key pattern: **structure-preserving parse + metadata-enriched chunks**.

**KG construction pipeline (arxiv 2507.03226).** 8-10 stage pipeline: DocumentParser → HybridChunker → SentenceSegmenter → ContentFilter → TripleExtractor → EntityRelationNormalizer → RelationEntityFilter → GraphProducer → KGLoader. Progressive cost design: cheap linguistic operations (SpaCy dependency parsing, sentence filtering) before expensive LLM calls. Interchangeable architecture: can swap between GPT-4o and dependency parsing based on cost calculation. Key pattern: **cheap-first progressive pipeline with cost-aware stage selection**.

### Synthesis

| Pattern | Systems | Plexus alignment |
|---|---|---|
| **Cheap-first ordering** | Elasticsearch, KG pipeline | Direct match — Phases 1-4 ordered by cost |
| **Partition-then-chunk** | Unstructured.io, LlamaIndex | Phase 3 (structural parsing) is the partition step; Phase 4 chunking for fan-out |
| **Plugin-per-type** | Tika, Spotlight | Declarative adapter spec — different specs for different file types |
| **Metadata preservation through stages** | Unstructured.io, LlamaIndex | Deterministic IDs + upsert means each phase enriches the same nodes |
| **Background/async later phases** | *None surveyed* | Novel in Plexus — most systems process synchronously |
| **Cost-aware stage selection** | KG pipeline | Relevant for Phase 4 — choose local model vs. cloud based on budget |
| **Chunk + fan-out + synthesize** | RAG literature, llm-orc | Phase 4 for long documents; llm-orc's native capability |

### Key findings

1. **No surveyed system does progressive async phased extraction.** This is novel in Plexus. Systems are either fully synchronous (Tika, Elasticsearch, Unstructured) or single-phase (Spotlight). Plexus's model of "immediate value from cheap phases, progressive enrichment from expensive phases" is architecturally distinct.

2. **Partition-then-chunk is the dominant pattern** for handling structure before semantic extraction. Plexus Phase 3 (heuristic/structural) naturally plays the partition role — it identifies sections, headings, code blocks. Phase 4 can then chunk intelligently using Phase 3's structural output rather than naive text splitting.

3. **Cheap-first is universal.** Every system with multiple stages orders them from cheapest to most expensive. This validates the Phase 1-4 ordering.

4. **Metadata preservation across stages is critical.** Unstructured.io and LlamaIndex both emphasize keeping original element metadata accessible after chunking. Plexus handles this naturally through deterministic IDs and upsert — each phase enriches the same node rather than creating a new representation.

5. **Chunk + fan-out + synthesize for long documents** is well-established in the RAG literature but not in knowledge graph construction pipelines. The KG pipeline paper uses chunking for extraction but doesn't fan-out with parallel LLM calls. llm-orc's DAG model with native fan-out is a good fit for this pattern.

6. **Cost-aware stage selection** is an emerging pattern. For Phase 4, the choice of local model (free, slower) vs. cloud model (paid, faster, better) could be a per-context or per-file decision.

---

## Q5: llm-orc integration architecture — use as-is, port, or hybrid?

**Method:** Spike (codebase exploration) — deep dive into llm-orc's execution model, MCP surface, and latency profile, plus Plexus's async patterns.

### llm-orc execution model (key facts)

**Call chain:** CLI/MCP → EnsembleExecutor → dependency analysis → phase-based DAG → asyncio.gather() per phase → per-agent dispatch (LLM or script subprocess). Fresh executor per request. No inter-request state.

**Fan-out:** `fan_out: true` on an agent expands into N parallel instances (one per array element from upstream). Instances run via asyncio.gather() with adaptive semaphore limiting (3-10 concurrent depending on count). Phase boundaries are synchronous — all agents in a phase must complete before the next phase starts.

**Script agents:** Subprocess execution (Python/Bash). Input via `INPUT_DATA` env var, output via stdout. ~50ms overhead per subprocess. Dangerous commands blocked. Result caching with 1-hour TTL.

**Error handling:** Per-agent isolation. If agent X fails, the ensemble continues. Dependent agents receive error status from upstream. Final result includes all outputs (failed and successful). No rollback.

**Latency profile:**
| Component | Time |
|---|---|
| Framework initialization | 50-150ms |
| Fan-out expansion | 1-10ms |
| Phase coordination | 5-20ms |
| Script agent (subprocess) | 50-500ms |
| LLM API call | 500ms-5s |
| **Total framework overhead** | **<200ms** |

The critical insight: **LLM calls dominate at 500ms-5s per agent. Framework overhead is <200ms. Porting to Rust doesn't make the LLM faster.**

**MCP surface:** FastMCP-based, supports stdio and HTTP transports. 25 tools. HTTP server can run as persistent daemon (`llm-orc serve-ensemble`). Each request creates a fresh executor but reuses HTTP connection pool.

**No warm-start:** Model HTTP pools are singleton within a process but not cached across process restarts. MCP HTTP server keeps the process alive, so pools persist across requests.

### Plexus async patterns (key facts)

Plexus already uses tokio extensively. `IngestPipeline::ingest()` is async. The MCP server runs on a tokio runtime. The adapter `process()` method is async. However, there is **no existing pattern for background/long-running work** — all async work is request/response style within the enrichment loop.

### Three options evaluated

**Option A: Invoke llm-orc as external process (CLI)**

Plexus spawns `llm-orc invoke <ensemble> --input <json>` via `tokio::process::Command`.

| Pro | Con |
|---|---|
| Zero Rust code for LLM orchestration | ~50-150ms process startup per invocation |
| Full llm-orc capability (fan-out, scripts, fallbacks) | Requires Python runtime on host |
| Independent evolution — llm-orc improves without Plexus changes | Two runtimes to deploy |
| Python ML ecosystem accessible | Serialization boundary (JSON) |
| Artifact tracking for debugging | No warm model pool across invocations |

Best for: batch processing, contexts where startup cost is amortized over many LLM calls.

**Option B: Port DAG concepts to Rust**

Reimplement phase-based execution, fan-out, agent dispatch in Rust using tokio.

| Pro | Con |
|---|---|
| Single runtime, no Python dependency | Massive implementation effort |
| Tighter integration with Plexus pipeline | Reimplements working, tested code |
| Faster process dispatch | Fan-out + DAG execution is nontrivial |
| No serialization boundary | Loses Python ML ecosystem |
| | Must maintain parity as llm-orc evolves |
| | Script agents still need subprocess |

The cost/benefit is unfavorable. Framework overhead (<200ms) is noise compared to LLM calls (500ms-5s). A Rust reimplementation saves <200ms per ensemble invocation while costing weeks of development and ongoing maintenance. The payoff is asymptotic to zero because the bottleneck is network I/O to the LLM, not framework overhead.

**Option C: Hybrid — llm-orc as persistent MCP service (RECOMMENDED)**

llm-orc runs as a persistent HTTP MCP server. Plexus calls it via reqwest for Phase 4 work. Phases 1-3 remain pure Rust.

| Pro | Con |
|---|---|
| Warm HTTP pool across requests | Requires llm-orc process running |
| Clean integration boundary (JSON-RPC) | Network hop (localhost, <1ms) |
| Plexus stays Rust-pure for Phases 1-3 | Python dependency at deployment time |
| Full llm-orc capability | Two processes to manage |
| Independent evolution | |
| Deployment: "start llm-orc once, Plexus calls it" | |

### Why Option C wins

1. **Phase alignment.** Phases 1-3 are fast, deterministic, no-LLM — they belong in Rust. Phase 4 is slow, LLM-dominated, benefits from DAG orchestration — it belongs in llm-orc. The boundary between Phase 3 and Phase 4 is a natural integration seam.

2. **The latency argument is settled.** Framework overhead <200ms. LLM calls 500ms-5s. Porting to Rust saves <200ms on a 2-20 second operation. Not worth the cost.

3. **Fan-out is llm-orc's killer feature for this use case.** The Shakespeare case (chunk → fan-out → synthesize) is exactly what llm-orc's DAG model with `fan_out: true` is designed for. Reimplementing this in Rust is weeks of work for no latency benefit.

4. **Deployment is tractable.** A managed Plexus deployment (per ADR-018, "managed server") already implies process management. Adding an llm-orc sidecar is incremental. Library embedders who don't need Phase 4 simply don't start llm-orc — Phases 1-3 are self-contained.

5. **The MCP surface is the right contract.** llm-orc already exposes `invoke`, `validate_ensemble`, `list_ensembles` as MCP tools. Plexus calls `invoke` with the ensemble name and input JSON. The result is a JSON object with per-agent responses. This is the thinnest possible integration.

### Integration sketch

```
Phase 1-3 (Rust, in-process):
  file → [file-info adapter] → emit → [metadata adapter] → emit → [heuristic adapter] → emit

Phase 4 (llm-orc, out-of-process):
  Plexus constructs input JSON from Phase 3 output
  → reqwest POST to llm-orc MCP HTTP endpoint
  → llm-orc runs semantic-extraction ensemble (possibly with fan-out)
  → returns concept/edge JSON
  → Plexus wraps result in Emission, routes through pipeline
  → enrichments fire on new nodes/edges
```

The Phase 4 adapter in Plexus is a thin Rust adapter that:
1. Takes Phase 3's structural output as input
2. Serializes it to JSON
3. Calls llm-orc via HTTP (or CLI fallback)
4. Deserializes the response into Emission nodes/edges
5. Returns the Emission for pipeline processing

This adapter could be ~100 lines of Rust. The complex work (chunking, fan-out, LLM prompting, synthesis) lives in llm-orc ensemble YAML + scripts.

### Open question: Phase 4 availability

What happens when llm-orc is not running? Options:
- **Graceful degradation** — Phases 1-3 complete, Phase 4 skipped. Graph is useful but not semantically enriched. This aligns with the progressive model: each phase adds value independently.
- **Queue for later** — Phase 4 input saved, processed when llm-orc becomes available. More complex but enables batch processing.
- **Fail loud** — If the user configured Phase 4, failure to reach llm-orc is an error. Simplest, most honest.

Recommendation: graceful degradation with logging. The graph from Phases 1-3 is already valuable. Phase 4 enrichment is progressive, not required. This matches every surveyed system's pattern (Q6): cheap phases produce immediate value.

### Implications for Q3 (phase execution model)

Phase 4 being out-of-process means the execution model must handle:
- Async HTTP call (already have tokio + reqwest capability)
- Timeout (llm-orc has per-agent timeouts; Plexus needs an outer timeout)
- Partial results (llm-orc returns per-agent success/failure; each chunk emission is independent per Invariant 30)
- Progress reporting (llm-orc emits events; Plexus could poll or receive callbacks)

---

## Q3: Phase execution model — how do non-blocking phases schedule and report?

**Method:** Design analysis — trace the execution model for all four phases, working from the current `IngestPipeline::ingest()` implementation.

### Current execution model

`IngestPipeline::ingest()` is synchronous-within-async: it routes to matching adapters, each adapter processes via a sink (emitting to the engine), the enrichment loop runs to quiescence, events are transformed, and outbound events are returned. All in one `ingest()` call. No background work, no streaming, no progress reporting.

This works perfectly for the current use cases (fragment ingestion, provenance operations). But phased extraction needs a new execution pattern.

### The phased model

**Phases 1-2 (blocking).** File info and metadata extraction are fast (sub-millisecond to tens of milliseconds). They complete within the `ingest()` call and return outbound events immediately. The caller gets useful information right away: file type, title, author metadata, category tags.

**Phases 3-4 (background).** Heuristic analysis (seconds) and semantic extraction (seconds to minutes) are too slow for a synchronous request/response. They must run in the background and emit results progressively.

### Design: each phase is an independent adapter emission

The key insight is that Invariant 30 (persist-per-emission) already gives us the primitive: each phase emits independently, and each emission is durable. We don't need a special "background phase" concept — we need a way to schedule additional `ingest()` calls that run after the initial call returns.

**Execution flow:**

```
Caller calls: ingest("extract-file", file_data)

  Phase 1 (file-info): runs synchronously
    → Emission: file node + belongs_to edge
    → Enrichments fire

  Phase 2 (metadata): runs synchronously
    → Emission: concept nodes + file→concept edges
    → Enrichments fire (TagConceptBridger, CoOccurrence)

  Schedule Phase 3: tokio::spawn(async { pipeline.ingest("heuristic-extract", file_ref) })

  Return: Phase 1+2 outbound events immediately

  [Background, seconds later]
  Phase 3 (heuristic): runs in spawned task
    → Emission: section nodes + link-extracted concepts + stats
    → Enrichments fire

  Schedule Phase 4: if llm-orc available,
    tokio::spawn(async { pipeline.ingest("semantic-extract", phase3_output) })

  [Background, seconds to minutes later]
  Phase 4 (semantic): calls llm-orc, awaits response
    → Emission: abstract concepts + concept-to-concept edges
    → Enrichments fire
    → For fan-out: each chunk emission persists independently
```

**Why this works:**
- Each phase is a regular adapter. No special execution machinery needed.
- The "extraction coordinator" is just an adapter that runs Phases 1-2 synchronously and spawns Phases 3-4 as background tasks.
- Each background phase calls back into `ingest()` with its own input_kind. The pipeline's existing routing, enrichment loop, and event transformation all apply.
- If Phase 3 fails, Phases 1-2 are already persisted. If Phase 4 fails, Phases 1-3 are already persisted. Progressive and resilient.

### Scheduling: who triggers background phases?

**Option A: Extraction coordinator adapter.** A single adapter handles the `extract-file` input_kind. It runs Phases 1-2 inline, then spawns tasks for Phases 3-4. Simple, centralized control.

**Option B: Phase chaining via events.** Phase 1's emission triggers Phase 2 (via enrichment or event handler). Phase 2 triggers Phase 3. Decoupled but harder to reason about. Risk of infinite loops without careful design.

**Option C: External scheduler.** The caller (MCP handler, CLI) decides which phases to run and calls `ingest()` for each. Most flexible but pushes orchestration to every caller.

**Recommendation: Option A.** The extraction coordinator is a single adapter that encapsulates the phase orchestration logic. It takes a file path (or file data) as input, runs the cheap phases synchronously, and spawns background tasks for expensive phases. This keeps the pipeline's existing model clean — each `ingest()` call is still request/response. The coordinator just happens to call `ingest()` again from background tasks.

### Completion signaling: how does the caller know enrichment is done?

For Phases 1-2 (blocking), the answer is simple: the `ingest()` call returns.

For Phases 3-4 (background), the caller needs to know when enrichment is complete. Options:

**1. Status node in the graph.** An `extraction-status` node for each file tracks which phases have completed. Phase transitions are edges: `file --phase-1-complete--> status`, etc. Queryable via standard graph queries. MCP clients can poll `evidence_trail` or a dedicated status query.

**2. Outbound events.** Each phase's `ingest()` call produces outbound events. A listener on the event bus (if one exists) could notify callers. However, the current architecture has no persistent event bus — events are returned from `ingest()` and then discarded.

**3. Callback/channel.** The extraction coordinator returns a `tokio::sync::watch::Receiver` that the caller can await for phase completion signals. Library embedders get a nice async API; MCP callers would need a poll-based wrapper.

**4. Simple poll.** The caller periodically checks whether expected Phase 3/4 nodes exist in the graph. No infrastructure needed but requires the caller to know what to look for.

**Recommendation: Status node + outbound events.** The status node is cheap (one node per file, updated per phase) and queryable by any client. It's also data in the graph — it participates in the graph's own consistency model. The extraction coordinator updates the status node as each phase completes. An MCP tool like `extraction_status(file_path)` could expose this.

Outbound events from background phases should still be returned (from their `ingest()` calls) even though no one is waiting for them synchronously. They're useful for logging and debugging.

### Timeout and cancellation

Each phase needs its own timeout:
- Phase 3 (heuristic): seconds (configurable, default 30s)
- Phase 4 (semantic): minutes (configurable, default 5min for short files, longer for fan-out)

Phase 4's timeout is the llm-orc ensemble timeout. llm-orc has per-agent timeouts internally, and the Plexus adapter applies an outer timeout via `tokio::time::timeout()`.

Cancellation: if the file is deleted or the context is removed while background phases are running, the phase should detect this (check context/file existence before emitting) and abort gracefully.

### Concurrency control

Multiple files may be extracted simultaneously. Phases 3-4 consume resources (CPU for heuristics, LLM capacity for semantics). Options:
- **Semaphore per phase type.** `tokio::sync::Semaphore` with configurable permits. E.g., 4 concurrent heuristic tasks, 2 concurrent semantic tasks.
- **Queue.** Background phases enqueue work; a worker pool processes them.
- **No limit initially.** Let tokio handle it. Add limits when load testing reveals the need.

Recommendation: semaphore per phase type. Simple, effective, configurable. Start with conservative defaults (4 heuristic, 2 semantic).

### Findings

1. **Each phase is a regular adapter emission.** No special execution machinery. The extraction coordinator spawns background tasks that call `ingest()` again.

2. **The pipeline's existing model (route → process → enrich → transform) is sufficient.** Background phases just produce additional `ingest()` calls. Enrichments fire after each one.

3. **Status nodes in the graph track phase completion.** Queryable by any client. MCP tool `extraction_status` exposes it.

4. **Concurrency via tokio semaphores.** Configurable permits per phase type prevent resource exhaustion.

5. **Progressive resilience is automatic.** Each phase's emission is durable (Invariant 30). Phase N+1 failure doesn't affect Phase N's results.

---

## Q2 + Q7: Declarative adapter primitives and spec format

**Method:** Design analysis — reverse-engineer FragmentAdapter into declarative primitives, then generalize.

### FragmentAdapter operations (reverse-engineered)

The FragmentAdapter does exactly these operations:

1. **Downcast input** to `FragmentInput { text, tags, source, date }`
2. **Compute deterministic ID** — UUID v5 from hash(adapter_id + text + sorted_tags)
3. **Create fragment node** — type "fragment", ContentType::Document, structure dimension, with properties: text, source (optional), date (optional)
4. **For each tag:**
   - Create concept node — deterministic ID `concept:{tag_lowercase}`, ContentType::Concept, semantic dimension, property: label
   - Create tagged_with edge — fragment → concept, cross-dimensional, weight 1.0
5. **Create chain node** — deterministic ID `chain:{adapter_id}:{source}`, ContentType::Provenance, provenance dimension
6. **Create mark node** — deterministic ID `mark:{adapter_id}:{fragment_id}`, ContentType::Provenance, provenance dimension, properties: chain_id, annotation (= text), file (= source), line, tags
7. **Create contains edge** — chain → mark, within provenance dimension

### Can this be expressed declaratively?

Yes. Here's what a YAML spec would look like:

```yaml
adapter:
  id: "manual-fragment"
  input_kind: "fragment"
  input_schema:
    text: string
    tags: string[]
    source: string?
    date: string?

  emit:
    # Fragment node
    - create_node:
        id: { hash: ["{adapter_id}", "{input.text}", "{input.tags | sort | join}"] }
        id_prefix: "fragment"
        type: "fragment"
        content_type: Document
        dimension: structure
        properties:
          text: "{input.text}"
          source: "{input.source}"
          date: "{input.date}"

    # Concept nodes + edges (one per tag)
    - for_each: "{input.tags}"
      as: tag
      emit:
        - create_node:
            id: "concept:{tag | lowercase}"
            type: "concept"
            content_type: Concept
            dimension: semantic
            properties:
              label: "{tag | lowercase}"
        - create_edge:
            source: "{fragment.id}"
            target: "concept:{tag | lowercase}"
            relationship: "tagged_with"
            cross_dimensional: [structure, semantic]
            weight: 1.0

    # Provenance (chain + mark)
    - create_provenance:
        chain_id: "chain:{adapter_id}:{input.source | default: 'default'}"
        chain_name: "{adapter_id} — {input.source | default: 'default'}"
        mark_id: "mark:{adapter_id}:{fragment.id}"
        annotation: "{input.text}"
        file: "{input.source | default: 'default'}"
        line: 1
        tags: "{input.tags}"
```

### Minimal primitive set

From the FragmentAdapter walkthrough and the Q1 phase analysis, the required primitives:

| Primitive | Purpose | Used by |
|---|---|---|
| `create_node` | Create a node with type, dimension, content_type, properties | All phases |
| `create_edge` | Create an edge (same or cross-dimensional) with relationship and weight | All phases |
| `for_each` | Iterate over an array, running inner operations per element | Phases 2-4 (tags, links, sections, concepts) |
| `id_template` | Deterministic ID from string interpolation: `concept:{tag}` | All phases |
| `hash_id` | Deterministic ID from content hash (UUID v5) | Phase 1 (file nodes), fragment nodes |
| `create_provenance` | Composite: creates chain + mark + contains edge | All phases (Invariant 7: dual obligation) |
| `update_properties` | Add/update properties on an existing node (via upsert) | Phases 3-4 (adding stats, enrichments to file node) |

**Higher-level primitives for specific phases:**

| Primitive | Purpose | Phase |
|---|---|---|
| `parse_sections` | Parse markdown/text into section structure | Phase 3 |
| `extract_links` | Extract hyperlinks, wiki-links, references | Phase 3 |
| `extract_frontmatter` | Parse YAML/TOML frontmatter | Phase 2 |
| `word_count` | Compute word count per section or file | Phase 3 |

The higher-level primitives are "batteries-included" extractors. They're not strictly necessary (a script agent in llm-orc could do the same work) but they make Phase 2-3 specs concise and fast.

### Q7: How does the spec relate to the Adapter trait?

**A `DeclarativeAdapter` struct implements the `Adapter` trait.** It's interpreted, not compiled:

```rust
pub struct DeclarativeAdapter {
    spec: AdapterSpec,  // parsed from YAML
}

#[async_trait]
impl Adapter for DeclarativeAdapter {
    fn id(&self) -> &str { &self.spec.adapter.id }
    fn input_kind(&self) -> &str { &self.spec.adapter.input_kind }

    async fn process(&self, input: &AdapterInput, sink: &dyn AdapterSink) -> Result<(), AdapterError> {
        // Downcast input to serde_json::Value (universal)
        // Walk the spec's emit list, evaluating templates against input
        // Build Emission from evaluated nodes/edges
        // sink.emit(emission)
    }
}
```

**Key design decisions:**

1. **Input is always JSON.** The declarative adapter doesn't use Rust-specific `FragmentInput` structs. Input arrives as `serde_json::Value` (from any transport: MCP, CLI, HTTP). The spec's `input_schema` validates the JSON.

2. **The spec is a data structure, not code.** It's interpreted at runtime by `DeclarativeAdapter::process()`. This means specs can be loaded from files, registered dynamically, and shared across deployments without recompilation.

3. **Existing Rust adapters remain valid.** `DeclarativeAdapter` is one implementation of the `Adapter` trait. `FragmentAdapter` and `ProvenanceAdapter` continue to work. The declarative path is for external consumers who can't write Rust.

4. **Outbound events are also declarative.** The spec includes a `transform` section that maps graph events to outbound events via pattern matching:

```yaml
  transform:
    - on: NodesAdded
      filter: { node_type: "fragment", adapter_id: "{adapter_id}" }
      emit: { kind: "fragment_indexed", detail: "{node_id}" }
    - on: NodesAdded
      filter: { node_type: "concept", adapter_id: "{adapter_id}" }
      emit: { kind: "concepts_detected", detail: "{node_ids | join: ', '}" }
```

### Can FragmentAdapter be fully expressed declaratively?

**Yes.** The YAML spec above captures all of FragmentAdapter's logic:
- Deterministic fragment ID via hash
- Fragment node with properties
- Concept nodes via for_each over tags
- Cross-dimensional tagged_with edges
- Provenance chain + mark via composite primitive
- Outbound events via transform section

The only thing the declarative spec can't express is arbitrary Rust logic. But FragmentAdapter doesn't use arbitrary logic — it's a structured transformation from input to graph mutations.

### Template expression language

The `{...}` expressions in the spec need a small template language:

- **Field access:** `{input.text}`, `{input.tags}`
- **Filters:** `{tag | lowercase}`, `{input.tags | sort | join}`, `{input.source | default: 'default'}`
- **Context variables:** `{adapter_id}`, `{context_id}`, `{fragment.id}` (refers to a node created earlier in the same emit block)

This is intentionally simple — closer to Liquid or Jinja than to a full programming language. Complex transformations belong in script agents (Phase 3-4), not in the template language.

### Findings

1. **7 core primitives cover the fragment adapter and all four extraction phases.** create_node, create_edge, for_each, id_template, hash_id, create_provenance, update_properties.

2. **DeclarativeAdapter implements the Adapter trait.** The spec is a data structure interpreted at runtime. No new concepts needed — it composes with the existing pipeline.

3. **Input is JSON, output is Emission.** This makes declarative adapters language-agnostic. Any transport that can send JSON can use them.

4. **The template language is intentionally limited.** Field access + filters + context variables. Complex logic belongs in script agents or Rust adapters.

5. **FragmentAdapter can be fully expressed declaratively.** This validates the primitive set — if it handles the existing built-in adapter, it should handle most custom adapters.

6. **create_provenance is a composite primitive.** It encapsulates Invariant 7 (dual obligation: every adapter emission produces both semantic and provenance nodes). This is the kind of "pit of success" design that prevents violations.

---

## Q4: Phase contribution interaction (closed — answered by Q1)

**Method:** Resolved during Q1 walkthrough.

Q1's analysis showed that phases use **distinct adapter IDs** (file-info, metadata, heuristic, semantic-llm), so their contributions are independent slots on the same edge. The contribution tracking system (ADR-003) handles this naturally:

- Phase 3 contributes 1.0 to `file → concept:xdg-open` under adapter ID "heuristic"
- Phase 4 contributes 0.8 to the same edge under adapter ID "semantic-llm"
- Scale normalization brings both to comparable ranges before summation
- Raw weight = sum of normalized contributions across all adapter slots

No special coordination is needed. The existing system handles multi-phase evidence accumulation as a natural consequence of per-adapter contribution slots + deterministic edge IDs.

The only design requirement: **each phase MUST use a distinct adapter ID.** If two phases share an adapter ID, their contributions would overwrite (LWW) rather than accumulate.

---

## Q8: Test corpora adequacy

**Method:** Spike (codebase exploration) — survey existing test-corpora submodule.

### Corpora inventory

| Corpus | Files | Format | Size range | Link type | Distinctive property |
|---|---|---|---|---|---|
| **pkm-webdev** | 51 md | Markdown | 200B-2KB | Wikilinks `[[Page]]` | Hub-and-spoke MOC structure |
| **pkm-datascience** | 516 md | Markdown | 0B-3.3KB | Wikilinks `[[Concept]]` | Dense link network (1.6 links/file), German terms |
| **arch-wiki** | 2,487 md | Markdown | 1KB-50KB | Standard `[text](path.md)` | Largest corpus, wiki-style hyperlinks |
| **shakespeare** | 42 txt | Plain text | 98KB-182KB | None | Zero explicit links, consistent act/scene structure, ~4,500 lines/file |

### Phase coverage assessment

**Phase 1 (file info):** All corpora provide file paths, sizes, extensions, timestamps. Adequate.

**Phase 2 (metadata):** Only arch-wiki has format-specific metadata (wiki categories, related articles in some files). PKM corpora may have YAML frontmatter. Shakespeare has no metadata. **Gap:** no corpus with rich YAML/TOML frontmatter (common in static site generators, Obsidian vaults with properties).

**Phase 3 (heuristic):**
- Section parsing: arch-wiki (rich heading hierarchy), shakespeare (acts/scenes)
- Link extraction: pkm-webdev (wikilinks), pkm-datascience (dense wikilinks), arch-wiki (markdown links), shakespeare (none — baseline)
- Statistical properties: all corpora provide word counts, section counts

**Phase 4 (semantic/LLM):**
- Abstract concept extraction: shakespeare is the critical test (no explicit structure, must infer themes)
- Concept-to-concept edges: arch-wiki (technical relationships between concepts)
- Long-document chunking: shakespeare (3,300-6,286 lines per file) is ideal for chunk + fan-out + synthesize
- Cross-language: pkm-datascience (German terms) tests concept normalization

### Scale gradient

51 → 516 → 2,487 files. Plus 42 massive files (shakespeare, 5.5MB total). Good range for performance testing.

### Gaps identified

1. **No rich frontmatter corpus.** Need files with YAML frontmatter (title, tags, date, categories, custom fields) to test Phase 2 metadata extraction. Common in Hugo/Jekyll blogs, Obsidian with properties.

2. **No code repository.** All corpora are documentation/literature. A code repo with import statements, function definitions, and file dependencies would test a different extraction pattern (dependency graphs, call graphs).

3. **No mixed-media corpus.** All files are text. Images, PDFs, or notebooks would test Phase 1's MIME detection and Phase 2's format-specific parsing.

4. **No multi-format corpus.** Each corpus uses one format. A real-world project might mix .md, .txt, .rst, .html, .pdf.

### Recommendation

The existing corpora are **adequate for the initial research cycle.** They cover the four phases' core behaviors across different document types, link styles, and scales.

For production testing later:
- Add a **frontmatter-rich corpus** (e.g., a sample Hugo blog or Obsidian vault with properties) for Phase 2
- Add a **small code corpus** (e.g., a Rust/Python project with clear module structure) for dependency extraction
- These are Phase 2 additions — not blocking for the architecture research

### Findings

1. **Existing corpora cover all four phases** with adequate variety. Shakespeare is particularly valuable for Phase 4 (semantic-only extraction, long-document chunking).

2. **The scale gradient (51 → 2,487 files)** enables performance testing across orders of magnitude.

3. **Primary gap is frontmatter-rich files** for Phase 2 metadata extraction. Secondary gap is code repositories for dependency-graph extraction.

4. **Gaps are not blocking.** The architecture can be designed and validated with existing corpora. Specialized test data can be added for specific phase testing later.

---

## Follow-on: Sketchbin and EDDI — can non-text adapters use the declarative spec?

**Method:** Design walkthrough — trace the phased extraction model against two real applications that don't fit the text-document mold.

### Case 1: Sketchbin (creative workshop — audio, code, visual artifacts)

Sketchbin ingests creative artifacts across modalities: audio recordings, code sketches, visual compositions, possibly video. Each artifact has modality-specific metadata and creator provenance.

**Phase 1 (file info):** Works identically to text files. MIME type detection gives us `audio/wav`, `image/png`, `text/javascript`, etc. File node with path, size, extension, modified timestamp. **Fully declarative.**

**Phase 2 (metadata):** Modality-dependent:
- **Audio:** Duration, sample rate, channels, format. Possibly embedded ID3/Vorbis tags (title, artist, album, genre). Tools: `ffprobe`, `mutagen` (Python), or Rust crate `symphonia`/`lofty`.
- **Image:** Dimensions, color space, EXIF data (camera, date, GPS). Tools: `exiftool`, Rust crate `kamadak-exif`.
- **Code:** Language detection, import statements, function signatures. Tools: tree-sitter, basic regex.
- **Video:** Duration, resolution, codec, framerate.

Can this be declarative? **Partially.** The metadata extraction itself requires format-specific parsers. But the *mapping* from extracted metadata to graph nodes/edges can be declarative. The architecture needs a two-layer approach:

```
Extractor (format-specific, produces JSON) → Declarative spec (maps JSON to graph)
```

The extractor is a **script agent** (Python/Bash subprocess) or a Rust function. It takes a file path and returns structured JSON:

```json
{
  "mime_type": "audio/wav",
  "duration_seconds": 127.3,
  "sample_rate": 44100,
  "channels": 2,
  "tags": { "title": "Morning sketch", "genre": "ambient" }
}
```

Then the declarative spec maps this JSON to nodes/edges:

```yaml
adapter:
  id: "sketchbin-metadata"
  input_kind: "media-metadata"

  emit:
    - create_node:
        id: "artifact:{input.file_hash}"
        type: "artifact"
        dimension: structure
        properties:
          mime_type: "{input.mime_type}"
          duration: "{input.duration_seconds}"
    - for_each: "{input.tags | entries}"
      as: tag_entry
      emit:
        - create_node:
            id: "concept:{tag_entry.value | lowercase}"
            type: "concept"
            dimension: semantic
        - create_edge:
            source: "artifact:{input.file_hash}"
            target: "concept:{tag_entry.value | lowercase}"
            relationship: "tagged_with"
            weight: 1.0
```

**Phase 3 (heuristic):** Modality-dependent analysis without LLM:
- **Audio:** Spectral analysis, beat detection, loudness profile. Script agent calling Python audio libraries (librosa, essentia).
- **Image:** Color histogram, edge detection, dominant colors, OCR for text-in-image. Script agent calling Python image libraries (Pillow, OpenCV).
- **Code:** AST parsing, dependency graph, complexity metrics. Script agent or Rust (tree-sitter).

These are all **script agents** in llm-orc or dedicated Rust functions. The output is JSON that feeds a declarative spec.

**Phase 4 (semantic/LLM):** This is where it gets interesting for non-text:
- **Audio:** Transcription (Whisper) → text → semantic extraction. Or: audio captioning models that produce descriptive text.
- **Image:** Vision LLM (describe the image, extract concepts, identify objects/scenes/moods).
- **Code:** LLM reads the code and extracts architectural concepts, design patterns, purpose.

Phase 4 for media naturally involves **modality-specific preprocessing** (transcription, captioning) before semantic extraction. This is a DAG: `transcribe → extract_concepts`. Exactly what llm-orc is designed for.

**Verdict for Sketchbin:** The **declarative spec handles the graph-mapping layer** (JSON → nodes/edges). The **format-specific extraction** (audio metadata, spectral analysis, image OCR) lives in script agents or Rust functions that produce JSON. The declarative spec doesn't need to know about audio codecs or image formats — it just maps structured data to graph structure.

**Can Sketchbin's adapter be fully declarative?** The graph mapping: yes. The extraction: no — it requires modality-specific code (script agents for Python audio/image libraries, or Rust crates). But the architecture cleanly separates these: extractors produce JSON, declarative specs consume it.

### Case 2: EDDI (movement analysis — movement encodings)

EDDI ingests movement data with domain-specific metadata: movement quality nodes, temporal dimension, custom edge relationships. The user has their own movement analysis scheme (partially inspired by Laban/Viewpoints, with custom algorithms).

**What does movement data look like?** This is a critical question. Possible formats:
- **Motion capture data** (BVH, C3D, FBX) — joint positions/rotations over time
- **Movement encoding** — the user's own representation of movement qualities (likely structured JSON/data)
- **Video of movement** — requires computer vision preprocessing
- **Annotations** — human-labeled movement qualities/moments

**Phase 1 (file info):** Standard file node. MIME type might be custom for movement encodings, or standard for video/data files.

**Phase 2 (metadata):** Movement-specific metadata: duration, performer, piece name, recording date, movement vocabulary terms. If the encoding format has a header or manifest, this is fast extraction.

**Phase 3 (heuristic):** This is where EDDI's custom algorithms live. The user mentioned "Viewpoints heuristics and my own algorithms." These are domain-specific computational analyses:
- Temporal structure (phrases, sections, transitions)
- Movement quality metrics (the user's own scheme)
- Spatial patterns, repetition detection
- Relationship between performers (if ensemble)

These algorithms are **the core of EDDI's value**. They're not generic — they embody the user's specific analytical framework.

**Phase 4 (semantic/LLM):** LLM analysis of movement data. Likely requires Phase 3's output as context — the LLM reasons about the movement qualities and temporal patterns that the heuristic algorithms detected, discovering higher-level themes and relationships.

**Can EDDI's adapter be declarative?**

The graph mapping (Phase 3 algorithm output → nodes/edges): **yes, mostly declarative.** If the movement analysis algorithms produce structured JSON like:

```json
{
  "qualities": [
    { "name": "sustained-flow", "start_time": 12.3, "end_time": 18.7, "intensity": 0.8 },
    { "name": "bound-effort", "start_time": 15.1, "end_time": 16.4, "intensity": 0.6 }
  ],
  "phrases": [
    { "id": "phrase-1", "start": 0.0, "end": 20.0, "dominant_quality": "sustained-flow" }
  ],
  "transitions": [
    { "from": "phrase-1", "to": "phrase-2", "type": "sudden" }
  ]
}
```

Then a declarative spec maps this to graph nodes/edges:

```yaml
adapter:
  id: "eddi-movement"
  input_kind: "movement-analysis"

  emit:
    - for_each: "{input.qualities}"
      as: quality
      emit:
        - create_node:
            id: "quality:{quality.name}"
            type: "movement-quality"
            dimension: semantic
            properties:
              label: "{quality.name}"
        - create_edge:
            source: "recording:{input.file_hash}"
            target: "quality:{quality.name}"
            relationship: "exhibits"
            weight: "{quality.intensity}"

    - for_each: "{input.phrases}"
      as: phrase
      emit:
        - create_node:
            id: "phrase:{input.recording_id}:{phrase.id}"
            type: "movement-phrase"
            dimension: temporal
            properties:
              start_time: "{phrase.start}"
              end_time: "{phrase.end}"
        - create_edge:
            source: "phrase:{input.recording_id}:{phrase.id}"
            target: "quality:{phrase.dominant_quality}"
            relationship: "characterized_by"
```

The **movement analysis algorithms themselves** are not declarative — they're the user's custom code (likely Python, possibly with specialized movement analysis libraries). These would be script agents in llm-orc or standalone Python processes that EDDI runs.

**Verdict for EDDI:** Same pattern as Sketchbin. The **graph mapping is declarative** (JSON → nodes/edges). The **domain-specific analysis** (movement quality algorithms) is custom code that produces JSON. The declarative spec doesn't need to understand movement — it maps structured analysis results to graph structure.

### The pattern: Extractor + Declarative Mapper

Both cases reveal the same two-layer architecture:

```
Layer 1: Extractor (domain-specific code → JSON)
  - Sketchbin: ffprobe, librosa, Pillow, OpenCV, Whisper
  - EDDI: custom movement analysis algorithms
  - Text docs: parse_sections, extract_links, frontmatter parsing

Layer 2: Declarative Mapper (JSON → graph nodes/edges)
  - Sketchbin adapter spec (YAML)
  - EDDI adapter spec (YAML)
  - Text extraction adapter spec (YAML)
```

Layer 1 is where domain expertise lives. It can be:
- A **script agent** in llm-orc (Python/Bash, for Phase 3-4 extractors)
- A **Rust function** (for Phase 1-2 fast extractors, or performance-critical Phase 3)
- An **external process** the application runs before calling Plexus

Layer 2 is the same for all domains — it's the declarative spec mapping structured data to graph mutations.

### Do these need custom Rust adapters?

**No — if the two-layer architecture is implemented.** The `DeclarativeAdapter` takes JSON input and applies the YAML spec. The domain-specific extraction happens outside the adapter boundary (in script agents, external processes, or Rust functions that produce JSON).

The only reason to write a custom Rust adapter would be:
1. **Performance-critical tight loops** — if the extraction + mapping must be sub-millisecond (unlikely for media analysis)
2. **Rust-only dependencies** — if the extraction requires a Rust crate with no JSON output mode (unlikely)
3. **Complex control flow** — if the mapping logic involves branching/recursion that the template language can't express (possible but unlikely if extractors produce well-structured JSON)

### Are the adapters compatible?

**Yes.** Sketchbin and EDDI adapters would be different YAML specs registered with the same pipeline. Each has its own `input_kind` ("media-metadata", "movement-analysis") and adapter ID. They coexist in the same Plexus instance, same context even — a context could contain both creative artifacts and movement recordings.

Cross-domain enrichment would fire automatically. If a Sketchbin audio recording and an EDDI movement recording both tag the concept "improvisation", CoOccurrenceEnrichment proposes a `may_be_related` edge. The graph discovers connections across modalities without either adapter knowing about the other.

### What the declarative spec needs to support this

The current primitive set (Q2) handles both cases. One addition worth noting:

- **Temporal dimension nodes.** EDDI needs nodes in the temporal dimension (phrases, moments). The `create_node` primitive already supports `dimension: temporal`. No new primitive needed — just documentation that the temporal dimension exists and is usable.

- **Numeric weights from input fields.** The `create_edge` primitive needs to accept `weight: "{quality.intensity}"` — a weight value from the input data, not hardcoded. This is important for EDDI's movement quality intensities and Sketchbin's confidence scores from ML models.

Both of these are already covered by the Q2 primitive set.

### Follow-on: Can enrichments be declarative?

**Method:** Design analysis — reverse-engineer TagConceptBridger and CoOccurrenceEnrichment into declarative patterns, then evaluate what a "MovementBridger" would need.

#### What enrichments actually do

Both built-in enrichments follow the same pattern:

```
1. Trigger: filter graph events for relevant changes
2. Query: inspect the context snapshot for related nodes/edges
3. Guard: check idempotency (does this edge already exist?)
4. Emit: produce new edges (and optionally new nodes)
```

**TagConceptBridger:**
- Trigger: `NodesAdded` events
- Query: for new concept nodes → scan marks for matching tags; for new mark nodes → look up matching concept nodes
- Guard: `references_edge_exists(context, source, target)`
- Emit: cross-dimensional `references` edge (provenance → semantic)

**CoOccurrenceEnrichment:**
- Trigger: `NodesAdded` or `EdgesAdded` events
- Query: build reverse index (source → concepts via `tagged_with` edges), count shared sources per concept pair
- Guard: `may_be_related_exists(context, a, b)`
- Emit: symmetric `may_be_related` edge pairs with normalized score

#### Declarative enrichment spec

The trigger → query → guard → emit pattern can be expressed declaratively:

```yaml
enrichment:
  id: "tag-bridger"

  # Run when these event types occur
  trigger:
    events: [NodesAdded]

  rules:
    # Rule 1: new concept → bridge to existing marks
    - match:
        node_dimension: semantic
        node_id_prefix: "concept:"
      query:
        find_nodes:
          dimension: provenance
          has_property_containing:
            property: "tags"
            value: "{node.label}"
      guard:
        edge_not_exists:
          source: "{found_node.id}"
          target: "{node.id}"
          relationship: "references"
      emit:
        create_edge:
          source: "{found_node.id}"
          target: "{node.id}"
          relationship: "references"
          cross_dimensional: [provenance, semantic]
          weight: 1.0

    # Rule 2: new mark → bridge to existing concepts
    - match:
        node_dimension: provenance
      for_each: "{node.tags | normalize}"
      as: tag
      query:
        find_node:
          id: "concept:{tag}"
      guard:
        edge_not_exists:
          source: "{node.id}"
          target: "concept:{tag}"
          relationship: "references"
      emit:
        create_edge:
          source: "{node.id}"
          target: "concept:{tag}"
          relationship: "references"
          cross_dimensional: [provenance, semantic]
          weight: 1.0
```

This works for TagConceptBridger. But CoOccurrenceEnrichment is harder — it needs:
- A **reverse index build** across all `tagged_with` edges
- **Pairwise combination** of concepts sharing a source
- **Count aggregation** and **score normalization** (count / max_count)
- **Symmetric edge pairs**

This is graph computation, not pattern matching. A declarative spec would need:

```yaml
enrichment:
  id: "co-occurrence"
  trigger:
    events: [NodesAdded, EdgesAdded]

  compute:
    # Build reverse index: source → set of concept targets via tagged_with
    reverse_index:
      edges: { relationship: "tagged_with", target_type: Concept }
      group_by: source
      collect: target

    # For each pair of concepts sharing a source, count shared sources
    pairwise:
      from: reverse_index
      count_shared: true
      normalize: max

    # Emit symmetric edges for each pair
    for_each_pair:
      guard:
        edge_not_exists:
          source: "{pair.a}"
          target: "{pair.b}"
          relationship: "may_be_related"
      emit:
        - create_edge:
            source: "{pair.a}"
            target: "{pair.b}"
            relationship: "may_be_related"
            dimension: semantic
            weight: "{pair.score}"
        - create_edge:
            source: "{pair.b}"
            target: "{pair.a}"
            relationship: "may_be_related"
            dimension: semantic
            weight: "{pair.score}"
```

This is **more complex** than the adapter spec. It introduces graph query primitives (`find_nodes`, `reverse_index`, `pairwise`) and aggregation (`count_shared`, `normalize`). The template language grows from field access + filters to a mini query language.

#### The spectrum: simple bridging vs. graph computation

Enrichments fall on a spectrum:

| Complexity | Example | Declarative? |
|---|---|---|
| **Pattern match** | TagConceptBridger: new node → find matching nodes → bridge | Yes, cleanly |
| **Aggregation** | CoOccurrenceEnrichment: reverse index → pairwise count → normalize | Possible but the spec is as complex as the Rust code |
| **Domain algorithm** | MovementBridger: temporal correlation between movement phrases → quality co-occurrence | Unlikely — domain logic too specific |

#### What would EDDI's MovementBridger do?

Hypothetically, the MovementBridger might:
- Detect movement qualities that co-occur in the same temporal phrase
- Bridge movement-quality nodes to concept nodes (e.g., "sustained-flow" → concept "fluidity")
- Create temporal proximity edges between phrases that share qualities
- Weight edges by the temporal overlap between quality occurrences

The first two are **pattern-matching** — declarative. The last two involve **temporal arithmetic** (overlap calculation, proximity scoring) — domain-specific computation that doesn't fit a generic template language.

#### Recommendation: two tiers

**Tier 1: Declarative enrichments (pattern matching + simple bridging).**
Expressed in YAML. A `DeclarativeEnrichment` struct implements the `Enrichment` trait and interprets the spec. Covers TagConceptBridger and simple bridgers like "when concept X appears, check for concept Y."

Primitives needed:
- `match` — filter events by node/edge properties
- `find_nodes` / `find_edges` — query the context snapshot
- `guard` — idempotency check (edge_not_exists)
- `emit` — create edges/nodes (reuses adapter primitives)

**Tier 2: Rust enrichments (graph computation + domain algorithms).**
CoOccurrenceEnrichment stays in Rust — the declarative spec would be as complex as the code. MovementBridger (EDDI) and any domain-specific enrichment with custom scoring algorithms stays in Rust.

The tier boundary is: **can the enrichment be expressed as "when X appears, bridge to Y"?** If yes, declarative. If it needs aggregation, scoring, or domain math, Rust.

#### Can external consumers write Tier 2 enrichments without Rust?

Not directly — the `Enrichment` trait requires Rust. But there's a workaround:

**Enrichment-as-adapter pattern.** Instead of a Rust enrichment, the domain computation runs as a Phase 3 or Phase 4 extractor (script agent in llm-orc or external process). The extractor queries the graph (via MCP or PlexusApi), computes the domain-specific scores, and emits the results as a new adapter emission.

For EDDI: instead of a MovementBridger enrichment that reacts to graph events, EDDI runs a "movement-correlation" script agent in its Phase 3 pipeline. The script reads the graph's movement-quality nodes and temporal data, computes temporal overlaps and co-occurrence scores, and produces JSON that the declarative mapper turns into edges.

This shifts the work from the enrichment loop (reactive, per-emission) to the extraction pipeline (scheduled, per-file). The tradeoff: enrichments fire automatically after every emission, while the extraction pipeline approach requires explicit scheduling. For domain-specific computation that only makes sense after a full file is processed, the pipeline approach is actually more appropriate — you don't want to re-run expensive temporal correlation on every intermediate emission.

#### Findings

1. **Simple bridging enrichments can be declarative.** TagConceptBridger fits cleanly. The spec is simpler than the Rust code.

2. **Graph computation enrichments stay in Rust.** CoOccurrenceEnrichment's reverse index + pairwise aggregation is better expressed in code than YAML.

3. **Domain-specific enrichments use the enrichment-as-adapter pattern.** EDDI's MovementBridger becomes a Phase 3 script agent that queries the graph and emits correlation edges. This is more natural anyway — domain computation belongs in the extraction pipeline, not the enrichment loop.

4. **Two built-in enrichments cover most cases.** TagConceptBridger (tag → concept bridging) and CoOccurrenceEnrichment (concept co-occurrence) are domain-agnostic. They fire for any adapter's output. Most custom adapters benefit from these without writing custom enrichments.

5. **The enrichment loop remains a Rust-only extension point for truly novel algorithms.** The parameterized built-ins and declarative tier cover most cases. Only algorithms with no existing pattern require Rust.

### Correction: enrichments are graph-wide, not per-file

The earlier suggestion of an "enrichment-as-adapter" pattern for domain-specific enrichments was wrong. It loses the key property of enrichments: they react to new data and update the **full graph**. When EDDI ingests a new recording with quality "sustained-flow", a MovementBridger enrichment should scan the *entire graph* for other recordings that also exhibit "sustained-flow" and bridge them. A per-file extractor can't do this — it only sees its own input.

The correct approach: the two existing enrichments are **generic graph patterns** that happen to be hardcoded. Parameterizing them covers most domain cases.

#### CoOccurrenceEnrichment as a parameterized pattern

Current hardcoded behavior: find Concept nodes that share a source via `tagged_with` edges → emit `may_be_related`.

The underlying algorithm: "find target nodes of type T that share a source via relationship R → emit relationship Z in dimension D."

| Application | R (edge) | T (target type) | Z (output relationship) | D (dimension) |
|---|---|---|---|---|
| Fragment (current) | `tagged_with` | Concept | `may_be_related` | semantic |
| EDDI movement | `exhibits` | movement-quality | `co_exhibited` | semantic |
| Sketchbin | `tagged_with` | Concept | `may_be_related` | semantic |
| Carrel papers | `cited_by` | Paper | `co_cited` | relational |

Sketchbin's adapter uses `tagged_with` edges to connect artifacts to concepts — it gets co-occurrence detection for free with the current enrichment (once generalized past `ContentType::Concept` to any target node). EDDI needs the same algorithm parameterized with different relationship types.

#### TagConceptBridger as a parameterized pattern

Current: bridges provenance↔semantic when a mark's tags match a concept's ID.

Generic pattern: "when a node in dimension A has a property whose value matches a node ID pattern in dimension B, create a cross-dimensional edge."

| Application | Dimension A | Property | Dimension B | ID pattern | Relationship |
|---|---|---|---|---|---|
| Provenance→Concept (current) | provenance | tags | semantic | `concept:{tag}` | `references` |
| Movement→Concept (EDDI) | temporal | qualities | semantic | `concept:{quality}` | `references` |
| Artifact→Concept (Sketchbin) | structure | tags | semantic | `concept:{tag}` | `references` |

Again, Sketchbin gets this for free if its artifacts use the same tagging pattern. EDDI needs parameterization for different dimensions and properties.

#### Three tiers (revised)

**Tier 0 — Parameterized built-ins.** CoOccurrenceEnrichment and TagConceptBridger become configurable with relationship types, target types, dimensions, and property names. Most applications (including Sketchbin and EDDI) configure instances of these patterns rather than writing new enrichments. Registered via the declarative adapter spec:

```yaml
enrichments:
  - type: co_occurrence
    params:
      edge_relationship: "exhibits"
      target_type: "movement-quality"
      output_relationship: "co_exhibited"
      output_dimension: semantic

  - type: dimension_bridge
    params:
      source_dimension: temporal
      source_property: "qualities"
      target_dimension: semantic
      target_id_pattern: "concept:{value}"
      relationship: "references"
```

**Tier 1 — Declarative enrichments (pattern matching).** For bridging patterns not covered by the built-in parameterizations. YAML spec with `match`, `find_nodes`, `guard`, `emit`. Still graph-wide and reactive.

**Tier 2 — Rust enrichments.** For truly novel graph algorithms that can't be expressed as parameterized patterns or declarative rules. Expected to be rare — most enrichments are variations of "bridge nodes that share a key" or "relate nodes that co-occur on a source."

#### Revised finding

**Most "custom enrichments" are actually parameterized instances of two generic patterns.** The MovementBridger isn't a new algorithm — it's CoOccurrenceEnrichment configured with `exhibits` instead of `tagged_with`. The enrichment loop's graph-wide reactivity is preserved because the parameterized enrichment still runs in the loop, scanning the full context snapshot on every emission. No Rust code needed for EDDI or Sketchbin — just configuration.

### Follow-on: Network science enrichments

The parameterized patterns handle local bridging — "nodes that share a key should be connected." But network science algorithms are a different category entirely: PageRank, community detection, betweenness centrality, HITS, label propagation, spectral clustering. These operate on **global graph topology**, not local node patterns.

#### How network science enrichments differ

| Property | Bridging enrichments | Network science enrichments |
|---|---|---|
| Scope | Local (new node + neighbors) | Global (full graph topology) |
| Cost | O(edges touching new node) | O(V + E) or worse |
| Cadence | Per-emission (reactive) | Periodic or on-demand (batch) |
| Output | New edges | Updated properties on existing nodes (scores, labels, rankings) |
| Ecosystem | Simple — Rust is fine | Mature Python libraries (NetworkX, igraph, graph-tool) |
| Convergence | Single pass, idempotent | Iterative (PageRank converges in ~20-50 iterations) |

Running PageRank after every fragment ingestion is wasteful. Community detection after every mark annotation is absurd. These algorithms need the graph to accumulate meaningful structure before they're useful, and they should run at a cadence that matches their cost.

#### Three execution models for enrichments

**1. Reactive enrichments (current enrichment loop).**
Trigger: every emission. Cost: low. Examples: TagConceptBridger, CoOccurrenceEnrichment.
Run in the enrichment loop after each `ingest()` call. Must be fast and idempotent. Local decisions only.

**2. Batch enrichments (network science).**
Trigger: on-demand, periodic, or threshold-based ("run after 100 new nodes"). Cost: moderate to high. Examples: PageRank, community detection, centrality scoring, HITS authority/hub.
Do NOT run in the per-emission enrichment loop. Run as a separate operation that:
- Exports the relevant subgraph
- Executes the algorithm (Python/NetworkX via script agent, or Rust)
- Returns updated node properties (scores, community labels, rankings)
- Applies updates via the adapter pipeline (new `ingest()` call with computed results)

**3. Semantic enrichments (LLM-based, already covered by Phase 4).**
Trigger: scheduled or on-demand. Cost: high. Examples: theme extraction, abstract concept discovery.
Already handled by the Phase 4 → llm-orc integration.

#### Where batch enrichments run

The natural fit: **llm-orc ensembles with script agents.** The same infrastructure that runs Phase 4 semantic extraction can run network science:

```yaml
ensemble:
  name: graph-analysis
  agents:
    - name: export-graph
      type: script
      script: export_subgraph.py
      # Queries Plexus MCP for nodes/edges, outputs adjacency list

    - name: pagerank
      type: script
      script: run_pagerank.py
      depends_on: [export-graph]
      # NetworkX PageRank on adjacency list, outputs node scores

    - name: community-detection
      type: script
      script: run_communities.py
      depends_on: [export-graph]
      fan_out: false
      # Louvain community detection, outputs community labels

    - name: apply-scores
      type: script
      script: format_for_plexus.py
      depends_on: [pagerank, community-detection]
      # Merges results into Plexus emission format
```

PageRank and community detection run in parallel (both depend on export, not each other). Results are merged and applied back to the graph as property updates on existing nodes.

The script agents use mature Python libraries:
```python
import networkx as nx
G = nx.from_edgelist(edges)
scores = nx.pagerank(G, alpha=0.85)
# Output: {"node_id": score, ...}
```

This reuses the llm-orc infrastructure without any LLM calls — the "LLM orchestrator" is also a general DAG executor for script agents. The name is a misnomer for this use case, but the machinery is exactly right.

#### Trigger model for batch enrichments

When should graph analysis run?

**Option A: On-demand.** User or application explicitly requests: "run graph analysis on context X." Simple, predictable. CLI: `plexus analyze my-context`. MCP tool: `analyze_context`.

**Option B: Threshold-based.** After N new emissions (configurable), trigger analysis automatically. E.g., "re-run PageRank after every 50 new nodes." Requires tracking emission count since last analysis.

**Option C: Scheduled.** Cron-like: "run community detection every hour." Requires a scheduler (systemd timer, cron, or in-process timer).

**Option D: Phase-based.** Treat graph analysis as a "Phase 5" that runs after Phase 4 completes for a batch of files. Natural extension of the phased model.

Recommendation: **Option A (on-demand) as the baseline**, with Option B (threshold) as an enhancement. On-demand is simplest and gives the user control. Threshold adds automation when the graph is actively growing (e.g., during a bulk import).

#### What batch enrichments produce

Unlike bridging enrichments (which create new edges), network science enrichments primarily **update properties on existing nodes**:

- **PageRank** → `pagerank_score: 0.034` on each node
- **Community detection** → `community: 7` on each node
- **Betweenness centrality** → `betweenness: 0.15` on each node
- **HITS** → `authority: 0.8, hub: 0.3` on each node

These are `update_properties` operations — the node already exists, we're adding derived metrics. The declarative adapter primitive `update_properties` (from Q2) handles this. The "apply-scores" script agent in the ensemble formats results as property updates, and the declarative mapper applies them.

Some algorithms also produce new edges:
- **Community detection** → `same_community` edges between nodes in the same cluster
- **Link prediction** → `predicted_link` edges for likely-but-missing connections

These use the standard `create_edge` primitive.

#### Findings

1. **Network science enrichments are a third category** — batch, global, expensive. They don't belong in the per-emission enrichment loop.

2. **llm-orc with script agents is the right execution model.** Same infrastructure as Phase 4, but running Python graph algorithms instead of LLM calls. PageRank in NetworkX is ~10 lines of Python.

3. **On-demand trigger is the right starting point.** "Analyze this context" is simpler and more predictable than automatic triggers. Threshold-based automation can be added later.

4. **Output is primarily property updates on existing nodes.** `update_properties` in the declarative spec handles this cleanly.

5. **The three enrichment tiers map to three execution models:**
   - Tier 0 (parameterized built-ins) + Tier 1 (declarative) → reactive enrichment loop
   - Tier 2 (network science) → batch via llm-orc script agents
   - Phase 4 (semantic) → llm-orc LLM agents (already designed)

6. **llm-orc is more than an "LLM orchestrator."** For Plexus, it's a general DAG executor for any expensive computation — LLM calls, network science, media analysis. The Python ecosystem access is the real value, not just LLM API wrappers.
