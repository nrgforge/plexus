# Invariant Propagation: When Process Artifacts Outlive Their Assumptions

Plexus has been developed through Research-Driven Development — a phased process where research produces essays, essays produce domain models, domain models inform architecture decisions, and decisions drive implementation. Each phase produces durable artifacts. Those artifacts accumulate. And that accumulation, it turns out, creates a failure mode the process wasn't designed to handle.

## What Was Working

The RDD process solved a real problem. AI-assisted development tends toward two failure modes: either you build before you understand (vibe coding) or you plan exhaustively before you learn anything (BDUF). RDD threads between them. Research produces a forcing function — the essay — that converts messy investigation into organized understanding. The domain model extracts vocabulary from the essay. ADRs capture decisions against that vocabulary. Behavior scenarios express what the software should do in domain terms. Implementation proceeds test-first.

Each phase has a gate. The user approves before the next phase begins. Each artifact uses the vocabulary established in the domain model. The argument audit checks logical consistency between the essay's evidence and the ADRs' conclusions. The conformance audit checks whether existing code matches accepted decisions.

For the first dozen essays and fifteen ADRs, this worked. The Plexus domain model grew to 60 concepts, 40 invariants, and a dozen resolved disambiguations. The vocabulary threaded cleanly from essays into ADRs into code. When the codebase said `emission`, the domain model said `emission`, and the ADRs said `emission`. Consistency was structural, not aspirational.

## What Started Breaking

The failure was subtle. It didn't look like a process failure — it looked like a bug.

ADR-015 introduced a consumer-facing `annotate` operation: mark a file location with tags in a named chain. The implementation created marks and chains through ProvenanceAdapter, producing provenance-dimension nodes. It worked. Tests passed. The essay describing the redesigned surface was committed.

But `annotate` violated an invariant. Invariant 7 states that all knowledge entering the graph carries both semantic content and provenance — there is no consumer-facing path that creates provenance without semantic content. An annotation's text *is* a fragment. Its tags produce concepts. Provenance layers on top of the semantic content the fragment provides. The `annotate` operation was creating provenance nodes without the corresponding semantic nodes.

The invariant existed. It was in the domain model. It was clear. It had been there since Essay 12 established provenance as epistemological infrastructure. So why did the violation happen?

Because the invariant was in one document, and the implementation context was in fifteen others. The LLM building the feature had read the ADR describing the public surface, the scenarios describing `annotate`, the essay analyzing consumer needs, the code implementing ProvenanceApi. Invariant 7 was one sentence in a 300-line domain model, competing for attention against thousands of lines of context spread across a dozen artifacts — artifacts that, in some cases, used language like "provenance-only operation" or described marks as separable from semantic content.

This is the information-theoretic problem. LLMs have exponential mutual-information decay with distance. A statement in the first document read has more influence than the same statement in the fifteenth. And invariants — the most important statements in the artifact set — live in one document among many. If a later document contradicts an invariant, the LLM may follow the document it read most recently, not the one with constitutional authority.

The fix was straightforward: `annotate()` now calls `ingest("fragment", ...)` to create the fragment node and concept nodes from tags before creating the chain and mark. Twelve documents were swept for language contradicting the corrected invariant. Supersession notes were added. The domain model was updated to make the bidirectional nature of the dual obligation explicit.

But the root cause wasn't a coding error. The root cause was that the RDD process moves forward — research, model, decide, build — without a mechanism for backward propagation when a decision changes the ground truth that prior artifacts assumed.

## The Structural Problem

The RDD artifact set is a directed graph. Research produces essays. Essays feed domain models. Domain models inform ADRs. ADRs drive scenarios. Scenarios drive implementation. Information flows forward through this chain.

But invariants don't flow forward — they apply everywhere simultaneously. When invariant 7 was strengthened to make the bidirectional dual obligation explicit, the change didn't just affect future ADRs. It retroactively invalidated language in prior essays, prior ADRs, and prior scenarios. Documents written before the strengthened invariant still described the world as it was when they were written.

For a human team, this is manageable. Humans maintain a mental model that updates when decisions change. When someone says "actually, provenance always requires semantic content too," the team updates their understanding and knows to distrust old descriptions. The old documents become historical — interesting for understanding how thinking evolved, but not authoritative.

For LLMs, old documents are not historical. They are context. An LLM in a new session reads old ADRs with the same credulity as new ones. If ADR-008 describes a pattern that invariant 7 now prohibits, and the LLM reads ADR-008 after the domain model, ADR-008's description has recency bias and greater specificity than the invariant's single sentence. The stale document wins.

This is not a hypothetical failure mode. It is what happened.

## What We Did

Three mechanisms were baked into the RDD skill set, each addressing a different aspect of the propagation problem.

**Invariants as constitution.** Every skill that reads artifacts — `/rdd-decide`, `/rdd-build`, `/rdd-research`, `/argument-audit` — now reads the domain model invariants first, before reading essays, ADRs, or code. This is not a suggestion; it is the first instruction in each skill's Step 1. When a later document contradicts an invariant, the skill is instructed to flag the contradiction and follow the invariant, not the document.

This exploits the same information-theoretic property that caused the problem. Primacy bias works in both directions: if the invariants are the first thing read, they anchor the LLM's interpretation of everything that follows. A subsequent document describing a "provenance-only path" is immediately suspect because it contradicts what was read first.

**Backward propagation.** `/rdd-decide` now has a mandatory step (Step 3.7) that triggers when an ADR introduces or changes a domain model invariant. The step requires sweeping all prior ADRs and essays for language that contradicts the new invariant, adding supersession notes to contradicting documents, logging the amendment in the domain model, and presenting the propagation summary to the user. `/rdd-model` has a corresponding step (Step 3.5) that detects when invariants are changed or strengthened during domain model updates and flags the propagation implications.

This is the mechanism that was missing. The RDD process had forward propagation (each phase reads the previous phase's output) but no backward propagation (a decision in a later phase can invalidate assumptions in earlier phases). The cost of a 10-minute backward sweep when invariants change is negligible compared to the cost of a stale assumption surviving into code.

**Tension detection at every phase.** `/rdd-research` now checks existing invariants before writing an essay — if findings contradict invariants, the tension is surfaced explicitly. `/argument-audit` now has an "Invariant Compliance" check (Step 1.5.5) that treats invariant violations as "constitutional violations" — higher severity than normal internal contradictions. `/rdd-build` now includes document-contradicts-invariant as a specific failure mode to flag when building reveals flaws.

These mechanisms are structural, not advisory. They are built into the process steps, not into a "best practices" section that can be ignored. An LLM executing `/rdd-decide` will encounter Step 3.7 in its instruction sequence and execute it, the same way it executes the argument audit or the conformance audit.

## Why Structural, Not Advisory

The temptation was to add a note to the Important Principles section: "Remember to check invariants." This would fail for the same reason the original invariant failed to prevent the `annotate` violation — principles sections are advisory context that compete with procedural instructions for attention.

The mechanisms are instead woven into the procedural steps:

- Step 1 of every artifact-reading skill says "read invariants first" — it is the literal first instruction
- Step 3.7 of `/rdd-decide` is a numbered step in the sequence, between conformance audit and scenario writing — it cannot be skipped without breaking the numbering
- Step 1.5.5 of `/argument-audit` is a substep of the internal consistency scan — it runs as part of the existing audit flow
- The `/rdd` orchestrator describes invariant amendments as "cross-cutting events that interrupt normal phase sequence" — not optional cleanup

The design exploits how LLMs process instructions. Numbered steps are executed sequentially. A step between Step 3.5 and Step 4 will be encountered and processed. An advisory principle at the bottom of a document may or may not influence behavior, depending on context window position and competing instructions.

## How Plexus Could Serve This Process

The invariant propagation problem is, at its core, a knowledge graph problem. There is a set of documents. Those documents make claims. Some claims are invariants — constitutional statements that outrank all others. When an invariant changes, every document that contradicts it needs updating. The question is: which documents contradict which invariants?

This is exactly what Plexus's cross-dimensional traversal was designed for.

Consider the artifact set as a Plexus context. Each essay, ADR, and domain model section is a fragment. Each invariant is a concept node. Tags connect documents to the invariants they reference or assume. Marks annotate specific locations where claims are made. Chains group marks by document.

When invariant 7 is strengthened, you could query:

```
evidence_trail(concept:"dual-obligation")
```

This returns every mark, fragment, and chain connected to the dual-obligation concept — every document that references it, every specific location where a claim about it is made. The backward propagation step becomes a graph traversal rather than a manual sweep.

The tag-to-concept bridging that already exists in Plexus would handle the connection automatically. A mark on an essay passage tagged `#dual-obligation` creates a `references` edge to `concept:dual-obligation`. When the invariant changes, querying inbound references to that concept returns every location in the artifact set that touches it.

The Hebbian dynamics add a layer. Documents that *frequently* reference an invariant — that have multiple marks tagged with it — have stronger connections. Documents with a single tangential reference have weaker ones. The normalized weight tells you which documents are most deeply coupled to the invariant, and therefore most likely to need updating when it changes.

This is not a feature request. It is an observation that the problem we solved procedurally — "sweep all prior documents for contradictions" — is the same problem Plexus solves structurally with cross-dimensional traversal. The RDD process generates exactly the kind of multi-document, cross-referencing, evolving knowledge that Plexus was built to manage.

Whether to build this integration is a separate question. The procedural mechanisms work. They are structural enough to prevent the failure mode. Building a Plexus-backed invariant tracker would be justified if the artifact set grows large enough that manual sweeps become unreliable — if the number of documents and invariants exceeds what a single backward-propagation step can hold in context.

For now, the process fixes are sufficient. But it is worth noting that the tool being built by the process is also the tool that could manage the process. There is a pleasing recursion in that.

## What This Teaches

The invariant propagation failure was not a failure of understanding. The invariant was understood. It was written down. It was correct. The failure was a failure of *reach* — the invariant's authority did not extend reliably across the full artifact set, because the mechanism for reading artifacts did not prioritize invariants over the documents they constrain.

The fix addresses reach through three structural interventions: primacy (read invariants first), propagation (sweep backward when invariants change), and detection (flag contradictions at every phase). These interventions are not specific to Plexus development — they apply to any RDD project where the artifact set grows beyond what a single context window can hold coherently.

The deeper lesson is about the relationship between process and tooling. Good process can compensate for tool limitations. But when the process itself generates the kind of data the tool is designed to manage, the gap between them narrows. Plexus is a knowledge graph that tracks provenance — where knowledge came from, how it's connected, what evidence supports it. The RDD artifact set is a body of knowledge with provenance — research that produced essays, essays that informed decisions, decisions that changed invariants. The structural similarity is not a coincidence. It is the same problem at different scales.
