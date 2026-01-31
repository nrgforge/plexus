# Trellis: Scaffolding Without Generation in AI-Assisted Creative Writing

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — January 2026*

---

## The Design Constraint

AI writing tools generate text. Trellis does not. This is not a limitation — it is the core design decision, grounded in emerging research on how AI interaction affects creative ownership and in a specific theory of what writers actually need.

The problem Trellis addresses is not "how do I write more?" but "how do I see what I've already written?" Writers accumulate fragments — sentences, observations, character sketches, plot ideas, research notes — across days, weeks, months. These fragments are the raw material of composition, but their interconnections are invisible until the writer manually traces them. The structure is in the material; the writer can't see it.

This is an instance of the opacity problem described in a companion essay: creative work accumulates structure faster than the creator can perceive. For writers specifically, the opacity is compounded by time — fragments captured weeks apart may share thematic connections that are invisible because the writer has forgotten the earlier fragment, or never consciously noticed the connection.

## Why Not Generate?

The instinct is to solve this with generation: AI that drafts prose, suggests continuations, writes dialogue. The research suggests caution.

Lee et al. [56] study varied scaffolding levels in human-AI co-writing and find that while paragraph-level AI content increases writing volume, it diminishes the writer's sense of ownership and satisfaction — extensive AI content reduces the experience of creative effort. The volume goes up; the felt authorship goes down. For writers who care about the process (which is most writers who write seriously), this is a poor trade.

Gero et al. [57] study eighteen creative writers integrating AI into their practice and find that personal essayists are the most restrictive about AI-generated text, using AI only for research and feedback, not prose. The writers who care most about voice — about the text sounding like them — are the least willing to accept generated content.

Ramesh et al. [58] advocate explicitly for process-oriented AI design: systems that function as "critical partners" scaffolding writing sub-processes rather than automating them, preserving active knowledge transformation over passive acceptance.

The pattern across these studies: generation helps with volume but hurts with ownership. Scaffolding — making the writer's own material more visible and more connected — preserves both.

## What Trellis Does

Trellis is a creative writing scaffolding system built on the principle of *scaffolding, not generation*. Writers accumulate fragments. Trellis, integrated with a knowledge graph engine (Plexus), enriches those fragments with semantic structure:

- **Fragment nodes** enter the graph as they're created
- **Structural edges** connect fragments by proximity (written in the same session, filed in the same collection)
- **Relational edges** connect fragments by shared terms, character references, thematic overlap
- **Semantic edges** connect fragments by deeper conceptual relationships (discovered via LLM extraction)

The writer doesn't interact with the graph directly. Instead, Trellis surfaces connections through coaching prompts: juxtapositions ("here are two fragments from different months — anything here?"), structural observations ("you've captured 30 fragments this month, clustered around three concepts"), and invitations to sort ("if you had to group these, how would you?").

The graph operates as infrastructure, not interface. The writer experiences it through what Trellis says, not through a visualization.

## The Non-Interpretation Constraint

The critical design constraint is what Trellis *doesn't* do: it does not interpret. "These three fragments share the concept of 'isolation'" is structural observation. "This character is struggling with loneliness" is interpretation — and is explicitly outside scope.

We call this the **mirror, not oracle** principle. The system reflects what the writer has already created; it does not tell the writer what their work means.

**Permitted behaviors:**
- Juxtaposing fragments without comment
- Asking genuinely open questions ("anything here?")
- Inviting sorting ("if you had to group these, how would you?")
- Surfacing material ("here are your fragments from this month")
- Acknowledging activity ("you've captured 30 fragments")

**Prohibited behaviors:**
- Naming themes ("these are about control")
- Claiming connections ("I noticed these relate")
- Interpreting meaning ("this suggests you're preoccupied with...")

When interpretation is occasionally appropriate — because the writer explicitly requests it — it is framed as possibility rather than assertion: "I wondered if these might be related" rather than "these are connected."

An honest caveat: this constraint is implemented through prompt engineering, structured output schemas, and speech-act classification — it is a UX design decision, not an architectural guarantee. The system's semantic extraction layer *does* identify themes and conceptual relationships internally; the constraint governs what gets *communicated*. The distinction between what the system knows and what it says is maintained by the output layer, not by the graph engine. This makes the constraint more fragile than an architectural property would be — it depends on prompt engineering continuing to work as models and extraction strategies evolve. The technical implementation is documented in a companion paper [62].

This framing maps onto Vygotsky's zone of proximal development — the tool operates in the space between what the writer can perceive unaided and what becomes visible with structural support, without crossing into the territory of doing the work for them.

## Self-Reinforcing Dynamics in the Writing Domain

Fragment connections start at sketch weight — the graph notices co-occurrence but does not treat it as significant. Edges strengthen through writer engagement that validates the connection:

**Active sorting.** When a writer groups fragments, moves them into a shared collection, or explicitly links them, the connecting edges strengthen. The writer has validated the relationship through intentional compositional action.

**Seed promotion.** Trellis tracks fragment maturity (seedling → developing → established). When a fragment is promoted, its edges to other fragments strengthen — the writer has signaled that this material matters.

**Revisitation after dormancy.** A fragment ignored for months that the writer returns to and connects to new work receives stronger reinforcement than one accessed continuously. This is the Bjork and Bjork [28] "desirable difficulty" applied to creative material — the faded connection, re-validated, strengthens more than the connection that never decayed.

**Thematic recurrence across sessions.** When the concept "isolation" appears in fragments captured weeks or months apart, the edge between those fragments strengthens through organic recurrence — the writer's own unconscious thematic preoccupations becoming visible.

The writer experiences the graph's structural confidence through what Trellis surfaces: juxtaposition prompts feature high-weight connections more prominently than sketch-weight ones. The system's coaching voice becomes more confident about validated connections ("you've returned to this theme four times across three months") than about tentative ones ("these fragments share a word").

## Open Questions

**Does non-interpretation hold under use?** The constraint is clear in principle. Whether writers experience Trellis's outputs as observation or interpretation — and whether that distinction matters to them — requires testing with real writers. A pilot study with 8-12 writers, comparing how they characterize Trellis's prompts (observation vs. interpretation), would answer this.

**Does structural awareness help or hinder generative writing?** Some writers deliberately avoid structural thinking while drafting. If Trellis's coaching prompts pull writers out of generative mode and into structural mode, the tool may impede the process it aims to support. The question is whether structural awareness can be surfaced *between* writing sessions (during review, planning, or exploration) rather than *during* composition — and whether that timing is sufficient.

**Is the validation signal strong enough?** The writing domain's validation mechanisms (sorting, promotion, revisitation, recurrence) are weaker signals than the code domain's (tests passing). Whether they produce enough reinforcement to differentiate signal from noise in the graph is an empirical question.

**Does the mirror, not oracle constraint survive LLM evolution?** The constraint is implemented through prompt engineering. As models become more capable, maintaining the line between structural observation and interpretation may become harder — more capable models may produce outputs that feel interpretive even when prompted for observation. The constraint may need to evolve from prompt engineering to architectural enforcement (output filtering, structured generation constraints).

## Implementation Status

Trellis has a working prototype with core accumulation and coaching features, described in a companion paper [62]. The Plexus integration — enriching fragments with semantic structure and applying self-reinforcing dynamics — is designed but not yet built. The validation mechanisms described above are at prototype stage: active sorting and seed promotion are implemented; dormancy-based reinforcement and thematic recurrence detection require the self-reinforcing edge dynamics that are not yet operational in Plexus.

---

## References

[28] Bjork, R.A. & Bjork, E.L. (1992). A New Theory of Disuse and an Old Theory of Stimulus Fluctuation. In *From Learning Processes to Cognitive Processes*, Erlbaum.

[56] Dhillon, P. S., Molaei, S., Li, J., et al. (2024). Shaping Human-AI Collaboration: Varied Scaffolding Levels in Co-writing with Language Models. In *Proc. CHI 2024*, ACM.

[57] Gero, K. I. et al. (2025). From Pen to Prompt: How Creative Writers Integrate AI into their Writing Practice. *arXiv:2411.03137*.

[58] Ramesh, V. et al. (2025). AI in the Writing Process: How Purposeful AI Support Fosters Student Writing. *arXiv:2506.20595*.

[62] Green, N. (2026). Trellis: A Creative Scaffolding System for Writers. *Working Paper*.

[66] Green, N. (2026). Semantic Extraction for Live Knowledge Graphs: An Empirical Study. *Working Paper*.
