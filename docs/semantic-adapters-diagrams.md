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
        Scheduler["Tier Scheduler"]

        subgraph external_adapters ["External Adapters"]
            SA_struct["Structure Adapter
            Tier 0-1"]
            SA_rel["Relational Adapter
            Tier 2"]
            SA_sem["Semantic Adapter
            Tier 3, LLM"]
            SA_move["Movement Adapter
            Tier 1-2"]
        end

        subgraph reflexive_adapters ["Reflexive Adapters"]
            RA_norm["Normalization
            Tier 4, LLM"]
            RA_topo["Topology
            Tier 4, algorithmic"]
            RA_cohere["Coherence
            Tier 4, LLM"]
        end

        Merger["Output Merger"]
        ProvGen["Provenance Generator"]
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

    Router --> Scheduler
    Scheduler --> SA_struct
    Scheduler --> SA_rel
    Scheduler --> SA_sem
    Scheduler --> SA_move

    SA_struct --> Merger
    SA_rel --> Merger
    SA_sem --> Merger
    SA_move --> Merger

    Merger --> ProvGen
    ProvGen --> Graph
    ProvGen --> Prov

    Graph --> RA_norm
    Graph --> RA_topo
    Graph --> RA_cohere
    RA_norm --> Merger
    RA_topo --> Merger
    RA_cohere --> Merger

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

## 3. Tiered Knowledge Pipeline

How a single file change cascades through processing tiers, with each tier emitting events as it completes.

```mermaid
sequenceDiagram
    participant App as Application
    participant AL as Adapter Layer
    participant T0 as Tier 0: Instant
    participant T1 as Tier 1: Fast
    participant T2 as Tier 2: Moderate
    participant T3 as Tier 3: Slow (LLM)
    participant T4 as Tier 4: Background
    participant Engine as PlexusEngine
    participant UI as Consumer (UI/Client)

    App->>AL: File changed: essay.md
    AL->>T0: Route to StructureAdapter
    T0->>Engine: file node (exists, 340KB)
    Engine->>UI: event: node_added (file)

    AL->>T1: Route to StructureAdapter (chunking)
    T1->>Engine: section nodes, contains edges
    Engine->>UI: event: nodes_added (5 sections)
    T1-->>AL: chunk boundaries for downstream

    AL->>T2: Route to RelationalAdapter (background)
    Note over T2: Uses chunk boundaries from T1
    T2->>Engine: citation edges, reference edges
    Engine->>UI: event: edges_added (references)

    AL->>T3: Route to SemanticAdapter (background)
    Note over T3: Only processes changed chunks
    T3->>Engine: concept nodes, thematic edges
    Engine->>UI: event: nodes_added (concepts)

    Note over T4: Triggered by mutation threshold
    T4->>Engine: may_be_related edges, community nodes
    Engine->>UI: event: topology_changed
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
        m1["File edit"] --> m2["Tier 0-1: structure"]
        m2 --> m3["Tier 2: relations
        background"]
        m2 --> m4["Tier 3: semantics
        background, LLM"]
        m3 --> m5(["event: edges_added"])
        m4 --> m6(["event: concepts_added"])
        m6 --> m7["File edit again"]
        m7 --> m8["Tier 0-1: delta only"]
        m8 --> m9["Tier 3: changed chunks only"]
    end

    subgraph trellis ["Trellis - Accumulative (weeks)"]
        direction TB
        t1["Fragment arrives
        Week 1"] --> t2["Tier 1-3: analyze"]
        t2 --> t3["...days pass..."]
        t3 --> t4["Fragment arrives
        Week 3"]
        t4 --> t5["Tier 1-3: analyze"]
        t5 --> t6["Tier 4: community
        detection triggered"]
        t6 --> t7(["Surface: implicit
        outline harvested"])
    end

    subgraph eddi ["EDDI - Streaming (milliseconds)"]
        direction TB
        e1["Gesture 1"] --> e2["Tier 0-1: node + labels"]
        e3["Gesture 2"] --> e4["Tier 0-1: node + labels"]
        e5["Gesture 3"] --> e6["Tier 0-1: node + labels"]
        e2 --> e7["Tier 2: clustering"]
        e4 --> e7
        e6 --> e7
        e7 --> e8["Tier 4: topology check"]
        e8 --> e9(["event: hub_emerged"])
        e8 --> e10(["event: community_formed"])
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
        +tier() AdapterTier
        +process(AdapterInput) Result~AdapterOutput~
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

    class AdapterTier {
        <<enum>>
        Instant
        Fast
        Moderate
        Slow
        Background
    }

    class AdapterTrigger {
        <<enum>>
        FileChanged
        FragmentReceived
        GestureSegmented
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
    SemanticAdapter ..> AdapterOutput : produces
    AdapterInput *-- AdapterData
    AdapterInput *-- AdapterTrigger
    AdapterOutput *-- ProvenanceEntry
    SemanticAdapter *-- AdapterTier
```

---

## 8. Concrete Adapter Implementations

How the abstract trait maps to specific adapter instances, organized by input kind and tier.

```mermaid
flowchart TB
    trait["SemanticAdapter trait
    id, name, dimensions,
    input_kind, tier, process"]

    subgraph file_content ["input_kind: file_content"]
        direction LR
        da["DirectoryAdapter
        Tier: Instant
        Dim: structure"]
        msa["MarkdownStructureAdapter
        Tier: Fast
        Dim: structure
        Produces chunk boundaries"]
        la["LinkAdapter
        Tier: Moderate
        Dim: relational"]
        llm["LLMSemanticAdapter
        Tier: Slow
        Dim: semantic
        Requires LLM"]
    end

    subgraph gesture_encoding ["input_kind: gesture_encoding"]
        ma["MovementAdapter
        Tier: Fast
        Dim: semantic
        Uses label vocabulary"]
    end

    subgraph graph_state ["input_kind: graph_state"]
        direction LR
        na["NormalizationAdapter
        Tier: Background
        Proposes may_be_related
        Requires LLM"]
        ta["TopologyAdapter
        Tier: Background
        Communities and hubs
        Algorithmic"]
        ca["CoherenceAdapter
        Tier: Background
        Cross-adapter conflicts
        Requires LLM"]
    end

    trait -.-> file_content
    trait -.-> gesture_encoding
    trait -.-> graph_state

    style file_content fill:#e1f5fe,stroke:#0288d1
    style gesture_encoding fill:#fce4ec,stroke:#c62828
    style graph_state fill:#fff3e0,stroke:#f57c00
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

## 12. Adapter Layer Scheduling

How the tier scheduler orchestrates parallel and sequential adapter execution.

```mermaid
flowchart TB
    input[/"Input arrives"/]

    input --> route{"Route by input_kind"}

    route -->|file_content| fc_adapters
    route -->|text_fragment| tf_adapters
    route -->|gesture_encoding| ge_adapters

    subgraph fc_adapters ["File Content Pipeline"]
        direction TB
        fc_t0["Tier 0: DirectoryAdapter
        sync, immediate"]
        fc_t1["Tier 1: MarkdownStructureAdapter
        parallel, fast"]
        fc_t2["Tier 2: LinkAdapter
        background"]
        fc_t3["Tier 3: LLMSemanticAdapter
        background, expensive"]

        fc_t0 --> fc_t1
        fc_t1 -->|chunk boundaries| fc_t2
        fc_t1 -->|chunk boundaries| fc_t3
    end

    subgraph tf_adapters ["Text Fragment Pipeline"]
        direction TB
        tf_t1["Tier 1: FragmentStructureAdapter
        fast"]
        tf_t3["Tier 3: LLMSemanticAdapter
        background"]

        tf_t1 --> tf_t3
    end

    subgraph ge_adapters ["Gesture Encoding Pipeline"]
        direction TB
        ge_t0["Tier 0: GestureNodeAdapter
        instant"]
        ge_t1["Tier 1: LabelMappingAdapter
        fast"]
        ge_t2["Tier 2: ClusterAdapter
        moderate"]

        ge_t0 --> ge_t1
        ge_t1 --> ge_t2
    end

    fc_adapters --> merge
    tf_adapters --> merge
    ge_adapters --> merge

    merge["Merger - dedup, combine reinforcement"]
    merge --> provgen["Provenance Generator"]
    provgen --> engine["PlexusEngine"]

    engine --> threshold{"Mutation threshold reached?"}
    threshold -->|yes| reflexive

    subgraph reflexive ["Reflexive Adapters (Tier 4)"]
        r_norm["NormalizationAdapter"]
        r_topo["TopologyAdapter"]
        r_cohere["CoherenceAdapter"]
    end

    reflexive --> merge

    threshold -->|no| done[/Wait for next input/]

    style fc_adapters fill:#e1f5fe,stroke:#0288d1
    style tf_adapters fill:#e8f5e9,stroke:#388e3c
    style ge_adapters fill:#fce4ec,stroke:#c62828
    style reflexive fill:#fff3e0,stroke:#f57c00
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
