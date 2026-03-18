# Research Log: Plexus Storage Location (ADR-002)

## Question 1: What storage location conventions do local-first SQLite tools, MCP servers, and embeddable Rust libraries use?

**Method:** Web search + codebase inspection

### Findings

#### XDG Base Directory Specification

The [XDG spec](https://specifications.freedesktop.org/basedir/latest/) defines four categories:

| Variable | Default | Purpose |
|----------|---------|---------|
| `$XDG_DATA_HOME` | `~/.local/share/` | User-specific data files (databases, persistent state) |
| `$XDG_CONFIG_HOME` | `~/.config/` | User-specific configuration |
| `$XDG_CACHE_HOME` | `~/.cache/` | Non-essential cached data |
| `$XDG_STATE_HOME` | `~/.local/state/` | User-specific state (logs, history) |

A Plexus SQLite database is **data** (persistent, user-specific, not configuration) — it belongs in `$XDG_DATA_HOME`. On macOS, the equivalent is `~/Library/Application Support/`. The Rust [`directories`](https://docs.rs/directories) crate handles this cross-platform:

| Platform | `data_dir()` for Plexus |
|----------|------------------------|
| Linux | `~/.local/share/plexus/` |
| macOS | `~/Library/Application Support/com.plexus.Plexus/` (or simpler qualifier) |
| Windows | `C:\Users\X\AppData\Roaming\plexus\data\` |

#### MCP Server Ecosystem

**Clawmarks** (inspected locally at `/Users/nathangreen/.nvm/versions/node/v18.16.0/lib/node_modules/clawmarks/`): Uses **project-local** storage. `.clawmarks.json` in the project root, configurable via `CLAWMARKS_PROJECT_ROOT` env var. Simple JSON file, not SQLite. Same pattern as current Plexus.

**MCP Memory Server** (Anthropic's official): [Broken storage path handling](https://github.com/modelcontextprotocol/servers/issues/692). Attempts `MEMORY_FILE_PATH` env var but NPX subprocess doesn't receive it. Default falls to NPX temp directory (`~/.npm/_npx/[hash]/...`). Data gets lost on updates. **This is a cautionary tale about fragile path configuration.**

**MCP SQLite Server** (Anthropic's): No opinion on where the DB lives — the user provides a path in the config. It's a generic SQLite tool, not an application with its own data.

**Pattern:** MCP servers that own their own data (memory, clawmarks) use project-local storage. Servers that access user-provided databases take a path parameter. Both patterns exist. Neither follows XDG.

#### Obsidian

[Two-tier architecture](https://help.obsidian.md/data-storage):
- **Vault data**: lives wherever the user creates the vault (their folder, their choice)
- **Vault config**: `.obsidian/` subdirectory inside the vault
- **Global app config**: OS-specific app support directory (macOS: `~/Library/Application Support/obsidian/`, Linux: `$XDG_CONFIG_HOME/obsidian/`)

This is the clearest precedent: **vault (project) data is user-placed, app data is centralized.** Obsidian doesn't decide where your vault lives. But it does manage its own state centrally.

#### Embeddable Rust Database Libraries

All three major options take a path from the caller:

- **sled**: `sled::open("my_db")` — bare path, no opinion on where
- **redb**: `Database::create("my_db.redb")` — bare path
- **SurrealDB**: `Surreal::new::<RocksDb>("tempdb")` — path or in-memory

**Pattern: embedded libraries never decide where to store data. The host application provides the path.** This is the correct pattern for Plexus-as-library.

#### Project Identity Schemes

For centralized storage (mapping project → database), three approaches exist:

1. **Path hash** — `sha256("/Users/nathangreen/Development/plexus")` → deterministic but breaks on directory rename/move
2. **Explicit project ID** — user or tool creates a stable identifier (e.g., in a `.plexus` config file in the project root), stored centrally. Survives renames. Requires initialization step.
3. **Content-addressable** — git object IDs, Software Heritage SWHIDs. Not applicable here since Plexus contexts aren't content-addressable.

Git itself uses option 1 for worktrees (path-based lookup in a central registry).

### Implications

Three clear patterns emerge by deployment mode:

| Mode | Who decides the path? | Convention |
|------|----------------------|------------|
| **Library** (Sketchbin embedding Plexus) | The host application | Library takes a path parameter. Sketchbin decides. |
| **Dev tool** (MCP server) | The tool, following XDG | `~/.local/share/plexus/{project-id}/plexus.db` |
| **Server** (Sketchbin production) | The deployment config | Path from config file, env var, or CLI flag |

The key insight: **Plexus the library should never decide where to store data. The transport/host layer decides.** The GraphStore trait takes a path (or connection). The MCP server picks the path based on XDG + project identity. Sketchbin picks the path based on its own architecture. A future gRPC server picks the path from config.

This aligns with invariant 38 ("transports are thin shells") and invariant 40 ("adapters extend the domain side, enrichments extend the graph intelligence side, transports extend the protocol side — these three dimensions are independent"). Storage location is a transport/host concern, not an engine concern.

### Open question for next iteration

The multi-application context sharing the user described (same "network-research" context accessed by Carrel, Manza, and Sketchbin) implies a **single centralized store** that multiple applications connect to. This is different from both project-local and per-app centralized. It suggests:

- One PlexusEngine running as a service (or one shared DB file)
- Multiple transports (MCP, embedded, gRPC) connecting to the same engine
- The store is identified by **context names**, not by which application uses it

This pushes toward a daemon/service model or a shared SQLite file in a well-known location. Need to research this in the next iteration.

## Question 2: How should multiple applications share access to the same Plexus contexts, and what would cross-context enrichment mean?

**Method:** Web search + codebase inspection

### Findings

#### SQLite Concurrent Access

[SQLite WAL mode](https://sqlite.org/wal.html) supports one writer and many readers simultaneously. Multiple processes can share a single SQLite file if they're on the same host. Key constraints:

- **WAL mode** allows concurrent reads while a write is in progress — readers don't block writers, writers don't block readers
- **Only one writer at a time** — a second writer waits until the first commits or rolls back
- **Same-host only** — WAL requires shared memory; [doesn't work over network filesystems](https://sqlite.org/lockingv3.html)
- **BEGIN CONCURRENT** (experimental) allows multiple writers to process simultaneously, serializing only at COMMIT

For Plexus's persist-per-emission model (invariant 30), this means: if Trellis and Carrel both embed PlexusEngine pointing at the same `.db` file, their emissions would serialize at the SQLite level. Reads (queries, evidence trails) would proceed concurrently without blocking. This is workable for the immediate use case.

**Risk:** Two embedded engines holding the same DB open simultaneously with in-memory DashMap caches would have stale cache problems — Engine A writes, but Engine B's in-memory cache doesn't see it until reloaded. This pushes toward either:
- A single engine process (daemon) that all apps connect to
- A shared-nothing model where each app has its own DB and a sync protocol merges changes

#### The Daemon Model (Ollama Precedent)

[Ollama](https://github.com/ollama/ollama) runs as a local HTTP service on port 11434. Architecture:
- `ollama serve` starts the daemon (launchd plist on macOS, systemd on Linux)
- REST API over HTTP — any client can connect
- Multiple clients connect concurrently to the same service
- `KeepAlive: true` restarts on crash

**Applied to Plexus:** A `plexus serve` daemon running a PlexusEngine with a single SQLite store in `~/.local/share/plexus/plexus.db`. Multiple transports connect:
- MCP server connects via IPC/HTTP → Claude Code uses Plexus
- Sketchbin embeds a Plexus client library that talks to the daemon
- Carrel, Trellis, Manza all connect to the same daemon
- All share the same contexts, same graph, same enrichment loop

**Advantages:**
- Single in-memory cache — no stale data problem
- Single enrichment loop — no duplicate enrichment processing
- Context sharing is automatic — all apps see all contexts
- Clean separation: daemon owns data, apps own UI/transport

**Disadvantages:**
- Runtime dependency — apps fail if daemon isn't running
- Deployment complexity — need to install and manage a background service
- Not embeddable in the simple sense — Sketchbin can't just `use plexus;` and call process()
- Latency — IPC/HTTP overhead vs. in-process function calls

#### The Shared-DB Model (SQLite Direct)

Simpler alternative: all apps open the same SQLite file directly via PlexusEngine.

**Advantages:**
- No daemon to manage
- Embeddable — each app links Plexus as a library
- Simple deployment

**Disadvantages:**
- Stale cache problem (as noted above)
- Enrichment runs independently in each process — potential duplicate work, divergent enrichment state
- No coordination on which enrichments are registered — one app's enrichments might not match another's

**Mitigation for stale cache:** PlexusEngine could reload contexts from disk before each read operation, or use SQLite's `data_version` pragma to detect changes. This adds read latency but keeps the embedded model.

#### Current Context Boundaries in Plexus (Codebase Inspection)

Context isolation is deeply architectural:

1. **Edges cannot cross context boundaries.** Edge validation checks `ctx.get_node(&edge.source)` within a single context — there is no lookup path to another context.
2. **PlexusEngine partitions via DashMap<ContextId, Context>.** Each context is a self-contained subgraph.
3. **Enrichments are strictly single-context.** The `Enrichment::enrich()` signature receives `&Context` (singular) — enrichments have no access to the engine or other contexts.
4. **All engine operations are scoped to a single ContextId.** `find_nodes(context_id, ...)`, `traverse(context_id, ...)`, etc.
5. **The adapter pipeline binds to a context on each `ingest()` call.** The context_id is a parameter, not a property of the adapter.

Five architectural barriers would need addressing for any form of cross-context awareness.

#### What Cross-Context Enrichment Would Mean

The user's scenario: "network-research" context contains research docs. "distributed-ecologies-short-fiction" context contains some of the same docs plus fiction fragments. Can an enrichment discover that `concept:distributed-systems` appears in both contexts and surface this?

**Three possible designs:**

**Option A: Meta-context (virtual overlay)**
A meta-context is a read-only view that unions nodes/edges from multiple constituent contexts. No new data is stored — it's a query-time composition. When you query `evidence_trail(concept:distributed-systems)` against the meta-context, it traverses marks and fragments from all constituent contexts.

- **Pros:** No cross-context edges, no mutation complexity, no new invariants violated
- **Cons:** No enrichment can run on a virtual view (enrichments produce emissions, which need a target context). Co-occurrence across contexts would not be detected.
- **Invariant tension:** None — meta-contexts are read-only projections, not mutations

**Option B: Cross-context enrichment (engine-aware enrichments)**
Modify the `Enrichment` trait to receive `&PlexusEngine` instead of `&Context`. An enrichment could then scan multiple contexts for shared concepts and propose `may_be_related` edges. But — where do those edges live? They can't live in either constituent context (they'd reference a node in another context, failing endpoint validation).

- **Pros:** True semantic bridging across contexts
- **Cons:** Requires cross-context edges, which requires fundamental changes to edge validation, persistence, and query traversal. Normalization scope becomes ambiguous.
- **Invariant tension:** Directly contradicts the bounded-subgraph model. Would require amending multiple invariants.

**Option C: Shared-concept convergence (deterministic IDs do the work)**
Plexus already uses deterministic concept IDs: `concept:{lowercase_tag}`. If the same tag appears in two contexts, the concept node has the same ID in both. A query operation on the engine (not an enrichment) could discover this convergence: "these contexts share 14 concept IDs." No cross-context edges needed — the convergence is a property of the ID scheme, surfaced at query time.

- **Pros:** Zero changes to enrichments, edges, or context model. Works today with a new query method on PlexusEngine.
- **Cons:** Only discovers exact tag matches, not semantic similarity. No co-occurrence detection across contexts.
- **Invariant tension:** None — this is a new query operation, not a change to the graph model

### Implications

The multi-application sharing question and the cross-context question are actually independent:

**Multi-app sharing** is a deployment/infrastructure question. Two viable architectures:
1. **Daemon model** (Ollama-style): single PlexusEngine process, multiple app clients. Best for shared contexts, no stale cache. More operational complexity.
2. **Shared-DB model**: multiple embedded PlexusEngines opening the same SQLite. Simpler deployment, but needs cache invalidation. Good enough if write contention is low.

**Cross-context awareness** is a data model question. Three options with increasing invasiveness:
1. **Option C (shared-concept convergence)** — zero changes, usable immediately
2. **Option A (meta-context)** — read-only overlay, modest engineering
3. **Option B (cross-context enrichment)** — fundamental architecture change, needs its own RDD cycle

The pragmatic sequence: start with Option C (it works today), evaluate whether it's sufficient, and only pursue A or B if the user needs richer cross-context intelligence.

### Open questions for next iteration

- Should the daemon model use HTTP (like Ollama), Unix domain sockets (faster, same-host only), or gRPC (already in the roadmap per open question 9)?
- For the shared-DB model, how does SQLite's `data_version` pragma work for cache invalidation? Is it sufficient for Plexus's access patterns?
- What does the GraphStore trait need to look like to support both embedded (direct SQLite) and client (daemon connection) modes transparently?

## Question 3: How would federated/distributed storage work for Plexus contexts shared across users on different hosts?

**Method:** Web search + codebase inspection + Sketchbin semantic-discovery document analysis

### Findings

#### The Scenario

Multiple users on different hosts/networks, each running a Sketchbin instance with an embedded Plexus engine. Some aspect of their local Plexus context is federated — contributing to a shared context with other users. The semantic-discovery document (Part 7) describes this as the "collective pattern": an artist collective creates a shared context, each member holds a local replica, and emissions replicate across members.

The key question: what replication mechanism carries Plexus data between hosts, and how does the data model support eventual consistency?

#### Plexus's Data Model Is Surprisingly CRDT-Friendly

Inspection of the Plexus codebase reveals that the existing data model has strong natural alignment with CRDT semantics:

**1. Contributions are per-adapter LWW registers.** Each edge stores `HashMap<AdapterId, f32>` — a map of independent last-writer-wins slots. Two users can contribute to the same edge (e.g., both tag a concept) without conflict: User A's SketchAdapter has adapter ID `sketch:alice`, User B's has `sketch:bob`. Their contributions land in separate slots. This is structurally a [LWW-Register Map](https://crdt.tech/implementations) — one of the best-understood CRDT patterns.

**2. Concept nodes use deterministic IDs.** `concept:{lowercase_tag}` means that if Alice tags a sketch "ambient" and Bob tags a sketch "ambient" on separate hosts, they both produce `concept:ambient`. On merge, the node upserts — properties update, but the identity converges. This is an [add-only set (G-Set)](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type#G-Set_(Grow-only_Set)) of concept nodes. Nodes are never removed from the semantic dimension in normal operation.

**3. Emissions are self-contained atomic bundles.** The `Emission` struct carries `Vec<AnnotatedNode>`, `Vec<AnnotatedEdge>`, and `Vec<Removal>`. Each emission is a complete, validated unit. This is the natural **unit of replication** — ship the emission, not individual row changes.

**4. Enrichments are idempotent.** Each enrichment checks context state before emitting (invariant 36). After receiving a replicated emission, the local enrichment loop can safely re-run — it will either produce the same enrichments it already has (no-op via upsert) or discover new co-occurrences from the merged data.

**5. Chain and fragment IDs are deterministic.** `chain:{adapter_id}:{source}`, `fragment:{uuid_v5(adapter_id, text, tags)}`. Same input on two hosts produces the same node IDs. On merge: upsert.

**Where it's less clean:**

- **Removals.** `Emission.removals` deletes nodes. In a CRDT context, deletes are notoriously hard — you need tombstones or an observed-remove set (OR-Set) to prevent a deleted node from reappearing when a replica that hasn't seen the delete replicates its add. Plexus doesn't currently have tombstones.
- **Raw weight recomputation.** `raw_weight` is computed from contributions via scale normalization. After merging contributions from two replicas, raw weight must be recomputed. This is derived state, not stored state — but the current code stores it in the DB alongside contributions. The recomputation needs to happen after merge.
- **Edge validation.** Edges require both endpoints to exist in the same context. During async replication, an edge might arrive before its endpoint nodes. The current engine rejects such edges. Replication would need to either ship nodes before edges (ordering dependency) or buffer pending edges.

#### CRDT-SQLite Ecosystem

Three projects bring CRDT semantics to SQLite:

**[cr-sqlite](https://github.com/vlcn-io/cr-sqlite)** — A SQLite extension that adds multi-master replication via CRDTs. Tables are marked as CRRs (conflict-free replicated relations) with `crsql_as_crr()`. Each column becomes a LWW register. Changes are tracked via a `crsql_changes` virtual table with per-column version vectors and site IDs. Merging inserts into `crsql_changes` on the receiving side. Supports arbitrary peer-to-peer topologies.

**[Corrosion](https://superfly.github.io/corrosion/)** (Fly.io) — A Rust application built on cr-sqlite that adds gossip-based replication via the SWIM protocol over QUIC. No central coordinator. Each node has a full replica. Used in production at Fly.io for global service discovery across thousands of nodes, with p99 latency of ~1 second. **This is the closest architectural precedent to what Plexus federation needs**: Rust, SQLite, CRDTs, gossip protocol, no central server.

**[SQLite-Sync](https://github.com/sqliteai/sqlite-sync)** — Newer entry, CRDT-based sync for collaborative scenarios. Less mature than cr-sqlite.

**Applicability to Plexus:** cr-sqlite operates at the **row level** — every INSERT/UPDATE/DELETE is tracked and replicated. This would work mechanically: Plexus's nodes and edges tables would become CRRs, and changes would propagate automatically. However:

- **Row-level replication exposes the full schema.** In the federated Sketchbin scenario, you may want to replicate *semantic output only* (concepts, co-occurrence edges, provenance chains) but not *source content* (fragment text, private annotations). cr-sqlite doesn't support selective column/table replication — it's all or nothing per CRR table.
- **Enrichment-produced edges would also replicate.** If Alice's CoOccurrenceEnrichment produces a `may_be_related` edge and it replicates to Bob, Bob's enrichment might produce the same edge independently — but with a different site_id, leading to duplicate tracking overhead. The semantic-discovery document explicitly recommends: "only primary emissions (from adapters) are replicated. Enrichment-produced emissions are local."
- **No application-level filtering.** cr-sqlite merges everything. There's no "replicate semantic dimension but not structure dimension" or "replicate only nodes matching this filter."

#### Emission-Level vs. Row-Level Replication

The core design choice:

**Row-level replication (cr-sqlite approach):**
- Automatic, comprehensive, battle-tested
- No application code needed for sync
- But: no semantic filtering, no privacy control, enrichments leak across replicas
- Treats the database as the replication boundary

**Emission-level replication (application-layer approach):**
- Each `sink.emit()` call produces a serializable Emission
- The emission is the replication unit — ship it to peers
- Application controls what to replicate: primary emissions only, semantic-only, full, etc.
- Aligns with invariant 38 (transports are thin shells) — replication is a transport concern
- But: requires building the replication protocol, ordering, deduplication, and merge logic

**The semantic-discovery document's design (Part 7) implicitly assumes emission-level replication.** It describes "semantic-only replication" where "source content stays on each member's Sketchbin; the shared context accumulates only the semantic landscape." This is impossible with row-level replication — you'd need application-level filtering.

#### Replication Scope: What Gets Federated?

Three tiers of replication scope emerge:

**Tier 1: Semantic-only (lightest, most privacy-preserving)**
- Concept nodes (semantic dimension)
- `tagged_with` edges (fragment → concept)
- `may_be_related` edges (concept ↔ concept, from co-occurrence)
- Chain and mark nodes (provenance dimension — who tagged what)
- `references` edges (mark → concept)
- **Not replicated:** Fragment text content, file paths, private annotations
- **Use case:** Shared discovery context for a collective. Members see what concepts others explore, not the content itself.

**Tier 2: Metadata + semantic (medium)**
- Everything in Tier 1, plus:
- Fragment metadata (title, source type) but not full text
- Sketch metadata (tags, type, creation date) but not media content
- **Use case:** Federated Sketchbin discovery. Enough to show "what this sketch is about" without hosting the content.

**Tier 3: Full replication (heaviest)**
- Everything: all nodes, all edges, all properties
- **Use case:** Backup, migration, or trusted collective with full content sharing

The replication tier is a **policy decision per shared context**, not a global setting. A collective might share a Tier 1 context for discovery and maintain private Tier 3 contexts for individual work.

#### ActivityPub as Federation Transport

ActivityPub is already the federation transport for Sketchbin (Phase 2 in the implementation sequence). It supports custom extensions via JSON-LD contexts and the [Fediverse Enhancement Proposals (FEP)](https://codeberg.org/fediverse/fep) process.

A Plexus emission could be serialized as a custom ActivityPub activity:

```json
{
  "@context": ["https://www.w3.org/ns/activitystreams", "https://plexus.dev/ns/v1"],
  "type": "plexus:Emission",
  "actor": "https://alice.sketchbin.art/actor",
  "target": "plexus:context:collective-ambient",
  "object": {
    "type": "plexus:SemanticBundle",
    "nodes": [...],
    "edges": [...],
    "replicationTier": "semantic-only"
  }
}
```

**Advantages of ActivityPub:**
- Already implemented for Sketchbin federation (Fedify)
- Trust network (follow/accept) provides access control for shared contexts
- JSON-LD extensibility allows custom vocabulary without breaking non-Plexus instances
- Two-hop propagation reuses the existing social graph

**Disadvantages:**
- ActivityPub is designed for social activities, not database replication — no ordering guarantees, no exactly-once delivery
- Non-Sketchbin instances (Mastodon, etc.) would ignore Plexus-specific activities
- Delivery is pull-based (inbox polling) with no guaranteed latency

**Alternative:** A purpose-built protocol alongside ActivityPub — use ActivityPub for social federation (follows, boosts, sketch previews) and a separate channel (WebSocket, QUIC gossip à la Corrosion) for Plexus emission replication. This separates the concerns: social interactions flow through standard federation, semantic replication flows through a Plexus-specific channel optimized for convergence speed and ordering.

#### How Emissions Map to Replication Events

Concretely, for emission-level replication:

1. **Adapter produces emission** → `sink.emit(emission)` commits to local context
2. **Engine fires graph events** → `NodesAdded`, `EdgesAdded`, etc.
3. **Replication layer intercepts** → serializes the emission (or a filtered version per replication tier)
4. **Transport ships to peers** → via ActivityPub custom activity, WebSocket, or gossip
5. **Receiving engine applies** → deserializes emission, validates, commits to local replica of the shared context
6. **Local enrichment loop runs** → discovers new co-occurrences from merged data, produces local enrichments

Key properties:
- **Step 3 filters by tier** — semantic-only means stripping fragment text, keeping only concept/edge/provenance nodes
- **Step 5 uses existing commit logic** — upsert handles convergent IDs, per-adapter contributions merge naturally
- **Step 6 is local** — enrichment emissions don't replicate, preventing feedback amplification
- **Ordering:** Emissions within an adapter are causally ordered (each depends on the context state after the previous one). Across adapters/users, emissions are independent. This matches operation-based CRDT requirements: operations commute across users (independent adapter slots), but are ordered within a user.

#### The GraphStore Trait and Replication

The current `GraphStore` trait has the right methods for local persistence but would need extension for replication awareness:

- `save_context()` does a **full replace** (DELETE all, INSERT all). This doesn't work for replication — you can't atomically replace the entire context when other replicas are contributing to it.
- Individual `save_node()` and `save_edge()` use upsert — these are replication-friendly.
- No method for **change tracking** (what changed since version N?) or **emission journaling** (replay this emission).

For federation, GraphStore would need something like:
- `journal_emission(context_id, emission, metadata)` — persist an emission with replication metadata (origin site, vector clock/version)
- `get_emissions_since(context_id, version)` — pull-based sync for catching up
- `merge_remote_emission(context_id, emission)` — apply a remote emission with conflict resolution

These don't need to exist in the trait immediately — they could live in a `ReplicatedStore` extension trait that wraps a base `GraphStore`. The base trait stays simple for single-instance use; the extension adds federation capabilities.

### Implications

**Plexus's data model is naturally suited for CRDT-based federation.** The per-adapter contribution map, deterministic concept IDs, self-contained emissions, and idempotent enrichments all align with well-understood CRDT patterns. The main gaps are removals (need tombstones) and raw weight recomputation (need to treat it as derived state).

**Emission-level replication is the right abstraction for Plexus.** Row-level (cr-sqlite) would work mechanically but can't support the semantic-only replication that the Sketchbin vision requires. Emissions are already self-contained, serializable, and atomic — they're the natural replication unit.

**The replication tier model (semantic-only / metadata+semantic / full) gives users control over what they share.** This is critical for the creative collective use case: share the semantic landscape, keep the source content private.

**ActivityPub is the right transport for social federation; a purpose-built channel may be needed for emission replication.** The social layer (follows, boosts, sketch previews) works through standard ActivityPub. The semantic replication layer has different requirements (ordering, convergence speed, selective filtering) that may warrant its own protocol — even if it runs alongside ActivityPub on the same infrastructure.

**The GraphStore trait doesn't need fundamental changes for single-instance and same-host sharing.** For federation, a `ReplicatedStore` extension trait could wrap the base trait, adding journaling and merge operations without changing the core interface.

### Open questions for next iteration

- How should removals work in a federated context? Tombstones add storage overhead but are necessary for convergence. What's the expected removal frequency in Plexus's usage patterns?
- What vector clock or version scheme should emissions carry for causal ordering? Lamport timestamps are simple but don't capture concurrent emissions from different users. Version vectors per-user would be more precise.
- Should the replication tier be fixed per shared context or configurable per emission? A collective might want to share semantic-only by default but allow full replication for specific collaborative documents.
- How does defederation work? If a user leaves a collective, their contributed nodes/edges remain in the shared context (committed data). Should there be a "retract" mechanism, or is persistence-after-departure the correct behavior?

## Question 4: What federation and shared-context use cases exist beyond Sketchbin, and what do they demand of the design?

**Method:** Web search + codebase inspection + consumer analysis

### Findings

#### Plexus's Known Consumers and Their Collaboration Patterns

The domain model (§Concepts: Consumer) lists four consumers with distinct collaboration profiles:

**Trellis (creative writing)** — Fragments, tags, intuitive observations. A writer captures thoughts while thinking through a problem. Collaboration scenario: multiple writers in a workshop sharing a Plexus context. Each writer contributes fragments tagged from their working vocabulary. The shared context discovers thematic overlaps between writers' practices — writer A's "displacement" connects to writer B's "exile" through co-occurrence with shared concepts like "memory" and "place." Each writer retains their own fragments; the shared context accumulates the semantic landscape of the workshop's collective concerns.

**Carrel (research coordination)** — Literature annotations, paper abstracts processed by LLM concept extraction, provenance chains documenting reading trails. Collaboration scenario: a research team sharing a context across a project. Researcher A annotates papers on federated learning; Researcher B annotates papers on governance. The shared context discovers where their reading intersects — "distributed-computing" and "governance" co-occur across both researchers' annotations, surfacing the governance-of-distributed-systems intersection neither was explicitly tracking. Essay 13 demonstrated exactly this with three consumers and real arXiv data: 72 nodes, 238 edges, including 94 `may_be_related` edges from co-occurrence.

**Manza (code analysis)** — Codebase concepts, architecture documentation, code annotations. Collaboration scenario: a development team sharing a context across a codebase. Different developers annotate different subsystems. The shared context reveals architectural connections — the team member working on the API layer and the team member working on the database layer discover shared concepts like "transaction-boundary" and "error-propagation" through co-occurrence in their independent annotations. Valuable for team onboarding: a new developer can traverse the shared concept graph to understand how subsystems connect.

**EDDI (interactive performance)** — Gesture-driven, real-time. Less relevant for federation (performance contexts are inherently local and ephemeral), but shared EDDI contexts could enable collaborative performance: two performers' gesture vocabularies converging in a shared graph that controls the environment.

The user's described scenario cuts across these: "network-research" context accessed by Carrel and Manza, plus "distributed-ecologies-short-fiction" context accessed by Manza, Trellis, and Carrel. **Contexts are curated lenses, not per-application containers.** A single user may have multiple contexts, each spanning multiple tools, each revealing different facets of their work.

#### Precedents: How Existing Tools Handle Shared Knowledge

**[Zotero Group Libraries](https://www.zotero.org/support/groups)** — Shared reference libraries for research teams. Any member can add references, tags, and notes. Everything syncs to all members. Three access levels: private (members only), public closed (visible, members contribute), public open (anyone contributes). **Key limitation:** Zotero shares *content* (references, PDFs, notes), not *semantic structure*. Two researchers both tagging papers with "federated-learning" share the tag string but not any derived understanding of how that concept connects to others. There's no co-occurrence detection, no concept graph, no emergent structure.

**[Hypothesis](https://web.hypothes.is/)** — Collaborative annotation layer over the web. Annotations can be public, private, or shared with a specific group. Mission: "help people reason more effectively together through a shared, collaborative discussion layer over all knowledge." **Key limitation:** Annotations are anchored to specific documents. There's no cross-document concept linking. If two researchers annotate different papers with similar observations, Hypothesis has no mechanism to surface the connection. It's a shared annotation layer, not a shared knowledge graph.

**[Discourse Graphs](https://discoursegraphs.com/)** — Modular, composable research argument structures. Break scientific research into atomic elements: questions, claims, evidence. Client-agnostic (works in Roam, Notion, Obsidian) with decentralized push-pull storage. Described as "[GitHub for scientific communication](https://research.protocol.ai/blog/2023/discourse-graphs-and-the-future-of-science/)." **Closest architectural parallel to Plexus.** Discourse Graphs separate evidence from claim, enabling multiple interpretations to coexist. Each claim links to underlying evidence, allowing readers to assess validity independently. The graph structure maps directly to Plexus's model: evidence nodes ≈ fragment nodes, claim nodes ≈ concept nodes, support/oppose edges ≈ `tagged_with`/`may_be_related` edges.

**[Obsidian Sync/Relay](https://help.obsidian.md/Collaborate+on+a+shared+vault)** — Shared vaults for team collaboration. Native sync is file-level with eventual consistency (not real-time). Relay plugin adds real-time multiplayer with live cursors via CRDTs. **Key pattern:** Obsidian shares the raw content (markdown files); the knowledge graph (backlinks, tags, graph view) is computed locally from the shared content. This is the inverse of what Plexus federation would do — Plexus shares the semantic graph while content stays local.

**[Collaborative Workflow Provenance](https://link.springer.com/chapter/10.1007/978-3-642-17819-1_6)** — Research on tracking who contributed what in collaborative scientific workflows. Uses W3C PROV-DM (provenance data model) to record origins of data products across workflow stages. **Key insight for Plexus:** provenance in collaborative settings needs per-contributor attribution. Plexus's per-adapter contribution model already handles this — each contributor's adapter has a distinct ID, and their contributions are stored independently on each edge.

#### Four Distinct Collaboration Patterns

Analyzing these tools and Plexus's consumers reveals four distinct patterns of shared-context collaboration, each with different requirements:

**Pattern 1: Shared Sensemaking (Research Team)**
A research group investigates a shared topic. Each member reads different sources, annotates differently, uses different tools (Carrel for papers, Manza for code, Trellis for reflective notes). The shared context accumulates all their semantic output. The graph discovers connections none of them individually tracked.

- **Replication need:** Semantic-only or metadata+semantic. Source content stays local (privacy, IP, licensing concerns for unpublished papers).
- **Provenance need:** High. "Who found this concept? Through what evidence?" is critical for research validity. Per-adapter contributions and provenance chains are essential.
- **Scale:** Small team (2-10 people), moderate volume (hundreds of emissions per person per project).
- **Topology:** Star or mesh. Could be centralized (shared server) or peer-to-peer (each researcher holds a replica).
- **Trust model:** High trust within the team. Access control at the group boundary, not per-emission.

**Pattern 2: Collective Creative Practice (Sketchbin)**
An artist collective shares a context reflecting their combined creative output. Each member publishes work from their Sketchbin. The shared context maps the collective's creative territory.

- **Replication need:** Semantic-only. Creative content stays on each member's Sketchbin (BYOS, data sovereignty).
- **Provenance need:** Medium. Attribution matters (whose sketch originated this concept?), but the primary value is the semantic landscape, not the audit trail.
- **Scale:** Small to medium collective (3-30 members), variable volume (depends on creative output).
- **Topology:** Peer-to-peer via ActivityPub. No central server. Each member's Sketchbin holds a replica.
- **Trust model:** Membership-based. Join/leave via follow/accept.

**Pattern 3: Team Knowledge Base (Development Team)**
A development team maintains a shared understanding of a codebase. Each developer annotates code, documents architecture decisions, records design rationale. The shared context reveals how subsystems connect and evolve.

- **Replication need:** Full or metadata+semantic. Architecture documentation benefits from sharing content (decision rationale, code comments).
- **Provenance need:** Medium. "When was this concept introduced? By whom?" helps with onboarding and archaeology.
- **Scale:** Team-sized (3-20 people), continuous low-volume emissions (annotations accumulate over months).
- **Topology:** Centralized (daemon or shared DB on a team server). Less need for peer-to-peer.
- **Trust model:** Team membership, potentially integrated with existing auth (GitHub org, SSO).

**Pattern 4: Open Discovery Network (Discourse Graphs)**
An open research community shares modular knowledge components — questions, claims, evidence — across institutions and tools. Anyone can contribute; the graph accumulates collective understanding of a field.

- **Replication need:** Full (open science model — share everything to maximize composability).
- **Provenance need:** Critical. "What evidence supports this claim? Who contributed it? Is it still current?" defines the value of the graph.
- **Scale:** Large (hundreds of contributors), high volume (academic output).
- **Topology:** Federated. Institutional nodes hold replicas. Contributions propagate through the network.
- **Trust model:** Open with attribution. Contributions carry provenance but aren't access-controlled. Quality assessed by the community through the graph structure itself (well-evidenced claims accumulate stronger connections than poorly-evidenced ones).

#### What the Patterns Demand of Plexus's Federation Design

Comparing across patterns reveals what the design must support:

| Requirement | Sensemaking | Creative | Dev Team | Open Discovery |
|---|---|---|---|---|
| Semantic-only replication | Required | Required | Optional | No |
| Full replication | No | No | Useful | Required |
| Per-contributor provenance | Critical | Medium | Medium | Critical |
| Peer-to-peer topology | Optional | Required | No | Required |
| Centralized topology | Optional | No | Required | Optional |
| Access control | Group-level | Membership | Team auth | Open |
| Defederation/retraction | Rare | Possible | N/A | Uncommon |

**Key observations:**

1. **Replication tiers (from Q3) are validated across all patterns.** Semantic-only is the common case for privacy-sensitive scenarios. Full replication is needed for open knowledge. The tier model handles all four.

2. **Per-contributor provenance is non-negotiable.** Every pattern needs to know who contributed what. Plexus's per-adapter contribution model handles this — each user's adapter has a distinct ID, and contributions are independent. The `chain:{adapter_id}:{source}` deterministic ID ensures each user's provenance chain is distinct even when contributing to the same shared context.

3. **Both peer-to-peer and centralized topologies are needed.** The creative and open-discovery patterns need P2P. The dev-team pattern needs centralization. The sensemaking pattern could go either way. This argues for **topology-agnostic replication** — the emission-level replication from Q3 works regardless of whether emissions travel through a central hub, a gossip network, or direct peer connections.

4. **Access control varies wildly.** Some contexts are fully open, some are team-only, some are membership-gated. This is a **per-context policy**, not a system-wide setting. The shared context metadata should declare its access model.

5. **The "curated lens" model is central.** Contexts aren't per-application or per-user — they're per-purpose. A research team's "network-research" context might receive emissions from Carrel (paper annotations), Manza (code analysis), and Trellis (reflective fragments), contributed by different team members using different tools. The context is the collaboration boundary, not the tool or the person.

#### The Discourse Graph Connection

[Discourse Graphs](https://discoursegraphs.com/) are the closest existing parallel to what Plexus federation would enable. Joel Chan's [research](https://joelchan.me/assets/pdf/Discourse_Graphs_for_Augmented_Knowledge_Synthesis_What_and_Why.pdf) describes them as:

> "Modular, reusable alternative to systematic reviews... each evidence page contains enough context for the user to assess its validity and relevance, and are used to build novel claims."

The mapping to Plexus is direct:

| Discourse Graph | Plexus |
|---|---|
| Evidence node | Fragment node (source material) |
| Claim node | Concept node (extracted idea) |
| Question node | Could be a concept with a specific type label |
| Support edge | `tagged_with` (fragment supports concept) |
| Oppose edge | Not currently in Plexus — would need a relationship type |
| Synthesis | Co-occurrence enrichment discovers concept relationships |
| Provenance | Chain → mark → references → concept trail |

The critical difference: Discourse Graphs are **manually constructed** — researchers explicitly create evidence nodes, claim nodes, and support/oppose links. Plexus's adapter pipeline **automates the extraction** — fragments enter, concepts and relationships emerge through the adapter and enrichment loop. Both produce graph structures; Plexus produces them as a side effect of normal work.

For federated shared contexts, Plexus could serve as the **infrastructure layer beneath Discourse Graphs** — each researcher's contributions are automatically structured into the graph, and the co-occurrence enrichment discovers connections that manual linking would miss.

**One missing piece:** Plexus has no `oppose` or `contradicts` edge type. In a research collaboration context, capturing disagreement is as important as capturing agreement. Two researchers might tag the same concept from conflicting evidence. Currently, Plexus would see both contributions and strengthen the concept — it wouldn't surface the tension. This is a potential enrichment design challenge: detecting when contributions from different adapters support contradictory claims about the same concept.

### Implications

**The shared context is Plexus's collaboration primitive.** Not a shared document (Obsidian), not a shared annotation layer (Hypothesis), not a shared reference library (Zotero). A shared **semantic landscape** that each participant enriches through their own tools and practices. Content stays local; understanding converges.

**This is a genuinely distinct position in the tool ecosystem.** Existing collaboration tools share either content (documents, references, files) or commentary (annotations, discussions). Plexus shares **derived semantic structure** — the concepts, relationships, and co-occurrence patterns that emerge from independent contributions. The semantic graph is the collaboration artifact.

**Four patterns (sensemaking, creative, dev-team, open-discovery) validate and stress-test the federation design from Q3.** The emission-level replication with tiered scoping handles all four. Per-contributor provenance is universal. Topology must be flexible (P2P and centralized). Access control is per-context policy.

**The Discourse Graph parallel suggests Plexus could be infrastructure for collaborative knowledge synthesis.** Automated extraction + shared co-occurrence enrichment + per-contributor provenance = a system that discovers connections across researchers' independent work without requiring manual graph construction. This is a larger ambition than any single consumer (Sketchbin, Carrel, Manza) but emerges naturally from the federation design.

### Open questions for next iteration

- Should Plexus support an `oppose`/`contradicts` relationship type for research contexts where capturing disagreement matters? What enrichment would detect conflicting evidence?
- How should context membership/access control be represented in the data model? Is it context metadata, a separate registry, or delegated to the transport layer?
- What's the minimum viable shared-context experience? Is it two Carrel users on the same host sharing a context (Pattern 1, centralized), or does the design need to handle P2P from the start?
- How does the "curated lens" model interact with shared contexts? If a context spans multiple tools and multiple users, who curates its sources? Is curation itself a federated operation?

## Question 5: What would an `opposes`/`contradicts` relationship type add as an exploration layer, and how does it interact with the weight model?

**Method:** Web search + codebase inspection + ADR analysis

### Findings

#### What ADR-003 Already Decided

ADR-003 (Reinforcement Mechanics) explicitly addressed the relationship between contribution sign and relationship polarity:

> "All contributions — regardless of sign — add positively to raw weight after scale normalization. A contribution value represents the **strength of an observation**, not its polarity; qualities like sentiment direction belong in annotations or edge properties."

This was a deliberate design choice. The contribution value (-1.0 to 1.0 for sentiment, 0-20 for test counts, etc.) measures *how strongly the adapter observed something*, not *what kind of thing it observed*. A sentiment adapter emitting -0.9 means "I strongly observed negative sentiment," not "this edge is negative." After scale normalization, that -0.9 maps to a position in [0, 1] relative to that adapter's range — the most negative observation gets the lowest scale-normalized value, and the most positive gets the highest.

The polarity of a relationship — support vs. opposition, relatedness vs. contradiction — belongs in the **relationship type**, not the contribution value. This is the correct separation of concerns. Contribution values answer "how strong?" Relationship types answer "what kind?"

#### Plexus Already Supports Multiple Edge Types Between the Same Pair

Inspection of `Context::add_edge()` (`src/graph/context.rs:167`) confirms: edge deduplication matches on `(source, target, relationship, source_dimension, target_dimension)`. Two edges between the same concept pair with different relationship types are stored as **separate edges**, each with independent per-adapter contributions.

This means the multigraph model the user described is already architecturally supported. `concept:decentralization` and `concept:governance` could have:

- A `may_be_related` edge with contributions from CoOccurrenceEnrichment (they co-occur in 4 fragments)
- A `contradicts` edge with contributions from a TensionEnrichment (they appear with opposing sentiment in 2 fragments)
- A `complements` edge with contributions from a ComplementarityEnrichment (they appear together in contexts where one addresses gaps the other leaves)

Each edge independently accumulates per-adapter contributions. Each can be queried separately. The relationship type carries the semantics; the contributions carry the strength. No changes to the weight model are needed.

#### Signed Graphs and Balance Theory

[Signed graph theory](https://en.wikipedia.org/wiki/Signed_graph) (Harary 1953, Cartwright & Harary 1956) studies graphs where edges carry positive or negative signs. The key result is **balance theory**: a signed graph is balanced if the product of edge signs around every cycle is positive. In social terms: "the friend of my friend is my friend" (positive) and "the enemy of my enemy is my friend" (positive cycle through two negatives).

Applied to Plexus: if concepts A, B, C form a triangle where A-B is `may_be_related`, B-C is `may_be_related`, but A-C is `contradicts`, the triangle is *imbalanced*. This structural signature could be detected by an enrichment — imbalanced triangles represent intellectual tensions worth surfacing. "These three concepts are all related, but two of them oppose each other despite both relating to the third."

Balance theory doesn't require changing the weight model — it's a structural property of which relationship types appear on edges, not the magnitude of those edges. A BalanceEnrichment could scan for imbalanced cycles and surface them as a query result or as new metadata on the involved edges.

#### Bipolar Weighted Argumentation Frameworks (QBAFs)

The formal framework closest to what the user describes is the [Quantitative Bipolar Argumentation Framework](https://www.emergentmind.com/topics/quantitative-bipolar-argumentation-framework-qbaf) (QBAF). In a QBAF:

- Each argument has an **intrinsic weight** (base strength)
- Arguments can have both **attackers** and **supporters**
- The final **acceptability degree** is computed by aggregating the influence of all attackers and supporters
- Crucially: attacks and supports of equal strength cancel each other out — a key property for modeling genuine debate

The QBAF maps to Plexus as follows:

| QBAF | Plexus |
|---|---|
| Argument | Concept node |
| Intrinsic weight | Node's base properties (no current equivalent; could be added) |
| Support relation | `may_be_related` or `supports` edge |
| Attack relation | `contradicts` or `opposes` edge |
| Acceptability degree | Query-time computation from both edge types |

The QBAF insight for Plexus is **not** that contributions should be negative. It's that **query-time computation should be able to aggregate across multiple relationship types**. Currently, query-time normalization operates on individual edges. A QBAF-inspired query would compute a concept's "net stance" by combining the strengths of its `supports` and `contradicts` edges — positive edges strengthen, negative edges weaken.

This would be a new **NormalizationStrategy** variant, not a change to contributions or edge storage. The data model stays the same; the query layer gains a new lens.

#### What "Opposes" Would Add: Three Exploration Layers

**Layer 1: Tension Detection**

An enrichment that detects when concepts appear in opposing contexts. Three possible signals:

1. **Sentiment divergence** — Two concepts co-occur, but fragments containing both show mixed sentiment. Some fragments are positive about the relationship, others negative. The adapter's contribution value (invariant 12 allows -1 to 1) already captures this per-fragment, but there's no enrichment that detects *divergence across fragments*.

2. **Contradictory tagging** — Two fragments are tagged with the same concepts but carry opposing claims. This requires content analysis (LLM-level), not just tag co-occurrence. A `ContradictionEnrichment` would need to compare fragment text, not just fragment tags.

3. **Cross-contributor disagreement** — In a shared context (Q4 patterns), two contributors' adapters might contribute to the same concept pair with systematically different contribution strengths. Researcher A consistently produces strong `tagged_with` contributions for "decentralization" + "resilience", while Researcher B produces weak ones. The per-adapter contribution map on a single edge already captures this, but no enrichment surfaces it as a distinct signal.

A `TensionEnrichment` could detect signal (1) by comparing contribution distributions across fragments and emit `contradicts` edges between concept pairs where evidence is systematically divided. This operates at the same level as CoOccurrenceEnrichment — reading context state, computing a metric, emitting edges — and terminates via the same idempotency pattern.

**Layer 2: Bipolar Navigation**

With both `may_be_related` and `contradicts` edges in the graph, traversal queries gain a new dimension. Instead of a single question ("what's related to this concept?"), you get three:

- **"What supports this concept?"** — traverse `may_be_related` edges (co-occurrence evidence)
- **"What challenges this concept?"** — traverse `contradicts` edges (tension evidence)
- **"What's the contested territory?"** — find concepts with both strong `may_be_related` AND strong `contradicts` edges to the same neighbor

The third query is the most interesting. It identifies intellectual frontiers — places where the evidence is genuinely divided. In a research context: "federated-learning and privacy are strongly related AND in tension" means the relationship is real but contested. This is exactly what a researcher needs to know.

The StepQuery typed traversal (ADR-013) already supports filtering by relationship type. A bipolar query would be two StepQueries composed: one filtering for `may_be_related`, one for `contradicts`, with the results compared.

**Layer 3: Disagreement Surfacing in Shared Contexts**

In a federated shared context (Q3, Q4), `contradicts` edges gain an additional meaning: they surface disagreement between contributors. If Alice's adapter contributes to a `contradicts` edge between "decentralization" and "efficiency" and Bob's adapter contributes to a `may_be_related` edge between the same pair, the graph now contains both edges — revealing that Alice and Bob have different perspectives on how these concepts relate.

The per-adapter contribution maps make this fully inspectable. A query can ask: "who contributed to the `contradicts` edge between these concepts? And who contributed to the `may_be_related` edge?" The provenance trail answers these questions through chain → mark → references → concept.

This is structurally different from a disagreement on a single edge (where both contributions just average out). Multiple relationship types between the same pair **preserve the disagreement rather than averaging it away**. The user's intuition — "I wouldn't want an edge to be binary" — is exactly right. Both perspectives coexist in the graph. The query layer decides how to present them.

#### What Would Need to Change

Remarkably little:

1. **New relationship types.** Define `contradicts`, `opposes`, or `tension` as relationship type strings. These are just strings on the `Edge.relationship` field — no schema change, no new struct. Convention, not mechanism.

2. **A TensionEnrichment.** An enrichment that detects sentiment divergence or contradictory evidence and emits `contradicts` edges. This follows the same pattern as CoOccurrenceEnrichment: read context snapshot, compute metric, emit edges if threshold met, terminate via idempotency.

3. **Query composition for bipolar navigation.** StepQuery (ADR-013) already supports relationship type filtering. A "contested territory" query is the intersection of two StepQueries. This might benefit from a first-class operation but doesn't require one.

4. **Optional: QBAF-style normalization strategy.** A NormalizationStrategy variant that computes a concept's "net stance" by combining `supports` and `contradicts` edge weights. This is additive to the existing normalization framework — it doesn't replace the default strategy.

What does NOT need to change:
- The weight model (contributions remain positive strength observations)
- The edge storage model (multigraph already works)
- Scale normalization (unchanged)
- The enrichment loop (TensionEnrichment follows existing patterns)
- The context boundary model (opposition edges stay within context like all edges)

#### The Sentiment Adapter Question

The user raised an interesting point about -1 to 1 ranges. Invariant 12 already supports this: adapters can emit any finite f32, including negative values. Scale normalization maps the full range to [0, 1] for raw weight computation.

But there's a subtlety. If a SentimentAdapter emits contributions on [-1, 1]:
- A contribution of -0.9 (strongly negative sentiment) scale-normalizes to near 0.0
- A contribution of +0.9 (strongly positive) scale-normalizes to near 1.0
- A contribution of +0.1 (weakly positive) scale-normalizes to ~0.5

The **raw contribution values are preserved** in the stored `HashMap<AdapterId, f32>`, even though the scale-normalized values are always positive. A query can inspect the raw contributions to recover the signed sentiment. But this requires looking at the contribution map, not the raw weight.

This suggests a potential enhancement: **contribution-level query faceting**. Instead of just raw weight (sum of normalized contributions), a query could facet by adapter and return the signed contribution values alongside the normalized weight. "This `tagged_with` edge has raw weight 2.3. Of that, the SentimentAdapter contributed -0.7 (negative sentiment), the CoOccurrenceAdapter contributed 0.85 (strong co-occurrence), and the ManualTagger contributed 1.0 (explicit tag)."

This isn't a weight model change — it's a query presentation enhancement. The data is already stored; it just needs a query path to surface it.

### Implications

**Opposition/contradiction is a relationship type, not a weight polarity.** ADR-003 already made this decision correctly. The user's instinct to have "multiple edges that enrich the graph" maps cleanly to Plexus's existing multigraph support. `may_be_related` and `contradicts` can coexist between the same concept pair as independent edges with independent contributions.

**A TensionEnrichment would be a natural peer to CoOccurrenceEnrichment.** Same pattern: read context state, detect a signal (sentiment divergence, contradictory evidence), emit edges. The enrichment loop handles the rest.

**Bipolar navigation opens a qualitatively new exploration dimension.** "What's the contested territory?" is a question that neither pure co-occurrence nor pure contradiction can answer — it requires both. This is the QBAF insight applied to knowledge graphs.

**In shared/federated contexts, multiple relationship types between the same pair preserve disagreement rather than averaging it.** This is the critical property for collaborative sensemaking. Two researchers can have genuinely different perspectives on how two concepts relate, and both perspectives survive in the graph as distinct edges.

**Almost nothing needs to change in the current architecture.** The multigraph model, the per-adapter contributions, the enrichment loop, and the StepQuery traversal all support this pattern today. The additions are: new relationship type strings, a new enrichment, and optionally a bipolar normalization strategy.

### Open questions for next iteration

- What signal should a TensionEnrichment actually detect? Sentiment divergence requires a SentimentAdapter that doesn't exist yet. Contradictory evidence detection requires LLM analysis of fragment content. What's the simplest useful tension signal?
- Should balance theory analysis (detecting imbalanced triangles) be an enrichment, a query operation, or both? Enrichments produce persistent graph mutations; query operations are ephemeral. The answer affects whether imbalance detection consumes graph space.
- How does the `contradicts` relationship type interact with the enrichment loop? If CoOccurrenceEnrichment proposes `may_be_related` and TensionEnrichment proposes `contradicts` between the same pair in the same loop round, does that cause oscillation? (Probably not — both would converge via idempotency — but worth verifying.)
