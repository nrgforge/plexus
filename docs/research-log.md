# Research Log: OQ10 — Emission Removal Variant

## Question 1: Can Emission support edge and node removal variants so that unlink_marks and delete_chain route through the adapter pipeline?

**Method:** Spike (codebase exploration)

**Findings:** OQ10 is already resolved. The implementation done during the ADR-012 build cycle added both removal types to `Emission`:

- `removals: Vec<Removal>` — node removals (with edge cascade in the engine)
- `edge_removals: Vec<EdgeRemoval>` — targeted edge removals (source + target + relationship)

`ProvenanceAdapter.process()` handles all three deletion operations:
- `DeleteMark` → single node removal (engine cascades edges)
- `UnlinkMarks` → targeted edge removal (`links_to` relationship)
- `DeleteChain` → multiple node removals in one emission (marks + chain node)

MCP routes all three through `pipeline.ingest()`. The engine's `emit_inner` processes removals in phases 3 (edges) and 4 (nodes with cascade), firing appropriate `NodesRemoved` and `EdgesRemoved` events.

`ProvenanceApi` still has direct methods (`delete_chain`, `delete_mark`, `unlink_marks`) but they are unused by any transport — vestigial from pre-adapter era.

**Implications:** OQ10 is resolved. No design work needed. The domain model's open question has been updated to reflect this. The vestigial `ProvenanceApi` direct methods could be removed during the ADR-014 build (when `PlexusApi` becomes the single entry point), but they're not harmful.
