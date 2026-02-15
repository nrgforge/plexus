# ADR-018: Emission-Level Replication for Federation

**Status:** Proposed

**Research:** [Essay 17](../essays/17-storage-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — replication layer, replication tier, ReplicatedStore, emission, contribution, replicate, ingest_replicated, journal

**Depends on:** ADR-016 (library rule), ADR-017 (shared-DB as Layer 2 prerequisite), ADR-003 (per-adapter contributions — CRDT alignment)

---

## Context

Plexus contexts need to replicate across users on different hosts. Four collaboration patterns emerged from the research: research teams (shared reading/thinking contexts), artist collectives (shared creative semantic landscapes), development teams (shared codebase understanding), and open discovery networks (shared knowledge components across institutions).

All four share a structural property: the shared context is the collaboration primitive. Not a shared document, annotation layer, or reference library — a shared semantic landscape that each participant enriches through their own tools.

Essay 17 found that Plexus's data model has strong natural CRDT alignment: per-adapter contributions are LWW-Register Maps (ADR-003), concept nodes use deterministic IDs (invariant 19), emissions are self-contained bundles, and enrichments are idempotent. Row-level CRDT replication (cr-sqlite, Corrosion) works in production but replicates everything — it cannot selectively exclude content while sharing semantic structure.

## Decision

### 1. Emissions are the replication unit

Each `sink.emit()` call produces a serializable, self-contained emission. The replication layer intercepts persisted emissions, filters them by policy, and ships them to peers. This is emission-level replication, not row-level.

The emission is already the unit of validation, contribution tracking, and persistence (invariant 30). Making it the unit of replication is a natural extension — the serialization boundary aligns with the existing validation boundary. The `Emission` struct's contents (annotated nodes, annotated edges, removals) are structurally serializable; a wire format (e.g., bincode, MessagePack, or a Plexus-specific format) is needed for the transport layer, but no new conceptual boundary is introduced.

The LWW-replace semantics from ADR-003 require a total or causal ordering of contributions from the same adapter ID across hosts. The versioning scheme (version vectors vs Lamport timestamps) is deferred but is a prerequisite for contribution convergence under federation.

### 2. Only primary emissions replicate

Enrichment-produced emissions are local to each replica (invariant 42). Only adapter-produced (primary) emissions propagate to peers. This prevents feedback amplification: without this constraint, an enrichment's output replicating to a peer would trigger that peer's enrichment loop, which would replicate back, creating an infinite cycle.

Each replica runs its own enrichment loop on received emissions. For deterministic enrichments (e.g., CoOccurrenceEnrichment — counting shared fragments), identical enrichment configuration and identical primary data produce identical derived structure, assuming order-independent processing. For non-deterministic enrichments — particularly LLM-based semantic interpretation (theme extraction, relationship inference, clustering) — replicas will diverge even with identical configuration and identical data, because the enrichment itself produces different output on each invocation. Convergence is a property of enrichment *nature*, not just enrichment *configuration*. With different enrichments, replicas diverge intentionally. See domain model open question 11 for the full analysis of coordination options.

### 3. Replication tiers per context

A context declares its replication tier — a per-context policy (invariant 43) controlling what data propagates:

| Tier | Includes | Excludes | Use case |
|------|----------|----------|----------|
| **Semantic-only** | Concept nodes, edges, provenance chains/marks (including mark annotation text), `references` edges | Fragment node full text (the source content that adapters process) | Shared discovery — see concepts and provenance, not source content |
| **Metadata + semantic** | Everything in semantic-only, plus fragment metadata (title, source type) | Fragment full text | Federated discovery — "what this is about" without hosting content |
| **Full** | Everything | Nothing | Backup, migration, open research |

Different contexts on the same instance can use different tiers. A collective might share a semantic-only context for discovery and maintain private full contexts for individual work.

**Note on annotation text and invariant 7:** Mark annotation text is semantic content (invariant 7: "an annotation IS a fragment"). In the semantic-only tier, mark annotation text replicates because it is part of the provenance trail's semantic layer — it describes what the annotator observed. What is *excluded* is the source content that adapters process (the full text of papers, code files, journal entries). The distinction: annotation text is the annotator's observation *about* source material; fragment full text is the source material itself.

### 4. ReplicatedStore extension trait

A `ReplicatedStore` trait wraps a base `GraphStore` to add federation capabilities:

- **Emission journaling:** persist each emission with replication metadata (origin site, version vector) for sync and replay
- **Pull-based sync:** "give me emissions since version N" for catching up after reconnection
- **Remote emission merge:** apply a remote emission with conflict resolution and raw weight recomputation

The base `GraphStore` stays simple for single-instance use. `ReplicatedStore` extends it. The simplest deployment (Layer 1) never encounters federation complexity.

### 5. Replication layer architecture

The replication layer has two responsibilities at different stack levels (invariant 44):

- **Outbound** (store-level): wraps `GraphStore`, journals persisted primary emissions, filters by replication tier, ships to peers
- **Inbound** (engine-level): applies remote emissions via `ingest_replicated(context_id, remote_emission)` — validates, commits, runs the enrichment loop, but skips outbound replication to prevent echo

The replication layer is not a transport (invariant 44). Consumer-facing transports remain thin shells (invariant 38). The replication layer is invisible to consumers — it coordinates between the engine and the store.

The `ingest_replicated()` path is infrastructure-internal, not consumer-facing. It does not violate ADR-012's constraint that all consumer writes go through `ingest()`. Consumer transports continue to use `ingest()` exclusively. `ingest_replicated()` is called only by the replication layer's inbound handler.

### 6. Adapter ID scoping for federation

Per-adapter contributions are LWW registers keyed by adapter ID (ADR-003). For federation, adapter IDs must be unique per user-instance: `{adapter_type}:{user_or_instance_id}` (invariant 13, amended). Without this, two users with the same adapter ID would collide in the same LWW slot, breaking CRDT convergence.

### Deferred

- **Federation transport protocol:** whether to use ActivityPub, a purpose-built sync protocol, or both. Depends on the first consumer (Sketchbin) and its social federation design.
- **Version vectors vs Lamport timestamps:** the emission journal's versioning scheme. Depends on expected topology and scale.
- **Tombstones for removals:** CRDT-safe deletion to prevent reappearance of deleted nodes. Depends on expected removal frequency.
- **Defederation and data retraction:** what happens when a user leaves a shared context. Protocol-level question.
- **Context membership and access control:** how shared context membership is represented and enforced.

### Alternatives considered

- **Row-level CRDT replication (cr-sqlite/Corrosion).** Proven at scale (Fly.io). But replicates everything — cannot selectively share semantic structure while excluding content. The replication tier requirement eliminates this option.
- **Document-level sync (CouchDB/PouchDB pattern).** Contexts as documents with revision tracking. Simpler than emission-level but loses the granularity of per-emission filtering and contribution tracking.
- **Central server only (no federation).** The managed server (ADR-017) covers the team-on-shared-infrastructure case. But it doesn't cover the local-first, cross-host, peer-to-peer case that the Sketchbin collective scenario requires.

## Consequences

**Positive:**

- Selective replication: semantic structure can propagate without content. Privacy-preserving collaboration.
- Natural CRDT alignment: per-adapter LWW contributions, deterministic IDs, and idempotent enrichments mean the data model is already convergence-friendly
- The emission is already the validation and persistence boundary — no new serialization boundary needed
- Layer isolation: consumers, adapters, and enrichments are completely unaffected by federation. Only the store and engine layers change.

**Negative:**

- Significant engineering: ReplicatedStore, emission journaling, version vectors, sync protocol, tombstones, and the inbound/outbound replication layer are substantial work
- Enrichment divergence: replicas with different enrichment configs produce different derived structure permanently. This is either a feature (personal analytical lens) or a problem (inconsistent shared understanding) depending on context — see domain model open question 11
- Edge validation during async replication may encounter edges before their endpoint nodes arrive. Ordering within an emission is guaranteed; ordering across emissions from different users is not.

- After merging remote emissions, scale normalization bounds (per-adapter min/max from ADR-003) may change, requiring recomputation of raw weights for all affected edges. Lazy recomputation (on query) vs eager recomputation (on merge) is an implementation decision.

**Neutral:**

- The base `GraphStore` trait is unchanged. Federation adds an extension, not a modification.
- The replication layer introduces a fourth architectural concern (federation coordination) independent of the three consumer-facing extension dimensions (adapters, enrichments, transports). Invariant 40 may need amendment to acknowledge this infrastructure dimension.
- This ADR establishes the architectural direction for federation. Detailed protocol design (transport, versioning, membership) requires its own research cycle when the first federation consumer (Sketchbin) is ready.
