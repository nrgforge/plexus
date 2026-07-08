---
spike: Default-install feature-flag intent
cycle: Default-Install Experience and Lens Design Principles
phase: pre-DISCOVER (grounding)
date: 2026-04-17
resolves: Uncertainty 2 — is the default-install enrichment gap a defect or a staged-onboarding pattern?
---

# Spike: Default-install feature-flag intent

## Question

The PLAY session of the prior cycle surfaced that three of four default enrichments are silently inactive in the default Homebrew build. The snapshot flagged this as carrying two competing framings:

- **Defect framing:** the default build fails to deliver the advertised value proposition.
- **Staged-onboarding framing:** the default correctly offers the mechanism baseline (tagged content → co-occurrence); consumers who want more add infrastructure in a deliberate progression.

The grounding action: locate or establish the deliberate intent behind `default = []` in `Cargo.toml`. If no documented rationale exists, that itself is the answer — defect framing wins.

## Evidence gathered

### 1. Commit history of the `embeddings` feature

The `embeddings` feature was introduced in commit `8d2ec7e` (2026-02-17) — *"feat: FastEmbedEmbedder behind embeddings feature"*. The commit message states:

> Default builds are unaffected — ONNX Runtime only compiles when `--features embeddings` is specified.

This is the only documented rationale in the commit record. It justifies the feature flag's existence, but is scoped to **build weight** (the ONNX Runtime dependency is non-trivial to compile), not to release-product shape.

The follow-up commit `cb8508c` (same day) added `sqlite-vec` to the same feature, extending the scope to "ONNX-based text embeddings (fastembed) and persistent vector search (sqlite-vec)".

`default = []` has been the same expression since feature introduction; the only change over time has been the removal of an unrelated `real_llm` feature flag (`01054b3`, 2026-03-14).

### 2. ADR references

ADR-026 (Embedding as Enrichment) §Consequences Negative:

> fastembed-rs adds a dependency (ONNX Runtime) to the Plexus binary. This is a non-trivial addition to the build. All dependencies are AGPL-3.0 compatible [...]. Eigen (MPL-2.0, inside ONNX Runtime) requires attribution in NOTICE file but does not propagate copyleft to the larger work.

ADR-026 acknowledges the build-weight tradeoff but makes **no claim** about release-binary configuration. The ADR is written at the engine-architecture layer, not at the distribution layer.

No ADR (scanning ADRs 000-037) records a decision about:

- Whether the Homebrew-distributed binary should include the `embeddings` feature
- What "default-install experience" the project commits to
- Whether `default = []` is a staged-onboarding design choice or a build-weight convenience

ADR candidate 8 from the PLAY routing (cycle-status line 87) names exactly this gap: *"Homebrew/release feature activation. Whether `embeddings` belongs in `default = []` or `default = [\"embeddings\"]`."* The gap is not filled yet — it is an open question.

### 3. Release configuration

`dist-workspace.toml` configures cargo-dist to build and publish five targets (macOS aarch64/x86_64, Linux aarch64/x86_64, Windows x86_64) via `dist build` with no feature overrides. `RELEASE.md` documents the cutting routine; it passes no feature flag.

The release pipeline therefore **inherits whatever `default = []` produces**. The Homebrew binary receives the default-cargo-build shape by omission, not by positive decision.

### 4. Pipeline builder behavior

`src/adapter/pipeline/builder.rs` lines 84–112: `with_default_enrichments()` unconditionally registers three enrichments (CoOccurrence, DiscoveryGap, TemporalProximity) and conditionally registers EmbeddingSimilarity via `#[cfg(feature = "embeddings")]`. The conditional compile is graceful — it does not produce an error or warning when the feature is off.

This is consistent with the feature-flag intent as documented (make ONNX opt-in) but produces the downstream effect PLAY observed (three of four defaults silently ineffective in the default build: EmbeddingSimilarity absent; DiscoveryGap registered but with no `similar_to` emissions to react to; TemporalProximity registered but reading a `created_at` property no default adapter writes).

## Determination

**The feature-flag rationale (`default = []`) is documented; the release-distribution rationale is not.**

- Why the `embeddings` feature exists as opt-in: documented in commit `8d2ec7e` and ADR-026 — the ONNX Runtime build cost justifies opt-in scope.
- Why the Homebrew-distributed binary specifically inherits `default = []` without feature overrides: **not documented**. This is not a deliberate staged-onboarding design choice. It is an inherited default at the release-configuration boundary — a decision that has never been separately made.

The two framings are therefore not equally weighted:

- **Defect framing** (specifically: *defect-by-omission at the release-configuration boundary*) is substantively correct. No documented intent establishes that Homebrew users should experience a mechanism-baseline-only default.
- **Staged-onboarding framing** is a plausible **remediation shape**, not a plausible description of the current state. A team could *decide* to adopt staged onboarding, document it (advertised progression paths, "lite vs full" formula split, or documented upgrade instructions), and the state would then match the framing. But that decision has not been made.

The PLAY field-note framing ("documentation/default-pipeline truthfulness problem") holds, with one refinement: the truthfulness gap is at the **release configuration**, not at the feature-flag level. The feature flag is correctly scoped to build weight; the release pipeline's omission of `--features embeddings` (or equivalent) is the unchecked step.

## Independent observations surfaced while auditing

Two issues surface that are **independent of the Homebrew release decision** and should be carried into DECIDE as separate concerns:

### Observation A — DiscoveryGapEnrichment has no triggers without embeddings

`with_default_enrichments()` always registers DiscoveryGap with input relationship `similar_to`. Without embeddings (or an external enrichment that emits `similar_to`), nothing in the default pipeline produces `similar_to` edges. DiscoveryGap will never fire.

This is coupled to the embeddings decision — if `embeddings` becomes a default, DiscoveryGap gains triggers. But the coupling deserves an explicit ADR rather than implicit dependency.

### Observation B — TemporalProximityEnrichment reads a property no adapter writes

`with_default_enrichments()` registers TemporalProximity with property key `"created_at"`. It reads `node.properties["created_at"]`. The field-note observed that the `created_at` value the adapter sets goes to metadata, not properties. Without an adapter writing `node.properties["created_at"]`, TemporalProximity never fires.

This is **not coupled** to the embeddings decision. It is a property-contract bug — either the adapters need to set `node.properties["created_at"]` or the enrichment needs to read metadata. Either way, the fix is orthogonal to whether the Homebrew build includes embeddings.

## Implications for downstream phases

### For DISCOVER (update mode, immediately next)

The assumption inversion "a consumer who `load_spec`s and ingests against the default pipeline gets Plexus's value proposition" should enter product discovery:

- As an **examined failed assumption** at the release-configuration layer (not as a hypothesis — Spike 2 resolves it toward defect framing).
- With scope carefully delimited: the assumption fails at the release-distribution boundary, not at the engine architecture layer. The engine architecture (`embeddings` as opt-in feature, core enrichments gated by compile-time cfg) is defensible.

The easy-vs-honest demo tension stands and deepens — honest-to-demo includes telling new consumers what their actual default experience is, whatever the team decides that should be.

### For DECIDE (candidate ADRs)

Three ADR candidates are now differentiated rather than conflated:

- **ADR: Release-binary feature profile.** Positive decision about what the distributed Homebrew binary contains. The options include: `default = ["embeddings"]` (full default), release-profile override (`--features embeddings` in dist config only), formula split (lite + full), or documented staged-onboarding (keep default slim, advertise the progression).
- **ADR: `created_at` property contract.** Independent bug. Either adapters write `node.properties["created_at"]` or TemporalProximity reads metadata. Decide which, document, fix.
- **ADR (coupled to release-binary decision): DiscoveryGap trigger sources.** DiscoveryGap's practical activation depends on the release-binary decision for `similar_to` availability. Name the coupling explicitly.

The ADR candidate "Default pipeline truthfulness" (cycle-status line 85) can now be framed with grounded vocabulary: it is a **release-configuration truthfulness** concern, not a pipeline-architecture concern.

### For BUILD (eventually)

Any BUILD change to `with_default_enrichments()` should be paused until the release-binary ADR is decided. Rewriting the function before the decision risks undoing a future deliberate staged-onboarding choice (if the team decides to keep default slim) or implementing a partial version of the full-default choice.

Property-contract bug (Observation B) can be fixed independently of the release-binary decision.

## What remains open

This spike resolves the *intent* question. It does not resolve the *normative* question: what SHOULD the release binary contain? That belongs to DECIDE, informed by DISCOVER's stakeholder update.

A further small audit — checking whether any user-facing document (README, install instructions, docs site) explicitly advertises the default experience — would refine the "truthfulness gap" framing by showing which specific claims the current default contradicts. This is a small action and can be run alongside DISCOVER.
