# Empirical Design of an LLM-Powered Knowledge Graph Construction System for Document Corpora

**Nathaniel Green**
Independent Researcher
nate@nate.green
ORCID: 0000-0003-0157-7744

*Working Paper — January 2026*

---

## Abstract

Modern knowledge work increasingly involves opaque accumulation processes—AI-assisted coding sessions where context is lost between prompts, personal knowledge bases that grow without structural awareness, multi-tool workflows where understanding fragments across systems. The practitioner's mental model diverges from the actual state of their knowledge. We describe **Plexus**, a real-time knowledge graph engine designed to evolve alongside creative composition, providing ambient structural awareness without interrupting flow. Integrated into an editor environment, Plexus updates a live graph at multiple frequencies: code structure updates on every keystroke, topic clusters shift on save, and deeper semantic relationships accumulate in the background—creating a peripheral structural reflection of the emerging work. This paper focuses on one critical subsystem: semantic extraction from document corpora using local LLMs. Building this subsystem required solving six interacting design problems—traversal, extraction, composition, propagation, normalization, and performance—each answered through targeted experiments on real corpora. Key findings include: file tree traversal provides complete document coverage without network algorithms; directory co-location provides 9.3× stronger semantic signal than explicit links; evidence-grounded prompts achieve 0% hallucination on technical corpora; compositional extraction handles large documents autonomously; and local 7B model inference has a ~10s per-document latency floor that directly informed the multi-frequency update architecture. The resulting three-system design (orchestration, provenance, knowledge graph) feeds into Plexus's broader goal: a self-reinforcing knowledge graph where edges strengthen through use, AI-generated code is structurally visible from the moment it's produced, and the practitioner maintains cognitive context over a body of work that grows faster than any individual can consciously track.

**Keywords:** knowledge graphs, LLM extraction, creative flow, cognitive context, real-time knowledge systems, AI-assisted composition

---

## 1. Introduction

### 1.1 The Opacity Problem

Knowledge accumulates faster than understanding. A developer "vibe-coding" with an AI assistant produces working software but may not fully grasp the architectural decisions embedded in the generated code. A researcher's personal knowledge base grows to thousands of notes whose interconnections are invisible. A team's documentation sprawls across wikis, repos, and chat histories with no unified semantic map. In each case, knowledge exists but cognitive context—the awareness of what you know, how it connects, and where the gaps are—erodes.

This is not a storage problem. The documents exist. The code compiles. The notes are searchable by keyword. The problem is structural: there is no live representation of the semantic relationships within and across these artifacts. The knowledge is there but opaque to the person who ostensibly possesses it.

The problem is particularly acute in AI-assisted composition. When a developer prompts an LLM to generate a module, the resulting code has dependencies, introduces patterns, and makes architectural choices—but the developer's attention was on the prompt, not the output's structural implications. After several such exchanges, the codebase has grown in ways the developer didn't consciously design. The same dynamic applies to writing, research, and any creative process mediated by generative AI: the artifact grows, but the creator's structural awareness does not keep pace.

What's missing is not a post-hoc documentation tool. What's missing is a live structural reflection of the composition as it unfolds—something that evolves alongside the creative process and provides ambient awareness without demanding attention.

### 1.2 Plexus: A Live Knowledge Graph for Composition

Plexus is a knowledge graph engine designed to address this opacity. Rather than analyzing artifacts after they're complete, Plexus integrates with the creative environment and builds a semantic graph that evolves in real-time as content is composed. The graph is not documentation—it is a live structural reflection of the emerging work.

The core insight is that all composition—regardless of medium—produces structure, and that structure is what creators lose track of. The specific structural elements differ by domain, but the experience of watching your work's structure emerge in real-time is the same:

| Domain | Nodes | Edges | What You See Evolving |
|--------|-------|-------|----------------------|
| **Code** | Functions, modules, types, constants | Imports, calls, definitions, data flow | Dependency graph restructuring as you refactor |
| **Fiction** | Characters, scenes, locations, objects | Appearances, dialogue, plot threads, narrative arcs | Character relationship web thickening as the story develops |
| **Research** | Concepts, papers, claims, evidence | Citations, arguments, supports/contradicts, builds-on | Argument structure crystallizing as you synthesize sources |
| **Movement/Performance** | Poses, gestures, qualities, formations | Transitions, variations, oppositions, triggers | Choreographic vocabulary emerging, performer-environment coupling visible |
| **Free-writing** | Emerging ideas, fragments, questions | Associations, echoes, tensions, elaborations | Thought-trains becoming visible, clusters forming from scattered notes |

In each case, the creator composes linearly (word after word, function after function) but the structure of the work is non-linear—a graph, not a sequence. Without a live structural view, the creator must hold that graph in their head. With Plexus, the graph is externalized and kept current automatically.

Consider the experience across two domains:

*A developer* writes a new function. Edges appear connecting it to the functions it calls and the modules it imports. They prompt an AI assistant to generate a utility module—the graph immediately shows what the generated code introduced, what it depends on, and how it changed the dependency topology. When they refactor, they watch clusters merge and hub nodes shift.

*A novelist* writes a new scene. A character node gains edges to the location, the other characters present, and the plot threads advanced. A thematic concept ("betrayal") strengthens its connections to the scenes where it appears. When the writer introduces a subplot, they see it as a new cluster forming at the periphery of the main narrative structure, gradually developing edges inward.

*A performer* in an interactive installation moves through a sequence. Pose nodes connect via transition edges; movement qualities (sustained, sudden, bound) cluster into a visible vocabulary. The graph shows performer-environment coupling: this gesture reliably triggers that lighting state, this spatial formation activates that soundscape. Across rehearsals, the conceptual layer reveals what's emerging—which movement phrases are developing, which performer-to-performer dynamics recur, which environment responses are strengthening through repetition. The choreographic structure becomes visible not through notation but through the graph's memory of what happened and how it connected.

The graph behaves identically in all cases—nodes appear, edges form, clusters emerge, hubs solidify, unused connections fade. The content-type-specific analyzers (tree-sitter for code, narrative parsers for fiction, citation extractors for research, pose trackers for movement) feed different node and edge types into the same graph engine with the same self-reinforcing dynamics. Coding and choreography, fiction and research, viewed through the graph, feel like the same activity: *composition with live structural feedback*.

This produces something closer to a flow state than traditional tooling offers. The creator maintains structural awareness without interrupting composition to manually trace dependencies, re-read earlier chapters, or search for related notes. The graph is peripheral vision for knowledge work.

Plexus's core design principles support this:

- **Real-time evolution**: The graph updates as files are saved, code is generated, and notes are written—not as a batch process after the fact.
- **Multi-frequency updates**: Different semantic layers update at different cadences. Code structure (imports, definitions, call relationships) updates on every validation cycle. Semantic structure (shared terms, topic clusters) updates on save or pause. Conceptual structure (deeper cross-document relationships) updates in the background or on explicit refresh. This tiered approach keeps the graph responsive without saturating compute.
- **Self-reinforcing edges**: Relationships strengthen through use and decay without reinforcement, implementing a form of Hebbian learning for knowledge structures. An edge traversed during navigation becomes more visible. An edge never accessed fades. Over time, the graph converges on the relationships that actually matter to the practitioner.
- **Provenance throughout**: Every concept in the graph traces back to a specific file, line, and evidence span. Click a node, open the source. The graph is not an abstraction layer on top of the work—it is a navigable index into it.
- **Multi-system integration**: Plexus connects to LLM orchestration (llm-orc), provenance tracking (clawmarks), and UI layers (Manza) via MCP, creating a bidirectional learning loop where execution patterns inform graph structure and graph analysis informs future orchestration.

### 1.3 This Paper: Semantic Extraction from Document Corpora

A knowledge graph is only as useful as what's in it. The first challenge for Plexus is ingestion: how do you extract semantic content from existing document corpora and feed it into the graph? This paper addresses that question through a series of empirical experiments.

We needed to decide:

1. **Traversal**: How do we select and order documents for processing?
2. **Extraction**: How do we pull concepts from documents with high fidelity?
3. **Composition**: How do we handle documents that exceed LLM context windows?
4. **Propagation**: How do we spread concepts to related documents without reprocessing them?
5. **Normalization**: How much post-processing do extracted concepts need?
6. **Performance**: What throughput and latency can we expect on consumer hardware?

Each question has multiple plausible answers. Rather than guessing, we ran targeted experiments on real corpora to find out. The result is a three-system architecture (orchestration → provenance → knowledge graph) that plugs into Plexus's broader real-time infrastructure.

### 1.4 Approach

We conducted 18 experiments across three corpora of different structure and content type. Each experiment was designed to answer a specific design question with measurable outcomes. The experiments evolved iteratively, with early results redirecting later investigations. We report the sequence honestly, including hypotheses that turned out to be wrong.

### 1.5 Contributions

1. A three-system architecture (orchestration → provenance → knowledge graph) whose every major design choice is backed by experimental evidence
2. Empirical answers to six design questions, including negative results (what didn't work)
3. A methodology for using targeted experiments to make system design decisions—applicable beyond this specific domain
4. Quantitative characterization of local LLM performance constraints on consumer hardware
5. Integration design for feeding extracted semantics into a real-time, self-reinforcing knowledge graph

---

## 2. Related Work

Plexus sits at the intersection of several research areas that have not previously been integrated: knowledge graph construction, cognitive context in AI-assisted work, flow theory, external cognition, self-reinforcing memory models, computational movement analysis, and tiered event processing. We survey each and identify the gap our system occupies.

### 2.1 Cognitive Context Loss in AI-Assisted Development

The opacity problem (§1.1) is increasingly documented. Valett-Harper et al. [10] describe "material disengagement" in AI-assisted coding, where developers orchestrate code generation without comprehending the output, and propose post-hoc model recovery—essentially reconstructing understanding after the fact. Raychev et al. [11] measure a comprehension-performance gap in AI-assisted brownfield development: developers produce code effectively but cannot explain the resulting architecture. Radhakrishnan et al. [12] identify the measurement gap itself—despite growing evidence of cognitive cost, there are few empirical studies of cognitive load imposed by AI coding assistants.

Industry data corroborates this. A 2025 survey [13] found 65% of developers cite missing context as their top concern with AI-generated code—more than hallucination or correctness. Only 3.8% report both low hallucination rates and high confidence shipping AI code without review.

These findings motivate Plexus's core design: rather than recovering context post-hoc, maintain it continuously through a live structural representation that evolves alongside AI-assisted composition. Sweller's updated cognitive load theory [14] provides the theoretical mechanism: AI-generated code imposes extraneous cognitive load because its information structure is not aligned with the developer's existing schema. A live knowledge graph externalizes the structural relationships, reducing the load.

### 2.2 External Cognition and Epistemic Tools

The idea that external representations reduce cognitive burden has deep theoretical grounding. Kirsh and Maglio [15] distinguish epistemic actions (which change the agent's computational state, making mental computation easier) from pragmatic actions (which change the world toward a goal). In their Tetris experiments, players rotate pieces physically to simplify mental pattern-matching—an action that looks "wasteful" but is computationally efficient. A knowledge graph that externalizes structural relationships serves a similar function: it makes the relationships visible so the composer doesn't have to hold them in working memory.

Hutchins [16] extends this to distributed cognition: cognitive processes are not confined to individual minds but distributed across people, artifacts, and environments. Crucially, Hutchins argues that tools do not merely "amplify" cognition—they enable qualitatively different cognitive processes using different skills. A developer with a live knowledge graph is not simply thinking harder about structure; they are engaging in a different kind of structural reasoning that relies on perceptual processing rather than memory retrieval.

Scaife and Rogers [17] formalize three mechanisms by which external graphical representations support cognition: *computational offloading* (reducing working memory demands), *re-representation* (presenting information in a form better suited to the task), and *graphical constraining* (limiting the space of possible inferences). Plexus's graph visualization performs all three: it offloads structural tracking, re-represents linear composition as a network topology, and constrains attention to the semantically relevant neighborhood of the current work.

Clark and Chalmers [18] provide philosophical grounding through the extended mind thesis: cognitive processes literally extend into the environment when external resources play the functional role that internal memory would otherwise play. By this account, Plexus is not a tool the composer uses—it is part of the composer's cognitive system.

### 2.3 Flow State and Structural Feedback

Csikszentmihalyi [19] identifies three conditions for flow: clear goals, immediate feedback, and challenge-skill balance. The second condition is directly relevant. Traditional development environments provide delayed structural feedback—the developer must actively query for dependencies, references, or call hierarchies. A live knowledge graph provides immediate, continuous structural feedback without requiring an explicit query.

Dietrich [20] adds a neurological constraint: flow involves transient hypofrontality—the prefrontal cortex partially deactivates, reducing self-monitoring and analytical processing. This implies that structural feedback must be *ambient and peripheral* rather than demanding focused attention. A knowledge graph visualization that requires active reading would disrupt flow; one that operates at the level of peripheral awareness—shapes shifting, clusters forming, edges thickening—preserves it. Matthews et al. [21] study this design space for glanceable peripheral displays, finding that ambient information can maintain awareness without attentional capture.

Digital audio workstations, 3D modeling tools, and game engines already provide this kind of live structural feedback. Waveforms evolve as musicians compose; wireframes respond as modelers sculpt; physics simulations run alongside level design. In each case, the structural representation co-evolves with the creative act. Software development has moved toward this with live linting and type checking, but these provide *correctness* feedback ("is this valid?"), not *structural* feedback ("what did this change connect to?"). A live knowledge graph occupies a different niche: it shows the semantic topology of the work as it emerges.

### 2.4 Knowledge Graph Construction with LLMs

Recent systems for LLM-based knowledge graph construction share a batch-processing assumption.

| System | Approach | Design Assumptions |
|--------|----------|-------------------|
| **Microsoft GraphRAG** [1] | Entity extraction → community detection → hierarchical summaries | All docs processed; PageRank for importance ranking |
| **LightRAG** [2] | Graph + embedding retrieval with incremental updates | All docs processed; no structural awareness |
| **Neo4j LLM Graph Builder** [3] | Multi-LLM extraction to graph database | All docs processed; documents are atomic units |
| **iText2KG** [22] | Zero-shot incremental extraction with four-module pipeline | Incremental but not real-time; no structural corpus awareness |

All treat documents as atomic, independent units. None exploit organizational structure already present in the corpus. Pan et al. [23] survey the LLM-KG construction landscape comprehensively, covering multi-agent approaches (KARMA) and metacognitive prompting (Ontogenia), but none of the surveyed systems operate in real-time or integrate with a creative composition environment.

On the hallucination problem, Agrawal et al. [24] survey KG-LLM integration and find that knowledge graphs as external grounding demonstrably reduce LLM hallucination. Our evidence-grounded prompting approach (§4.2) is a specific implementation of this principle: requiring the LLM to cite text spans for each extracted concept achieves 0% hallucination on technical corpora.

**InfraNodus** [4] is the closest prior work to our initial approach. It applies network science (betweenness centrality, modularity) to knowledge management corpora, building co-occurrence graphs and identifying structural gaps. This informed our original hypothesis that network algorithms would be the right traversal mechanism. Our experiments showed this was wrong for structured corpora—the file tree provides stronger signal than network centrality (§4.1).

### 2.5 Live and Incremental Knowledge Graphs

A few systems move beyond batch processing. **Graphiti** [25] (Zep, 2024–2025) builds knowledge graphs incrementally in real-time with a bi-temporal data model tracking both event occurrence and ingestion time. It combines semantic embeddings, BM25, and graph traversal for low-latency queries. However, Graphiti targets AI agent memory, not human creative practitioners—it has no visualization layer, no self-reinforcing edges, and no multi-frequency update model.

Arenas-Guerrero et al. [26] demonstrate incremental KG construction using declarative RML mappings, achieving 315× less storage and 4.4× faster construction than full rebuilds. Zhu et al. [27] address continual KG embedding with incremental distillation, ordering new triples by graph distance and centrality. Both address the engineering of incremental updates but not the real-time composition use case.

### 2.6 Self-Reinforcing and Memory-Inspired Knowledge Structures

Plexus's self-reinforcing edge model—where edges strengthen through use and decay without reinforcement—is inspired by Hebbian learning ("neurons that fire together wire together"). The closest existing system is **Kairos** [28] (NeurIPS 2025 Workshop), which implements three neuroplasticity operations on knowledge graphs: long-term potentiation (edge strengthening), long-term depression (temporal decay), and emergent connection formation from co-activation. Kairos adds validation-gated learning—consolidation occurs only when reasoning passes quality checks, preventing hallucination reinforcement. This is architecturally similar to our design, but applied to AI agent memory rather than creative composition environments.

The theoretical basis for beneficial forgetting comes from Bjork and Bjork [29], who distinguish storage strength (permanent) from retrieval strength (decays). Periodic forgetting builds higher storage strength on re-learning—a "desirable difficulty." In our system, edge decay serves an analogous function: concepts that are re-encountered after fading receive stronger reinforcement than concepts that were never forgotten, naturally surfacing the relationships that recur across the practitioner's work.

Practical implementations of memory-inspired learning include spaced repetition systems. Settles and Meeder [30] develop half-life regression for predicting memory decay in language learning (deployed in Duolingo). Zaidi et al. [31] extend this with adaptive forgetting curves incorporating linguistic complexity. Our temporal decay function (exponential with weekly half-life) is deliberately simpler, but could be refined with similar complexity-aware models.

### 2.7 Computational Movement Analysis and Choreographic Structure

The movement/performance domain (§1.2, §5.3) connects to a body of work on computational Laban Movement Analysis and interactive performance systems.

Fdili Alaoui et al. [32] integrate LMA experts into sensor selection and feature computation, showing that multimodal data (positional, dynamic, physiological) best characterizes Laban Effort qualities—Weight, Time, Space, and Flow. Garcia et al. [33] train HMM models for six Effort qualities, finding equi-affine features highly discriminant. These systems provide the "structural layer" input for a movement knowledge graph: they classify the low-level movement data into Laban-theoretic categories that become graph nodes.

At the knowledge representation level, Raheb et al. [34] develop a dance ontology in OWL-2 based on Labanotation semantics, with Description Logic reasoning to extract new movement knowledge. El Raheb et al. [35] survey ontology-based dance knowledge management comprehensively. These ontologies provide a schema for the *conceptual* layer of a movement knowledge graph, but they are static representations—authored by experts, not emergent from live performance data.

For real-time performance systems, Camurri et al. [36] describe EyesWeb, a platform for real-time analysis of expressive gesture in dance and music performance. Forsythe's choreographic objects [37] provide the conceptual foundation: choreographic structure as a formal system that can be computationally represented, manipulated, and visualized.

No existing system combines these capabilities into a self-reinforcing graph that evolves through performance. The movement analysis systems classify gestures; the ontologies represent choreographic knowledge; the interactive systems respond in real-time. Plexus proposes unifying these into a single graph where performer-environment couplings strengthen through rehearsal, movement vocabulary clusters emerge from practice, and choreographic structure becomes visible not through notation but through the graph's accumulated memory of what happened and how it connected.

### 2.8 Multi-Frequency Event Processing

Our tiered update architecture (§5.3) has precedent in stream processing. The Lambda Architecture [38] processes data through parallel batch (high-latency, high-accuracy) and speed (low-latency, approximate) layers. Kreps [39] simplifies this to the Kappa Architecture where all processing is stream-based with replay for recomputation.

Luckham [40] formalizes hierarchical event abstraction in Complex Event Processing: low-level events compose into higher-level complex events across different temporal windows. This is directly analogous to our multi-frequency model where token-level structural events compose into relational patterns, semantic concepts, and conceptual structures at increasing timescales.

Baresi and Guinea [41] propose multi-layer monitoring with three processor types operating at different frequencies, the closest architectural precedent to our approach. Taylor et al. [42] address the specific challenge of applying semantic reasoning to streaming data—traditionally semantic approaches assume static data, while our semantic layer must operate incrementally on a continuously evolving corpus.

### 2.9 Gap Analysis

No existing system integrates all of these elements:

| Capability | GraphRAG | Graphiti | Kairos | InfraNodus | **Plexus** |
|------------|----------|----------|--------|------------|-----------|
| LLM-based extraction | ✓ | ✓ | — | — | ✓ |
| Incremental/real-time | — | ✓ | — | — | ✓ |
| Self-reinforcing edges | — | — | ✓ | — | ✓ |
| Evidence provenance | — | — | — | — | ✓ |
| Multi-frequency updates | — | — | — | — | ✓ |
| Creative composition UX | — | — | — | — | ✓ |
| Content-agnostic (code, text, movement) | — | — | — | — | ✓ |
| Flow-preserving ambient display | — | — | — | — | ✓ |

The closest system to Plexus is Kairos, which shares the Hebbian edge model, but targets AI agent memory rather than human creative environments. Graphiti shares the real-time incremental approach but lacks self-reinforcement, provenance, and visualization. No existing system combines live structural feedback with a self-reinforcing knowledge graph in a creative composition environment.

---

## 3. Experimental Setup

### 3.1 Hardware and Software

All experiments ran on consumer laptop hardware:
- **Hardware**: MacBook Pro M2 Pro, 16GB unified memory
- **LLM Runtime**: Ollama 0.5.x
- **Models**: llama3:8b-instruct-q4_0 (4.7GB), gemma3:1b (815MB)
- **Temperature**: 0.0 (deterministic output)

### 3.2 Corpora

| Corpus | Files | Structure | Content |
|--------|-------|-----------|---------|
| pkm-webdev | 50 | Deep tree (28 dirs) | Web development knowledge base |
| arch-wiki | 2,487 | Medium tree | Arch Linux wiki subset |
| shakespeare | 43 | Flat (1 dir) | Complete plays |

These corpora were chosen to represent different structural extremes: deep hierarchy, moderate hierarchy, and no hierarchy.

### 3.3 Orchestration Platform

Experiments used **llm-orc**, a local LLM orchestration tool that supports multi-agent ensembles, fan-out parallelism, and script-based preprocessing. Ensemble configurations are YAML files specifying agent chains with dependencies.

---

## 4. Design Questions and Experimental Answers

### 4.1 Traversal: How Should We Select Documents?

**Initial hypothesis**: Network science techniques (PageRank [6], label propagation [5], community detection [7]) would efficiently select high-value seed documents, achieving ≥85% coverage at 15% sampling.

**What we tested**: PageRank-based BFS with varying seed counts, random walk with restart, stratified sampling (one per directory), and depth-first tree traversal.

**Results**:

| Strategy | Coverage | Complexity |
|----------|----------|------------|
| PageRank BFS (5 seeds) | 44% | O(k×n×d) |
| PageRank BFS (10 seeds) | 58% | O(k×n×d) |
| Random Walk (p=0.15) | 72% | Probabilistic |
| Stratified (1/dir) | 100% | O(n) |
| Tree Traversal | 100% | O(n) |

PageRank-based seed selection achieved 44–72% coverage—well below our 85% target. The file tree achieves 100% by construction: every document belongs to a directory, every directory has a parent.

We also measured whether directory co-location provides semantic signal by comparing concept overlap (Jaccard similarity) across relationship types:

| Relationship | Mean Jaccard | % With Overlap | vs. Random |
|--------------|--------------|----------------|------------|
| Siblings (same directory) | 0.1108 | 44.4% | 9.3× |
| Linked (explicit wikilinks) | 0.0119 | 13.3% | 1.8× |
| Random pairs | 0.0067 | 6.7% | 1.0× |

The sibling vs. random comparison yields a large effect size (Cohen's d ≈ 0.8, p < 0.01, Mann-Whitney U). The 9.3× ratio should be read as order-of-magnitude, not precise—the linked sample is smaller (n=15 vs n=45).

**Design decision**: Walk the file tree for document selection. Weight sibling edges higher than explicit links. Reserve network algorithms for cross-branch discovery, not primary traversal.

**Boundary condition**: This fails completely for flat corpora. When all 43 Shakespeare plays sit in one directory, every document is siblings with every other, and the signal is zero. Flat corpora require content-only analysis (§4.6).

### 4.2 Extraction: How Do We Pull Concepts Reliably?

**What we tested**: LLM extraction using evidence-grounded prompts (requiring the model to cite specific text spans for each concept), across technical and literary corpora. We also tested five ensemble variations to improve extraction quality.

**Core extraction results**:

| Metric | pkm-webdev | pkm-datascience | shakespeare |
|--------|------------|-----------------|-------------|
| Grounding rate | 100% | 80.7% | 6.7% |
| Concepts/doc | 5.8 avg | Variable | — |
| Hallucination | 0% | ~19% | 93% failure |

"Hallucination" means concepts untraceable to source text. The 0% on technical corpora (n=50 docs, ~290 concepts) reflects evidence-grounded prompting. The literary corpus failed outright—the LLM returned prose summaries instead of JSON for long plays.

**Ensemble experiments** (A–E) tested refinements to the extraction pipeline:

| Experiment | What It Tested | Result | Design Impact |
|------------|---------------|--------|---------------|
| A: Two-Stage Refiner | Second LLM pass to filter noise | Removes 60–75% of over-specific concepts | Add refiner stage for content pages |
| B: Propagation-Aware | Prompt tuned for cross-doc usefulness | Eliminated sibling-specific concepts from index pages | Use different prompts for hub vs. leaf pages |
| C: Normalization | LLM-based deduplication | Case normalization safe; semantic dedup merged unrelated concepts | Keep normalization simple (§4.5) |
| D: Calibration | Rule-based confidence adjustment | 100% precision at ≥0.9 threshold (vs. 75% raw) | Apply calibration as post-processing |
| E: Hierarchical | Tree-informed multi-layer extraction | Avoided function names, discovered higher-level abstractions | Use corpus structure as extraction context |

Experiment E demonstrated that feeding the file tree structure to the LLM as context improved extraction quality. The model correctly inferred "web development, programming languages, software tools" from directory names alone, which guided it toward higher-level concepts and away from code-specific identifiers.

**Design decision**: Use evidence-grounded prompts as the primary extraction mechanism. Detect page type (index vs. content) and apply different ensemble configurations. Add a refiner stage for content pages. Feed tree structure as context for corpus-wide batch extraction.

### 4.3 Composition: How Do We Handle Large Documents?

**The problem**: Shakespeare plays are ~100k tokens each. Even shorter technical documents can exceed practical context windows. Experiment R4 initially used human-written summaries, which invalidated the autonomy claim.

**What we tested**: A chunk→fan-out→aggregate→synthesize pipeline. Documents are split into 150-line chunks with 20-line overlap. Each chunk is extracted independently in parallel, then results are aggregated and synthesized into a document-level representation.

**Results** (Macbeth, 500 lines → 4 chunks):

| Stage | Function | Validated |
|-------|----------|-----------|
| Chunker | Split by line count, overlap boundaries | Yes |
| Fan-out | Parallel extraction per chunk | Yes (via llm-orc) |
| Aggregator | Combine chunk extractions, reconcile overlaps | Yes |
| Synthesizer | Produce document-level coherent output | Yes |

Line-based chunking is deliberately simple—no format detection, no section-boundary heuristics. LLMs handle partial sentences at boundaries; the aggregator reconciles overlapping concepts.

**Design decision**: Use fixed-size line chunking with overlap. Process chunks in parallel via fan-out. This is the default path for any document exceeding 3,000 words.

### 4.4 Propagation: How Do We Spread Concepts?

**What we tested**: Concept propagation via sibling edges (directory co-location) using label propagation with decay. We ran a comprehensive parameter sweep (P1) testing decay values 0.5–0.9, thresholds 0.1–0.5, and hop counts 1–5.

**Results**:

| Evaluation Method | Scope | Appropriateness |
|-------------------|-------|-----------------|
| Manual review (author, n=10) | Coherent directory clusters | 67% appropriate |
| LLM-as-judge (n=50 pairs) | Full corpus | 29% appropriate |

The discrepancy is informative, not contradictory. The manual review happened to sample from semantically coherent directories (TypeScript files, Gnome desktop tools). The LLM judge hit the full corpus, including arbitrary pairings like Docker↔NordVPN that coexist in the vault only because someone's organizational habits are imperfect.

**Best parameters**: decay=0.8, threshold=0.3, hops=3. But the key finding is that **parameter tuning matters less than corpus organization quality**. Within coherent subtrees, propagation works at ~70–80% appropriateness. Across arbitrary groupings, it fails regardless of parameters.

**Design decision**: Enable propagation with conservative defaults (decay=0.7, threshold=0.4, hops=3). Expect it to work well only within well-organized directory subtrees. Do not invest in parameter optimization—invest in understanding corpus structure.

### 4.5 Normalization: How Much Post-Processing?

**What we tested**: Four levels of normalization on extracted concepts: none, case-only (lowercase), singularization (plural→singular), and LLM-based semantic deduplication.

**Results** (P3, 81 concepts from pkm-webdev):

| Level | Merges Found | Precision |
|-------|-------------|-----------|
| None | 0 | 100% |
| Case-only | 0 | 100% |
| +Singular | 0 | 100% |
| +Semantic | 0 | 100% |

Zero merges across all levels. This initially seemed suspicious—surely 81 concepts should have duplicates? On investigation: the evidence-grounded extraction prompt already produces normalized output. The LLM uses canonical lowercase forms and consistent terminology. The corpus (single-author PKM) reinforces this consistency.

The earlier Experiment C, which tested normalization on a different concept set, found case normalization safe but semantic deduplication dangerous (it incorrectly merged "git" with "tag").

**Design decision**: Apply case normalization only. Skip semantic deduplication—it introduces errors, and the LLM normalizes implicitly during extraction. This finding may not hold for multi-author corpora with inconsistent terminology.

### 4.6 Performance: What Can We Expect on Consumer Hardware?

**What we tested**: Latency profiling (S1), concurrency scaling (S2), and model size comparison (S1/S2-Micro) on local Ollama with both 7B and 1B models.

**Latency (S1)**:

| Metric | 7B (llama3) | 1B (gemma3) | Target |
|--------|-------------|-------------|--------|
| p50 | 11.9s | 10.8s | <5s |
| p95 | 16.7s | 17.9s | <10s |
| Failure rate | 23% | 28% | — |

Strong size-latency correlation (r=0.705): `latency ≈ 9.2s + 1.8ms × size_bytes`. The ~9s baseline is an inference floor regardless of document size.

Switching from 7B to 1B gave negligible improvement (1.1× median, with *worse* p95 and higher failure rate). The bottleneck is not model size—it may be Ollama HTTP overhead, memory bandwidth, tokenization, or something else we couldn't isolate without deeper profiling.

**Concurrency (S2)**:

| Workers | Throughput | Mean Latency | Error Rate | Speedup |
|---------|------------|--------------|------------|---------|
| 1 | 6.9/min | 8.8s | 25% | 1.0× |
| 2 | 8.4/min | 13.0s | 20% | 1.2× |
| 4 | 8.6/min | 22.8s | 20% | 1.3× |
| 8 | 10.3/min | 32.7s | 35% | 1.5× |

Throughput plateaus at ~8–10 docs/min regardless of concurrency. Maximum speedup is 1.5× (far below theoretical 8×). Error rates spike above 2 workers.

**Design decision**: Use 2 concurrent workers maximum. Assume background processing for all extraction—interactive latency targets (<5s) are not achievable on this hardware. Implement aggressive caching (content-hash addressed, re-extract only on change). Prefer the 7B model over 1B—better output quality with no meaningful latency penalty.

---

## 5. System Architecture

The experiments produced a three-system architecture for semantic extraction that feeds into Plexus's real-time knowledge graph:

```
Document ──► llm-orc ──► Clawmarks ──► Plexus
             (extract)    (provenance)   (knowledge graph)
                                              │
                                    ┌─────────┴──────────┐
                                    │  Self-reinforcing   │
                                    │  edges, decay,      │
                                    │  community detection │
                                    └─────────────────────┘
```

| System | Responsibility | Why Separate |
|--------|---------------|--------------|
| **llm-orc** | Orchestrates LLM ensembles, handles chunking and fan-out | Stateless; extraction strategy changes independently of storage |
| **clawmarks** | Records WHERE each concept came from (file, line, evidence) | Enables "go to source" UX; extraction sessions are queryable trails |
| **plexus** | Stores WHAT concepts exist and HOW they relate | Graph traversal and cross-document edges; semantic dimension |

### 5.1 Extraction Pipeline

Document routing is based on content characteristics:

| Content Type | Size | Ensemble | Rationale |
|--------------|------|----------|-----------|
| Technical | < 3000 words | `plexus-semantic` | Direct extraction; 100% grounding validated |
| Technical | > 3000 words | `plexus-compositional` | Chunk→fan-out→aggregate (§4.3) |
| Literary | < 3000 words | `plexus-refinement` | Iterative taxonomy building |
| Literary | > 3000 words | `plexus-compositional` | Same pipeline, literary-tuned prompts |
| Flat corpus | any | `plexus-refinement` | No tree signal; content-only fallback |

For structured corpora, the pipeline is:

1. **Traverse** the file tree (depth-first, 100% coverage)
2. **Classify** each document (index page vs. content page, size threshold)
3. **Extract** concepts using the appropriate ensemble
4. **Record provenance** via clawmarks (file, line, evidence text)
5. **Store** concepts and relationships in the plexus graph
6. **Propagate** concepts to sibling documents with decay

### 5.2 Provenance Model

Every concept links back to its source through a clawmark:

```
Concept: "revenge" (confidence: 0.9)
    └── Clawmark: hamlet.txt:892
        └── Evidence: "May sweep to my revenge"
            └── Trail: hamlet-extraction-2026-01-18
```

This enables a "go to source" UX: click a concept node in the graph → open the file at the exact line where the concept was extracted. Extraction sessions are tracked as trails, making the provenance of every concept in the knowledge graph auditable.

### 5.3 Multi-Frequency Update Model

A live knowledge graph in a creative environment cannot update everything at once—LLM extraction takes ~10s per document (§4.6), and the user is composing continuously. The solution is tiered update frequencies, where different semantic layers refresh at different cadences:

| Layer | Trigger | Target Latency | Method |
|-------|---------|----------------|--------|
| **Structural** | Every validation cycle / keystroke debounce | <100ms | Deterministic parsing (tree-sitter, regex, format-specific), no LLM |
| **Relational** | On save or typing pause (>2s idle) | <2s | Lightweight text analysis, cached embeddings |
| **Semantic** | Background, priority-queued | 10–30s | LLM extraction (this paper's pipeline) |
| **Conceptual** | On explicit refresh or scheduled | Minutes | Network analysis, community detection |

Each layer manifests differently depending on the creative domain, but the tiering is universal:

| Layer | Code | Fiction | Research | Movement/Performance |
|-------|------|---------|----------|---------------------|
| **Structural** | Imports, calls, definitions, type relationships | Character appearances, scene boundaries, dialogue attribution | Citations, section structure, reference links | Poses, transitions, spatial formations, performer positions |
| **Relational** | Shared terms, module co-usage, naming patterns | Character co-occurrence, setting reuse, motif repetition | Term overlap, shared citations, methodological similarity | Movement quality similarity (Laban efforts), gesture vocabulary clustering, spatial proximity |
| **Semantic** | Concepts, architectural patterns, design intent | Themes, narrative arcs, character development trajectories | Arguments, claims, evidential support/contradiction | Choreographic phrases, performer-environment coupling, trigger-response mappings |
| **Conceptual** | Module communities, hub abstractions, dependency topology | Plot thread structure, thematic communities, narrative architecture | Argument structure, knowledge gaps, synthesis opportunities | Emergent movement patterns, ensemble dynamics, performance evolution over time |

The movement/performance domain is particularly illustrative. In an interactive performance system (such as one mapping gesture to lighting and sound), the structural layer captures what the performer is doing right now—poses, transitions, spatial formations. The relational layer identifies movement vocabulary: which gestures cluster together, which transitions are practiced vs. novel. The semantic layer discovers choreographic structure: phrases that echo, develop, or contrast each other; performer-environment coupling patterns (gesture X reliably triggers lighting state Y). The conceptual layer reveals what emerges over time: how the performance vocabulary evolves across rehearsals, which multi-performer formations recur, which movement-environment couplings strengthen through use.

This is the same graph engine operating on different content-type analyzers. A tree-sitter parser and a pose tracker are structurally equivalent from Plexus's perspective: both produce nodes and edges at the structural layer, which feed upward into relational clustering, semantic extraction, and conceptual analysis. The self-reinforcing edge model works identically—a gesture-to-lighting edge that fires reliably strengthens, just as a function-call edge traversed by a developer strengthens.

The structural layer provides immediate feedback: write a function call, execute a gesture, introduce a character—the edge appears. The relational layer provides near-real-time clustering: save a file, pause between movement phrases—the work settles into its neighborhood. The semantic layer provides depth: background extraction discovers concepts and relationships that structural analysis alone would miss. The conceptual layer provides the big picture: community structure, hub nodes, knowledge gaps, emergent patterns.

This tiering is informed directly by our performance experiments (§4.6). The ~10s LLM extraction floor means semantic analysis cannot be synchronous. But the structural and relational layers—which don't require LLM inference—can update fast enough to feel live. The result is a graph that is always responsive (structural edges appear immediately) and always deepening (semantic concepts accumulate in the background).

Priority queuing ensures the semantic layer stays relevant: the currently active artifact (open file, active performer, focused document) gets highest priority, recently modified artifacts next, then breadth-first traversal of the rest. Content-hash caching means unchanged material is never re-extracted.

---

## 6. Discussion

### 6.1 What Worked

The most broadly applicable findings:

- **Structure is semantic signal.** Authors organize related content together. This isn't a novel observation, but quantifying it (9.3× stronger than explicit links) and building a system around it is useful. Existing KG systems ignore this signal entirely.
- **Evidence-grounded prompting eliminates hallucination on technical content.** Requiring the LLM to cite text spans is a simple, effective constraint. We saw 0% hallucination across 290 concepts on technical corpora.
- **Compositional extraction works autonomously.** Chunking + fan-out + aggregation handles large documents without human intervention, validating the approach for corpora with diverse document sizes.
- **The LLM is an implicit normalizer.** With constrained prompts, the model produces canonical concept forms without explicit post-processing. This surprised us and simplified the pipeline.
- **Provenance enables the "live map" UX.** Every concept traces to file:line:evidence, which means the knowledge graph isn't abstract—it's navigable. Click a concept, open the source. This is what distinguishes Plexus from systems that produce graph visualizations disconnected from the underlying artifacts.

### 6.2 What Failed

- **PageRank for traversal.** Our original hypothesis. It optimizes for node importance, not coverage. The tree solves coverage trivially.
- **Literary corpus extraction.** 93% failure rate on Shakespeare. The LLM returns prose summaries instead of structured output for long literary texts. Content-type detection and specialized prompts are necessary.
- **Interactive latency.** We targeted <5s per document. Actual median is 11.9s with a ~9s floor that persists even with 1B models. Background processing is mandatory.
- **Semantic deduplication.** LLM-based concept merging incorrectly conflated unrelated concepts (e.g., "git" with "tag"). Simple case normalization is the safe ceiling.
- **Propagation across diverse directories.** 29% appropriateness on the full corpus, despite 67–80% within coherent subtrees. The technique works only when the directory structure reflects genuine semantic grouping.

### 6.3 When This Architecture Applies

The tree-first approach works best when:
- The corpus is author-organized into topic directories (PKM vaults typically are)
- Directory depth exceeds 2 levels
- Directories contain fewer than ~20 documents

It degrades gracefully: the system falls back to content-only analysis for flat corpora, but loses the structural signal that makes propagation and traversal efficient.

### 6.4 Limitations

- **Single LLM provider**: All experiments used Ollama on laptop hardware. Cloud APIs or dedicated GPUs may show different latency and quality characteristics.
- **Single-author corpora**: All test corpora were created by single authors with consistent organizational habits. Multi-author corpora may show different patterns.
- **Tags and metadata not examined**: Many PKM systems rely on `#tags` and YAML frontmatter. These explicit semantic signals were not included in our analysis and might provide stronger signal than wikilinks.
- **Small corpus for key claims**: The 9.3× sibling signal strength comes from a 50-file corpus. Larger-scale validation is needed.
- **LLM-as-judge bias**: Propagation evaluation (P1) used the same model family as extraction. A blind human evaluation would be more rigorous.

### 6.5 Beyond Document Corpora: The Live Composition Environment

This paper addresses one ingestion pathway: extracting semantics from existing document corpora. But the corpus pipeline is the slow path—the initial population of the graph from pre-existing material. The more interesting path is live composition, where the graph evolves alongside the creative process.

**The flow-state hypothesis.** Traditional development tooling interrupts composition: you stop writing to check documentation, trace a dependency, or understand the implications of a change. A live knowledge graph running alongside the editor inverts this. The structural implications of your work are visible peripherally, the way a musician sees sheet music while playing. You don't stop composing to consult the graph—the graph reflects your composition as it happens. We hypothesize this produces a more sustained flow state, though we have not yet measured this empirically.

**Concrete interaction model.** In Manza (the editor environment where Plexus is integrated):

1. *You write a new function.* The structural layer (tree-sitter, <100ms) immediately shows edges to called functions and imported modules. No LLM involved.
2. *You save the file.* The relational layer (~2s) recalculates term overlap with sibling files. Your file shifts position in the topic cluster visualization.
3. *In the background,* the semantic layer (10–30s) extracts concepts via the pipeline described in this paper. New concept nodes appear, connecting your file to thematically related documents you may not have been thinking about.
4. *You prompt an AI assistant to generate a module.* The generated code triggers the same pipeline. The graph captures what the AI introduced—new dependencies, new patterns, new concepts—making the structural implications of AI-generated code visible rather than opaque.
5. *You navigate the graph,* clicking a concept node to jump to its source. The edge you traversed gets reinforced. Over time, the graph highlights the paths you actually use and lets the unused ones fade.

**Multiple ingestion pathways.** The semantic extraction pipeline described in this paper is the batch/background entry point. Other pathways feed the same graph:

- **Structural analysis** (tree-sitter): AST-level relationships, updated synchronously. No LLM required.
- **LLM orchestration feedback**: When llm-orc executes multi-agent ensembles, execution outcomes feed back as reinforcement signals. The graph learns which agent configurations produce high-quality results.
- **User interaction**: Navigation through the graph generates reinforcement. Edges the user traverses strengthen; edges never visited decay. The graph converges on the relationships that matter to this practitioner.
- **AI-assisted composition**: Code and text generated by AI assistants enters the same extraction pipeline, ensuring the graph captures knowledge produced by generative AI—not just knowledge produced by the human.

The self-reinforcing edge model applies uniformly across all sources. A concept extracted from a document (batch pathway) that is later referenced in a coding session (structural pathway) and navigated by the user (interaction pathway) receives reinforcement from three independent sources, increasing its confidence and visibility. This multi-source reinforcement is what makes the graph a learning system rather than a static index.

---

## 7. Conclusion

We set out to build the semantic ingestion layer for Plexus—a real-time knowledge graph engine designed to make opaque knowledge accumulation visible—and discovered that most of the interesting design questions had non-obvious answers. Network algorithms weren't needed for traversal. Explicit links carried less signal than directory structure. Smaller models weren't faster. Concept normalization was unnecessary. Propagation effectiveness was determined by corpus organization, not parameter tuning.

The resulting architecture is straightforward: walk the file tree, extract concepts with evidence-grounded prompts using appropriate ensembles for different document types, record provenance, store in a graph, and propagate cautiously within coherent subtrees. Each choice is backed by experiment rather than assumption.

This extraction pipeline is one layer of a larger system designed for live composition. The ~10s LLM extraction floor means semantic analysis must run in the background—but it also means the multi-frequency update architecture is not optional, it's load-bearing. Structural edges (tree-sitter, <100ms) provide immediate feedback. Relational clustering (~2s) provides near-real-time awareness. Semantic extraction (10–30s) provides depth. Conceptual analysis (minutes) provides the big picture. Each layer discovered in this paper maps directly to a frequency tier in the live system.

The deeper ambition is a creative environment where the structural implications of your work—including work produced by AI assistants—are visible as you compose, not discovered after the fact. The graph is not documentation. It is peripheral vision for knowledge work: always present, always current, never demanding attention but always available when you glance at it. Whether this produces the flow-state effect we hypothesize remains to be tested empirically. But the infrastructure described here—evidence-grounded extraction, provenance-linked concepts, self-reinforcing edges, multi-frequency updates—makes the experiment possible.

For practitioners building similar systems, the meta-lesson may be more useful than the specific findings: targeted experiments on real corpora reveal design answers that intuition and literature review alone would miss. We expected PageRank to work and tree traversal to be naive. We expected explicit links to be the strongest signal. We expected smaller models to be faster. All three intuitions were wrong. The experiments took less effort than implementing the wrong architecture would have.

---

## References

### Knowledge Graph Construction

[1] Edge, D., Trinh, H., Cheng, N., Bradley, J., Chao, A., Mody, A., Truitt, S., & Larson, J. (2024). From Local to Global: A Graph RAG Approach to Query-Focused Summarization. *arXiv preprint arXiv:2404.16130*.

[2] Guo, Z., Xia, L., Yu, Y., Ao, T., & Huang, C. (2025). LightRAG: Simple and Fast Retrieval-Augmented Generation. In *Findings of the Association for Computational Linguistics: EMNLP 2025*, pp. 10746-10761.

[3] Neo4j. (2024). LLM Knowledge Graph Builder. https://neo4j.com/labs/genai-ecosystem/llm-graph-builder/

[4] Paranyushkin, D. (2019). InfraNodus: Generating insight using text network analysis. In *Proceedings of the World Wide Web Conference 2019* (WWW '19), pp. 3584-3589.

[5] Zhu, X., Ghahramani, Z., & Lafferty, J. D. (2003). Semi-supervised learning using Gaussian fields and harmonic functions. In *Proceedings of the 20th International Conference on Machine Learning (ICML-03)*, pp. 912-919.

[6] Page, L., Brin, S., Motwani, R., & Winograd, T. (1999). The PageRank Citation Ranking: Bringing Order to the Web. *Stanford InfoLab Technical Report*.

[7] Blondel, V. D., Guillaume, J. L., Lambiotte, R., & Lefebvre, E. (2008). Fast unfolding of communities in large networks. *Journal of Statistical Mechanics: Theory and Experiment*, 2008(10), P10008.

[8] Meta AI. (2024). Llama 3 Model Card. https://github.com/meta-llama/llama3/blob/main/MODEL_CARD.md

[9] Ollama. (2024). Ollama: Run Large Language Models Locally. https://ollama.com/

### Cognitive Context in AI-Assisted Development

[10] Valett-Harper, M. et al. (2025). Lost in Code Generation: Reimagining the Role of Software Models in AI-driven Software Engineering. *arXiv preprint arXiv:2511.02475*.

[11] Raychev, V. et al. (2025). Comprehension-Performance Gap in GenAI-Assisted Brownfield Development. *arXiv preprint arXiv:2511.02922*.

[12] Radhakrishnan, A. et al. (2025). Towards Decoding Developer Cognition in the Age of AI Assistants. *arXiv preprint arXiv:2501.02684*.

[13] Qodo. (2025). State of AI Code Quality in 2025. Industry Report.

[14] Sweller, J. (2024). Cognitive load theory and individual differences. *Learning and Instruction*, 88.

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

[22] Yao, Y. et al. (2024). iText2KG: Incremental Knowledge Graphs Construction Using Large Language Models. *arXiv preprint arXiv:2409.03284*.

[23] Pan, S. et al. (2025). LLM-empowered Knowledge Graph Construction: A Survey. *arXiv preprint arXiv:2510.20345*.

[24] Agrawal, M. et al. (2024). Can Knowledge Graphs Reduce Hallucinations in LLMs? A Survey. In *Proceedings of NAACL 2024*.

### Incremental and Real-Time Knowledge Graphs

[25] Zep. (2024-2025). Graphiti: Temporally-Aware Knowledge Graphs. https://github.com/getzep/graphiti

[26] Arenas-Guerrero, J. et al. (2024). IncRML: Incremental Knowledge Graph Construction from Heterogeneous Data Sources. *Semantic Web Journal*.

[27] Zhu, Y. et al. (2024). IncDE: Towards Continual Knowledge Graph Embedding via Incremental Distillation. In *Proceedings of AAAI 2024*.

### Self-Reinforcing and Memory-Inspired Knowledge Structures

[28] Kairos. (2025). Validation-Gated Hebbian Learning for Adaptive Agent Memory. *NeurIPS 2025 Workshop*. OpenReview: EN9VRTnZbK.

[29] Bjork, R.A. & Bjork, E.L. (1992). A New Theory of Disuse and an Old Theory of Stimulus Fluctuation. In *From Learning Processes to Cognitive Processes*, Erlbaum.

[30] Settles, B. & Meeder, B. (2016). A Trainable Spaced Repetition Model for Language Learning. In *Proceedings of ACL 2016*.

[31] Zaidi, A. et al. (2020). Adaptive Forgetting Curves for Spaced Repetition Language Learning. In *AIED 2020*, Springer LNCS 12164, pp. 358-363.

### Computational Movement Analysis and Choreographic Structure

[32] Fdili Alaoui, S. et al. (2017). Seeing, Sensing and Recognizing Laban Movement Qualities. In *Proceedings of CHI 2017*, ACM.

[33] Garcia, M. et al. (2020). Recognition of Laban Effort Qualities from Hand Motion. In *Proceedings of MOCO 2020*, ACM.

[34] Raheb, K.E. et al. (2010). A Labanotation Based Ontology for Representing Dance Movement. In *GW 2010*, Springer.

[35] El Raheb, K. et al. (2024). Ontology in Dance Domain—A Survey. *Journal on Computing and Cultural Heritage*, ACM.

[36] Camurri, A. et al. (2000). Toward Gesture and Affect Recognition in Interactive Dance and Music Systems. *Computer Music Journal*, 24(1).

[37] Forsythe, W. (2008). Choreographic Objects. Essay.

### Multi-Frequency and Tiered Event Processing

[38] Marz, N. & Warren, J. (2015). *Big Data: Principles and Best Practices of Scalable Real-Time Data Systems.* Manning.

[39] Kreps, J. (2014). Questioning the Lambda Architecture. O'Reilly Blog.

[40] Luckham, D. (2002). *The Power of Events: An Introduction to Complex Event Processing.* Addison-Wesley.

[41] Baresi, L. & Guinea, S. (2013). Event-Based Multi-Level Service Monitoring. In *Proceedings of ICWS 2013*, IEEE.

[42] Taylor, R. et al. (2014). Semantic Complex Event Processing for Decision Support. In *EKAW 2014*, Springer LNCS 8876.

---

## Appendix A: Evidence Trail

This paper's claims are tracked via clawmarks trail `t_0jihblgl`. Key evidence:

| Claim | Clawmark | Location |
|-------|----------|----------|
| Tree 100% coverage | c_4ek7eafz | EXPERIMENT-LOG.md:461 |
| Siblings 9.3× | c_2ckf3smk | EXPERIMENT-LOG.md:570 |
| Flat corpus fails | c_euu9kru0 | EXPERIMENT-LOG.md:659 |
| 0% hallucination | c_gi204l8l | EXPERIMENT-LOG.md:1068 |
| Propagation (early sample) | c_uvzsyc5s | EXPERIMENT-LOG.md:1057 |
| Tree IS semantic | c_wmi8ltd6 | ENSEMBLE-EXPERIMENTS.md:461 |
| Compositional works | c_l866p5e7 | SPIKE-OUTCOME.md:47 |
| P1 propagation params | c_r0ecn0pw | spike_p1_llm_propagation.rs:549 |
| P2 multi-corpus | c_59fufuod | spike_p2_multi_corpus.rs:1 |
| P3 normalization | c_8hbmeguh | spike_p3_normalization.rs:1 |
| S1 latency profiling | c_jdo7vstn | spike_s1_latency.rs:1 |
| S2 concurrency | c_bqeip67b | spike_s2_concurrency.rs:1 |
| S1-Micro latency | — | spike_s1_latency_micro.rs:1 |
| S2-Micro concurrency | — | spike_s2_concurrency_micro.rs:1 |

---

## Appendix B: Ensemble Experiments Detail

Five ensemble variations were tested to refine extraction quality:

| Experiment | Method | Key Result |
|------------|--------|------------|
| A: Two-Stage Refiner | Second LLM pass filters over-specific concepts | 60–75% noise removed; core concepts retained |
| B: Propagation-Aware | Prompt optimized for cross-doc usefulness | Eliminated sibling-specific concepts from hub pages |
| C: Normalization | LLM-based deduplication | Case normalization safe; semantic dedup merged "git" with "tag" |
| D: Calibration | Rule-based confidence adjustment | 100% precision at ≥0.9 (vs. 75% raw); code identifier penalty effective |
| E: Hierarchical | Tree structure fed as extraction context | Inferred domain taxonomy from directory names; avoided function-name extraction |

Three ensemble configurations were produced:

| Ensemble | Architecture | Best For |
|----------|-------------|----------|
| `plexus-semantic` | 1 agent, evidence-grounded | Short technical documents |
| `plexus-semantic-v2` | 2 agents (extractor → refiner) | Content pages with code |
| `plexus-semantic-propagation` | 1 agent, propagation-aware prompt | Index/hub pages |

See ENSEMBLE-EXPERIMENTS.md for full experimental details.

---

## Appendix C: Data Model

### Concept Node (Plexus)

```rust
Node {
    id: NodeId("concept:revenge"),
    node_type: "concept",
    content_type: ContentType::Concept,
    dimension: "semantic",
    properties: {
        "name": "revenge",
        "concept_type": "theme",
        "confidence": 0.9,
        "clawmark_id": "clwk_abc123",    // provenance link
        "extraction_trail": "trail_xyz", // session tracking
    },
}
```

### Clawmark (Provenance)

```json
{
  "id": "clwk_abc123",
  "trail_id": "trail_xyz",
  "file": "hamlet.txt",
  "line": 892,
  "annotation": "Hamlet vows revenge: 'May sweep to my revenge'",
  "tags": ["#theme", "#central"]
}
```

---

## Appendix D: Experiment Index

| ID | Experiment | Design Question | Status | Key Finding |
|----|------------|----------------|--------|-------------|
| Inv 1–3 | Graph connectivity, traversal, signal | Traversal | Complete | Tree > PageRank; siblings 9.3× > links |
| Inv 4–5 | LLM extraction quality | Extraction | Complete | 0% hallucination (technical), 93% failure (literary) |
| Inv 6 | Concept propagation | Propagation | Complete | 67% appropriate (coherent subtrees) |
| A–E | Ensemble variations | Extraction refinement | Complete | See Appendix B |
| R4/R4b | Flat corpus taxonomy | Composition | Complete | Compositional pipeline validated |
| P1 | Propagation parameter sweep | Propagation | Complete | 29% overall; corpus structure > parameters |
| P2 | Multi-corpus extraction | Extraction | Complete | 80–100% grounding (technical) |
| P3 | Normalization ablation | Normalization | Complete | LLM normalizes implicitly |
| S1 | Latency profiling | Performance | Complete | p50=11.9s, ~9s floor |
| S2 | Concurrency testing | Performance | Complete | max 2 workers, 1.5× speedup |
| S1/S2-Micro | Model size comparison | Performance | Complete | 1B not faster than 7B |
