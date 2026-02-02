# Plexus: Architecture of a Live Knowledge Graph Engine

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — January 2026*

---

## What Plexus Is

Plexus is a knowledge graph engine designed for live creative composition. Unlike batch-processing knowledge graph systems that index finished documents (GraphRAG [1], LightRAG [2], iText2KG [22]), Plexus integrates with the creative environment and builds a semantic graph that evolves as content is composed. It receives data from domain-specific clients, processes it at multiple frequencies, and emits events that clients use to render ambient structural feedback.

This essay describes the architecture: what's built, what the design decisions are, and why.

## Design Constraints

Three constraints shape the architecture:

**1. Heterogeneous latency.** A live knowledge graph cannot update everything at once. Parsing an import statement takes milliseconds; extracting semantic concepts via LLM takes ~10 seconds per document [66]. The architecture must accommodate this variance — some layers update at keystroke speed, others in the background — without forcing the user to wait for the slowest operation.

**2. Domain independence.** The graph engine should not embed domain knowledge. Whether the nodes are functions, characters, or poses, the engine stores typed nodes and weighted edges, applies decay dynamics, and emits events. Domain knowledge lives in *semantic adapters* — pluggable components that extract structure from domain-specific content. This separation is a design hypothesis, not a validated property (see the companion essay on self-reinforcing dynamics for discussion of where it may break down).

**3. Ambient operation.** The graph exists to provide structural feedback without demanding attention. This means event-driven architecture: Plexus emits events as the graph changes; clients subscribe and render as they see fit. The engine never blocks the user's workflow.

## Architecture Overview

Plexus has three layers: the graph engine, semantic adapters, and domain consumers.

```
┌──────────────────────────────────────────┐
│          Domain Consumers                │
│   Manza (code) │ Trellis (writing) │ ...│
│         ▲ events    │ data ▼             │
├──────────────────────────────────────────┤
│          Plexus Graph Engine             │
│   Nodes, Edges, Decay, Events, MCP      │
│         ▲ extraction  │ requests ▼       │
├──────────────────────────────────────────┤
│          Semantic Adapters               │
│  llm-orc + clawmarks │ parsers │ ...    │
└──────────────────────────────────────────┘
```

**The graph engine** (Rust, SQLite storage) manages nodes, edges, weights, decay, and event emission. It is exposed via the Model Context Protocol (MCP), allowing any MCP-compatible client to send data and subscribe to events. The engine is operational.

**Semantic adapters** fulfill two required capabilities for each domain:
1. *Semantic extraction*: Transforming domain-specific content into graph nodes and edges at the relational, semantic, and conceptual layers.
2. *Provenance*: Grounding every extracted concept in its source material — the evidence chain from graph node back to the artifact that produced it.

For text-based domains, **llm-orc** (LLM orchestration) provides semantic extraction and **clawmarks** (provenance tracking) provides grounding. This reference implementation is validated experimentally in a companion paper [66]. Other domains use different strategies: deterministic parsers for structured code, gesture classifiers for movement, manual annotation where automated extraction is infeasible.

**Domain consumers** are independent systems that use Plexus. Manza (a file viewer and editor for code and documents) is operational. Trellis (creative writing scaffolding) has a working prototype [62]. Others can be built by any MCP client.

The architectural principle: **Plexus receives nodes and edges and emits events — it is agnostic to how those nodes and edges were derived.** The graph engine does not know whether a concept was extracted by an LLM, classified by a gesture recognizer, or produced by a deterministic parser.

## Data Model

The graph consists of:

- **Nodes**: Typed entities with properties and a content dimension (structural, relational, semantic, conceptual). Node types are client-defined strings, not engine-level enums — the engine stores whatever types clients send without interpreting them.
- **Edges**: Typed relationships with weight, decay parameters, and reinforcement history.
- **Contexts**: Scoped subgraphs (a project, a chapter, a rehearsal session) that partition the graph without duplicating nodes.
- **Dimensions**: The four semantic layers (structural, relational, semantic, conceptual) that organize nodes by abstraction level.

Edge weights evolve through three mechanisms: reinforcement (validation events increase weight), decay (exponential decay with configurable half-life), and emergence (co-occurring concepts form new edges). The dynamics are specified in detail in the companion essay on self-reinforcing dynamics.

## Multi-Frequency Update Model

The core engineering challenge: different semantic layers have fundamentally different computational costs, and the architecture must serve them all without blocking the user.

| Layer | Trigger | Target Latency | Method |
|-------|---------|----------------|--------|
| **Structural** | Every validation cycle / keystroke debounce | <100ms | Deterministic parsing (tree-sitter, regex), no LLM |
| **Relational** | On save or typing pause (>2s idle) | <2s | Lightweight text analysis, cached embeddings |
| **Semantic** | Background, priority-queued | 10–30s | LLM extraction (validated in [66]) |
| **Conceptual** | On explicit refresh or scheduled | Minutes | Network analysis, community detection |

**Implementation status:** The structural layer achieves <100ms updates in Manza's current implementation via tree-sitter parsing. The semantic layer's ~10s floor is validated in [66]. The full multi-frequency coordination model — particularly the handoff between layers and the priority queuing — is partially implemented. Fast structural updates work; slower semantic tiers are operational but the coordination model is incomplete.

This tiered approach has precedent in stream processing. The Lambda Architecture [37] processes data through parallel batch and speed layers. Luckham [39] formalizes hierarchical event abstraction in Complex Event Processing. Baresi and Guinea [40] propose multi-layer monitoring with processors at different frequencies — the closest architectural precedent. We frame the architecture in terms of tiered event processing because the key constraint is *heterogeneous latency*: different layers have fundamentally different costs, and the architecture must accommodate the variance.

**Priority queuing** ensures the semantic layer stays relevant: the currently active artifact (open file, focused document) gets highest priority, recently modified artifacts next, then breadth-first traversal of the rest. Content-hash caching means unchanged material is never re-extracted.

## The Semantic Adapter Interface

Plexus without semantic adapters is a dependency graph. The adapters are what populate the upper three layers (relational, semantic, conceptual) with richer structure. The interface is simple:

**Semantic extraction adapter**: Accept raw domain content, produce typed nodes with properties and weighted edges. The structural layer can be populated by clients directly (via deterministic analysis); the upper layers require an adapter.

**Provenance adapter**: Record source identifier (file, session, performer), location within source (line, timestamp, position), evidence span (the specific content that produced the concept), and extraction context (session, configuration, confidence).

The reference implementation for text domains:

**llm-orc** manages LLM ensemble configurations (YAML files specifying agent chains), handles fan-out parallelism for compositional extraction, and supports multiple model profiles. It routes documents through appropriate ensembles based on content type and size.

**clawmarks** records file, line number, evidence text span, and extraction session for every concept. This enables "go to source" — click a concept node, open the file at the exact line — and provides audit trails for how the graph was populated.

The bidirectional integration goes beyond extraction: llm-orc execution outcomes feed back into Plexus as reinforcement signals. When extraction produces high-quality results, the concepts receive a confidence boost. When extraction fails or produces low-confidence output, the graph marks those regions for re-processing.

Key findings from experimental validation [66]:
- File tree traversal provides 100% coverage and exploits organizational structure (directory co-location provides 9.3x stronger semantic signal than explicit links)
- Evidence-grounded prompts achieve 0% hallucination on technical corpora
- Compositional extraction (chunk → fan-out → aggregate) handles documents exceeding context windows
- ~10s LLM inference floor on consumer hardware validates the multi-frequency architecture
- 2 concurrent workers maximum before error rates spike

## Event Emission

Plexus is event-driven. As the graph changes, it emits typed events:

- Node created / updated / removed
- Edge created / strengthened / weakened / removed
- Cluster formed / dissolved
- Hub node identified
- Validation event processed

Clients subscribe to the events they care about and render them however suits their domain. Manza renders events as a visual graph display. Trellis translates events into coaching prompts. A hypothetical performance system could translate edge weight changes into environmental parameter shifts. The engine doesn't know or care what the client does with the events.

This separation — engine emits structured events, client decides rendering — is what makes the architecture extensible. Adding a new domain consumer requires implementing a semantic adapter and a client that subscribes to events. No engine changes needed (if the adapter interface is expressive enough — a hypothesis under test).

## What's Built, What's Designed, What's Planned

Honesty about implementation status:

**Built and operational:**
- Rust graph engine with SQLite storage
- MCP protocol for client integration
- Event emission architecture
- llm-orc + clawmarks semantic adapter (text domains)
- Manza integration (code/document client)
- Structural layer updates (<100ms via tree-sitter)
- Semantic layer extraction (validated in [66])

**Designed and partially implemented:**
- Multi-frequency coordination model (fast tiers work; coordination incomplete)
- Priority queuing for semantic extraction
- Trellis integration (prototype exists [62]; Plexus integration designed)

**Designed but not yet built:**
- Self-reinforcing edge dynamics (design specification — see companion essay)
- Active invalidation
- Emergent edge creation
- Additional semantic adapters (movement domain, deterministic parsers)
- EDDI (interactive performance consumer)

## Open Questions

- **Does the adapter interface generalize?** The text-domain adapter works. Whether the same interface serves movement, research, and other domains without engine-level modifications is the content-agnosticism question — testable only by building more adapters.
- **How does the graph scale?** The engine has not been evaluated under sustained load from large codebases or long manuscripts. SQLite may become a bottleneck; the persistence layer may need to evolve.
- **Is MCP the right protocol?** MCP provides a standard interface but imposes constraints. Whether its request/response model accommodates high-frequency event streams (structural layer updates at keystroke speed) needs validation under realistic conditions.

---

## References

[1] Edge, D. et al. (2024). From Local to Global: A Graph RAG Approach to Query-Focused Summarization. *arXiv:2404.16130*.

[2] Guo, Z. et al. (2025). LightRAG. In *Findings of ACL: EMNLP 2025*, pp. 10746-10761.

[22] Lairgi, Y. et al. (2024). iText2KG. In *Proc. WISE 2024*. arXiv:2409.03284.

[25] Rasmussen, P. (2025). Zep: A Temporal Knowledge Graph Architecture for Agent Memory. *arXiv:2501.13956*.

[37] Marz, N. & Warren, J. (2015). *Big Data: Principles and Best Practices of Scalable Real-Time Data Systems.* Manning.

[39] Luckham, D. (2002). *The Power of Events.* Addison-Wesley.

[40] Baresi, L. & Guinea, S. (2013). Event-Based Multi-Level Service Monitoring. In *Proc. ICWS 2013*, IEEE.

[62] Green, N. (2026). Trellis: A Creative Scaffolding System for Writers. *Working Paper*.

[66] Green, N. (2026). Semantic Extraction for Live Knowledge Graphs: An Empirical Study. *Working Paper*.
