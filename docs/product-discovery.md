# Product Discovery: Plexus

*2026-03-25 (updated from 2026-03-16 original; query surface research cycle)*

## Stakeholder Map

### Direct Stakeholders

**Consumer application developers** — build applications that ingest domain-specific data into a knowledge graph and take action based on the graph's evolving state. They interact with Plexus through an adapter (Rust or YAML spec) and a transport (MCP, gRPC, direct embedding). They need Plexus to handle graph mechanics (contribution tracking, enrichment, persistence, normalization) so they can focus on domain-specific extraction and domain-appropriate responses to structural signals. Actions vary by domain: surfacing latent connections, building outlines from accumulated fragments, coordinating research across shared contexts, or triggering real-time responses to structural changes. The graph itself may or may not be visualized directly. Example consumers from the research corpus: Trellis (creative writing scaffolding), EDDI (interactive performance), Carrel (research coordination), Manza (code analysis), Sketchbin (multimedia metadata).

**Domain-specific extractor authors** — write the extraction logic that produces structured JSON from raw source material (scripts, LLM prompts, Rust functions). They don't need to understand graph structure. They write against the extractor contract: raw input in, structured JSON out. The declarative adapter spec (YAML) maps their output to graph structure.

**The engine developer** — maintains and extends Plexus itself. Needs the architecture to be navigable, the module boundaries to be clear, and the test suite to verify invariants. The primary stakeholder for the system design and this product discovery artifact.

### Indirect Stakeholders

**End users of consumer applications** — writers using Trellis, performers using EDDI, researchers using Carrel. They never interact with Plexus directly. They experience it through the consumer application's actions on graph state — surfaced insights, suggested connections, structured outlines, responsive environments. Their needs are mediated by the consumer developer's adapter and application logic.

**Collaborators in federated contexts** — users who share a context across Plexus instances. They contribute data independently; the graph converges through deterministic concept IDs and replication. They experience shared understanding without shared source material. (Federation is designed but not yet implemented.)

## Jobs and Mental Models

### Consumer Application Developer

**Jobs:**
- Ingest my domain's data and get it into the graph with provenance — without learning graph internals
- Receive structural signals back that I can act on in my domain's language (not raw graph mutations)
- Trust that evidence from multiple sources strengthens the signal automatically
- Extend extraction to my domain without writing Rust (via YAML adapter specs and llm-orc ensembles)
- Deploy Plexus in my application's context — as an embedded library, a dev tool, or a service — without Plexus dictating storage location or runtime constraints
- Query the graph on my own schedule and get results in my domain's language — not raw graph data, but connections and discoveries expressed through my lens
- Discover unexpected connections across domains — things I didn't know to look for, surfaced in terms I understand
- Understand why a connection exists — what evidence supports it, how corroborated it is, where it came from

**Mental model:**
"I set up the I/O — configure an adapter for my domain, optionally add an extractor via `.llm-orc/` — and Plexus derives structure from whatever I send it. The connections it finds are my signal. I decide what to do with those signals in my application: surface insights, build outlines, trigger responses, or something else entirely.

When I query the graph, I look through my lens — a definition of how I want to receive data, in my domain's vocabulary. Cross-domain discoveries don't get hidden; they get translated into terms I understand. I control when I look (a CRON job, a user action, a scheduled check) and the lens shapes what I see."

The consumer developer thinks in terms of their domain (fragments, gestures, code files, citations), not graph primitives (nodes, edges, contributions). They expect the adapter to be the single artifact they need to understand. The setup should be relatively easy. Everything behind the adapter — sink mechanics, enrichment loops, scale normalization — is invisible infrastructure.

There are two interaction paradigms: **push** (Plexus emits events through the adapter's outbound side; the consumer reacts) and **pull** (the consumer queries on its own schedule through its lens; the consumer controls timing and question). Many real consumer workflows are pull — a CRON job checking "what new connections emerged since yesterday, in my vocabulary" is fundamentally pull. The lens concept lives primarily in the pull paradigm: the consumer looks through it when they choose to look.

### Domain-Specific Extractor Author

**Jobs:**
- Process raw source material into structured concepts and relationships
- Not need to know how the graph works — just produce JSON that the mapper understands
- Use the tools I know (Python scripts, LLM prompts) rather than learning a new framework
- Have my extraction logic compose with Plexus's enrichment loop — my extracted concepts should trigger co-occurrence detection, bridging, and discovery gaps automatically

**Mental model:**
"I write an extractor (script or LLM prompt) that understands my source format. I write a YAML spec that maps my extractor's output to graph structure. Plexus handles the rest — contribution tracking, normalization, enrichment, persistence."

### Engine Developer

**Jobs:**
- Navigate the codebase quickly — know which module owns which responsibility
- Trust that tests verify invariants, not just exercise code paths
- Make structural changes (refactoring) with confidence that behavior is preserved
- Understand why each architectural decision was made (provenance to ADRs and essays)

**Mental model:**
"The system has clear layers: graph primitives at the bottom, adapter/enrichment pipeline in the middle, transports at the top. Changes in one layer don't ripple to others. The domain model and system design are the authoritative maps."

## Value Tensions

- **Abstraction vs. domain fidelity:** Plexus must be general enough to serve any content domain (writing, movement, code, research), yet the value it provides to each domain depends on domain-specific extraction quality. How much domain-specific logic should live inside Plexus (as adapters) vs. outside (as extractors)?

- **Immediate value vs. deep extraction:** Rust-native phases provide instant feedback; LLM-based phases provide richer semantic understanding but take seconds and may be unavailable. The system bets that layered, incremental value is better than all-or-nothing — but this means consumers must handle a graph that enriches asynchronously over time.

- **Library autonomy vs. shared infrastructure:** The library rule (Invariant 41) gives consumers full control over storage and deployment. But federation requires coordination infrastructure that a pure library can't provide. How much coordination logic belongs in the library vs. in a separate coordination layer?

- **Evidence accumulation vs. noise tolerance:** Multi-source reinforcement strengthens signal, but it also means that systematic bias in extractors compounds. Evidence persists until explicitly retracted (Invariant 10). This is by design — the system is async, and a user returning after a year should find their graph intact. The question is whether the retraction mechanism (ADR-027) is sufficient for consumers who need to prune, or whether additional hygiene tools are needed.

- **Provenance completeness vs. ingestion throughput:** The dual obligation (Invariant 7) requires every write to carry both semantic content and provenance. This is epistemologically sound but adds overhead to every ingest call. At what scale does this become a bottleneck?

- **Scope vs. serendipity:** *(Added 2026-03-25, query surface research)* A consumer wants to see its domain clearly — Trellis queries about writing, not raw research metadata. But the entire value of a shared graph is cross-domain discovery. The lens-as-enrichment resolves this toward translation rather than filtering: the lens creates domain-translated relationships at write time, so cross-domain discoveries arrive pre-translated into the consumer's vocabulary. The consumer sees everything, expressed in terms they understand. The remaining tension: how much translation fidelity is possible? Some cross-domain connections may resist translation into a specific domain's vocabulary — the concept may not have a natural analog. How a lens handles untranslatable connections (surface them raw? Omit them? Create a generic "cross-domain" relationship?) is a design question for DECIDE.

- **Query simplicity vs. query power:** *(Added 2026-03-25, query surface research)* The write-heavy/query-light pattern promises a thin query surface — few primitives, intelligence at write time. But composing those primitives for real consumer queries (provenance-filtered traversal ranked by evidence diversity through a lens) is not simple. How much composition should Plexus handle versus exposing primitives for the consumer (or an LLM intermediary) to compose?

## Assumption Inversions

| Assumption | Inverted Form | Implications |
|------------|--------------|-------------|
| Domain-agnosticism is achievable | What if some domains are fundamentally incompatible with a shared graph model? | Movement gestures and literary fragments share no vocabulary. If cross-domain bridging never produces useful signal, the shared namespace is overhead, not value. This is the starting point and aspiration — text-based domains are the clear first milestone. EDDI (movement/performance) is a later validation case. Open to this not playing out for all domains. |
| Tags are a primary input signal | What if users don't tag at all? | Tags are one form of semantic data among many — no more special than any other ingested content. The system's purpose is to derive structure from unstructured input (via extraction: SpaCy NER, LLM-based concept extraction, co-occurrence). Tags supplement but should not be required. If the enrichment chain depends too heavily on tags specifically, domains without tagging vocabularies get no value. |
| Plexus surfaces connections; consumers decide what to do | What if consumers need Plexus to go further — summarize, recommend, or generate? | The architectural stance is that Plexus derives structure and surfaces connections; consumer applications decide how to act on them (Essay 04's "mirror, not oracle" framing, implicitly reinforced by the enrichment loop architecture). But some consumers may need Plexus itself to produce higher-order outputs — summaries, recommendations, generated content. The current architecture doesn't serve these jobs directly; consumers would need to build that layer themselves on top of graph queries. |
| Evidence accumulates but never decays | What if stale evidence actively misleads? | By design, a user can return after a year and find their graph intact — async, long-lived use is a core use case. Temporal decay would only make sense in session-based contexts (e.g., EDDI performances). For most consumers, persistence is the feature. The question is whether retraction tools (ADR-027) are sufficient for consumers who do need pruning. |
| LLM extraction is optional | What if the Rust-native extraction provides so little value that users skip Phase 1 and wait for Phase 3? | The phased model assumes Phase 1 provides immediate, meaningful value. If Phase 1 is just file registration metadata, consumers might perceive the system as slow (waiting for Phase 3) rather than fast (Phase 1 is instant). |
| Federation is peer-to-peer | What if most real deployments are hub-and-spoke (one shared server, many clients)? | The CRDT-aligned replication design optimizes for autonomous peers. A centralized model would be simpler, require less coordination, and cover the majority use case. The peer-to-peer design may be over-engineering. |
| One user per instance (for now) | What if multi-user access on a single instance is needed before federation? | The current MCP server has no authentication or authorization. Adding multi-user to a single instance is a different problem than federation — it needs access control, not replication. |
| The query surface should be thin (few primitives, intelligence at write time) | What if consumers actually need rich query-time computation? | The write-heavy/query-light pattern works for Graphiti's single-consumer case. Multi-consumer convergence + provenance-scoped queries + evidence diversity ranking may demand more query-time intelligence than the "thin" framing admits. The question is whether "thin" means few primitives or simple composition — Plexus may need few primitives but rich composition. *(Added 2026-03-25)* |
| A lens is a read-side concern separate from the write-side adapter | What if the lens IS a write-side concern — an enrichment? | **Validated by product conversation.** The lens is an enrichment that translates cross-domain graph content into one consumer's domain vocabulary at write time. This preserves write-heavy/query-light, keeps the three extension axes (Invariant 40), and means each consumer's lens makes the graph richer for all consumers. The open question shifts from "read-side vs. write-side" to "what does the lens enrichment contract look like?" — how does a consumer declare their translation rules alongside their adapter spec? *(Added 2026-03-25, resolved toward write-side)* |
| Cross-domain discovery requires explicit consumer action | What if the most valuable discoveries happen when users aren't looking for them? | If discovery requires the consumer to switch modes or issue special queries, users may never discover cross-domain connections. Push-based discovery signals — "this new connection crosses your domain boundary and is highly corroborated" — might be more valuable than pull-based discovery queries. The push paradigm (outbound events) already exists; the question is whether discovery events should be a first-class event type. *(Added 2026-03-25)* |

## Product Vocabulary

| User Term | Stakeholder | Context | Notes |
|-----------|-------------|---------|-------|
| fragment | Consumer developer | "I'm sending a fragment to Plexus" — a piece of writing, an annotation, a note | The minimum unit of content. Not a graph term — the user doesn't think in nodes. |
| tag | Consumer developer, end user | "I tagged this with #travel" — a human-applied label | One form of semantic data among many. Tags are additional signal but not assumed to be present — Plexus derives structure from unstructured input via extraction. Not more special than any other ingested content. |
| connection | End user, consumer developer | "Plexus found a connection between X and Y" | The user-facing word for what the system models as edges with contributions and normalized weights. Connections are the primary signal to consumer applications — the basis for whatever action the consumer takes. Users don't think in weights. |
| chain | Consumer developer | "Create a research chain for this project" | A grouping of related marks. Users think of it as a project or trail, not a provenance-dimension node. |
| mark | Consumer developer | "Mark this passage" — annotate a location in a file | A provenance record. Users think of it as a bookmark with notes, not a graph node. |
| discovery | End user, consumer developer | "Plexus discovered that these two themes are related" | The user-facing word for cross-domain or unexpected connections surfaced by the graph. Includes both intra-domain discoveries (connections within one consumer's data) and cross-domain discoveries (connections between consumers' data, translated through the consumer's lens). *(Updated 2026-03-25 — expanded from enrichment output to include cross-domain query results)* |
| extraction | Consumer developer | "Run extraction on this file" | The phased process of turning a file into graph structure. Users think of it as "analyze this file," not as three-phase adapter dispatch. |
| adapter | Consumer developer | "I need to write an adapter for my data" | The extension point. Consumer developers think of it as a plugin or connector, not a bidirectional integration contract. |
| spec | Extractor author | "I wrote a YAML spec for my extractor" | The declarative adapter specification. Authors think of it as a configuration file, not a runtime-interpreted adapter. |
| analyze | Engine developer, consumer developer | "plexus analyze my-context" | On-demand external enrichment via llm-orc. Users think of it as "run analysis," not "execute external enrichment ensemble and re-ingest results." |
| lens | Consumer developer | "Define a lens for how Trellis sees the graph" — the consumer's definition of how cross-domain content translates into their domain vocabulary | A write-time enrichment, not a read-time filter. The lens operates over the whole graph (like any enrichment) and creates domain-translated relationships — e.g., "this-research-concept-relates-to-writing-theme." Translation happens at enrichment time, so by the time the consumer pulls, the graph already speaks their language. Lens output is public: other consumers can see the translated relationships. This means each consumer's lens makes the graph richer for everyone — cross-domain pollination compounds with each consumer added. Defined alongside the adapter spec (the adapter already knows the domain vocabulary). Preserves Invariant 40 — the lens is an enrichment, not a fourth extension axis. *(Added 2026-03-25)* |
| corroboration | Consumer developer, end user | "How corroborated is this connection?" — how many independent sources support it | User-facing name for evidence diversity. "Corroboration" is closer to how users think about trust than "evidence diversity count." Four different kinds of evidence are more trustworthy than a hundred of the same kind. *(Added 2026-03-25)* |

## Product Debt

Assumptions baked into the architecture that may not match actual user needs.

| Assumption | Baked Into | Actual User Need | Gap Type | Resolution |
|------------|-----------|-----------------|----------|------------|
| All consumers need provenance | Invariant 7 (dual obligation) | Some consumers may want lightweight ingestion without provenance overhead | Over-abstraction | Validate: do any real consumer workflows skip provenance? If so, consider a "lightweight ingest" path. Deferred — no consumer has requested this. |
| Tags are a significant input signal | *(TagConceptBridger removed)* co-occurrence enrichment chain | Tags are one input among many — users may not tag at all. Plexus should derive structure from unstructured input via extraction (SpaCy, LLM). | Over-reliance on single input type | **Resolved:** TagConceptBridger was removed from the codebase — tag bridging is domain-specific. Extraction (NER, concept extraction, co-occurrence) and embedding similarity (ADR-026) provide structure without tags. Domains that use tags and need bridging implement their own adapter. The enrichment chain no longer depends on tags specifically. |
| Outbound events are sufficient feedback | Adapter's transform_events() | Consumers may need richer query-time feedback (e.g., "what changed since my last query?") beyond the event stream | Missing workflow | Event cursors (OQ-8) are the solution — persistent sequence-numbered event log in SQLite, pull-based "changes since N" queries. Without cursors, the pull paradigm forces Plexus-as-server (consumer must be listening). With cursors, the graph is just a file — write, walk away, come back, query. In scope for this cycle. *(Updated 2026-03-25)* |
| MCP is the right transport for interactive use | mcp/ module; stdio transport | MCP works well for LLM-mediated use but poorly for direct app-to-app integration without an LLM host | Over-abstraction of transport needs | Essay 09 identified this: gRPC is better for app-to-app. MCP is one transport; others are needed for non-LLM consumers. |
| Single-context operation is the default | MCP server's active_context model | Some workflows may need cross-context operations (compare two projects, merge contexts) | Missing workflow | Meta-context (read-only union query) is designed but not implemented. Cross-context writes are not designed. |
| Push (events) is the primary feedback paradigm | Adapter's `transform_events()`, outbound event model | Many consumer workflows are pull-based — a CRON job checking for new connections, a user-initiated query, a scheduled analysis. Without event cursors, pull forces Plexus into an always-on server role (consumer must be listening when events fire). With cursors, the graph is just SQLite on disk — consumers write, walk away, come back, query "what's new since sequence N?" | Missing workflow | Event cursors are the enabling infrastructure for the pull paradigm. Without them, Plexus-as-library is limited to write-only; consumers must run Plexus-as-server to get feedback. Cursors preserve the library rule (Invariant 41) for read workflows. In scope for this cycle. *(Added 2026-03-25, updated after product conversation)* |
| Query results are raw graph data | `PlexusApi` query methods return nodes, edges, paths | Consumers need results expressed in their domain vocabulary — not "node concept:travel has normalized weight 0.73" but "the travel theme is strongly connected to your provence fragments." | Mental model mismatch | The lens-as-enrichment approach: domain-translated relationships are created at write time, so query results already contain domain-vocabulary structure. The gap narrows to: do the existing query primitives (`find_nodes`, `traverse`, `evidence_trail`) plus lens-created structure give consumers enough to work with? Or do queries still need a translation layer for results that the lens didn't pre-translate? *(Updated 2026-03-25 — lens-as-enrichment partially resolves this)* |
