# Gate Reflection: Default-Install and Lens Design Principles — DISCOVER → MODEL

**Date:** 2026-04-17
**Phase boundary:** DISCOVER (update mode) → MODEL
**Cycle:** Default-Install Experience and Lens Design Principles

## Belief-mapping question composed for this gate

> "What would you need to believe for *in-process fastembed* (compile ONNX into the binary, ship a heavier default) to be the right shipping decision, instead of the llm-orc/Ollama path? The current move favors the latter because Ollama is already a soft dependency for semantic extraction — but if there's a consumer class for which in-process embedding is genuinely load-bearing (e.g., EDDI's real-time loop, an air-gapped deployment, a consumer that doesn't run Ollama), that should be visible before DECIDE writes the ADR."

Paired with commitment gating: *"Of the items in Solution Scoping, which are you building on as settled premises, and which remain genuinely open?"*

## User's response

Primary response to belief-mapping (verbatim):

> "Good question. I guess the question should trace back to where the decision for embeddings there originated. If it originated from an earnest need to use that to make the basic Plexus enrichment work based on our research... or perhaps there's also the version where plexus is embedded within an application itself? I agree the an app has many jobs that need various query surfaces via potentially different lens grammars. I really like extending that analogy into grammar territory. I think it opens the door to thinking more structurally about how lenses work. If lenses encode a 'grammar' then we can query using that grammar. We can quite literally have the graph speak the language of the consumer."

Follow-up response on scoping the grammar framing (verbatim):

> "The lens as grammars question I think ultimately is a big enough hypothesis to warrant its own RDD cycle. So here I'd like to run with a simpler version of that idea to drive back into the validations we were attempting in 'play' before we cycled back here. But I like this insight."

Response to commitment gating (verbatim):

> "A second play by a non-builder won't happen. I'm the only one testing this at present, but I like the spirit of it. The default embeddings question is good, but my concern is leaving it parallel code paths that confuse context, so this needs to be made explicit. The lens question for Trellis is open. For this, I'm certainly interested in the Trellis consumer but I'm more interested in what our experiments teach us about Plexus. I will certainly use the results from our experiments to help build out Trellis but we are not building Trellis here. This is an exercise using real domain examples to ground our work."

Response to agent's unilateral `**Skipped phases:** model, architect` attempt (verbatim):

> "Why is model, architect marked as skipped?"
> "I think even if they're light phases we should move through the full cycle."

## Pedagogical move selected

Belief-mapping (primary, on embedding shipping direction) paired with commitment gating (phase-close). No interpretation recorded per ADR-066.

## Commitment gating outputs

**Settled premises (the user is building on these going into MODEL/DECIDE):**

- ADR-026's three-paths embedding architecture (fastembed-in-process, llm-orc ensemble, Ollama direct) stands as the architectural frame.
- `Embedder` trait is the extension seam — both backends live behind it.
- Ollama is acceptable as a default-adjacent dependency for the Homebrew/CLI deployment class, consistent with semantic extraction's existing Ollama reliance via llm-orc.
- Per-job lens-grammar framing at *composition-shape awareness* level — lens rules shape query patterns, not just vocabulary. The full lens-as-grammar treatment is parked for a future RDD cycle with its own shape.
- Grounding-examples stance: Trellis, Carrel, EDDI, etc. are real-domain grounding cases used to stress-test Plexus design decisions. This cycle builds Plexus, not any specific consumer application.
- "Both embedding backends first-class" with a **load-bearing constraint**: the dual-backend framing must not degrade into parallel code paths that confuse context. DECIDE's ADR needs to specify build-time configuration per deployment class (Homebrew/CLI → Ollama-backed; library-with-features → in-process), not runtime dynamic branching.
- Second PLAY by a non-builder stakeholder is **not happening** this cycle. Methodologically valuable but not realistic given current testing resourcing (the practitioner is the sole tester).
- Full RDD cycle shape stands: DISCOVER → MODEL → DECIDE → ARCHITECT → BUILD (→ optional PLAY, SYNTHESIZE). MODEL and ARCHITECT are planned as light-touch passes rather than skipped.

**Open questions (the user is holding these open going into MODEL/DECIDE):**

- Specific Trellis lens predicates (named vs. structural) — DECIDE will test analytically while drafting the two alternative specs as part of the lens-grammar ADR.
- DiscoveryGap trigger-broadening shape — document as second-stage enrichment, broaden trigger set to include structural-absence patterns independent of `similar_to`, or both. DECIDE's call.
- Onboarding quickstart revision scope — docs-only, or docs + code helper (e.g., a `plexus check-embeddings` command that validates Ollama availability)? DECIDE/BUILD scope.
- Whether the MODEL pass surfaces any new invariants (unlikely but not ruled out); confirmed pending is the dimension-semantics clarification (feed-forward item 10 from cycle-status).

**Specific commitments carried forward to MODEL:**

- Product-discovery artifact is the canonical source for user-facing vocabulary going into MODEL (including the new composition-shape-awareness note on `lens`, the grounding-examples clarification on the Consumer Application Developer stakeholder, and the three new value tensions). Any MODEL vocabulary changes must originate from these product-discovery updates, not from engineering convenience.
- Feed-forward item 10 (dimension mismatch between content adapter `structure` and declarative spec `semantic`) is MODEL's principal work — clarifying dimension semantics in the domain model.
- The parallel-code-paths constraint carries into MODEL only if MODEL surfaces a new invariant about backend-selection; otherwise it belongs to DECIDE as an ADR constraint.
- The lens-as-grammar parked hypothesis (captured in `cycle-status.md` §Hypotheses Parked for Future Cycles) stays parked. MODEL does not vocabulary-ize "grammar" terms this cycle. DECIDE works from composition-shape awareness only.
