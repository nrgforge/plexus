# ADR-031: Structural Output and Handoff to Semantic Extraction

**Status:** Proposed

**Research:** [Essay 25](../archive/essays/25-closing-the-extraction-gap.md)

**Domain model:** [domain-model.md](../domain-model.md) — structural output, vocabulary bootstrap, independent accumulation

**Depends on:** ADR-030 (structural module trait), ADR-019 (phased extraction), ADR-021 (semantic extraction llm-orc integration)

**Invariants:** 52, 53, 54

---

## Context

The extraction coordinator currently creates `SemanticInput::for_file(file_path)` for semantic extraction — no structural context is passed forward (extraction.rs line 522). The `SemanticInput` struct already has a `sections: Vec<SectionBoundary>` field, but it's always empty because no structural analysis output feeds into it.

Essay 25 established two key findings: (1) vocabulary bootstrapping (entity names from structural analysis) improves semantic extraction recall, and (2) relationship priming is counterproductive — it reduces recall from 81% to 38%. The handoff mechanism must carry vocabulary forward without creating sequential dependency between the phases' graph contributions (independent accumulation, Invariant 45).

Note: The research tested multiple vocabulary algorithms — TextRank/TF-IDF (Essay 24) and SpaCy NER (Essay 25) — both richer sources than Rust-native structural modules (which produce heading text and link targets). Vocabulary has two independent sources that merge: (1) Rust structural modules contribute terms via `SemanticInput.vocabulary`, and (2) llm-orc script agents (TextRank, SpaCy, etc.) can produce additional vocabulary internally before entity extraction agents run. The improvement *direction* from vocabulary priming is established by research; the *magnitude* depends on which algorithms run and how their outputs compose — an empirical BUILD-time question.

Structural output varies wildly by modality. A well-organized markdown file yields headings and links. A Shakespeare play in a `.txt` file may yield nothing. An empty structural output is the expected case for many file types (Invariant 52).

**Key design insight:** Chunking is semantic extraction's responsibility, not structural analysis's. Semantic extraction must chunk *every* file — most files have no useful structural boundaries. When structural metadata is available (e.g., heading boundaries from markdown), it can improve chunk quality — but it's never required. Structural modules produce vocabulary and structural metadata as output; how that metadata informs chunking is semantic extraction's decision. Where chunking happens — in Rust before calling llm-orc, or inside the ensemble as a script agent — is an empirical question resolved during BUILD.

## Decision

### StructuralOutput type

```rust
/// Combined output from structural analysis modules for a single file.
#[derive(Debug, Clone, Default)]
pub struct StructuralOutput {
    /// Vocabulary terms discovered by structural analysis.
    /// Entity names, key terms, link targets — passed to semantic
    /// extraction as a glossary hint (not a constraint).
    pub vocabulary: Vec<String>,

    /// Optional structural metadata that may improve semantic extraction's
    /// chunking. Section boundaries from headings, function boundaries from
    /// code parsing, page breaks from PDFs. Semantic extraction uses these
    /// as hints — it always has a default chunking strategy that works
    /// without any metadata.
    pub sections: Vec<SectionBoundary>,

    /// Graph emissions from structural analysis.
    /// Concept nodes, structural edges — emitted by the coordinator
    /// on behalf of each module with the module's adapter ID.
    pub emissions: Vec<ModuleEmission>,
}

/// Graph mutations from a single structural module.
#[derive(Debug, Clone)]
pub struct ModuleEmission {
    /// The module's adapter ID (for contribution tracking).
    pub module_id: String,
    /// Nodes to emit (concept nodes, structural metadata nodes).
    pub nodes: Vec<AnnotatedNode>,
    /// Edges to emit.
    pub edges: Vec<AnnotatedEdge>,
}
```

`StructuralOutput` reuses the existing `SectionBoundary` type directly — no new chunking hint type. `SectionBoundary { label, start_line, end_line }` is the right shape for structural metadata that happens to describe file regions. Modules that produce section-like metadata return it here; modules that don't (most modules, for most files) leave it empty. Semantic extraction treats sections as optional improvement signals for its own chunking strategy.

### Chunking is semantic extraction's problem

Semantic extraction always chunks. Every file gets chunked; structural metadata improves chunk quality when available but is never required. Concepts that appear in multiple chunks get multiple contribution slots — reinforcement strengthens the signal. Overlapping chunks are fine — deterministic concept IDs ensure upsert, not duplication.

Where chunking happens — in Rust before calling llm-orc, or inside the llm-orc ensemble as a script agent — is an empirical question resolved during BUILD. This ADR specifies only what structural context is available for the chunking decision, not where the chunking logic lives.

### Merge strategy (Invariant 53)

When multiple modules match the same file, their outputs are merged:

- **Vocabulary:** unioned (deduplicated, case-insensitive).
- **Sections:** concatenated, sorted by `start_line`. Overlapping sections from different modules are preserved — semantic extraction decides how to use them.
- **Emissions:** each module's emissions are kept separate (distinct `module_id`). The coordinator emits them individually with the correct adapter ID.

An empty merge (no modules matched, or all modules returned empty output) produces `StructuralOutput::default()` — the zero value. This is the expected case for many file types.

### Handoff to semantic extraction

The coordinator extends `SemanticInput` to carry structural context:

```rust
// Current: no structural context
SemanticInput::for_file(file_path)

// New: structural context forwarded
SemanticInput::with_structural_context(file_path, sections, vocabulary)
```

`SemanticInput` gains a `vocabulary: Vec<String>` field alongside the existing `sections: Vec<SectionBoundary>`. The vocabulary is serialized into the JSON payload that llm-orc receives, where entity-primed agents use it as a glossary hint.

### What is NOT handed off

Relationship priming is explicitly excluded (Invariant 54). Structural analysis may discover relationships (e.g., "heading A contains reference to concept B"), and those are emitted to the graph with the module's adapter ID. But the relationship structure is not forwarded to semantic extraction — only entity names. This prevents the LLM from validating structural findings instead of exploring independently (Essay 25).

## Consequences

**Positive:**

- Semantic extraction receives vocabulary hints on cold start (when the graph is empty and there are no existing concepts to draw from)
- No new type for chunking signals — reuses existing `SectionBoundary`
- Chunking works universally — every file gets chunked, structural metadata improves quality when available
- Empty structural output is a clean zero value — no special-casing needed

**Negative:**

- `SemanticInput` grows a new field (`vocabulary`), which changes the JSON payload to llm-orc. The `extract-semantic.yaml` ensemble needs to handle this field (or ignore it gracefully if absent)

**Neutral:**

- Module emissions are emitted by the coordinator, not by the modules themselves. This means the coordinator's `process()` method grows — it handles file I/O, dispatch, merge, module emission, and semantic extraction handoff. This is acceptable because the coordinator is explicitly a coordination adapter (ADR-019).
- Where chunking happens (Rust or llm-orc script agents) and what strategy is used are empirical BUILD-time decisions. This ADR specifies only what structural context is available to inform chunking, not where the logic lives.
