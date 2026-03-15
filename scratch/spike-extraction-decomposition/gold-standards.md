# Gold Standard Extractions

Claude-produced reference extractions for each test corpus sample.
These serve as the baseline against which SLM outputs are measured.

---

## Sample 1: Essay 02 — The Opacity Problem

### Entities/Concepts
- opacity problem
- cognitive load
- working memory
- external structural representation
- situation awareness
- epistemic action
- cognitive offloading
- vibe-coding
- material disengagement
- knowledge graph
- zone of proximal development
- AI-assisted composition
- computational offloading
- re-representation

### Relationships
- opacity problem --caused_by--> cognitive load exceeding working memory
- external structural representation --remedies--> opacity problem
- vibe-coding --exemplifies--> opacity problem
- material disengagement --describes--> vibe-coding phenomenon
- cognitive offloading --mechanism_of--> external structural representation
- epistemic action --distinct_from--> pragmatic action
- situation awareness --eroded_by--> opacity problem
- AI-assisted composition --accelerates--> opacity problem
- knowledge graph --instance_of--> external structural representation
- computational offloading --component_of--> cognitive offloading

### Themes
- knowledge accumulates faster than understanding
- structural awareness vs. generative flow (tension)
- tools should externalize awareness, not reasoning
- domain-general nature of opacity (not AI-specific)
- cognitive science grounding for design decisions

---

## Sample 2: Essay 12 — Provenance as Epistemological Infrastructure

### Entities/Concepts
- provenance
- epistemology
- chain (provenance)
- mark (provenance)
- fragment
- TagConceptBridger
- Hebbian contribution
- dual obligation (Invariant 7)
- adapter
- enrichment
- multi-phase processing
- deterministic ID
- cross-dimensional traversal

### Relationships
- provenance --provides--> epistemological foundation for semantics
- chain --contains--> mark
- mark --references--> concept (via TagConceptBridger)
- adapter --produces--> provenance alongside semantics (dual obligation)
- fragment --has--> provenance mark
- Hebbian contribution --tracks--> per-adapter evidence
- multi-phase processing --uses--> separate chains per phase
- TagConceptBridger --bridges--> provenance to semantic dimension

### Themes
- epistemology vs. bookkeeping distinction
- provenance must live at adapter level (domain knowledge required)
- graph should be both ontological and epistemological
- existing designs anticipate future use cases without knowing it
- independent verification strengthens confidence

---

## Sample 3: Essay 18 — Phased Extraction Architecture

### Entities/Concepts
- phased extraction
- extraction coordinator
- Phase 1 (file info)
- Phase 2 (metadata)
- Phase 3 (heuristic)
- Phase 4 (semantic/LLM)
- declarative adapter spec
- llm-orc
- fan-out
- enrichment tiers (0, 1, 2)
- parameterized enrichment
- CoOccurrenceEnrichment
- TagConceptBridger
- batch graph analysis
- PageRank
- community detection
- progressive extraction
- contribution tracking

### Relationships
- Phase 1 --precedes--> Phase 2 --precedes--> Phase 3 --precedes--> Phase 4
- cheap phases --should_not_wait_for--> expensive phases
- Phase 4 --delegates_to--> llm-orc
- declarative adapter spec --maps--> JSON to graph mutations
- enrichment tier 0 --parameterizes--> existing enrichments
- enrichment tier 2 --runs_via--> llm-orc ensemble (batch)
- contribution tracking --handles--> multi-phase evidence naturally
- llm-orc --is_more_than--> LLM orchestrator (general DAG executor)

### Themes
- cheap-first is universal across systems
- progressive value: graph useful from first phase
- declarative specs: YAML not Rust for new domains
- two-layer architecture: extractor + mapper
- llm-orc as general computation DAG, not just LLM orchestrator
- graceful degradation when expensive phases unavailable

---

## Sample 4: Essay 04 — Trellis

### Entities/Concepts
- Trellis
- scaffolding (not generation)
- fragment
- coaching prompt
- mirror not oracle principle
- non-interpretation constraint
- seed promotion
- dormancy reinforcement
- thematic recurrence
- zone of proximal development
- creative ownership
- knowledge graph (Plexus)
- self-reinforcing dynamics
- sketch weight

### Relationships
- Trellis --built_on--> Plexus knowledge graph
- scaffolding --preserves--> creative ownership (unlike generation)
- fragment --connected_by--> structural, relational, semantic edges
- coaching prompt --surfaces--> graph connections to writer
- mirror not oracle --constrains--> what Trellis communicates
- active sorting --strengthens--> fragment edges
- seed promotion --strengthens--> fragment edges
- dormancy reinforcement --inspired_by--> desirable difficulty (Bjork)
- thematic recurrence --detected_across--> sessions over time

### Themes
- generation helps volume but hurts ownership
- non-interpretation as design constraint (fragile, prompt-dependent)
- structural awareness between sessions, not during composition
- graph as infrastructure not interface
- writer validation signals weaker than code domain signals
