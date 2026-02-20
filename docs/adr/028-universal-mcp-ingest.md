# ADR-028: Universal MCP Ingest Tool

**Status:** Accepted
**Research:** Essay 23
**Depends on:** ADR-012 (unified ingest pipeline), ADR-020 (declarative adapter specs)
**Supersedes:** ADR-015 (workflow-oriented write surface); ADR-020's `DeclarativeAdapter` naming (renamed to `SemanticAdapter`)

## Context

The MCP server exposes `annotate` as its only write tool — a narrow composite operation (ADR-015) that creates a text fragment, a provenance chain, and a mark. The internal API already has the right abstraction: `PlexusApi.ingest(context_id, input_kind, data)` routes to matching adapters, runs the enrichment loop, and returns outbound events (ADR-012). But the transport doesn't expose this generality. File extraction, semantic analysis, and declarative adapter specs are all unreachable.

Essay 23 established that `ingest` is the universal write path. `annotate` is one kind of ingest, not a distinct operation. The transport should reflect the same generality as the internal API (Invariant 38: transports are thin shells).

## Decision

**Replace `annotate` with `ingest(data, input_kind?)`.**

Every transport exposes a single write tool:

```
ingest(data: object, input_kind?: string)
```

`data` is JSON deserialized as `serde_json::Value`. `input_kind` is optional — callers who know what they're sending can specify it for deterministic routing; callers who don't can omit it and let the pipeline detect it. The transport is a thin shell: it forwards `data` and the optional `input_kind` to `PlexusApi.ingest()`. When `input_kind` is absent, the transport passes a sentinel (e.g., `"auto"`) and an input classifier in the adapter layer resolves it before routing.

**Detection precedence** when `input_kind` is absent:
1. Field-pattern matching: `{text}` or `{annotation}` → `"content"`, `{file_path}` → `"extract-file"`, JSON matching a registered semantic spec → that spec's input kind, etc.
2. No match → error with guidance on valid input shapes

### Core adapters

Three core adapters, named by purpose:

| Adapter | Input kind | Purpose |
|---------|-----------|---------|
| **ContentAdapter** | `"content"` | Direct content ingestion. Text + tags + origin metadata → fragment + provenance (chain + mark + contains). Always produces both semantic content and provenance (Invariant 7). Provenance granularity depends on what the caller provides: location-specific (file, line, chain_name) or source-level (source metadata). |
| **ExtractionCoordinator** | `"extract-file"` | Phased file extraction. Phase 1 (registration, file node) and Phase 2 (heuristic analysis) run in Rust. Phase 3 delegates to SemanticAdapter for LLM-powered extraction. Dual obligation (Invariant 7) is satisfied across phases: Phase 1 creates the structural record; Phases 2–3 produce semantic content + provenance. |
| **SemanticAdapter** | Per registered spec | Extracts semantics from content via YAML specs backed by llm-orc ensembles. Used by ExtractionCoordinator (Phase 3) and by domain-specific extraction (Sketchbin, EDDI, literary text). Different spec, different ensemble, same engine. |

ContentAdapter and ExtractionCoordinator are **sibling adapters** — each handles a different input kind. ContentAdapter handles direct content submission (Trellis text, Carrel annotations). ExtractionCoordinator handles file ingestion. SemanticAdapter is invoked by ExtractionCoordinator in Phase 3 and can also be invoked directly for domain-specific extraction via registered specs.

Adding a new domain means writing two YAML files: a semantic adapter spec and an llm-orc ensemble spec. No Rust code.

**`PlexusApi.annotate()` is removed.** The annotation workflow's composite logic (chain name normalization, auto-creation, fragment + chain + mark emission) migrates into ContentAdapter.

**DeclarativeAdapter (ADR-020) is renamed to SemanticAdapter.** The YAML-spec-driven adapter that maps JSON to graph primitives retains its declarative mechanism but takes a name that describes its purpose: semantic extraction. ADR-020's architectural decisions (YAML specs, template expressions, two-layer extraction) remain valid — only the name changes. The term `DeclarativeAdapter` is retired from the domain vocabulary.

**SemanticAdapter is reimplemented as a declarative YAML engine.** The existing `SemanticAdapter` Rust struct (519 lines with hardcoded `parse_response()` logic) becomes a YAML spec interpreter. Its orchestration responsibility (calling llm-orc) and its mapping responsibility (JSON → graph primitives) are both expressed declaratively. The Rust code becomes infrastructure for interpreting specs, not domain-specific extraction logic.

### Semantic adapter specs are registered

Semantic adapter specs are registered with Plexus (YAML files, validated at registration time). Each spec declares:
- An `input_kind` that the pipeline uses for routing
- Which llm-orc ensemble to invoke
- How to map the ensemble's JSON response to graph primitives (nodes, edges, provenance)

Specs are coupled to the client's domain — a Sketchbin asset has a specific extraction ensemble and response mapping, an EDDI dataset has another. Registration provides validation (catch spec errors once), discovery (clients can enumerate available input kinds), and efficiency (no per-request overhead).

### Enrichments are registered per context

Core enrichments (Rust, reactive, fast) are registered with the pipeline:
- TagConceptBridger, CoOccurrenceEnrichment, EmbeddingSimilarityEnrichment, DiscoveryGapEnrichment, TemporalProximityEnrichment

External enrichments (llm-orc, background) are also registered, not per-call. They're triggered by emissions but run asynchronously.

Enrichments are coupled to the graph context, not to any specific input domain. TagConceptBridger doesn't care whether the fragment came from Trellis, Carrel, or a Sketchbin asset — it bridges tags to concepts regardless. This is why enrichments are registered per context rather than arriving with requests.

### Provenance is layered

The caller always provides **origin provenance** — where the data came from: a file path (Macbeth.txt), a user submission (Trellis), an arXiv paper (Carrel). This is Layer 1. Every input carries origin.

The pipeline adds **structural provenance** — what was found within that source: themes extracted at specific line ranges, concepts discovered in sections, relationships between passages. This is Layer 2.

Both layers flow through `ingest`. The caller provides origin; the pipeline adds depth. For annotations (Carrel marking a passage in a paper), the caller provides both origin (the paper) and specific location (line, passage). For unstructured text (Trellis submission), the caller provides origin (source: "trellis"). For file extraction (ingesting Macbeth.txt), the caller provides only origin (the file path); Phase 1 creates a file node as the structural origin record, and Phase 3 SemanticAdapter creates provenance-dimension chains and marks tracing where concepts were found.

### JSON is the wire format

All transport-facing input arrives as `serde_json::Value`. The input classifier or registered semantic adapter spec interprets the JSON. For typed-struct adapters invoked internally (ExtractionCoordinator, ContentAdapter), the routing layer constructs typed input from the JSON fields.

## Consequences

**Positive:**
- One tool, one path — any adapter registered in the pipeline is reachable from any transport
- ContentAdapter guarantees fragment + provenance for direct content (Invariant 7)
- Callers provide data + origin provenance; the pipeline handles everything else
- New domains = two YAML files (semantic adapter spec + ensemble spec), no Rust
- Core adapters are stable (3 in Rust); domain variety lives in YAML specs
- Enrichments are context-level, domain-agnostic — registered once, affect all data
- Aligns with Invariant 34 (all writes through `ingest()`) and Invariant 38 (transports are thin shells)
- No backward compatibility burden — the system is pre-1.0

**Negative:**
- Input classification adds complexity at the adapter layer when `input_kind` is omitted
- `ingest` is less self-documenting than purpose-named tools — callers benefit from schema documentation
- Full pipeline registration increases server startup cost

**Neutral:**
- The annotation workflow's behavior is unchanged — same output, different entry point
- ADR-015 is superseded: the workflow is still a single call, but the transport no longer names individual workflows
- The internal API signature `PlexusApi.ingest(context_id, input_kind, data)` is unchanged
- SemanticAdapter retains its name but changes implementation: from hardcoded Rust to declarative YAML spec engine. DeclarativeAdapter (ADR-020) is renamed, not removed — the mechanism survives, the name changes.
