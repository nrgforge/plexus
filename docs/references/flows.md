# Flow Diagrams

The three flows consumers (and the M3 probes) had to reverse-engineer,
drawn once. Each diagram names the code that implements it.

## 1. Weight pipeline — contribution → raw weight → normalized weight

Three layers (ADR-003, Invariant 8). Only the first is stored.

```mermaid
flowchart LR
    subgraph emission [Emission time]
        A["Adapter emits edge<br/>combined_weight = its value<br/>(or WeightSpec template, ADR-043-era #7)"]
        L["Enrichment emits edge<br/>with explicit contributions map<br/>(e.g. lens per-source keys)"]
        A --> S
        L --> S
        S{"EngineSink<br/>contributions empty?"}
        S -- yes --> S1["insert emitter slot:<br/>contributions[adapter_id] = weight"]
        S -- "no (issue #13)" --> S2["respect explicit map<br/>(no emitter slot)"]
    end
    S1 --> R
    S2 --> R
    subgraph recompute ["Commit time (every emit)"]
        R["recompute_combined_weights<br/>(ADR-043 max-abs)"]
        R --> R1["per contributor:<br/>normalized = value / max|values|<br/>ratios + sign preserved"]
        R1 --> R2["raw_weight = Σ normalized<br/>across contributors"]
    end
    R2 --> Q["Query time:<br/>NormalizationStrategy<br/>(OutgoingDivisive, Softmax)<br/>— never stored"]
```

- Stored layer: `edge.contributions` (`src/adapter/sink/engine_sink.rs`)
- Computed layer: `Context::recompute_combined_weights` (`src/graph/context.rs`)
- Query layer: `src/query/normalize.rs`

## 2. Ingest → enrichment loop → lens translation

The write path, foreground and background (issue #5 unified them).

```mermaid
flowchart TD
    I["ingest(context, input_kind, data)"] --> RC["ADR-017 §2 coherence:<br/>reload_if_changed (data_version)"]
    RC --> SY["sync_spec_lenses:<br/>register/deregister lenses from<br/>the specs table (issues #10/#11)"]
    SY --> RT["route by input_kind (fan-out)"]
    RT --> AD["adapter.process → sink.emit<br/>(commit + persist per emission)"]
    AD --> EV["GraphEvents"]
    EV --> EL["enrichment loop<br/>(run to quiescence)"]
    EL --> CE["core enrichments:<br/>CoOccurrence, TemporalProximity<br/>(fragment-scoped default, #6),<br/>DiscoveryGap, Embedding*"]
    EL --> LE["lens enrichments:<br/>from-relationships → lens:{consumer}:{to}<br/>min_corroboration threshold (#4)"]
    CE --> EL
    LE --> EL
    EL --> OE["adapter.transform_events<br/>→ outbound events"]

    subgraph background [extract-file background phases]
        BG["structural + semantic tasks<br/>emit via EngineSink"] --> BE["run_background_enrichment:<br/>same loop, pipeline's live<br/>registry cell (issue #5 / T11)"]
    end
    BE -.->|"same enrichments,<br/>lenses included"| EL
```

- Entry + sync: `IngestPipeline::ingest` / `sync_spec_lenses` (`src/adapter/pipeline/ingest.rs`)
- Loop: `run_enrichment_loop` (`src/adapter/enrichment/enrichment_loop.rs`)
- Background: `run_background_enrichment` (`src/adapter/adapters/extraction.rs`)
- Lens: `LensEnrichment::enrich` (`src/adapter/enrichments/lens.rs`)

## 3. explain_edge — "why is this connection here?"

One call replaces the three-query reverse-engineering the M3 probes did
(issue #14).

```mermaid
flowchart LR
    Q["explain_edge(source, target,<br/>relationship?)"] --> N["resolve both endpoints<br/>+ displayable text/label"]
    Q --> E["collect ALL edges between<br/>the pair, both directions<br/>(parallel edges included — #12)"]
    E --> P["per edge:<br/>relationship, direction, raw_weight,<br/>stored contributions, corroboration"]
    P --> T["lens edges: parse<br/>lens:{consumer}:{to}:{from} keys<br/>→ translated_from"]
    N --> OUT["EdgeExplanation (JSON)"]
    T --> OUT
```

- `src/query/explain.rs` (`explain_pair`), surfaced as `PlexusApi::explain_edge`
  and the `explain_edge` MCP tool.
