# ADR-015: Workflow-Oriented Write Surface

**Status:** Accepted

**Research:** [Essay 14](../research/semantic/essays/14-public-surface-redesign.md), [Research Log Q2](../research-log.md)

**Domain model:** [domain-model.md](../domain-model.md) — annotate, mark, chain, ingest, transport

**Depends on:** ADR-012 (unified ingest pipeline), ADR-014 (transport-independent API)

---

## Context

The current MCP surface exposes `create_chain` and `add_mark` as separate tools. A consumer annotating a file location must first create a chain (if one doesn't exist), then add a mark to it — two tool calls requiring the consumer to understand the chain/mark containment model. This exposes graph structure as implementation detail.

MCP tool design research (Essay 14) found that workflow-based design — exposing operations that match user intent rather than graph primitives — reduces sequential calls and LLM hallucination risk. The operation "annotate this location with these tags" is one conceptual action.

## Decision

### `annotate` replaces `create_chain` + `add_mark`

The `annotate` operation accepts a chain name (not a chain ID), a file location, annotation text, and tags. If no chain with that name exists in the context, one is created automatically. The deterministic chain ID for user-named chains follows the existing scheme: `chain:provenance:{normalized_name}` where `normalized_name` is the chain name lowercased, with whitespace replaced by hyphens and characters that conflict with ID format separators (`:`, `/`) replaced by hyphens. Non-ASCII characters are preserved. Empty or whitespace-only names are rejected. This fits the `chain:{adapter_id}:{source}` pattern — the adapter is "provenance" (ProvenanceAdapter) and the source is the normalized name.

`annotate` is a `PlexusApi`-level composite operation, not a single ingest call. The API layer: (1) resolves the chain name to a deterministic chain ID, (2) checks if the chain exists in the context, (3) if not, creates it via `ingest("provenance", CreateChain{...})`, then (4) creates the mark via `ingest("provenance", AddMark{...})`. This is two ingest calls in the chain-creation case, one in the common case where the chain already exists. The composition lives in `PlexusApi`, not in the adapter or transport.

`create_chain` is removed as a standalone consumer-facing operation. Chains are created implicitly through `annotate` or through adapter-produced provenance (e.g., FragmentAdapter creating `chain:{adapter_id}:{source}`).

### Write tools at the transport layer

Transports present `ingest` with workflow-oriented names:

| Transport tool | API call | Description |
|---------------|----------|-------------|
| `annotate` | `PlexusApi.annotate(...)` → 1-2 `ingest()` calls | Mark a location, auto-create chain |
| `ingest_fragment` | `ingest("fragment", FragmentInput{...})` | Send a tagged fragment |
| `link_marks` | `ingest("provenance", LinkMarks{...})` | Connect two marks |
| `unlink_marks` | `ingest("provenance", UnlinkMarks{...})` | Remove a mark connection |
| `delete_mark` | `ingest("provenance", DeleteMark{...})` | Remove a mark |
| `delete_chain` | `ingest("provenance", DeleteChain{...})` | Remove a chain and its marks |

Five of six route through a single `ingest()` call. `annotate` is the exception — it's a `PlexusApi` composite that may issue two `ingest()` calls (chain creation + mark creation). All paths get dual obligation, enrichment, and outbound events.

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
