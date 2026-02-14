# ADR-015: Workflow-Oriented Write Surface

**Status:** Accepted

**Research:** [Essay 14](../essays/14-public-surface-redesign.md), [Research Log Q2](../research/research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — annotate, mark, chain, ingest, transport

**Depends on:** ADR-012 (unified ingest pipeline), ADR-014 (transport-independent API)

---

## Context

The current MCP surface exposes `create_chain` and `add_mark` as separate tools. A consumer annotating a file location must first create a chain (if one doesn't exist), then add a mark to it — two tool calls requiring the consumer to understand the chain/mark containment model. This exposes graph structure as implementation detail.

MCP tool design research (Essay 14) found that workflow-based design — exposing operations that match user intent rather than graph primitives — reduces sequential calls and LLM hallucination risk. The operation "annotate this location with these tags" is one conceptual action.

## Decision

### `annotate` replaces `create_chain` + `add_mark` **UPDATED: annotate produces semantic content**

> **Updated.** The original design routed `annotate` through ProvenanceAdapter only — creating marks and chains without semantic content. This violated the bidirectional dual obligation (ADR-001 §5): all knowledge entering the graph requires both semantic content and provenance. An annotation's text IS a fragment; its tags produce concepts. The `annotate` workflow must produce semantic output (fragment node, concept nodes, `tagged_with` edges) alongside provenance output (mark, chain, `contains` edge). ProvenanceAdapter handles the provenance-dimension mechanics internally, but `annotate` is not a provenance-only operation. Implementation of the updated workflow (composing fragment ingest with provenance creation) is tracked as future work.

The `annotate` operation accepts a chain name (not a chain ID), a file location, annotation text, and tags. If no chain with that name exists in the context, one is created automatically. The deterministic chain ID for user-named chains follows the existing scheme: `chain:provenance:{normalized_name}` where `normalized_name` is the chain name lowercased, with whitespace replaced by hyphens and characters that conflict with ID format separators (`:`, `/`) replaced by hyphens. Non-ASCII characters are preserved. Empty or whitespace-only names are rejected. This fits the `chain:{adapter_id}:{source}` pattern — the adapter is "provenance" (ProvenanceAdapter) and the source is the normalized name.

`annotate` is a `PlexusApi`-level composite operation. The API layer: (1) resolves the chain name to a deterministic chain ID, (2) checks if the chain exists in the context, (3) if not, creates it via `ingest("provenance", CreateChain{...})`, (4) ingests the annotation text as a fragment via `ingest("fragment", FragmentInput{...})`, and (5) creates the mark via `ingest("provenance", AddMark{...})`. The annotation text enters the semantic graph as a fragment; the mark provides the provenance layer on top.

`create_chain` is removed as a standalone consumer-facing operation. Chains are created implicitly through `annotate` or through adapter-produced provenance (e.g., FragmentAdapter creating `chain:{adapter_id}:{source}`).

### Write tools at the transport layer

Transports present `ingest` with workflow-oriented names:

| Transport tool | API call | Description |
|---------------|----------|-------------|
| `annotate` | `PlexusApi.annotate(...)` → 2-3 `ingest()` calls | Mark a location with semantic content, auto-create chain |
| `ingest_fragment` | `ingest("fragment", FragmentInput{...})` | Send a tagged fragment |
| `link_marks` | `ingest("provenance", LinkMarks{...})` | Connect two marks |
| `unlink_marks` | `ingest("provenance", UnlinkMarks{...})` | Remove a mark connection |
| `delete_mark` | `ingest("provenance", DeleteMark{...})` | Remove a mark |
| `delete_chain` | `ingest("provenance", DeleteChain{...})` | Remove a chain and its marks |

`annotate` is a `PlexusApi` composite: it creates a fragment (semantic content), a mark (provenance), and optionally a chain (if new). `ingest_fragment` is the direct fragment path. Link/unlink/delete are operations on existing graph structure. All paths get dual obligation, enrichment, and outbound events.

### Alternatives considered

- **Keep `create_chain` as a separate operation.** The status quo. Forces consumers to understand the chain/mark containment model. Two tool calls for the common case. Violates the workflow-based design principle.

- **Make chains entirely implicit (never user-visible).** Rejected: chains are meaningful domain objects. A research chain and a writing chain group marks by purpose. The consumer should name the chain; the system should handle creation.

- **Accept chain ID instead of chain name.** Rejected: chain IDs are deterministic (`chain:{adapter_id}:{source}`) but opaque. Names are what consumers think in. The API resolves names to IDs.

## Consequences

**Positive:**

- One tool call to annotate, not two
- Consumers don't need to understand chain/mark containment
- `create_chain` disappears from the consumer-facing surface — fewer tools, less context window consumption
- Chains are still first-class objects — consumers name them and query them (`list_chains`, `get_chain`)

**Negative:**

- Auto-creation means chains can be created with just a name — no description field in the common case. Consumers who want descriptions must use `update_chain` (or the `annotate` operation could accept an optional description for new chains).
- Chain name uniqueness becomes important. Two `annotate` calls with the same chain name must resolve to the same chain. This is handled by deterministic chain IDs: `chain:provenance:{normalized_name}`. If a chain is deleted and later re-created with the same name, the same ID is reused — this is consistent with the upsert semantics of deterministic IDs throughout the system.

**Neutral:**

- `ProvenanceInput::CreateChain` still exists in the adapter layer — `annotate` uses it internally. The change is at the API and transport layers, not the adapter layer.
- Existing integration tests that use `ProvenanceInput::CreateChain` directly remain valid — they test the adapter, not the consumer-facing surface.
