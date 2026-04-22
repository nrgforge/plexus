# ADR-040: DiscoveryGap Trigger Sources

**Status:** Accepted

**Research:** Spike 2 Observation A (`docs/housekeeping/spikes/spike-default-install-intent.md`, 2026-04-17); PLAY field notes Finding 1 (`docs/essays/reflections/field-notes.md`, 2026-04-16)

**Product discovery:** Product Debt row *"DiscoveryGap silently dead because its trigger relationship has no default producer"*

**Domain model:** [domain-model.md](../domain-model.md) — DiscoveryGapEnrichment, EmbeddingSimilarityEnrichment, latent evidence, structural evidence, discovery gap, core enrichment

**Depends on:** ADR-024 (core and external enrichment architecture), ADR-026 (embedding as enrichment), ADR-038 (release-binary feature profile — determines whether `similar_to` is available in the default build)

---

## Context

`DiscoveryGapEnrichment` is a core enrichment (ADR-024) that detects structurally-disconnected but latently-similar node pairs — the delta between `similar_to` (from embedding similarity) and explicit structural evidence. The default parameterization sets its trigger relationship to `"similar_to"`. In the default Homebrew build (before ADR-038), no enrichment produces `similar_to` edges — `EmbeddingSimilarityEnrichment` is feature-gated off. DiscoveryGap is registered but has nothing to react to; it is silently dead. PLAY Finding 1 named this.

ADR-038 decided that the default Homebrew build does **not** wire a Rust-side embedding enrichment. `with_default_enrichments()` registers `EmbeddingSimilarityEnrichment` only when the `embeddings` feature is compiled in (library-consumer path). In the default build, no `similar_to` producer is built in. Consumers who want embedding activate it by declaring an external enrichment in their adapter spec (an llm-orc ensemble that emits `similar_to` edges via `ingest()`).

This leaves DiscoveryGap's activation in the default Homebrew build as a function of whether a consumer has authored a spec that produces `similar_to` — or whether any other producer is active. The underlying architectural coupling that Spike 2 Observation A surfaced persists under the ADR-038 resolution: DiscoveryGap's practical activation in any build depends on whether *some* producer of `similar_to` (or an alternative trigger) is active. Without naming the trigger-source contract explicitly, the coupling between DiscoveryGap and EmbeddingSimilarity remains an implicit dependency.

This ADR makes the coupling explicit, names the accepted trigger sources, and avoids broadening DiscoveryGap beyond its current algorithm — the enrichment stays focused on latent-structural disagreement per ADR-024; what changes is the documented surface from which its latent signal arrives.

## Decision

### DiscoveryGap's trigger relationship is parameterized; its intent is "latent evidence"

The `trigger_relationship` parameter on `DiscoveryGapEnrichment` is not merely a string — it is the **declared source of latent evidence** for this enrichment instance. The enrichment's algorithm reacts to `EdgesAdded` events with `relationship == trigger_relationship`, and for each such edge emits a `discovery_gap` edge when no explicit structural edge exists between the endpoints. The name of the trigger relationship is conventional (`"similar_to"` is the default); the semantics are "this relationship carries latent evidence the enrichment should compare against structural evidence."

### Accepted trigger sources for the default parameterization

In the default parameterization (trigger = `"similar_to"`), DiscoveryGap accepts emissions of `similar_to` from **any** of these producers, without preferring one over the others:

- **`EmbeddingSimilarityEnrichment`** as a core enrichment (ADR-026, reactive) — present only in `features = ["embeddings"]` builds. The in-process `FastEmbedEmbedder` runs reactively in the enrichment loop, producing `similar_to` edges on new nodes. This is the library-consumer path.
- **Consumer-declared external enrichment via adapter spec** (ADR-024, ADR-025, ADR-038) — a declarative adapter spec may declare an llm-orc ensemble that computes embeddings and emits `similar_to` edges via `ingest()`. This is the activation path for the default Homebrew build when a consumer wants embedding-based discovery. Per Invariant 49, `similar_to` edges re-entering through `ingest()` trigger the core enrichment loop, and DiscoveryGap fires on them exactly as it would from any other source.
- **Declarative adapter specs emitting `similar_to` directly** — a spec's `emit` section may create `similar_to` edges using the `create_edge` primitive (e.g., a consumer with an upstream similarity source of its own). DiscoveryGap reacts identically.
- **Other consumers' lenses** — if another consumer's lens translates some source relationship into `similar_to` (uncommon but permissible), DiscoveryGap reacts to the translated edges.

The enrichment is **source-agnostic**: it does not inspect which adapter or enrichment produced a `similar_to` edge. Invariant 50 applies — enrichments are structure-aware, not type-aware.

### No algorithm broadening

DiscoveryGap does **not** take on new triggers beyond the one it is parameterized with. Spike 2's option (b) — "broaden its trigger set to include structural-absence patterns that don't require `similar_to` (e.g., co-occurrence gaps)" — is rejected. The rationale:

- DiscoveryGap's algorithm is specifically "latent evidence exists between A and B, structural evidence does not." Co-occurrence-based gap detection (two concepts that share tags but have no `may_be_related` edge, say) is a different algorithm — it compares two structural signals to each other rather than comparing latent to structural. Conflating them obscures the discovery-gap concept.
- The declarative path already exists: a consumer who wants co-occurrence-based gap detection can register a second `DiscoveryGapEnrichment` instance with `trigger_relationship: "co_occurred"` in their spec's `enrichments:` section. The core enrichment is reusable under different parameterizations; this is the architectural extension point.
- Keeping the core small preserves ADR-024's framing: four pairwise/local core enrichments, each a crisp algorithm. The extension surface is open (declarative spec, external enrichment) — consumers grow capability through the declarative/ensemble path, not by expanding the Rust core.

### Multiple DiscoveryGap instances are allowed

Because `DiscoveryGapEnrichment` is parameterized (ADR-022, ADR-024), a consumer spec may register multiple instances under different parameterizations — one with `trigger_relationship: "similar_to"`, one with `trigger_relationship: "embedding:mistral:similar_to"`, one with any other declared latent-evidence relationship. Each parameterization produces a distinct `id()` per ADR-024's convention (`discovery_gap:{trigger}:{output}`), so two instances parameterized with different trigger relationships or different output relationships register as distinct enrichments. Invariant 39 (enrichment deduplication by `id()`) ensures each unique parameterization is registered once per pipeline; a second spec attempting to register the same parameterization is a no-op (the existing registration stands; no replacement, no collision error).

### Documentation: DiscoveryGap's activation is a function of trigger availability

The enrichment's docstring, the architecture narrative (README, system design), and any user-facing onboarding material must state explicitly:

> `DiscoveryGapEnrichment` detects pairs with latent evidence but no structural evidence. It fires only when some producer emits its configured trigger relationship. In the default Homebrew build, there is no built-in producer of `similar_to` — the lean baseline is CoOccurrence-only. Activation is consumer-side: a declarative adapter spec that declares an llm-orc-backed external enrichment emitting `similar_to` edges (ADR-038) activates DiscoveryGap on those edges. In `features = ["embeddings"]` builds, the in-process `EmbeddingSimilarityEnrichment` is the built-in producer. Without either path, DiscoveryGap does not fire. This is expected behavior, not a bug. Plexus remains usable; embedding-dependent discovery simply stays quiet until a producer is active.

This closes the truthfulness gap PLAY surfaced (documentation claimed four enrichments; default build delivered one active) at the DiscoveryGap-specific level.

## Consequences

**Positive:**

- The implicit coupling between DiscoveryGap and EmbeddingSimilarity is now explicit and documented. Users encountering a silent-idle DiscoveryGap have a named reason (no trigger-relationship producer is active) rather than a mysterious no-op.
- The enrichment stays source-agnostic — any producer of the configured trigger relationship activates it, including llm-orc ensembles and consumer-emitted edges. This keeps the extension surface open.
- The algorithm is preserved (ADR-024). The decision doesn't expand the Rust core; it names the surface.
- Combined with ADR-038, DiscoveryGap's default-build activation is through the consumer-spec path — the user authors (or adopts) a spec that declares an llm-orc-backed embedding enrichment, and DiscoveryGap activates on the resulting `similar_to` edges. This is consistent with Invariant 61 (consumer owns spec) and with the single documented extension mechanism for non-default-registered enrichments.

**Negative:**

- Without a consumer-authored spec that produces `similar_to` (and without the `features = ["embeddings"]` in-process path), DiscoveryGap does not fire in the default build. This ADR does not *solve* the silent-idle case for users who install Plexus without authoring a spec — it *names* it, documents it, and makes the dependency visible. The activation mechanism is documented at the onboarding path (ADR-038) via worked-example spec references. Solving it with a Plexus-side default (i.e., making a particular spec ship with the binary) is out of scope for this cycle.
- Users expecting DiscoveryGap to detect structural-absence patterns broadly (not only latent-vs-structural disagreement) will not find that here. The declarative path is available for those patterns; the core enrichment stays narrow. (See §Decision "No algorithm broadening" for the full rationale and the rejection of Spike 2's option (b).)

**Neutral:**

- ADR-024 is unchanged — DiscoveryGap remains one of four core enrichments.
- ADR-026 is unchanged — embedding is still an enrichment, and the two modes (core, external) still produce `similar_to` in the same way. This ADR documents how DiscoveryGap relates to those producers; it does not change them.
- ADR-038 supplies the trigger producer for the default build; this ADR names the accepted producer set.
- Scenarios in `026-027-embedding-and-retraction.md` continue to exercise the core path; new scenarios in this cycle exercise the graceful-idle path (no trigger → no emission) and the llm-orc re-entry path.

## Provenance

**Drivers:**
- Spike 2 Observation A (`docs/housekeeping/spikes/spike-default-install-intent.md`) — named the DiscoveryGap trigger-coupling problem as a separate ADR candidate, coupled to the release-binary decision but deserving its own explicit treatment.
- PLAY field notes Finding 1 (`docs/essays/reflections/field-notes.md`) — grounded the problem as observed behavior.
- Product Debt row *"DiscoveryGap silently dead because its trigger relationship has no default producer"* — routed the issue from DISCOVER to DECIDE with options (a)/(b)/(both).
- ADR-024's architectural framing — the rejection of option (b) (broaden the algorithm) draws directly from ADR-024's "four core enrichments, each a crisp algorithm" posture and from the "declarative/ensemble path is the extension surface" principle.

The decision framings in this ADR (source-agnostic trigger, no algorithm broadening, multiple parameterizations allowed) trace to ADR-024 and Spike 2 directly.
