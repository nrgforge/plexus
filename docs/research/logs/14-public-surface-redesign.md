# Research Log: Public Surface Design

## Question 1: What queries do the three known consumers (Trellis, Carrel, Manza) actually need, and can the existing query primitives express them?

**Method:** Spike (consumer query mapping against existing FindQuery, TraverseQuery, PathQuery)

**Findings:**

### Existing query system

Three primitives: `FindQuery` (filter nodes), `TraverseQuery` (BFS from origin), `PathQuery` (shortest path). All context-scoped, all executed in-memory.

Key limitations:
- **TraverseQuery has no dimension filtering.** It discovers everything within N hops regardless of dimension.
- **TraverseQuery allows only one relationship filter.** Cross-dimensional evidence trails follow different relationships at different depths (`references` then `contains`).
- **No typed multi-hop traversal.** Can't say "follow `references` at hop 1, then `contains` at hop 2."
- All filters are AND-only. No OR, no negation.

### Consumer query map

**Trellis** (app-to-app, write-heavy): Needs basic reads — list concepts, find fragments for a concept, check edge weights. Mostly expressible with FindQuery + single-hop TraverseQuery.

**Carrel** (LLM host, annotation): Simple provenance queries work (list_chains, get_chain, list_marks already exist). The hard query: "what other evidence supports the same concept as this mark?" — requires multi-hop cross-dimensional traversal that over-fetches massively with current primitives.

**Manza** (real-time visualization): Full graph snapshots work (iterate nodes + edges). Neighborhood exploration works (single-hop traverse). The hard queries: evidence trails, event streaming (delta since last update), cluster detection.

### The shared gap

All three consumers need the same cross-dimensional query: **"What is the evidence trail for concept X?"**

This requires:
1. concept → marks (via `references`, incoming) — provenance dimension
2. concept → fragments (via `tagged_with`, incoming) — structure dimension
3. marks → chains (via `contains`, incoming) — stay in provenance dimension

Each step follows a different relationship type. Current TraverseQuery can only filter by one relationship. Depth-2 with direction Both returns everything within 2 hops — massive over-fetch that the consumer must post-filter.

### How Essay 13 actually does it

The Essay 13 "key traversals" (the most important queries in the system) don't use the query system at all. They use raw `ctx.edges().filter(|e| ...)` — manual iteration over all edges with hand-written predicates. The query system is literally unused for the queries that matter most.

### The missing primitive

A **typed traversal** (or "path pattern") that specifies the relationship to follow at each hop:

```
from(concept_id)
  .step(Incoming, "references")    // → marks
  .step(Incoming, "tagged_with")   // → fragments
  .step(Incoming, "contains")      // → chains
```

This would express the evidence trail as a single query instead of 3 separate traversals + post-filtering.

**Implications:**

1. The query system works for simple single-dimension queries. Cross-dimensional queries — the whole point of the multi-dimensional graph — are not well-served.
2. The gap is not about missing features on existing primitives. It's a missing *primitive* — typed multi-hop traversal with per-step relationship filtering.
3. All three consumers need the same core query (evidence trail). The abstraction belongs in Plexus core, not in transport-specific code.
4. Manza's event streaming need (delta since last update) is a separate infrastructure concern (OQ8), not a query ergonomics issue.

---

## Question 2: What should the MCP tool surface look like?

**Method:** Web research on MCP tool design patterns + synthesis against consumer needs

**Findings:**

### MCP tool design principles (from ecosystem research)

1. **"Less is More"** — tool definitions consume 5-7% of context window before a single prompt. More tools = worse LLM selection accuracy + higher token cost. ([Klavis: Less is More](https://www.klavis.ai/blog/less-is-more-mcp-design-patterns-for-ai-agents))

2. **Workflow-based design** — instead of granular CRUD, create atomic operations that handle entire workflows. `deploy_project` instead of `create_project` + `add_env_vars` + `create_deployment`. Reduces sequential calls and hallucination risk. ([Klavis](https://www.klavis.ai/blog/less-is-more-mcp-design-patterns-for-ai-agents))

3. **Progressive discovery** — guide agents through logical stages rather than exposing everything at once. ([Klavis](https://www.klavis.ai/blog/less-is-more-mcp-design-patterns-for-ai-agents))

4. **Design for the LLM, not the human** — tool names, descriptions, and error messages should be optimized for agent comprehension. ([Arcade: 54 Patterns](https://blog.arcade.dev/mcp-tool-patterns))

5. **MCP spec (2025-06-18)** adds `outputSchema` for structured results and `annotations` for tool behavior metadata. Tools are model-controlled — the LLM selects them based on description matching. ([MCP spec](https://modelcontextprotocol.io/specification/2025-06-18/server/tools))

### Current surface vs. principles

The current 19-tool surface violates "Less is More" directly. But the issue isn't just count — it's that the tool names expose graph primitives (`create_chain`, `add_mark`, `link_marks`) rather than user intent. An LLM selecting between `create_chain` and `add_mark` needs to understand the chain/mark containment model. That's implementation leakage.

### Key insight: MCP is one transport, not the API

The conversation established that Plexus's API should be transport-independent. MCP is a thin shell that translates MCP tool calls into API operations. So the question isn't "what MCP tools should exist?" — it's "what API operations should exist?" and then "how does MCP present them?"

This means we should design the API first, then derive the MCP surface from it. The MCP surface might present operations with more LLM-friendly names or collapse some operations for ergonomics, but the underlying API is what matters.

### Proposed API operations

**Write (all via ingest pipeline):**
- `ingest(context_id, input_kind, data)` — the single write endpoint. Adapters registered for each input_kind handle the rest.

**Provenance reads:**
- `list_chains(context_id, status?)` — list annotation chains
- `get_chain(context_id, chain_id)` — chain with its marks
- `list_marks(context_id, filters)` — search marks
- `list_tags(context_id)` — all tags in use
- `get_links(context_id, mark_id)` — mark's incoming/outgoing links

**Graph reads:**
- `evidence_trail(context_id, node_id)` — THE cross-dimensional query (new)
- `traverse(context_id, start, depth, direction)` — generic exploration
- `find_nodes(context_id, filters)` — node search
- `find_path(context_id, source, target)` — shortest path

**Context management:**
- `context_create`, `context_delete`, `context_list`, `context_rename`
- `context_add_sources`, `context_remove_sources`

**Provenance mutations (non-ingest):**
- `update_mark(context_id, mark_id, changes)` — read-modify-write
- `archive_chain(context_id, chain_id)` — status change

That's ~17 operations. But the MCP surface can collapse some:
- Write operations could be presented as named tools (`annotate`, `ingest_fragment`) that call `ingest` underneath with the appropriate input_kind — more LLM-friendly than a generic `ingest` tool requiring schema knowledge.
- `update_mark` and `archive_chain` are mutations that don't go through ingest (read-modify-write pattern). They stay as direct tools.

### MCP surface (derived from API)

**Write tools (workflow-oriented names):**
- `annotate` — create a mark (auto-creates chain if needed). Wraps `ingest("provenance", AddMark{...})`.
- `ingest_fragment` — send a fragment with tags. Wraps `ingest("fragment", FragmentInput{...})`.
- `link_marks` / `unlink_marks` — structural annotation operations
- `delete_mark` / `delete_chain` — destructive operations

**Read tools:**
- `evidence_trail` — the cross-dimensional query (NEW)
- `list_chains` / `get_chain` / `list_marks` / `list_tags` / `get_links` — provenance reads
- `traverse` — generic graph exploration

**Context tools:**
- `context_create` / `context_delete` / `context_list` / `context_rename`
- `context_add_sources` / `context_remove_sources`

**Mutation tools (non-ingest):**
- `update_mark` / `archive_chain`

Total: ~17 MCP tools. Down from 19, but more importantly:
- Write tool names match user intent (`annotate` vs `add_mark`)
- All writes route through ingest (dual obligation, enrichment, outbound events)
- The key new capability (`evidence_trail`) is a first-class tool
- Context tools are unchanged (already clean CRUD)

### Open question: should `annotate` auto-create chains?

Currently `create_chain` is a separate operation. The workflow-based pattern suggests: let `annotate` accept an optional chain name, auto-create the chain if it doesn't exist, and return the chain_id + mark_id. This collapses two tool calls into one for the common case while preserving the ability to pre-create chains.

This would remove `create_chain` as a separate MCP tool entirely.

**Implications:**

1. API design should be transport-independent. MCP derives from it, doesn't define it.
2. The "Less is More" principle supports collapsing write tools into workflow-oriented operations.
3. The biggest win isn't reducing tool count — it's adding `evidence_trail` as a first-class operation. This is the query that makes the multi-dimensional graph useful to consumers.
4. Non-ingest mutations (`update_mark`, `archive_chain`) are a design smell — they bypass the pipeline. Worth noting but not blocking.
