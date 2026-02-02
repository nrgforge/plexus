# ADR-000: Research-Driven Development Workflow

**Status:** Accepted

**Date:** 2026-02-02

---

## Context

Plexus is a research-informed system — its design draws on Hebbian dynamics, knowledge graph theory, Laban movement analysis, and other domains where decisions should be grounded in existing literature and proven approaches rather than invented from scratch.

We need a development workflow that:

- Produces publishable artifacts at each stage, not just code
- Grounds design decisions in research (open source implementations, academic literature)
- Arrives at precise specifications before writing implementation code
- Creates a traceable path from research question to running software
- Supports reflection and communication through essays

---

## Decision

Development follows a six-phase flow. Each phase produces a concrete artifact. The default direction of travel is forward, but any phase can loop back to an earlier one when new information demands it.

### Phase 1: Research

Investigate the problem space through web research, academic literature, and open source implementations. The goal is to understand what already exists, what approaches are proven, and what trade-offs others have encountered.

**Artifact:** Research notes, literature references, annotated links.

**Exit criteria:** Enough understanding to reason about design trade-offs.

### Phase 2: Design

Produce design documents and exploratory essays. Design docs capture system architecture, trait definitions, data models, and interaction diagrams. Essays explore the *why* — reasoning through trade-offs, comparing approaches, articulating principles.

Design docs and essays are written to be publishable. They are first-class artifacts, not throwaway notes.

**Artifacts:** Design documents (system overview, UML/Mermaid diagrams, bootstrap code). Exploratory essays.

**Exit criteria:** Design is internally consistent, trade-offs are explicit, and open questions are enumerated.

### Phase 3: Architecture Decision Record

Distill design decisions into an ADR. The ADR captures what was decided, why, what alternatives were considered, and what consequences follow. An ADR is precise enough to derive behavioral specifications from.

If writing the ADR reveals ambiguity or contradiction in the design, loop back to Phase 2.

**Artifact:** ADR document (this format).

**Exit criteria:** ADR is accepted. Design decisions are unambiguous. A fresh reader could derive behavioral expectations from the ADR alone.

### Phase 4: Behavioral Specification (BDD)

Write behavioral specs (Given/When/Then) that express the ADR's decisions as testable scenarios. BDD specs describe the system's observable behavior at its boundaries — what goes in, what comes out, what events fire, what errors occur.

If writing BDD scenarios reveals gaps in the ADR, loop back to Phase 3.

**Artifact:** BDD specification files.

**Exit criteria:** Scenarios cover the happy path, error paths, and edge cases identified in the ADR. A developer could implement from the specs without reading the design docs.

### Phase 5: Test-Driven Development (TDD)

Implement the system by writing failing tests (derived from BDD scenarios), then writing the minimum code to make them pass. Red-green-refactor.

If implementation reveals a design flaw, loop back to the appropriate earlier phase rather than patching around it.

**Artifact:** Test suite and implementation code.

**Exit criteria:** All BDD scenarios pass. Code is minimal — no speculative features, no premature abstraction.

### Phase 6: Essay

Write a reflective essay about the process. What did we learn? What surprised us? Where did the research-to-implementation path hold, and where did it break down? What would we do differently?

Essays are publishable artifacts — they communicate the thinking behind the system to others.

**Artifact:** Published essay.

**Exit criteria:** Essay captures the journey from research question to working implementation.

---

## Feedback Loops

The phases are not a waterfall. Common loops:

- **BDD reveals design gaps** → return to Phase 2 or 3. This is expected and healthy. Writing precise scenarios is the fastest way to find imprecision in design.
- **TDD reveals a flaw** → return to the phase where the flaw originates. A failing test that can't be fixed without changing the trait interface is a Phase 2 problem, not a Phase 5 problem.
- **Implementation informs research** → discovering a library or paper during TDD that changes the approach. Return to Phase 1, update the research, propagate forward.
- **Essay clarifies thinking** → writing about the process may reveal that a design decision was poorly motivated. Update the ADR.

The cost of looping back is low because each phase produces a written artifact. Revising a design doc is cheaper than debugging a wrong abstraction.

---

## Consequences

**Positive:**

- Every design decision has a traceable path from research to implementation
- Publishable artifacts accumulate naturally — design docs, essays, ADRs are not extra work, they are the work
- BDD and TDD catch design flaws before they become entrenched in code
- The essay phase forces reflection, which improves future iterations

**Negative:**

- More upfront writing before code exists. This is the right trade-off for a research-informed system but may feel slow on simple features.
- Maintaining consistency across artifacts as designs evolve requires discipline. When an ADR changes, the BDD specs and design docs must be updated.

**Neutral:**

- Not every change requires all six phases. Bug fixes and small features may start at Phase 4 or 5. The full flow applies to new subsystems and significant design decisions.
