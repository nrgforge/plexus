# Orientation: Plexus

A content-agnostic knowledge graph engine that derives structure from unstructured input. Consumer applications send domain-specific data (creative writing fragments, research citations, code files, movement encodings) through adapters; Plexus tracks per-source contributions, detects cross-domain connections via enrichment algorithms, and returns structural signals — queryable through each consumer's own vocabulary (lens), with provenance explaining every connection. Consumers decide what to do with the signals.

*(Rewritten at RDD graduation, 2026-07-07. The graduated research corpus this document previously mapped is archived at [`archive/`](archive/); see [`rdd-graduation.md`](rdd-graduation.md) for the redirect map.)*

## Start here

[`vision.md`](vision.md) — the product vision (multiple clients, shared contexts, per-consumer lenses, cross-pollinated data), where the implementation honestly stands against it, and the current milestones (M0–M3) with the engineering queue.

## Key Constraints

1. **All writes go through `ingest()`** (Invariant 34) — no public API for raw graph primitives.
2. **All knowledge carries semantic content + provenance** (Invariant 7) — the dual obligation.
3. **Adapters, enrichments, transports are independent extension axes** (Invariant 40) — changes in one don't affect the others.
4. **Transports are thin shells** (Invariant 38) — adding a transport never touches adapters, enrichments, or the engine.
5. **Event cursors preserve the library rule for reads** (Invariant 58) — consumers write, walk away, come back, query "changes since N."
6. **Vocabulary layers are durable graph data; lens enrichments are durably registered on the context** (Invariant 62) — the specs table is the context's lens registry.

Full vocabulary and all 62 invariants: [`invariants.md`](invariants.md) — code cites these by number; the numbering is stable.

## Reading paths

**Build a consumer app / write a spec** →
[`../README.md`](../README.md) → [`references/spec-author-guide.md`](references/spec-author-guide.md) → worked example at `examples/specs/embedding-activation.yaml`

**Understand the architecture** →
[`system-design.md`](system-design.md) (modules, responsibilities, dependency rules, amendment log) → [`references/field-guide.md`](references/field-guide.md) (module-to-code map) → [`invariants.md`](invariants.md)

**Why is it built this way?** →
[`decisions/`](decisions/) (43 ADRs, 000–042, immutable) → [`archive/`](archive/) (essays, domain model with amendment history, product discovery, PLAY field notes)

**What's being worked on?** →
[`vision.md`](vision.md) §Milestones + GitHub issues (the engineering queue)

## Verification

- `cargo test` — 535 tests default-run (448 lib + 86 acceptance + 1 doc); `PLEXUS_INTEGRATION=1` adds real-Ollama gated tests (T6–T12)
- `tools/play-harness/` — programmatic consumer-surface scenarios against the shipped binary (crawl / walk / run / stale / extract-fg / extract-bg); `python3 tools/play-harness/play.py --help`
