# ADR-021: Phase 3 Integration via llm-orc Service

**Status:** Accepted

**Research:** [Essay 18](../essays/18-phased-extraction-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — extraction phase, extractor, graph analysis

**Depends on:** ADR-019 (phased extraction)

---

## Context

Phase 3 (semantic extraction) is the only extraction phase requiring LLM calls. Essay 18 evaluated three integration options:

- **Option A: CLI subprocess** — spawn llm-orc per file. Cold-start penalty (~1s) per invocation. Unacceptable for batch extraction of hundreds of files.
- **Option B: Port to Rust** — rewrite llm-orc's DAG execution in Rust. Framework overhead is under 200ms; LLM API calls are 500ms–5s. Porting saves <200ms on operations taking 2–20s. The bottleneck is network I/O to the LLM, not framework overhead.
- **Option C: Persistent MCP service** — llm-orc runs as a persistent HTTP service; Plexus calls it for Phase 3 work. Zero cold-start penalty for subsequent calls. Recommended.

llm-orc is a Python-based DAG orchestrator with fan-out, script agents, model fallback chains, and artifact tracking. Its Python ecosystem access (NetworkX, librosa, Whisper, custom analysis libraries) is the real value beyond LLM orchestration.

## Decision

### llm-orc as persistent service

llm-orc runs as a persistent MCP service. Plexus calls it via HTTP for Phase 3 work. Phases 1–2 stay pure Rust. The boundary between Phase 2 and Phase 3 is a natural integration seam — Phase 2 produces structural output (sections, extracted terms, statistics) that becomes Phase 3's input. This is also a technology boundary: Phases 1–2 are Rust in-process; Phase 3 crosses a process boundary to an external Python service.

### Thin Phase 3 adapter

The Phase 3 adapter in Plexus is thin: serialize Phase 2 output to JSON, call llm-orc's `invoke` endpoint, deserialize the response into an Emission. The complex work — chunking, fan-out, LLM prompting, synthesis — lives in llm-orc ensemble YAML and script agents.

### Graceful degradation

When llm-orc is not running, Phases 1–2 complete normally and Phase 3 is skipped. The graph is useful but not semantically enriched. No hard failure. (Invariant 47.)

### Long documents: chunk → fan-out → synthesize

Documents too large for a single LLM context (e.g., a 6,000-line Shakespeare play) use Phase 2's structural boundaries (acts, scenes, sections) for intelligent chunking rather than naive text splitting. llm-orc's `fan_out: true` runs parallel semantic extraction per chunk. A synthesis agent merges chunk-level concepts into file-level themes.

Each chunk emission persists independently. If extraction fails on chunk 7 of 12, chunks 1–6 are already in the graph. Progressive and resilient.

### Data Contracts

The Phase 2→3 boundary uses structured JSON with formal schemas:

- **Phase 2 output** (`docs/schemas/phase2-output.schema.json`): `SemanticAdapter::build_input()` serializes file path, section boundaries, existing concepts, and file metadata as JSON. This is the input to llm-orc's semantic extraction ensemble.
- **Phase 3 result** (`docs/schemas/phase3-result.schema.json`): `SemanticAdapter::parse_response()` expects JSON with `concepts` (label + confidence) and `relationships` (source, target, relationship type, weight). This is the output from llm-orc's final synthesis agent.

Both schemas use JSON Schema draft-07 for validation.

## Consequences

**Positive:**

- Leverages llm-orc's mature DAG execution, fan-out, and Python ecosystem without porting to Rust
- Single HTTP call per file — thinnest possible integration
- Graceful degradation means the system works without llm-orc running
- Intelligent chunking using Phase 2 structural output rather than naive splitting

**Negative:**

- Runtime dependency on an external service for full semantic enrichment. Operational complexity — llm-orc must be running for Phase 3 to execute.
- Phase 2→3 data handoff requires a serialization format for structural output. This format is not yet defined.
- llm-orc ensemble YAML and script agents for Plexus extraction are not yet written. The orchestration logic (chunking strategy, prompt engineering, synthesis) lives outside the Plexus codebase.

**Neutral:**

- llm-orc's role extends beyond LLM orchestration — it becomes a general DAG executor for any expensive computation (LLM calls, network science, media analysis). See ADR-023.
- The Phase 2→3 boundary could move: if future Rust LLM libraries become sufficiently capable, Phase 3 could be brought into Rust without changing the extraction architecture. The boundary is a deployment choice, not an architectural constraint.
- **Shared contract language:** The Phase 2→3 serialization format and the llm-orc agent output format should both use JSON Schema. If llm-orc agents declare their output schema in the ensemble YAML (as JSON Schema), Plexus adapter specs can reference the same schema as their `input_schema`. The contract between extractor (Layer 1) and mapper (Layer 2) becomes a shared, validatable artifact rather than an ad-hoc JSON blob. This is a design direction for the llm-orc side — adding `output_schema` declarations to ensemble agent definitions.
