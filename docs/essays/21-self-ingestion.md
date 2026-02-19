# Self-Ingestion: What Plexus Learns When It Models Itself

**Nathaniel Green**
Independent Researcher
nate@nate.green | ORCID: 0000-0003-0157-7744

*Working Essay — February 2026*

---

## The Experiment

We pointed Plexus at its own source code. The question was simple: does the knowledge graph engine produce useful intelligence about a codebase, or does it just rearrange syntax?

The experiment had three layers, each designed to test a different claim about Plexus's value:

1. **Regex tag extraction** — walk 59 Rust source files, extract tags from module paths, public type names, impl-for pairs, and ADR references. Feed them through the full enrichment pipeline (TagConceptBridger, CoOccurrenceEnrichment, EmbeddingSimilarityEnrichment at threshold 0.55, DiscoveryGapEnrichment). No LLM involved.

2. **LLM extraction via llm-orc** — run the same files through a `code-concepts` ensemble (llama3:8b analyzer → gemma3:1b tag producer). Compare output quality head-to-head with regex.

3. **Graph querying** — ask the resulting graph questions that `grep` and `find` cannot answer. Evaluate whether the answers constitute genuine intelligence about the codebase.

---

## What the Regex Graph Produced

234 concepts. 2,130 `similar_to` edges. 2,760 `may_be_related` edges. 1,640 `discovery_gap` edges. 373 `tagged_with` and `references` edges. Ingestion took 39 seconds including embedding computation.

The numbers look healthy. The enrichment pipeline ran without issues at this scale — 234 concepts means 27,261 possible pairs, and embedding similarity selected 3.9% of them. Co-occurrence produced 1,380 undirected pairs. Discovery gaps identified 820 undirected pairs where embedding similarity existed but structural connection did not.

But the numbers are misleading.

### The ADR Problem

Of 820 discovery gap pairs, 243 were ADR-to-ADR matches. `ADR-009 ↔ ADR-019` scored 1.0 similarity. `ADR-004 ↔ ADR-014` scored 0.98. These aren't meaningful relationships — they're an artifact of how embedding models handle number strings. "ADR-009" and "ADR-019" are nearly identical sequences that differ by one digit. The embedder can't distinguish between "these are related architectural decisions" and "these are strings that look alike."

This isn't a bug in the pipeline. The pipeline did exactly what it should: it found labels that embed similarly but don't co-occur. The problem is that the *labels themselves* don't carry enough meaning for embeddings to work well on them. "ADR-009" doesn't tell the embedder that this is about "tag-concept bridging." A human reading the label needs context the embedder doesn't have.

### The Module Path Problem

`adapter` appeared as a tag in 23 of 59 files. This makes it the dominant hub in the co-occurrence graph — 118 connections. But `adapter` as extracted by regex is a directory name, not a concept. It tells you where a file lives in the filesystem, not what it does. The co-occurrence graph faithfully reports that many concepts appear in `src/adapter/` files. This is true and useless.

The same problem affects `default` (14 files — it's Rust's `Default` trait implementation, appearing wherever a type derives Default), `analysis` (10 files — a directory), and `types` (4 files — a common module name).

### The Relationship Gap

Regex extraction captures names but not relationships. When it finds `impl AdapterSink for EngineSink`, it extracts two tags: `adaptersink` and `enginesink`. These become two concept nodes in the graph. But the *relationship* between them — that one implements the other — is lost. They co-occur (both appear in `engine_sink.rs`), so they get a `may_be_related` edge. But `may_be_related` doesn't tell you *how* they're related. It's the graph equivalent of "these two things were in the same room."

This matters because the queries you want to ask — "what implements this trait?", "what depends on this module?", "trace the dependency chain from API to storage" — all require typed relationships.

---

## The Discovery Gaps, Honestly

Filtering out ADR noise, 577 real discovery gap pairs remained. The strongest ones:

| Gap | Similarity | What It Means |
|-----|-----------|---------------|
| `markdownanalyzer ↔ markdownstructureanalyzer` | 1.00 | Two names for related analysis concepts in separate modules. The name similarity is the whole signal. |
| `sqlitestore ↔ sqlitevecstore` | 0.92 | Both implement storage, live in the same `storage/` directory, but have no co-occurring tags because they're defined in separate files. |
| `provenanceadapter ↔ provenanceentry` | 0.86 | The adapter produces entries, but they're in different modules. |
| `plexusapi ↔ plexusengine` | 0.76 | The public API wraps the engine. They're structurally disconnected because `PlexusApi` lives in `api.rs` and `PlexusEngine` in `graph/engine.rs`. |
| `engine_sink ↔ sink` | 0.75 | `EngineSink` implements the `AdapterSink` trait. Different files, no shared tags. |
| `enrichment ↔ enrichmentdeclaration` | 0.83 | The `Enrichment` trait (used in 7 files) and `EnrichmentDeclaration` (in `declarative.rs`) are clearly related but structurally disconnected. |

These gaps are *technically correct*. The concepts are semantically similar and structurally unconnected. But the reason they're unconnected is that our extraction can't capture the connections that exist in the code. `PlexusApi` wraps `PlexusEngine` — there's a `use crate::graph::PlexusEngine` import and method calls. `EngineSink` implements `AdapterSink` — there's an `impl` block. The connections exist in the source. Our extraction just can't see them.

Discovery gaps are designed to surface relationships the graph hasn't captured. Here, they're surfacing *extraction failures*. The graph doesn't know about these connections because we didn't tell it. The enrichment machinery is working. The extraction is what's failing.

---

## What the LLM Extraction Found

We built a `code-concepts` ensemble — two agents in an llm-orc pipeline — and ran it on the same files.

**Agent 1** (llama3:8b): reads a source file, extracts purpose, typed concepts, relationships, and noise classifications.

**Agent 2** (gemma3:1b): normalizes concepts into tags and preserves relationships as edges.

Head-to-head comparison on `engine_sink.rs`:

| Dimension | Regex | LLM |
|-----------|-------|-----|
| **Tags** | `adapter, engine_sink, enginesink, adaptersink, adr-006, adr-001, adr-010, adr-003, adr-023, adr-005` | `engine sink` (module), `adapter sink` (trait), `emission` (concept), `enrichment registry` (dependency), `framework context` (abstraction) |
| **Relationships** | None | `engine sink → adapter sink` (implements), `emission → engine sink` (produces) |
| **Noise filtering** | None | `ADR-001` flagged as "notation, not a concept"; `PlexusEngine` flagged as "implementation detail" |
| **Purpose** | None | "AdapterSink implementation for validating and committing graph emissions" |

The LLM extracted what the code *does*. Regex extracted what the code *contains*.

On `api.rs`, the LLM produced: purpose "Transport-independent API layer for consumer-facing operations," and the relationship `plexus_api → plexus_engine` (consumes). This is the exact relationship that appeared as a discovery gap in the regex graph. The LLM would have emitted it as an edge, eliminating the gap at the source.

On `sqlite_vec.rs`, the LLM extracted "Persistent vector storage using sqlite-vec for KNN vector search" as the purpose. Embedding that purpose string produces far better similarity matches than embedding `sqlitevecstore` — because it describes the *concept*, not just the name.

### The Noise Problem is the Interesting Problem

When we said "`adapter` in 23 files is noise," we were cheating — we already know the codebase. For someone graphing their own codebase for the first time, they don't know what's noise. This is exactly the kind of judgment a knowledge graph should help with.

In fact, "noise" may be the wrong frame. A concept appearing in 23 files is a signal — it's a cross-cutting concern, an architectural axis. The problem is that regex flattens three different things into the same tag:

- `adapter` as a **directory name** (structural, tells you where a file lives)
- `Adapter` as a **trait** (semantic, tells you what a module does)
- `FragmentAdapter` as an **implementation** (specific, tells you how)

An LLM distinguishes these because it understands context. On `engine_sink.rs`, it classified `AdapterSink` as a trait (high signal) while ignoring `adapter` as a path component. On `api.rs`, it flagged ADR references as notation. This filtering is not rule-based — it's judgment, applied per file, adapting to what it reads.

This means noise filtering is not a preprocessing step. It's a *semantic* operation that requires understanding the code. You can't write a frequency threshold that works across codebases because what counts as noise depends on the codebase's vocabulary. A concept appearing in 23 files might be genuinely important (it's the core abstraction) or genuinely noise (it's a module path). Only understanding can distinguish them.

---

## Querying the Graph: What Intelligence Looks Like

A knowledge graph is only as valuable as the questions it can answer. We tested six query types against the persisted graph.

### Impact Analysis

*"What breaks if I change the Enrichment trait?"*

Co-occurrence returns 38 concepts across 7 files — the full blast radius. `CoOccurrenceEnrichment`, `EmbeddingSimilarityEnrichment`, `DiscoveryGapEnrichment`, `TagConceptBridger`, `EnrichmentRegistry`, and every test helper that exercises enrichments.

This query works well today. Co-occurrence is a reliable proxy for coupling — things that appear in the same file are likely to be affected by changes to each other. With LLM-enriched data, you'd additionally know *how* they're coupled: which concepts implement the trait vs. use it vs. configure it.

### Provenance Trace

*"Where is this concept defined?"*

`enrichment` traces to 7 source files: `cooccurrence.rs`, `discovery_gap.rs`, `embedding.rs`, `enrichment.rs`, `integration_tests.rs`, `tag_bridger.rs`, `temporal_proximity.rs`. This is more useful than `grep "enrichment"` because it's semantic — it finds files where enrichment is a *concept*, not just a string match.

### Cross-Cutting Concerns

*"What concepts span multiple modules?"*

`types` appears in 4 directories: `analysis/`, `adapter/`, `provenance/`, `query/`. `traits` appears in 3: `analysis/`, `adapter/`, `storage/`. `provenance` appears in 2: `adapter/`, `provenance/`. These cross-cutting patterns reveal architectural decisions — and potential coupling risks.

### Onboarding Map

*"What are the most important concepts for a newcomer?"*

Concepts ranked by co-occurrence reach and file span: `adapter` (118 reach, 23 files), `analysis` (39 reach, 10 files), `enrichment` (38 reach, 7 files), `graph` (23 reach, 6 files), `query` (21 reach, 7 files). File span distinguishes architectural concepts from test helpers — `waitfornodeenrichment` has 20 reach but appears in only 1 file.

### The Missing Query

*"Trace the dependency chain from user request to storage."*

This query requires typed `depends_on` edges. The regex graph can't answer it because `PlexusApi` and `PlexusEngine` are a discovery gap (similarity 0.76, no structural connection). The LLM found this relationship — it extracted `plexus_api → plexus_engine` as a `consumes` edge. With LLM-enriched data, you could trace: `plexusapi → plexusengine → context → sqlitestore`.

---

## The Architecture of a CodeAdapter

The spike points toward a three-agent ensemble that generalizes across any codebase:

**Agent 1: File Concept Extractor** — per-file, parallelizable. Reads source code, extracts purpose (1 sentence), typed concepts (trait, struct, pattern, dependency), relationships (implements, depends_on, wraps), and noise classifications. Language-agnostic: the same prompt works on Rust, Python, TypeScript, Go — the LLM understands all of them.

**Agent 2: Relationship Synthesizer** — needs aggregate view. Given all extracted concepts across files, infers cross-module relationships that no single file reveals. "PlexusApi delegates to PlexusEngine" is visible from reading both files, but not from either alone.

**Agent 3: Signal Calibrator** — needs frequency distribution. Given the full tag distribution plus sample contexts, classifies each concept as architectural axis (high-frequency, meaningful), implementation detail (low-frequency, specific), or structural noise (high-frequency, not meaningful). This is where it says "In this codebase, `adapter` is a module namespace" vs. "In that codebase, `adapter` is a design pattern they chose deliberately."

This maps cleanly to the existing pipeline:

```
CodeAdapter (Agent 1 per file)
  → FragmentInput with LLM-extracted tags + purpose as text
    → TagConceptBridger (existing)
    → CoOccurrenceEnrichment (existing)
    → EmbeddingSimilarityEnrichment (existing, now embedding purpose strings)
    → DiscoveryGapEnrichment (existing)

Post-ingestion enrichments:
  Agent 2 → typed relationship edges
  Agent 3 → noise annotations on concept nodes
```

Agents 2 and 3 could be enrichments — they fire after initial ingestion, read graph state, and emit edges or node property updates. The entire flow runs through the existing `IngestPipeline`.

---

## What We Learned

**The enrichment pipeline works at scale.** 234 concepts, 27,261 possible pairs, meaningful selectivity (3.9%). Ingestion is fast (39 seconds with embeddings). Persistence and reload are correct. The graph machinery is not the bottleneck.

**Extraction quality is the bottleneck.** Regex extraction captures syntax. LLM extraction captures semantics. The difference is the difference between "these two things were in the same file" and "this thing implements that thing." Discovery gaps in the regex graph are technically correct but shallow — they surface extraction failures, not architectural insights.

**Noise filtering requires understanding.** What counts as noise depends on the codebase's own vocabulary. This is not a rule-based operation — it's semantic judgment that adapts to context. An LLM does this naturally; regex can't.

**The graph's value is in typed queries.** Impact analysis, provenance tracing, cross-cutting concern detection, and onboarding maps all work with co-occurrence data. But dependency chain tracing — the query with the most architectural value — requires typed relationships that only LLM extraction can produce.

**Embeddings on purpose strings beat embeddings on names.** "Persistent vector storage for KNN vector search" embeds into a meaningful neighborhood. `sqlitevecstore` embeds into a neighborhood of similar-looking strings. The LLM's purpose extraction directly improves embedding quality downstream.

**The self-modeling question has a conditional answer.** Can Plexus model itself? With regex extraction: partially — it finds what lives together but not why. With LLM extraction: yes — it captures purpose, relationships, and architectural structure. The graph engine is ready. The extraction layer needs the semantic depth that the `code-concepts` ensemble provides.
