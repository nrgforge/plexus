# The Opacity Problem: Why Creators Lose Track of Their Own Work

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — January 2026*

---

## The Problem

Knowledge accumulates faster than understanding. A developer "vibe-coding" with an AI assistant produces working software but may not fully grasp the architectural decisions embedded in the generated code. A researcher's personal knowledge base grows to thousands of notes whose interconnections are invisible. A team's documentation sprawls across wikis, repos, and chat histories with no unified structural map. In each case, knowledge exists but cognitive context — the awareness of what you know, how it connects, and where the gaps are — erodes.

This is not a storage problem. The documents exist. The code compiles. The notes are searchable by keyword. The problem is structural: there is no live representation of the semantic relationships within and across these artifacts. The knowledge is there but opaque to the person who ostensibly possesses it.

We call this the **opacity problem**: the condition where a creator's artifacts contain more structure than the creator can perceive.

## The Cognitive Mechanism

The opacity problem has a cognitive mechanism grounded in established research. Working memory is sharply limited — Sweller's cognitive load theory [14] formalizes this constraint: information that is not organized into existing schemas imposes extraneous cognitive load, consuming capacity that could be directed at the task itself. When information density outpaces comprehension, the excess becomes invisible. Not lost — invisible. The structure is in the artifact but not in the creator's head.

Kirschner et al. [44] formalize this through collaborative cognitive load theory: when task complexity exceeds individual cognitive capacity, the load must be distributed — across collaborators, across time, or across external representations. The opacity problem is what happens when none of these distribution mechanisms are available. The creator works alone (or with an AI that doesn't share the cognitive burden), the pace is too fast for temporal distribution (reflection after the fact), and no external representation captures the accumulating structure.

This is domain-general. A dance improvisation session accumulates gesture data, spatial formations, and performer-environment couplings faster than any choreographer can mentally catalog. A rapid prototyping session produces design variants whose structural differences are invisible without explicit comparison. A research sprint across dozens of papers generates conceptual connections that exceed individual working memory. The common thread: any process that generates structure faster than the creator can track produces opacity.

## AI Makes It Acute

The opacity problem is not unique to AI-assisted work — but AI makes it acute by accelerating production while adding an interruption-heavy interaction pattern.

Consider "vibe-coding": a developer prompts an AI to generate modules iteratively, accepting code that works without fully comprehending how it works. After a dozen exchanges, the codebase has architectural decisions the developer didn't make, dependency patterns they didn't design, and structural implications they never evaluated. The code compiles; the tests pass; the developer has lost structural awareness of their own project.

Cito and Bork [10] describe this as "material disengagement" — developers orchestrate code generation without comprehending the output. Qiao et al. [11] measure the resulting comprehension-performance gap in brownfield development. Al Haque et al. [12] note that few empirical studies of cognitive load from AI coding assistants yet exist — the phenomenon is documented but under-measured. An industry survey (not peer-reviewed) [13] found 65% of developers report AI misses context during refactoring, with 60% citing similar gaps in test generation and review.

Beyond the volume problem, the interaction pattern itself is costly. Mark et al. [42] demonstrate that interrupted work exacts significant cognitive costs — workers compensate for interruptions by completing tasks faster but experience substantially more stress, frustration, and time pressure. The prompt→wait→evaluate cycle of AI-assisted composition is an interruption factory: each generation forces the creator out of compositional flow into an analytical evaluation mode. Subramonyam et al. [43] identify a deeper challenge: the "gulf of envisioning," where users struggle to formulate effective prompts because they cannot anticipate the system's behavior. The interaction alternates between two incompatible cognitive modes — generative composition and critical evaluation of unfamiliar output — and the mode-switching is itself a source of cognitive load.

A recent preprint, the Vibe-Check Protocol [53], formalizes this phenomenon through three metrics: the Cold Start Refactor (measuring skill retention when AI is removed), Hallucination Trap Detection (measuring vigilance via signal detection theory), and the Explainability Gap (measuring the metacognitive disconnect between generated code complexity and the developer's understanding).

Noda et al. [54] identify the three dimensions at stake — feedback loops, cognitive load, and flow state — as core drivers of developer experience, providing an independent validation of the problem space.

## Why Better Prompting Won't Fix It

The instinctive response is to make the AI interaction better: more context-aware models, better prompt engineering, more sophisticated evaluation interfaces. These help at the interaction level but don't address the structural problem. Even a perfect AI interaction — one that never interrupts, always explains its decisions, and produces exactly what the developer intended — still produces artifacts whose accumulated structure exceeds what the developer can hold in working memory. The opacity problem is about information density, not interaction quality. Better interaction reduces the *rate* at which opacity accumulates. It doesn't eliminate the *phenomenon*.

The same applies to slower generation. Deliberate, careful, human-only composition also produces opacity — it just takes longer to get there. A novelist fifty chapters into a manuscript has the same problem as a developer fifty AI-assisted modules into a codebase: the structure of the artifact has grown beyond what a single person can hold in their head. The difference is time, not kind.

## The Remedy: External Structural Representations

The remedy is not better prompting, slower generation, or more documentation. It is structural. The creator needs an external representation of the relationships their work contains.

This claim is well-grounded in cognitive science. Endsley [45] defines situation awareness as the perception of elements in the environment, comprehension of their meaning, and projection of their future state — precisely what erodes in information-dense composition. Creators lose perception of what has been added, comprehension of how it connects, projection of where the structure is heading.

Kirsh and Maglio [15] distinguish epistemic actions (changing the agent's computational state to make mental computation easier) from pragmatic actions (changing the world toward a goal). An external structural representation is epistemic: it doesn't change the work, it changes the creator's cognitive relationship to the work. Scaife and Rogers [17] identify three specific mechanisms: *computational offloading* (reducing working memory demands), *re-representation* (presenting information in a form better suited to the task), and *graphical constraining* (limiting the space of possible inferences).

Larkin and Simon [46] demonstrate that diagrams reduce cognitive load by making information explicit that would otherwise require search and inference — a diagram is worth ten thousand words because it replaces memory retrieval with perceptual processing. Kirsh [47] extends this: external representations do not merely offload cognition but expand the cognitive system's boundaries, enabling forms of reasoning that are impossible with internal representations alone.

A live structural representation that evolves alongside the creative process could provide ongoing situation awareness — what was added, how it connects, where clusters are forming, which relationships are strengthening — without requiring the creator to stop working and explicitly query for this information.

## A Caveat on Cognitive Offloading

External structural representations are not unambiguously beneficial. Gerlich [48] finds a significant negative correlation between frequent AI tool usage and critical thinking abilities, mediated by increased cognitive offloading. Klein and Klein [49] introduce the "extended hollowed mind" framework: when AI frictionlessly provides answers, users systematically bypass the effortful cognitive processes essential for deep learning. EEG evidence shows that LLM interaction reduces frontal theta power — a marker of working memory load — suggesting reduced engagement of the cognitive processes central to learning. Matuschak and Nielsen [50] argue that transformative tools for thought must avoid reducing the user to a passive consumer of externally generated structure.

This critique targets tools that outsource *reasoning* — where the tool does the thinking and the user accepts the output. A structural tool that externalizes *awareness* (what connects to what) while preserving *interpretation* (what it means) may avoid this failure mode. The graph shows that three fragments share a concept; the creator decides whether that matters. Whether this distinction — structural offloading versus reasoning offloading — is stable and implementable is itself a research question. But it suggests that the cognitive offloading critique, while serious, is not a blanket objection to external structural representations.

## Open Questions

The opacity problem is well-established. The case for external structural representations as a remedy is well-grounded in theory. What remains open:

- **What form should the representation take?** A knowledge graph is one option. Hierarchical views, timelines, spatial maps, and dependency lists are others. The right form likely depends on the domain and the creator's cognitive style. This is a design question, not a theoretical one.

- **Should the representation be ambient or on-demand?** Continuous peripheral feedback avoids interruption but competes for attention. On-demand querying requires mode-switching but only appears when wanted. Context-triggered notifications split the difference. The optimal point in this design space is an empirical question.

- **Does continuous structural awareness help or hinder?** Many successful creative practitioners deliberately suppress structural awareness during composition. Writers who don't outline. Developers who spike solutions before architecting. If structural awareness is sometimes counterproductive during the generative phase, a structural tool needs an off switch — or needs to be subtle enough that ignoring it is costless.

- **Is the opacity problem evenly distributed?** Expert practitioners develop internal schemas that let them hold more structural complexity than novices. The opacity problem may be most acute for intermediate practitioners — past the point where the work is simple enough to hold in one's head, before developing the expertise to compress it into schemas. If so, the target audience for structural tools is narrower than "all creators."

These questions motivate the design of Plexus, a live knowledge graph engine for creative composition, and inform its evaluation agenda.

---

## References

[10] Cito, J. & Bork, D. (2025). Lost in Code Generation: Reimagining the Role of Software Models in AI-driven Software Engineering. *arXiv preprint arXiv:2511.02475*.

[11] Qiao, Y., Hundhausen, C., Haque, S., & Shihab, M. I. H. (2025). Comprehension-Performance Gap in GenAI-Assisted Brownfield Programming. *arXiv preprint arXiv:2511.02922*.

[12] Al Haque, E., Brown, C., LaToza, T. D., & Johnson, B. (2025). Towards Decoding Developer Cognition in the Age of AI Assistants. *arXiv preprint arXiv:2501.02684*.

[13] Qodo. (2025). State of AI Code Quality in 2025. Industry Report.

[14] Sweller, J. (2024). Cognitive load theory and individual differences. *Learning and Individual Differences*, 110, 102423.

[15] Kirsh, D. & Maglio, P. (1994). On Distinguishing Epistemic from Pragmatic Action. *Cognitive Science*, 18(4), 513-549.

[17] Scaife, M. & Rogers, Y. (1996). External Cognition: How Do Graphical Representations Work? *Int. J. Human-Computer Studies*, 45(2), 185-213.

[42] Mark, G., Gudith, D., & Klocke, U. (2008). The Cost of Interrupted Work: More Speed and Stress. In *Proc. CHI 2008*, ACM.

[43] Subramonyam, H., Pondoc, C. L., Seifert, C., Agrawala, M., & Pea, R. (2024). Bridging the Gulf of Envisioning. In *Proc. CHI 2024*, ACM.

[44] Kirschner, P. A., Sweller, J., Kirschner, F., & Zambrano, R. J. (2018). From Cognitive Load Theory to Collaborative Cognitive Load Theory. *Int. J. CSCL*, 13(2), 213-233.

[45] Endsley, M. R. (1995). Toward a Theory of Situation Awareness in Dynamic Systems. *Human Factors*, 37(1), 32-64.

[46] Larkin, J. H. & Simon, H. A. (1987). Why a Diagram is (Sometimes) Worth Ten Thousand Words. *Cognitive Science*, 11(1), 65-99.

[47] Kirsh, D. (2010). Thinking with External Representations. *AI & Society*, 25(4), 441-454.

[48] Gerlich, M. (2025). AI Tools in Society: Impacts on Cognitive Offloading and the Future of Critical Thinking. *Societies*, 15(1), 6.

[49] Klein, G. & Klein, H. (2025). The Extended Hollowed Mind: Why Foundational Knowledge is Indispensable in the Age of AI. *PMC*.

[50] Matuschak, A. & Nielsen, M. (2019). How can we develop transformative tools for thought? Essay.

[53] Aiersilan, A. (2026). The Vibe-Check Protocol: Quantifying Cognitive Offloading in AI Programming. *arXiv preprint arXiv:2601.02410*.

[54] Noda, A., Forsgren, N., Storey, M.-A., & Greiler, M. (2023). DevEx: What Actually Drives Productivity. *ACM Queue*, 21(2).
