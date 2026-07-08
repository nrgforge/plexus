# Changelog

## 0.5.0 - 2026-07-08

Consumer-trust release: weights you can display, evidence you can query.

### Breaking / behavior changes

- **Raw weights change on first recompute (ADR-043).** Scale
  normalization is now max-abs (`value / max|values|` per contributor),
  replacing divide-by-range (ADR-005, superseded). Ratios and sign are
  preserved and near-ties stay near-ties (previously a 100× spread);
  rank order within a contributor is unchanged, but absolute raw-weight
  values shift for existing graphs on their next write.
- **Lens edges no longer carry a redundant emitter contribution slot.**
  Edges emitted with explicit contribution maps keep exactly those
  contributions (a single-source lens edge previously showed doubled
  weight). Corroboration counts on lens edges now mean "distinct source
  relationships."

### Added

- **`explain_edge`** (18th MCP tool + `PlexusApi::explain_edge`) — one
  call answers "why is this connection here?": both endpoints with
  displayable text, every edge between the pair (parallel edges
  included), stored contributions, corroboration, and lens contribution
  keys parsed into `translated_from`.
- **Flow diagrams** at `docs/references/flows.md` — the weight
  pipeline, the ingest → enrichment → lens flow, and explain-edge
  resolution.

### Fixed

- **`traverse` reports all parallel edges.** The first edge to a
  neighbor no longer hides the others; nodes still appear once per
  traversal.

## 0.4.0 - 2026-07-08

Multi-process coherence and the cross-pollination release.

- Reads reload on `data_version` change (ADR-017 §2): long-lived
  processes see other processes' writes.
- Spec lenses sync across processes at ingest time, both directions
  (load and unload).
- Background extraction phases run the enrichment loop — consumer
  lenses now cover deep llm-orc extraction.
- Extraction agents receive document content (previously metadata only).
- `weight:` accepts template expressions; per-rule `min_corroboration`
  in the lens grammar; `node_types` scoping for TemporalProximity
  (fragment-scoped by default — behavior change; the enrichment ID
  changed, so prior temporal contributions are orphaned per
  Invariant 13).
- `analyze` requires an explicit `--ensemble`.

## 0.3.0 and earlier

See git history and `docs/decisions/`.
