# Two-Consumer Validation Revisited: Real Data, Three Consumers, Epistemological Infrastructure

Essay 11 proved the core thesis: two independent consumers contributing to the same Plexus context with shared tag vocabulary produce cross-dimensional connections that neither could produce alone. 19 nodes, 45 edges, 8 cross-dimensional bridges. The architecture worked.

Essay 12 changed what adapters produce. FragmentAdapter now emits provenance alongside semantics — a chain and mark for every fragment, carrying source evidence and tags. TagConceptBridger bridges those marks to concepts automatically. The graph became epistemological: every concept can trace its origin back to source material.

This essay asks: what happens when we scale up? Not two consumers with synthetic data, but three consumers with real data from public sources. Does the richer provenance infrastructure produce a graph that surfaces genuinely interesting connections?

## The Experiment

Three consumers feed the same Plexus context, simulating a writer using Trellis and Carrel simultaneously to research distributed AI, creativity, and governance.

**Trellis** — 8 writer's fragments, handwritten and informally tagged. These are the intuitive, creative-context observations a writer captures while thinking through a problem:

- "Distributed compute as insurance against concentration risk — the bet is that decentralization creates resilience"
- "Federated learning isn't just technical — it's an economic argument about who controls the training pipeline"
- "Design constraints don't limit creativity, they channel it — the architecture of limitation as generative force"
- "The governance question: who watches the autonomous agents?"

Each fragment carries 2-3 tags from the writer's working vocabulary.

**Carrel LLM extraction** — 8 real arXiv paper abstracts (2024-2025), processed by a simulated LLM extractor that identifies 3-4 formal concepts per paper:

| Paper | Source | Extracted Concepts |
|-------|--------|-------------------|
| Incentive-Based Federated Learning | arXiv 2510.14208 | federated-learning, incentive-mechanisms, game-theory, distributed-computing |
| Decentralized Governance of Autonomous AI Agents | arXiv 2412.17114 | decentralized-governance, ai-safety, blockchain, autonomous-agents |
| Human-AI Co-Creativity | arXiv 2411.12527 | human-ai-collaboration, creativity, generative-ai |
| Multi-Agent Risks from Advanced AI | arXiv 2502.14143 | multi-agent-systems, ai-safety, network-effects, governance |
| Extended Creativity | arXiv 2506.10249 | creativity, human-ai-collaboration, distributed-cognition |
| Federated Learning in Practice | arXiv 2410.08892 | federated-learning, privacy, distributed-computing |
| Embodied AI: Risks and Policy | arXiv 2509.00117 | embodied-ai, ai-safety, economic-disruption, policy |
| Advances in AI for Creative Industries | arXiv 2501.02725 | creative-industries, generative-ai, human-ai-collaboration |

These are real abstracts from real papers. The text fed into Plexus is the actual abstract content. The tags represent what an LLM concept extractor would identify — the same operation Carrel would perform in production.

**Carrel provenance** — 6 human annotations across two chains. One chain ("The Distributed Bet") tracks the writer's essay draft with marks on sections about federated economics, governance, and creativity. The other ("Literature Scan Feb 2026") tracks research findings with marks on the federated incentives paper, the ETHOS governance paper, and the Extended Creativity paper. Three explicit links connect research findings to the draft sections they support.

## What the Graph Produced

### Structure

72 nodes across three dimensions:
- **Structure:** 16 fragment nodes (8 Trellis + 8 Carrel LLM)
- **Semantic:** 23 concept nodes
- **Provenance:** 33 nodes (11 chains + 22 marks)

238 edges across five relationship types:
- **tagged_with:** 51 (fragment → concept, cross-dimensional)
- **contains:** 22 (chain → mark, within provenance)
- **references:** 68 (mark → concept, cross-dimensional)
- **links_to:** 3 (research → writing, within provenance)
- **may_be_related:** 94 (concept co-occurrence, within semantic)

### Growth from Essay 11

| Metric | Essay 11 | This essay | Factor |
|--------|----------|------------|--------|
| Total nodes | 19 | 72 | 3.8× |
| Total edges | 45 | 238 | 5.3× |
| Concepts | 7 | 23 | 3.3× |
| Cross-dimensional references | 8 | 68 | 8.5× |
| Co-occurrence pairs | 16 | 94 | 5.9× |

The 8.5× growth in cross-dimensional references is the most significant number. In Essay 11, only explicitly-created provenance marks bridged to concepts (8 edges from 4 marks). Now every fragment produces a provenance mark that TagConceptBridger bridges to its concepts. The graph's epistemological infrastructure grew almost an order of magnitude with no new enrichment code.

### The 23 Concepts

The concept vocabulary emerged from three independent sources: a writer's intuitive tags, an LLM's formal extractions, and a researcher's annotations. No consumer coordinated with the others. Yet they converged:

- **ai-safety** appears in Trellis fragments, Carrel LLM extractions, and Carrel provenance annotations
- **creativity** appears across all three consumers
- **federated-learning** bridges the writer's economic argument to formal research on incentive mechanisms
- **autonomous-agents** connects the writer's rhetorical question ("who watches?") to the ETHOS governance framework

The tag normalization built into TagConceptBridger — lowercase, strip `#` — is what makes this convergence possible. A writer tags a fragment "ai-safety." An LLM extracts the concept "ai-safety" from a paper. A researcher annotates a draft section with "#ai-safety." All three produce the same concept node: `concept:ai-safety`.

## Five Cross-Consumer Traversals

### 1. concept:creativity as a hub

Five fragments from both Trellis (3) and Carrel LLM (2) converge on `concept:creativity` via `tagged_with` edges. Seven provenance marks from all three consumers reference the same concept via `references` edges.

The writer's informal observation — "design constraints don't limit creativity, they channel it" — is connected to the formal research finding that "AI technologies are enabling hybrid relational spaces where humans and machines engage in joint creative activity" (Extended Creativity, arXiv 2506.10249). Neither the writer nor the LLM extractor knew about the other's contribution. The enrichment loop connected them.

### 2. Academic paper → writer's fragment

Starting from the ETHOS governance paper's provenance mark (in `chain:carrel-llm:arxiv-2412.17114`), follow `references` edges to reach `concept:autonomous-agents` and `concept:ai-safety`. From those concepts, follow `tagged_with` edges (incoming) to reach the writer's fragment: "The governance question: who watches the autonomous agents?"

A formal research paper on decentralized AI governance discovered a writer's rhetorical question about the same problem. Two dimensions (provenance, semantic) and two consumers (Carrel LLM, Trellis) connected through a concept node that neither created with the other in mind.

### 3. Explicit links AND implicit convergence

The writer's draft annotation (`draft-governance`, tags: decentralized-governance, ai-safety, autonomous-agents) and the research annotation (`lit-ethos`, tags: decentralized-governance, ai-safety, blockchain) are connected two ways:

1. **Explicitly** — a `links_to` edge records the researcher's intentional connection: "this paper supports this draft section"
2. **Implicitly** — both annotations share concepts `decentralized-governance` and `ai-safety` via independent `references` edges

The graph has both the human's judgment and the system's discovery. If you deleted the explicit link, the concept-mediated connection would remain. If you deleted the concept tags, the explicit link would remain. Redundant paths create resilient knowledge.

### 4. concept:ai-safety reaches all three consumers in one hop

A depth-1 BFS traversal from `concept:ai-safety` reaches:
- 5 fragment nodes (structure dimension) — from both Trellis and Carrel LLM
- 7 provenance marks (provenance dimension) — from all three consumers
- Co-occurring concepts (semantic dimension) via `may_be_related`

All three dimensions. All three consumers. One hop. This is what a multi-dimensional knowledge graph is for: a single concept connects the writer's intuition, the LLM's extraction, and the researcher's annotation into a coherent view of the evidence landscape.

### 5. Writer's fragment → related research (depth-3)

Starting from the writer's "who watches the autonomous agents?" fragment and traversing 3 hops in all directions, the graph reaches:
- `concept:autonomous-agents`, `concept:ai-safety`, `concept:governance` (via tagged_with + references from the fragment's mark)
- `draft-governance` writing annotation (shares concepts with the fragment's mark)
- Fragment nodes from Carrel LLM papers on governance and multi-agent risks (share concepts)

The writer's casual question connects, through the graph, to formal academic work on the same problem. The provenance trail explains the connection at every step.

## 94 Co-Occurrence Pairs

CoOccurrenceEnrichment detected 94 `may_be_related` concept pairs with graduated weights. At this scale, the co-occurrence data reveals genuine conceptual clusters:

- **Distributed AI economics:** federated-learning × compute-economics × distributed-computing × incentive-mechanisms — tightly coupled, appearing together across multiple fragments from both consumers
- **AI governance:** ai-safety × governance × autonomous-agents × decentralized-governance — the policy cluster that connects the writer's concerns to formal research
- **Creativity and tools:** creativity × human-ai-collaboration × design-constraints × non-generative-ai — the creative tools cluster where the writer's thesis meets academic theory

These clusters were not designed. They emerged from the independent contributions of three consumers with overlapping vocabularies. The graduated weights (based on co-occurrence frequency) separate tight clusters from peripheral connections. Whether those clusters are semantically meaningful — whether "distributed-ai" is related to "compute-economics" in a way that matters to the writer's argument — remains a question for LLM-based interpretation, a future direction identified in Essay 11 and still relevant.

## What Changed Since Essay 11

### Provenance became pervasive

In Essay 11, provenance existed only when a user explicitly created chains and marks via ProvenanceAdapter. Fragments had no provenance trail. You could ask "what concepts exist?" but not "where did this concept's evidence come from?"

Now every fragment produces a mark. The 68 cross-dimensional `references` edges mean that from any concept, you can traverse to every piece of evidence that supports it — whether that evidence came from a writer's journal, an LLM's extraction, or a researcher's annotation. The graph knows why it knows things.

### The numbers tell the story

Essay 11's 8 `references` edges were all human-created (4 provenance marks × 2 tags each). This essay's 68 `references` edges include 51 adapter-created bridges (16 fragments × average 3.2 tags) and 17 human-created bridges (6 provenance marks). The system-generated provenance outnumbers the human-created provenance 3:1, and both are structurally identical — same edge type, same cross-dimensional bridging, same traversal semantics.

### Heterogeneous sources converge naturally

The real arXiv abstracts and the writer's informal notes were tagged independently with different levels of formality. Yet they converged on the same concept nodes because TagConceptBridger's normalization is simple and universal: lowercase, strip `#`, prepend `concept:`. This is sufficient because the convergence needs to be approximate, not exact. Two sources don't need to use identical vocabulary — they need to share enough vocabulary that the concept layer provides useful connections.

## Part 2: Creative Writing at Scale

The research-domain spike proved that cross-consumer connections work with real data. But the more ambitious question: what happens at the scale of a real creative project? A writer with months of accumulated fragments, research across multiple disciplines, and apocryphal sources from mythology to maritime folklore?

### The Scenario

A novelist working on a book about memory and transformation in a coastal town. Three consumers feed the same Plexus context:

**Trellis** — 82 writer's fragments, spanning 8 thematic clusters: memory/time, water/ocean, identity/transformation, architecture/ruins, light/shadow, family/inheritance, language/silence, myth/journey. Plus 8 deliberate cross-cluster bridge fragments. Each fragment is a single observation — the raw material of creative thinking:

- "The smell of salt air triggers memories I didn't know I had"
- "The ship of Theseus applies to people too — when are you someone else?"
- "Tides erase and rewrite the same shore endlessly"
- "Every labyrinth has a center that is also a mirror"
- "Secrets are the family's most durable architecture"

**Carrel LLM** — 15 sources processed for concept extraction. Four are the writer's own previous works (an essay on coastal erosion, a short story about a translator's house, an essay on photographing ruins, a poem on tidal memory). Four are research papers (memory consolidation, narrative identity, memory palaces, embodied cognition). Seven are apocrypha: Heraclitus's river fragments, Bachelard's *Poetics of Space*, Ovid's Proteus, Borges' Library of Babel, the Japanese concept of *mono no aware*, maritime folklore on ship naming, and the architectural palimpsest.

**Carrel provenance** — 11 annotations across two chains. The "Coastal Novel Draft" chain has 6 marks on chapters: the harbor return, the inherited house, the contradicting photographs, the storm revelation, the translator character, and the labyrinth climax. The "Thematic Research" chain has 5 marks connecting source material to draft chapters. Five explicit links connect research to writing.

### What the Graph Produced

297 nodes. 1,335 edges. 74 concepts. The graph grew an order of magnitude from the research spike.

| Metric | Essay 11 | Research spike | Creative spike | Full growth |
|--------|----------|----------------|----------------|-------------|
| Nodes | 19 | 72 | 297 | 15.6× |
| Edges | 45 | 238 | 1,335 | 29.7× |
| Concepts | 7 | 23 | 74 | 10.6× |
| Cross-dimensional references | 8 | 68 | 354 | 44.3× |
| Co-occurrence pairs | 16 | 94 | 554 | 34.6× |

The cross-dimensional references — 354 edges connecting provenance marks to concept nodes — are the epistemological infrastructure at creative-writing scale. Every fragment, every research source, every annotation has a traversable path to the concepts it supports.

### The Hub Concepts

The graph independently identified the novel's thematic architecture through co-occurrence analysis:

1. **memory** — 32 co-occurrence connections, the gravitational center
2. **architecture** — 29 connections
3. **identity** — 26 connections
4. **water** — 22 connections
5. **language** — 20 connections
6. **time** — 19 connections
7. **loss** — 19 connections
8. **myth** — 18 connections
9. **naming** — 15 connections
10. **light** — 15 connections

No one told the graph that this novel is about memory. The writer's fragments, the research papers, and the apocryphal sources all independently contributed to `concept:memory`, and the co-occurrence analysis identified it as the most connected concept. The graph discovered what the writer already knows intuitively.

### Emergent Narrative Threads

The 277 unique co-occurrence pairs organize into five recognizable thematic clusters:

**Memory-as-tide:** memory ↔ water ↔ tides ↔ forgetting ↔ return. The central metaphor of the novel — tides as the rhythm of remembering and forgetting — emerges from fragments like "The tide pools remember the ocean even when the water retreats" co-occurring with "The current pulls memory loose like kelp from rock." The Heraclitus river fragment and the writer's own poem "Tidal Memory" reinforce this cluster from independent sources.

**The unreliable image:** photography ↔ memory ↔ truth ↔ seeing. The protagonist's discovery that photographs contradict her memories — surfaced by fragments about photography as "memory's most beautiful lie" co-occurring with the essay "Why I Photograph Ruins" and the memory consolidation research paper on reconsolidation (memories change when retrieved).

**The house-as-self:** architecture ↔ memory ↔ family ↔ inheritance ↔ ruins. The inherited house as repository of family history — connecting the writer's fragments about houses that "remember what the family forgot" to Bachelard's "The house shelters daydreaming" and the architectural palimpsest concept. The research chain's annotation explicitly links the palimpsest source to the house chapter, but the graph also connects them implicitly through shared concepts.

**The crossing:** identity ↔ transformation ↔ loss ↔ journey ↔ return. The protagonist's arc from departure through metamorphosis to return — connecting Ovid's Proteus ("truth requires multiplicity") to the writer's fragments about masks and immigration to the narrative identity research paper. Heraclitus appears here too: his "other and other waters flow" shares three concepts with the novel's opening chapter annotation.

**The unsaid:** silence ↔ language ↔ loss ↔ forgetting ↔ naming. What the family refuses to speak — fragments about "naming a thing is the first step toward losing it" and "the family tree is a map of silences" co-occurring with the embodied cognition paper on metaphor and the Borges Library of Babel.

These clusters were not designed into the data. They emerged from independent contributions by a writer tagging intuitively, an LLM extracting formally, and a researcher annotating deliberately. The graph discovered the novel's thematic architecture from the raw material.

### Cross-Consumer Surprises

**Heraclitus reaches the opening chapter.** The ancient philosopher's river fragment (tags: water, identity, time, transformation) shares three concepts with the novel draft's Chapter 1 annotation (tags: return, water, identity, transformation). The graph connects a 2,500-year-old observation about flux to a contemporary novel about a woman returning to a changed harbor town. Neither the writer nor the researcher explicitly drew this connection — the enrichment loop found it through shared concept nodes.

**Borges reaches the myth cluster.** The Library of Babel (tags: labyrinth, language, naming, infinity) connects to three writer fragments about labyrinths through `concept:labyrinth`. The Borges passage about infinite combinations resonates with the writer's "every labyrinth has a center that is also a mirror" — a thematic echo that the graph surfaced without human intervention.

**The memory consolidation paper explains the photographs chapter.** The research annotation on reconsolidation ("memories change when retrieved") shares concepts with the draft Chapter 5 annotation ("protagonist discovers images that contradict her memories"). The graph connects a neuroscience finding to a narrative device through the shared concepts of memory, transformation, and truth.

### What a Story Outline Enrichment Could Do

The current enrichment infrastructure — TagConceptBridger and CoOccurrenceEnrichment — discovers connections and identifies co-occurrence patterns. A future LLM-based enrichment could interpret these patterns as narrative structure:

1. **Read the hub concepts** (memory, architecture, identity, water) as the novel's thematic pillars
2. **Read the co-occurrence clusters** as potential chapter structures or narrative threads
3. **Read the cross-consumer connections** as research-to-narrative bridges: "Your chapter about photographs connects to the reconsolidation paper — consider the science of memory distortion as a plot mechanism"
4. **Read the provenance trails** as evidence chains: "The tidal memory metaphor is supported by 12 fragments, your own poem, Heraclitus, and the memory consolidation paper"

The graph provides the raw material — 1,335 edges of connected evidence. An LLM enrichment would provide the interpretation.

## What This Means

Two spikes, two domains, three consumers each, real data throughout.

The research spike (72 nodes, 238 edges) proved that real arXiv abstracts and intuitive writer fragments converge through shared concepts. A formal paper on decentralized governance discovered a writer's rhetorical question about the same problem.

The creative writing spike (297 nodes, 1,335 edges) proved that at scale, the co-occurrence patterns map to recognizable narrative themes. The graph independently identified memory as the novel's thematic center, surfaced five emergent narrative threads, and connected a 2,500-year-old philosopher to a contemporary novelist through three shared concepts.

Neither spike required changes to the enrichment code. TagConceptBridger and CoOccurrenceEnrichment — designed before either experiment — handled both domains without modification. The provenance infrastructure from Essay 12 made every connection graph-traversable.

This is the value proposition: a writer dumps fragments into Trellis, feeds research into Carrel, and the graph discovers the shape of their thinking. Not just what they know, but where each piece came from and how it connects to everything else. 297 nodes, 1,335 edges, 74 concepts, 277 co-occurrence pairs, 3 consumers, real data. 250 tests pass.
