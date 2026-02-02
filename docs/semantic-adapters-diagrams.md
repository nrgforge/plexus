# Semantic Adapters: System Diagrams

Companion to [semantic-adapters.md](./semantic-adapters.md). All diagrams use Mermaid syntax.

---

## 1. System Overview

The full Plexus pipeline from external input through graph building to consumer applications.

```mermaid
flowchart TB
    subgraph inputs ["External Inputs"]
        FC["File Change - Manza"]
        TF["Text Fragment - Trellis"]
        GE["Gesture Encoding - EDDI"]
    end

    subgraph adapter_layer ["Adapter Layer"]
        direction TB
        Router["Input Router"]
        SchedMon["Schedule Monitor"]

        subgraph input_adapters ["Input-Triggered Adapters"]
            SA_doc["Document Adapter
            file content"]
            SA_frag["Fragment Adapter
            text fragments"]
            SA_move["Movement Adapter
            gesture encodings"]
        end

        subgraph reflexive_adapters ["Scheduled Adapters"]
            RA_norm["Normalization
            LLM-assisted"]
            RA_topo["Topology
            algorithmic"]
            RA_cohere["Coherence
            LLM-assisted"]
        end

        Sink["AdapterSink
        commit + provenance"]
    end

    subgraph engine ["PlexusEngine"]
        Graph["Graph Store"]
        Events["Event System"]
        Prov["Provenance"]
    end

    subgraph consumers ["Consumer Applications"]
        Manza["Manza - ambient graph viz"]
        Trellis["Trellis - surfaced connections"]
        EDDI["EDDI - environment control"]
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
    Events --> Manza
    Events --> Trellis
    Events --> EDDI
```

---

## 2. Multi-Dimensional Graph Structure

How nodes and edges are organized across dimensions, with cross-dimensional edges providing the connective tissue.

```mermaid
flowchart LR
    subgraph structure ["Structure Dimension"]
        dir1["biomechanics-papers/"]
        file1["laban-analysis.md"]
        file2["reaches.md"]
        sec1["Act 3 Scene 1"]
        dir1 -->|contains| file1
        dir1 -->|contains| file2
        file1 -->|contains| sec1
    end

    subgraph semantic ["Semantic Dimension"]
        c1(["concept:laban-effort"])
        c2(["concept:kinesphere"])
        c3(["concept:sudden"])
        c4(["concept:weight-flow"])
        g1(["gesture-4827"])
        c1 -->|related_to| c4
        c2 -->|extends| c1
        g1 -->|exhibits| c3
    end

    subgraph relational ["Relational Dimension"]
        r1["file1 cites file2"]
    end

    subgraph provenance ["Provenance Dimension"]
        chain1["chain: semantic-run"]
        mark1["mark: LLM identified
        laban-effort"]
        mark2["mark: 0.6 confidence
        on weight-flow"]
        chain1 -->|contains| mark1
        chain1 -->|contains| mark2
    end

    sec1 -.->|found_in| c1
    file1 -.->|found_in| c3
    mark1 -.->|derived| c1
    mark2 -.->|derived| c4

    style structure fill:#e1f5fe,stroke:#0288d1
    style semantic fill:#f3e5f5,stroke:#7b1fa2
    style relational fill:#e8f5e9,stroke:#388e3c
    style provenance fill:#fff3e0,stroke:#f57c00
```

---

## 3. Progressive Emission Pipeline

How a single file change flows through one adapter's internal phases, with each phase emitting through the sink.

```mermaid
sequenceDiagram
    participant App as Application
    participant AL as Adapter Layer
    participant DA as DocumentAdapter
    participant Sink as AdapterSink
    participant Engine as PlexusEngine
    participant UI as Consumer (UI/Client)

    App->>AL: File changed: essay.md
    AL->>DA: process(input, sink, cancel)

    Note over DA: Phase 1: instant
    DA->>Sink: emit(file node, 340KB)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: node_added (file)

    Note over DA: Phase 2: fast (chunking)
    DA->>Sink: emit(section nodes, contains edges)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: nodes_added (5 sections)

    Note over DA: Phase 3: moderate (cross-refs)
    DA->>Sink: emit(citation edges)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: edges_added (references)

    Note over DA: Phase 4: slow (LLM via llm-orc)
    DA->>Sink: emit(concept nodes, thematic edges)
    Sink->>Engine: commit + provenance
    Engine->>UI: event: nodes_added (concepts)

    DA->>AL: Ok(()) — done

    Note over AL: Later: scheduled TopologyAdapter triggers
```

---

## 4. Cross-Modal Concept Bridging

How two independent adapters operating on different modalities converge on shared concept nodes through labels.

```mermaid
flowchart TB
    subgraph doc_path ["DocAdapter Path - Text"]
        paper["laban-analysis.md:87
        the sudden quality of slash"]
        llm1["LLM Extraction
        Tier 3"]
        paper --> llm1
    end

    subgraph move_path ["MovementAdapter Path - Gesture"]
        gesture["gesture-4827
        encoding: 0.8, 0.2, 0.9
        labels: sudden, strong, indirect"]
        map1["Label Mapping
        Tier 1"]
        gesture --> map1
    end

    subgraph shared ["Shared Semantic Dimension"]
        concept_sudden(["concept:sudden"])
        concept_strong(["concept:strong"])
    end

    llm1 -->|"creates or finds"| concept_sudden
    map1 -->|"creates or finds"| concept_sudden
    map1 -->|"creates or finds"| concept_strong

    subgraph reinforcement ["Reinforcement Result"]
        result["concept:sudden
        Sources: 2 - doc, movement
        Reinforcement: MultipleAnalyzers
        Confidence: high"]
    end

    concept_sudden --> result

    style doc_path fill:#e3f2fd,stroke:#1565c0
    style move_path fill:#fce4ec,stroke:#c62828
    style shared fill:#f3e5f5,stroke:#7b1fa2
    style reinforcement fill:#e8f5e9,stroke:#2e7d32
```

---

## 5. Reflexive Adapter: Propose-Don't-Merge Cycle

How the NormalizationAdapter proposes relationships and the graph's own dynamics determine their fate.

```mermaid
flowchart TB
    subgraph discovery ["1. Discovery"]
        scan["NormalizationAdapter scans
        concept nodes with similar labels"]
        pair["Candidate pair found:
        concept:sudden and concept:abrupt"]
        scan --> pair
    end

    subgraph proposal ["2. Proposal"]
        edge["Create may_be_related edge
        weight: 0.3, confidence: 0.15"]
        prov["Provenance mark:
        LLM assessed 0.72 similarity"]
        pair --> edge
        pair --> prov
    end

    subgraph reinforced ["3a. Reinforced Path"]
        r1["Users traverse the edge"]
        r2["Nodes share community membership"]
        r3["Other adapters link to both"]
        r_result["Edge strengthens
        confidence: 0.15 to 0.65
        Connection is real"]
        r1 --> r_result
        r2 --> r_result
        r3 --> r_result
    end

    subgraph decayed ["3b. Decay Path"]
        d1["Nodes in different neighborhoods"]
        d2["No traversals occur"]
        d3["No shared community"]
        d_result["Edge decays
        confidence: 0.15 to 0.01
        Connection was spurious"]
        d1 --> d_result
        d2 --> d_result
        d3 --> d_result
    end

    edge --> reinforced
    edge --> decayed

    style discovery fill:#fff3e0,stroke:#e65100
    style proposal fill:#e3f2fd,stroke:#1565c0
    style reinforced fill:#e8f5e9,stroke:#2e7d32
    style decayed fill:#fbe9e7,stroke:#bf360c
```

---

## 6. Three Application Scenarios: Temporal Profiles

Side-by-side comparison of how Manza, Trellis, and EDDI interact with the same underlying graph engine on different timescales.

```mermaid
flowchart LR
    subgraph manza ["Manza - Continuous (seconds)"]
        direction TB
        m1["File edit"] --> m2["DocumentAdapter emits
        structure, then relations,
        then concepts via LLM"]
        m2 --> m3(["events at each emission"])
        m3 --> m4["File edit again"]
        m4 --> m5["cancel previous, restart
        delta-only on changed chunks"]
    end

    subgraph trellis ["Trellis - Accumulative (weeks)"]
        direction TB
        t1["Fragment arrives
        Week 1"] --> t2["FragmentAdapter emits
        structure + concepts"]
        t2 --> t3["...days pass..."]
        t3 --> t4["Fragment arrives
        Week 3"]
        t4 --> t5["FragmentAdapter emits
        structure + concepts"]
        t5 --> t6["TopologyAdapter triggers:
        community detected"]
        t6 --> t7(["Surface: implicit
        outline harvested"])
    end

    subgraph eddi ["EDDI - Streaming (milliseconds)"]
        direction TB
        e1["Gesture 1"] --> e2["MovementAdapter emits
        node + labels + cluster"]
        e3["Gesture 2"] --> e4["MovementAdapter emits
        node + labels + cluster"]
        e5["Gesture 3"] --> e6["MovementAdapter emits
        node + labels + cluster"]
        e2 --> e7["TopologyAdapter triggers"]
        e4 --> e7
        e6 --> e7
        e7 --> e8(["event: hub_emerged"])
        e7 --> e9(["event: community_formed"])
    end

    style manza fill:#e1f5fe,stroke:#0288d1
    style trellis fill:#e8f5e9,stroke:#388e3c
    style eddi fill:#fce4ec,stroke:#c62828
```

---

## 7. Adapter Trait and Data Flow

The `SemanticAdapter` trait interface and how data flows through it.

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

---

## 8. Concrete Adapter Implementations

How the abstract trait maps to specific adapter instances, organized by input kind and trigger mode.

```mermaid
flowchart TB
    trait["SemanticAdapter trait
    id, name, dimensions,
    input_kind, schedule, process"]

    subgraph input_triggered ["Input-Triggered Adapters (schedule = None)"]
        direction LR
        da["DocumentAdapter
        input_kind: file_content
        Dims: structure, semantic, relational
        Internal phases: file-type detection,
        chunking, cross-refs, LLM via llm-orc"]
        fa["FragmentAdapter
        input_kind: text_fragment
        Dims: structure, semantic
        Internal phases: parsing, LLM extraction"]
        ma["MovementAdapter
        input_kind: gesture_encoding
        Dims: structure, semantic
        Internal phases: node creation,
        label mapping, clustering"]
    end

    subgraph scheduled ["Scheduled Adapters (schedule = Some)"]
        direction LR
        na["NormalizationAdapter
        input_kind: graph_state
        Schedule: MutationThreshold
        Proposes may_be_related edges"]
        ta["TopologyAdapter
        input_kind: graph_state
        Schedule: MutationThreshold
        Communities and hubs"]
        ca["CoherenceAdapter
        input_kind: graph_state
        Schedule: Condition
        Cross-adapter conflicts"]
    end

    trait -.-> input_triggered
    trait -.-> scheduled

    style input_triggered fill:#e1f5fe,stroke:#0288d1
    style scheduled fill:#fff3e0,stroke:#f57c00
```

---

## 9. Ontological vs. Epistemological: Two Kinds of Knowledge

How the provenance dimension relates to the rest of the graph.

```mermaid
flowchart TB
    subgraph ontological ["Ontological: What We Model As Existing"]
        direction LR
        subgraph struct ["Structure"]
            s1["file: laban-analysis.md"]
            s2["section: chapter-3"]
        end
        subgraph sem ["Semantic"]
            c1(["concept:kinesphere"])
            c2(["concept:laban-effort"])
        end
        subgraph rel ["Relational"]
            r1["cites: paper-A to paper-B"]
        end
        s1 -->|contains| s2
        s2 -.->|found_in| c1
        c1 -->|related_to| c2
    end

    subgraph epistemological ["Epistemological: How We Came to Assert It"]
        subgraph prov ["Provenance"]
            chain["chain: extraction-run-03"]
            m1["mark: LLM extracted
            kinesphere from ch3
            confidence: 0.85"]
            m2["mark: Is kinesphere
            distinct from personal-space?
            confidence: 0.6"]
            chain -->|contains| m1
            chain -->|contains| m2
        end
    end

    m1 -.->|derived| c1
    m2 -.->|questions| c1
    m1 -.->|source| s2

    style ontological fill:#f5f5f5,stroke:#616161
    style epistemological fill:#fff8e1,stroke:#f9a825
    style struct fill:#e1f5fe,stroke:#0288d1
    style sem fill:#f3e5f5,stroke:#7b1fa2
    style rel fill:#e8f5e9,stroke:#388e3c
    style prov fill:#fff3e0,stroke:#f57c00
```

---

## 10. EDDI: Streaming Session Flow

How gesture data flows through the system during a live EDDI session, from motion capture to environmental response.

```mermaid
sequenceDiagram
    participant Performer
    participant EDDI
    participant Plexus
    participant Client as Environment Client

    Note over Performer,Client: Session begins

    Performer->>EDDI: continuous motion
    EDDI->>EDDI: segment gesture, encode features
    EDDI->>Plexus: gesture encoding + labels [sudden, strong]

    Plexus->>Plexus: Tier 0 - gesture node created
    Plexus->>Plexus: Tier 1 - concept:sudden found/created

    Performer->>EDDI: continuous motion
    EDDI->>EDDI: segment gesture, encode features
    EDDI->>Plexus: gesture encoding + labels [sudden, light]

    Plexus->>Plexus: Tier 0 - gesture node created
    Plexus->>Plexus: Tier 1 - concept:sudden reinforced

    Note over Plexus: Tier 2 runs clustering
    Plexus->>Plexus: gestures 1,2 cluster together

    Note over Plexus: Tier 4 topology check
    Plexus->>Plexus: concept:sudden becomes hub (3+ connections)
    Plexus->>Client: event hub_emerged (concept:sudden)

    Client->>Client: lighting shifts to match sudden quality

    Performer->>EDDI: continuous motion
    EDDI->>EDDI: segment gesture, encode features
    EDDI->>Plexus: gesture encoding + labels [sustained, heavy]

    Note over Plexus: New community forming
    Plexus->>Plexus: community detected (sustained-heavy)
    Plexus->>Client: event community_formed

    Client->>Client: sound shifts to match new quality
```

---

## 11. Trellis: Accumulative Harvesting

How fragments accumulate over weeks and emergent structure gets surfaced back to the writer.

```mermaid
flowchart TB
    subgraph week1 ["Week 1"]
        f1["the body knows
        before the mind does"]
        f2["proprioception as
        a form of knowledge"]
    end

    subgraph week2 ["Week 2"]
        f3["embodied cognition
        in dance training"]
    end

    subgraph week4 ["Week 4"]
        f4["somatic awareness
        precedes verbal articulation"]
    end

    subgraph semantic ["Semantic Dimension - accumulated"]
        c1(["concept:embodied-knowing"])
        c2(["concept:proprioception"])
        c3(["concept:somatic-awareness"])
        c4(["concept:pre-verbal"])

        c1 -->|related_to| c2
        c1 -->|related_to| c3
        c3 -->|related_to| c4
        c2 -->|related_to| c3
    end

    f1 -.-> c1
    f1 -.-> c4
    f2 -.-> c2
    f2 -.-> c1
    f3 -.-> c1
    f3 -.-> c3
    f4 -.-> c3
    f4 -.-> c4

    subgraph topology ["Tier 4: Topology Analysis"]
        community["Community detected:
        embodied-knowing, proprioception,
        somatic-awareness, pre-verbal"]
        hub["Hub identified:
        concept:embodied-knowing
        4 connections"]
        harvest["Implicit outline harvested:
        4 fragments across 4 weeks
        converge on embodied knowing"]
    end

    semantic --> community
    community --> hub
    hub --> harvest

    subgraph surface ["Surfaced to Writer"]
        msg["Your fragments from the last month
        cluster around embodied knowing --
        the body as a way of understanding
        that precedes language.
        Fragments 1,2,3,4 form a potential outline."]
    end

    harvest --> msg

    style week1 fill:#e3f2fd,stroke:#1565c0
    style week2 fill:#e3f2fd,stroke:#1565c0
    style week4 fill:#e3f2fd,stroke:#1565c0
    style semantic fill:#f3e5f5,stroke:#7b1fa2
    style topology fill:#fff3e0,stroke:#f57c00
    style surface fill:#e8f5e9,stroke:#2e7d32
```

---

## 12. Adapter Layer Orchestration

How the adapter layer routes input, manages sinks, and monitors scheduled adapters.

```mermaid
flowchart TB
    input[/"Input arrives"/]

    input --> route{"Route by input_kind"}

    route -->|file_content| da
    route -->|text_fragment| fa
    route -->|gesture_encoding| ma

    subgraph da ["DocumentAdapter"]
        direction TB
        da_spawn["Spawn with sink + cancel token"]
        da_p1["Phase: file node"]
        da_p2["Phase: chunking"]
        da_p3["Phase: cross-refs"]
        da_p4["Phase: LLM via llm-orc"]
        da_spawn --> da_p1
        da_p1 -->|sink.emit| da_p2
        da_p2 -->|sink.emit| da_p3
        da_p3 -->|sink.emit| da_p4
        da_p4 -->|sink.emit| da_done["Ok - done"]
    end

    subgraph fa ["FragmentAdapter"]
        direction TB
        fa_spawn["Spawn with sink + cancel token"]
        fa_p1["Phase: parse + structure"]
        fa_p2["Phase: LLM extraction"]
        fa_spawn --> fa_p1
        fa_p1 -->|sink.emit| fa_p2
        fa_p2 -->|sink.emit| fa_done["Ok - done"]
    end

    subgraph ma ["MovementAdapter"]
        direction TB
        ma_spawn["Spawn with sink + cancel token"]
        ma_p1["Phase: gesture node"]
        ma_p2["Phase: label mapping"]
        ma_p3["Phase: clustering"]
        ma_spawn --> ma_p1
        ma_p1 -->|sink.emit| ma_p2
        ma_p2 -->|sink.emit| ma_p3
        ma_p3 -->|sink.emit| ma_done["Ok - done"]
    end

    da -->|each emission| engine
    fa -->|each emission| engine
    ma -->|each emission| engine

    engine["PlexusEngine
    commit + provenance + events"]

    engine --> monitor{"Schedule monitor"}

    subgraph scheduled ["Scheduled Adapters"]
        r_norm["NormalizationAdapter
        trigger: 50 new mutations"]
        r_topo["TopologyAdapter
        trigger: 50 new mutations"]
        r_cohere["CoherenceAdapter
        trigger: multi-source concept"]
    end

    monitor -->|condition met| scheduled
    scheduled -->|sink.emit| engine

    style da fill:#e1f5fe,stroke:#0288d1
    style fa fill:#e8f5e9,stroke:#388e3c
    style ma fill:#fce4ec,stroke:#c62828
    style scheduled fill:#fff3e0,stroke:#f57c00
```

---

## 13. Edge Lifecycle: From Proposal to Reinforcement or Decay

The full lifecycle of an edge in the system, from initial creation through reinforcement dynamics.

```mermaid
flowchart TB
    subgraph origin ["Two Origin Paths"]
        direction LR
        ext["External adapter
        produces edge"]
        ref["Reflexive adapter
        proposes may_be_related"]
    end

    ext --> active
    ref --> weak

    active["Active
    weight: 1.0
    confidence: 0.0"]

    weak["Weak Edge
    weight: 0.3
    confidence: 0.15"]

    active -->|"evidence added"| reinforced
    active -->|"no reinforcement"| decaying
    weak -->|"graph dynamics confirm"| reinforced
    weak -->|"no confirmation"| decaying

    reinforced["Reinforced
    strength up
    confidence up"]

    reinforced -->|"more evidence"| reinforced
    reinforced -->|"diverse sources"| strong
    reinforced -->|"reinforcement stops"| decaying

    strong["Strong
    confidence above 0.7
    multiple source types"]

    strong -->|"reinforcement stops"| decaying

    decaying["Decaying
    recency_factor drops
    half-life: context-dependent"]

    decaying -->|"new evidence arrives"| reinforced
    decaying -->|"strength near 0"| negligible

    negligible["Negligible
    effectively invisible
    to queries"]

    style origin fill:#e3f2fd,stroke:#1565c0
    style active fill:#fff3e0,stroke:#f57c00
    style weak fill:#fbe9e7,stroke:#bf360c
    style reinforced fill:#e8f5e9,stroke:#2e7d32
    style strong fill:#c8e6c9,stroke:#1b5e20
    style decaying fill:#ffecb3,stroke:#ff8f00
    style negligible fill:#efebe9,stroke:#795548
```
