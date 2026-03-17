# Gold Standard Extractions — Essay 24

Claude-produced reference extractions for each test corpus essay.
These serve as the baseline against which SLM outputs are measured.

**Correction:** "zone of proximal development" was originally included for Essay 02
but does not appear in the text (it is in Essay 04). Removed from this gold standard.
All recall figures in Essay 24 use 13 entities for Essay 02.

---

## Essay 02 — The Opacity Problem

Source: `docs/essays/02-opacity-problem.md` (117 lines)

### Entities (13)
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
- AI-assisted composition
- computational offloading
- re-representation

### Relationships (10)
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

### Themes (5)
- knowledge accumulates faster than understanding
- structural awareness vs. generative flow (tension)
- tools should externalize awareness, not reasoning
- domain-general nature of opacity (not AI-specific)
- cognitive science grounding for design decisions

---

## Essay 12 — Provenance as Epistemological Infrastructure

Source: `docs/essays/12-provenance-as-epistemological-infrastructure.md` (104 lines)

### Entities (13)
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

### Relationships (8)
- provenance --provides--> epistemological foundation for semantics
- chain --contains--> mark
- mark --references--> concept (via TagConceptBridger)
- adapter --produces--> provenance alongside semantics (dual obligation)
- fragment --has--> provenance mark
- Hebbian contribution --tracks--> per-adapter evidence
- multi-phase processing --uses--> separate chains per phase
- TagConceptBridger --bridges--> provenance to semantic dimension

### Themes (5)
- epistemology vs. bookkeeping distinction
- provenance must live at adapter level (domain knowledge required)
- graph should be both ontological and epistemological
- existing designs anticipate future use cases without knowing it
- independent verification strengthens confidence
