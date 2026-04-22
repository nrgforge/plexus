# Conformance Scan Report — DECIDE phase (Default-Install Experience and Lens Design Principles)

**Scanned against:** `docs/decisions/038` through `042` (Proposed), `docs/decisions/024`, `025`, `026`, `033`, `034` (Accepted)
**Codebase:** `/Users/nathangreen/Development/plexus`
**Date:** 2026-04-20
**Cycle:** Default-Install Experience and Lens Design Principles

---

## Summary

- **ADRs checked:** 10 (5 Proposed + 5 Accepted prior)
- **Proposed ADRs with zero violations:** 2 (ADR-038, ADR-041)
- **Proposed ADRs with violations:** 3 (ADR-039, ADR-040, ADR-042)
- **Accepted ADRs in conformance:** 5 (ADR-024, 025, 026, 033, 034) — no violations
- **Total violations found:** 7 (3 Structural, 2 Gap, 2 Drift)

---

## Conformance Debt Table

| # | ADR | Violation | Type | Location | Resolution |
|---|-----|-----------|------|----------|------------|
| D-01 | ADR-039 | `ContentAdapter` creates fragment and concept nodes without writing `created_at` to `node.properties`. The property ends up in `NodeMetadata.created_at` (framework bookkeeping), which `TemporalProximityEnrichment` does not read. Enrichment fires, finds no timestamp on any adapter-created node, silently emits nothing. | Structural | `src/adapter/adapters/content.rs:183–200` (fragment node construction); concept node via `concept_node()` helper in `src/adapter/types.rs` | `fix:` — write `PropertyValue::String(chrono::Utc::now().to_rfc3339())` to `node.properties["created_at"]` at node construction in `ContentAdapter::process()` and in the `concept_node()` helper (or at each call site that creates concept nodes); trivial per site, medium across all sites |
| D-02 | ADR-039 | `ExtractionCoordinator::run_registration()` creates file nodes and `extraction-status` nodes without writing `created_at` to `node.properties`. | Structural | `src/adapter/adapters/extraction.rs:296–372` (file node and status node construction in `run_registration`) | `fix:` — write `created_at` ISO-8601 UTC string to `node.properties` on both nodes at construction; trivial |
| D-03 | ADR-039 | `DeclarativeAdapter::interpret_create_node()` creates nodes from spec primitives without automatically injecting `created_at` into `node.properties`. ADR-039 §"Built-in adapters write `created_at`" states DeclarativeAdapter should write `created_at` on each created node unless the spec's emit explicitly sets the property (in which case the spec-authored value wins). | Structural | `src/adapter/adapters/declarative.rs:887–905` (`interpret_create_node`) | `fix:` — after processing `cn.properties` map, insert `"created_at"` with current UTC ISO-8601 string if not already present in the rendered properties; trivial |
| D-04 | ADR-039 | `TemporalProximityEnrichment::extract_timestamp()` parses `PropertyValue::String(s)` as `s.parse::<u64>().ok()` — treating the string as epoch milliseconds. ADR-039 mandates ISO-8601 UTC string format parsed via `chrono::DateTime<Utc>`. An ISO-8601 `created_at` like `"2026-04-16T12:34:56Z"` fails `u64` parse and yields `None`, so even if adapters start writing ISO-8601 strings the enrichment silently skips all nodes. | Structural | `src/adapter/enrichments/temporal_proximity.rs:112–123` (`extract_timestamp`) | `fix:` — change string branch to `chrono::DateTime::parse_from_rfc3339(s).ok().map(\|dt\| dt.timestamp_millis() as u64)`; update tests to use ISO-8601 strings rather than `PropertyValue::Int` for the `created_at` property (existing tests use integer values, which pass via the `Int` branch — they do not catch the ISO-8601 gap); small |
| D-05 | ADR-040 | `DiscoveryGapEnrichment` module doc and struct doc do not include the explicit documentation the ADR mandates: that the enrichment fires only when some producer emits its configured trigger relationship, that in the default Homebrew build there is no built-in producer of `similar_to`, and that silent-idle-by-design is expected behavior (not a bug). Current doc mentions "from embeddings" parenthetically but does not name the silent-idle contract. | Gap | `src/adapter/enrichments/discovery_gap.rs:1–12` (module doc), `src/adapter/enrichments/discovery_gap.rs:22–28` (struct doc) | `docs:` — expand module and struct doc to include the trigger-dependency statement; trivial |
| D-06 | ADR-042 | `resolve_dimension()` in `DeclarativeAdapter` rejects any dimension string not in the six hardcoded core values (`structure`, `semantic`, `provenance`, `relational`, `temporal`, `default`) with `"unknown dimension: {}"`. This blocks consumer-declared extensibility dimensions (`"gesture"`, `"harmonic"`, `"movement-phrase"`) at spec `process()` time. ADR-042 requires syntactic validation only — rejecting empty strings, whitespace, and reserved characters (`:`, `\0`); every other string must pass, including novel consumer dimensions. The current architecture also defers dimension errors to `process()` time, contradicting ADR-042's requirement that `load_spec` validate syntactic well-formedness upfront (Invariant 60 boundary). | Structural | `src/adapter/adapters/declarative.rs:226–239` (`resolve_dimension`); spec validator path in `from_yaml()` / `validate_spec()` | `refactor:` — (a) replace the exclusive match in `resolve_dimension` with a permissive well-formedness check accepting any non-empty string without whitespace or reserved characters; (b) update node/edge construction call sites to accept owned strings instead of `&'static str`; (c) add the syntactic well-formedness check to `validate_spec()` so `load_spec` fails fast on malformed dimension values per Invariant 60. Medium — touches `interpret_create_node`, `interpret_create_edge`, and the validator tree |
| D-07 | ADR-042 (Drift) / ADR-009 (Provenance) | `src/graph/node.rs` lines 8–10 the `dimension` module doc comment reads "See ADR-009: Multi-Dimensional Knowledge Graph Architecture." ADR-009 is "Automatic Tag-to-Concept Bridging" (superseded by removal of `TagConceptBridger`). The multi-dimensional graph architecture is not in ADR-009. The same incorrect reference appears at line 157 on the `dimension` field of `Node`. | Drift | `src/graph/node.rs:10`, `src/graph/node.rs:157` | `docs:` — update comment to reference the correct ADR for dimension architecture (or remove the ADR citation and use a generic reference — the dimension design predates the ADR-009 repurpose, so the reference was never correct); trivial |

---

## Prior ADRs (024, 025, 026, 033, 034) — Conformance Status

All five accepted prior ADRs are in conformance. Specific checks:

- **ADR-024:** `DiscoveryGapEnrichment`, `TemporalProximityEnrichment`, `CoOccurrenceEnrichment`, `EmbeddingSimilarityEnrichment` all present with correct `id()` formats. `TagConceptBridger` absent from codebase. External enrichment path (llm-orc via spec `ensemble:`) wired through `DeclarativeAdapter`. No violations.
- **ADR-025:** `DeclarativeAdapter.from_yaml()`, `enrichments()`, `lens()` all present. `ensemble:` field wired to `LlmOrcClient`. `enrichments:` YAML section instantiates `co_occurrence`, `discovery_gap`, `temporal_proximity`, `embedding_similarity` types. No violations.
- **ADR-026:** `EmbeddingSimilarityEnrichment` present. `FastEmbedEmbedder` gated behind `#[cfg(feature = "embeddings")]` at `src/adapter/enrichments/embedding.rs:120–169`. `with_default_enrichments()` registers `EmbeddingSimilarityEnrichment` only within the `#[cfg(feature = "embeddings")]` block at `src/adapter/pipeline/builder.rs:96–109`. No violations.
- **ADR-033:** `LensSpec`, `TranslationRule`, `NodePredicate` defined. `DeclarativeAdapter::lens()` returns `Option<Arc<dyn Enrichment>>`. `LensEnrichment` implements namespace convention `lens:{consumer}:{to}` and per-source contribution keys `lens:{consumer}:{to}:{from}`. No violations.
- **ADR-034:** `QueryFilter` with `contributor_ids`, `relationship_prefix`, `min_corroboration` at `src/query/filter.rs:13–20`. `RankBy` enum present. All query structs have optional `filter` field. No violations.

---

## Notes

**Pattern: the `created_at` property contract is a four-site fix.** D-01 through D-04 are a single coherent failure: the ADR-039 bug is a full producer-consumer mismatch, not one missing line. Four sites must move together — three producer sites (ContentAdapter, ExtractionCoordinator, DeclarativeAdapter) and one consumer site (the ISO-8601 parser in TemporalProximityEnrichment). The fix is mechanical at each site, but coordinated: writing ISO-8601 strings at the producer side only works if the consumer parses ISO-8601; fixing the parser without fixing the producers changes nothing observable. BUILD should treat D-01 through D-04 as a single `fix:` commit.

**The `resolve_dimension` issue (D-06) is the highest-consequence structural violation.** A consumer who authors a spec declaring `dimension: "gesture"` or any novel dimension gets a runtime error when `process()` first runs — not a load-time validation failure. This makes the extensibility story ADR-042 promises impossible to exercise without modifying Plexus Rust code. The fix requires changing `resolve_dimension` from an exclusive allowlist to a syntactic well-formedness check, and updating the node/edge construction call sites to accept owned strings rather than `&'static str`. The current behavior also means the `validate_spec` path at `from_yaml()` time passes even for invalid dimension strings (the dual-obligation check runs but dimension validation does not) — a spec with `dimension: ""` deserializes successfully only to fail later. The resolution should move validation to `validate_spec()` per Invariant 60.

**D-07 is a known-drift item.** Cycle-status MODEL-phase entry flagged this as a DECIDE/BUILD cleanup. Listed here for completeness.

**Scenario candidates vs. implementation cleanup:**

Debt items that map naturally to behavior scenarios (observable from the outside, testable):

- D-01 through D-04 (combined): "TemporalProximityEnrichment fires between two nodes that both carry `created_at` written by ContentAdapter." Also: "TemporalProximityEnrichment correctly parses ISO-8601 UTC string for `created_at`."
- D-06: "A declarative adapter spec declaring `dimension: gesture` loads successfully and creates nodes in that dimension." And validation-side: "A declarative adapter spec declaring `dimension: ""` fails at `load_spec` with a clear validation error" and "A declarative adapter spec declaring `dimension: lens:trellis` fails at `load_spec` with a clear validation error."

Purely implementation cleanup with no natural scenario shape:

- D-05: expanding a docstring — documentation deliverable, not a behavior change.
- D-07: correcting an incorrect ADR citation — docs only.
