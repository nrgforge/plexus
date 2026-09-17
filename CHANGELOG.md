# Changelog

## 0.6.0 - 2026-09-16

Remote-hosting release: reach llm-orc over HTTP, serve MCP over HTTP.
No breaking changes; stdio and the local llm-orc subprocess remain the
defaults.

### Added

- **`plexus mcp --transport http --bind <addr>`** — the same tool set
  over MCP streamable HTTP at `http://<bind>/mcp` (default bind
  `127.0.0.1:8766`; `--bind 127.0.0.1:0` picks a free port and logs
  `listening on <url>`). Many clients share one process. No auth: bind
  loopback and let a reverse proxy or the tailnet be the boundary.
  - **Session semantics:** each `Mcp-Session-Id` gets its own active
    context. `set_context` in one client never affects another; the
    graph, loaded specs, and lenses are shared across sessions. Sessions
    are in-memory, so a restart invalidates them and clients
    re-initialize. An unknown session id is answered with HTTP 401.
- **`--llm-orc-url <base>` / `PLEXUS_LLM_ORC_URL`** on `plexus mcp` and
  `plexus analyze` — `HttpClient` talks to a running `llm-orc serve`
  (`GET /health`, `POST /api/ensembles/{name}/execute`) instead of
  spawning `llm-orc` locally. The flag wins over the env var; unset
  keeps `SubprocessClient`. No overall request timeout (long ensembles
  are normal); 10 s connect timeout; 404 → ensemble not found, 5xx →
  unavailable (extraction degrades gracefully, Invariant 47).
- **`db_checkpoint`** (19th MCP tool + `PlexusApi::db_checkpoint` +
  `GraphStore::checkpoint`) — runs `PRAGMA wal_checkpoint(TRUNCATE)` and
  returns `{busy, wal_pages, checkpointed_pages}`. Database-level, no
  active context needed; `busy: true` means another connection held the
  database, retry later.
- Library: `PipelineBuilder::default_pipeline_with_client`,
  `PlexusMcpServer::with_client` / `new_session`, `McpServerConfig`,
  `McpTransport`, `llm_orc::{HttpClient, select_client, resolve_url}`.

### Shared database (ADR-017) — what a consumer may rely on

A long-lived HTTP process plus an occasional stdio process on one
SQLite file is the ADR-017 shared-DB case. The store opens in WAL mode;
every connection has a 5 s busy timeout (rusqlite default), so
concurrent writers wait rather than fail; each persist is one short
transaction (persist-per-emission, Invariant 30), so no writer holds
the lock for the length of an ingest. Readers never block on writers.
SQLite auto-checkpoints at 1000 WAL pages; `db_checkpoint` forces a
full truncating checkpoint on demand.

### Dependencies

- `reqwest` (rustls) for `HttpClient`; `axum` and the rmcp
  `transport-streamable-http-server` feature for the HTTP transport.

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
