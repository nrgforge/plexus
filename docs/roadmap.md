# Roadmap: Plexus

**Last updated:** 2026-04-07 (ARCHITECT complete — MCP consumer interaction surface cycle)
**Derived from:** System Design v1.2, ADR-036, ADR-037, Invariants 60–62, Reflection 003, product-discovery.md (2026-04-02), conformance scan 036-037

## Current State

**Active cycle:** MCP consumer interaction surface — BUILD in progress. WP-A through WP-G.2 complete (see Completed Work Log for commit trail). **WP-H pending** — the cycle's e2e acceptance criterion (first real MCP consumer workflow) requires live MCP transport verification, which was not delivered by WP-A through WP-G.2. WP-H also folds in a scope-reductive design correction: remove file-based spec auto-loading in favor of intentional `load_spec` only.

The cycle adds runtime spec loading (ADR-037) and exposes the full query surface via MCP (ADR-036). No new modules, no new dependency edges — all work flows through existing seams. The central new capability is that persisted lens enrichments rehydrate at library construction time, making vocabulary layers a durable property of the **context** rather than the **consumer process**. When any consumer holds the library against a context, it transiently runs every lens registered on that context — so cross-pollination between consumer domains happens automatically.

## Work Packages

### WP-A: Fix `register_specs_from_dir` (conformance debt)

**Objective:** Close conformance scan Violation 3 — `register_specs_from_dir` currently calls `register_adapter()` only, silently dropping each spec's `enrichments()` and `lens()`. Any existing deployment using file-based spec auto-discovery has broken enrichment wiring today. This is a two-line fix that can ship independently, ahead of the rest of the cycle.

**Changes:**
- `adapter/pipeline/ingest.rs` (or wherever `register_specs_from_dir` lives) — replace the `register_adapter(Arc::new(adapter))` call with the equivalent of `register_integration`, extracting enrichments and lens from each `DeclarativeAdapter` before registering
- One regression test: given a spec directory containing a spec with enrichments + lens, assert all three are registered on the pipeline

**Scenarios covered:** scenarios/037 § "register_specs_from_dir wires enrichments and lens"

**Dependencies:** None. **Open choice.**

**Risk:** minimal. The fix makes broken behavior correct; it cannot make working behavior broken.

---

### WP-B: Specs persistence foundation + runtime registration

**Objective:** Establish all the infrastructure for runtime spec loading without exposing any user-facing surface. After WP-B, the system behaves identically to before — but the machinery for WP-C/D is in place. This is the riskiest WP of the cycle because it introduces interior mutability into `IngestPipeline` for the first time.

**Changes:**
- `storage/sqlite.rs` — add `migrate_add_specs_table()` migration creating the `specs` table with composite PK `(context_id, adapter_id)` per ADR-037 §2 schema
- `storage/traits.rs` — define `PersistedSpec` struct (fields: `context_id`, `adapter_id`, `spec_yaml`, `loaded_at`) as the return type for specs queries. **Use a struct, not a tuple** — this enables non-breaking evolution of the persisted-spec shape (additional fields can be added without breaking callers).
- `storage/traits.rs` — add `persist_spec`, `query_specs_for_context(context_id) -> Vec<PersistedSpec>`, `delete_spec` methods to `GraphStore` trait, all with default no-op implementations (same extensibility pattern as `persist_event`)
- `storage/sqlite.rs` — implement the three new methods on `SqliteStore`
- `graph/engine.rs` — add `PlexusEngine::persist_spec`, `query_specs_for_context`, `delete_spec` methods that delegate to `GraphStore` (parallel to existing `persist_event`/`query_events_since`)
- `adapter/pipeline/ingest.rs` — introduce interior mutability on the adapter vector and enrichment registry. **Tier 1 decision (BUILD-time, local):** `RwLock<Vec<Arc<dyn Adapter>>>` + `RwLock<EnrichmentRegistry>` is the leading candidate since registration is infrequent relative to reads. Alternative: `arc-swap::ArcSwap` if copy-on-write semantics fit the access pattern better. Both are interchangeable — can swap between them without touching callers. **Tier 2 restructuring (OUT OF SCOPE for this cycle):** moving adapter storage to a keyed collection (e.g., `DashMap<AdapterId, ...>`) is a structural change, not a concurrency-primitive swap, because it changes how adapters are indexed (Vec iteration order → hash-keyed lookup). If BUILD discovers a reason to want this, **pause and escalate** — it deserves its own architectural decision.
- `adapter/pipeline/builder.rs` — add `PipelineBuilder::with_persisted_specs(specs: Vec<PersistedSpec>)` method that parses each spec, extracts the lens enrichment (via `DeclarativeAdapter::lens()`), and registers it on the pipeline being built. The adapter is NOT registered (lens-only rehydration per ADR-037 §2 startup path). Effect (a) is not re-run (vocabulary edges already persist). **The builder registers every spec passed in — no filtering logic, no selective loading.** The contract is: specs table is the context's lens registry, and holding a context means running all its lenses.
- Boundary tests for every new seam: specs migration, trait default no-op, SqliteStore implementations, engine delegation, builder rehydration

**Scenarios covered:** None directly — foundational infrastructure. Scenarios 037 are covered by WP-C and WP-D (rehydration path).

**Dependencies:** None. **Open choice.** Can be built in parallel with WP-A.

**Risk:** moderate. Interior mutability is a load-bearing concurrency change. The fitness criterion ("interior mutability scope confined to adapter vector and enrichment registry, no lock held across ingest()") is the discipline that keeps this safe. Recommend holding a review checkpoint between WP-B and WP-C specifically on the interior mutability choice.

---

### WP-C: `load_spec` / `unload_spec` on PlexusApi

**Objective:** Expose the three-effect model (Invariant 62) as a runtime API operation. This is the main functional payload of the cycle — after WP-C, embedded Rust consumers can load specs at runtime (MCP consumers still cannot until WP-F).

**Changes:**
- `api.rs` — add `load_spec(context_id: &str, spec_yaml: &str) -> Result<SpecLoadResult, SpecLoadError>`. Flow: validate spec YAML → parse `DeclarativeAdapter::from_yaml` → extract adapter + enrichments + lens → register on pipeline via WP-B's interior mutability path → write spec row to storage via WP-B's engine method → run lens enrichment over existing context content to produce initial vocabulary layer → return `SpecLoadResult` with adapter_id, registered enrichment IDs, lens namespace, vocabulary edges created count.
- `api.rs` — add `unload_spec(context_id: &str, adapter_id: &str) -> Result<(), SpecUnloadError>`. Flow: remove adapter from routing → deregister lens enrichment → delete spec row from storage. Leaves vocabulary edges in the graph (Invariant 62).
- `api.rs` — define `SpecLoadResult`, `SpecLoadError`, `SpecUnloadError` types
- Validation: at minimum, YAML parses, `DeclarativeAdapter::from_yaml` succeeds, lens rules (if present) reference valid relationship type formats. Additional checks (e.g., lens rules reference existing relationship types in the graph) are a BUILD call — start conservative, expand if scenarios demand.
- All-or-nothing error handling: if any step fails after validation, the earlier steps must roll back. The fitness criterion ("fail-fast atomicity") is the discipline here.
- Acceptance tests exercising each scenario in scenarios/037

**Scenarios covered:** scenarios/037 § Spec Validation (all 4), Complete Spec Wiring (both), Lens Enrichment Execution (all 3), Spec Persistence § "loaded spec persists in SQLite", Spec Unloading (both), Vocabulary Layer Discovery.

**Dependencies:** **Hard** on WP-B (requires specs table, interior mutability, engine spec methods).

**Risk:** moderate. Rollback-on-failure is the tricky part. Decide upfront whether rollback is manual (undo each successful step on error) or transactional (do everything in a SQLite transaction + in-memory staging). Manual is simpler but error-prone; transactional is cleaner but requires threading a transaction handle through pipeline registration. Recommend manual rollback with clear invariant tests.

---

### WP-D: Startup spec re-instantiation via PipelineBuilder

**Objective:** Close the multi-consumer cross-pollination story. After WP-D, when any library instance is constructed against a context that has persisted specs, those specs' lens enrichments are automatically registered on the new pipeline — so future ingests by any adapter on that context trigger all registered lenses.

**Changes:**
- MCP binary's `main.rs` (or equivalent host code) — before constructing `PlexusApi`, read the specs table via the store, and pass the resulting `Vec<(ContextId, String)>` to `PipelineBuilder::with_persisted_specs`. The builder (already delivered in WP-B) handles the rest.
- Acceptance test: load spec via `PlexusApi::load_spec`, drop the api + pipeline + engine, reconstruct everything via the builder pointing at the same store, ingest via a different adapter, assert the persisted lens fires. This is the canonical test for Invariant 62 effect (b).
- Documentation of the host-side ceremony in the field guide when it's regenerated.

**Scenarios covered:** scenarios/037 § Spec Persistence § "persisted specs re-register on startup", "lens enrichment fires correctly after restart".

**Dependencies:** **Hard** on WP-B (needs `with_persisted_specs`) and WP-C (needs something that can write to the specs table, so the test can exercise load-then-rehydrate).

**Risk:** low. The builder already does the work; WP-D is just wiring the host and writing the acceptance test.

---

### WP-E: MCP query tools (6 thin wrappers)

**Objective:** Expose the existing `PlexusApi` query methods through MCP so LLM consumers can exercise the pull paradigm and composable filters. Six of the seven new MCP tools from ADR-036 §1 — the seventh (`load_spec`) is WP-F.

**Changes:**
- `mcp/mod.rs` — add `#[tool]` handlers for `find_nodes`, `traverse`, `find_path`, `changes_since`, `list_tags`, `shared_concepts`. Each takes flat optional parameter fields (never nested objects) and delegates to the corresponding `PlexusApi` method. Parameter mapping:
  - `find_nodes`: `node_type`, `dimension`, `contributor_ids`, `relationship_prefix`, `min_corroboration` → `FindQuery` + `QueryFilter`
  - `traverse`: `origin`, `max_depth`, `direction`, `rank_by`, `contributor_ids`, `relationship_prefix`, `min_corroboration` → `TraverseQuery` + `QueryFilter` + `RankBy`
  - `find_path`: `source`, `target`, `max_length`, `direction`, `contributor_ids`, `relationship_prefix`, `min_corroboration` → `PathQuery` + `QueryFilter`
  - `changes_since`: `cursor`, `event_types`, `adapter_id`, `limit` → `CursorFilter`
  - `list_tags`: no parameters beyond active context
  - `shared_concepts`: `context_a`, `context_b`
- Module header comment "Tools: 9 total" → "Tools: 16 total" (15 after WP-E, 16 after WP-F)
- One boundary integration test per tool asserting delegation to `PlexusApi`

**Scenarios covered:** scenarios/036 § Graph Query Tools, Event Cursor Tool, Discovery Tools, End-to-End Integration.

**Dependencies:** None. **Open choice.** Can be built in parallel with WP-A/B/C/D since the underlying API methods already exist.

**Risk:** low. Mechanical wrapping. Main risk is getting the flat-parameter-to-structured-type mapping right — mitigated by per-tool tests.

**Commit structure:** may split into 2 commits (e.g., find_nodes + traverse + find_path in one, changes_since + list_tags + shared_concepts in another) for reviewability. Single commit is also acceptable if the author prefers.

---

### WP-F: MCP `load_spec` tool

**Objective:** The seventh and final new MCP tool — the one that's not a thin wrapper. Exposes `PlexusApi::load_spec` through MCP so consumers can declare their identity at interaction time.

**Changes:**
- `mcp/mod.rs` — add `#[tool]` handler for `load_spec` that takes a single `spec_yaml: String` parameter (the full spec content inline, per ADR-036 §2) and delegates to `PlexusApi::load_spec`. Result marshalling: `SpecLoadResult` → JSON response with adapter_id, lens_namespace, vocabulary_edges_created count. Error marshalling: `SpecLoadError` → MCP `ErrorData`.
- Boundary test: real MCP handler invokes real `PlexusApi::load_spec` with a valid spec, asserts result shape; invokes with malformed YAML, asserts error shape.

**Scenarios covered:** scenarios/036 § Spec Loading Tool (both scenarios).

**Dependencies:** **Hard** on WP-C (needs `PlexusApi::load_spec` to exist).

**Risk:** low. One tool, well-defined wrapping.

---

### WP-G: `evidence_trail` + `QueryFilter` and `RankBy::NormalizedWeight`

**Objective:** Close two unrelated conformance debts from the query surface cycle, both of which land in the `query` module. Two independently-revertible commits within one WP.

**Sub-package G.1: evidence_trail gains optional QueryFilter (ADR-036 §5, Violation 2)**

**Changes:**
- `query/step.rs` — `StepQuery` accepts `filter: Option<QueryFilter>` (already present per ADR-034) threaded into the step-by-step traversal logic
- `api.rs` — `PlexusApi::evidence_trail` signature gains `filter: Option<QueryFilter>` parameter, piped through to the underlying `StepQuery` construction
- `mcp/mod.rs` — `evidence_trail` MCP tool gains flat optional filter fields (`contributor_ids`, `min_corroboration`; `relationship_prefix` included for API consistency but will typically return empty results for evidence trails — document this in tool description per ADR-036 §5)
- Tests: `api::tests::evidence_trail_accepts_filter` and at least one scenario per filter field

**Scenarios covered:** scenarios/036 § Evidence Trail Filter Conformance.

**Commit:** own `feat:` commit.

**Sub-package G.2: RankBy::NormalizedWeight wiring (ADR-034, Violation 1)**

**Changes:**
- `query/filter.rs` — add `RankBy::NormalizedWeight(Box<dyn NormalizationStrategy>)` variant
- Resolve the `Clone`/`Debug`/`PartialEq` derivation wart on the enum (trait objects don't auto-derive). Most likely approach: manual `Debug` impl that prints "NormalizedWeight(<strategy>)", and drop `Clone`/`PartialEq` from `RankBy` if not already required. If `Clone` IS required elsewhere, require `NormalizationStrategy: Clone` or use a different container (e.g., `Arc<dyn NormalizationStrategy>`).
- `query/types.rs` — `TraversalResult::rank_by()` handles the new variant by computing normalized weights via the injected strategy before sorting
- Test: `traverse::tests::rank_by_normalized_weight_uses_outgoing_divisive`
- If G.2's type-system wart turns out to be uglier than expected (e.g., it forces removing `Clone` from `RankBy` which cascades through downstream code), this sub-package can be reverted independently of G.1 without affecting the evidence_trail work.

**Commit:** own `feat:` commit, separate from G.1.

**Dependencies:** **Open choice.** G.1 and G.2 are independent of WP-A/B/C/D/E/F and of each other.

**Risk:** G.1 is low (small signature change, well-understood). G.2 is the unknown — the type-system wart is the reason this was deferred in WP-C originally. If the wart proves intractable, reverting G.2 leaves the ADR-034 conformance debt open, in which case: amend the ADR text with a supersession note explaining why the third variant was never implemented (per the user's standing principle — update ADRs when necessary or when genuinely superseded, not casually).

---

### WP-H: E2E verification + intentional-only spec loading

**Objective:** Satisfy the cycle's own acceptance criterion at the MCP transport layer. Product discovery (2026-04-02) named "first real MCP consumer workflow" as the end-to-end acceptance definition; WP-A through WP-G.2 built the components but did not verify them through live MCP framing. WP-H delivers that verification via a subprocess-driven acceptance test exercising the two-consumer happy path. In the same WP, a scope-reductive design correction lands: file-based spec auto-loading is removed so the e2e semantics are unambiguous (state comes from `load_spec` only, never from directory scanning at construction time).

This is the last WP in the cycle — it closes BUILD.

---

**Sub-package H.1: Remove file-based spec auto-loading (design correction)**

**Trigger:** Standing principle check during WP-G.2 reflection surfaced that file-based auto-discovery (`register_specs_from_dir`, `with_adapter_specs`) violates consumer-intent (Invariant 61) and creates latent double-registration with persisted specs. Loading a spec should be intentional.

**ADR-037 supersession:**
- Add a dated supersession note at the head of ADR-037 citing the removal and rationale
- Strike or mark-as-superseded §4 ("Fix `register_specs_from_dir` to wire complete specs") and related lines in §2 (startup path) and the Consequences section
- The standing principle (ADR-037 ARCHITECT phase) applies: this IS a genuine supersession — the decision has changed, not the implementation

**Code removals:**
- `src/adapter/pipeline/ingest.rs`: delete `register_specs_from_dir` method (~60 lines)
- `src/adapter/pipeline/builder.rs`: delete `with_adapter_specs` builder method; remove its call from `default_pipeline` (~10 lines)
- `src/adapter/integration_tests.rs:4350`: delete the test that exercises `register_specs_from_dir`
- `tests/acceptance/spec.rs`: delete `register_specs_from_dir_wires_enrichments_and_lens` (tests removed code)

**Doc updates:**
- `docs/product-discovery.md` — remove "file on disk (auto-discovery at startup)" from spec ownership paragraph (line 11); remove/update the "File-based auto-discovery is one delivery path" assumption-inversion row (~line 161)
- `docs/domain-model.md` — audit the "load spec" action description; if it lists file-based as a delivery mechanism, update
- `docs/references/field-guide.md` — remove references to `with_adapter_specs` / `register_specs_from_dir` from the adapter/pipeline entry
- `docs/system-design.md` — check if the MCP cycle's Amendment 5 lists file-based loading; update if so

**Tests:**
- Verify `PipelineBuilder::with_persisted_specs` rehydration still works (orthogonal path — unaffected by removal, but worth an explicit test-pass confirmation)
- Verify `tests/acceptance/spec.rs::persisted_spec_rehydrates_across_restart` still passes — this is the canonical rehydration test
- Verify `two_consumers_two_lenses_on_same_context` still passes at API level

**Commit:** `refactor:` — scope reduction, no new behavior. ADR update included in same commit (the ADR lines the code implemented are removed together).

**Dependencies:** None. Can ship independently of H.2.

**Risk:** low. Code removal is localized. Main failure mode: a downstream crate/deployment relying on auto-discovery silently breaks. Plexus is pre-1.0 and the only known consumers are this workspace's own binary — no external breakage vector today.

---

**Sub-package H.2: Live MCP e2e harness (acceptance verification)**

**Trigger:** The cycle's product-discovery-defined acceptance criterion ("first real MCP consumer workflow") requires transport-level verification, not just API-level. Boundary tests at the MCP handler layer verify JSON delegation; they do not verify that the handler is reachable through actual MCP protocol framing (initialize handshake, JSON-RPC over stdio, tool dispatch by the rmcp framework).

**Objective:** One acceptance test that drives the compiled `plexus mcp` binary as a subprocess, speaks MCP to it over stdin/stdout, and verifies the two-consumer vocabulary-layer cross-pollination story end-to-end.

**Test architecture:**
- `tests/acceptance/mcp_e2e.rs` — new file
- Helper harness module (`tests/acceptance/mcp_harness.rs` or inlined) providing:
  - `McpHarness::spawn(db_path)` — spawn subprocess via `Command::new(env!("CARGO_BIN_EXE_plexus")).args(["mcp", "--db", ...])` with stdin/stdout piped, stderr captured for failure diagnostics
  - `send_initialize()` — MCP handshake (protocol version negotiation)
  - `call_tool(name, arguments) -> JsonValue` — serialize JSON-RPC request, write with newline, read response
  - `shutdown()` — close stdin, wait for exit with timeout (e.g., 5s), kill if timeout
- Client approach: **raw JSON-RPC** over stdin/stdout, not rmcp client types. Rationale: raw JSON-RPC is a stable wire protocol that the test exercises directly — it won't drift with rmcp crate version changes. A ~100 LoC helper is a cheaper long-term maintenance burden than an rmcp client dependency dance. If future needs grow, revisit.

**Hello-world scenario (the two-consumer cross-pollination story):**

1. **Setup:** Create temp dir, spawn binary pointing at temp SQLite path. Verify startup by calling MCP `initialize`.

2. **First consumer arrives:**
   - `set_context "test"` (auto-create)
   - `load_spec` with Consumer-1 YAML (adapter id `consumer-1-content`, input kind `consumer-1.fragment`, lens namespace `lens:consumer-1` with translation `may_be_related → thematic_connection`)
   - Assert: response JSON carries `adapter_id: "consumer-1-content"`, `lens_namespace: "lens:consumer-1"`, `vocabulary_edges_created: 0` (empty context)

3. **First consumer ingests:**
   - `ingest` with content containing two tags that co-occur (triggers `CoOccurrenceEnrichment`, emits `may_be_related` edges, Consumer-1's lens translates to `lens:consumer-1:thematic_connection`)
   - Assert: response JSON indicates event count > 0
   - `find_nodes` with `relationship_prefix: "lens:consumer-1:"` — assert non-empty result, concepts present

4. **Second consumer arrives on same context:**
   - `load_spec` with Consumer-2 YAML (distinct adapter id, input kind, lens namespace `lens:consumer-2`)
   - Assert: response JSON carries `lens_namespace: "lens:consumer-2"`, `vocabulary_edges_created > 0` (Consumer-2's lens sweeps existing edges from step 3)

5. **Second consumer ingests:**
   - `ingest` with its own content
   - Assert: response JSON indicates event count > 0

6. **Cross-pollination verification (THE cycle's point):**
   - `find_nodes` with `relationship_prefix: "lens:consumer-1:"` — assert edges exist from content ingested by BOTH consumers (Consumer-1's lens is still registered and fired on Consumer-2's ingest)
   - `find_nodes` with `relationship_prefix: "lens:consumer-2:"` — same assertion, mirrored
   - `find_nodes` with `relationship_prefix: "lens:"` — assert both namespaces appear

7. **Shutdown:** close stdin, wait for exit.

**Adapter choice for ingest:** use the built-in content adapter (tag-based `FragmentInput`). Rationale: deterministic, Rust-only (no llm-orc dependency), test-friendly. Semantic extraction via llm-orc is out of scope for this test — verifying transport, not extraction.

**Commit:** `test:` — test-only addition (after H.1's `refactor:` commit). No production code changes.

**Dependencies:** Hard on H.1 (needs unambiguous "state comes from `load_spec` only" semantics). Hard on all prior WPs (test exercises the full surface).

**Risk:** moderate — harness code has multiple failure modes.
- Subprocess startup timing: binary may need compile + cold start. Mitigation: use debug-profile binary (fast compile), `initialize` handshake serves as readiness check.
- Stdout framing: MCP responses are line-delimited JSON. Need a `BufReader` reading lines, with timeout per-read.
- Log output on stderr: tracing logs go to stderr. Capture but don't parse; surface on failure for diagnostics.
- Test flakiness risk: subprocess tests are historically flakier than in-process. Mitigation: use tokio timeout wrappers; ensure deterministic Consumer-1/Consumer-2 content; assert on SETS of node/edge IDs, not ordered lists.
- CI cost: test compiles the binary. Acceptable for one test; if the harness grows to many tests, move to a dedicated binary target.

---

## Open questions raised during WP-H planning

These are decisions or investigations that WP-H implementation will need to resolve:

**OQ-H1 — rmcp protocol version negotiation.** What protocol version does the current `rmcp` crate use by default, and what should the harness send in `initialize`? First step of WP-H.2 is to read the rmcp crate or run the binary manually and observe its initialize behavior.

**OQ-H2 — default_pipeline semantics after H.1.** After removing `with_adapter_specs` from `default_pipeline`, the remaining pipeline construction is: default adapters + default enrichments + persisted-spec rehydration (from `with_persisted_specs`). Is there any gap where a consumer expected file-based specs but now gets nothing? The WP-D acceptance test establishes the answer: persisted specs still rehydrate at construction. But anything that dropped a YAML file into `adapter-specs/` and expected it to "just work" will silently do nothing after H.1. Document this in the ADR-037 supersession note as a deliberate behavior change.

**OQ-H3 — cross-pollination visibility in the harness.** The canonical cross-pollination test requires Consumer-1's lens to fire on Consumer-2's ingest. Confirm that the `changes_since` event cursor (or `find_nodes` with `relationship_prefix`) reveals lens-created edges from the OTHER consumer's adapter. If the enrichment loop isn't persisting correctly to the event log after WP-C's fix (signal 46 in cycle status), this test could fail for reasons unrelated to WP-H. Include a quick check at the start of WP-H.2: `changes_since` from cursor 0 must return lens edges produced by the second ingest.

**OQ-H4 — test timing budget.** What's a reasonable timeout for each MCP tool call? Handshake is fast (<100ms expected). `load_spec` with lens sweep over empty context is fast. Ingest with content adapter is fast. `find_nodes` is fast. Tentative: 5s per call, 30s for the whole test. Revisit if flaky.

**OQ-H5 — integration_tests.rs impact.** `src/adapter/integration_tests.rs:4350` uses `register_specs_from_dir` in what appears to be an integration test path. Before deleting it, determine whether the test's intent is still covered elsewhere. If not, translate its intent to a `load_spec`-based test before removing the old one.

---

## Dependency Graph

```
WP-A through WP-G.2 ──[complete — see Completed Work Log]──┐
                                                           │
WP-H.1 (remove file-based auto-load) ─[open choice]────────┤
  │                                                        │
  │ (hard)                                                 │
  ▼                                                        │
WP-H.2 (live MCP e2e harness) ─[hard on H.1 + A–G]─────────┘
```

**Classification key:**
- **Hard dependency:** B cannot be built without A — structural necessity (the code literally won't compile or function without A's changes in place)
- **Implied logic:** simpler to build A first, but not required
- **Open choice:** genuinely independent — build any first

**Hard dependencies in this cycle (as-built):**
- WP-C hard on WP-B, WP-D hard on WP-B + WP-C, WP-F hard on WP-C — all resolved
- **WP-H.2 hard on WP-H.1** (harness needs unambiguous state-from-load_spec semantics)
- **WP-H.2 hard on WP-A through WP-G.2** (harness exercises full surface)

**Open choices remaining:**
- **WP-H.1** (scope-reductive design correction — can ship independently)

**Recommended build order for WP-H:** H.1 first (small refactor + ADR supersession), then H.2 (harness + acceptance test). No parallelism in WP-H — H.2 depends on H.1's semantic clarification.

**As-shipped order for WP-A through WP-G.2:** A → B → C → WP-C UUID fix → D → pre-WP-F MCP ingest fix → E → F → F no-context test follow-up → G.1 → G.2. See Completed Work Log for commits.

---

## Transition States

Each transition state represents a coherent intermediate architecture where the system is functional, tests pass, and the build can be paused without leaving the codebase in a broken state.

### TS-1: Conformance debt cleared (after WP-A)

The silent enrichment/lens drops in `register_specs_from_dir` are fixed. Any existing deployment using file-based spec auto-discovery now wires enrichments and lens correctly. No new surface area, no new capability — corrective only. But shippable on its own.

**Capabilities:** Everything that worked before, but file-based specs now actually work as documented.

### TS-2: Infrastructure in place, unused (after WP-A + WP-B)

The `specs` table exists (empty for new installations). `IngestPipeline` supports runtime registration via interior mutability. `PipelineBuilder::with_persisted_specs` exists but is not called by any host. No behavioral change — the machinery is wired but no one uses it.

**Capabilities:** Same as TS-1. Internal-only additions.

**What a cautious reviewer can verify at TS-2:** that the interior mutability is scoped correctly (fitness criterion), that the new storage methods have default no-op implementations for non-SQLite backends, that no code path in the existing system accidentally uses the new registration methods yet.

### TS-3: Rust-embedded load_spec working (after WP-A + WP-B + WP-C)

Embedded Rust consumers can call `PlexusApi::load_spec` and `unload_spec` at runtime. The three-effect model works: spec persists, lens runs, vocabulary layer builds up. MCP consumers still cannot declare identity at interaction time — they depend on file-based auto-discovery as before.

**Capabilities:** Full embedded consumer lifecycle. The end-to-end workflow from product discovery is achievable in a Rust test or an embedded Trellis integration, but not yet through MCP.

### TS-4: Multi-session persistence working (after WP-A + WP-B + WP-C + WP-D)

The critical transition: persisted specs survive library reconstruction. On Day 1, Trellis loads its spec via `load_spec`. On Day 2, Carrel constructs a fresh library instance against the same SQLite database — Carrel's `PipelineBuilder::with_persisted_specs` reads the specs table and registers Trellis's lens on Carrel's pipeline. When Carrel ingests, Trellis's lens fires, producing `lens:trellis:*` edges on Carrel's new content. This is the multi-consumer cross-pollination story working end-to-end.

**Capabilities:** Multi-session consumer workflows. The specs table is genuinely the context's lens registry. Consumer identity decouples from process identity.

### TS-5: MCP query surface live (after TS-4 + WP-E + WP-G.1 + WP-G.2)

LLM consumers can exercise the pull paradigm (`changes_since`) and composable filters (`find_nodes`, `traverse`, `find_path` with `relationship_prefix`, `contributor_ids`, `min_corroboration`) via MCP. `evidence_trail` accepts filters. `RankBy::NormalizedWeight` is available for consumers that want query-time normalized ranking. The only remaining gap is that MCP consumers still cannot declare identity at interaction time — they must rely on persisted specs from a prior embedded load.

**Capabilities:** Full MCP read surface. Pull paradigm via MCP. Composable filters via MCP. This is the point where LLM agents can start doing meaningful query work against a Plexus context they didn't set up.

### TS-6: First real MCP consumer workflow — capability-level (after TS-5 + WP-F)

The full acceptance criterion from product discovery is **achievable in principle** via the MCP transport: create context → `load_spec` via MCP → ingest via the newly-loaded adapter → query through the lens → `load_spec` a second spec for a second consumer → query across both vocabulary layers. Each constituent tool is MCP-reachable and individually verified by boundary tests.

**Capabilities:** End-to-end MCP consumer workflow is reachable. Invariant 62 holds across process boundaries. The MCP tool count reaches 16 and the query surface reaches parity with `PlexusApi` for reads.

**Unverified at TS-6:** no live MCP round-trip has exercised the full workflow in a single subprocess — boundary tests verify handler logic and JSON marshalling but not the rmcp protocol framing layer under real consumer conditions. TS-7 closes that gap.

### TS-7: End-to-end MCP workflow verified + intentional-only spec loading (after WP-H.1 + WP-H.2)

The cycle's product-discovery-defined acceptance criterion is verified by a live MCP subprocess acceptance test exercising the two-consumer vocabulary-layer cross-pollination story in a single run. Spec loading is intentional-only (`load_spec`) — file-based auto-discovery is removed. ADR-037 carries a dated supersession note documenting the behavior change.

**Capabilities:** All TS-6 capabilities, verified end-to-end under real MCP protocol framing. Spec loading semantics are unambiguous (one path, intentional). The harness is available as a foundation for real integration experiments beyond the test context. The cycle is complete.

---

## Open Decision Points

These are decisions the architect phase deliberately deferred to BUILD, or principles the build phase should honor:

- **Interior mutability mechanism for IngestPipeline adapter vector and enrichment registry** (ADR-037 §5). **Tier 1 (local BUILD decision, no ripple):** `RwLock<Vec<Arc<dyn Adapter>>>` + `RwLock<EnrichmentRegistry>` is the leading candidate — registration is infrequent, reads dominate, simple semantics. Alternative: `arc-swap::ArcSwap` if read-time copy-on-write fits the access pattern better. Both are interchangeable without touching callers. **Decide during WP-B.** **Tier 2 (NOT a local decision):** moving adapter storage to a keyed collection such as `DashMap<AdapterId, Arc<dyn Adapter>>` would restructure how adapters are indexed — changing from Vec iteration (order-preserving) to hash-keyed lookup (order-lost). That's a structural change, not a mechanism swap, and would require a separate architectural decision (likely a new ADR). **If BUILD discovers reason to want Tier 2, pause and escalate — do not silently restructure.** The fitness criterion in system-design.md makes this boundary explicit.

- **Rollback strategy for `load_spec` failures** (Invariant 60, all-or-nothing). Options: manual rollback (undo each successful step on error — simpler, more code, more chances to miss a cleanup path); transactional rollback (SQLite transaction + in-memory staging — cleaner, but requires threading a transaction through pipeline registration which currently doesn't expect one). **Decide during WP-C.** Lean toward manual rollback with exhaustive invariant tests — the steps are few and the cleanup is tractable.

- **Validation extent for `load_spec`** (Invariant 60). Minimum: YAML parses, `DeclarativeAdapter::from_yaml` succeeds, lens rules (if present) reference valid relationship type formats. Additional checks (e.g., lens rules reference existing relationship types in the graph, spec adapter ID is unique within the context) are a judgment call. **Decide during WP-C.** Start conservative — add checks when scenarios demand.

- **Rehydration error handling** (ADR-037 §2 startup path). When `PipelineBuilder::with_persisted_specs` encounters a spec row that fails to parse or extract its lens, the current design says "log and continue" (non-fatal). Is that right? Alternative: fail the builder, force the host to diagnose. **Decide during WP-B.** Current recommendation (non-fatal) preserves availability — a broken persisted spec shouldn't prevent the library from starting up. But this is worth revisiting if operators need stronger guarantees.

- **Whether `PipelineBuilder::with_persisted_specs` is per-context or multi-context.** Current design: takes `Vec<(ContextId, String)>`, so it can rehydrate multiple contexts in one call. An alternative is a per-context method (`with_persisted_specs_for_context(context_id)`). **Decide during WP-B.** Current design is more general; per-context variant can be added later as a helper if needed.

- **`RankBy::NormalizedWeight` type-system fallout** (ADR-034 Violation 1, WP-G.2). The `Box<dyn NormalizationStrategy>` variant breaks auto-derived `Clone`/`Debug`/`PartialEq` on `RankBy`. If removing those derives cascades through downstream code, WP-G.2 may need to revert and take a different approach (e.g., `Arc<dyn NormalizationStrategy>`, or a concrete `NormalizationStrategyKind` enum instead of trait objects). **Decide during WP-G.2.** If WP-G.2 reverts, the conformance debt remains open — amend ADR-034 with a supersession note per the standing principle (ADRs updated only when necessary or genuinely superseded, never casually).

- **ADR immutability principle (standing).** ADRs are authoritative records of decisions. Amend them only when a later decision genuinely supersedes them (not when "what shipped was slightly different from what the text said"). When an ADR is superseded, mark it explicitly in the ADR file. This principle was set during the ARCHITECT phase of this cycle and applies going forward.

- **Spec YAML grammar versioning (deferred, but discipline is active now).** The YAML grammar inside `spec_yaml` is currently unversioned. Until versioning is introduced, **any change to the declarative spec grammar must be forward-compatible (additive only)** — no renaming fields, no removing primitives, no restructuring sections. Breaking changes would cause existing spec rows in the specs table to fail parsing, and under the current "log and continue" rehydration error policy, consumers would silently lose all vocabulary layers. When the first breaking grammar change is proposed, pause and add: (1) `spec_version` field at the top of each YAML, (2) a migration path for old rows, (3) a **fail-loud** policy for unknown versions (not logged-and-continue — unknown version is operator-visible error, it should shout). Not in scope for this cycle; the discipline is "additive only" until versioning is added.

- **In-process spec cache vs specs table authority.** When two processes hold the library against the same context simultaneously and one process calls `load_spec` to update its spec, the specs table gets the new row (source of truth) but the other process's in-memory pipeline still has the old lens registered. The in-process pipeline is a cache that doesn't auto-refresh. Spec updates made by one process become visible to another only on that process's next restart. **Library mode assumes one-process-at-a-time workflows, so this is latent — not a bug today.** Becomes relevant when concurrent embedded consumers or Plexus-as-server arrive. Solutions at that point will involve some form of change notification (file-watcher on the SQLite file, version counter polled on each ingest, explicit `refresh()` API). Not in scope for this cycle; flagged so it doesn't get discovered under pressure.

- **Cross-cutting concern at commit boundary** (carried over from query surface cycle). Enrichment event persistence was missed because `emit_inner()` and `emit()` are separate commit paths. This cycle adds another pair of commit paths (`load_spec` write path, `unload_spec` delete path, rehydration read path). Consider pushing persistence-per-emission logic into a central place to prevent recurrence. **Deferred — revisit after the MCP cycle ships.**

---

## Completed Work Log

### Cycle: MCP Consumer Interaction Surface — Part 1 (2026-04-01 — 2026-04-13)

*Cycle in progress. WP-H (e2e harness + intentional-only spec loading) is the remaining work. Part 1 below records WP-A through WP-G.2 as shipped; WP-H will be appended to the Completed Work Log when it lands.*

**Derived from:** System Design v1.2, ADR-036, ADR-037, Invariants 60–62, Reflection 003, product-discovery.md (2026-04-02)

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-A | Fix `register_specs_from_dir` | `925d76a` | Done |
| WP-B | Specs persistence foundation + interior mutability + builder rehydration | `7a12874` | Done |
| WP-C | `load_spec` / `unload_spec` on PlexusApi | `22838b5`, `fbe7fb7` (UUID fix) | Done |
| WP-D | Startup spec re-instantiation via host + builder | `6661d2c` | Done |
| pre-WP-F | `api.ingest` accepts context name, not UUID | `0b9d9d3` | Done |
| WP-E | MCP query tools (6 thin wrappers) | `38612bd` | Done |
| WP-F | MCP `load_spec` tool | `11d686c`, `8be7722` (no-context test) | Done |
| WP-G.1 | `evidence_trail` + QueryFilter | `98343bb` | Done |
| WP-G.2 | `RankBy::NormalizedWeight` variant | `22e24a2` | Done |

**Summary:**
- Three-effect model for `load_spec` (durable graph data + durable enrichment registration + transient adapter wiring) working end-to-end
- `PipelineBuilder::with_persisted_specs` rehydrates persisted lens enrichments at library construction time — vocabulary layers are a property of the **context**, not the **consumer process**
- MCP transport grew from 9 to 16 tools: 1 session, 1 ingest, 6 context, 7 graph read, 1 spec load
- `evidence_trail` now composable with `QueryFilter` — Invariant 59 holds for every query primitive
- `RankBy::NormalizedWeight` variant available at the Rust API level (not exposed via MCP in this cycle)
- Standing principle established mid-cycle: **ADRs are immutable unless genuinely superseded** — update them when necessary, never casually to match what shipped
- Final state: 426 lib tests + 67 acceptance tests + 1 doc test (494 total)

**Dependency graph (as-built):** A, B, E, G.1, G.2 open-choice starting points. C hard on B. D hard on B+C. F hard on C. Pre-WP-F bug fix blocked F's e2e acceptance criterion. As-shipped order: A → B → C → WP-C fix → D → pre-WP-F fix → E → F → F follow-up test → G.1 → G.2.

**Conformance debt carried forward:** None. All three items from the Query Surface cycle (register_specs_from_dir, evidence_trail QueryFilter, RankBy::NormalizedWeight) closed in WP-A, WP-G.1, WP-G.2 respectively.

**Key decisions surfaced during BUILD:**
- Interior mutability Tier 1 (`RwLock<Vec<Arc<dyn Adapter>>>` + `RwLock<EnrichmentRegistry>`) sufficient; Tier 2 restructuring never needed
- Manual rollback on `load_spec` failure; validation (YAML parse + `DeclarativeAdapter::from_yaml` + lens extraction) gates the state-mutating steps
- `PersistedSpec` as a named struct (not tuple) — enables non-breaking schema evolution
- Non-fatal "log and continue" for malformed persisted specs on startup
- Specs table keyed by stable `ContextId` UUID, not user-facing name — rename-safe
- `register_specs_from_dir` (file-based) and `load_spec` (API-based) coexist; idempotency protects edge output but the enrichment loop may fire twice per event — deferred de-duplication to a later cycle
- MCP wire format decoupled from API types (inline `serde_json::json!` for `SpecLoadResult` instead of deriving `Serialize`) — transport owns the JSON shape independently of in-process Rust types
- `RankBy` lost `Clone/Copy/PartialEq/Eq` derives (zero usage in codebase); `Box<dyn NormalizationStrategy>` over Arc (no Clone needed); manual `Debug` impl avoids cascading `+ Debug` bound into the trait
- `NormalizedWeight` deliberately NOT exposed via MCP — ADR-036 §1 specified only "raw_weight" and "corroboration"; feature creep avoided

**Deferred concerns logged for later cycles:**
- File-based + persisted spec interaction: enrichment loop may fire twice if both sources register the same `adapter_id`
- Spec YAML grammar versioning: additive-only until `spec_version` field is introduced; breaking change requires pause-and-escalate
- Concurrent-process spec cache staleness: latent in library mode (one-process-at-a-time assumption); surfaces with concurrent embedded consumers or server mode
- Cross-cutting enrichment event persistence concern (from query surface cycle): `load_spec` and `unload_spec` added another pair of commit paths; consolidating persistence-per-emission to a central place remains future work

---

### Cycle: Query Surface (2026-03-26 — 2026-04-01)

**Derived from:** System Design v1.1, ADR-033, ADR-034, ADR-035, Essays 001–002

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-A | Event Cursor Persistence | `7222991` | Done |
| WP-B | Lens Declaration and Translation | `8333d10` | Done |
| WP-C | Composable Query Filters | `8b3230b`, `4cb566e` | Done |

**Summary:**
- WP-A: Event persistence in SQLite, `changes_since()` API, cursor types, 7 acceptance tests
- WP-B: LensSpec/TranslationRule types, LensEnrichment (many-to-one, idempotent), YAML deserialization, 9 acceptance tests
- WP-C: QueryFilter (contributor_ids, relationship_prefix, min_corroboration), RankBy enum (RawWeight + Corroboration only — NormalizedWeight variant deferred to MCP cycle WP-G.2), filter on all query structs, 12 acceptance tests (9 scenario + 3 cross-WP integration)
- Bug fix: enrichment loop events now persist to event log (pre-existing gap)
- Final state: 403 lib tests + 58 acceptance tests (461 total)

**Dependency graph (as-built):** A → B → C, all implied-logic dependencies. Could have been built in other orders.

**Conformance debt carried forward:** (1) `RankBy::NormalizedWeight` variant prescribed in ADR-034 but never implemented — slated for MCP cycle WP-G.2. (2) `evidence_trail` missing QueryFilter parameter — slated for MCP cycle WP-G.1. (3) `register_specs_from_dir` silently dropping enrichments and lens — slated for MCP cycle WP-A.

---

### Cycle: Operationalization (2026-03-17 — 2026-03-20)

**Derived from:** ADR-029, Essay 26, operationalization design spec

**Track A — Structural Module System (RDD)**

| WP | Title | Status |
|----|-------|--------|
| WP-A1 | StructuralModule trait + StructuralOutput types | Done |
| WP-A2 | ExtractionCoordinator refactor (fan-out dispatch, module registry) | Done |
| WP-A3 | MarkdownStructureModule (pulldown-cmark, heading/link extraction) | Done |
| WP-A4 | PipelineBuilder wiring (with_structural_module, with_default_structural_modules) | Done |

**Track B — Operationalization**

| WP | Title | Commit | Status |
|----|-------|--------|--------|
| WP-B1 | .llm-orc cleanup | `e29c081` | Done |
| WP-B2 | Tier 1 acceptance tests | `4d82b59`, `83176ad`, `6712562`, `a012c5b` | Done |
| WP-B3 | Research graduation | `b917ae6`, `1041ef7` | Done |
| WP-B4 | Tier 2 acceptance tests | `bf018cf` | Done |

**Summary:**
- Track A delivered the structural module system (StructuralModule trait, ExtractionCoordinator fan-out, MarkdownStructureModule, PipelineBuilder wiring)
- Track B delivered operationalization (llm-orc cleanup, acceptance tests Tier 1+2, research graduation)
- Final state: 382 lib tests + 30 acceptance tests

---

### Cycle: Architectural Consolidation (2026-03-16 — 2026-03-17)

**Derived from:** ADR-029, Essay 26

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-1 | Decompose adapter/ into submodules | `b94bbc0`, `c466f33`, `ef261c8` | Done |
| WP-2 | Extract pipeline builder from MCP | `37d789e` | Done |
| WP-3 | Remove enrichment from EngineSink (ADR-029 D2) | `19214ba` | Done |
| WP-4 | Remaining ADR-029 cleanup | (prior session commits) | Done |
| WP-5 | Note open questions in domain model | (prior session commits) | Done |

**Summary:**
- ADR-029 fully implemented. EngineSink is purely commit+persist. PipelineBuilder owns construction. MCP is a thin shell.
- TagConceptBridger removed entirely.
- Final state: 364 lib tests, clippy clean, all conformance drift addressed.
