# llm-orc Integration Test Drive Log

**Date:** 2026-02-16
**Context:** ADRs 019-023 (Phased Extraction) closed out. First end-to-end test of the Plexus → llm-orc graph analysis pipeline via MCP.

---

## Goal

Verify that the graph analysis pipeline works end-to-end: Plexus exports a context graph → llm-orc runs PageRank + community detection scripts → results come back as property updates ready for `GraphAnalysisAdapter` ingestion.

Both the Plexus MCP server and llm-orc MCP server are available in the same Claude Code session, so we can orchestrate the full loop manually.

---

## What Worked

1. **Ensemble creation and validation.** Created `plexus-graph-analysis` ensemble with two parallel script agents (pagerank, community detection). Validated via `validate_ensemble`.

2. **Script execution.** Both Python scripts (`run_pagerank.py`, `run_communities.py`) run correctly when invoked directly via stdin.

3. **End-to-end invocation.** `invoke` on the ensemble with a 5-node, 8-edge graph returned correct results:
   - **PageRank**: all nodes scored 0.2 (correct for symmetric bidirectional graph — equal authority)
   - **Community detection**: found two clusters — `{travel, avignon, provence}` and `{jazz, improv}` — exactly the expected geography/music split

4. **analysis-result schema compliance.** Both scripts output `{"updates": [{"node_id": "...", "properties": {...}}]}`, matching `docs/schemas/analysis-result.schema.json`.

---

## Issues Encountered

### Issue 1: Agent `type: script` required but not documented

**Symptom:** Validation error: "Agent has no model_profile configured"

**Cause:** The ensemble YAML initially had agents without `type: script`. Without this field, llm-orc assumes the agent is an LLM agent and requires a `model_profile`.

**Fix:** Added `type: script` to each agent in the ensemble YAML.

**Lesson:** Script agents must explicitly declare `type: script`. The `create_ensemble` MCP tool doesn't infer this from the presence of a `script` field.

### Issue 2: Script path resolution depends on MCP server cwd

**Symptom:** "Script not found" errors when using relative paths like `scripts/specialized/plexus/run_pagerank.py`.

**Cause:** The llm-orc MCP server resolves script paths relative to its own working directory, which may differ from the `.llm-orc/` project directory. When the MCP server is spawned by Claude Code for the Plexus project, its cwd is the Plexus project root, not the llm-orc project root.

**Fix (temporary):** Used absolute paths in the ensemble YAML. This works for local development but isn't portable.

**Fix (proper):** Keep scripts and ensembles in the consuming project's `.llm-orc/` directory. The llm-orc MCP server resolves paths relative to the `set_project` directory. See [Local .llm-orc Directory](#local-llm-orc-directory) below.

### Issue 3: llm-orc wraps script input in an envelope

**Symptom:** Scripts received `{"input": "<json string>", "parameters": {...}, "context": {}}` instead of raw graph JSON. Scripts looked for `config["nodes"]` at the top level and found nothing → empty results.

**Cause:** llm-orc has two input wrapping formats:

1. **ScriptAgentInput format:** `{"agent_name": "...", "input_data": "<json string>", "context": {}, "dependencies": {}}`
2. **Legacy wrapper format:** `{"input": "<json string or dict>", "parameters": {...}, "context": {}}`

The `invoke` MCP tool uses the legacy format for script agents. The actual graph data is nested inside the `"input"` field, and ensemble-level `parameters` are in the `"parameters"` field.

**Fix:** Added `unwrap_input()` helper to both scripts that handles all three formats (ScriptAgentInput, legacy wrapper, and direct invocation). Returns `(data_dict, parameters_dict)` tuple.

**Lesson:** Any script intended for llm-orc execution must handle input envelope unwrapping. This should be documented or extracted into a shared utility.

### Issue 4: Large JSON payloads may fail silently

**Symptom:** A 5-node graph with extra `type` and `properties` fields on each node returned empty results, while the same graph with minimal node structure (`{"id": "..."}` only) worked.

**Cause:** Likely a JSON escaping or size issue in the MCP transport layer when the `input_data` string parameter is large. The exact threshold wasn't determined.

**Workaround:** Keep node objects minimal in graph exports — `id` is the only required field for the analysis scripts. Additional properties can be included but may hit transport limits for large graphs.

**Status:** Not fully diagnosed. May be a double-escaping issue rather than a size issue. Needs further investigation with stderr logging.

---

## Local .llm-orc Directory

Plexus now has its own `.llm-orc/` directory with:

```
.llm-orc/
├── ensembles/
│   ├── plexus-graph-analysis.yaml    ← NEW: PageRank + community detection
│   ├── plexus-semantic.yaml          ← existing: LLM semantic extraction
│   ├── plexus-semantic-v2.yaml       ← existing
│   └── ... (other existing ensembles)
├── scripts/
│   ├── chunker.sh                    ← existing
│   └── specialized/
│       └── plexus/
│           ├── run_pagerank.py       ← NEW
│           └── run_communities.py    ← NEW
├── profiles/
│   ├── ollama-gemma3-1b.yaml
│   └── ollama-llama3.yaml
└── artifacts/                        ← execution artifacts from test runs
```

The ensemble uses relative paths (`scripts/specialized/plexus/run_pagerank.py`) so it's portable — as long as `set_project` points to the Plexus project root.

The canonical copies also exist in `llm-orchestra-library/scripts/specialized/plexus/` (the llm-orc library submodule) for reuse by other projects.

---

## Script Input Contract

All Plexus scripts for llm-orc must handle the envelope unwrapping pattern:

```python
def unwrap_input(raw_json):
    """Unwrap llm-orc envelope to get actual data and parameters.

    Handles three formats:
    1. ScriptAgentInput: {"agent_name": "...", "input_data": "<json>", ...}
    2. Legacy wrapper:   {"input": "<json or dict>", "parameters": {...}, ...}
    3. Direct:           {"nodes": [...], "edges": [...], ...}

    Returns (data_dict, parameters_dict).
    """
    envelope = json.loads(raw_json) if raw_json.strip() else {}

    # Format 1: ScriptAgentInput
    input_data = envelope.get("input_data", "")
    if isinstance(input_data, str) and input_data.strip():
        try:
            return json.loads(input_data), envelope.get("parameters", {}) or {}
        except json.JSONDecodeError:
            pass

    # Format 2: Legacy wrapper
    if "input" in envelope and "parameters" in envelope:
        inner = envelope["input"]
        params = envelope.get("parameters", {}) or {}
        if isinstance(inner, str) and inner.strip():
            try:
                return json.loads(inner), params
            except json.JSONDecodeError:
                return envelope, params
        if isinstance(inner, dict):
            return inner, params

    # Format 3: Direct
    return envelope, {}
```

---

## Data Flow (Verified)

```
Plexus Context
  │
  ▼
export_graph_for_analysis()  →  graph-export JSON
  │                              (docs/schemas/graph-export.schema.json)
  ▼
llm-orc invoke("plexus-graph-analysis", graph_json)
  │
  ├──► run_pagerank.py    →  {"updates": [{"node_id": "...", "properties": {"pagerank_score": 0.2}}]}
  │
  └──► run_communities.py →  {"updates": [{"node_id": "...", "properties": {"community": 0}}]}
  │
  ▼
Merge results (analysis-result JSON)
  │                         (docs/schemas/analysis-result.schema.json)
  ▼
parse_analysis_response()  →  Vec<(NodeId, PropertyMap)>
  │
  ▼
GraphAnalysisAdapter.process()  →  property update emissions
  │
  ▼
Context (nodes now have pagerank_score, community properties)
```

Steps verified end-to-end through MCP: export → invoke → parse.

---

## Full Round-Trip Verified (2026-02-16, session 2)

### Bug fix: SubprocessClient dropping RunningService

The `SubprocessClient::connect()` method created a `RunningService` from `().serve(transport)` but only stored the `Peer` handle. When `connect()` returned, the `RunningService` was dropped, killing the subprocess. First invocation after handshake got "Transport closed".

**Fix:** Store `RunningService<RoleClient, ()>` alongside the `Peer` in the `SubprocessClient` struct. The service stays alive for the client's lifetime.

### Bug fix: cmd_analyze missing project_dir

`cmd_analyze()` created `SubprocessClient::new()` without calling `.with_project_dir()`. The llm-orc subprocess didn't know where to find `.llm-orc/ensembles/`. Fixed by passing `std::env::current_dir()`.

### Live integration test

Added `live_graph_analysis_round_trip` test (`#[ignore]` — requires llm-orc installed):
- Builds 5-node, 8-edge context (geography + music clusters)
- Spawns real llm-orc subprocess via `SubprocessClient`
- Runs `graph-analysis` ensemble (PageRank + community detection)
- Applies results via `GraphAnalysisAdapter`
- Asserts: PageRank scores present on all nodes, travel/jazz in different communities

**Result: PASS** — full Plexus → llm-orc → Plexus round-trip works.

### Conformance note: standalone Plexus MCP server

~~The `mcp__plexus__*` MCP server available in Claude Code sessions is a standalone provenance tool that exposes `add_mark`, `create_chain`, etc. directly — bypassing the ingest pipeline.~~ **Resolved** in two stages:

1. (commit `e06a649`) Engine MCP server became the active binary with `annotate` as single write path.
2. (commit `fda27ff`) Trimmed MCP surface from 19 to **8 tools** — removed all 11 mark/chain/link management tools that bypassed the ingest pipeline. Marks, chains, and links are internal graph structures managed by the pipeline, not consumer-facing primitives. See [MCP Surface Redesign](#mcp-surface-redesign-2026-02-16-session-4) below.

---

## Semantic Extraction with Fan-Out (2026-02-16, session 3)

### Field name mismatch fixed

All ensemble prompts used `"name"` for concepts and `"confidence"` for relationship weights. `parse_response()` expected `"label"` and `"weight"`. Fixed both sides:
- Ensemble prompts now request `label` and `weight` (canonical names)
- `parse_response()` accepts `name` as fallback for `label`, `confidence` as fallback for `weight` (defensive against LLM output variance)

### JSON extraction from LLM prose

Small models (gemma3:1b) wrap JSON in explanation text and markdown fences. Added `extract_json()` helper that tries:
1. Direct JSON parse
2. Extract from ` ```json ... ``` ` fenced blocks
3. Find first `{` to last `}` span

### Fan-out pipeline verified

Created `semantic-extraction` ensemble with three stages:

```
extract_content.py  →  concept-extractor (fan_out: true)  →  synthesizer
   (read file,           (parallel LLM per chunk)             (merge chunks)
    detect MIME,
    chunk by lines)
```

**Live test result:** Invoked on `README.md`:
- `extract_content.py` read file, detected `text/markdown`, produced 2 chunks
- `concept-extractor[0]` and `[1]` ran in parallel (gemma3:1b, ~7s each)
- `synthesizer` merged results → 5 concepts (plexus, adapter pipeline, provenance tracking, evidence trails, source manifest) + 4 relationships
- `parse_response()` + `extract_json()` correctly parsed the synthesizer output (which included explanation prose around the JSON)
- Total round-trip: ~40s including llm-orc startup + 3 LLM calls

### AgentResult fan-out deserialization fix

Fan-out gathered results have `response: [...]` (array) instead of `response: "..."` (string). Added custom deserializer that accepts both, converting arrays to JSON strings.

### Single-agent extraction also verified

Direct invocation of `plexus-semantic-micro` (single gemma3:1b agent, no fan-out) with inline text:
- 12 concepts extracted from 5-sentence Rust text in 16s
- All concepts semantically relevant (rust, ownership model, borrow checker, tokio, cargo, etc.)

---

## MCP Surface Redesign (2026-02-16, session 4)

### Problem: Invariant 7 violation in MCP tools

The engine MCP server initially exposed 19 tools — including direct management of marks, chains, and links (`list_marks`, `update_mark`, `delete_mark`, `list_chains`, `get_chain`, `archive_chain`, `delete_chain`, `list_tags`, `link_marks`, `unlink_marks`, `get_links`). These tools let consumers create and manipulate provenance structures without going through the ingest pipeline, violating Invariant 7 (dual obligation: semantic content AND provenance).

### Key insight: marks, chains, and links are internal

Marks, chains, and links are **internal graph structures** produced by the ingest pipeline — they are the provenance dimension's implementation, not a consumer-facing API. No external consumer needs to manage them directly. The `annotate` tool goes through `FragmentAdapter` + `ProvenanceAdapter` → enrichment loop, which creates the correct provenance structures automatically.

### Final MCP surface: 8 tools

| Tool | Category | Purpose |
|------|----------|---------|
| `set_context` | Session | Activate a context (auto-creates if needed) |
| `annotate` | Write | Single write path → full ingest pipeline |
| `context_list` | Context | List contexts with sources |
| `context_create` | Context | Create a new context |
| `context_delete` | Context | Delete a context |
| `context_rename` | Context | Rename a context |
| `context_add_sources` | Context | Add file/directory sources |
| `context_remove_sources` | Context | Remove sources |
| `evidence_trail` | Read | Query concept evidence (ADR-013) |

**Commit:** `fda27ff` — `refactor: trim MCP surface to 8 tools (Invariant 7 conformance)`

### Implications for declarative adapters

The MCP surface redesign establishes a pattern that declarative adapters must follow:

1. **Consumers produce structured data, not graph primitives.** The MCP `annotate` tool takes annotation text + metadata. It does NOT take "create this node, create this edge." The adapter pipeline handles graph construction internally.

2. **The two-layer split is validated end-to-end.** Both the llm-orc integration and the MCP redesign confirm the same architecture: external tools produce structured JSON (Layer 1: extractor), and Plexus adapters map that JSON to graph operations (Layer 2: declarative mapper). Consumers never directly manipulate the graph.

3. **Three verified input patterns exist:**
   - **MCP `annotate`:** text + metadata → `FragmentAdapter` → graph
   - **llm-orc graph analysis:** graph-export JSON → scripts → analysis-result JSON → `GraphAnalysisAdapter` → graph
   - **llm-orc semantic extraction:** file path → ensemble (extract + LLM + synthesize) → concepts JSON → `SemanticAdapter` → graph

   All three follow the same shape: **structured JSON in → adapter interprets → emissions out**. This is exactly what `DeclarativeAdapter` formalizes with YAML specs.

4. **The adapter is the boundary, not the transport.** Whether structured JSON arrives via MCP, llm-orc subprocess, or direct Rust API call, the adapter's job is the same: validate input, map to graph primitives, enforce Invariant 7. The declarative spec language should be transport-agnostic.

---

## Next Steps

1. ~~**Investigate large payload issue**~~ — **Open.** Debug error logging has been added to llm-orc. Retry with verbose stderr next time a large graph export fails silently.

2. ~~**Extract `unwrap_input` to shared utility**~~ — **Resolved.** Fixed on the llm-orc side; envelope unwrapping is now handled by the framework, not individual scripts.

3. ~~**Wire `semantic-extraction` ensemble into `SemanticAdapter`**~~ — **Done** (commit `2951a54`). `process()` now prefers the `"synthesizer"` key from the results HashMap, falling back to last agent response.

4. ~~**Database schema migration**~~ — **Done** (commit `98fefbb`). `SqliteStore::open()` auto-detects old `weight` column and rebuilds the edges table with `raw_weight` using create-copy-swap.

5. ~~**Replace standalone Plexus MCP server**~~ — **Done** in two stages: engine MCP server with `annotate` as single write path (commit `e06a649`), then trimmed to 8 tools by removing all mark/chain/link management tools (commit `fda27ff`). Invariant 7 fully enforced.

6. **Declarative adapter primitives (ADR-020)** — **Next.** The llm-orc integration validates the two-layer architecture that declarative specs formalize. All three verified input patterns (MCP annotate, graph analysis, semantic extraction) follow the same shape: structured JSON → adapter → emissions. `DeclarativeAdapter` has 6/7 primitives implemented in Rust; remaining work is YAML parsing, `update_properties`, parameterized enrichment wiring, and declarative enrichments. See the [RDD research cycle](../../research-log.md) for design space exploration.
