# Semantic Adapters: Architecture & Design

## The Core Distinction

Plexus maintains a separation between two kinds of knowledge in the graph:

**Ontological (what we model as existing)** — Nodes and edges in the structure, semantic, relational, and temporal dimensions represent things in the domain: concepts, documents, gestures, functions, relationships between them. The graph says: *"laban-effort is a concept related to weight-flow."*

**Epistemological (how we came to assert it)** — The provenance dimension tracks the modeling process itself: which adapter extracted what, from where, with what confidence, and why. The provenance says: *"We believe laban-effort exists because the DocAdapter's LLM extracted it from laban-analysis.md:42 on March 3rd with 0.85 confidence, and it was reinforced when the MovementAdapter independently clustered gestures mapping to the same concept."*

These live in the same graph but serve different purposes. A node IS a thing. A mark is a Post-it note stuck inside the page of a book — anchored to specific source material, recording an observation about the modeling process.

### Grounded Example

Given a context spanning research documents across three repos:

```
Context: "movement-research"
Sources:
  - /repos/biomechanics-papers/
  - /repos/gesture-taxonomy/
  - /repos/improvisation-notes/
```

The adapters populate dimensions like this:

```
 STRUCTURE dimension (DirectoryAdapter, programmatic)
 ├── [biomechanics-papers/] ──contains──▶ [laban-analysis.md]
 ├── [gesture-taxonomy/]    ──contains──▶ [upper-body/reaches.md]
 └── [improvisation-notes/] ──contains──▶ [session-2024-03.md]
     Ontological claim: "these documents exist and are organized this way"

 SEMANTIC dimension (DocAdapter, LLM-powered)
 ├── [concept:laban-effort] ──related_to──▶ [concept:weight-flow]
 ├── [concept:reach-space]  ──extends────▶  [concept:kinesphere]
 └── [concept:improvisation]──applies────▶  [concept:laban-effort]
     Ontological claim: "these ideas exist and relate this way"

 RELATIONAL dimension (LinkAdapter, programmatic)
 ├── [laban-analysis.md]    ──cites──────▶  [reaches.md]
 └── [session-2024-03.md]   ──references─▶  [laban-analysis.md]
     Ontological claim: "the author made these connections"

 PROVENANCE dimension (automatic, tracks the above)
 └── chain: "semantic-extraction-run-2024-03"
     ├── mark (decision): "LLM identified 'laban-effort' as core
     │     concept from laban-analysis.md:42"
     ├── mark (question): "Is 'weight-flow' distinct from
     │     'effort-weight'? LLM gave 0.6 confidence"
     └── mark (decision): "Clustered 'reach' and 'kinesphere'
           via MovementAdapter gesture analysis"
     Epistemological claim: "here's HOW and WHY we asserted what's
     in the other dimensions"
```

Cross-dimensional edges connect provenance to the things it's about:

```
 PROVENANCE                              SEMANTIC
 [mark: "LLM identified     ──derived──▶ [concept:laban-effort]
  laban-effort"]              (cross-dim)

 SEMANTIC                                STRUCTURE
 [concept:laban-effort]      ──found_in─▶ [laban-analysis.md:42]
                              (cross-dim)
```

This means you can always trace backward: see a concept, ask "where did this come from?", follow cross-dim edges to provenance marks, and audit the chain of reasoning.

---

## Three Application Scenarios

The adapter layer must support three fundamentally different interaction patterns. These are not hypothetical — they correspond to real applications.

### Scenario 1: Manza (Ambient / Continuous)

**What it is:** An editor and viewer where users create contexts, view document collections, visualize the knowledge graph, and watch it evolve in real-time as they write.

**Temporal profile:** Continuous. User edits a document, saves, and the graph updates. The experience is ambient — the graph is a living companion to the writing/coding process, contributing to flow rather than interrupting it.

**Adapter interaction pattern:**
```
User edits document
       │
       ▼
File change event
       │
       ▼
Adapter(s) re-analyze affected content
       │
       ▼
Graph mutations (new nodes, new edges, reinforced edges)
       │
       ▼
Event emitted → UI updates visualization
```

**What adapters produce:** Incremental graph mutations. The user added a section about "somatic awareness" → the semantic adapter extracts a new concept node → cross-dimensional edges link it to the document section → the graph visualization animates the new connections appearing.

**Key requirement:** Incremental analysis. Re-analyzing the entire corpus on every keystroke is not viable. Adapters need to handle deltas: "this section changed, update the relevant subgraph."

### Scenario 2: Trellis (Accumulative / Periodic)

**What it is:** A system that prompts a writer via email or text ("brainstorm ideas for 2 minutes") and accumulates fragments over days, weeks, months. Uses the Plexus graph to surface latent connections between fragments: "mirror not oracle."

**Temporal profile:** Accumulative. Fragments arrive asynchronously over long timescales. The graph grows slowly but the interesting signal is in the *emergent structure* — implicit outlines, recurring themes, conceptual clusters that the writer didn't consciously organize.

**Adapter interaction pattern:**
```
Fragment arrives (text message, email response)
       │
       ▼
Adapter analyzes fragment in isolation
       │
       ▼
Graph mutations (new nodes, new edges)
       │
       ▼
Periodic: surface connections to the writer
  - "These 4 fragments from the last month cluster around X"
  - "This idea from Tuesday echoes something from 3 weeks ago"
  - "An outline is emerging: [harvested structure]"
```

**What adapters produce:** The same currency (nodes, edges, reinforcement) but the *consumption pattern* is different. Trellis doesn't show the graph directly — it queries it to find:
- Community detection → emergent topic clusters
- High-degree nodes → recurring themes (hub concepts)
- Path analysis → chains of related fragments that form implicit outlines
- Edge strength → which connections are reinforced across many fragments

**Key requirement:** The graph needs to support "harvesting" — identifying when a subgraph has accumulated enough structure to surface as a meaningful pattern. This is a query/network-science concern more than an adapter concern, but the adapters must produce graph structures that enable it (concepts need to be normalized enough to cluster).

**Key distinction from Manza:** Manza's graph serves the writer's ambient awareness. Trellis's graph serves the system's ability to reflect patterns back. The graph is the same, but the consumer relationship is inverted.

### Scenario 3: EDDI (Streaming / Session-based)

**What it is:** Emergent Dynamic Design Interface. A stream of incoming gesture data over the course of a session (or across sessions). The graph accumulates, and Plexus emits events when topology changes occur. A subscribing client uses these events to alter an environment (light, sound, projection).

**Temporal profile:** Streaming. Gesture data arrives continuously during a session. The graph must update within frame-time constraints. Between sessions, the graph persists and accumulates.

**Adapter interaction pattern:**
```
Continuous gesture stream (in EDDI)
       │
       ▼
Segmentation + encoding (in EDDI)
       │
       ▼
Encoded gesture with labels sent to Plexus
       │
       ▼
Adapter maps encoded gesture → graph mutations
  - New gesture node (or reinforcement of existing one)
  - Transition edges between sequential gestures
  - Cluster membership edges
  - Concept nodes from labels (Laban efforts, Viewpoints, etc.)
       │
       ▼
Graph topology change detected
  - New community formed
  - Hub gesture emerged
  - Edge crossed strength threshold
       │
       ▼
Event emitted → subscribed client alters environment
```

**What adapters produce:** Same currency (nodes, edges, reinforcement) but from a fundamentally different source. Not text. Not files. The adapter here is computational — it receives pre-segmented, pre-encoded gesture representations and maps them into graph vocabulary.

**Key requirements:**
- **Pre-segmented input:** EDDI handles gesture segmentation and encoding upstream. Plexus receives structured gesture representations, not raw motion data.
- **Dual representation:** Each gesture arrives with both a numerical encoding (feature vector for clustering) AND semantic labels (Laban effort qualities, Viewpoints categories, etc.). The encoding enables computational clustering; the labels provide semantic handles that bridge into the shared concept vocabulary.
- **Event emission on topology change:** The primary output isn't "here's your updated graph" — it's "the graph just changed in a significant way." New cluster formed, edge crossed threshold, community split. These are the events that drive environmental responses.
- **Session semantics:** A session is a bounded temporal window. The graph may accumulate across sessions, but within-session patterns (gesture sequences, escalating movement vocabulary) are distinct from cross-session patterns (movement style evolution, recurring motifs).

---

## Tiered Knowledge: Cost-Aware Graph Building

Different kinds of knowledge have different computational costs. The system should be honest about this — emitting knowledge as it becomes available rather than blocking on the most expensive operation.

### The Fidelity Tiers

```
Tier 0 (instant):   "This file exists, is 340KB, lives in /research/"
                     → structure dimension: file node with size property
                     Cost: ~0. Filesystem stat.

Tier 1 (fast):      "This file has 5 acts, 23 scenes"
                     → structure dimension: section nodes, contains edges
                     Cost: milliseconds–seconds. Requires parsing/chunking.
                     Note: for large files (Shakespeare play as one TXT),
                     chunking is unavoidable upfront cost.

Tier 2 (moderate):  "Act 3 Scene 1 contains the 'To be or not to be' soliloquy"
                     → relational dimension: reference edges, key-passage markers
                     Cost: seconds. Structural analysis of chunks.

Tier 3 (slow):      "This play explores themes of indecision, mortality, and duty"
                     → semantic dimension: concept nodes, thematic edges
                     Cost: seconds–minutes + API cost. LLM processing of chunks.

Tier 4 (background): "The theme of indecision in Hamlet connects to the
                      concept of 'suspension' in your movement research"
                     → cross-dimensional reinforcement, community detection
                     Cost: variable. Graph-wide analysis, potentially LLM-assisted.
```

Each tier emits events when complete. The graph is always in a partially-built state and that's fine — it's honest about what it knows at each fidelity level. Manza can show the file structure immediately, then animate new semantic connections appearing as Tier 3 completes in the background.

### Tiers Inform Each Other

Cheap tiers inform expensive tiers about where to focus:

```
Tier 1 (structural parsing)
  │
  ├── Chunks the file into processable sections
  │   (load-bearing: bad chunking ruins downstream extraction)
  │
  └── Detects which chunks changed (for incremental updates)
        │
        ├──▶ Tier 2 only re-analyzes changed chunks
        └──▶ Tier 3 only sends changed chunks to LLM
```

This means the structural adapter isn't just producing structure nodes — it's producing the chunking strategy that all downstream tiers depend on. The structural adapter's section boundaries become the units of incremental reprocessing.

### Tiers Apply to All Domains

The movement domain has its own tier structure:

```
Tier 0 (instant):   "A gesture was received, session timestamp X"
                     → structure: gesture node with timestamp

Tier 1 (fast):      "This gesture has encoding [0.8, 0.2, 0.9, ...]
                      and labels [sudden, strong, indirect]"
                     → semantic: concept nodes (sudden, strong, indirect)
                     → edges: gesture ──exhibits──▶ concept:sudden

Tier 2 (moderate):  "This gesture clusters with gestures 42, 87, 103"
                     → relational: cluster membership edges
                     → reinforcement: shared-cluster edges strengthen

Tier 3 (slow):      "A new movement community has emerged around
                      'explosive-direct' gestures in this session"
                     → community detection, topology-change events
                     Cost: depends on graph size and analysis frequency
```

### Cost Implications for the Adapter Trait

The tiered model suggests adapters should declare their tier/cost level:

```rust
pub trait SemanticAdapter: Send + Sync {
    // ... existing methods ...

    /// Processing tier — determines scheduling priority and parallelism.
    ///
    /// Lower tiers run first and may inform higher tiers.
    /// Multiple adapters at the same tier can run in parallel.
    fn tier(&self) -> AdapterTier;
}

pub enum AdapterTier {
    /// Instant: filesystem, metadata, trivial computation
    /// Always runs synchronously before other tiers.
    Instant,

    /// Fast: parsing, chunking, structural analysis
    /// Runs quickly, may produce chunking that informs later tiers.
    Fast,

    /// Moderate: cross-reference analysis, pattern matching
    /// May run in background, emits events on completion.
    Moderate,

    /// Slow: LLM calls, expensive computation
    /// Always runs in background, emits events on completion.
    Slow,

    /// Background: graph-wide analysis, community detection, cross-adapter unification
    /// Runs periodically or on significant graph changes.
    Background,
}
```

The adapter layer uses tiers to schedule work:
1. Run all `Instant` adapters synchronously → emit events
2. Run all `Fast` adapters (parallel) → emit events → feed chunking to later tiers
3. Queue `Moderate` adapters in background → emit events on completion
4. Queue `Slow` adapters in background → emit events on completion
5. Schedule `Background` adapters periodically or on topology thresholds

---

## Cross-Modal Concept Bridging

The most valuable signal in the system occurs when independent adapters, operating on different modalities, arrive at the same concept. This happens through **shared labels in the semantic dimension**.

### How It Works

The semantic dimension is a shared namespace of labeled concepts. Adapters from any domain contribute to it by producing concept nodes with meaningful labels.

```
DocAdapter reads a paper about Laban Movement Analysis:
  → extracts concept node: concept:sudden
  → extracts concept node: concept:strong
  → extracts concept node: concept:kinesphere

MovementAdapter receives a gesture from EDDI:
  encoding: [0.8, 0.2, 0.9, ...]
  labels: ["sudden", "strong", "indirect"]
  → creates gesture node with encoding in properties
  → finds or creates concept:sudden (same node!)
  → finds or creates concept:strong (same node!)
  → creates edges: gesture ──exhibits──▶ concept:sudden
```

When the MovementAdapter produces `concept:sudden` and that node already exists (created by DocAdapter from text), the edge from the gesture to the concept is reinforced by a new, independent source. The system sees:

```
concept:sudden
  ├── created by: DocAdapter (from laban-analysis.md:87)
  ├── reinforced by: MovementAdapter (from gesture encoding)
  │
  │   Reinforcement type: MultipleAnalyzers
  │   Source diversity: 2 (Analyzer("doc"), Analyzer("movement"))
  │   Confidence boost: significant (independent modalities agree)
  │
  ├── edge from: [laban-analysis.md:87] ──found_in──▶ concept:sudden
  └── edge from: [gesture-4827] ──exhibits──▶ concept:sudden
```

### Labels Are the Bridge

The labels that accompany data into the system are **load-bearing**. A gesture labeled only `cluster-7` is isolated — it connects to nothing outside the movement domain. A gesture labeled with Laban effort qualities (`sudden`, `strong`, `indirect`) or Viewpoints vocabulary (`repetition`, `duration`, `kinesthetic-response`) is automatically connected to everything else in the graph that references those concepts.

This means the labeling that happens upstream (in EDDI, in Trellis's prompt design, in Manza's content analysis) determines the richness of cross-modal connections. The graph doesn't need a universal ontology — it just needs overlapping vocabulary where domains genuinely overlap. Movement and text share Laban vocabulary. Music and movement might share temporal quality vocabulary. Code and architecture might share structural vocabulary.

### The Abstraction

The semantic dimension works the same way regardless of input modality:

| Domain | Input | Extraction method | Labels produced |
|--------|-------|------------------|-----------------|
| Text/documents | File content | LLM extraction | Concepts, themes, entities |
| Code | Source files | Tree-sitter + LLM | Functions, patterns, architectures |
| Movement | Gesture encodings | Clustering + vocabulary mapping | Effort qualities, Viewpoints |
| Images | Pixel data / embeddings | Vision model | Subjects, composition, mood |
| Audio | Waveform / features | Audio analysis + classification | Rhythm, dynamics, timbre |

Every domain can participate in the shared semantic namespace as long as its adapter produces labeled concept nodes. The labels don't come from a universal ontology — they emerge from the vocabularies natural to each domain, and they overlap where the domains genuinely intersect.

---

## What the Three Scenarios Reveal About Adapter Design

| Concern | Manza | Trellis | EDDI |
|---------|-------|---------|------|
| **Input trigger** | File change event | Async fragment arrival | Gesture segmented upstream |
| **Analysis mode** | Incremental (delta from Tier 1) | Isolated (single fragment) | Per-gesture (discrete events) |
| **Source material** | Files (text, code) | Text fragments | Encoded gestures + labels |
| **Extraction method** | LLM + programmatic | LLM + programmatic | Algorithmic + label vocabulary |
| **Primary output** | Graph viz updates | Surfaced connections | Topology-change events |
| **Timescale** | Seconds (edit cycle) | Days/weeks/months | Milliseconds (frame time) |
| **Decay relevance** | Medium (weekly half-life) | Low (months-scale memory) | None within session, configurable across |
| **Cost profile** | Tiers 0–3 on file change | Tiers 1–3 per fragment | Tiers 0–2 per gesture, Tier 3 periodic |

---

## The Adapter Trait

A semantic adapter should be broader than `ContentAnalyzer`. It needs to:

1. **Declare its input contract** — what kind of data it consumes
2. **Declare its output dimensions** — which dimensions it populates
3. **Declare its processing tier** — how expensive it is (determines scheduling)
4. **Produce the universal currency** — nodes, edges, and reinforcement evidence
5. **Carry provenance metadata** — so the system can automatically record WHY each mutation happened

The adapter does NOT need to:
- Know about the event system (that's the engine's job)
- Know about visualization (that's the lens system's job)
- Manage its own provenance (the adapter layer wraps this automatically)
- Handle persistence (the engine handles this)
- Know about other adapters (cross-adapter reinforcement happens through shared concept nodes)

### Proposed Trait Shape

```rust
/// A semantic adapter transforms domain-specific input into graph mutations.
///
/// Unlike ContentAnalyzer (which assumes file-based content analysis),
/// SemanticAdapter is domain-agnostic. It receives typed input and produces
/// graph mutations with provenance metadata.
#[async_trait]
pub trait SemanticAdapter: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Which dimensions this adapter populates
    fn dimensions(&self) -> Vec<&str>;

    /// The type of input this adapter accepts.
    ///
    /// Used by the engine to route data to the right adapter.
    /// Examples: "file_content", "text_fragment", "gesture_encoding"
    fn input_kind(&self) -> &str;

    /// Processing tier — determines scheduling priority and parallelism.
    fn tier(&self) -> AdapterTier;

    /// Process input and produce graph mutations.
    ///
    /// The AdapterInput is domain-specific (file bytes, text string,
    /// gesture encoding, etc). The AdapterOutput is universal:
    /// nodes, edges, reinforcement evidence, and provenance metadata.
    async fn process(&self, input: &AdapterInput) -> Result<AdapterOutput, AdapterError>;
}

/// Domain-specific input to an adapter
pub struct AdapterInput {
    /// The context this input belongs to
    pub context_id: ContextId,

    /// The input data (typed by the adapter's input_kind)
    pub data: AdapterData,

    /// What triggered this processing
    pub trigger: AdapterTrigger,

    /// Previous state, if available (for incremental processing)
    pub previous: Option<AdapterSnapshot>,
}

pub enum AdapterData {
    /// File content (for document/code adapters)
    FileContent {
        path: String,
        content: Vec<u8>,
        content_type: ContentType,
    },

    /// Text fragment (for accumulative systems like Trellis)
    TextFragment {
        text: String,
        source: String,        // "email", "sms", "voice-transcription"
        timestamp: DateTime<Utc>,
    },

    /// Encoded gesture with semantic labels (for movement systems like EDDI)
    GestureEncoding {
        encoding: Vec<f32>,    // Feature vector for clustering
        labels: Vec<String>,   // Semantic handles: ["sudden", "strong", "indirect"]
        gesture_type: String,  // "reach", "turn", "stillness"
        session_id: String,
        timestamp: DateTime<Utc>,
    },

    /// Generic structured data (for domains not yet enumerated)
    Structured(serde_json::Value),
}

pub enum AdapterTrigger {
    /// A file changed (Manza)
    FileChanged { path: String },
    /// A new fragment arrived (Trellis)
    FragmentReceived,
    /// A gesture was segmented from stream (EDDI)
    GestureSegmented,
    /// Manual/programmatic invocation
    Manual,
}

pub enum AdapterTier {
    /// Instant: filesystem, metadata, trivial computation
    Instant,
    /// Fast: parsing, chunking, structural analysis
    Fast,
    /// Moderate: cross-reference analysis, pattern matching
    Moderate,
    /// Slow: LLM calls, expensive computation
    Slow,
    /// Background: graph-wide analysis, community detection
    Background,
}

/// Universal output from any adapter
pub struct AdapterOutput {
    /// Nodes to add or update
    pub nodes: Vec<Node>,

    /// Edges to add or update
    pub edges: Vec<Edge>,

    /// Nodes to remove (for incremental updates)
    pub removals: Vec<NodeId>,

    /// Provenance metadata — the adapter describes WHY it made
    /// these mutations. The adapter layer converts this into
    /// provenance marks automatically.
    pub provenance: Vec<ProvenanceEntry>,
}

pub struct ProvenanceEntry {
    /// What was decided/observed
    pub description: String,

    /// What kind of observation
    pub entry_type: ProvenanceEntryType,

    /// Which output nodes/edges this entry explains
    pub explains: Vec<NodeId>,

    /// Confidence in this assertion
    pub confidence: f32,

    /// Where in the source material this came from
    pub source_location: Option<SourceLocation>,
}

pub enum ProvenanceEntryType {
    /// Adapter made a definite assertion
    Decision,
    /// Adapter flagged uncertainty
    Question,
    /// Adapter noted something needs human review
    NeedsReview,
}
```

### The Adapter Layer (Orchestration)

The adapter layer sits between raw input and the engine. It schedules adapters by tier, routes data, and handles provenance automatically:

```
  Raw Input (file change / fragment / gesture / ...)
       │
       ▼
  ┌──────────────────────────────────────────────────┐
  │              Adapter Layer                        │
  │                                                   │
  │  1. Route to adapter(s) by input_kind             │
  │  2. Schedule by tier:                             │
  │     Instant → sync, immediate                     │
  │     Fast → parallel, quick                        │
  │     Moderate → background, emit on complete        │
  │     Slow → background, emit on complete            │
  │     Background → periodic / threshold-triggered    │
  │  3. Merge outputs (dedup nodes, combine evidence)  │
  │  4. Create provenance marks from ProvenanceEntry   │
  │  5. Commit mutations to engine                     │
  │                                                   │
  │  Fast tiers feed chunking/context to slow tiers    │
  └──────────────────────────────────────────────────┘
       │                    │
       ▼                    ▼
  PlexusEngine         Provenance
  (graph mutations)    (automatic marks)
       │
       ▼
  Event System
  (topology changes → subscribers)
```

### Relationship to Existing ContentAnalyzer

`ContentAnalyzer` becomes one *kind* of `SemanticAdapter` — specifically, one that handles `AdapterData::FileContent`. The existing analyzers (MarkdownStructureAnalyzer, LinkAnalyzer, SemanticAnalyzer) can be wrapped or refactored to implement `SemanticAdapter` instead, with `input_kind() = "file_content"`.

This is a generalization, not a replacement. The analysis orchestrator and result merger still apply to the file-content case. The adapter layer is the broader concept that encompasses non-file, non-text, non-LLM input sources.

---

## Reflexive Adapters: The Graph Refining Itself

External adapters (structure, semantic, relational, movement) take input from outside and build the graph. But the graph also needs processes that operate on the graph itself — examining its own structure, proposing connections, detecting emergent patterns. These are **reflexive adapters**: their input is graph state, and their output is graph mutations.

Reflexive adapters are the same `SemanticAdapter` trait with `input_kind = "graph_state"`, running at Tier 4 (Background). They fit the same architecture — they produce nodes, edges, reinforcement, and provenance — but their trigger is graph-internal rather than external.

### The Layered Picture

```
  External Adapters (Tiers 0–3)
  ├── StructureAdapter (Tier 0/1): file tree, chunking
  ├── RelationalAdapter (Tier 2): links, references
  ├── SemanticAdapter (Tier 3): LLM concept extraction
  └── MovementAdapter (Tier 1/2): gesture encoding + labels
           │
           ▼
      Graph accumulates
           │
           ▼
  Reflexive Adapters (Tier 4 / Background)
  ├── NormalizationAdapter: propose relationships between similar concepts
  ├── TopologyAdapter: community detection, hub identification
  └── CoherenceAdapter: cross-adapter reconciliation
           │
           ▼
      Graph refines itself
           │
           ▼
      Events emitted (communities found, concepts linked, etc.)
```

### Three Reflexive Concerns

**1. Concept Normalization (NormalizationAdapter)**

The label-based bridging mechanism works when labels match exactly (`concept:sudden` from both text and movement). But in practice, you get near-misses: "sudden" vs "suddenness" vs "abrupt." The NormalizationAdapter finds these candidate pairs using fuzzy matching, potentially LLM-assisted.

**Critical design principle: propose, don't merge.** Two similarly-labeled concepts could be entirely different things depending on context. "Sudden" in Laban effort theory and "sudden" in software engineering ("sudden failure mode") are the same label but different concepts — their graph neighborhoods are completely different. Destructively merging nodes based on label similarity would lose real distinctions.

Instead, the NormalizationAdapter:

1. Finds candidate pairs (same or similar labels, LLM-assisted fuzzy matching)
2. Creates `may_be_related` edges — **weak, low confidence** — between candidates
3. Lets the graph's own reinforcement dynamics determine if the connection is real:
   - If the nodes' neighborhoods overlap, users traverse between them, or they appear in the same communities → the edge gets reinforced naturally, confidence rises
   - If the nodes live in completely different neighborhoods and nobody traverses the edge → it decays and becomes negligible

**The reflexive adapter proposes; the graph disposes.** This is consistent with the Hebbian model throughout the system — connections that get reinforced survive, connections that don't fade. The adapter accelerates discovery of *potential* connections but doesn't make unilateral decisions about what's equivalent.

The provenance trail records what happened: *"NormalizationAdapter proposed may_be_related between concept:sudden (Laban context) and concept:abrupt (music context) — LLM assessed 0.72 semantic similarity based on label proximity. Edge created with initial confidence 0.15."* If the edge later strengthens to 0.8 through independent reinforcement, or decays to nothing, the provenance explains both outcomes.

**2. Topology Analysis (TopologyAdapter)**

Community detection, hub identification, and structural pattern recognition. This is the network science layer — algorithmic, not LLM-powered.

- **Community detection** (Louvain algorithm): discovers emergent groupings in the graph. Critical for Trellis's "harvesting" use case — surfacing when a cluster of fragments has accumulated enough structure to represent an implicit outline.
- **Hub identification**: finds high-degree nodes that serve as conceptual anchor points. In Manza, these are the core concepts in a research collection. In EDDI, these are the dominant movement qualities in a session.
- **Topology-change events**: the primary output for EDDI. "A new community formed," "a hub emerged," "two communities merged." These events drive environmental responses.

Trigger: periodic or threshold-based ("50 new edges since last run").

**3. Cross-Adapter Coherence (CoherenceAdapter)**

More subtle than normalization. This handles cases where different adapters contribute to overlapping concept space with potentially inconsistent semantics. For example, DocAdapter and MovementAdapter both reference "effort" but with subtly different scopes.

The CoherenceAdapter examines nodes that have edges from multiple adapter sources and assesses whether the adapter contributions are consistent. Where they diverge, it can flag the divergence (provenance mark of type `Question`) rather than trying to resolve it — surfacing the tension for human review or for the graph's own dynamics to settle.

Trigger: when a new adapter source contributes to a concept that already has contributions from other adapters.

### Reflexive Adapters in the Trait System

Reflexive adapters implement the same `SemanticAdapter` trait:

```rust
struct NormalizationAdapter {
    similarity_threshold: f32,  // minimum similarity to propose a link
}

#[async_trait]
impl SemanticAdapter for NormalizationAdapter {
    fn id(&self) -> &str { "normalization" }
    fn name(&self) -> &str { "Concept Normalization" }
    fn dimensions(&self) -> Vec<&str> { vec!["semantic"] }
    fn input_kind(&self) -> &str { "graph_state" }
    fn tier(&self) -> AdapterTier { AdapterTier::Background }

    async fn process(&self, input: &AdapterInput) -> Result<AdapterOutput, AdapterError> {
        // input.data is AdapterData::GraphState { context_id, ... }
        // 1. Find concept nodes with similar labels
        // 2. For each candidate pair, assess similarity (LLM or embedding distance)
        // 3. Create may_be_related edges with low initial confidence
        // 4. Record provenance for each proposed link
        todo!()
    }
}
```

This means `AdapterData` needs a variant for graph-state input:

```rust
pub enum AdapterData {
    // ... existing variants ...

    /// Graph state snapshot (for reflexive adapters)
    /// The adapter receives a view of the current graph and produces
    /// mutations based on structural analysis.
    GraphState {
        context_id: ContextId,
        /// Which nodes/edges to examine (can be filtered by the adapter layer
        /// to only include what's changed since the adapter's last run)
        scope: ReflexiveScope,
    },
}

pub enum ReflexiveScope {
    /// Examine everything in the context
    Full,
    /// Examine only nodes/edges added since the given timestamp
    Since(DateTime<Utc>),
    /// Examine a specific set of nodes (e.g., "all concept nodes
    /// that have been touched by multiple adapters")
    Nodes(Vec<NodeId>),
}
```

---

## Design Decisions

### 1. Gesture segmentation is EDDI's responsibility
Plexus receives pre-segmented, pre-encoded gesture representations. The `process()` method handles discrete events, same as any other adapter. EDDI owns signal processing; Plexus owns graph building and topology analysis.

### 2. The semantic dimension is shared across all domains
"Semantic" means "meaning in this domain" and the `ContentType` on the node disambiguates origin. Text concepts and gesture qualities live in the same dimension. This is what enables cross-modal bridging — if they were in separate dimensions, shared concepts would be separate nodes instead of reinforcing each other.

### 3. Labels bridge modalities
The labels that accompany input data are load-bearing for cross-adapter connections. A gesture with Laban effort labels connects to text about Laban theory through the same concept nodes. No special unification logic required — shared vocabulary in the semantic dimension handles it naturally.

### 4. Decay is configured per-context
The same adapter may serve different applications (DocAdapter in both Manza and Trellis), but decay behavior differs by use case. Context-level decay configuration lets applications set appropriate timescales without coupling decay to adapter logic.

### 5. Cheap tiers inform expensive tiers
Structural parsing (Tier 1) produces the chunking that semantic extraction (Tier 3) depends on. For incremental updates, Tier 1 identifies which chunks changed, so Tier 3 only re-processes the delta. This keeps LLM costs proportional to what actually changed, not the size of the entire corpus.

### 6. Reflexive adapters propose, they don't merge
Concept normalization and cross-adapter coherence create weak `may_be_related` edges rather than destructively merging nodes. Two similarly-labeled concepts may be entirely different depending on context — their graph neighborhoods are the real signal about equivalence. The graph's own reinforcement dynamics (Hebbian: connections that get used survive, those that don't fade) determine which proposed relationships are real. This preserves information and avoids false unification.

---

## Open Questions

1. **Incremental state management:** The `AdapterSnapshot` concept needs design work. For text, this is likely "which chunks have I processed and what did I produce." For movement, it might be "current cluster centroids." Should this state live in the adapter, in the context, or in a separate state store?

2. **Chunking as a first-class concept:** Since Tier 1 chunking is load-bearing for all downstream tiers, should chunks be represented in the graph (as structure-dimension nodes) or managed as adapter-internal state? If they're in the graph, other adapters can reference them. If they're internal, the coupling is looser.

3. **Background tier scheduling:** What triggers Tier 4 (Background) processing? Options: periodic timer, threshold on graph mutations since last run ("50 new nodes added, time for community detection"), or event-driven ("a new adapter source contributed to a concept that already had 3 sources"). Different reflexive adapters likely need different triggers.

4. **Canonical pointers vs. pure emergence:** When the NormalizationAdapter's `may_be_related` edges do get reinforced and two concepts are clearly the same, should the system eventually designate one as canonical (with the other having a `same_as` pointer), or should they remain as two nodes with a strong equivalence edge permanently? Canonical pointers simplify queries but add a concept of "primary" that may be arbitrary. Strong equivalence edges are more honest but create permanent duplication in the graph.
