# ADR-036: MCP Query Surface

**Status:** Accepted

## Context

The MCP transport exposes 9 tools: session management (`set_context`), a single write path (`ingest`, ADR-028), context lifecycle (6 tools), and one graph read (`evidence_trail`). The Rust API layer (ADR-014) offers a substantially larger query surface — `find_nodes`, `traverse`, `find_path`, `changes_since`, `list_tags`, `shared_concepts`, plus provenance reads and write mutations — none reachable through MCP.

The query surface cycle (ADRs 033–035) delivered composable filters, lens declaration, and event cursors at the API level. Without MCP exposure, LLM-mediated consumers — the primary MCP audience — cannot exercise these capabilities. The pull paradigm (Invariant 58) remains inaccessible via MCP.

Product discovery identifies pull as fundamental: "a CRON job checking 'what new connections emerged since yesterday, in my vocabulary' is fundamentally pull." Invariant 38 (thin shell) requires the transport to expose the same query endpoints as any other transport.

## Decision

### 1. Expose seven new tools through MCP

| Tool | API Method | Purpose |
|------|-----------|---------|
| `load_spec` | `PlexusApi::load_spec` | Load a declarative adapter spec (adapter + lens + enrichments) onto the active context (ADR-037) |
| `find_nodes` | `PlexusApi::find_nodes` | Node search by type, dimension, properties; incident-edge filter semantics (ADR-034) |
| `traverse` | `PlexusApi::traverse` | Multi-depth graph traversal from origin node |
| `find_path` | `PlexusApi::find_path` | Shortest path between two nodes |
| `changes_since` | `PlexusApi::changes_since` | Pull-based event query with sequence cursor (ADR-035) |
| `list_tags` | `PlexusApi::list_tags` | Concept discovery — all tags in active context |
| `shared_concepts` | `PlexusApi::shared_concepts` | Cross-context concept intersection |

Total tool count: 16 (9 existing + 7 new).

### 2. Serialize filter parameters as flat optional fields

`QueryFilter` (ADR-034) fields appear as flat optional parameters on `find_nodes`, `traverse`, and `find_path`:

- `contributor_ids: Option<Vec<String>>` — provenance-scoped filtering
- `relationship_prefix: Option<String>` — lens-scoped filtering
- `min_corroboration: Option<usize>` — evidence diversity threshold

`CursorFilter` fields appear as flat optional parameters on `changes_since`:

- `event_types: Option<Vec<String>>`
- `adapter_id: Option<String>`
- `limit: Option<usize>`

`RankBy` is an optional string on `traverse`: `"raw_weight"` or `"corroboration"`.

`Direction` is an optional string on `traverse` and `find_path`: `"outgoing"`, `"incoming"`, `"both"` (default: `"outgoing"`).

Flat fields are preferred over nested objects on the assumption that LLM consumers construct flat JSON more reliably — a convention followed by existing MCP tools in this project.

`load_spec` takes a single `spec_yaml: String` parameter — the full YAML content of the declarative adapter spec. The consumer sends the spec content inline, not a file path reference. This preserves transport independence (Invariant 38) and consumer ownership (Invariant 61).

### 3. StepQuery kept internal

`StepQuery` (arbitrary multi-hop traversal) is not exposed via MCP. It is a building block for programmatic Rust consumers that requires constructing arbitrary step sequences. `evidence_trail` (already exposed, Section 5) wraps a `StepQuery` internally — the distinction is that `evidence_trail` is a purpose-built composite query for provenance tracing, while raw `StepQuery` exposes arbitrary step sequences that require programmatic construction. LLM consumers use `evidence_trail` for provenance and compose `traverse` calls for other multi-hop queries.

### 4. Cursor state is client-managed

The client passes a sequence number to `changes_since` and receives `latest_sequence` in the response for the next poll. Plexus stores no per-client cursor state. This preserves Invariant 58 (library rule for reads).

### 5. Existing evidence_trail gains filter parameter

Invariant 59 requires provenance-scoped filtering composable with *any* query primitive, including `evidence_trail`. The existing `PlexusApi::evidence_trail` method and its MCP tool gain an optional `QueryFilter` parameter, piped through to the underlying `StepQuery`.

**Useful filter fields for evidence_trail:** `contributor_ids` (scope the provenance trail to a specific adapter's contributions) and `min_corroboration` (filter to well-corroborated edges in the trail) compose meaningfully. `relationship_prefix` does not compose well with the provenance traversal pattern — evidence trails traverse provenance-dimension edges (`contains`, `references`) that do not use `lens:` namespace prefixes. Applying `relationship_prefix: "lens:trellis:"` to `evidence_trail` will typically produce empty results.

### Out of scope

**Provenance writes:** `update_mark`, `archive_chain`, `delete_mark`, `delete_chain`, `link_marks`, `unlink_marks`, `retract_contributions` are explicitly excluded from the MCP surface. The transport's write path is `ingest` (ADR-028, Invariant 34); provenance is a consequence of ingestion, not a separate write concern. These operations exist at the Rust API level for programmatic consumers but do not belong in the MCP consumer model.

**Deeper provenance reads:** `list_chains`, `get_chain`, `list_marks`, `get_links` exist at the Rust API level. `evidence_trail` covers the primary MCP use case — tracing a concept back to its source material. Deeper provenance inspection can be surfaced later if LLM consumers need to drill beyond what `evidence_trail` returns.

## Consequences

**Positive:**
- Pull paradigm (Invariant 58) accessible via MCP for first time
- Composable filters (ADR-034) exercisable by LLM consumers
- Transport surface approaches API surface (Invariant 38)
- 16 tools within usable range for LLM tool selection
- Consumer can declare identity at interaction time via `load_spec`

**Negative:**
- 16 tools approaches upper bound for LLM tool selection reliability; further tiers may need tool grouping or selection hints
- `evidence_trail` API method requires a refactor prerequisite (add `QueryFilter` parameter)

**Neutral:**
- Provenance writes excluded by design; deeper provenance reads available at Rust API if needed later
- `load_spec` is the only tool that is not a thin wrapper — it involves validation, persistence, and enrichment execution. All other new tools are thin wrappers.
- No new Rust query logic for the query tools — purely transport wiring plus one parameter addition
