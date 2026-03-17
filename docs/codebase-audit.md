# Codebase Audit: Plexus

**Date:** 2026-03-15
**Scope:** Whole codebase — all modules, documentation, and configuration
**Coverage:** This analysis sampled strategically across ~40 of 53 source files, 8 ADRs, and the full module hierarchy. It is representative, not exhaustive. The `.llm-orc/` artifacts directory, `test-corpora` submodule, and `docs/essays/` content were not analyzed in depth. Test files were sampled across all modules but not every individual test was read.

## Executive Summary

Plexus is a network-aware knowledge graph engine implemented in Rust (~27,400 lines across 53 source files) that models semantic concepts, provenance chains, and file references as typed nodes in a unified graph with self-reinforcing edges. It is exposed primarily via the Model Context Protocol (MCP) and designed for Claude-adjacent AI tooling (Trellis, Carrel, Manza, EDDI).

The architecture is genuinely well-considered. The write path implements a clean hexagonal pattern: adapters transform domain-specific input into graph mutations via sinks, with an event-driven enrichment loop that runs to quiescence after each ingest. A transport-independent API facade (`PlexusApi`, ADR-014) separates the single write endpoint (`ingest()`) from the MCP transport. The read path takes a deliberate shortcut: sync methods directly query the in-memory `DashMap` cache for performance, bypassing the pipeline entirely. This CQRS-aligned split is documented and intentional.

The most significant findings are: (1) the adapter module has accumulated 70% of the codebase (~19K lines) with two parallel extraction paths (`SemanticAdapter` and `DeclarativeAdapter`) that ADR-028 planned to converge but hasn't yet; (2) the persist-per-emission pattern combined with O(edges × adapters) normalization creates a latency profile that scales poorly with graph size; (3) several documented invariants (`ProposalSink` constraints, configurable `max_rounds`, `PlexusApi` holding `ProvenanceApi`) describe a system that differs from what the code actually implements; and (4) telemetry is nearly absent from the production write path, making runtime behavior opaque.

The codebase's strengths — a rich ADR trail, deliberate architectural layering, strong test suite (~425 tests), and a unified data model that enables cross-dimension traversal — are worth protecting. The areas needing attention are primarily about convergence (closing the gap between documented architecture and actual code) and operational readiness (telemetry, error classification, performance at scale).

## Architectural Profile

### Patterns Identified

| Pattern | Confidence | Description |
|---------|-----------|-------------|
| Ports-and-Adapters (Hexagonal) | High | Fully implemented on the write path (adapters → sinks → engine); absent on the read path (direct engine access). |
| CQRS-aligned async/sync split | High | Writes are async through `IngestPipeline`; reads are sync `DashMap` lookups. Enforced by API convention, not type system. |
| Internal event-driven reactive loop | High | Enrichments cascade via `GraphEvent` slices until quiescence or safety valve (10 rounds). |
| Write-through cache | High | Every emission persists immediately to SQLite; no batching, no deferred flush. |
| Unified data model | High | Provenance, semantic concepts, and file references are all graph nodes distinguished by `dimension` and `node_type`. |
| Type-erased plugin contract | High | `Box<dyn Any>` as adapter input with stringly-typed `input_kind` dispatch. |
| Declarative adapter DSL | High | YAML-driven adapter specs bypass Rust compilation for end-user extensibility. |
| Facade pattern | High | `PlexusApi` as transport-independent routing layer (ADR-014). |
| Cache-aside with no eviction | High | Entire graph in `DashMap`, full context clone per read; no eviction or pagination. |

### Quality Attribute Fitness

| Optimizes For | At the Expense Of | Evidence |
|--------------|-------------------|----------|
| Transport substitutability | Transport-level error observability | MCP flattens rich error types to `err_text(e.to_string())` |
| Read performance | Bounded memory growth | Full context clone per read; no eviction or pagination |
| Durability / crash safety | Write throughput | Persist-per-emission; enrichment loop can trigger 12+ SQLite transactions per ingest |
| Extensibility | Compile-time type safety | `Box<dyn Any>` adapter inputs; runtime downcast failures |
| Compositional enrichment | Predictable latency | Cascading enrichment rounds with opaque per-request cost |
| Simplicity | Reliability under failure | `Mutex::lock().unwrap()` throughout storage/MCP creates panic-on-poison risk |
| Low-noise output | Operability / debuggability | ~12 tracing calls in entire non-test codebase |

### Inferred Decisions

**1. Rust for the engine, YAML/Python for the periphery** (High confidence)
- Evidence: `src/adapter/declarative.rs` (~2K lines), `.llm-orc/scripts/`, ADR-020
- The declarative adapter and subprocess LLM client exist to let Rust's boundary stop at the engine.

**2. MCP chosen for client ecosystem** (High confidence)
- Evidence: Bidirectional `rmcp` usage (server to consumers, client to `llm-orc`), ADR-014
- MCP was the lingua franca of the surrounding Claude-adjacent tooling.

**3. SQLite by default assumption, formally deferred** (High confidence)
- Evidence: ADR-002 status "Deferred"; `rusqlite` with `bundled` feature; `GraphStore` trait exists but only `SqliteStore` implements it.
- The zero-infrastructure choice for an MCP tool server.

**4. Dimensions added post-hoc** (High confidence)
- Evidence: Schema migration comments, `serde(default)` guards, `#[allow(dead_code)]` on dimension constants with "Phase 5.1+" comment.
- Originally a flat graph model; dimensions grafted on for multi-modal support.

**5. Provenance progressively elevated** (High confidence)
- Evidence: ADR-001 "UPDATED by Essay 12, strengthened: bidirectional"; `ContentType::Provenance` as first-class content type.
- From metadata to first-class dimension via Essay 12 inflection point.

**6. Two parallel extraction paths not yet converged** (High confidence)
- Evidence: `SemanticAdapter` (1,985 lines Rust) and `DeclarativeAdapter` (1,996 lines) coexist; ADR-028 planned convergence is aspirational.
- The ADR decided the future; the code has not arrived there.

---

## Tradeoff Map

| Optimizes For | At the Expense Of | Evidence |
|--------------|-------------------|----------|
| Durability | Write throughput | Persist-per-emission (engine_sink.rs:L297); enrichment loop triggers 12+ SQLite transactions per ingest |
| Correctness (always-consistent weights) | Throughput at scale | O(edges × adapters) normalization per emission (context.rs:L302-346) |
| Extensibility | Type safety | `Box<dyn Any>` adapter input (traits.rs:L18); runtime downcast failures |
| Read performance | Memory boundedness | Full context clone per read (engine.rs:L152); no eviction |
| Model uniformity | Schema enforcement | Provenance as graph nodes (provenance/api.rs:L44-65); silent defaults on missing properties |
| Test convenience | Production safety | `ExtractionCoordinator::new()` without backend (extraction.rs:L86-97); Phases 2-3 silently skip |
| Backward compatibility | Vocabulary clarity | `combined_weight` serialized as `raw_weight` (edge.rs:L88-90); dual naming |
| Smooth transition | Code health | `parse_response()` vestigial method (semantic.rs:L212-297); contradicts ADR-003 contribution invariant |

---

## Findings

### Macro Level

#### Finding: Hexagonal Write Path, Direct Read Path

**Observation:** The write path implements a clean hexagonal architecture: `Adapter` trait → `AdapterSink` → `EngineSink` → `PlexusEngine`. The read path bypasses this entirely: `PlexusApi` calls `self.engine` directly for `find_nodes()`, `traverse()`, `find_path()`.
- `src/adapter/traits.rs:L54-81` — `Adapter` trait as the outward-facing port
- `src/adapter/sink.rs:L124-131` — `AdapterSink` as the inner port
- `src/api.rs:L136-163` — Read methods call `self.engine` directly
- `src/api.rs:L9-22` — Module doc calls the split "intentional"

**Pattern:** Ports-and-Adapters with asymmetric discipline. The write side has full port boundaries; the read side has none.

**Tradeoff:** Optimizes for read performance and simplicity at the expense of uniform abstraction. If a future transport needs read-path middleware (auth, caching, observability), there is no interception point.

**Question:** What would it cost to add per-context access control to graph reads — and does the asymmetry between the write and read paths make that harder?

**Stewardship:** This appears to be a conscious tradeoff — no action needed. Consider annotating the read methods in `PlexusApi` to explain that the direct engine access is intentional, so future maintainers don't interpret it as debt.

---

#### Finding: Event-Driven Enrichment Loop Inside Request-Response Shell

**Observation:** After every primary write, `IngestPipeline` runs an enrichment loop (up to 10 rounds) until quiescence or safety valve. Four enrichments are registered by default; an optional fifth behind `embeddings` feature.
- `src/adapter/enrichment_loop.rs:L32-108` — Loop with safety valve at `max_rounds`
- `src/adapter/enrichment.rs:L24-35` — `Enrichment` trait: receives `events: &[GraphEvent]`, returns `Option<Emission>`
- `src/adapter/ingest.rs:L244-252` — Loop runs after all adapter emissions complete (ADR-029)

**Pattern:** Internal event-driven reactive loop. Events trigger reactions, reactions produce new events, process runs to fixpoint.

**Tradeoff:** Optimizes for compositional, decoupled enrichment logic at the expense of unpredictable per-request latency. Maximum cost is O(rounds × enrichments × context clone cost).

**Question:** What is the worst-case duration of a single `ingest()` call when all four enrichments are active and each fires in cascade?

**Stewardship:** Consider surfacing enrichment loop telemetry (`rounds`, `quiesced`) as part of the return value or debug log, not just as a warning on safety-valve failure.

---

#### Finding: Write-Through Cache with Per-Emission Persistence

**Observation:** Every `emit()` call through `EngineSink` triggers a full `save_context()` to SQLite. The enrichment loop can trigger 12+ separate persist transactions per ingest call.
- `src/graph/engine.rs:L203-219` — `with_context_mut()` persists synchronously after every mutation
- `src/adapter/engine_sink.rs:L383-396` — Each `emit()` triggers a full context save
- `src/storage/sqlite.rs:L415-532` — `save_context()` opens `BEGIN IMMEDIATE`, iterates all nodes and edges

**Pattern:** Write-Through Cache with Eager Consistent Persistence.

**Tradeoff:** Optimizes for durability and crash safety at the expense of write throughput. An adapter emitting 3 times triggers 3 full SQLite writes.

**Question:** What happens to write throughput when a context grows to tens of thousands of nodes?

**Stewardship:** The write-through behavior is appropriate for an MCP tool server. If high-throughput ingestion is needed, the path to improvement is batching across an adapter's full `process()` call.

---

#### Finding: Type-Erased Plugin Contract via `Box<dyn Any>`

**Observation:** The entire ingest path uses `Box<dyn Any + Send + Sync>` as the data payload. Each adapter performs its own downcast. There is no compile-time contract between caller and adapter about the expected type.
- `src/adapter/traits.rs:L14-19` — `AdapterInput.data: Box<dyn Any + Send + Sync>`
- `src/api.rs:L64-71` — Public `ingest()` takes `Box<dyn Any + Send + Sync>`
- `src/mcp/mod.rs:L172-175` — MCP transport passes `serde_json::Value`; Rust callers pass typed structs

**Pattern:** Type Erasure as Extensibility Mechanism. Two implicit protocols (JSON from MCP, typed structs from Rust) ride the same interface.

**Tradeoff:** Optimizes for extensibility and transport independence at the expense of type safety and discoverability. Wrong-type errors surface only at runtime.

**Question:** What is the developer experience for a new contributor who wants to add an adapter — how do they discover the expected input type?

**Stewardship:** Document each adapter's expected input type in a central table. For the static adapter set, consider a typed enum dispatch to buy back compile-time safety.

---

#### Finding: Provenance as First-Class Graph Dimension

**Observation:** Chains, marks, and links are `Node` instances with `dimension = PROVENANCE`. `ProvenanceApi` is a read-only view that filters the shared node/edge store.
- `src/provenance/api.rs:L44-65` — `list_chains()` filters nodes by `node_type == "chain" && dimension == PROVENANCE`
- `src/adapter/types.rs:L291-307` — `chain_node()` and `mark_node()` factory helpers
- `src/api.rs:L529-533` — `ProvenanceApi` constructed transiently per call

**Pattern:** Unified Data Model (everything-is-a-node). Cross-dimension traversal is free; schema enforcement is not.

**Tradeoff:** Optimizes for model uniformity and cross-dimension traversal at the expense of schema enforcement. Missing properties fall back to `unwrap_or("")`.

**Question:** What happens when a mark node is created without a `file` property — silent empty string or loud failure?

**Stewardship:** Consider logging a warning in `node_to_mark_view()` when required properties are absent.

---

#### Finding: Declarative Adapter Extension Point (YAML DSL)

**Observation:** Beyond compiled adapters, the system supports loading adapter specs from YAML at runtime. A mini-language of primitives (`create_node`, `create_edge`, `for_each`, `hash_id`) with a template engine.
- `src/adapter/ingest.rs:L85-138` — `register_specs_from_dir()` scans for `*.yaml`, invalid specs are warned and skipped
- `src/adapter/declarative.rs:L1-10` — "External consumers write YAML specs instead of Rust"
- `src/mcp/mod.rs:L98-111` — Specs loaded on MCP server startup

**Pattern:** Plugin Configuration DSL. Reduces barrier to extending the system but introduces a second execution model.

**Tradeoff:** Optimizes for end-user extensibility at the expense of debuggability and robustness. Template rendering errors surface at runtime.

**Question:** When a YAML template produces a malformed node ID, how does that propagate through the pipeline?

**Stewardship:** Consider adding a `validate` subcommand or dry-run option to detect template errors before production deployment.

---

### Meso Level

#### Finding: graph/ Is a Stable Hub; adapter/ Has Intimate Coupling to Its Internals

**Observation:** `graph/` has zero outward dependencies on `adapter/`, `query/`, or `provenance/` (except `graph/engine.rs → query/` for query execution). However, `EngineSink` directly accesses `Context.nodes` and `Context.edges` as public fields.
- `src/adapter/engine_sink.rs:L281` — `ctx.edges[idx]` direct index
- `src/adapter/engine_sink.rs:L329-343` — `ctx.edges.retain(...)` bypassing Context methods
- `src/adapter/engine_sink.rs:L355-363` — `ctx.nodes.remove(...)` direct mutation

**Pattern:** Inappropriate Intimacy (Code Smell). `EngineSink` is functionally a friend of `Context`, performing structural mutations that `Context` could encapsulate.

**Tradeoff:** Optimizes for keeping the 5-phase emission logic readable in one place at the expense of encapsulation.

**Question:** What would each emit phase look like as a method on `Context`, and would that make invariants easier to enforce?

**Stewardship:** Add `Context::remove_node()`, `Context::retain_edges()` methods. Make `nodes` and `edges` `pub(crate)` to prevent external access.

---

#### Finding: GraphEvent Defined in graph/ but Carries Adapter-Layer Semantics

**Observation:** `GraphEvent` lives in `src/graph/events.rs` but all 6 variants carry `adapter_id` and `context_id` — adapter-layer concepts.
- `src/graph/events.rs:L11-50` — All variants carry `adapter_id: String`
- `src/adapter/mod.rs:L34` — Re-exports `GraphEvent` from `graph/`

**Pattern:** Misplaced Type. Adapter semantics embedded in the core layer to avoid circular dependency.

**Tradeoff:** Optimizes for having the engine fire events without depending on adapter/ at the expense of the graph layer knowing about adapter IDs.

**Question:** What would the graph layer's event type look like if it carried only structural information?

**Stewardship:** Consider splitting into lean `StructuralEvent` (in `graph/`) and richer `AdapterEvent` (in `adapter/`).

---

#### Finding: mcp/ Is the Composition Root, Carrying Direct Adapter Construction Knowledge

**Observation:** `mcp/mod.rs` imports 10+ concrete adapter/enrichment types by name to wire the pipeline. Adding a new adapter requires touching this file.
- `src/mcp/mod.rs:L14-18` — Imports `CoOccurrenceEnrichment`, `ContentAdapter`, etc.
- `src/mcp/mod.rs:L66-110` — Full pipeline construction with enrichment vec

**Pattern:** Composition Root embedded in a transport module.

**Tradeoff:** Optimizes for a single, clear entry point at the expense of coupling the MCP transport to all concrete types. A second transport would replicate wiring.

**Question:** What would break if pipeline construction moved to a `PlexusApi::default_pipeline()` builder?

**Stewardship:** Extract pipeline construction into a transport-neutral location so future transports can reuse it.

---

#### Finding: Edges Reference Valid Nodes Only at Write Time; No Storage-Level Enforcement

**Observation:** The "edges reference existing nodes" invariant is enforced only in `EngineSink.commit_edges()`. `Context::add_edge()`, `PlexusEngine::add_edge()`, and SQLite have no endpoint validation.
- `src/adapter/engine_sink.rs:L244-258` — Checks endpoints before committing
- `src/graph/context.rs:L193-257` — `add_edge()` has no endpoint check
- `src/storage/sqlite.rs:L56-59` — No `FOREIGN KEY` on source/target

**Pattern:** Layered validation with a gap. Pipeline validates; internal helpers and storage do not.

**Tradeoff:** Optimizes for pipeline throughput at the expense of making the invariant breakable from direct engine callers.

**Question:** What would prevent a future integration from calling `PlexusEngine::add_edge()` directly and accumulating dangling edges?

**Stewardship:** Either add endpoint check to `Context::add_edge()`, document the caller obligation on `PlexusEngine::add_edge()`, or add SQLite foreign keys.

---

#### Finding: Enrichment Loop Truncation Is Silent — `quiesced: false` Never Surfaces

**Observation:** When the safety valve fires, `tracing::warn!` is logged. The `quiesced: false` field in `EnrichmentLoopResult` is discarded by `IngestPipeline`. Callers cannot distinguish truncated from fully-converged results.
- `src/adapter/enrichment_loop.rs:L96-101` — Safety valve emits `tracing::warn!` only
- `src/adapter/ingest.rs:L246-253` — Extracts `events` from result; discards `quiesced`

**Pattern:** Soft-fail with observability-only signaling.

**Tradeoff:** Optimizes for robustness (a buggy enrichment doesn't brick the pipeline) at the expense of correctness transparency.

**Question:** What would a consumer do differently if they knew their ingest call returned with a truncated enrichment loop?

**Stewardship:** Expose `quiesced` via `OutboundEvent` or as a flag on the return. At minimum, log it in the MCP transport.

---

#### Finding: ProposalSink Is Documented But Does Not Exist

**Observation:** MEMORY.md and prior architecture descriptions reference `ProposalSink` as enforcing `may_be_related` only, weight cap, and no removals. No such type exists anywhere in `src/`.
- `src/adapter/mod.rs` — No `ProposalSink` export
- Grep for `ProposalSink` across `src/` — zero matches

**Pattern:** Documented invariant with no implementation.

**Tradeoff:** The current system has no mechanism to distinguish proposal-mode (tentative) from authoritative emissions.

**Question:** What behavior does the system exhibit when an enrichment generates a `may_be_related` edge — is it treated with the same authority as a `calls` edge?

**Stewardship:** Remove `ProposalSink` from documentation (MEMORY.md, scenarios) or implement it. The current state overstates the system's enforcement capabilities.

---

#### Finding: ADR-028 Describes a System That Doesn't Yet Exist

**Observation:** ADR-028 declares `DeclarativeAdapter` renamed to `SemanticAdapter` and the existing hardcoded `SemanticAdapter` reimplemented as a YAML engine. Both types still exist as independent ~2K-line implementations.
- `docs/adr/028-universal-mcp-ingest.md:L46` — "The term `DeclarativeAdapter` is retired"
- `src/adapter/mod.rs:L44` — `pub use declarative::DeclarativeAdapter;` still exported
- `src/adapter/semantic.rs:L121` — `SemanticAdapter` struct with hardcoded parsing

**Pattern:** Aspirational documentation. The ADR decided the future; the code has not arrived.

**Tradeoff:** Optimizes for recording architectural intent early at the expense of documentation accuracy.

**Question:** What does a reader learn about actual system behavior from ADR-028 today?

**Stewardship:** Either update ADR-028 to reflect current state ("convergence deferred") or complete the convergence.

---

#### Finding: Multiple Documentation Drift Points

**Observation:** Several documented claims differ from implementation:
- `docs/adr/014-transport-independent-api.md:L56` — Claims `PlexusApi` holds `ProvenanceApi` reference; it's constructed transiently per call
- `docs/adr/010-enrichment-trait-and-loop.md:L30` — Describes per-emission enrichment loop; ADR-029 changed to per-pipeline-call
- `docs/adr/010-enrichment-trait-and-loop.md:L52` — Claims `max_rounds` is configurable; not exposed in public API
- `src/graph/node.rs:L11-12` — "Staged for Phase 5.1+" on actively-used dimension constants
- `docs/scenarios/003-reinforcement-mechanics.md:L49-55` — References `ProposalSink` which doesn't exist
- `docs/adr/028-universal-mcp-ingest.md:L44` — Struck-through `annotate()` retained note, but method was subsequently removed

**Pattern:** Version drift, aspirational documentation, and comment rot at multiple sites.

**Tradeoff:** Optimizes for low-friction documentation at the expense of accuracy.

**Question:** What would a new contributor conclude about the system's current behavior from reading these documents?

**Stewardship:** A documentation sweep to align the 6 identified drift points with actual code. Highest-value targets: ADR-028 convergence status and `node.rs` dimension comments.

---

### Micro Level

#### Finding: Duplicated Response Parsing Logic in SemanticAdapter

**Observation:** `SemanticAdapter` has four parse methods (`parse_response`, `parse_agent_response`, `parse_spacy_response`, `parse_themes`) with structurally identical inner loops. `NodeId::from_string(format!("concept:{}", ...))` is hand-rolled in 6+ places despite `concept_node()` in `types.rs` already encapsulating the normalization.
- `src/adapter/semantic.rs:L212-296` — `parse_response()` (vestigial, backward compat)
- `src/adapter/semantic.rs:L427-491` — `parse_agent_response()` standard branch
- `src/adapter/semantic.rs:L530-556` — `parse_spacy_response()` relationships
- `src/adapter/semantic.rs:L597-638` — `parse_themes()`

**Pattern:** Code Smell — Duplicated Code (Dispensables) combined with Feature Envy toward `types::concept_node`.

**Tradeoff:** Optimizes for parser-per-shape locality at the expense of consistency. Changing the concept node ID scheme requires touching 6+ sites.

**Question:** What would break if the concept node ID scheme changed from `concept:{label}` to something else?

**Stewardship:** Collapse concept-loop variants into a shared helper. Replace inline `NodeId::from_string` calls with `concept_node()`. Retire `parse_response()`.

---

#### Finding: ExtractionCoordinator Silently Skips Phases 2-3 Without Backend

**Observation:** `ExtractionCoordinator::new()` requires no backend. MCP server registers it without `.with_engine()`. Phase 1 runs; Phases 2 and 3 are silently skipped with `Ok(())` returned.
- `src/adapter/extraction.rs:L86-97` — Constructor requires no backend
- `src/mcp/mod.rs:L70` — `pipeline.register_adapter(Arc::new(ExtractionCoordinator::new()))` — no `.with_engine()`
- `src/adapter/extraction.rs:L461-558` — Background phases only execute if `has_backend`

**Pattern:** Code Smell — Speculative Generality / Anti-Pattern — Silent Misconfiguration.

**Tradeoff:** Optimizes for test convenience at the expense of production safety.

**Question:** What signal would an operator have that extraction is silently producing only Phase 1 output?

**Stewardship:** Either require backend at construction or return an error when phase 2 adapter is registered but no backend is configured.

---

#### Finding: O(edges × adapters) Normalization Per Emission

**Observation:** `recompute_combined_weights()` scans the entire edge set twice after every edge commit, including single-edge enrichment rounds.
- `src/graph/context.rs:L302-346` — Two full passes over `self.edges`
- `src/adapter/engine_sink.rs:L297-299` — Called unconditionally on any edge commit

**Pattern:** Code Smell — Bloater (global work for local change).

**Tradeoff:** Optimizes for correctness at the expense of throughput. 8 agents × 2 passes × 10K edges = 160K comparisons per document.

**Question:** What would it cost to maintain running per-adapter min/max alongside the edge set?

**Stewardship:** Track which adapters had their range affected. A dirty-flag per adapter would reduce most recomputes to O(edges × 1 adapter).

---

#### Finding: `pulldown-cmark` Is a Dead Dependency

**Observation:** Listed in `[dependencies]` with zero usage anywhere in `src/`.
- `Cargo.toml:L30` — `pulldown-cmark = "0.10"`
- Grep for `pulldown`, `cmark` — zero matches

**Pattern:** Dead dependency.

**Tradeoff:** Increases compile times and dependency surface for no benefit.

**Question:** Was this previously used for Markdown parsing, and what replaced it?

**Stewardship:** Remove from `Cargo.toml`. Run `cargo build` to confirm.

---

#### Finding: `ContentType::Movement`, `Narrative`, `Agent` Are Phantom Variants

**Observation:** These enum variants exist only in `FromStr`/`resolve_content_type` mappers. No adapter, test, or YAML spec creates nodes with these types.
- `src/graph/node.rs:L87-102` — Variants declared
- `src/adapter/declarative.rs:L246-247` — String mapper resolves them
- No production callsite creates these types

**Pattern:** Feature Ghosts — domain concepts preserved in the type system with no data pathway activating them.

**Tradeoff:** Optimizes for type-system domain richness at the expense of silent confusion about what the system actually handles.

**Question:** What existing adapter would create a `ContentType::Movement` node?

**Stewardship:** Either write acceptance tests that exercise these pathways or mark with `#[allow(dead_code)]` and a doc comment linking to the motivating ADR.

---

#### Finding: `parse_response()` Is Vestigial and Contradicts ADR-003

**Observation:** `parse_response()` is only called by two `#[ignore]` integration tests. It does not emit contribution keys, contradicting ADR-003's reinforcement mechanics invariant.
- `src/adapter/semantic.rs:L212-297` — Method creates edges with `combined_weight` but no `contributions`
- `src/adapter/semantic.rs:L347-366` — `process()` calls only `parse_agent_response`

**Pattern:** Vestigial Structure with latent correctness defect.

**Tradeoff:** Optimizes for smooth transition at the expense of a method whose behavior contradicts a system invariant.

**Question:** What incorrect assumptions would a developer form by reading the live integration test that uses `parse_response`?

**Stewardship:** Update the two live tests to use `parse_agent_response`, then delete `parse_response`.

---

### Multi-Lens Observations

#### Convergence: Persist-Per-Emission + O(N) Normalization + Enrichment Loop (3 lenses)
Pattern Recognition, Architectural Fitness, and Structural Health all converged on the same performance concern: the combination of per-emission SQLite writes, full-context normalization, and multi-round enrichment creates a latency profile that scales poorly. A context with 10K edges and 4 enrichments running 3 rounds could trigger 12+ full-context serializations with 20K comparisons each. This is the single most impactful performance constraint.

#### Convergence: Documentation vs. Reality Gap (3 lenses)
Decision Archaeology, Documentation Integrity, and Intent-Implementation Alignment all found the same pattern: ADRs that describe planned states as decided states (ADR-028 convergence, ADR-010 configurability, ADR-014 struct composition). The documentation is well-intentioned but has not been maintained as the code evolved.

#### Convergence: Type Erasure and Invariant Enforcement (3 lenses)
Dependency & Coupling, Invariant Analysis, and Test Quality all found that the `Box<dyn Any>` contract interacts with contribution tracking: anonymous emissions (no `FrameworkContext`) silently bypass contribution tracking, the MCP transport passes `serde_json::Value` while Rust callers pass typed structs, and no test verifies the anonymous bypass behavior. Three separate concerns that compound into a single systemic risk.

---

## Stewardship Guide

### What to Protect

1. **The hexagonal write path.** The adapter → sink → engine layering is clean and well-documented. The `IngestPipeline` as the single write entry point (ADR-012/028) is a genuine strength.

2. **The unified data model.** Everything-is-a-node enables `evidence_trail` and cross-dimension traversal with no additional infrastructure. This is the architectural decision that makes the knowledge graph uniquely powerful.

3. **The ADR trail.** Thirty ADRs and 26+ essays provide extraordinary architectural context. Even where documentation has drifted, the intent and reasoning are preserved and valuable.

4. **The enrichment loop design.** Event-driven reactive enrichment with cooperative quiescence is elegant and composable. New enrichments can be added without modifying existing ones.

5. **The test suite.** ~425 tests with good coverage of the adapter layer's core behavior. The recent work adding assertion messages to `EngineSink` tests is a model for the rest.

### What to Improve (Prioritized)

1. **Close the documentation-reality gap** — Highest value, lowest effort. Update ADR-028 (convergence status), ADR-010 (trigger model, configurability), ADR-014 (struct composition), `node.rs` dimension comments, and scenarios-003 `ProposalSink` reference. Remove `ProposalSink` from MEMORY.md. This prevents future contributors from building on false premises.
   - Findings: ADR-028 aspirational, ADR-010 version drift, dimension comment rot, ProposalSink phantom

2. **Fix `ExtractionCoordinator` silent misconfiguration** — The MCP server registers it without `.with_engine()`, silently producing only Phase 1 output. Either require the backend at construction or return an error.
   - Findings: ExtractionCoordinator silent skip, dual construction paths

3. **Add telemetry to the write path** — `tracing::debug!` spans around adapter dispatch, enrichment loop rounds, and emission commits. Surface `EnrichmentLoopResult.quiesced` in the return or log. Currently ~12 tracing calls in the entire non-test codebase; the production write path is nearly silent.
   - Findings: Silent production system, enrichment loop truncation silent

4. **Remove dead code** — Delete `pulldown-cmark` from `Cargo.toml`, remove `parse_response()` from `semantic.rs` (update 2 `#[ignore]` tests to use `parse_agent_response`), remove blanket `#[allow(dead_code)]` from dimension module (apply precisely to actually-unused constants), clean unused dev-deps.
   - Findings: Dead dependency, vestigial `parse_response`, imprecise `#[allow(dead_code)]`, phantom ContentType variants

5. **Add assertion messages to integration tests** — ~77 silent assertions in `integration_tests.rs`. Add message arguments to `assert_eq!`/`assert_ne!` calls. The `engine_sink.rs` tests are already a model.
   - Finding: Assertion Roulette at scale

6. **Fill test gaps for identified invariant boundaries** — Add tests for: enrichment loop `quiesced: false`, anonymous edge emission bypass, edge endpoint validation in `Context::add_edge`, `combined_weight` staleness after `apply_mutation`.
   - Findings: 4 test-code correspondence gaps

7. **Encapsulate `Context` fields** — Add `Context::remove_node()`, `Context::retain_edges()` methods. Consider making `nodes` and `edges` `pub(crate)`. This is a prerequisite for future `Vec<Edge>` → `HashMap` migration.
   - Findings: Inappropriate Intimacy, `Vec<Edge>` O(N) scans

8. **Extract pipeline construction from mcp/** — Move pipeline wiring to a transport-neutral location so future transports can reuse it without importing all concrete adapter types.
   - Finding: mcp/ as composition root

9. **Consider batched normalization / persist** — For performance at scale: batch enrichment-loop persists to once per ingest call; track per-adapter dirty flags to reduce normalization from O(edges × adapters) to O(edges × changed-adapters).
   - Findings: Persist-per-emission, O(N) normalization convergence

10. **Replace Mutex `.unwrap()` with error propagation** — In `storage/sqlite.rs` and `mcp/mod.rs`, replace `lock().unwrap()` with `lock().map_err(...)` to prevent panic cascades from mutex poisoning.
    - Finding: Mutex poison panic risk

### Ongoing Practices

- **Review ADRs when code changes diverge from their decisions.** The documentation-reality gap is the result of ADRs not being updated when implementation changed. A commit that modifies behavior described by an ADR should include an ADR update in the same PR.

- **Add assertion messages to new test code.** The `engine_sink.rs` pattern (messages on every assertion) should be the standard for all new tests.

- **Log enrichment loop behavior at `debug` level.** When adding new enrichments, verify they reach quiescence within the safety valve by running with `RUST_LOG=plexus=debug`.

- **Test with the `embeddings` feature periodically.** The embeddings integration path has no CI coverage. Manual verification after `fastembed` or `sqlite-vec` version bumps prevents silent regression.

- **Validate YAML adapter specs before deployment.** The declarative adapter path has no dry-run validation. Until a `validate` subcommand exists, test specs against synthetic input before adding them to production `adapter-specs/` directories.
