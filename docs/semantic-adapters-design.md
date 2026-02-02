# Semantic Adapters: System Design

Technical design for the adapter layer. See [semantic-adapters.md](./semantic-adapters.md) for conceptual overview.

---

## System Overview

```mermaid
flowchart TB
    subgraph inputs ["External Inputs"]
        FC["File Change"]
        TF["Text Fragment"]
        GE["Gesture Encoding"]
    end

    subgraph adapter_layer ["Adapter Layer"]
        Router["Input Router"]
        SchedMon["Schedule Monitor"]

        subgraph input_adapters ["Input-Triggered"]
            SA_doc["DocumentAdapter"]
            SA_frag["FragmentAdapter"]
            SA_move["MovementAdapter"]
        end

        subgraph scheduled_adapters ["Scheduled"]
            RA_norm["NormalizationAdapter"]
            RA_topo["TopologyAdapter"]
            RA_cohere["CoherenceAdapter"]
        end

        Sink["AdapterSink"]
    end

    subgraph engine ["PlexusEngine"]
        Graph["Graph Store"]
        Events["Event System"]
        Prov["Provenance"]
    end

    FC --> Router
    TF --> Router
    GE --> Router

    Router --> SA_doc
    Router --> SA_frag
    Router --> SA_move

    SA_doc --> Sink
    SA_frag --> Sink
    SA_move --> Sink

    Sink --> Graph
    Sink --> Prov

    Graph --> SchedMon
    SchedMon --> RA_norm
    SchedMon --> RA_topo
    SchedMon --> RA_cohere
    RA_norm --> Sink
    RA_topo --> Sink
    RA_cohere --> Sink

    Graph --> Events
```

---

## Adapter Trait

```mermaid
classDiagram
    class SemanticAdapter {
        <<trait>>
        +id() str
        +name() str
        +dimensions() Vec~str~
        +input_kind() str
        +schedule() Option~Schedule~
        +process(AdapterInput, AdapterSink, CancellationToken)
    }

    class AdapterSink {
        <<trait>>
        +emit(AdapterOutput)
    }

    class AdapterInput {
        +context_id: ContextId
        +data: AdapterData
        +trigger: AdapterTrigger
        +previous: Option~AdapterSnapshot~
    }

    class AdapterOutput {
        +nodes: Vec~Node~
        +edges: Vec~Edge~
        +removals: Vec~NodeId~
        +provenance: Vec~ProvenanceEntry~
    }

    class AdapterData {
        <<enum>>
        FileContent
        TextFragment
        GestureEncoding
        GraphState
        Structured
    }

    class Schedule {
        <<enum>>
        Periodic
        MutationThreshold
        Condition
    }

    class AdapterTrigger {
        <<enum>>
        FileChanged
        FragmentReceived
        GestureSegmented
        Scheduled
        Manual
    }

    class ProvenanceEntry {
        +description: String
        +entry_type: ProvenanceEntryType
        +explains: Vec~NodeId~
        +confidence: f32
        +source_location: Option~SourceLocation~
    }

    SemanticAdapter ..> AdapterInput : receives
    SemanticAdapter ..> AdapterSink : emits through
    AdapterSink ..> AdapterOutput : receives
    AdapterInput *-- AdapterData
    AdapterInput *-- AdapterTrigger
    AdapterOutput *-- ProvenanceEntry
    SemanticAdapter *-- Schedule
```

### Bootstrap Trait (Rust)

Minimal code to start implementation. The class diagram above is the source of truth for the full model.

```rust
#[async_trait]
pub trait SemanticAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn dimensions(&self) -> Vec<&str>;
    fn input_kind(&self) -> &str;
    fn schedule(&self) -> Option<Schedule> { None }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
        cancel: &CancellationToken,
    ) -> Result<(), AdapterError>;
}

pub trait AdapterSink: Send + Sync {
    fn emit(&self, output: AdapterOutput) -> Result<(), AdapterError>;
}

pub enum Schedule {
    Periodic { interval_secs: u64 },
    MutationThreshold { count: usize },
    Condition(Box<dyn Fn(&GraphSummary) -> bool + Send + Sync>),
}
```

### Relationship to ContentAnalyzer

`ContentAnalyzer` becomes an internal implementation detail. The recommended path: a single DocumentAdapter that wraps existing analyzers as internal phases, emitting through the sink as each completes.

---

## Concrete Adapters

```mermaid
flowchart TB
    subgraph input_triggered ["Input-Triggered (schedule = None)"]
        direction LR
        da["DocumentAdapter
        input_kind: file_content
        dims: structure, semantic, relational"]
        fa["FragmentAdapter
        input_kind: text_fragment
        dims: structure, semantic"]
        ma["MovementAdapter
        input_kind: gesture_encoding
        dims: structure, semantic"]
    end

    subgraph scheduled ["Scheduled (schedule = Some)"]
        direction LR
        na["NormalizationAdapter
        input_kind: graph_state
        trigger: MutationThreshold"]
        ta["TopologyAdapter
        input_kind: graph_state
        trigger: MutationThreshold"]
        ca["CoherenceAdapter
        input_kind: graph_state
        trigger: Condition"]
    end

```

---

## Progressive Emission Sequence

How a file change flows through one adapter's internal phases.

```mermaid
sequenceDiagram
    participant App as Application
    participant AL as Adapter Layer
    participant DA as DocumentAdapter
    participant Sink as AdapterSink
    participant Engine as PlexusEngine
    participant UI as Consumer

    App->>AL: File changed: essay.md
    AL->>DA: process(input, sink, cancel)

    Note over DA: Phase 1: instant
    DA->>Sink: emit(file node)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: node_added

    Note over DA: Phase 2: chunking
    DA->>Sink: emit(section nodes, contains edges)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: nodes_added

    Note over DA: Phase 3: cross-refs
    DA->>Sink: emit(citation edges)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: edges_added

    Note over DA: Phase 4: LLM via llm-orc
    DA->>Sink: emit(concept nodes, thematic edges)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: nodes_added

    DA->>AL: Ok(()) done

    Note over AL: Later: TopologyAdapter triggers
```

---

## Multi-Dimensional Graph

```mermaid
flowchart LR
    subgraph structure ["Structure"]
        dir1["biomechanics-papers/"]
        file1["laban-analysis.md"]
        sec1["chapter 3"]
        dir1 -->|contains| file1
        file1 -->|contains| sec1
    end

    subgraph semantic ["Semantic"]
        c1(["concept:laban-effort"])
        c3(["concept:sudden"])
        g1(["gesture-4827"])
        g1 -->|exhibits| c3
    end

    subgraph provenance ["Provenance"]
        mark1["mark: LLM extracted
        laban-effort, conf 0.85"]
    end

    sec1 -.->|found_in| c1
    mark1 -.->|derived| c1

```

---

## Adapter Layer Orchestration

```mermaid
flowchart TB
    input[/"Input arrives"/]
    input --> route{"Route by input_kind"}

    route -->|file_content| da["DocumentAdapter
    spawn with sink + cancel"]
    route -->|text_fragment| fa["FragmentAdapter
    spawn with sink + cancel"]
    route -->|gesture_encoding| ma["MovementAdapter
    spawn with sink + cancel"]

    da -->|each emission| engine
    fa -->|each emission| engine
    ma -->|each emission| engine

    engine["PlexusEngine
    commit + provenance + events"]

    engine --> monitor{"Schedule monitor"}
    monitor -->|condition met| scheduled["Scheduled Adapters
    spawn with sink + cancel"]
    scheduled -->|each emission| engine

```

**Input-triggered path:** Route by `input_kind` → spawn adapter with sink + cancel token → each `sink.emit()` commits mutations, creates provenance marks, fires events.

**Scheduled path:** Monitor trigger conditions → when met, invoke `process()` with GraphState input → same commit/event pipeline.

**Cancellation:** When new input supersedes in-flight work, cancel the token. Already-emitted mutations remain valid.

---

## Edge Lifecycle

```mermaid
flowchart TB
    subgraph origin ["Two Origin Paths"]
        direction LR
        ext["External adapter"]
        ref["Reflexive adapter"]
    end

    ext --> active["weight: 1.0
    confidence: 0.0"]
    ref --> weak["weight: 0.3
    confidence: 0.15"]

    active -->|"evidence"| reinforced
    active -->|"no activity"| decaying
    weak -->|"confirmed"| reinforced
    weak -->|"no confirmation"| decaying

    reinforced["Reinforced"] -->|"diverse sources"| strong["Strong
    confidence > 0.7"]
    reinforced -->|"stops"| decaying

    strong -->|"stops"| decaying

    decaying["Decaying
    half-life: per-context"] -->|"new evidence"| reinforced
    decaying -->|"near zero"| negligible["Negligible"]

```

---

## Design Decisions

1. **Adapters are coarse-grained.** One adapter owns its full pipeline. Internal phase ordering, file-type branching, and llm-orc delegation are the adapter's business.

2. **Sink-based progressive emission.** `sink.emit()` commits immediately and fires events. The graph is always partially built — that's correct, not an error state.

3. **Two trigger modes.** Input-triggered (run on matching input) and scheduled (run on timer/threshold/condition). Same `process(input, sink, cancel)` interface.

4. **Cancellation via token.** Long-running adapters check a cancel token periodically. Already-emitted mutations remain valid.

5. **Semantic dimension is shared.** All domains contribute concept nodes to the same namespace. `ContentType` disambiguates origin. This enables cross-modal bridging.

6. **Labels bridge modalities.** Shared vocabulary in the semantic dimension — no special unification logic.

7. **Decay is per-context.** Same adapter, different decay in Manza vs Trellis vs EDDI.

8. **Reflexive adapters propose, don't merge.** Weak `may_be_related` edges. Graph dynamics (Hebbian reinforcement/decay) determine what's real.

9. **Cross-adapter dependency via graph state.** External adapters are independent. Reflexive adapters depend on accumulated graph state, not specific adapter outputs.

---

## Open Questions

1. **AdapterSnapshot design.** What does incremental state look like per domain? File: chunk hashes + output node IDs. Movement: cluster centroids. Graph state: timestamp of last run. Likely adapter-specific.

2. **Chunking as graph nodes.** Should chunks be structure-dimension nodes (queryable, referenceable) or adapter-internal state (looser coupling)?

3. **Canonical pointers vs pure emergence.** When `may_be_related` strengthens to high confidence — designate one node as canonical, or keep both with a strong equivalence edge?

4. **Edge garbage collection.** Negligible edges persist indefinitely. Cleanup threshold needed, or is accumulation intentional?

5. **Session boundaries (EDDI).** Separate session contexts? Same context with temporal windowing? Session metadata on nodes/edges?
