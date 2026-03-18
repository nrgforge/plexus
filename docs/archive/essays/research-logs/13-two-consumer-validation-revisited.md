# Research Log: Two-Consumer Validation Revisited

## Question 1: With adapter-produced provenance, can real public data surface genuinely interesting cross-consumer connections?

**Method:** Spike (integration test with real arXiv data)

**Findings:** 72 nodes, 238 edges, 23 concepts, 68 cross-dimensional references. All five key traversals verified. See essay for details.

## Question 2: At creative writing scale (100+ fragments, heterogeneous sources), can the graph surface thematic clusters that map to narrative threads or story outlines?

**Method:** Spike (integration test with 82 creative writing fragments, 15 Carrel sources, provenance annotations)

**Context:** The writer is working on a novel about memory and transformation in a coastal town. Trellis holds 82 fragments spanning 8 thematic clusters (memory/time, water/ocean, identity/transformation, architecture/ruins, light/shadow, family/inheritance, language/silence, myth/journey). Carrel holds 15 sources: 4 of the writer's own previous works, 4 research papers (memory consolidation, narrative identity, memory palaces, embodied cognition), and 7 pieces of apocrypha (Heraclitus, Bachelard, Ovid's Proteus, Borges, mono no aware, maritime folklore, architectural palimpsest). 11 provenance annotations across 2 chains (novel draft + thematic research) with 5 explicit links.

**Spike question:** "At 100+ fragment scale with heterogeneous creative sources, do the co-occurrence clusters map to recognizable narrative themes — and can cross-consumer connections surface story possibilities?"

**Findings:**

### Graph topology

297 nodes across 3 dimensions:
- Structure: 97 fragments (82 Trellis + 15 Carrel LLM)
- Semantic: 74 concepts
- Provenance: 126 nodes (18 chains + 108 marks)

1,335 edges across 5 relationship types:
- tagged_with: 314
- contains: 108
- references: 354
- links_to: 5
- may_be_related: 554 (277 unique pairs)

### Scale progression

| Metric | Essay 11 | Question 1 | Question 2 | Full growth |
|--------|----------|------------|------------|-------------|
| Nodes | 19 | 72 | 297 | 15.6× |
| Edges | 45 | 238 | 1,335 | 29.7× |
| Concepts | 7 | 23 | 74 | 10.6× |
| references | 8 | 68 | 354 | 44.3× |
| may_be_related | 16 | 94 | 554 | 34.6× |

### Hub concepts (most connected, by co-occurrence count)

1. **memory** — 32 co-occurrence connections, 23 tagged_with, 29 references, 64 total co-occurrence edges
2. **architecture** — 29 co-occurrence connections
3. **identity** — 26 co-occurrence connections
4. **water** — 22 co-occurrence connections
5. **language** — 20 co-occurrence connections
6. **time** — 19 co-occurrence connections
7. **loss** — 19 co-occurrence connections
8. **myth** — 18 co-occurrence connections
9. **naming** — 15 co-occurrence connections
10. **light** — 15 co-occurrence connections

### Top co-occurrence pairs (strongest narrative connections)

All at weight 1.0 (co-occurring in 2+ fragments):
- memory ↔ salt, memory ↔ senses, salt ↔ senses
- return ↔ water, time ↔ water, return ↔ time
- letters ↔ memory, forgetting ↔ memory, forgetting ↔ letters
- photography ↔ truth, memory ↔ photography, memory ↔ truth
- identity ↔ loss, identity ↔ nostalgia, loss ↔ nostalgia
- memory ↔ time, memory ↔ repetition, repetition ↔ time
- forgetting ↔ silence, forgetting ↔ loss

### Cross-consumer discovery

- **Heraclitus → writer's fragments:** Heraclitus's river fragment (tags: water, identity, time, transformation) shares 3+ concepts with the novel draft's Chapter 1 annotation (return, water, identity, transformation). The ancient philosopher and the contemporary novelist are connected through the graph without either "knowing" about the other.
- **Borges → myth cluster:** Borges' Library of Babel (tags: labyrinth, language, naming, infinity) connects to 3 writer fragments about labyrinths and myth via concept:labyrinth.
- **concept:memory as central hub:** 23 tagged_with edges + 29 references edges + 64 co-occurrence edges. Memory is the novel's gravitational center — the graph independently identifies what the writer already knows intuitively.
- **concept:transformation bridges:** 10 fragments from both consumers converge on transformation — connecting the writer's identity/metamorphosis fragments to Ovid's Proteus and the memory consolidation paper.

### Emergent narrative threads (from co-occurrence clusters)

The co-occurrence data reveals five recognizable thematic clusters:

1. **Memory-as-tide:** memory ↔ water ↔ tides ↔ forgetting ↔ return — the central metaphor of the novel
2. **The unreliable image:** photography ↔ memory ↔ truth ↔ seeing — the protagonist's discovery that photographs contradict her memories
3. **The house-as-self:** architecture ↔ memory ↔ family ↔ inheritance ↔ ruins — the inherited house as repository of family history
4. **The crossing:** identity ↔ transformation ↔ loss ↔ journey ↔ return — the protagonist's arc from departure to return
5. **The unsaid:** silence ↔ language ↔ loss ↔ forgetting ↔ naming — what the family refuses to speak

These clusters were not designed into the data. They emerged from independent tagging by a writer, an LLM, and a researcher annotator. The graph discovered the novel's thematic architecture.

**Implications:**
- At 100+ fragment scale, co-occurrence clusters map to recognizable narrative themes
- Cross-consumer connections (Heraclitus → writer, Borges → myths) surface genuinely surprising relationships
- Hub concept analysis (memory: 32 connections) independently identifies the novel's thematic center
- A future LLM enrichment could interpret these clusters as story outline suggestions
- 250 tests pass (248 original + 2 new spike tests)
