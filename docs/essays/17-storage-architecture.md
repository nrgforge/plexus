# Where Knowledge Lives: Storage Architecture for a Knowledge Graph That Spans Modes, Hosts, and Trust Boundaries

## The Problem Has Three Shapes

ADR-002 was deferred with a single question: should Plexus's SQLite database live in the project working directory (discoverable but polluting) or somewhere centralized (clean but needing project identity)? That question assumed one deployment mode — Plexus as a local dev tool, one database per project directory.

The question has grown. Plexus now faces at least three deployment modes with fundamentally different storage requirements, plus a fourth that sits between them:

**As a dev tool** (MCP server for Claude Code), Plexus runs locally, one context per project. The immediate question is path management — where does `.plexus.db` go so it doesn't pollute the project directory but remains discoverable?

**As an embedded library** (Sketchbin's local semantic engine), Plexus runs inside an application. The database is part of the application's data, not a dev tool artifact. The host application owns the storage path. Sketchbin's BYOS (bring-your-own-storage) architecture means SQLite for the graph, S3/R2 for media — and Plexus doesn't get to have opinions about where either lives.

**As a managed service** (an AGPL-licensed commercial deployment), Plexus runs on a server — managed by the user or by a hosting provider. The database path comes from deployment config, environment variables, or orchestration tooling. This mode sits between the single-user dev tool and the fully federated case: multiple users can connect to a shared instance without needing peer-to-peer replication. A managed Plexus server could serve as the hub for a research team or development group, providing shared contexts without requiring every participant to run their own engine.

**As a federated engine** (shared contexts across users on different hosts), Plexus's data replicates. An artist collective, a research team, or a development group shares a Plexus context. Each member holds a local replica. Emissions propagate between hosts. The database isn't a single file anymore — it's a distributed data structure with consistency requirements.

These shapes demand a storage architecture that scales from "where does the file go?" through "how do multiple apps share it?" to "how does it replicate across the internet?" The architecture must serve all without the simplest case paying for the complexity of the most ambitious one.

## The Library Rule

The clearest finding from the research is a negative one: **Plexus the library should never decide where to store data.**

Every embeddable database library follows this pattern. sled takes a path. redb takes a path. SurrealDB takes a path. The Rust `directories` crate can compute XDG-compliant platform-specific paths, but the library doesn't call it — the host application does.

The MCP server ecosystem confirms this from the transport side. Anthropic's MCP Memory Server tried to manage its own storage path through environment variables and broke — the NPX subprocess didn't receive the variable, data ended up in a temp directory, and updates destroyed it. Clawmarks stores `.clawmarks.json` in the project root, which works but pollutes the project directory. Anthropic's MCP SQLite server takes a path parameter and has no opinions. The tools that try to be clever about paths break. The tools that accept a path work.

Obsidian provides the architectural precedent: vault data lives wherever the user places it, vault config lives in `.obsidian/` inside the vault, and global app config lives in the OS-standard application support directory. Obsidian doesn't decide where your vault lives. But it manages its own state centrally.

The rule for Plexus: `GraphStore` takes a path (or connection). The transport/host layer decides what to pass. The MCP server picks the path based on XDG conventions and project identity. Sketchbin picks the path based on its own architecture. A managed server picks the path from deployment config. A future gRPC server picks the path from its own configuration.

This aligns with invariant 38 (transports are thin shells) and invariant 40 (adapters, enrichments, and transports are independent dimensions). Storage location is an infrastructure concern. It belongs in the host layer, not the engine.

### Where the MCP Server Should Put It

For the immediate dev-tool case, the XDG Base Directory Specification provides the answer. The spec distinguishes between data and configuration: `$XDG_DATA_HOME` (default `~/.local/share/`) is for persistent user data like databases, while `$XDG_CONFIG_HOME` (default `~/.config/`) is for settings files — things a user might hand-edit or version control. A Plexus SQLite database is data, not configuration: it's machine-generated, binary, and not meaningfully editable. It belongs in the data directory. (A hypothetical `plexus.toml` with user preferences would belong in `~/.config/plexus/` — but that's a separate concern from where the graph lives.)

The Rust `directories` crate handles cross-platform mapping:

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/plexus/` |
| macOS | `~/Library/Application Support/plexus/` |
| Windows | `C:\Users\X\AppData\Roaming\plexus\data\` |

Project identity within that directory can use path hashing (deterministic, breaks on rename) or an explicit project ID (survives renames, requires init). Path hashing is simpler and matches git's worktree pattern. The database path becomes `~/.local/share/plexus/{hash}/plexus.db`, where `{hash}` is a deterministic function of the project's absolute path.

> **Superseded by ADR-016:** The per-project hash approach was rejected. A single centralized database at `~/.local/share/plexus/plexus.db` holds all contexts. Contexts — not project directories — are the organizational unit. Per-project databases actively prevent cross-project context sharing, which is the motivating scenario for ADR-017. The MCP server creates or selects contexts by name; the same context is accessible from any project directory.

This removes `.plexus.db` from the project directory. The MCP server resolves the path at startup.

## Sharing Contexts Across Applications

The path question was the easy part. The harder question: how do multiple applications share the same Plexus contexts?

The user's scenario is concrete: a "network-research" context accessed by Carrel (paper annotations), Manza (code analysis), and Trellis (reflective writing fragments). The context isn't per-application — it's a curated lens across applications. Each tool contributes to the same semantic landscape through its own adapter, and the shared concept graph reveals connections none of them could discover alone.

Two local architectures can support this, and a third option removes the multi-app coordination problem entirely:

### The Daemon Model

A `plexus serve` process running a single PlexusEngine with one SQLite store. Multiple applications connect via IPC, HTTP, or gRPC. Ollama established this pattern — a local service on a fixed port, managed by launchd or systemd, with a REST API that any client can use.

For Plexus: the daemon holds the single in-memory DashMap cache, runs the single enrichment loop, and manages persistence. The MCP server connects as one client. Sketchbin connects as another. Carrel, Trellis, and Manza all talk to the same daemon and see the same contexts.

The advantages are significant: no stale cache problem (one cache, one process), no duplicate enrichment processing (one loop), automatic context sharing (all clients see all contexts). The separation is clean — the daemon owns data and intelligence, applications own UI and domain logic.

The disadvantages are real: runtime dependency (applications fail if the daemon isn't running), deployment complexity (managing a background service), loss of simple embeddability (Sketchbin can't just `use plexus;` and call functions in-process), and IPC latency.

### The Shared-DB Model

All applications open the same SQLite file directly, each running their own PlexusEngine instance. SQLite's WAL mode supports concurrent reads and serialized writes on the same host. For Plexus's persist-per-emission model, Trellis and Carrel writing to the same file would serialize at the SQLite level. Reads proceed concurrently without blocking.

This is simpler to deploy — no daemon, no service management. But it introduces the stale cache problem: Engine A writes, Engine B's in-memory DashMap doesn't see the change until reloaded. SQLite's `data_version` pragma can detect changes (the value increments on each write), enabling a cache invalidation strategy: check `data_version` before reads, reload if changed. This adds read latency but preserves the embedded model.

The bigger issue is enrichment coordination. Two engines running enrichments independently could produce duplicate work, and their enrichment registrations might differ. Engine A has CoOccurrenceEnrichment registered; Engine B doesn't. The shared context would have inconsistent enrichment state depending on which engine processed each emission.

### The Managed Server

A third option avoids the local-coordination problem entirely: a Plexus server running on infrastructure — self-hosted or managed. This is the daemon model deployed remotely, with clients connecting over HTTP or gRPC rather than local IPC. For a research team at an institution or a development team with shared infrastructure, a managed server is the natural choice: one canonical store, one enrichment loop, accessible from any machine on the network.

This mode becomes especially relevant under an AGPL commercial license, where a managed tier provides the shared infrastructure without requiring every participant to install and configure Plexus locally. The `GraphStore` trait doesn't change — the server wraps it in a service layer, exactly as the local daemon would.

### The Pragmatic Sequence

Start with the shared-DB model — it works today with minimal changes. Add `data_version` polling for cache coherence. Accept that enrichment coordination is imperfect for now (in practice, if applications register the same enrichments, idempotency handles the duplication).

One prerequisite: the current `save_context()` implementation does a full replace (DELETE all rows, INSERT all rows). This is safe for single-engine use but destructive under concurrent multi-engine writes — Engine A's full replace would overwrite Engine B's recent commits. The shared-DB model requires changing persistence to incremental upserts (save individual nodes and edges) rather than full-context replacement. The individual `save_node()` and `save_edge()` methods already use upsert semantics; the change is to stop using the bulk-replace path for shared databases.

Move to the daemon model when enrichment coordination or cache coherence becomes a real problem, not a theoretical one. The daemon model is strictly more capable but operationally heavier. The `GraphStore` trait doesn't need to change for either model — it already takes a path and provides load/save operations. The daemon would wrap a `GraphStore` in a service layer; the shared-DB model uses `GraphStore` directly.

The managed server becomes relevant when the user base extends beyond a single machine — when contexts are shared across a team rather than across applications on one person's laptop.

## Context Boundaries and Cross-Context Awareness

Within a single Plexus instance (whether daemon or shared-DB), contexts are deeply isolated. Five architectural barriers enforce this: edges validate endpoints within a single context, the DashMap partitions by context ID, enrichments receive a single context snapshot, all engine operations are scoped to a context ID, and the adapter pipeline binds to a context on each ingest call.

This isolation is a feature, not a limitation. Contexts are the unit of semantic coherence — a bounded subgraph where cross-dimensional edges connect and enrichments discover structure. Breaking this boundary would compromise the normalization scope, the enrichment termination guarantees, and the clean separation between what different lenses reveal about the same material.

But users work across contexts. The same document can be a source in "network-research" and "distributed-ecologies-short-fiction." The same concept — "distributed-systems" — appears in both. Can the system surface this?

Three options exist, with increasing invasiveness:

**Shared-concept convergence** requires zero changes. Plexus uses deterministic concept IDs: `concept:{lowercase_tag}`. If the same tag appears in two contexts, the concept node has the same ID in both. A new query method on PlexusEngine — `shared_concepts(context_a, context_b)` — returns the intersection. This is a property of the ID scheme, surfaced at query time. It discovers exact tag matches but not semantic similarity.

**Meta-contexts** are read-only views that union nodes and edges from multiple constituent contexts. When you query a meta-context, it traverses across all constituent contexts. No new data is stored — it's query-time composition. No enrichment can run on a virtual view (enrichments produce emissions, which need a target context), but read-only traversal works. This requires modest engineering — a virtual context type that delegates reads to its constituents.

**Cross-context enrichment** would modify the enrichment trait to receive the engine instead of a single context, enabling enrichments to scan multiple contexts and propose connections. But where would cross-context edges live? They can't satisfy endpoint validation in either constituent context. This requires fundamental changes to the graph model — a separate RDD cycle, not a storage decision.

The pragmatic sequence: implement shared-concept convergence first (it works today with a new query method), evaluate whether it's sufficient, and pursue meta-contexts only if the user needs richer cross-context intelligence.

### Resonance: Cross-Context Discovery as a Network Effect

The Sketchbin semantic-discovery design sketches a more ambitious vision for cross-context awareness — one that doesn't break context isolation but creates value across context boundaries through what might be called *resonance*.

The scenario: Alice and Bob each have a Sketchbin instance with a "shared" context that they federate. Alice follows Bob. If Alice and Bob's shared contexts contain similar concepts — overlapping tags, convergent semantic landscapes — a meta-enrichment could surface content from Bob's network (people Bob follows) that resonates with Alice's work. Alice sees art from people she doesn't follow, surfaced not by social connection but by semantic similarity to her own creative practice.

This is not cross-context edge creation. The contexts remain distinct. What changes is that *features* of one context (its concept distribution, its tag vocabulary, its co-occurrence patterns) become inputs to a discovery process that operates across contexts. The semantic landscape of each context becomes a signal, not a boundary to cross.

This raises questions that extend beyond storage architecture into enrichment design and federation protocol: Is resonance a new type of enrichment — one that reads context summaries rather than context contents? Can it remain federated, with each instance computing its own resonance locally from replicated semantic summaries? How does this interact with the replication tiers — does semantic-only replication provide enough signal for useful resonance?

These questions warrant their own research cycle. The storage architecture needs to support this direction without committing to it — which it does, because resonance operates on context metadata and replicated semantic structure, not on cross-context edges or shared databases. The context boundary stays intact. The value flows through the federation layer, not through the graph model.

## Data Co-location: The Graph Is Not the Content

A question that surfaces naturally from shared and federated contexts: do the source documents — the papers, code files, sketches, and fragments that adapters process — need to live alongside the graph?

They do not. The graph stores *semantic structure and provenance*, not source content. When Carrel processes a paper, the adapter extracts concepts and tags, creating concept nodes and `tagged_with` edges. The fragment node records metadata — title, source identifier, adapter ID — but the paper itself stays wherever it was when the adapter read it. When a mark annotates a file location, the provenance chain records the file path and line number, but the file content is not copied into the graph.

This separation is fundamental, not incidental. Adapters are universal code — the same `MarkdownAdapter` or `LlmConceptAdapter` runs on any machine. But the *input data* they process is machine-specific. Alice's paper collection lives on Alice's laptop. Bob's codebase lives on Bob's workstation. When both contribute to a shared context, the graph accumulates their semantic output — the concepts, relationships, and provenance chains — without needing access to each other's source material.

The implications for federation are significant:

**Source content stays local.** In a shared research context, the graph records that Alice found concept "federated-learning" in a paper via her Carrel adapter, and that Bob found the same concept in a codebase via Manza. The co-occurrence enrichment discovers the connection. Neither Alice nor Bob needs the other's source material — the semantic structure carries the intelligence.

**Provenance references are local.** A mark's file path (`/Users/alice/papers/smith-2024.pdf:47`) is meaningful on Alice's machine and meaningless on Bob's. This is correct. The provenance chain answers "where did this knowledge come from?" with respect to the originator's environment. On a receiving replica, the provenance tells you *who* found it and *when*, even if you can't follow the file path. The adapter ID and source identifier provide the attribution; the local path provides the audit trail for the originator.

**Adapters don't need to be identical across collaborators.** Alice uses Carrel for papers; Bob uses Manza for code. Both contribute to the same context through different adapters. The adapter ID on each contribution distinguishes the source. The graph doesn't require or expect that every collaborator runs the same tools — it only requires that each tool produces well-formed emissions.

This means the graph is lightweight relative to the content it describes. A shared context for a research team might reference thousands of papers and hundreds of code files, but the graph itself stores only the extracted concepts, the relationships between them, and the provenance chains that record who found what. The content stays where it is because it's large, private, and machine-specific. The graph is smaller by comparison — though "small" is relative. A context with many fine-grained concepts, dense co-occurrence enrichment, and deep provenance chains could grow significantly. The ratio of graph size to content size depends on the granularity of concept extraction and the density of the enrichment layer. For most use cases the graph remains orders of magnitude smaller than the source material it describes, making it practical to replicate even when the content itself cannot move.

## Federation: When Contexts Cross Hosts

The previous section discussed shared contexts in the same-host sense — multiple applications accessing the same database. This section uses "shared context" in a stronger sense: a context that replicates across users on different hosts, each holding a local replica that converges through emission propagation. Same concept, different boundary.

The most ambitious storage question: how does a Plexus context replicate across users on different hosts and networks?

The scenario comes from the Sketchbin semantic-discovery design — a researched direction, not an existing implementation. The design envisions an artist collective creating a shared context. Each member's Sketchbin holds a local replica. When a member publishes a sketch, their adapter produces an emission against the shared context. That emission commits locally and propagates to other members. Receiving instances commit the emission to their local replica and run the enrichment loop.

But the scenario extends beyond Sketchbin. Four distinct collaboration patterns emerged from the research:

**Research teams** share a context across a project. Researchers contribute via Carrel (paper annotations), Manza (code analysis), and Trellis (reflective fragments). The shared context discovers where their reading and thinking intersects.

**Artist collectives** share a context reflecting combined creative output. The semantic landscape maps the collective's creative territory — which members explore similar themes, where vocabularies diverge.

**Development teams** maintain shared understanding of a codebase. Developers annotate code and document architecture decisions. The shared context reveals how subsystems connect.

**Open discovery networks** share modular knowledge components — questions, claims, evidence — across institutions and tools. This is the Discourse Graph pattern, where Plexus could serve as automated infrastructure beneath manually-constructed research argument graphs.

All four patterns share a structural property: **the shared context is the collaboration primitive.** Not a shared document (Obsidian), not a shared annotation layer (Hypothesis), not a shared reference library (Zotero). A shared *semantic landscape* that each participant enriches through their own tools and practices. Content stays local; understanding converges.

What makes this distinct is not the sharing of semantic structure per se — graph databases and knowledge platforms already do that. It's the combination: local-first storage, adapter-mediated extraction from heterogeneous tools, enrichment-driven discovery, and selective replication that shares understanding without sharing content. Existing collaboration tools share content or commentary. Plexus shares the derived semantic structure that emerges from independent contributions, without requiring participants to use the same tools or expose their source material.

### Why the Data Model Is Naturally Suited for Replication

Plexus's data model has strong natural alignment with CRDT (Conflict-free Replicated Data Type) semantics — the formal framework for eventually consistent distributed data:

**Contributions are per-adapter LWW registers.** Each edge stores `HashMap<AdapterId, f32>` — independent last-writer-wins slots keyed by adapter ID. Two users contributing to the same edge produce contributions in separate slots. No conflict. This is structurally a LWW-Register Map, one of the best-understood CRDT patterns. This property depends on adapter IDs being unique per user-instance, not just per adapter type: Alice's Carrel adapter must have a different ID (e.g., `carrel:alice`) than Bob's (`carrel:bob`). If two users share the same adapter ID, their contributions would collide in the same LWW slot. Federation requires an adapter ID naming convention that includes user or instance identity — a prerequisite that the domain model should make explicit.

**Concept nodes use deterministic IDs.** `concept:{lowercase_tag}` means Alice tagging "ambient" and Bob tagging "ambient" on different hosts produce the same node ID. On merge, the node upserts. This is an add-only set — nodes converge via identity, not coordination.

**Emissions are self-contained.** The `Emission` struct — annotated nodes, annotated edges, removals — is a complete, validated bundle. It's the natural unit of replication.

**Enrichments are idempotent.** After receiving a replicated emission, the local enrichment loop can re-run safely. It either produces enrichments it already has (no-op via upsert) or discovers new co-occurrences from the merged data.

Where it's less clean: removals need tombstones (a deleted node could reappear when a replica that missed the delete replicates its add). Raw weight is derived state that needs recomputation after merge. Edge validation during async replication may encounter edges before their endpoint nodes arrive.

### Emission-Level Replication, Not Row-Level

Projects like cr-sqlite and Corrosion (Fly.io's Rust+SQLite+CRDT system) prove that row-level CRDT replication of SQLite databases works in production. Corrosion uses cr-sqlite with SWIM gossip over QUIC to replicate SQLite globally across thousands of nodes at Fly.io.

But row-level replication is wrong for Plexus. It replicates everything — every row in every table. The federated scenario requires *selective* replication: share the concept graph without sharing the source content. A researcher's shared context should reveal what concepts they're exploring, not expose their unpublished paper annotations.

Emission-level replication solves this. Each `sink.emit()` call produces a serializable emission. The replication layer intercepts the emission, filters it by policy, and ships it to peers. Three replication tiers emerged from the research:

**Semantic-only** (lightest, most privacy-preserving): Concept nodes, `tagged_with` edges, `may_be_related` edges, provenance chains and marks, `references` edges. Fragment text content is excluded. Use case: shared discovery context for a collective. Members see what concepts others explore, not the content itself.

**Metadata + semantic** (medium): Everything in semantic-only, plus fragment metadata (title, source type) but not full text. Use case: federated discovery. Enough to show "what this sketch is about" without hosting the content.

**Full replication** (heaviest): Everything. Use case: backup, migration, open research contexts where sharing everything maximizes composability.

The replication tier is a policy per shared context, not a global setting. A collective might share a semantic-only context for discovery and maintain private full contexts for individual work.

### The Replication Protocol

The emission travels from source to peers through a transport. For Sketchbin, the researched direction points toward ActivityPub as the federation transport — it's extensible via JSON-LD, and a Plexus emission could be serialized as a custom activity type. The trust network (follow/accept) provides access control. Neither Sketchbin's ActivityPub integration nor its federation layer is implemented yet — this is a design direction informed by research, not a description of existing infrastructure.

Two options for carrying emissions:

**ActivityPub itself** as the transport. Social interactions and emission replication flow through the same protocol. Simple to deploy (one federation stack), but ActivityPub is designed for social activities, not database replication — no ordering guarantees, no exactly-once delivery, inbox-polling latency.

**A purpose-built channel** alongside ActivityPub. Social interactions (follows, boosts, sketch previews) flow through standard ActivityPub. Emission replication flows through a Plexus-specific protocol optimized for convergence speed and ordering — potentially WebSocket, QUIC gossip (the Corrosion pattern), or a simple pull-based sync API.

The pragmatic approach: use the simpler option first (single transport), add a purpose-built channel when the ordering and latency requirements exceed what it can provide.

On the receiving end, the emission commits to the local replica through the existing engine pipeline: deserialize, validate (upsert handles convergent IDs, per-adapter contributions merge naturally), run the local enrichment loop. Only primary emissions replicate — enrichment-produced emissions are local, preventing feedback amplification across replicas.

A consequence of local-only enrichment: replicas with different enrichment configurations will produce different enrichment-derived structure. If Alice registers `CoOccurrenceEnrichment` and Bob registers `CoOccurrenceEnrichment` + a hypothetical `ClusterEnrichment`, their replicas will have different `may_be_related` edges and different clustering structure. This divergence is permanent — enrichment emissions don't replicate, so the enrichment-derived layers never converge across replicas. Whether this is acceptable depends on the use case. For the creative collective pattern, each member having their own enrichment "lens" on shared primary data may be a feature — different enrichment configurations reveal different structure in the same material. For the research team pattern, where everyone should see the same co-occurrence graph, it argues for standardized enrichment registrations within a shared context. The storage architecture doesn't resolve this; it's an enrichment coordination question for the federation protocol.

### What GraphStore Needs

The current `GraphStore` trait has the right methods for local persistence. For federation, an extension trait — `ReplicatedStore` — would add:

- **Emission journaling**: persist each emission with replication metadata (origin site, version vector) for sync and replay
- **Pull-based sync**: "give me emissions since version N" for catching up after reconnection
- **Remote emission merge**: apply a remote emission with conflict resolution and raw weight recomputation

This extension wraps a base `GraphStore` — the base trait stays simple for single-instance use. The daemon model and the shared-DB model both work with the base trait. Federation adds the extension. The simplest case doesn't pay for the complexity of the most ambitious one.

## Invariant Tensions

One tension with existing invariants:

**Invariant 38 (transports are thin shells)** says transports call `ingest()` and query endpoints without touching adapters, enrichments, or the engine. A replication transport has more responsibility — filtering emissions by tier, managing ordering, handling merge. This isn't "thin" in the same way an MCP tool handler is thin.

The resolution: replication is not a transport in the invariant-38 sense. It's **infrastructure** — not between the consumer and the engine, but coordinating between the engine and the storage backend. Consumers still interact through `ingest()` and query endpoints via their transport. The replication layer is invisible to them. Invariant 38 holds for consumer-facing transports; replication operates below that boundary.

The replication layer has two distinct responsibilities that sit at different levels of the stack. **Outbound** (journaling and shipping emissions to peers) wraps `GraphStore` — it intercepts persisted emissions, filters them by replication tier, and sends them to peers. This is purely a storage-level concern. **Inbound** (receiving and applying remote emissions) needs the engine, not just the store — remote emissions must go through validation, commit, and the enrichment loop to maintain invariant compliance. A dedicated `ingest_replicated(context_id, remote_emission)` path on the engine would handle this: validate, commit, run enrichments, but skip outbound replication (to prevent echo). The replication layer coordinates between these two levels without being either a transport or a store.

This distinction should be explicit in the domain model if federation is pursued. A new concept — *replication layer* — would handle emission journaling (store-level) and remote emission ingestion (engine-level). It's neither a transport (consumer-facing) nor a store (persistence), but a coordination layer that spans both.

## The Architecture in Three Layers

Bringing the research together, the storage architecture has three layers, each independently deployable:

### Layer 1: Local Storage (Now)

`GraphStore` takes a path. The MCP server resolves it via XDG conventions. Sketchbin passes it from its own config. Single SQLite file, single process.

**Changes needed:** Move the MCP server's default path from `cwd/.plexus.db` to `~/.local/share/plexus/plexus.db` (see supersession note above — single centralized database, not per-project). The `GraphStore` trait is unchanged.

### Layer 2: Shared Access (Near-term)

Multiple applications share a single SQLite file via the shared-DB model, or connect to a Plexus daemon. Contexts are shared across tools. Cross-context awareness via shared-concept convergence queries. A managed server deployment fits here — the same daemon model, deployed on infrastructure rather than locally.

**Changes needed:** `data_version` polling for cache coherence in the shared-DB model. A `plexus serve` daemon for the service model. A `shared_concepts()` query method on PlexusEngine. The `GraphStore` trait is unchanged.

### Layer 3: Federation (Future)

Emission-level replication across hosts. `ReplicatedStore` extension trait wrapping `GraphStore`. Replication tier policies per context. Purpose-built sync protocol (potentially alongside ActivityPub for Sketchbin's social federation).

**Changes needed:** `ReplicatedStore` trait, emission serialization, version vectors, tombstones for removals, replication tier filtering, sync protocol. Significant engineering, but isolated from the core engine — consumers, adapters, and enrichments are unaffected.

Each layer builds on the previous one without requiring it. Layer 1 works alone. Layer 2 adds sharing without requiring federation. Layer 3 adds federation without changing how local storage or shared access work. A user running Plexus as a solo dev tool uses Layer 1 only and never encounters the complexity of Layers 2 or 3.

## What the Shared Context Changes

The most important finding from this research isn't technical — it's conceptual. The shared context changes what Plexus is.

As a local dev tool, Plexus is a knowledge graph for one person's project. Useful, but bounded. As a shared context across applications, Plexus becomes a semantic meeting point — a place where different tools and different practices contribute to a common understanding. As a federated context across users, Plexus becomes infrastructure for collaborative sensemaking — a system that discovers connections across independent contributions without requiring coordination or consensus.

Existing collaboration tools share content (documents, references, files) or commentary (annotations, discussions). Plexus shares derived semantic structure — the concepts, relationships, and co-occurrence patterns that emerge from work done in whatever tools people already use. The local-first, adapter-mediated model is what makes this distinctive: the shared context doesn't require anyone to change their workflow or expose their source material. It reveals what their independent workflows have in common.

The data that makes this possible is lightweight relative to what it describes. The graph stores concepts, relationships, and provenance — not the source material itself. A shared context doesn't require participants to share their documents, their code, or their creative work. It requires them to share what their tools discovered in those materials. The semantic landscape is practical to replicate because it's compact compared to the content it represents. The content stays local because it belongs to whoever created it.

This is the storage question that matters. Not "where does the file go?" — that's a path resolution problem, solved by XDG conventions and the library rule. The storage question that matters is: **what does it mean for knowledge to live in a place that multiple people, tools, and hosts can contribute to and learn from?** The answer is the shared context — a semantic landscape that accumulates intelligence from independent sources and makes that intelligence traversable by anyone with access.

## Future Research

Several questions surfaced during this research that extend beyond the storage architecture into enrichment design, federation protocol, and graph semantics. Each warrants its own investigation:

**Resonance and cross-context discovery.** The Sketchbin vision of a "resonance feed" — surfacing content from across a social graph based on semantic similarity between contexts — raises questions about meta-enrichment (enrichments that operate on context summaries rather than context contents), federated computation of similarity scores, and whether semantic-only replication provides sufficient signal for useful discovery. This could be the subject of a dedicated research cycle on federated enrichment.

**Opposition and contradiction as relationship types.** Research question 5 (preserved in the research log) explored what `opposes`/`contradicts` edges would add as an exploration layer. The architecture already supports this via multigraph edges, and the weight model (ADR-003) correctly separates contribution strength from relationship polarity. A `TensionEnrichment` that detects sentiment divergence or contradictory evidence, and a QBAF-inspired bipolar normalization strategy, are natural extensions. This belongs in a research cycle on enrichment design, not storage architecture.

**Defederation and data retraction.** When a user leaves a shared context, their contributed nodes and edges remain (committed data). Whether there should be a "retract" mechanism — and how it interacts with CRDT convergence, tombstones, and derived state like co-occurrence edges — is an open question for the federation protocol design.

**Context membership and access control.** How shared context membership is represented (context metadata, a separate registry, or delegation to the transport layer) and how access control interacts with replication tiers are protocol-level questions that the storage architecture leaves open by design.

**Version vectors and causal ordering.** The emission journal needs a versioning scheme. Lamport timestamps are simple but don't capture concurrent emissions from different users. Version vectors per-user would be more precise but heavier. The right choice depends on expected topology and scale, which will become clearer as Layer 2 matures.

**Removal semantics in federated contexts.** Plexus's `Emission.removals` deletes nodes, but in a CRDT context, deletions need tombstones to prevent reappearance. The expected removal frequency in practice — and whether a simpler "never delete, only supersede" policy would suffice — needs investigation.
