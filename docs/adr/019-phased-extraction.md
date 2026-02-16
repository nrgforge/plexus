# ADR-019: Phased Extraction as Regular Adapter Emissions

**Status:** Proposed

**Research:** [Essay 18](../essays/18-phased-extraction-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — extraction phase, extraction coordinator, extraction status

**Depends on:** ADR-001 (adapter architecture), ADR-010 (enrichment loop), ADR-012 (unified ingest pipeline)

---

## Context

Plexus currently accepts pre-tagged fragments — knowledge already extracted by humans or external LLMs. To be useful across domains (creative media, movement analysis, technical documentation), Plexus needs to extract knowledge from raw files: markdown, audio, code, movement encodings, images.

Extraction varies by three orders of magnitude in cost. MIME type detection takes microseconds; YAML frontmatter takes milliseconds; section structure takes seconds; LLM semantic extraction takes minutes. Essay 18 found that every surveyed system (Elasticsearch, Tika, Spotlight, Unstructured.io, LlamaIndex) orders stages cheapest-first — but all process synchronously, blocking until the slowest stage completes.

For Plexus, blocking the caller for Phase 3 (minutes) is unacceptable. The graph should be immediately useful from cheap phases, with expensive phases enriching progressively in the background. This progressive async phased extraction works because of two existing properties: deterministic concept IDs (concepts discovered by multiple phases converge to the same node) and per-adapter contribution tracking (each phase's evidence accumulates in its own slot).

## Decision

### Three extraction phases, each a regular adapter emission

No new execution machinery. Each extraction phase is a separate adapter with its own adapter ID, emitting through the existing pipeline (route → process → enrichment loop → outbound events). Each phase boundary corresponds to a real execution-model difference. Phases are:

- **Phase 1 — Registration** (instant, blocking): File node creation (MIME type, file size, extension, path, modification timestamp) in the structure dimension AND format-specific metadata (YAML frontmatter, ID3 tags, EXIF data) producing concept nodes in the semantic dimension. A single phase because both operations are instant, blocking, in-process, and read the same file. If metadata parsing fails (malformed frontmatter, corrupt EXIF), the adapter catches the error and still emits the file node — fault isolation without separate `ingest()` calls. Uses separate contribution keys within the adapter for file-system evidence vs. metadata evidence when distinction matters.
- **Phase 2 — Analysis** (moderate, background): Modality-dispatched heuristic extraction, no LLM. The coordinator selects the right Phase 2 adapter based on MIME type from Phase 1. Multiple Phase 2 adapters exist — text (section structure, link extraction, term frequency), audio (spectral analysis, beat detection), code (AST parsing), image (color histograms) — each with its own adapter ID. "Phase 2" is a cost tier and execution model, not a single algorithm.
- **Phase 3 — Semantic** (slow, background, LLM): Abstract concepts not present literally — themes, implied relationships. See ADR-021 for integration boundary.

### Extraction coordinator

An adapter handling `extract-file` input kind. Runs Phase 1 synchronously within its `ingest()` call and returns its outbound events immediately. Spawns Phases 2–3 as tokio tasks that call `ingest()` again with their own input kinds when they complete.

Each background phase is a separate `ingest()` call with its own adapter ID and sink — not a continuation of the coordinator's emission. If Phase 3 fails, Phases 1–2 are already persisted (Invariant 30: persist-per-emission). The synchronous semantics of individual `ingest()` calls (ADR-012) are preserved — the coordinator's `ingest()` returns after Phase 1; background phases are independent pipeline runs.

**Phase ordering:** The coordinator spawns Phase 3 only after Phase 2 completes. Phase 3 depends on Phase 2's structural output (sections, extracted terms) as input for intelligent chunking and LLM prompting. The coordinator awaits Phase 2's `ingest()` completion, then spawns Phase 3 with Phase 2's output. Phases 2 and 3 are sequential background tasks, not parallel.

**Modality dispatch:** The coordinator uses Phase 1's MIME type to select the appropriate Phase 2 adapter. A markdown file dispatches to the text heuristic adapter; an audio file dispatches to the audio heuristic adapter. Each Phase 2 adapter has its own adapter ID (e.g., `extract-heuristic-text`, `extract-heuristic-audio`). If no Phase 2 adapter matches the file type, Phase 2 is skipped and the extraction status reflects this.

### Extraction status

A structure-dimension node per file tracks which phases have completed. Queryable by any client via an MCP tool (`extraction_status`). The status node participates in the graph's consistency model — not external state.

### Incremental enrichment

Enrichments fire after each phase's emission independently. When Phase 1 adds concept nodes from metadata, TagConceptBridger creates `references` edges. When Phase 2 adds more concepts from heuristic analysis, CoOccurrenceEnrichment detects new pairs. No "wait for all phases" barrier.

### Concurrency control

`tokio::sync::Semaphore` per phase type prevents resource exhaustion. Conservative defaults: 4 concurrent analysis tasks, 2 concurrent semantic tasks.

## Consequences

**Positive:**

- No new execution machinery — phases compose using existing adapter pipeline
- Progressive value: graph is useful from the moment Phase 1 completes
- Cross-phase evidence diversity strengthens raw weight automatically via contribution tracking
- Each phase's failure is isolated — earlier phases' results survive
- Incremental enrichment fires naturally after each phase

**Negative:**

- Background phases calling `ingest()` create a self-triggering pattern (adapter spawns more adapter runs). Must ensure this doesn't create unbounded recursion — each background phase is terminal (it doesn't spawn further phases).
- The extraction coordinator is a coordination adapter that knows about other adapters' input kinds. This creates a coupling between the coordinator and the phase adapters — changing a phase's input kind requires updating the coordinator.

**Neutral:**

- ADR-001 Decision 1 says "an adapter owns its entire processing pipeline." The extraction coordinator is a departure from this principle — it spawns other adapters by calling `ingest()` with their input kinds, creating inter-adapter coupling. This is an accepted tradeoff: the coordinator is an adapter-level coordination pattern, not a framework mechanism. Each phase adapter still owns its own pipeline; the coordinator only triggers them. The coupling is narrow — the coordinator knows phase adapters' input kinds but not their internals. ADR-001's "LLM delegation (via llm-orc)" example envisioned delegation as internal to a single adapter; the phased model externalizes it as separate adapter runs for the benefit of progressive persistence and independent failure isolation.
- Outbound events from background phases are not returned to the original caller (the consumer who triggered extraction). The original caller receives Phase 1 events synchronously. Phase 2–3 events are available via event cursors (OQ8) when implemented, or via polling `extraction_status`.
- Essay 18 originally proposed four phases (file info, metadata, heuristic, semantic). The metadata phase was merged into Phase 1 (Registration) because both operations are instant, blocking, in-process, and read the same file. The boundary between them was a "what they produce" distinction, not an execution-model distinction. Every surveyed system (Tika, Unstructured, LlamaIndex) unifies detect+metadata into one parsing step. The three remaining phase boundaries each correspond to a real execution-model difference: sync/async (Phase 1 vs 2) and Rust/external-service (Phase 2 vs 3).
