# ADR-030: Structural Module Trait

**Status:** Proposed

**Research:** [Essay 18](../archive/essays/18-phased-extraction-architecture.md), [Essay 25](../archive/essays/25-closing-the-extraction-gap.md)

**Domain model:** [domain-model.md](../domain-model.md) — structural module, module registry, MIME dispatch, structural output, vocabulary bootstrap, independent accumulation

**Depends on:** ADR-019 (phased extraction), ADR-012 (unified ingest pipeline)

**Invariants:** 45, 51, 52, 53, 55

---

## Context

The extraction coordinator currently models structural analysis components as `Phase2Registration { mime_prefix: String, adapter: Arc<dyn Adapter> }` and dispatches via `find_phase2_adapter()` — a find-first lookup returning a single `Arc<dyn Adapter>`. This has two problems:

1. **Wrong abstraction.** Structural modules use MIME-based dispatch, not `IngestPipeline`'s `classify_input` routing (Invariant 55). Wrapping them in `Arc<dyn Adapter>` with a MIME prefix stored externally obscures the actual dispatch mechanism and forces unnecessary trait obligations (`input_kind`, `AdapterInput`).

2. **Find-first, not fan-out.** The current `find_phase2_adapter()` returns the first matching adapter. The domain model requires fan-out — all matching structural modules execute (Invariant 51), mirroring the input routing fan-out principle (Invariant 17). A markdown file should be processed by both a heading parser and a link extractor simultaneously.

## Decision

### Replace `Phase2Registration` with a `StructuralModule` trait

```rust
/// A registered component for heuristic structural analysis.
///
/// Dispatched by the extraction coordinator via MIME type affinity —
/// a different routing mechanism than IngestPipeline's input-kind routing
/// (Invariant 55). Whether this trait extends Adapter or stands alone
/// is a BUILD-time implementation decision.
#[async_trait]
pub trait StructuralModule: Send + Sync {
    /// Stable identifier for contribution tracking.
    /// Convention: `extract-analysis-{modality}-{function}`
    /// e.g., `extract-analysis-text-headings`, `extract-analysis-text-links`
    fn id(&self) -> &str;

    /// MIME type prefix this module handles (e.g., `text/`, `text/markdown`).
    fn mime_affinity(&self) -> &str;

    /// Analyze a file and produce structural output.
    ///
    /// Returns `StructuralOutput` (vocabulary + optional sections + graph emissions).
    /// An empty output is normal — not every file yields useful structure.
    async fn analyze(&self, file_path: &str, content: &str) -> StructuralOutput;
}
```

Key design choices:

- **Async `analyze()`.** The coordinator runs in tokio. Most modules (like the markdown parser) are synchronous internally, but the trait is async to avoid a breaking change when a future module needs I/O (e.g., shelling out to `tree-sitter`, fetching linked resources). Zero cost for sync modules.
- **Content passed in, not read by module.** The coordinator reads the file once and passes content to all matching modules. Modules don't do their own I/O by default, though the async trait permits it. The `content: &str` parameter works for text-based modalities; binary-file modules (audio, image) would read from `file_path` directly or require a signature change — deferred until a binary module is needed.
- **`StructuralOutput` returned, not emitted.** Modules return their output; the coordinator handles emission and merging. This keeps modules focused on analysis, not graph mechanics.
- **Routing distinction, not ontological distinction.** Structural modules use MIME-based dispatch rather than `IngestPipeline`'s `classify_input` routing. Whether `StructuralModule` extends `Adapter` or is its own trait is resolved during BUILD — what matters is the MIME dispatch mechanism and `StructuralOutput` return type.

### Replace `Vec<Phase2Registration>` with a module registry

The extraction coordinator holds `modules: Vec<Arc<dyn StructuralModule>>` (replacing `phase2_adapters: Vec<Phase2Registration>`). Registration via `register_module(module: Arc<dyn StructuralModule>)` (replacing `register_phase2(mime_prefix, adapter)`).

### Fan-out dispatch replaces find-first

```rust
fn matching_modules(&self, mime_type: &str) -> Vec<&Arc<dyn StructuralModule>> {
    self.modules
        .iter()
        .filter(|m| mime_type.starts_with(m.mime_affinity()))
        .collect()
}
```

All matching modules execute. Empty result means structural analysis is a passthrough (Invariant 52).

## Consequences

**Positive:**

- Structural modules have a focused contract — `analyze(path, content) → StructuralOutput` — with MIME-based dispatch instead of `IngestPipeline`'s input-kind routing
- Fan-out dispatch aligns with Invariant 51 and the established input routing principle (Invariant 17)
- The coordinator controls file I/O (read once, pass to all modules) — no redundant disk reads
- Empty registry gracefully passes through to semantic extraction (Invariant 52)

**Negative:**

- Breaking change to `ExtractionCoordinator`'s public API — `register_phase2()` becomes `register_module()`, `Phase2Registration` is removed
- Existing tests that register structural analysis adapters via `register_phase2(mime_prefix, adapter)` need migration to the new trait
- Whether `StructuralModule` extends `Adapter` or is its own trait is deferred to BUILD — either approach satisfies the MIME dispatch requirement

**Neutral:**

- Each structural module still needs its own adapter ID for contribution tracking (Invariant 45). The module's `id()` serves this purpose — the coordinator uses it when emitting the module's graph findings.
- The coordinator emits on behalf of each module (using the module's ID), rather than modules emitting directly. This is a consequence of modules returning `StructuralOutput` rather than having direct sink access.
