# Roadmap: Plexus

**Last updated:** 2026-04-24 (BUILD WP-A/B/C/D/E all complete — Default-install experience and lens design principles cycle)
**Derived from:** System Design v1.3, ADRs 038–042, conformance scan 038-042 (7 debt items, all Bug), DECIDE gate reflection (2026-04-21), WP-D stewardship (2026-04-23), WP-E close (2026-04-24)

## Current State

**Active cycle:** Default-install experience and lens design principles — BUILD complete (2026-04-24). Five ADRs landed in DECIDE (all Accepted as of gate close 2026-04-21): ADR-038 (release-binary feature profile), ADR-039 (`created_at` property contract), ADR-040 (DiscoveryGap trigger sources), ADR-041 (lens grammar conventions — structural predicates for discovery-oriented jobs, convention not requirement), ADR-042 (dimension extensibility guidance — documentation + syntactic validation). Light-touch ARCHITECT pass updated system-design.md to v1.3. WP-A/B/C/D/E all landed. PLAY pending; user has elected to do PLAY as practitioner (partial-fidelity inhabitation; non-builder PLAY remains a future-cycle strengthening action).

**No new modules, no new dependency edges.** All BUILD work is code-level (property-contract producer/consumer alignment; resolve_dimension refactor; docstring drift cleanup) plus onboarding documentation (README lean-baseline framing, worked-example spec, spec-author dimension-choice guidance, lens grammar convention).

**Central commitment — validated:** the ADR-038 reframing ("positive decision, not defect-by-omission") holds. WP-D delivered substantive documentation: worked-example spec with empirical tautology-threshold crossing, honest README (lean baseline + capability-loss transparency + two activation paths), spec-author guide covering dimension/lens-grammar/minimum-useful-spec, and shipped-adapter convention notes inline with adapter source.

## Work Packages

### WP-A: `created_at` property contract — coordinated four-site `fix:` (ADR-039) ✅ Complete (`f82bd76`, 2026-04-22)

**Objective:** Close conformance debt D-01 through D-04 as a single coherent fix. The bug is a full producer/consumer mismatch: three built-in adapters write `created_at` into `NodeMetadata` while `TemporalProximityEnrichment` reads `node.properties["created_at"]` and parses it as `u64` epoch milliseconds. After this WP, `TemporalProximityEnrichment` actually fires on adapter-created nodes with timestamps within the threshold — the silent-dead enrichment PLAY surfaced becomes live in the default build.

**Changes:**
- `src/adapter/adapters/content.rs` (fragment construction path) — write `PropertyValue::String(chrono::Utc::now().to_rfc3339())` to `node.properties["created_at"]` at fragment node construction; also at concept-node construction from fragment tags (via `concept_node()` helper in `src/adapter/types.rs`, or at each call site that creates concept nodes — BUILD decides where the write lives)
- `src/adapter/adapters/extraction.rs` (`run_registration` path) — write `created_at` ISO-8601 UTC string to `node.properties` on both the file node and the `extraction-status` node
- `src/adapter/adapters/declarative.rs` (`interpret_create_node` path) — after processing `cn.properties` map, insert `"created_at"` with current UTC ISO-8601 string if not already present in the rendered properties (ADR-039 §"Spec authors using TemporalProximityEnrichment must write the declared property" — spec-authored value wins when present)
- `src/adapter/enrichments/temporal_proximity.rs` (`extract_timestamp`) — change string branch from `s.parse::<u64>().ok()` to `chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp_millis() as u64)`; graceful degradation on parse failure (node skipped, not errored)
- Tests — update existing tests that use `PropertyValue::Int` for `created_at` to use ISO-8601 strings (they currently pass via the `Int` branch and do not catch the ISO-8601 gap)
- At least one acceptance test: two nodes ingested within the threshold window both carry `created_at` written by ContentAdapter → `TemporalProximityEnrichment` emits a symmetric edge pair

**Scenarios covered:** `docs/scenarios/038-042-default-install-lens-design.md` § TemporalProximity property-contract scenarios

**Dependencies:** None. **Open choice.**

**Risk:** low. Four coordinated sites but each change is mechanical. The coupling is the risk — writing ISO-8601 at the producer side without fixing the parser changes nothing observable; fixing the parser without fixing the producers also changes nothing. **Ship as a single `fix:` commit.** Partial landing is worse than no landing.

---

### WP-B: Dimension extensibility — `resolve_dimension` + `validate_spec` (ADR-042) ✅ Complete (`2cc25ee`, 2026-04-22)

**Objective:** Close conformance debt D-06 — the highest-consequence structural violation in the cycle. `resolve_dimension` in `DeclarativeAdapter` currently rejects any dimension string not in a hardcoded allowlist (`structure`, `semantic`, `provenance`, `relational`, `temporal`, `default`) with a `process()`-time error. This blocks ADR-042's extensibility commitment: a consumer authoring a spec declaring `dimension: "gesture"` or any novel dimension cannot use Plexus without modifying Rust. After this WP, syntactic validation is the gate, not semantic policy.

**Grep-before-committing outcome (2026-04-22):** `&'static str` was *not* entrenched on the wire or in contribution keys. `Node.dimension`, `Edge.source_dimension`, `Edge.target_dimension`, and `CreateNodePrimitive.dimension` were already `String`; `Edge::new_in_dimension` / `new_cross_dimensional` / `Node::new_in_dimension` all take `impl Into<String>`. The lifetime was scoped purely to `resolve_dimension`'s return type and three local call sites — risk call-out closed green. Implementation eliminated `resolve_dimension` entirely in favor of a `validate_dimension_syntax(&str) -> Result<(), AdapterError>` helper, with call sites passing the already-`String` dimension through directly.

**Changes:**
- `src/adapter/adapters/declarative.rs` (`resolve_dimension`) — replace the exclusive `match` with a permissive well-formedness check: accept any string that is not empty, does not contain whitespace, and does not contain reserved characters (currently `:` and `\0`). Everything else passes.
- Node/edge construction call sites — update to accept owned `String` instead of `&'static str` for the dimension field. Touches `interpret_create_node`, `interpret_create_edge`, and their callers in the spec interpreter.
- `src/adapter/adapters/declarative.rs` (`validate_spec` path) — add the syntactic well-formedness check to spec validation so `load_spec` fails fast on malformed dimension values per Invariant 60. Previously, a spec with `dimension: ""` would deserialize successfully and fail only at `process()` time — the validation boundary moves forward to `load_spec`.
- Tests — scenarios for: (a) a spec declaring `dimension: "gesture"` loads successfully and creates nodes in that dimension; (b) a spec declaring `dimension: ""` fails at `load_spec` with a clear validation error; (c) a spec declaring `dimension: "lens:trellis"` fails at `load_spec` with a clear validation error (reserved `:` character).

**Scenarios covered:** `docs/scenarios/038-042-default-install-lens-design.md` § Dimension extensibility + dimension syntactic validation

**Dependencies:** None. **Open choice.** Can ship in parallel with WP-A and WP-C.

**Risk:** medium. The call-site touching (owned-string migration from `&'static str`) is the unknown. If the `&'static str` requirement is entrenched deeper than expected in the emit path — e.g., if dimension strings end up on the wire or in contribution keys where static-lifetime is assumed — this expands in scope. Recommend BUILD opens this WP by grepping all call sites of the dimension-typed field before committing to the signature change. If the grep surfaces load-bearing static-lifetime consumers, pause and escalate (likely ADR amendment rather than continued refactor).

---

### WP-C: Developer-facing documentation drift (`docs:` — D-05 + D-07) ✅ Complete (`4c028aa`, 2026-04-22)

**Objective:** Close two small documentation debts surfaced by the conformance scan. Both are single-file edits, trivial per site, no code behavior change.

**Changes:**
- `src/adapter/enrichments/discovery_gap.rs` (module doc + struct doc) — expand to include the trigger-dependency statement ADR-040 mandates: that `DiscoveryGapEnrichment` fires only when some producer emits its configured trigger relationship; that in the default Homebrew build there is no built-in producer of `similar_to`; that silent-idle-by-design is expected behavior, not a bug; that activation paths are in-process (`features = ["embeddings"]`) or consumer-declared external enrichment (ADR-038).
- `src/graph/node.rs` (dimension module doc at line 10; `dimension` field doc at line 157) — remove the incorrect "See ADR-009: Multi-Dimensional Knowledge Graph Architecture" reference (ADR-009 is "Automatic Tag-to-Concept Bridging," superseded). Replace with a generic reference or drop the ADR citation entirely. The dimension design predates the ADR-009 repurpose; the reference was never correct.

**Scenarios covered:** None — documentation-only debt.

**Dependencies:** None. **Open choice.**

**Risk:** minimal.

---

### WP-D: ADR-038 onboarding deliverables — README, worked-example spec, spec-author documentation ✅ Complete (2026-04-23, pending commit)

**Objective:** Land the documentation deliverables that make ADR-038's "positive decision, not defect-by-omission" framing substantive rather than nominal. This is the cycle's largest documentation WP — it spans README, a new worked-example spec file, onboarding material, and the spec-author documentation that ADR-042 requires. Without this WP, the ADR-038 reframing reasserts as defect-by-omission at the onboarding layer.

**Deliverables (ordered by consumer reading path):**

- **README updates.** Name the lean Homebrew/CLI baseline explicitly: two enrichments active by default (CoOccurrence always; TemporalProximity after WP-A lands the property contract); DiscoveryGap registered but idle (requires a `similar_to` producer); EmbeddingSimilarity not registered. Name the two activation paths per deployment class (in-process via `features = ["embeddings"]`; consumer-declared external enrichment via adapter spec + llm-orc ensemble). Point to the worked-example spec. **Capability-loss transparency is load-bearing alongside capability-present framing:** readers must see what functionality they do NOT get without llm-orc activation — DiscoveryGap stays idle (no latent-structural disagreement detection), EmbeddingSimilarity produces no signal (no semantic-similarity edges), any cross-domain lens translation of LLM-extracted structure is absent (because semantic extraction itself is llm-orc-dependent). Avoid apologetic framing ("unfortunately…") and avoid overclaim ("four core enrichments ship"). Direct, accurate: the lean baseline delivers CoOccurrence + TemporalProximity + graceful-idle; the llm-orc path unlocks DiscoveryGap + EmbeddingSimilarity + semantic-extraction-driven lens content.
- **Worked-example spec at `examples/specs/embedding-activation.yaml`** (or equivalent path chosen by BUILD). Declares an llm-orc ensemble that computes embeddings and emits `similar_to` edges. **Quality bar: must cross the tautology threshold** — the example must demonstrate `similar_to` edges emerging over content the author did not pre-encode with overlapping tags. Embedding-over-untagged-prose is the target shape. A pre-tagged worked example would repeat the PLAY field-notes-flagged tautology failure mode.
- **Onboarding material** (install guide, quickstart). "When to choose the in-process path" section: library consumers whose end-users cannot install llm-orc build with `features = ["embeddings"]`. Name the binary-weight and model-download cost explicitly. "How to activate embedding in the default build" section: install llm-orc, configure a provider (Ollama and OpenAI-compatible endpoints as two typical shapes, not prescribing either), author or adopt a declarative adapter spec, call `load_spec`. Reference the worked example.
- **Spec-author documentation on dimension choice** (ADR-042 §Documentation-only semantic guidance). A dedicated section describing: what dimensions are (extensible string facets declared on nodes); why the choice is load-bearing (Invariant 50 enrichment filtering; `find_nodes` dimension filter); the shipped-adapter conventions (`structure` / `semantic` / `relational` / `temporal` / `provenance` with brief intent); guidance for consumers authoring novel dimensions. **Landing criterion:** a spec author authoring a spec for a node type colliding with a shipped-adapter node type (e.g., `fragment`) must encounter the dimension-choice guidance before declaring the node's dimension — in one navigation hop from the first spec-authoring reference.
- **`create_node` primitive field-level docs** — name `dimension` as load-bearing and link to the dimension-choice section.
- **Shipped-adapter convention notes in adapter-level docs** — ContentAdapter docs name that it places fragments in `structure`; ExtractionCoordinator docs name its dimension choices. Consumers writing a spec that will coexist with a shipped adapter can check adapter-level docs directly.
- **Lens grammar convention documentation** (ADR-041 §Documented in product discovery, spec-author documentation, and interaction specs). Spec-author docs describe the structural-predicate convention for discovery-oriented jobs, backed by composition-shape reasoning, with phenomenology held as hypothesis not principle. The per-job (not per-app) framing is named. Interaction-specs updates already landed in DECIDE; spec-author docs reference them.
- **"Minimum-useful-spec" pattern** (Product Debt row routed from DECIDE) — a worked example in interaction specs that makes deliberate dimension choices and names why; contrasts with "minimum-viable-spec" (valid grammar, no useful signal). Infrastructure preconditions for useful signal are called out.

**Scenarios covered:** None — documentation deliverables. ADR-038, ADR-041, ADR-042 all rest on the documentation lever being substantive. The check at the end of BUILD is whether a reader can trace the activation path from README to worked example to ingest-and-query in one pass.

**Dependencies:** **Implied logic** on WP-A (the README's lean-baseline description is more accurate once TemporalProximity actually fires — otherwise the README must hedge TemporalProximity's status). **Implied logic** on WP-B (the spec-author documentation on dimension-choice references a fail-fast syntactic validator; if the validator isn't live, the documentation describes behavior that doesn't yet match code). Not *hard* dependencies — docs can precede code, but the narrative is cleaner when code lands first.

**Risk:** moderate — the quality bar is the risk. The worked-example spec crossing the tautology threshold is non-trivial: it needs real untagged prose, an llm-orc provider choice (or a clear "bring your own provider" framing), and reproducible output. BUILD may discover the worked example is harder than expected — if so, pause and escalate: a thin worked example is worse than no worked example for ADR-038's reframing, because it creates the *appearance* of a complete documentation lever while delivering tautological output. ADR-038 Consequences Negative names this risk explicitly.

---

### WP-E (Optional): Silent-idle debug instrumentation (ADR-042 empirical escalation signal) ✅ Complete (2026-04-24, pending commit)

**Objective:** Convert ADR-042's escalation trigger from observational-only to detectable. ADR-042 rejected option (i) (warn-on-divergence for Plexus-known node types) in favor of documentation-only guidance, with the escalation path preserved. The escalation trigger as originally stated — "if repeated PLAY or user observation surfaces the failure mode" — depends on observational signal that may not arrive given Plexus's small user base and partial-fidelity PLAY inhabitation. Debug-level instrumentation of divergence events gives a future cycle evidence to mine.

**Changes:**
- In the spec-load path (`validate_spec` or immediately after): detect when a spec's `create_node` primitive has a `node_type` matching a shipped-adapter node type (e.g., `fragment`, `file`, `extraction-status`) but a `dimension` diverging from the shipped convention. Log at debug level — not warn, not error, just observable.
- No warnings, no validation errors, no behavioral change. The spec loads successfully. This is purely observable signal.
- Test: a spec declaring `node_type: fragment, dimension: semantic` triggers the debug log; a spec declaring `node_type: gesture_phrase, dimension: harmonic` does not.

**Scenarios covered:** None — observability deliverable.

**Dependencies:** None. **Open choice.** Independent of all other WPs in this cycle.

**Risk:** minimal. Named in ADR-042 as a BUILD opportunity, not a requirement.

**Decide at BUILD entry whether to include WP-E.** The cost of including is small; the cost of excluding is that ADR-042's escalation path stays observational-only. Recommendation: include if BUILD is already touching the spec validator path for WP-B (near-zero marginal cost); defer otherwise.

**Changes:**
- `adapter/pipeline/ingest.rs` (or wherever `register_specs_from_dir` lives) — replace the `register_adapter(Arc::new(adapter))` call with the equivalent of `register_integration`, extracting enrichments and lens from each `DeclarativeAdapter` before registering
- One regression test: given a spec directory containing a spec with enrichments + lens, assert all three are registered on the pipeline

**Scenarios covered:** scenarios/037 § "register_specs_from_dir wires enrichments and lens"

**Dependencies:** None. **Open choice.**

**Risk:** minimal. The fix makes broken behavior correct; it cannot make working behavior broken.

---

## Dependency Graph

```
WP-A (created_at contract) ──[open choice]──┐
WP-B (dimension extensibility) ─[open choice]┤
WP-C (doc drift D-05 + D-07) ──[open choice]┤
WP-E (debug instrumentation) ──[open choice]┤
                                            │
                              (implied logic)│
                                            │
                                            ▼
WP-D (ADR-038 onboarding docs) ─[implied logic on WP-A + WP-B]
```

**Classification key:**
- **Hard dependency:** B cannot be built without A — structural necessity (code literally won't compile or function without A).
- **Implied logic:** simpler to build A first, but not required. B can proceed with A's changes stubbed or explicitly called out as "pending."
- **Open choice:** genuinely independent — build any first.

**No hard dependencies in this cycle.** Every WP can in principle ship independently. WP-D has implied-logic dependencies on WP-A and WP-B because the documentation describes behavior that those WPs make live — WP-D can proceed before them if the documentation explicitly notes which descriptions are pending-live. But the cleaner narrative is to land WP-A and WP-B first, then document against working code.

**Recommended build order:** WP-A → WP-C → WP-B → WP-D. Rationale:

- **WP-A first** because the `created_at` bug is the cycle's most observable defect (silent-dead TemporalProximity in every default-install build today). Shipping the fix early lets the rest of BUILD validate against a working enrichment.
- **WP-C second** because it's trivial and clears the drift noise from the codebase before the larger refactor.
- **WP-B third** because the call-site migration is the cycle's largest code-surface-area change. The grep-before-committing risk call-out (see WP-B Risk) is a pause-and-escalate point.
- **WP-D last** because the README and worked example can describe all three preceding WPs as live.

WP-E (optional) can slip into WP-B if BUILD is already touching the validator path, or ship as a separate `feat:` commit, or be deferred. Decide at BUILD entry.

---

## Transition States

Each transition state represents a coherent intermediate architecture where the system is functional, tests pass, and the build can be paused without leaving the codebase in a broken state.

### TS-1: TemporalProximity actually fires (after WP-A)

The `created_at` property contract is coherent end-to-end: ContentAdapter, ExtractionCoordinator, and DeclarativeAdapter write ISO-8601 UTC strings to `node.properties["created_at"]`; `TemporalProximityEnrichment` parses them correctly. For the first time since the enrichment was added, the default Homebrew build emits `temporal_proximity` edges on nodes with timestamps within the threshold window.

**Capabilities:** CoOccurrence + TemporalProximity active by default on tagged content. DiscoveryGap still idle (no trigger producer in the default build — by design per ADR-040). EmbeddingSimilarity still absent from the default build (by design per ADR-038).

### TS-2: Dimension extensibility live (after WP-A + WP-B)

Consumers authoring specs with novel dimensions (`"gesture"`, `"harmonic"`, `"movement-phrase"`) are accepted by `load_spec`. Malformed dimensions (empty, whitespace, reserved characters) fail at `load_spec` with clear validation errors (Invariant 60). Shipped-adapter conventions remain documented but not enforced. The extensibility promise of ADR-042 is exercisable by real consumers without modifying Plexus Rust.

**Capabilities:** TS-1 capabilities + declarative-spec dimension extensibility. ADR-042's documentation-only semantic guidance is the remaining lever; WP-D lands the documentation.

### TS-3: Documentation debt cleared (after WP-A + WP-B + WP-C)

Code-level docstrings reflect the ADR chain. `DiscoveryGapEnrichment` module/struct docs carry the trigger-source contract (ADR-040). `src/graph/node.rs` no longer cites the wrong ADR for dimension architecture. No user-facing behavior change — pure cleanup.

**Capabilities:** Same as TS-2. Internal signal-to-noise improves.

### TS-4: Default-install onboarding coherent (after WP-A + WP-B + WP-C + WP-D)

The full documentation lever lands. README describes the lean baseline honestly. The worked-example spec at `examples/specs/embedding-activation.yaml` demonstrates embedding activation over untagged prose (crosses the tautology threshold). Spec-author documentation on dimension choice is reachable in one navigation hop from the first spec-authoring reference. Lens grammar convention is documented. The "minimum-useful-spec" pattern is live in interaction specs.

**Capabilities:** ADR-038's "positive decision, not defect-by-omission" reframing is fully grounded — a new consumer installing via Homebrew can read README, follow the activation path, and produce `similar_to` edges over their own content without reading Plexus source. The cycle's documentation commitments land in substance, not just declaration.

**What TS-4 does not deliver:** a validated observation that documentation-only guidance actually prevents the PLAY Finding 3 failure mode (spec author silently picks a diverging dimension for a colliding node type). That validation is empirical and requires real-author usage — see Open Decision Points.

---

## Open Decision Points

These are decisions or open questions carried into BUILD from DECIDE's gate reflection and the susceptibility snapshot at the DECIDE → ARCHITECT boundary.

**This cycle:**

- **Worked-example provider choice (WP-D).** Settled at gate: **Ollama via llm-orc** is the provider the practitioner will use for empirical validation of the worked-example spec. Onboarding prose still names OpenAI-compatible endpoints as the other common shape (llm-orc handles provider routing; Plexus is provider-indifferent), but the tautology-threshold verification is done against the Ollama path.

- **Embedding strategy within llm-orc (WP-D, BUILD exploration).** ADR-038 decided that embedding is reached via a consumer-declared external enrichment running through llm-orc. It did NOT decide which embedding model, which parameters, which threshold, or which ensemble shape to use. These are BUILD-phase choices to be explored empirically against the tautology-threshold bar. Candidate model families (nomic-embed-text, mxbai-embed-large, others available via Ollama) and parameter shapes (similarity threshold, batch size, output relationship name) are part of the worked-example spec's design. **Treat as exploratory:** BUILD may need to try multiple model/parameter combinations before finding one that crosses the tautology threshold on realistic untagged prose. Document the final choice and the rejected alternatives in the spec's inline comments or a companion note so readers understand the choice space, not just the landing point.

- **Worked-example tautology threshold (WP-D).** The worked example must demonstrate `similar_to` edges emerging over content the author did not pre-encode with overlapping tags. Embedding-over-untagged-prose is the shape. BUILD should verify this empirically before committing the example — run the spec against untagged prose, observe the emitted edges, confirm they reflect semantic similarity rather than mechanical tag coincidence. A pre-tagged example is a failure, not a deliverable. **Recursive-tautology awareness:** the challenge is not just "does this specific example cross a threshold" but "does the approach rise above what we're already experiencing to get away from tautology" — which includes not carrying pre-encoded practitioner assumptions about what emergent structure should look like into the worked example's prose selection or the ensemble's parameter shape. Our demonstration of how to escape tautology can itself be tautological. **Escalation path if the worked example cannot rise above tautology: escalate to the practitioner (user). The response shape may be a new research cycle on emergent-structure demonstration, not a WP-D patch.** Shipping a thin example is worse than shipping no example because it creates the *appearance* of a complete documentation lever while delivering tautological output — the exact failure mode ADR-038 Consequences Negative names.

- **Onboarding tone (WP-D).** ADR-038 reframed the lean default as honest-to-demo. README language should carry that stance without apology and without overclaim. BUILD should flag any onboarding text that reads as either: (a) apologetic for the lean baseline ("unfortunately, embedding isn't wired by default — you'll need to..."), or (b) overclaiming on the default binary ("four core enrichments ship out of the box"). Both fail the honest-to-demo test. Correct tone: "The default Homebrew binary ships with two active enrichments... to activate embedding-based discovery, authoring a declarative adapter spec..." — direct, accurate, no apologetic framing.

- **Silent-idle debug instrumentation (WP-E — optional).** Named in ADR-042 as an empirical escalation opportunity for dimension-choice divergence. Convert ADR-042's escalation trigger from observational to detectable. **Include WP-E if BUILD is already touching the validator path (near-zero marginal cost); defer otherwise.** If deferred, ADR-042's escalation path stays observational-only — not a blocker, but named here so the choice is visible.

**Open questions carried from DECIDE gate:**

- **Empirical escalation signal for ADR-042.** Whether BUILD instruments silent-idle detection is a BUILD-phase choice (see WP-E above). Not a blocking architectural decision.

- **Phenomenology-of-discovery hypothesis (ADR-041) — split treatment.** ADR-041's convention rests on two arguments doing different work (per the ADR's own "Which argument carries which part" paragraph): composition-shape reasoning (analytical, load-bearing for the extensibility preference) and phenomenology-of-discovery (hypothesis-level, load-bearing for the per-job split). The validation opportunity splits with them:
  - **Composition-shape — partially validatable in-cycle IF WP-D crosses tautology.** Once the worked example produces emergent `similar_to` edges over untagged prose, BUILD can run both grammar conventions (`lens:trellis:thematic_connection` vs `lens:trellis:latent_pair`) over the same emergent content and observe whether the analytical walk-through's claims about interpretive-location, query-shape, and composition-shape-on-extension actually obtain. This is an in-cycle observation, not a blocker — if the analytical claims hold up empirically, the convention's extensibility-preference argument is grounded; if they don't, ADR-041's composition-shape reasoning needs revision. **Add to BUILD stewardship at WP-D close if the worked example crosses tautology; skip silently if it does not (no emergent content → no test possible).**
  - **Phenomenology — remains future-cycle.** The "noticing yourself vs being told" experiential difference needs a non-builder stakeholder observing emergent content; partial-fidelity inhabitation (builder inhabiting their own design) recurs if the practitioner self-tests. No second PLAY session this cycle; phenomenology validation carries forward as the legitimate future-cycle concern.
  - If BUILD's composition-shape observation surfaces surprises — claims holding more strongly than expected, or failing against real content — record in a reflection and route to the next cycle's opening.

- **ADR-038 reframing is contingent on WP-D's deliverables.** Cycle-status §Feed-Forward names this explicitly: "positive decision, not defect-by-omission" holds only if BUILD lands the README updates, worked-example spec, and onboarding material with substance. Weak or delayed deliverables reassert the defect-by-omission framing. **BUILD stewardship checkpoint at WP-D close:** re-read ADR-038's Consequences Negative against what shipped. If the documentation lever is thin, the ADR's core claim becomes false — the correct response is either strengthen WP-D or amend ADR-038 with a supersession note acknowledging the reframing did not land.

- **Second PLAY session (optional).** A non-builder stakeholder inhabiting the Consumer Application Developer role after BUILD lands would materially strengthen the validation of ADR-038's reframing and ADR-042's documentation lever. **Not happening this cycle** (user is the sole tester at present; methodologically valuable but not realistic in current resourcing — cycle-status §Phase Status). Carried forward as a recommended strengthening action for a follow-up cycle.

**Standing principles (carry forward from prior cycles):**

- **ADR immutability principle.** ADRs are authoritative records of decisions. Amend them only when a later decision genuinely supersedes them, not when "what shipped was slightly different from what the text said." When an ADR is superseded, mark it explicitly in the ADR file. Set during the MCP cycle's ARCHITECT phase; applies going forward.

- **Spec YAML grammar versioning (discipline is active now).** The YAML grammar inside `spec_yaml` is currently unversioned. Until versioning is introduced, any change to the declarative spec grammar must be forward-compatible (additive only) — no renaming fields, no removing primitives, no restructuring sections. Breaking changes would cause existing spec rows in the specs table to fail parsing. When the first breaking grammar change is proposed, pause and add: (1) `spec_version` field at top of YAML, (2) a migration path for old rows, (3) a **fail-loud** policy for unknown versions. Not in scope for this cycle; discipline is "additive only."

- **In-process spec cache vs specs table authority.** When two processes hold the library against the same context and one calls `load_spec`, the other process's in-memory pipeline still has the old lens registered until it restarts. Library mode assumes one-process-at-a-time; latent until concurrent embedded consumers or server mode arrive. Not in scope this cycle.

- **Cross-cutting concern at commit boundary** (carried over from query surface cycle). `load_spec`, `unload_spec`, and rehydration all represent commit paths where persistence-per-emission-style logic could be centralized. **Deferred — revisit after ADRs accumulate enough commit paths to justify the abstraction.**

---

## Completed Work Log

### Cycle: Default-Install Experience and Lens Design Principles (2026-04-17 — 2026-04-23, BUILD substantively complete)

**Derived from:** System Design v1.3, ADRs 038–042, conformance scan 038-042 (7 debt items), BUILD stewardship (WP-A–WP-D).

| WP | Title | Commits | Status |
|----|-------|---------|--------|
| WP-A | `created_at` property contract (ADR-039) — coordinated four-site `fix:` | `f82bd76` | Done |
| WP-C | Developer-facing documentation drift (D-05 + D-07) | `4c028aa` | Done |
| WP-B | Dimension extensibility — `validate_dimension_syntax` + `validate_spec` (ADR-042) | `2cc25ee` | Done |
| WP-D | ADR-038 onboarding deliverables — README, worked example, spec-author guide | `67ed2c7`+`58cda46`+`d880159` | Done |
| WP-E | Silent-idle debug instrumentation (ADR-042 empirical escalation) | `56781fb` | Done |

**WP-D summary (2026-04-23):**
- Worked-example spec at `examples/specs/embedding-activation.yaml` (declarative adapter + llm-orc ensemble) with inline activation flow, dimension-coexistence commentary, and lens-grammar hooks.
- llm-orc ensemble config + Python script at `.llm-orc/ensembles/embedding-similarity.yaml` + `.llm-orc/scripts/embedding/embedding-similarity.py` — ~140-line stdlib-only script, no LLM agents, nomic-embed-text over Ollama HTTP embeddings API, truncation on context-exceed + graceful degradation.
- Two reproducible fixture corpora: `test-corpora/collective-intelligence/` (8 arXiv abstracts spanning biology, human crowd, robotics, algorithmic, theoretical subfields) and `test-corpora/public-domain-stories/` (6 pre-1928 short stories by distinct authors with diverse motifs). Transparent CURATION README per corpus with selection criterion, observed embedding structure, and falsifiability invitation (swap fixture / swap model / add third corpus).
- README rewrite: names lean baseline (2 active by default; DiscoveryGap idle; EmbeddingSimilarity absent), capability-loss transparency, two activation paths (in-process via `features = ["embeddings"]`; consumer-declared via adapter spec + llm-orc), library + MCP usage, 17-tool MCP inventory. Tone direct, not apologetic, not overclaiming.
- Spec-author guide at `docs/references/spec-author-guide.md`: when-to-use decision, spec anatomy, dimension-choice (with shipped-adapter conventions table), lens grammar conventions (composition-shape load-bearing + phenomenology held as hypothesis + per-job framing), ensemble integration, minimum-useful-spec pattern cross-referenced to interaction-specs.
- Inline module docs: `CreateNodePrimitive` field-level docs naming `dimension` as load-bearing with pointer to spec-author-guide; `ContentAdapter` + `ExtractionCoordinator` module docs carry shipped-adapter convention tables inline for grep-discoverability.
- T12 acceptance test in `tests/acceptance/mcp_matrix_llm_orc.rs` — gated behind `PLEXUS_INTEGRATION=1`, loads worked example via MCP `load_spec`, ingests a four-doc batch through real llm-orc subprocess + Ollama, verifies `similar_to` edges emerge. 2.27s run; pins worked-example regression.
- `McpHarness::spawn_with_env` helper in `tests/acceptance/mcp_harness.rs` — non-breaking addition allowing per-test env-var configuration of spawned subprocess (used by T12 to calibrate `SIMILARITY_MIN` for short inline fixtures).

**Tautology threshold crossed empirically** — within-corpus similarity lands 0.72-0.90 (abstract pair `swarms-phase-transitions ↔ idle-ants-role` at 0.855; story pair `desirees-baby ↔ gift-of-the-magi` at 0.897). Cross-corpus similarity all falls below 0.72 — clean separation between narrative-prose register and scientific-abstract register. Legible within-abstract sub-clusters: biological (`honeybee ↔ idle-ants` 0.816), human-crowd (`mosh-pit ↔ autonomous-vehicles` 0.805), theoretical (`swarms-phase-transitions` is the hub). Within-story cluster is uniform (0.83-0.90) — an observation of `nomic-embed-text`'s register-over-content bias on narrative prose rather than a demo failure.

**Final test state:** 444 lib + 86 acceptance + 1 doc = 531 default-run; PLEXUS_INTEGRATION=1 adds T6/T7/T8/T11/T12 real-Ollama gated tests.

**Deferred / not done this cycle:**
- WP-E silent-idle debug instrumentation — optional; near-zero marginal cost if ever added next to `validate_dimension_syntax`.
- Composition-shape two-lens walkthrough over actual emergent graph — precondition (tautology crossed) was met, but the concrete comparison of `lens:trellis:thematic_connection` vs `lens:trellis:latent_pair` over WP-D's emergent `similar_to` edges was not done. The analytical comparison exists in ADR-041 against an illustrative case; repeating it against real emergent content would produce concrete evidence for composition-shape claims. Deferred to either WP-E scope or a follow-up cycle.
- Second PLAY by a non-builder stakeholder — methodologically valuable but declined for this cycle (user is sole tester).

**Standing principle established this cycle:** ADR-038's "lean baseline is a positive decision" reframing is contingent on sustained documentation quality. Future architectural changes that degrade the worked example, the spec-author guide, or the honest-to-demo README framing would re-surface the defect-by-omission framing; treat documentation-layer decay as first-class structural debt.

---

### Cycle: MCP Consumer Interaction Surface (2026-04-01 — 2026-04-16) ✅

Cycle complete. Records WP-A through WP-H.2 plus post-WP hardening (llm-orc wiring, phase nomenclature rename, outbound-event symmetry, extended test matrix).

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
| WP-H.1 | Remove file-based spec auto-discovery + ADR-037 §4 supersession | `81ce6ef` | Done |
| WP-H.2 | Live MCP e2e subprocess acceptance harness | `ce7dda5` | Done |
| Post-H Finding A | ExtractionCoordinator multi-context refactor | `f15807d` | Done |
| Post-H Finding A+B | `with_llm_client` builder method (wires semantic extraction; propagates client to `load_spec`) | `cede6c4` | Done |
| Post-H rename | Extraction-phase nomenclature → descriptive names | `0d042ac` | Done |
| Test matrix | McpHarness extraction + T1/T2/T3/T5 + unload_spec MCP tool + T4 + T6/T7/T8 gated | `532f6ba`, `6a0addb`, `6951d1a`, `f810808` | Done |
| Outbound events | DeclarativeAdapter + ExtractionCoordinator `transform_events` override | `3f04363` | Done |
| Test matrix extended | T9 (N-consumer), T10 (consumer cycling), T11 (confirmed background-phase lens gap) | `2557206` | Done |

**Summary:**
- Three-effect model for `load_spec` (durable graph data + durable enrichment registration + transient adapter wiring) working end-to-end
- `PipelineBuilder::with_persisted_specs` rehydrates persisted lens enrichments at library construction time — vocabulary layers are a property of the **context**, not the **consumer process**
- `PipelineBuilder::with_llm_client` wires SemanticAdapter AND propagates the client to declarative adapters with `ensemble:` fields via `load_spec`
- MCP transport grew from 9 to 17 tools: 1 session, 1 ingest, 6 context, 7 graph read, 2 spec lifecycle (`load_spec` + `unload_spec`)
- `evidence_trail` now composable with `QueryFilter` — Invariant 59 holds for every query primitive
- `RankBy::NormalizedWeight` variant available at the Rust API level (not exposed via MCP in this cycle)
- Consumer-facing outbound events: DeclarativeAdapter + ExtractionCoordinator emit `{type}_created` / `file_registered` / `concept_created` events; MCP consumers see meaningful counts from ingest
- File-based spec auto-discovery removed (intentional-only loading per Invariant 61)
- Extraction-phase nomenclature standardized: registration / structural_analysis / semantic_extraction
- Standing principle established mid-cycle: **ADRs are immutable unless genuinely superseded** — update them when necessary, never casually to match what shipped
- **Confirmed architectural gap (T11):** lenses do not fire on background-phase emissions; consumers using `extract-file` route will not see lens translation over LLM-extracted structure. Use declarative adapter with `ensemble:` field (foreground path) for that case.
- Final state: 425 lib tests + 82 acceptance tests + 1 doc test = 508 default-run; 511 with `PLEXUS_INTEGRATION=1` (T6/T7/T8/T11 real-Ollama)

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
