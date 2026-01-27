# Plexus: A Content-Agnostic Self-Reinforcing Knowledge Graph for Live Creative Composition

**Nathaniel Green**
Independent Researcher
nate@nate.green
ORCID: 0000-0003-0157-7744

*Working Paper — January 2026*

---

## Abstract

Knowledge accumulates faster than understanding across all creative domains — code grows through AI-assisted generation, research notes proliferate across tools, choreographic vocabulary develops through rehearsal — yet practitioners lose structural awareness of their own work. We present **Plexus**, a content-agnostic knowledge graph engine designed to evolve alongside creative composition, providing ambient structural awareness without interrupting flow. Plexus operates across five creative domains (code, fiction, research, movement/performance, free-writing) using domain-specific analyzers that feed a unified graph engine with self-reinforcing edge dynamics inspired by Hebbian learning. The system updates at multiple frequencies — structural changes appear in <100ms, relational clustering in <2s, semantic extraction in 10–30s, and conceptual analysis on longer timescales — creating a peripheral structural reflection of the emerging work. We ground the design in external cognition theory, flow-state research, memory-inspired learning models, and computational movement analysis. We describe integration points with three companion systems: llm-orc (LLM orchestration), Trellis (creative writing scaffolding), and EDDI (interactive performance). A companion paper [Paper 1] provides experimental validation of the semantic extraction layer. This paper presents the system vision, theoretical grounding, architecture, and evaluation agenda.

**Keywords:** knowledge graphs, creative composition, self-reinforcing networks, Hebbian learning, multi-frequency updates, external cognition, content-agnostic systems, flow state

---

## 1. Introduction

### 1.1 The Opacity Problem

Knowledge accumulates faster than understanding. A developer "vibe-coding" with an AI assistant produces working software but may not fully grasp the architectural decisions embedded in the generated code. A researcher's personal knowledge base grows to thousands of notes whose interconnections are invisible. A team's documentation sprawls across wikis, repos, and chat histories with no unified semantic map. In each case, knowledge exists but cognitive context — the awareness of what you know, how it connects, and where the gaps are — erodes.

This is not a storage problem. The documents exist. The code compiles. The notes are searchable by keyword. The problem is structural: there is no live representation of the semantic relationships within and across these artifacts. The knowledge is there but opaque to the person who ostensibly possesses it.

The problem is particularly acute in AI-assisted composition. When a developer prompts an LLM to generate a module, the resulting code has dependencies, introduces patterns, and makes architectural choices — but the developer's attention was on the prompt, not the output's structural implications. After several such exchanges, the codebase has grown in ways the developer didn't consciously design. The same dynamic applies to writing, research, and any creative process mediated by generative AI: the artifact grows, but the creator's structural awareness does not keep pace.

What's missing is not a post-hoc documentation tool. What's missing is a live structural reflection of the composition as it unfolds — something that evolves alongside the creative process and provides ambient awareness without demanding attention.

### 1.2 Plexus: A Live Knowledge Graph for Composition

Plexus is a knowledge graph engine designed to address this opacity. Rather than analyzing artifacts after they're complete, Plexus integrates with the creative environment and builds a semantic graph that evolves in real-time as content is composed. The graph is not documentation — it is a live structural reflection of the emerging work.

The core insight is that all composition — regardless of medium — produces structure, and that structure is what creators lose track of. The specific structural elements differ by domain, but the experience of watching your work's structure emerge in real-time is the same:

| Domain | Nodes | Edges | What You See Evolving |
|--------|-------|-------|----------------------|
| **Code** | Functions, modules, types, constants | Imports, calls, definitions, data flow | Dependency graph restructuring as you refactor |
| **Fiction** | Characters, scenes, locations, objects | Appearances, dialogue, plot threads, narrative arcs | Character relationship web thickening as the story develops |
| **Research** | Concepts, papers, claims, evidence | Citations, arguments, supports/contradicts, builds-on | Argument structure crystallizing as you synthesize sources |
| **Movement/Performance** | Poses, gestures, qualities, formations | Transitions, variations, oppositions, triggers | Choreographic vocabulary emerging, performer-environment coupling visible |
| **Free-writing** | Emerging ideas, fragments, questions | Associations, echoes, tensions, elaborations | Thought-trains becoming visible, clusters forming from scattered notes |

In each case, the creator composes linearly (word after word, function after function) but the structure of the work is non-linear — a graph, not a sequence. Without a live structural view, the creator must hold that graph in their head. With Plexus, the graph is externalized and kept current automatically.

Consider the experience across three domains:

*A developer* writes a new function. Edges appear connecting it to the functions it calls and the modules it imports. They prompt an AI assistant to generate a utility module — the graph immediately shows what the generated code introduced, what it depends on, and how it changed the dependency topology. When they refactor, they watch clusters merge and hub nodes shift.

*A novelist* writes a new scene. A character node gains edges to the location, the other characters present, and the plot threads advanced. A thematic concept ("betrayal") strengthens its connections to the scenes where it appears. When the writer introduces a subplot, they see it as a new cluster forming at the periphery of the main narrative structure, gradually developing edges inward.

*A performer* in an interactive installation moves through a sequence. Pose nodes connect via transition edges; movement qualities (sustained, sudden, bound) cluster into a visible vocabulary. The graph shows performer-environment coupling: this gesture reliably triggers that lighting state, this spatial formation activates that soundscape. Across rehearsals, the conceptual layer reveals what's emerging — which movement phrases are developing, which performer-to-performer dynamics recur, which environment responses are strengthening through repetition. The choreographic structure becomes visible not through notation but through the graph's memory of what happened and how it connected.

The graph behaves identically in all cases — nodes appear, edges form, clusters emerge, hubs solidify, unused connections fade. The content-type-specific analyzers (tree-sitter for code, narrative parsers for fiction, citation extractors for research, pose trackers for movement) feed different node and edge types into the same graph engine with the same self-reinforcing dynamics. Coding and choreography, fiction and research, viewed through the graph, feel like the same activity: *composition with live structural feedback*.

This produces something closer to a flow state than traditional tooling offers. The creator maintains structural awareness without interrupting composition to manually trace dependencies, re-read earlier chapters, or search for related notes. The graph is peripheral vision for knowledge work.

### 1.3 Design Principles

Plexus's core design principles:

- **Real-time evolution**: The graph updates as files are saved, code is generated, and notes are written — not as a batch process after the fact.
- **Multi-frequency updates**: Different semantic layers update at different cadences. Code structure (imports, definitions, call relationships) updates on every validation cycle. Semantic structure (shared terms, topic clusters) updates on save or pause. Conceptual structure (deeper cross-document relationships) updates in the background or on explicit refresh. This tiered approach keeps the graph responsive without saturating compute.
- **Self-reinforcing edges**: Relationships strengthen through use and decay without reinforcement, implementing a form of Hebbian learning for knowledge structures. An edge traversed during navigation becomes more visible. An edge never accessed fades. Over time, the graph converges on the relationships that actually matter to the practitioner.
- **Provenance throughout**: Every concept in the graph traces back to a specific file, line, and evidence span. Click a node, open the source. The graph is not an abstraction layer on top of the work — it is a navigable index into it.
- **Multi-system integration**: Plexus connects to LLM orchestration (llm-orc), provenance tracking (clawmarks), and UI layers (Manza) via MCP, creating a bidirectional learning loop where execution patterns inform graph structure and graph analysis informs future orchestration.
- **Content-agnostic engine**: The same graph engine, edge dynamics, and update architecture serve all creative domains. Only the analyzers differ.

### 1.4 This Paper

This paper presents the system design, theoretical grounding, and evaluation agenda for Plexus. A companion paper [Paper 1] reports the empirical experiments that validated the semantic extraction layer — one critical subsystem within the broader architecture. Here we address the full system: the content-agnostic graph engine, the self-reinforcing edge model, the multi-frequency update architecture, and integration with three companion systems (llm-orc, Trellis, EDDI).

---

## 2. Related Work

Plexus integrates ideas from several research areas not previously combined: external cognition, flow theory, cognitive context in AI-assisted work, self-reinforcing memory models, computational movement analysis, multi-frequency event processing, and knowledge graph construction.

### 2.1 External Cognition and Epistemic Tools

The idea that external representations reduce cognitive burden has deep theoretical grounding. Kirsh and Maglio [15] distinguish epistemic actions (which change the agent's computational state, making mental computation easier) from pragmatic actions (which change the world toward a goal). In their Tetris experiments, players rotate pieces physically to simplify mental pattern-matching — an action that looks "wasteful" but is computationally efficient. A knowledge graph that externalizes structural relationships serves a similar function: it makes the relationships visible so the composer doesn't have to hold them in working memory.

Hutchins [16] extends this to distributed cognition: cognitive processes are not confined to individual minds but distributed across people, artifacts, and environments. Crucially, Hutchins argues that tools do not merely "amplify" cognition — they enable qualitatively different cognitive processes using different skills. A developer with a live knowledge graph is not simply thinking harder about structure; they are engaging in a different kind of structural reasoning that relies on perceptual processing rather than memory retrieval.

Scaife and Rogers [17] formalize three mechanisms by which external graphical representations support cognition: *computational offloading* (reducing working memory demands), *re-representation* (presenting information in a form better suited to the task), and *graphical constraining* (limiting the space of possible inferences). Plexus's graph visualization performs all three: it offloads structural tracking, re-represents linear composition as a network topology, and constrains attention to the semantically relevant neighborhood of the current work.

Clark and Chalmers [18] provide philosophical grounding through the extended mind thesis: cognitive processes literally extend into the environment when external resources play the functional role that internal memory would otherwise play. By this account, Plexus is not a tool the composer uses — it is part of the composer's cognitive system.

### 2.2 Flow State and Structural Feedback

Csikszentmihalyi [19] identifies three conditions for flow: clear goals, immediate feedback, and challenge-skill balance. The second condition is directly relevant. Traditional development environments provide delayed structural feedback — the developer must actively query for dependencies, references, or call hierarchies. A live knowledge graph provides immediate, continuous structural feedback without requiring an explicit query.

Dietrich [20] adds a neurological constraint: flow involves transient hypofrontality — the prefrontal cortex partially deactivates, reducing self-monitoring and analytical processing. This implies that structural feedback must be *ambient and peripheral* rather than demanding focused attention. A knowledge graph visualization that requires active reading would disrupt flow; one that operates at the level of peripheral awareness — shapes shifting, clusters forming, edges thickening — preserves it. Matthews et al. [21] study this design space for glanceable peripheral displays, finding that ambient information can maintain awareness without attentional capture.

Digital audio workstations, 3D modeling tools, and game engines already provide this kind of live structural feedback. Waveforms evolve as musicians compose; wireframes respond as modelers sculpt; physics simulations run alongside level design. In each case, the structural representation co-evolves with the creative act. Software development has moved toward this with live linting and type checking, but these provide *correctness* feedback ("is this valid?"), not *structural* feedback ("what did this change connect to?"). A live knowledge graph occupies a different niche: it shows the semantic topology of the work as it emerges.

### 2.3 Cognitive Context Loss in AI-Assisted Work

The opacity problem (§1.1) is increasingly documented. Cito and Bork [10] describe "material disengagement" in AI-assisted coding, where developers orchestrate code generation without comprehending the output, and propose post-hoc model recovery. Qiao et al. [11] measure a comprehension-performance gap in AI-assisted brownfield development. Al Haque et al. [12] identify the measurement gap — few empirical studies of cognitive load from AI coding assistants exist. A 2025 survey [13] found 65% of developers cite missing context as their top concern with AI-generated code. Sweller's updated cognitive load theory [14] provides the mechanism: AI-generated code imposes extraneous cognitive load because its information structure is not aligned with the developer's existing schema. A live knowledge graph externalizes the structural relationships, reducing the load.

### 2.4 Self-Reinforcing and Memory-Inspired Knowledge Structures

Plexus's self-reinforcing edge model — where edges strengthen through use and decay without reinforcement — draws on Hebbian learning ("neurons that fire together wire together") and memory research. The theoretical basis comes from Bjork and Bjork [28], who distinguish storage strength (permanent) from retrieval strength (decays). Periodic forgetting builds higher storage strength on re-learning — a "desirable difficulty." This maps directly to three neuroplasticity-inspired operations on the knowledge graph:

| Operation | Neuroscience Analogue | Graph Behaviour |
|-----------|----------------------|----------------|
| Edge strengthening | Long-term potentiation (LTP) | Traversed or validated edges increase in weight |
| Temporal decay | Long-term depression (LTD) | Unaccessed edges lose retrieval strength over time |
| Emergent connections | Co-activation | Concepts that appear together across documents form new edges |

In our system, edge decay serves an analogous function to desirable difficulty: concepts that are re-encountered after fading receive stronger reinforcement than concepts that were never forgotten, naturally surfacing the relationships that recur across the practitioner's work.

Practical implementations of memory-inspired learning include spaced repetition systems. Settles and Meeder [29] develop half-life regression for predicting memory decay in language learning (deployed in Duolingo). Zaidi et al. [30] extend this with adaptive forgetting curves incorporating linguistic complexity. Our temporal decay function (exponential with weekly half-life) is deliberately simpler, but could be refined with similar complexity-aware models.

To our knowledge, no existing knowledge graph system applies Hebbian dynamics to a creative composition environment. Spaced repetition systems optimise *recall* of known facts; Plexus optimises *discovery* of emergent structure.

### 2.5 Computational Movement Analysis and Choreographic Structure

The movement/performance domain connects to a body of work on computational Laban Movement Analysis and interactive performance systems.

Fdili Alaoui et al. [31] integrate LMA experts into sensor selection and feature computation, showing that multimodal data (positional, dynamic, physiological) best characterizes Laban Effort qualities — Weight, Time, Space, and Flow. Garcia et al. [32] train HMM models for six Effort qualities, finding equi-affine features highly discriminant. These systems provide the "structural layer" input for a movement knowledge graph: they classify the low-level movement data into Laban-theoretic categories that become graph nodes.

At the knowledge representation level, Raheb et al. [33] develop a dance ontology in OWL-2 based on Labanotation semantics, with Description Logic reasoning to extract new movement knowledge. El Raheb et al. [34] survey ontology-based dance knowledge management comprehensively. These ontologies provide a schema for the *conceptual* layer of a movement knowledge graph, but they are static representations — authored by experts, not emergent from live performance data.

For real-time performance systems, Camurri et al. [35] describe EyesWeb, a platform for real-time analysis of expressive gesture in dance and music performance. Forsythe's choreographic objects [36] provide the conceptual foundation: choreographic structure as a formal system that can be computationally represented, manipulated, and visualized.

No existing system combines these capabilities into a self-reinforcing graph that evolves through performance. The movement analysis systems classify gestures; the ontologies represent choreographic knowledge; the interactive systems respond in real-time. Plexus proposes unifying these into a single graph where performer-environment couplings strengthen through rehearsal, movement vocabulary clusters emerge from practice, and choreographic structure becomes visible not through notation but through the graph's accumulated memory of what happened and how it connected.

### 2.6 Multi-Frequency Event Processing

Our tiered update architecture (§3.3) has precedent in stream processing. The Lambda Architecture [37] processes data through parallel batch (high-latency, high-accuracy) and speed (low-latency, approximate) layers. Kreps [38] simplifies this to the Kappa Architecture where all processing is stream-based with replay for recomputation.

Luckham [39] formalizes hierarchical event abstraction in Complex Event Processing: low-level events compose into higher-level complex events across different temporal windows. This is directly analogous to our multi-frequency model where token-level structural events compose into relational patterns, semantic concepts, and conceptual structures at increasing timescales.

Baresi and Guinea [40] propose multi-layer monitoring with three processor types operating at different frequencies, the closest architectural precedent to our approach. Keskisärkkä [41] address the specific challenge of applying semantic reasoning to streaming data — traditionally semantic approaches assume static data, while our semantic layer must operate incrementally on a continuously evolving corpus.

### 2.7 Knowledge Graph Systems

Recent systems for LLM-based knowledge graph construction share a batch-processing assumption. Microsoft GraphRAG [1] uses entity extraction with community detection and hierarchical summaries. LightRAG [2] combines graph and embedding retrieval with incremental updates. Neo4j LLM Graph Builder [3] uses multi-LLM extraction. iText2KG [22] provides zero-shot incremental extraction. Pan et al. [23] survey the LLM-KG construction landscape comprehensively. Agrawal et al. [24] find that knowledge graphs as external grounding demonstrably reduce LLM hallucination. InfraNodus [4] applies network science to knowledge management corpora.

Among incremental systems, Graphiti [25] builds knowledge graphs incrementally in real-time with a bi-temporal data model. Arenas-Guerrero et al. [26] demonstrate incremental KG construction using declarative RML mappings. Zhu et al. [27] address continual KG embedding with incremental distillation.

Our companion paper [Paper 1] reports experiments showing that file tree traversal outperforms network algorithms for document selection, evidence-grounded prompts achieve 0% hallucination on technical corpora, and local LLM inference has a ~10s floor that directly informed the multi-frequency architecture.

### 2.8 Gap Analysis

No existing system integrates all of these elements:

| Capability | GraphRAG | Graphiti | Spaced Repetition | InfraNodus | **Plexus** |
|------------|----------|----------|--------------------|------------|-----------|
| LLM-based extraction | ✓ | ✓ | — | — | ✓ |
| Incremental/real-time | — | ✓ | — | — | ✓ |
| Self-reinforcing edges | — | — | ✓ (recall-only) | — | ✓ |
| Evidence provenance | — | — | — | — | ✓ |
| Multi-frequency updates | — | — | — | — | ✓ |
| Creative composition UX | — | — | — | — | ✓ |
| Content-agnostic (code, text, movement) | — | — | — | — | ✓ |
| Flow-preserving ambient display | — | — | — | — | ✓ |

Graphiti shares the real-time incremental approach but lacks self-reinforcement, provenance, and visualization. Spaced repetition systems implement Hebbian-style dynamics but optimise recall of known facts rather than discovery of emergent structure. No existing system combines live structural feedback with a self-reinforcing knowledge graph in a creative composition environment.

---

## 3. System Design

### 3.1 Architecture Overview

Plexus is implemented as a Rust-based knowledge graph engine with SQLite storage. It connects to external systems via the Model Context Protocol (MCP):

```
                    ┌──────────────┐
                    │   Manza UI   │
                    │  (editor)    │
                    └──────┬───────┘
                           │ MCP
              ┌────────────┼────────────┐
              │            │            │
       ┌──────┴──────┐ ┌──┴───┐ ┌──────┴──────┐
       │   llm-orc   │ │Plexus│ │  clawmarks  │
       │(orchestrate)│ │(graph)│ │(provenance) │
       └─────────────┘ └──────┘ └─────────────┘
```

The separation of concerns is deliberate:
- **Plexus** stores and queries the knowledge graph (nodes, edges, contexts, dimensions)
- **llm-orc** orchestrates LLM ensembles for semantic extraction — stateless, strategy changes independently
- **clawmarks** records provenance (file, line, evidence span) — enables "go to source" UX
- **Manza** provides the editor environment with ambient graph visualization

### 3.2 Data Model

The graph consists of:

- **Nodes**: Typed entities (concept, function, character, pose, fragment) with properties and a content dimension (structural, relational, semantic, conceptual)
- **Edges**: Typed relationships with weight, decay parameters, and reinforcement history
- **Contexts**: Scoped subgraphs (a project, a chapter, a rehearsal session) that partition the graph without duplicating nodes
- **Dimensions**: The four semantic layers (structural, relational, semantic, conceptual) that organize nodes by abstraction level

Edge weights evolve over time through three mechanisms:
1. **Reinforcement**: Traversal, validation, or co-activation increases weight
2. **Decay**: Exponential decay with configurable half-life (default: 1 week)
3. **Emergence**: Co-occurring concepts across documents create new edges

### 3.3 Multi-Frequency Update Model

A live knowledge graph cannot update everything at once — LLM extraction takes ~10s per document [Paper 1, §4.6], and the user is composing continuously. The solution is tiered update frequencies:

| Layer | Trigger | Target Latency | Method |
|-------|---------|----------------|--------|
| **Structural** | Every validation cycle / keystroke debounce | <100ms | Deterministic parsing (tree-sitter, regex, format-specific), no LLM |
| **Relational** | On save or typing pause (>2s idle) | <2s | Lightweight text analysis, cached embeddings |
| **Semantic** | Background, priority-queued | 10–30s | LLM extraction (validated in [Paper 1]) |
| **Conceptual** | On explicit refresh or scheduled | Minutes | Network analysis, community detection |

Each layer manifests differently depending on the creative domain:

| Layer | Code | Fiction | Research | Movement/Performance |
|-------|------|---------|----------|---------------------|
| **Structural** | Imports, calls, definitions, type relationships | Character appearances, scene boundaries, dialogue attribution | Citations, section structure, reference links | Poses, transitions, spatial formations, performer positions |
| **Relational** | Shared terms, module co-usage, naming patterns | Character co-occurrence, setting reuse, motif repetition | Term overlap, shared citations, methodological similarity | Movement quality similarity (Laban efforts), gesture vocabulary clustering, spatial proximity |
| **Semantic** | Concepts, architectural patterns, design intent | Themes, narrative arcs, character development trajectories | Arguments, claims, evidential support/contradiction | Choreographic phrases, performer-environment coupling, trigger-response mappings |
| **Conceptual** | Module communities, hub abstractions, dependency topology | Plot thread structure, thematic communities, narrative architecture | Argument structure, knowledge gaps, synthesis opportunities | Emergent movement patterns, ensemble dynamics, performance evolution over time |

The movement/performance domain is particularly illustrative. In an interactive performance system (such as one mapping gesture to lighting and sound), the structural layer captures what the performer is doing right now — poses, transitions, spatial formations. The relational layer identifies movement vocabulary: which gestures cluster together, which transitions are practiced vs. novel. The semantic layer discovers choreographic structure: phrases that echo, develop, or contrast each other; performer-environment coupling patterns (gesture X reliably triggers lighting state Y). The conceptual layer reveals what emerges over time: how the performance vocabulary evolves across rehearsals, which multi-performer formations recur, which movement-environment couplings strengthen through use.

This is the same graph engine operating on different content-type analyzers. A tree-sitter parser and a pose tracker are structurally equivalent from Plexus's perspective: both produce nodes and edges at the structural layer, which feed upward into relational clustering, semantic extraction, and conceptual analysis. The self-reinforcing edge model works identically — a gesture-to-lighting edge that fires reliably strengthens, just as a function-call edge traversed by a developer strengthens.

Priority queuing ensures the semantic layer stays relevant: the currently active artifact (open file, active performer, focused document) gets highest priority, recently modified artifacts next, then breadth-first traversal of the rest. Content-hash caching means unchanged material is never re-extracted.

### 3.4 Self-Reinforcing Edge Dynamics

The self-reinforcing model implements three operations inspired by neuroplasticity:

| Operation | Neuroscience Analogue | Graph Behaviour |
|-----------|----------------------|----------------|
| Edge strengthening | Long-term potentiation (LTP) | Traversed or validated edges increase in weight |
| Temporal decay | Long-term depression (LTD) | Unaccessed edges lose retrieval strength over time |
| Emergent connections | Co-activation | Concepts that appear together across documents form new edges |

**Reinforcement sources** are heterogeneous:
- **User navigation**: Clicking a concept node, following an edge, expanding a cluster
- **Structural co-occurrence**: Two concepts appearing in the same file or function
- **Extraction validation**: A concept re-extracted from a modified document confirms existing edges
- **Cross-system feedback**: llm-orc execution outcomes, clawmarks trail traversal

**Decay function**: `w(t) = w₀ × e^(-λt)` where λ corresponds to a configurable half-life (default: 1 week). This ensures unused edges fade but don't disappear instantly — they remain discoverable but visually recede.

**Emergence**: When two concepts co-occur across multiple documents without an explicit edge, a new edge is created with initial weight proportional to co-occurrence frequency. This enables the graph to discover relationships the extractors didn't explicitly identify.

The combination produces a graph that converges on the relationships that actually matter to the practitioner — frequently traversed paths become highways, neglected connections fade to trails, and surprising co-occurrences surface as new paths.

### 3.5 Semantic Extraction Layer

The semantic extraction pipeline is validated experimentally in [Paper 1]. Key findings that inform the system design:

- **File tree traversal** provides 100% coverage and exploits organizational structure (directory co-location provides 9.3× stronger semantic signal than explicit links)
- **Evidence-grounded prompts** achieve 0% hallucination on technical corpora
- **Compositional extraction** (chunk → fan-out → aggregate) handles documents exceeding context windows
- **~10s LLM inference floor** on consumer hardware makes synchronous extraction impossible, validating the multi-frequency architecture
- **2 concurrent workers maximum** before error rates spike

The extraction pipeline routes documents through appropriate ensemble configurations based on content type and size, records provenance via clawmarks, and stores results in the Plexus graph with propagation to sibling documents.

---

## 4. Integration Points

### 4.1 llm-orc: Orchestration

llm-orc provides the LLM orchestration layer. It manages ensemble configurations (YAML files specifying agent chains), handles fan-out parallelism for compositional extraction, and supports multiple model profiles. The separation from Plexus is deliberate: extraction strategies evolve independently of graph storage.

The bidirectional integration goes beyond extraction: llm-orc execution outcomes feed back into Plexus as reinforcement signals. When an ensemble produces high-quality results, the concepts it extracted receive a confidence boost. When extraction fails or produces low-confidence output, the graph marks those regions for re-processing.

### 4.2 clawmarks: Provenance Tracking

clawmarks provides provenance for every concept in the graph. Each extracted concept links to a clawmark recording the source file, line number, evidence text span, and extraction session (trail). This enables:

- **"Go to source" UX**: Click a concept node → open the file at the exact line
- **Audit trails**: Every concept's extraction history is queryable
- **Confidence grounding**: Concepts with strong evidence spans receive higher confidence

Extraction sessions are organized as trails, providing a temporal narrative of how the graph was populated.

### 4.3 Trellis: Writer's Fragment Enrichment

<!-- PLACEHOLDER: New content to be written -->

Trellis is a creative writing scaffolding system built on the principle of *scaffolding, not generation* — it supports the writer's process without producing content on their behalf. Writers accumulate fragments: sentences, observations, character sketches, plot ideas, research notes. These fragments are the raw material of composition, but their interconnections are invisible until the writer manually traces them.

Plexus integration enriches these fragments with semantic structure:
- **Fragment nodes** enter the graph as they're created
- **Structural edges** connect fragments by proximity (written in the same session, filed in the same collection)
- **Relational edges** connect fragments by shared terms, character references, thematic overlap
- **Semantic edges** connect fragments by deeper conceptual relationships (discovered via LLM extraction)

The critical constraint is **non-interpretation**: Plexus reveals structure the writer has already created but does not impose interpretation. "These three fragments share the concept of 'isolation'" is structural observation. "This character is struggling with loneliness" is interpretation — and is explicitly outside Plexus's scope. The graph shows *what connects*; the writer decides *what it means*.

<!-- TODO: Add theoretical grounding — Vygotsky/ZPD, SDT, writing center pedagogy -->

### 4.4 EDDI: Gesture-to-Graph Pathway

<!-- PLACEHOLDER: New content to be written -->

EDDI (Environment-Driven Dynamic Interaction) is an interactive performance system that maps gesture to environment (lighting, sound, projection). The performer's body becomes the input device; the performance space becomes the output. Plexus provides EDDI with a memory — a graph that accumulates the history of performer-environment interactions and makes choreographic structure visible.

The gesture-to-graph pathway maps EDDI's data streams onto Plexus's four-layer model:

- **Structural layer**: Motion History Images (MHI) and Motion Energy Images (MEI) from pose estimation produce pose nodes, transition edges, and spatial formation data. Updated at camera frame rate, debounced to structural layer latency (<100ms).
- **Relational layer**: Laban Effort qualities (Weight, Time, Space, Flow) extracted from movement data cluster gestures into a movement vocabulary. Spatial proximity and temporal co-occurrence create relational edges between performers.
- **Semantic layer**: Choreographic phrases — sequences of gestures that form compositional units — are discovered through pattern recognition. Performer-environment coupling (gesture X triggers lighting state Y) becomes explicit as semantic edges with trigger/response semantics.
- **Self-reinforcing edges**: Performer-environment couplings that fire reliably strengthen. Movement phrases that recur across rehearsals gain weight. Novel gestures start with low-weight edges that strengthen through repetition — or fade if they were one-off explorations.

<!-- TODO: Add references — El Nasr arousal research, embodied cognition, Schödl video textures -->

---

## 5. Evaluation Agenda

Plexus makes several claims that require empirical validation. We outline the evaluation agenda as concrete, measurable studies.

### 5.1 Flow-State Hypothesis

**Claim**: A live knowledge graph providing ambient structural feedback produces more sustained flow states than traditional development tooling.

**Measurement**: Csikszentmihalyi's [19] three conditions (clear goals, immediate feedback, challenge-skill balance) provide an operationalizable framework. A within-subjects study comparing development sessions with and without Plexus, measuring:
- Time-in-flow (self-reported via experience sampling)
- Task-switching frequency (observable)
- Structural awareness accuracy (quiz on codebase topology before/after)

### 5.2 Self-Reinforcing Edge Convergence

**Claim**: The Hebbian edge dynamics cause the graph to converge on useful relationships — the graph "learns" what matters to the practitioner.

**Measurement**:
- Edge weight distribution over time (does it stabilize or oscillate?)
- Precision of high-weight edges (do the strongest edges correspond to relationships the practitioner considers important?)
- Comparison with static-weight baseline

### 5.3 Content-Agnostic Operation

**Claim**: The same graph engine serves code, text, and movement domains with only the analyzers differing.

**Measurement**: Deploy Plexus with three analyzer sets (tree-sitter for code, narrative parser for fiction, pose tracker for movement) and measure:
- Graph structural properties across domains (degree distribution, clustering coefficient, modularity)
- Whether the self-reinforcing dynamics produce comparable convergence behavior
- Whether the multi-frequency update model achieves target latencies across domains

### 5.4 Trellis Pilot Study

**Claim**: Fragment enrichment via Plexus helps writers discover connections without violating the non-interpretation constraint.

**Measurement**:
- Writer self-report on whether suggested connections feel like observation vs. interpretation
- Comparison of writing session productivity (fragments connected, ideas developed) with/without Plexus
- Qualitative analysis of writer interaction patterns

### 5.5 EDDI Integration

**Claim**: Gesture data can feed the structural layer of a knowledge graph, and the self-reinforcing dynamics produce useful performer-environment coupling patterns.

**Measurement**:
- Latency from gesture to graph update (target: <100ms structural, <2s relational)
- Whether high-weight edges correspond to intentional choreographic choices vs. noise
- Performer perception of the graph as useful rehearsal tool

---

## 6. Discussion and Conclusion

Plexus proposes that all creative composition shares a common structural dynamic: artifacts grow, connections form, patterns emerge, and the creator's awareness of this structure determines the quality of their engagement with their own work. By externalizing this structure in a self-reinforcing knowledge graph that updates at multiple frequencies, we aim to provide what amounts to peripheral vision for knowledge work.

The theoretical grounding spans external cognition [15]–[18], flow state [19]–[21], memory-inspired learning [28]–[30], and computational movement analysis [31]–[36]. The multi-frequency architecture [37]–[41] makes the system responsive despite the ~10s LLM extraction floor demonstrated in [Paper 1]. The self-reinforcing edge model, inspired by Hebbian dynamics, means the graph converges on what matters to each practitioner rather than presenting a static extraction result.

What distinguishes Plexus from existing knowledge graph systems is the combination of live updating, self-reinforcement, evidence provenance, and content-agnostic operation. No existing system (§2.8) integrates all of these. The closest precedent — Graphiti [25] — shares the real-time incremental approach but targets AI agent memory, not human creative practice, and lacks self-reinforcing dynamics, provenance, and visualization.

The system is partially built: the Rust graph engine exists, the semantic extraction pipeline is experimentally validated [Paper 1], and the llm-orc and clawmarks integrations are operational. What remains is the content-agnostic extension (Trellis, EDDI), the self-reinforcing edge dynamics, the ambient visualization layer, and — most importantly — empirical validation of the flow-state hypothesis that motivates the entire design.

We have deliberately separated what's built from what's designed from what's planned. The evaluation agenda (§5) specifies concrete, measurable studies for each claim. Until those studies are conducted, Plexus remains a grounded design with partial implementation — not a validated system.

The companion paper [Paper 1] demonstrates the methodology we intend to apply throughout: targeted experiments on real data, honest reporting of what failed, and design decisions backed by evidence rather than assumption. Extending this approach to the remaining subsystems — self-reinforcing dynamics, content-agnostic operation, flow-state measurement — is the next phase of work.

---

## References

### Knowledge Graph Construction

[1] Edge, D., Trinh, H., Cheng, N., Bradley, J., Chao, A., Mody, A., Truitt, S., & Larson, J. (2024). From Local to Global: A Graph RAG Approach to Query-Focused Summarization. *arXiv preprint arXiv:2404.16130*.

[2] Guo, Z., Xia, L., Yu, Y., Ao, T., & Huang, C. (2025). LightRAG: Simple and Fast Retrieval-Augmented Generation. In *Findings of the Association for Computational Linguistics: EMNLP 2025*, pp. 10746-10761.

[3] Neo4j. (2024). LLM Knowledge Graph Builder. https://neo4j.com/labs/genai-ecosystem/llm-graph-builder/

[4] Paranyushkin, D. (2019). InfraNodus: Generating insight using text network analysis. In *Proceedings of the World Wide Web Conference 2019* (WWW '19), pp. 3584-3589.

### Cognitive Context in AI-Assisted Development

[10] Cito, J. & Bork, D. (2025). Lost in Code Generation: Reimagining the Role of Software Models in AI-driven Software Engineering. *arXiv preprint arXiv:2511.02475*.

[11] Qiao, Y., Hundhausen, C., Haque, S., & Shihab, M. I. H. (2025). Comprehension-Performance Gap in GenAI-Assisted Brownfield Programming: A Replication and Extension. *arXiv preprint arXiv:2511.02922*.

[12] Al Haque, E., Brown, C., LaToza, T. D., & Johnson, B. (2025). Towards Decoding Developer Cognition in the Age of AI Assistants. *arXiv preprint arXiv:2501.02684*.

[13] Qodo. (2025). State of AI Code Quality in 2025. Industry Report. https://www.qodo.ai/reports/state-of-ai-code-quality/

[14] Sweller, J. (2024). Cognitive load theory and individual differences. *Learning and Individual Differences*, 110, 102423.

### External Cognition and Epistemic Tools

[15] Kirsh, D. & Maglio, P. (1994). On Distinguishing Epistemic from Pragmatic Action. *Cognitive Science*, 18(4), 513-549.

[16] Hutchins, E. (1995). *Cognition in the Wild.* MIT Press.

[17] Scaife, M. & Rogers, Y. (1996). External Cognition: How Do Graphical Representations Work? *International Journal of Human-Computer Studies*, 45(2), 185-213.

[18] Clark, A. & Chalmers, D. (1998). The Extended Mind. *Analysis*, 58(1), 7-19.

### Flow State and Creative Feedback

[19] Csikszentmihalyi, M. (1990). *Flow: The Psychology of Optimal Experience.* Harper & Row.

[20] Dietrich, A. (2004). Neurocognitive mechanisms underlying the experience of flow. *Consciousness and Cognition*, 13(4), 746-761.

[21] Matthews, T. et al. (2006). Designing and evaluating glanceable peripheral displays. In *Proc. DIS '06*, ACM.

### LLM-Based Knowledge Graph Construction

[22] Lairgi, Y., Moncla, L., Cazabet, R., Benabdeslem, K., & Cléau, P. (2024). iText2KG: Incremental Knowledge Graphs Construction Using Large Language Models. In *Proceedings of WISE 2024*. arXiv:2409.03284.

[23] Bian, H. et al. (2025). LLM-empowered Knowledge Graph Construction: A Survey. *arXiv preprint arXiv:2510.20345*.

[24] Agrawal, M. et al. (2024). Can Knowledge Graphs Reduce Hallucinations in LLMs? A Survey. In *Proceedings of NAACL 2024*.

### Incremental and Real-Time Knowledge Graphs

[25] Zep. (2024-2025). Graphiti: Temporally-Aware Knowledge Graphs. https://github.com/getzep/graphiti

[26] Van Assche, D. et al. (2024). IncRML: Incremental Knowledge Graph Construction from Heterogeneous Data Sources. *Semantic Web Journal*, Special Issue on Knowledge Graph Construction.

[27] Liu, J., Ke, W., Wang, P., Shang, Z., Gao, J., Li, G., Ji, K., & Liu, Y. (2024). Towards Continual Knowledge Graph Embedding via Incremental Distillation. In *Proceedings of AAAI 2024*, pp. 8759-8768.

### Self-Reinforcing and Memory-Inspired Knowledge Structures

[28] Bjork, R.A. & Bjork, E.L. (1992). A New Theory of Disuse and an Old Theory of Stimulus Fluctuation. In *From Learning Processes to Cognitive Processes*, Erlbaum.

[29] Settles, B. & Meeder, B. (2016). A Trainable Spaced Repetition Model for Language Learning. In *Proceedings of ACL 2016*.

[30] Zaidi, A. et al. (2020). Adaptive Forgetting Curves for Spaced Repetition Language Learning. In *AIED 2020*, Springer LNCS 12164, pp. 358-363.

### Computational Movement Analysis and Choreographic Structure

[31] Fdili Alaoui, S. et al. (2017). Seeing, Sensing and Recognizing Laban Movement Qualities. In *Proceedings of CHI 2017*, ACM.

[32] Garcia, M. et al. (2020). Recognition of Laban Effort Qualities from Hand Motion. In *Proceedings of MOCO 2020*, ACM.

[33] El Raheb, K. & Ioannidis, Y. (2012). A Labanotation Based Ontology for Representing Dance Movement. In *GW 2011*, Lecture Notes in Computer Science, vol. 7206, Springer, pp. 106-117.

[34] Paul, S., Das, P. P., & Rao, K. S. (2025). Ontology in Dance Domain—A Survey. *ACM Journal on Computing and Cultural Heritage*, 18(1), Article 16, pp. 1-32.

[35] Camurri, A., Hashimoto, S., Ricchetti, M., Trocca, R., Suzuki, K., & Volpe, G. (2000). EyesWeb: Toward Gesture and Affect Recognition in Interactive Dance and Music Systems. *Computer Music Journal*, 24(1), 57-69.

[36] Forsythe, W. (2008). Choreographic Objects. Essay.

### Multi-Frequency and Tiered Event Processing

[37] Marz, N. & Warren, J. (2015). *Big Data: Principles and Best Practices of Scalable Real-Time Data Systems.* Manning.

[38] Kreps, J. (2014). Questioning the Lambda Architecture. O'Reilly Blog.

[39] Luckham, D. (2002). *The Power of Events: An Introduction to Complex Event Processing.* Addison-Wesley.

[40] Baresi, L. & Guinea, S. (2013). Event-Based Multi-Level Service Monitoring. In *Proceedings of ICWS 2013*, IEEE.

[41] Keskisärkkä, R. (2014). Semantic Complex Event Processing for Decision Support. In *ISWC 2014*, Part II, Springer LNCS 8797, pp. 529-536.
