# ADR-023: Graph Analysis as External Batch Computation

**Status:** Accepted

**Research:** [Essay 18](../essays/18-phased-extraction-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — graph analysis

**Depends on:** ADR-010 (enrichment loop), ADR-012 (unified ingest pipeline), ADR-021 (llm-orc integration)

---

## Context

Network science algorithms — PageRank, community detection, betweenness centrality, HITS, label propagation — operate on global graph topology and are computationally expensive. Running PageRank after every fragment ingestion is wasteful. These algorithms have a fundamentally different execution cadence from reactive enrichments.

Essay 18 identified this as a distinct concept from enrichment. Enrichments are reactive (per-emission, in the enrichment loop, terminating via idempotency). Graph analysis is batch (on-demand or threshold-triggered, outside the enrichment loop, entering results via `ingest()`).

## Decision

### Graph analysis is not enrichment

Graph analysis runs as llm-orc ensembles with script agents, not in the per-emission enrichment loop. The enrichment loop (Invariant 36) governs reactive enrichments. Graph analysis operates outside it (Invariant 49).

Results are property updates on existing nodes (`pagerank_score: 0.034`, `community: 7`) applied back through the adapter pipeline via `ingest()` (Invariant 34: all writes through ingest). A thin adapter with a stable ID (e.g., `graph-analysis:pagerank`, `graph-analysis:community`) receives llm-orc's analysis output and emits `update_properties` mutations. Per Invariant 13, graph analysis adapter IDs must be stable across sessions for contribution tracking.

### llm-orc ensemble pattern

```yaml
ensemble:
  name: graph-analysis
  agents:
    - name: export-graph
      type: script
      script: export_subgraph.py

    - name: pagerank
      type: script
      script: run_pagerank.py
      depends_on: [export-graph]

    - name: community-detection
      type: script
      script: run_communities.py
      depends_on: [export-graph]

    - name: apply-scores
      type: script
      script: format_for_plexus.py
      depends_on: [pagerank, community-detection]
```

Independent algorithms (pagerank, community-detection) run in parallel. Python's NetworkX handles computation. The DAG structure is llm-orc's existing capability.

### Data Contracts

The graph analysis boundary uses structured JSON with formal schemas:

- **Graph export** (`docs/schemas/graph-export.schema.json`): `export_graph_for_analysis()` serializes context nodes (id, type, dimension, label) and edges (source, target, relationship, weight) as JSON. This is the input to llm-orc's graph analysis ensemble scripts.
- **Analysis result** (`docs/schemas/analysis-result.schema.json`): Each script agent returns JSON with an `updates` array of `{node_id, properties}` objects. `parse_analysis_response()` deserializes these into `GraphAnalysisInput` for ingestion.

Both schemas use JSON Schema draft-07 for validation.

### Trigger model

Triggered on-demand (`plexus analyze my-context`) or by threshold ("after 50 new nodes"). Not reactive per-emission. The specific trigger mechanism (CLI command, MCP tool, threshold monitor) is an implementation detail deferred to the build phase.

## Consequences

**Positive:**

- Network science computation stays in Python (NetworkX) where the ecosystem is mature
- Parallel execution of independent algorithms via llm-orc's DAG model
- Results enter through `ingest()` — they get contribution tracking, enrichment loop, and all pipeline guarantees
- No per-emission overhead — graph analysis runs only when requested or when threshold is met

**Negative:**

- Requires llm-orc running as a service (same dependency as Phase 3). When llm-orc is unavailable, graph analysis cannot run.
- The export step (subgraph → NetworkX format) requires serialization that doesn't exist yet. The graph export format is not defined.
- Graph analysis produces property updates on existing nodes. The current `update_properties` primitive assumes the target node exists. If the graph has changed between export and result application, stale node references may cause rejections.

**Neutral:**

- This confirms llm-orc's role beyond LLM orchestration — it's a general DAG executor for any expensive Python computation. The "LLM" in the name is historical; the Python ecosystem access is the real value.
- Graph analysis results do not trigger further graph analysis (no cascade). They enter via `ingest()`, which includes the enrichment loop as part of its standard pipeline — if a property update changes something an enrichment cares about, the enrichment fires. This is correct behavior, not a concern.
