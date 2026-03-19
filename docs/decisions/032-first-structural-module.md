# ADR-032: First Structural Module — Markdown

**Status:** Proposed

**Research:** [Essay 18](../archive/essays/18-phased-extraction-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — structural module, module registry, vocabulary bootstrap

**Depends on:** ADR-030 (structural module trait), ADR-031 (structural output and handoff)

**Invariants:** 51, 52, 55

---

## Context

ADR-030 defines the `StructuralModule` trait. ADR-031 defines `StructuralOutput` and the handoff to semantic extraction. Neither specifies which modules ship with Plexus or how modules are registered.

The operationalization design spec calls for "first module: markdown structure parser (pulldown-cmark → heading tree, section boundaries, link extraction)" as the BUILD phase deliverable that validates the module interface end-to-end.

Two questions to decide:

1. **Does Plexus ship built-in modules or are all modules consumer-registered?** The enrichment architecture has a clear answer: core enrichments (CoOccurrence, DiscoveryGap, TemporalProximity, EmbeddingSimilarity) ship with Plexus; external enrichments are consumer-provided. Should structural modules follow the same pattern?

2. **Where is the first module registered — in PipelineBuilder or directly on ExtractionCoordinator?**

## Decision

### Plexus ships one built-in structural module: `MarkdownStructureModule`

This module has MIME affinity `text/markdown` and produces:

- **Sections** from heading hierarchy (ATX headings parsed via `pulldown-cmark`). Each heading starts a section; the section ends at the next heading of equal or higher level, or at end-of-file. Returned as `Vec<SectionBoundary>` — optional structural metadata that semantic extraction can use to improve chunking quality, but semantic extraction always has its own default chunking strategy.
- **Vocabulary** from internal link targets (`[text](url)` — extract the text as a potential entity name), heading text (stripped of formatting), and code block language identifiers.
- **Graph emissions** — what the module emits to the graph (heading-derived concepts, link-target entities, structural metadata nodes) is determined during BUILD, not prescribed here. The `ModuleEmission` type supports it; what's graph-worthy is an empirical question.

This follows the enrichment pattern: ship useful defaults, allow consumer registration for domain-specific needs. One built-in module is enough to validate the interface end-to-end. More can be added as consumer demand surfaces.

### Modules are registered on ExtractionCoordinator, wired via PipelineBuilder

`PipelineBuilder` gains a `with_structural_module(module)` method that passes modules to the `ExtractionCoordinator` during pipeline construction. This keeps `PipelineBuilder` as the single construction site (consistent with its role for adapters and enrichments) while the coordinator owns the module registry at runtime.

Plexus's default pipeline configuration registers `MarkdownStructureModule` automatically. Consumers can add additional modules via `PipelineBuilder` or skip the default by constructing the coordinator manually.

### Graph emissions determined empirically

What the markdown module should emit directly to the graph — heading-derived concepts, link targets as entities, structural metadata — is not prescribed by this ADR. The `ModuleEmission` type in `StructuralOutput` supports emissions; what's worth emitting is discovered during BUILD through experimentation. The module's primary value is sections and vocabulary for semantic extraction, but that doesn't preclude direct graph contributions if they prove useful.

## Consequences

**Positive:**

- One concrete module validates the entire pipeline: trait → dispatch → merge → handoff → semantic extraction
- Markdown is the most common input format for Trellis (the first consumer) — immediate practical value
- `pulldown-cmark` is a zero-dependency Rust crate, keeping the build fast and the module pure

**Negative:**

- Only one built-in module means most file types get no structural analysis (passthrough per Invariant 52). This is acceptable — semantic extraction handles unstructured input
- What the module emits to the graph is an open question resolved during BUILD — the ADR doesn't prescribe or constrain it

**Neutral:**

- `pulldown-cmark` is a new dependency. It's well-maintained, widely used, and small.
- The `PipelineBuilder.with_structural_module()` method mirrors the existing `with_enrichment()` pattern — consistent API surface
