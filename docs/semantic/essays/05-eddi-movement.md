# EDDI: Toward a Knowledge Graph for Interactive Performance

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — January 2026*

---

## A Research Direction, Not a System

This essay describes a research direction: applying a live knowledge graph to interactive performance, where a performer's movement history drives environmental response (lighting, sound, projection). The system described — EDDI (Emergent Dynamic Design Interface) — does not exist as an implementation. What exists is the graph engine it would build on (Plexus, described in a companion essay), a body of movement analysis research that informs the design, and earlier prototype work on gesture segmentation. This essay is a map of the research space, not a report on a built system.

## The Idea

Most interactive performance systems are memoryless. EyesWeb [35] reacts to gesture in real-time but starts from zero every session. DASKEL [55] provides bidirectional skeleton-to-Labanotation conversion but does not accumulate a history of what the performer has done. Each performance exists in isolation. Nothing carries over.

EDDI proposes an environment that *remembers*. A performer moves through a sequence; gesture data feeds a knowledge graph that accumulates over the session (or across sessions). Pose nodes connect via transition edges. Movement qualities cluster into vocabulary. Performer-environment couplings — gesture X triggers lighting state Y — form as semantic edges and strengthen through repetition. The graph drives the environment's response: as edge weights shift, the environment's response shifts with them.

The performer never sees the graph. Unlike the code or writing domains where Plexus clients render events as visual displays or coaching prompts, in EDDI the graph is embodied in the environment itself. The performer experiences structural feedback by inhabiting a space that responds to their accumulated history — colors warming, sounds layering, the space becoming more responsive as its "memory" of the performer's vocabulary grows. This is the most literal form of ambient structural feedback: the feedback *is* the medium.

## Three Bodies of Work

The research direction draws on three areas that have developed in isolation with complementary limitations.

**Classification systems.** Fdili Alaoui et al. [31] and Garcia et al. [32] use multimodal data to characterize Laban Effort qualities (Weight, Time, Space, Flow). These provide the kind of structural-layer input that a movement semantic adapter would feed to Plexus — pose classification, quality detection, gesture boundary identification. The limitation: these systems classify gestures but say nothing about what the classified gestures *mean* in compositional context. A gesture labeled "sudden-direct-strong" is categorized but not contextualized.

**Dance ontologies.** El Raheb and Ioannidis [33] and Paul et al. [34] encode Labanotation semantics in OWL-2 with Description Logic reasoning — expert-authored schemas for choreographic structure. The limitation: these ontologies are static and expert-authored, precisely the opposite of what emerges from live rehearsal. They represent what dance scholars know about movement, not what a specific performer is doing right now.

**Real-time performance systems.** EyesWeb [35] reacts to performer input in real-time; LuminAI [59] combines bottom-up clustering with top-down Laban/Viewpoints encodings to classify and respond to performer movement. A longitudinal study of LuminAI with fifteen dancers found that the system fostered exploration, enhanced spatial awareness, and expanded movement vocabulary [60]. The limitation: these systems are memoryless. Each session starts from zero.

Plexus proposes connecting these: classification systems populate the graph's structural layer, ontological concepts inform the semantic layer, and the graph's self-reinforcing dynamics provide the cross-session memory that real-time systems lack. Whether this integration is feasible is the research question.

## The Gesture-to-Graph Pathway

The design maps gesture data onto Plexus's four-layer model:

**Structural layer.** Pose estimation (MediaPipe, OpenPose, or similar) produces keypoint streams. Earlier prototype work explored **Skeleton-MHI** (skel-mhi), which applies Motion History Images [63] to skeletal data rather than raw video, using energy dissipation patterns to detect gesture boundaries. The structural layer captures what the performer is doing right now — poses, transitions, spatial formations — at the fastest update frequency.

**Relational layer.** Gesture segments cluster into movement vocabulary through feature similarity. Which gestures cluster together? Which transitions are practiced vs. novel? Spatial proximity and temporal co-occurrence create relational edges between performers in ensemble work.

**Semantic layer.** Choreographic phrases — sequences of gestures that form compositional units — are discovered through pattern recognition. Performer-environment coupling (gesture X reliably triggers lighting state Y) becomes explicit as semantic edges.

**Conceptual layer.** What emerges over time: how the performance vocabulary evolves across rehearsals, which formations recur, which couplings strengthen through use.

This is where the design is most speculative and the challenges most substantial. The structural layer is tractable — pose estimation and energy-based segmentation are established techniques. The relational layer is feasible — clustering by feature similarity is well-understood. The semantic and conceptual layers require solving problems that the movement analysis community has not solved: identifying choreographic phrases computationally, distinguishing deliberate repetition from habit, detecting compositional development across sessions. These are hard open problems, not engineering tasks.

## The Reinforcement Problem in Movement

Viewpoints composition — developed by Mary Overlie and extended by Anne Bogart — is grounded in the observation that repetition creates meaning: a gesture that occurs once is exploration, one that recurs is a choice, one that recurs and *transforms* is vocabulary. This maps naturally onto self-reinforcing edge dynamics: edges that represent recurring gestures strengthen; novel gestures start at sketch weight.

The problem is distinguishing intentional repetition from accident. The system can detect temporal recurrence — a pose-sequence appearing twice. It cannot reliably distinguish deliberate compositional repetition from habit, fatigue, or physical constraint. A performer who returns to the same movement phrase because it's compositionally important and one who returns because they're tired look identical to a pose tracker.

This is the fundamental challenge for the movement domain and the reason EDDI remains a research direction. The code domain has tests — an objective, automated validation signal. The writing domain has explicit writer actions (sorting, promoting, linking) — intentional compositional signals. The movement domain has temporal recurrence as a *proxy* for intentional repetition without a reliable way to verify the inference.

Possible approaches, none fully satisfactory:
- **Temporal pattern detection**: Recurrence across sessions (not just within a session) is a stronger signal of intentional choice. A gesture that appears in three separate rehearsals is more likely deliberate than one that repeats within a single improvisation.
- **Energy and quality analysis**: Deliberate repetition may differ from habitual repetition in movement quality — more varied dynamics, greater spatial range, intentional transformation. Laban Effort analysis could distinguish "the same gesture done with different intent." Whether current classification accuracy is sufficient for this is uncertain.
- **Performer confirmation**: An explicit signal (a button press, a verbal cue, a post-session review) where the performer marks significant moments. This breaks the ambient design but provides ground truth.
- **Accept the noise**: For simpler applications (installations, games, therapeutic settings), the distinction between intentional and accidental may not matter. Consistent gesture-environment coupling is useful regardless of whether the gesture was "meant." Reserve the intention-detection problem for the choreographic use case specifically.

## Environmental Response

EDDI translates graph state into environmental parameters through arousal-theoretic mapping. Seif El-Nasr et al. [61] demonstrate that computational control of environmental parameters (color, intensity, direction) reliably modulates arousal and valence in interactive settings.

The proposed mapping: edge weight maps to arousal level, which modulates environmental parameters. A well-established coupling (high edge weight) produces a committed environmental response — warmer colors, higher intensity, richer sonic texture. A novel gesture (low edge weight) produces a tentative response. The performer experiences the graph's structural confidence as the *felt intensity* of the environment's response.

This mapping is not implemented. The specific transfer functions need to be designed and tuned through rehearsal. The arousal-theoretic framework provides direction, not a fixed algorithm.

## What Needs to Happen

For EDDI to move from research direction to prototype:

1. **Validate gesture segmentation latency.** The skel-mhi approach has design targets (<10ms segmentation) but no empirical validation. Does skeleton-based MHI actually achieve real-time segmentation at the quality needed for reliable structural-layer input?

2. **Build a movement-domain semantic adapter.** This is the prerequisite for testing whether Plexus's architecture serves the movement domain at all. Even a minimal adapter (pose classification + temporal provenance, without the harder semantic/conceptual layers) would test the integration.

3. **Test the content-agnosticism hypothesis.** Does the Plexus graph engine work with gesture data without engine-level modifications? Do the edge dynamics behave sensibly with movement-domain validation signals? This is a direct test of the architectural claim.

4. **Prototype the environmental response.** A minimal installation — gesture data → Plexus → edge weights → lighting parameters — would test whether the feedback loop is perceptible and whether performers find it useful or distracting.

5. **Confront the intention problem.** Either develop a reliable method for distinguishing intentional from accidental repetition, or scope the system to use cases where the distinction doesn't matter.

Each of these is a research project. EDDI is not a single system to build; it is a research program at the intersection of computational movement analysis, knowledge graph systems, and interactive performance design.

---

## References

[31] Fdili Alaoui, S. et al. (2017). Seeing, Sensing and Recognizing Laban Movement Qualities. In *Proc. CHI 2017*, ACM.

[32] Garcia, M. et al. (2020). Recognition of Laban Effort Qualities from Hand Motion. In *Proc. MOCO 2020*, ACM.

[33] El Raheb, K. & Ioannidis, Y. (2012). A Labanotation Based Ontology for Representing Dance Movement. In *GW 2011*, LNCS 7206, Springer.

[34] Paul, S., Das, P. P., & Rao, K. S. (2025). Ontology in Dance Domain—A Survey. *ACM J. Computing and Cultural Heritage*, 18(1).

[35] Camurri, A. et al. (2000). EyesWeb: Toward Gesture and Affect Recognition in Interactive Dance and Music Systems. *Computer Music Journal*, 24(1), 57-69.

[36] Forsythe, W. (2008). Choreographic Objects. Essay.

[55] DASKEL. (2023). An Interactive Choreographic System with Bidirectional Human Skeleton-Labanotation Conversion. In *Proc. Pacific Graphics 2023*.

[59] Trajkova, M., Jacob, M., & Magerko, B. (2024). Exploring Collaborative Movement Improvisation Towards the Design of LuminAI. In *Proc. CHI 2024*, ACM.

[60] Trajkova, M. et al. (2025). Bringing LuminAI to Life: Studying Dancers' Perceptions of a Co-Creative AI. In *Proc. C&C 2025*, ACM.

[61] Seif El-Nasr, M. et al. (2007). Dynamic Lighting for Tension in Games. *Game Studies*, 7(1).

[63] Bobick, A. F. & Davis, J. W. (2001). The Recognition of Human Movement Using Temporal Templates. *IEEE Trans. PAMI*, 23(3), 257-267.

[66] Green, N. (2026). Semantic Extraction for Live Knowledge Graphs: An Empirical Study. *Working Paper*.
