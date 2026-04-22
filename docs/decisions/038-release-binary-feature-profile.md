# ADR-038: Release-Binary Feature Profile

**Status:** Accepted

**Research:** Spike 2 (`docs/housekeeping/spikes/spike-default-install-intent.md`, 2026-04-17); PLAY field notes Finding 1 and §"Misreading the enrichment surface as closed when it is open" (`docs/essays/reflections/field-notes.md`, 2026-04-16)

**Product discovery:** Value tension *"Default-lean install vs. full capability out-of-the-box"*; Assumption inversion *"The default-install experience delivers the advertised value proposition"* (validated as wrong at the release-configuration boundary); Product Debt rows on EmbeddingSimilarity shipping direction and onboarding demos.

**Domain model:** [domain-model.md](../domain-model.md) — core enrichment, external enrichment, EmbeddingSimilarityEnrichment, latent evidence, declarative adapter spec, parameterized enrichment

**Depends on:** ADR-024 (core and external enrichment architecture — the "external enrichment via llm-orc ensemble" path is the activation mechanism this ADR leans on), ADR-025 (declarative adapter spec extensions — specs declare their own enrichments), ADR-026 (embedding as enrichment — engine-architecture layer; this ADR is the release-distribution layer it was missing)

---

## Context

Spike 2 established that the feature-flag rationale for `default = []` in `Cargo.toml` *is* documented at the engine-architecture layer (commit `8d2ec7e`, ADR-026 §Consequences Negative): the ONNX Runtime build cost justifies opt-in scope for the `embeddings` feature. What was *not* documented is the release-distribution rationale — `dist-workspace.toml` and the release workflow pass no feature flags, so the Homebrew binary inherits `default = []` by omission rather than by positive decision. This is defect-by-omission at the release-configuration boundary.

The PLAY field notes surfaced the downstream effect: a consumer installing Plexus via Homebrew and reading the architecture documentation sees "four core enrichments" but gets one (CoOccurrence, the only default enrichment that works on user-supplied tags without infrastructure). EmbeddingSimilarity is feature-gated off; DiscoveryGap and TemporalProximity have dependent failures addressed by ADR-040 and ADR-039 respectively. The architectural story is true of builds with explicit feature flags or running ensemble infrastructure — it is not true of the default Homebrew build as distributed.

The PLAY field notes also surfaced the corrective — §"Misreading the enrichment surface as closed when it is open":

> Plexus's core enrichment surface is deliberately small and pairwise because the **extension surface is open and declarative**. Path 3 — external enrichments via llm-orc ensembles with Python script agents — is the intended mechanism for consumer-specific enrichment logic, including graph-science algorithms. A script agent can query graph state, run networkx / igraph / graph-tool, and emit new relationship types via `ingest()`; those relationships enter the core enrichment loop and the lens translates them like any other source.

This architectural stance (ADR-024's external enrichment path) is already the sanctioned mechanism for capabilities that require heavier infrastructure than the Rust core carries. The question this ADR resolves is not "how do we add llm-orc-backed embedding into Plexus's Rust code" but "what does the distributed binary contain, and where does the extension point live for consumers who want more."

Product discovery identified two deployment classes that shape the decision:

- **llm-orc-adjacent deployments** — developers, CLI users, server installs, any deployment where the consumer can install and configure llm-orc with an embedding provider (Ollama locally, OpenAI-compatible HTTP, or any other llm-orc-supported provider). Consumers in this class extend Plexus via declarative adapter specs that declare external enrichments running through llm-orc.
- **Embedded-library deployments** — consumers linking Plexus as a Rust library into their own application, distributed to end-users who cannot be expected to install llm-orc or configure a provider. For these consumers, in-process embedding is load-bearing; the `embeddings` feature flag is the opt-in.

A third case must be named because it is the most common failure mode: **a user installs the Homebrew binary with no llm-orc and no custom adapter spec.** Plexus must not break in this case. The binary is legitimately lean — CoOccurrence and other structural enrichments run; embedding-based enrichment simply does not fire because no producer is active, by design.

## Decision

### `default = []` stays. The Homebrew binary remains feature-lean.

`Cargo.toml`'s `default = []` is preserved. The distributed Homebrew / `cargo-dist` / shell-installer binary continues to compile without `embeddings` — no ONNX Runtime, no in-process fastembed-rs, no sqlite-vec. The release workflow (`dist-workspace.toml`, `.github/workflows/release.yml`) continues to pass no feature overrides. This is now a positive decision, not a defect-by-omission: the distributed binary is intentionally lean, and the extension path is declarative.

### No Rust code path to llm-orc for embedding

This ADR explicitly declines to introduce a Rust-side `LlmOrcEmbedder` or any equivalent type that embeds llm-orc invocation inside the Rust `EmbeddingSimilarityEnrichment`. Plexus's Rust core does not gain a second code path to llm-orc. llm-orc is reached only through the **already-sanctioned** integration point: the declarative adapter spec's external-enrichment path (ADR-024, ADR-025) — which invokes llm-orc ensembles via the existing `SemanticAdapter` and the spec's `ensemble:` mechanism.

Rationale:

- The declarative path is strictly more capable than a Rust `LlmOrcEmbedder` would be — consumers can parameterize the llm-orc ensemble (model choice, batch size, threshold, output relationship name), test it independently of Plexus Rust changes, and version it with their own spec. A Rust-side `LlmOrcEmbedder` would expose none of this directly; consumers would need to wait for Plexus Rust releases to change embedding behavior.
- Given the capability asymmetry above, a Rust `LlmOrcEmbedder` would be a second code path to llm-orc that duplicates infrastructure work the declarative path already does, for strictly less flexibility. This is the parallel-code-paths concern the DISCOVER gate conversation warned against — not as an abstract anti-pattern, but as a concrete one that would leave contributors maintaining two paths to the same dependency while consumers can only fully use one.
- ADR-024's "external enrichment" framing covers this case exactly. An llm-orc ensemble that reads graph nodes, computes embeddings via its configured provider, and emits `similar_to` edges back via `ingest()` is the intended mechanism.

### `with_default_enrichments()` registers no embedding enrichment in the default build

`PipelineBuilder::with_default_enrichments()` currently registers `EmbeddingSimilarityEnrichment` under `#[cfg(feature = "embeddings")]`. This stays. When the `embeddings` feature is off (the Homebrew default), no embedding enrichment is registered by the builder — same behavior the binary ships today at this layer.

What changes is the stance: this is no longer a silent-dead path that should be patched; it is a deliberate lean baseline with a documented consumer-side activation mechanism.

### `features = ["embeddings"]` remains first-class for library consumers

Library consumers who need in-process embedding (because their end-users cannot install llm-orc) build with `plexus = { version = "X", features = ["embeddings"] }`. In that build, `with_default_enrichments()` registers `EmbeddingSimilarityEnrichment` with `FastEmbedEmbedder`, as today. Nothing at this layer changes.

This is the load-bearing case for the in-process path. It is not deprecated, not weakened, and not a fallback — it is the right answer for consumers whose deployment shape requires Rust-embedded embedding.

### Consumer-side activation: embedding via adapter spec

Consumers using the default Homebrew binary who want embedding-based enrichment activate it by authoring a declarative adapter spec that declares an embedding-producing external enrichment. The spec describes an llm-orc ensemble that:

1. Reads new-node events (or runs on-demand).
2. Submits node labels/content to llm-orc for embedding.
3. Computes pairwise similarity above a threshold.
4. Emits `similar_to` edges back through `ingest()`.

Once emitted, those `similar_to` edges enter the core enrichment loop (Invariant 49), activating DiscoveryGap (ADR-040) and the lens's translation rules (ADR-033) as if they had come from any other source.

**Where llm-orc's provider configuration lives:** llm-orc itself — the user configures Ollama, an OpenAI-compatible endpoint, or any other supported provider through llm-orc's configuration mechanism. Plexus is indifferent to which provider llm-orc calls. The spec may name a model (e.g., `nomic-embed-text`) but does not name a provider; provider routing is llm-orc's concern.

This path is not new — it is ADR-024's external-enrichment path made operational. What this ADR adds is documentation and a worked example that consumers reach for when they want embedding in the default build.

### Plexus does not break without llm-orc or a consumer-authored spec

If the Homebrew user installs Plexus, does not install llm-orc, does not author a custom spec, and ingests content, Plexus continues to work. Specifically:

- `EmbeddingSimilarityEnrichment` is not registered (it is absent from the build, not silently failing).
- No `similar_to` edges are produced by any built-in path.
- DiscoveryGap (ADR-040) does not fire — no trigger source is active.
- CoOccurrence still fires on `tagged_with` edges.
- Structural-analysis modules still run; TemporalProximity (after ADR-039 lands) fires on nodes that carry `created_at`.
- The graph accepts writes, emits events, serves queries.

This is the honest lean baseline. Onboarding documentation names it as such.

### Documentation per deployment class

The README, install instructions, and onboarding material explicitly document:

- **Homebrew/CLI default baseline:** the binary ships lean. **Two enrichments active by default:** CoOccurrence (always, on `tagged_with` edges) and TemporalProximity (after ADR-039's property-contract fix lands). **DiscoveryGap is registered but idle** — it requires a `similar_to` producer to fire, and the default build ships none. **EmbeddingSimilarity is not registered** in the default build; it is available via the `embeddings` feature flag (in-process path) or via a consumer-authored external enrichment (llm-orc-driven path). The lean baseline is a correct choice, not a deferred feature; the activation paths are named below.
- **How to activate embedding in the default build:** install llm-orc, configure it with an embedding provider (naming Ollama and OpenAI-compatible endpoints as two typical shapes, not prescribing either), author or adopt a declarative adapter spec that declares an embedding-producing external enrichment, load it via `load_spec`. A **worked-example spec** is a named deliverable of this ADR — committed at `examples/specs/embedding-activation.yaml` (or equivalent path chosen by BUILD) and referenced from the onboarding documentation. The worked example must produce `similar_to` edges on content the author did not pre-encode with overlapping tags (i.e., the example must cross the tautology threshold — see §Consequences Negative on the worked-example quality bar).
- **When to choose the in-process path:** library consumers whose end-users cannot install llm-orc build with `features = ["embeddings"]`. Include the binary-weight and model-download cost explicitly.
- **What breaks and what does not:** the graph works without embedding; the consumer sees CoOccurrence-driven structure and whatever else their adapter produces. Adding embedding through a spec does not require rebuilding Plexus.

This closes the Spike 2 "release-distribution rationale" gap at the documentation layer. The architecture's descriptive narrative matches the binary: four core enrichments *available*; two active by default in the distributed binary (CoOccurrence always; TemporalProximity after ADR-039 lands the property-contract fix); DiscoveryGap registered but idle until a `similar_to` producer is active; EmbeddingSimilarity available under `features = ["embeddings"]` for the library-consumer path or activated via a consumer-authored adapter spec declaring an external enrichment.

## Consequences

**Positive:**

- Plexus's Rust core stays lean and narrow. No second code path to llm-orc. No duplicate embedder abstraction to maintain.
- Consumer sovereignty over embedding strategy is preserved (Invariant 61). Consumers choose the provider (via llm-orc configuration), the model, the threshold, the output relationship — all in their spec. Plexus does not force a default that may not fit the consumer's use case.
- The extension path aligns with an already-documented architectural pattern (ADR-024's external enrichment via llm-orc ensemble). No new mechanism introduced.
- The Homebrew binary stays light: no ONNX Runtime weight, no model download on first use.
- The "easy-to-demo vs honest-to-demo" tension (product discovery) resolves toward honest-to-demo at the default-install layer. Users see CoOccurrence-driven behavior on tagged content by default; the upgrade path to embedding is named and documented rather than hidden behind hidden feature flags.
- Library consumers retain the in-process path without ambiguity. `features = ["embeddings"]` is the explicit opt-in for deployment shapes that require it.

**Negative:**

- The default Homebrew build does not deliver embedding-based discovery out-of-the-box. A user who reads about four core enrichments and installs via Homebrew without authoring a spec sees **two active by default** (CoOccurrence always; TemporalProximity after ADR-039). DiscoveryGap is registered but idle — it has no `similar_to` producer in the default build and stays quiet until one is activated. EmbeddingSimilarity is not registered at all in the default build. This remains a visible gap between "what the engine supports" and "what the default binary does." The mitigation is documentation, not engineering — the onboarding path explicitly names the lean baseline as the baseline and points to the spec-declaration path for activation.
- Consumers who want embedding in the default build must do spec-authoring work (or adopt a worked-example spec). This is higher friction than a binary that just ships with embedding on. The tradeoff is accepted: consumer-owned specs is the architectural posture (Invariant 61), and consumers' specs are already the single artifact they maintain.
- BUILD must produce a worked-example spec (or reference implementation) for the documented activation path. Without a concrete example, the "declare embedding in your adapter spec" instruction is abstract. The example belongs with the documentation deliverables. **Quality bar:** the worked example must cross the tautology threshold — it must demonstrate `similar_to` edges emerging over content the author did not pre-encode with overlapping tags. A worked example built on pre-tagged hand-coordinated content would repeat the tautology failure mode PLAY field-note §"Crawl-step results and the tautology threshold" warned against: mechanism demonstrated, value not delivered. Embedding-over-untagged-prose is the shape to target.
- DiscoveryGap's silent-idle state in the default build persists until a consumer activates embedding via spec. This is now a deliberate consequence, not a defect (ADR-040 makes the trigger-dependency explicit).

**Neutral:**

- ADR-026's engine-architecture decisions (embedding as enrichment, enrichment ID encodes model identity, sqlite-vec for vector storage under the embeddings feature) are unchanged. This ADR operates at release distribution, not engine architecture.
- ADR-024's external-enrichment path is unchanged. This ADR makes it the named activation mechanism for default-build embedding.
- ADR-040 (DiscoveryGap trigger sources) is unchanged at its core; its default-build activation story becomes "trigger is produced by a consumer-authored spec using llm-orc ensemble" rather than "trigger is produced by a Plexus-wired llm-orc embedder."
- ADR-042 (spec-author dimension guidance) is independent.
- Spec validation (Invariant 60) is unaffected. `load_spec` validates specs and fires lens enrichments regardless of backend selection.
- Future ADR could reconsider if the spec-authoring-to-activate-embedding friction proves prohibitive in practice. That evidence does not exist yet.
- **Considered and rejected: bundling the worked-example spec as an install-time artifact.** An alternative to "consumer-authored spec activates embedding" is for Plexus to ship a reference spec alongside the binary — e.g., placing `embedding-activation.yaml` at a default-load path such that an installed consumer encounters a working activation path without having to author one. This was considered at the DECIDE gate (2026-04-21) and rejected on a specific reason: the bundled spec would produce no effect without llm-orc and a configured embedding provider, so shipping it would re-create the release-configuration truthfulness gap at a different layer — the user would see a "default spec" that appears to come with the binary but in fact does nothing unless separately-installed infrastructure is present. The worked-example spec at `examples/specs/embedding-activation.yaml` (or equivalent) is therefore a **documentation artifact** referenced from onboarding material, not an install-time artifact auto-loaded on the user's behalf. This preserves ADR-038's core reframing (honest-to-demo at the default-install layer) at every layer where a user might form a default-behavior expectation.

## Provenance

**Drivers:**
- Spike 2 (`docs/housekeeping/spikes/spike-default-install-intent.md`) — established that the release-distribution rationale was undocumented and the feature-flag rationale was scoped to build-weight.
- PLAY field notes §"Misreading the enrichment surface as closed when it is open" (`docs/essays/reflections/field-notes.md`) — the architectural posture *"core enrichment surface is deliberately small and pairwise because the extension surface is open and declarative"* is load-bearing for this ADR's decision to activate via spec rather than via Rust.
- PLAY field notes Finding 1 — grounded the problem as observed behavior.
- Product discovery value tension *"Default-lean install vs. full capability out-of-the-box"* — the two deployment classes (llm-orc-adjacent, embedded-library) frame the decision.
- DISCOVER gate conversation (`docs/housekeeping/gates/default-install-lens-design-discover-gate.md`, 2026-04-17) — the parallel-code-paths constraint and build-time (not runtime) configuration requirement originated here. The user correction during DECIDE (no Rust code path to llm-orc; use the already-sanctioned spec-declaration path) extended the parallel-code-paths constraint one step further: not just at runtime, but at the boundary between Rust core and external infrastructure.
- ADR-024's external-enrichment framing — the activation mechanism (llm-orc ensemble declared in spec) is ADR-024's path applied.
- Invariant 61 (consumer owns spec) — consumer-side activation via spec is the consistent resolution.

**Note on decision shape:** An earlier draft of this ADR proposed a Rust `LlmOrcEmbedder` that would wire llm-orc into `with_default_enrichments()` for the default build. That draft was corrected during DECIDE to the present shape: no Rust code path to llm-orc, consumer-declared activation via spec. The correction traces to the principle that Plexus's Rust core does not duplicate an integration path already available through the declarative adapter spec — "not having an embedding enrichment in the Rust code does not need another code path to llm-orc" (DECIDE clarification, 2026-04-20). The present ADR is the corrected shape; the rejected draft is preserved only in git history.
