# Field Notes

**Play session:** 2026-04-16
**Practitioner:** Nathan (inhabiting Consumer Application Developer)

## Stakeholder: Consumer Application Developer

**Super-Objective:** Ingest domain-specific data into a knowledge graph, receive structural signals in domain vocabulary, and act on those signals — without learning graph internals.

**Domain inhabited:** Shared writing context containing Carrel (writing aggregation, voice profiles, Semantic Scholar research, publishing) and Trellis (creative-writing fragment accumulation, latent-connection surfacing). The two intersect where research themes share terrain with creative-writing themes — which is the architectural reason to put them on a single shared context.

**Point of Concentration:** Session setup + declaring consumer identity (`set_context`, `load_spec`).

---

### Two tools overlap on "get me a workspace"

**Observation:** `context_create` and `set_context` both cover the "I want to start working" action. `context_create` only creates; `set_context` creates-if-needed AND activates. On instinct I reached for `context_create` because my prior sentence said "create a new context" — then I needed a second call to activate the context I had just created. A single `set_context("writing-context-test")` would have been sufficient. The split has a legitimate use case (seeding multiple contexts programmatically without switching), but the default first-session path is two calls for what felt like one action.

### The pull toward pre-alignment when sketching multiple lenses

**Observation:** The natural move when sketching multiple lens vocabularies is to pre-align on shared concepts ("both Carrel and Trellis care about theme, so both lenses should point at a shared `theme:patience` node"). The gamemaster framing itself pulled in that direction — surfacing "how does the shared ground work structurally?" as *the* design question, with options A/B/C all variations on up-front coordination. But pre-alignment undermines Plexus's own premise — that semantic extraction layers, adapters, and cross-lensing discover structure from unstructured input. If the consumer specifies the alignment up-front, the semantic extraction layer has nothing left to discover — the consumer has done its job in advance.

The lens is a translator from graph-emergent signal into actionable consumer vocabulary. It is **not** a pre-coordinated ontology between consumers. Each consumer ingests raw material in their own shape. Plexus's enrichments (co-occurrence, embedding similarity, temporal proximity, discovery gap) surface latent structure. Each lens independently translates that structure into the language its app can act on. Cross-pollination happens because the structure is shared (same underlying graph), not because the vocabularies were pre-coordinated.

This is fundamental to Plexus's value proposition. Getting it wrong in how consumers are *invited* to think about lens design (product discovery, interaction specs, worked examples) would undermine the entire premise of the extraction layer doing real work.

### Phenomenology of discovery constrains lens output language

**Observation:** Trellis's architecture paper (§3.7) distinguishes *receiving information* from *having a discovery* — "when someone tells you 'these ideas are connected,' you receive an observation. When you notice the connection yourself, you have a discovery." That's not just a UX value; it's a constraint on lens design. A lens edge that *names* a connection ("these fragments share theme:patience") cancels the discovery phenomenology — the writer receives the interpretation instead of having the discovery themselves.

For consumers like Trellis whose value proposition is creative capability development, the lens must write signals that *create conditions for discovery* without *asserting what's there*. The lens output language should be structural/topological predicates — `has_N_semantic_neighbors`, `bridges_communities`, `member_of_candidate_cluster` (cluster with no name attached), `latent_pair` (shared concepts, no direct edge — candidate for juxtaposition), `dormant_since_T`, `density_shift_this_period`. Describes the *shape* of connections, not their meaning. The writer supplies meaning through juxtaposition-triggered reflection.

This is a strong constraint on the lens spec grammar. It may not apply to every consumer — Carrel and similar orchestration/publishing apps probably need named relationships (`lens:carrel:draft_about_theme`) to drive their logic — but for consumers whose core job is supporting the user's interpretive work, the lens language needs to be deliberately un-interpretive.

### Three complementary layers for Trellis's lens

**Observation:** Three structurally-uninterpretive layers together give Trellis what it needs:

1. **Thematic semantic extraction** — extract concepts from fragments as graph nodes, let co-occurrence edges emerge. The lens points at structural co-occurrence, not at "the theme."
2. **Network science lens** — topological properties (centrality, community membership, bridge nodes, clustering coefficient, dormancy). Intrinsically uninterpretive — describes the *shape* of the graph around a node.
3. **Juxtaposition selection** — surface pairs/sets that share structural criteria. The lens prepares edges; queries pick along strategies (semantic similarity, semantic bridging, temporal proximity, diversity injection, random — already in the Trellis paper's §4.3.1 selection-strategy table).

The "latent discovery gap" case is especially valuable for Trellis: pairs that share extracted concepts but have no direct edge are prime juxtaposition candidates. Plexus's discovery-gap enrichment already does this work. For a creative-scaffolding consumer, surfacing *absence* (unconnected pairs that could be connected) matters at least as much as surfacing *presence* (high-confidence similarity).

### Writers don't compose graph queries — coaching modes are the query DSL

**Observation:** The scaffold question "moment the app needs a graph signal" implicitly suggested the user composes queries. In practice the writer has no precise language for this — they interact with coaching-mode UI (a juxtaposition card, an invitation question, a digest email, an ad-hoc "show me something"). Trellis's internal query composer translates the coaching mode into the graph query (`find_nodes` with some `relationship_prefix: "lens:trellis:..."`). The writer never sees the graph query surface.

This shapes lens design: lens edges are written for the *app's query composer*, not for the *user*. The user's vocabulary lives in the UI copy; the lens vocabulary lives in the graph. They don't have to match.

Also: scheduled vs ad-hoc query modes are just timing. Plexus's MCP surface doesn't distinguish. A Trellis scheduler firing daily digests and the writer ad-hoc invoking "show me something" both land as the same `find_nodes` / `traverse` calls. Trellis's scheduler is its own concern.

### Querying begets ingestion — the consumer-Plexus feedback loop

**Observation:** Lens signals aren't endpoints. The user responds to a signal (in Trellis's case, by writing a reflection; in Carrel's case, by drafting or marking). The response produces new ingest content. The graph restructures. The next query draws on the enriched graph. This is a tight feedback loop — the lens processes a graph it partly authored.

Two consequences:

1. **In a shared context, cross-pollination is the fly-wheel.** A Trellis juxtaposition surfaces a Carrel-ingested research paper next to a Trellis fragment — the writer reflects, writes a new fragment, ingests it. The new fragment is now Carrel-visible too. The shared context means each consumer's coaching/aggregation output feeds the other's input. This is load-bearing for why the shared-context architecture matters.

2. **Different consumers have different loop tightness.** Trellis's loop is tight: query → reflect → capture new fragment → ingest → re-query. Carrel's loop is looser: research digest → write new draft → eventually publish → the published output becomes ingested content on the next scan. The writing act is the long step between query and ingest. Consumers with reflective/creative roles (Trellis) have tight loops; consumers with publishing/orchestration roles (Carrel) have looser ones. Both matter; Plexus supports both because it's indifferent to cadence.

The loop character is why Plexus's value compounds over time — not because the graph simply grows, but because the graph's emergent structure is shaped by responses to its own prior surfaces. The first ingest is raw material. The hundredth ingest was partly occasioned by what the system surfaced at the fiftieth.

### Apps have multiple jobs — classifying them by the most visible surface can obscure the discovery core

**Observation:** Early framing held Trellis as "the discovery app" and Carrel as "the orchestration/publishing app, with a looser loop and named lens relationships." Stepping into Carrel from the user perspective — *"I'm searching for a thesis across this work... a desktop where the things we're interested in gather and on that surface, interesting connections emerge"* — revealed Carrel is equally discovery-oriented. Its publishing pipeline is downstream of thesis-finding, not the point of the app. The phenomenology-of-discovery constraint applies to both apps for the same reason: if the lens names connections, the writer receives observations instead of having discoveries.

Classifying consumers by their most visible surface (Trellis=coaching; Carrel=publishing) misses that both have a *discovery core*. Plexus's lens-design affordances need to support the discovery core even for apps whose user-facing surface is orchestration or publishing. The named-relationship vs. structural-predicate question isn't a per-app choice; it's a per-job choice within an app. The same app may need named relationships for its publishing pipeline's internal routing and structural predicates for its thesis-finding surface.

### Writing emerges *between* applications — apps are lenses on material, not containers of it

**Observation:** A striking consequence of the shared-context/cross-pollination architecture: between Trellis and Carrel, **writing can emerge that isn't centered in either app a priori**. Whether a given piece of writing "belongs to" Carrel's domain (research-driven essay) or Trellis's domain (creative reflection that grew from a fragment) depends on what the cross-pollination surfaces and which surface the writer chooses to follow. The writer's actual creative output lives in the intersection, not inside either consumer.

This implies a design posture: consumer apps are **lenses on shared material**, not **containers for their own material**. Neither Carrel nor Trellis "owns" the writing. The graph (the context) is consumer-app-neutral. Each consumer offers a different affordance for engaging the same material — Carrel's thesis-finding desktop, Trellis's juxtaposition coaching, some future publishing consumer's target adapters — but the material itself belongs to the writer and lives in the shared graph.

This has implications for how consumers are *architected* and *marketed*. A consumer that tries to become a container (its own database, its own storage, its own closed surface) cuts itself off from cross-pollination and defeats the shared-context value proposition. Consumers should be thin surfaces over the shared graph, with their value in the *lens translation* and *UI affordances*, not in data hoarding.

### Mechanism convergence across different-concern apps is evidence Plexus's premise works

**Observation:** Trellis and Carrel started from very different-sounding concerns — creative fragment capture with mirror-coaching vs. external research aggregation with publishing pipeline. In the naive product description they read as two distinct apps. Yet stepping through the RDD scaffold questions (raw input, user's job, moment, signal form), they converged on very similar Plexus mechanisms: declarative adapters per input_kind, lens emitting structural/topological predicates, query patterns composed internally by the app's coaching/digest logic, a feedback loop where surfaced signals drive new ingest.

This convergence is itself a finding. Plexus's premise — that semantic extraction + core enrichments + lens translation are general mechanisms — holds up: apps with different user-facing surfaces reduce to similar Plexus interaction shapes. It also raises a product-design question the practitioner is already asking: are Trellis and Carrel actually two apps, or are they one app with two user-surface modes? Keeping them separate for now is a reasonable choice (different user-facing jobs warrant different UX), but the architectural convergence suggests a possible future consolidation — or that more domain-specific consumers could be authored cheaply, since the mechanism beneath them is shared.

### Indeterminacy, delayed recognition, and the writer's journey

**Observation:** Both Carrel and Trellis share an *indeterminacy quality* — they accept input whose relevance is felt but not articulable at add-time. The writer adds things because they feel thematically resonant; the graph reveals *how* they're resonant later. This implies the lens needs to support *delayed recognition* signals, not just point-in-time similarity. Plexus's temporal dimension (ingest timestamps, `changes_since`, temporal-proximity enrichment) is load-bearing for this class of query — specifically for queries like "what's now related that wasn't obviously related when I added it?" or "what has my graph grown around while I wasn't looking?"

Both apps serve the same underlying writer journey:

> *thematic intent → accumulate (heterogeneous, opportunistic, felt-but-not-articulated) → structural emergence over time → recognition moment → articulation (writing)*

A concrete anchoring use case: a writer working on a short story or novel. They know the themes they want to explore but not the thesis, the arc, or how their materials will fit together. As thoughts arise, they flow into Trellis. As external materials are encountered (papers, blog posts, other writers), they flow into Carrel. Both feed the same shared context. Over weeks or months, the thesis emerges between them — visible as density shifts, latent pairs, cross-field concept bridges surfaced by the lenses over accumulated structure.

Notable consequence: **the writer doesn't need to know in advance whether a thought belongs in Trellis or Carrel.** Separation of the two apps is about *capture ergonomics* (different UIs for different input sources — a fragment-capture interface vs. an RSS/feed/file-drop interface), not about separation of concerns in the graph itself. The graph is consumer-app-neutral; the apps differ in how they *receive* material and how their lenses *translate* emergent structure back into signals their UIs can surface.

The delayed-recognition class of signal is also an evaluative criterion for the MCP query surface: can a consumer compose a query that distinguishes "things that were related when I added them" from "things that became related after I added them"? This is not obviously well-supported by the current query surface — something to notice as the specs get authored and tested.

### Misreading the enrichment surface as closed when it is open

**Observation:** While sketching Trellis's lens, initial framing claimed network-science signals (bridges, communities, centrality, clustering, dormancy) "cannot be expressed with current enrichments" because the Rust-native core only has four pairwise/local enrichments: cooccurrence, embedding_similarity, temporal_proximity, discovery_gap. This was a misreading of the architecture.

The correct picture (ADR-024 and the declarative adapter design): Plexus's core enrichment surface is deliberately small and pairwise because the **extension surface is open and declarative**. Path 3 — external enrichments via llm-orc ensembles with Python script agents — is the intended mechanism for consumer-specific enrichment logic, including graph-science algorithms. A script agent can query graph state, run networkx/igraph/graph-tool, and emit new relationship types via `ingest()`; those relationships enter the core enrichment loop and the lens translates them like any other source.

The lens is **agnostic to where its source relationships come from**. Core enrichments, declarative config, external ensemble, another consumer's lens — all identical at the translation interface. The lens grammar is open: a consumer grows their app's signal surface by authoring ensembles, not by waiting for Plexus to grow its core.

**Why this matters beyond the immediate misstep:** the mis-framing "the core is the surface" is a trap consumers may fall into when looking at Plexus's out-of-the-box enrichments and concluding it's too constrained. Outward-facing documentation, worked examples, and onboarding materials need to make the open extension surface visible — otherwise consumers will either abandon Plexus as insufficient, or (worse) request features as additions to the Rust core when the declarative/ensemble path is the intended fit. The core's smallness is a design choice, not a limitation.

### Crawl-step results and the tautology threshold

**Observation:** The first live test (load both minimal specs, ingest four tagged content items, query both lens namespaces) validated the spec-loading and cross-pollination mechanism. Both specs loaded; both lenses translated every `may_be_related` edge; cross-pollination was visible as shared concept nodes reachable under both `lens:trellis:` and `lens:carrel:` prefixes. The **plumbing works**.

But the test as designed was **tautological**. The tags were hand-chosen to overlap. CoOccurrenceEnrichment detected the hand-encoded overlaps. The lens translated them. The system reported back to the user exactly what the user had encoded. Zero information was added by Plexus between input and output. A novelist looking at this result would rightly ask: "why did I need a knowledge graph to find overlaps in tags I explicitly wrote?"

This is the **tautology threshold** — the boundary between mechanism validation and value validation. Plexus crosses it only when at least one mechanism adds structure the user did not encode:

| Mechanism | Escapes tautology via |
|---|---|
| Semantic extraction (llm-orc ensemble over prose) | Concepts extracted from text content, not supplied as tags |
| Embedding similarity | Proximity between items sharing no tokens |
| Discovery-gap at scale | Latent pairs the writer's blind spot would miss |
| Temporal surfacing | Structure formed after content was added — beyond user working memory |
| Topological analysis | Centrality / communities cognitively infeasible to compute by hand |
| External citation networks | Structure the user did not author (Semantic Scholar, RSS co-citation) |

The crawl step operated none of these. Only CoOccurrenceEnrichment over user-supplied tags. For the practitioner in this session, the tautology was obvious because the practitioner controlled the tag choices. For a real consumer, it is less obvious: a novelist tagging fragments in-the-moment doesn't self-consciously design the tag overlaps, but the tag-vocabulary IS still their encoding. Tautology shifts from "the operator designed it" to "the writer's own tacit tagging discipline supplied it."

**Implications for product framing:**

1. The onboarding path needs to make the tautology threshold visible. Demos and quickstart examples that use pre-tagged content and show cross-pollination "working" are actively misleading — they demonstrate mechanism while hiding that the mechanism alone adds no value. A consumer who adopts Plexus on the strength of such a demo and then uses it with their own tagged content may get mechanism but not value, and blame Plexus instead of the demo's hidden tautology.
2. The minimum value-demonstrating setup is not the minimum mechanism setup. Real worked examples should ingest *untagged prose* and demonstrate emergence of structure the example's author didn't pre-encode.
3. The "simplest thing that works" for Plexus is not a good marketing ask. "Simplest thing that adds value beyond its input" is what consumers need to see.

**Practical note for the walk step:** Crossing the threshold requires wiring semantic extraction (llm-orc ensemble), embeddings (fastembed or Ollama embedder), or scale+time. The existing T8 gated test already demonstrates the semantic-extraction path is live but requires Ollama running. The walk step should deliberately choose which threshold to cross and verify that the chosen setup produces structure the user did not supply. Anything short of that is more mechanism validation, not more value validation.

### A real crawl: what the default Homebrew build actually does

**Observation:** The first "crawl step" was ceremonial re-verification of an acceptance test that already existed (`tests/acceptance/mcp_e2e.rs`) — different strings, same mechanism confirmation, no new information. The practitioner correctly flagged this: *"I think we need a 'crawl' step that actually teaches us something about the system."* Rerunning to produce genuinely informative findings revealed real issues that the first crawl had hidden.

**Finding 1 — Three of the four "default" enrichments are silently inactive in the Homebrew-installed binary (`plexus 0.2.0`).**

`PipelineBuilder::with_default_enrichments()` installs four enrichment instances. Diagnostic traversal of the post-ingest graph showed only `may_be_related`, `tagged_with`, and the two lens-translated relationships were present. No `similar_to`, no `discovery_gap`, no `temporal_proximity` edges appeared despite conditions that should have triggered them if the enrichments were alive. Root causes:

- **EmbeddingSimilarityEnrichment: feature-gated OFF.** `Cargo.toml` has `default = []` and the enrichment is only pushed under `#[cfg(feature = "embeddings")]`. The Homebrew build does not enable the `embeddings` feature. Any consumer expecting semantic similarity out-of-the-box from the default install will get nothing.
- **DiscoveryGapEnrichment: installed but without triggers.** It's instantiated with `"similar_to"` as the trigger relationship, which only `EmbeddingSimilarityEnrichment` produces. With embeddings off, DiscoveryGap has nothing to fire on. It's present in the enrichment registry but silently dead.
- **TemporalProximityEnrichment: installed but reading a non-existent property.** It reads `node.properties["created_at"]`. No built-in adapter writes `created_at` as a node property — `created_at` exists on node *metadata*, which the enrichment doesn't consult. All timestamped nodes are invisible to TemporalProximity. Silently dead.
- **CoOccurrenceEnrichment: works.** It's the only default enrichment that actually does something in the default build, and it only operates on `tagged_with` edges (which exist only when the user supplies tags).

A new consumer installing Plexus via Homebrew, reading the architecture docs, and expecting "four core enrichments" gets **one**. This is a documentation/default-pipeline truthfulness problem. The architecture's descriptive narrative (four enrichments, pairwise/local) is true of the *registry*; it is not true of *what runs*.

**Finding 2 — The declarative adapter DID register and route — so it's not scaffolding.**

Test: ingest `{"id": "test-001"}` with `input_kind: "trellis.fragment"`. Result: a `fragment:test-001` node was created with `node_type: fragment` and `dimension: semantic`. The declarative adapter's emit block is live code. This contradicts the prior field note's claim that the spec's adapter block was scaffolding — the block WAS exercised; I just wasn't routing to it. Correction applied.

However: the emit block only creates a bare node. No tags, no concepts, no edges. A realistic consumer adapter needs a richer emit sequence (create the fragment node, create concept nodes per entity, connect them with tagged_with) for any downstream enrichment to have signal to operate on. The minimum-spec I drafted produces isolated nodes that cannot be enriched.

**Finding 3 — Dimension mismatch between the content adapter and my declarative spec.**

The content adapter puts fragments in `dimension: structure`. My declarative spec put them in `dimension: semantic`. Both now coexist:

```
fragment:e614a910... → node_type: fragment, dimension: structure  (content adapter)
fragment:test-001   → node_type: fragment, dimension: semantic   (declarative spec)
```

Two fragments with the same node_type but in different dimensions. Whether this matters for query and enrichment behavior is itself a design question — I declared `semantic` somewhat arbitrarily when drafting. The dimension surface is not well-surfaced in the spec grammar; new consumers might pick a dimension without understanding the consequences. Worth a note in any onboarding documentation.

**Finding 4 — Untagged prose produces isolated fragment nodes with zero structural signal.**

Ingesting `{"text": "A small piece of untagged prose..."}` via the content adapter created a fragment node, populated the text property, and stopped. No tags → no tagged_with edges → no concept nodes → no co-occurrence → no may_be_related → no lens translations. The fragment is structurally isolated in the graph: present but unreachable by any concept-centric query.

This is the single most important finding for Plexus's current value proposition: **in the default Homebrew build, untagged content produces nothing useful.** The "extraction layer does the real work" premise is aspirational against what the default build actually provides. The real work requires either:

1. Ensemble-driven concept extraction (Ollama running, declarative adapter with `ensemble:`, or external enrichment)
2. `--features embeddings` rebuild (similarity-driven structure)
3. Consumer-supplied tags (tautology)

A consumer ingesting a body of prose (papers, fragments, notes) into a default-installed Plexus will see their fragments sit as isolated nodes in a sea of nothing. The signal they expected will not arrive. They will conclude Plexus doesn't work, and they'll be right — for the default build. The architectural story is only true of builds with explicit feature flags and/or running ensemble infrastructure.

**Finding 5 — The prior crawl's "success" was an artifact of choosing tagged content.**

Had I ingested realistic prose (an essay, a paper abstract, a captured fragment as a user might actually type it), the crawl would have produced zero edges and the tautology threshold critique would have been self-evident. The fact that the first crawl "worked" is itself evidence of the value-validation gap: I had to supply tags for anything to happen. In real use, users don't tag prose with thematically-coordinated overlapping labels. The minimum-useful Plexus setup is further from the default-install-and-ingest experience than the documentation suggests.

**Implications:**

1. **Default pipeline truthfulness** — `with_default_enrichments()` should either: (a) not install enrichments it can't make work (remove DiscoveryGap when embeddings are off; remove TemporalProximity until an adapter writes `created_at`), or (b) document with brutal clarity that three of four are inactive by default. Current behavior is a silent trap.
2. **Homebrew build feature flags** — the release pipeline should either enable `embeddings` by default or produce a second build variant that does, since the out-of-the-box experience is materially different with and without.
3. **Adapter onboarding** — minimum-viable specs need tagged-content examples to produce any signal at all. This should be stated, not hidden. Or the content adapter should ingest prose through an optional in-process concept extractor, so first-run users see something happen with untagged text.
4. **The "consumers own specs" narrative needs a companion "consumers need X wired for specs to add value"** — currently, spec-authoring documentation treats the lens as if it'll translate rich structure, but the rich structure depends on infrastructure most first-time users won't have. The spec is necessary but not sufficient.

These are substantive, consequential findings about the gap between Plexus's advertised behavior and its default-install behavior. None of them were visible from the first ceremonial crawl. Real crawl earns real findings.
