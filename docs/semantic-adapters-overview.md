# Semantic Adapters: How It Works

A high-level walkthrough of how Plexus builds knowledge graphs from different kinds of input.

---

## The Big Picture

Plexus takes input from different domains — text documents, writing fragments, gesture data — and builds a shared knowledge graph. Different "adapters" know how to read different kinds of input. They all produce the same thing: nodes and edges in the graph.

```mermaid
flowchart LR
    A["Text & Code"] --> P["Plexus"]
    B["Writing Fragments"] --> P
    C["Gesture Data"] --> P
    P --> G(("Knowledge
    Graph"))
    G --> E1["Visualization"]
    G --> E2["Pattern Surfacing"]
    G --> E3["Environment Events"]
```

The graph is the common currency. What goes in varies wildly. What comes out depends on the application. The graph sits in the middle, accumulating structure.

---

## Two Kinds of Knowledge

The graph tracks two distinct things:

```mermaid
flowchart TB
    subgraph what ["What We Know"]
        direction LR
        N1(["concept: kinesphere"])
        N2(["concept: laban-effort"])
        N1 -->|related_to| N2
    end

    subgraph how ["How We Know It"]
        direction LR
        M1["LLM extracted kinesphere
        from chapter 3, confidence 0.85"]
    end

    M1 -.->|explains| N1

    style what fill:#f3e5f5,stroke:#7b1fa2
    style how fill:#fff3e0,stroke:#f57c00
```

**What we know** — concepts, documents, gestures, relationships between them. This is the substance of the graph.

**How we know it** — which adapter extracted what, from where, with what confidence. This is provenance. It lets you trace any assertion back to its source.

---

## Adapters: One Interface, Many Domains

Every adapter does the same thing: take input, emit graph mutations through a sink. They differ in what they consume and how complex they are internally — but the interface is the same.

```mermaid
flowchart TB
    subgraph trait ["Every Adapter"]
        direction LR
        IN["Input"] --> PROCESS["process()"] --> SINK["sink.emit()
        Nodes + Edges + Provenance"]
    end

    DA["Document Adapter
    reads files, uses LLM
    multiple internal phases"] -.-> trait
    MA["Movement Adapter
    reads gestures, uses labels"] -.-> trait
    NA["Normalization Adapter
    reads the graph itself
    runs on a schedule"] -.-> trait

    style trait fill:#e3f2fd,stroke:#1565c0
```

This uniformity is the point. The graph engine doesn't care whether a concept came from a research paper or a dancer's movement — it's a concept node either way. An adapter can be as simple or as complex as it needs to be internally; the framework only sees what comes out of the sink.

---

## Cheap First, Expensive Later

Not all knowledge costs the same to extract. A single adapter works through progressively more expensive phases internally, emitting what it knows as soon as it knows it.

```mermaid
flowchart LR
    subgraph phases ["Inside One Adapter"]
        direction LR
        A0["emit: file exists, 340KB"] --> A1["emit: 5 sections found"] --> A3["emit: themes - mortality,
        duty, indecision"]
    end

    style phases fill:#e3f2fd,stroke:#1565c0
```

The first emission is free (filesystem metadata). The second is cheap (parsing). The third is expensive (LLM calls). Each emission triggers events immediately, so the UI can show structure while semantics are still being extracted in the background.

Crucially, cheap phases inform expensive phases. Structural parsing identifies which sections changed, so the LLM phase only processes the delta — not the whole file.

---

## The Bridge: Shared Vocabulary

The most interesting thing happens when independent adapters arrive at the same concept from different directions.

```mermaid
flowchart TB
    Paper["Paper about Laban theory
    mentions 'sudden' quality"] --> C(["concept: sudden"])
    Gesture["Dancer's gesture
    labeled 'sudden'"] --> C

    C -->|"two independent sources agree"| Strong["Reinforced connection
    high confidence"]

    style C fill:#f3e5f5,stroke:#7b1fa2
    style Strong fill:#e8f5e9,stroke:#2e7d32
```

A text adapter reads a paper and extracts the concept "sudden." A movement adapter receives a gesture labeled "sudden." They both point to the same concept node. The system sees independent agreement across modalities — strong evidence that the concept is real and meaningful.

The labels that accompany data into the system are what make this work. A gesture labeled with Laban vocabulary connects to everything else that references Laban vocabulary. A gesture labeled only "cluster-7" connects to nothing.

---

## Edges: Use It or Lose It

Connections in the graph are not permanent assertions. They follow Hebbian dynamics — connections that get reinforced survive, connections that don't fade away.

```mermaid
flowchart LR
    New["New edge
    created"] --> Used{"Reinforced?"}
    Used -->|"yes: traversed,
    confirmed, multi-source"| Stronger["Grows stronger"]
    Used -->|"no activity"| Weaker["Decays over time"]
    Weaker --> Gone["Fades to negligible"]
    Stronger -->|"more evidence"| Stronger

    style New fill:#e3f2fd,stroke:#1565c0
    style Stronger fill:#e8f5e9,stroke:#2e7d32
    style Weaker fill:#fff3e0,stroke:#f57c00
    style Gone fill:#efebe9,stroke:#795548
```

Confidence comes from evidence diversity, not volume. An edge confirmed by four different kinds of evidence is more trustworthy than one confirmed a hundred times by the same kind.

---

## The Graph Refining Itself

External adapters build the graph from outside input. Reflexive adapters examine the graph itself and propose refinements.

```mermaid
flowchart TB
    External["External adapters
    build the graph"] --> Graph(("Graph"))
    Graph --> Reflexive["Reflexive adapters
    examine the graph"]
    Reflexive -->|"propose weak edges"| Graph

    style Graph fill:#f3e5f5,stroke:#7b1fa2
```

The key rule: **reflexive adapters propose, they never merge.** When the normalization adapter notices "sudden" and "abrupt" might mean the same thing, it creates a weak `may_be_related` edge. If the graph's own dynamics confirm the connection (the nodes share communities, users traverse between them), the edge strengthens naturally. If not, it fades. No information is destroyed.

---

## Three Applications, One Graph

The same graph engine serves three fundamentally different interaction patterns:

```mermaid
flowchart TB
    subgraph manza ["Manza"]
        direction TB
        M1["User edits a document"]
        M2["Graph updates in real time"]
        M3["Visualization animates changes"]
        M1 --> M2 --> M3
    end

    subgraph trellis ["Trellis"]
        direction TB
        T1["Fragments arrive over weeks"]
        T2["Graph accumulates quietly"]
        T3["Patterns surfaced to writer"]
        T1 --> T2 --> T3
    end

    subgraph eddi ["EDDI"]
        direction TB
        E1["Gestures stream in live"]
        E2["Graph detects topology shifts"]
        E3["Environment responds"]
        E1 --> E2 --> E3
    end

    style manza fill:#e1f5fe,stroke:#0288d1
    style trellis fill:#e8f5e9,stroke:#388e3c
    style eddi fill:#fce4ec,stroke:#c62828
```

**Manza** — an editor. The graph is a living companion to writing. Continuous updates, seconds-scale feedback.

**Trellis** — a writing accumulator. Fragments arrive over days and weeks. The graph finds latent connections the writer didn't consciously make. Mirror, not oracle.

**EDDI** — a gesture-driven environment controller. Movement data streams in, the graph detects when something structurally interesting happens (new cluster, hub emerged), and emits events that alter light, sound, or projection.

Different timescales. Different inputs. Different outputs. Same graph underneath.
