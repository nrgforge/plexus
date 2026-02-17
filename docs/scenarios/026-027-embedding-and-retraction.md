# Behavior Scenarios: ADR-026 (Embedding as Enrichment) & ADR-027 (Contribution Retraction)

---

## Feature: Embedding Similarity Enrichment (ADR-026)

### Scenario: Embedding enrichment fires on new nodes
**Given** an EmbeddingSimilarityEnrichment registered with model "test-model", threshold 0.7, and output relationship "similar_to"
**And** a context with existing concept nodes ["travel", "journey"] that have cached embeddings with cosine similarity 0.85
**When** a NodesAdded event fires for a new concept node "voyage" whose embedding has cosine similarity 0.9 to "travel" and 0.88 to "journey"
**Then** the enrichment emits symmetric edge pairs: voyage↔travel (contribution 0.9) and voyage↔journey (contribution 0.88) with relationship "similar_to"
**And** the enrichment's contribution slot is "embedding:test-model" on each edge

### Scenario: Embedding enrichment respects similarity threshold
**Given** an EmbeddingSimilarityEnrichment registered with threshold 0.7
**And** a context with existing concept node "apple" with cached embedding
**When** a NodesAdded event fires for a new concept node "democracy" whose embedding has cosine similarity 0.3 to "apple"
**Then** the enrichment emits no edges (0.3 < 0.7 threshold)

### Scenario: Embedding enrichment produces symmetric edge pairs
**Given** an EmbeddingSimilarityEnrichment registered with output relationship "similar_to"
**When** the enrichment detects similarity 0.8 between nodes A and B
**Then** two edges are emitted: A→B and B→A, both with contribution 0.8
**And** both edges have relationship type "similar_to"

### Scenario: Embedding enrichment is idempotent
**Given** an EmbeddingSimilarityEnrichment that previously emitted similar_to edges for node pair (A, B)
**When** a new enrichment round fires with the same context state
**Then** the enrichment returns None (quiescent) — the edges already exist

### Scenario: Embedding enrichment batches node bursts
**Given** an EmbeddingSimilarityEnrichment
**When** a NodesAdded event contains 10 new concept nodes
**Then** the enrichment embeds all 10 nodes in a single batch call (not 10 individual calls)
**And** compares each new embedding against all cached embeddings

### Scenario: Embedding enrichment ID encodes model name
**Given** an EmbeddingSimilarityEnrichment configured with model_name "nomic-embed-text-v1.5"
**Then** its `id()` returns "embedding:nomic-embed-text-v1.5"

### Scenario: Different embedding models produce separate contribution slots
**Given** a context where EmbeddingSimilarityEnrichment "embedding:model-a" emitted a similar_to edge A→B with contribution 0.8
**When** a second EmbeddingSimilarityEnrichment "embedding:model-b" also detects similarity 0.75 between A and B
**Then** edge A→B has two contribution slots: {"embedding:model-a": 0.8, "embedding:model-b": 0.75}
**And** the raw weight is computed via scale normalization across both slots

### Scenario: Embedding enrichment filters by node dimension
**Given** an EmbeddingSimilarityEnrichment configured to embed concept nodes in the semantic dimension
**And** a context with a provenance-dimension mark node and a semantic-dimension concept node
**When** a NodesAdded event fires containing both nodes
**Then** only the concept node is embedded and cached
**And** the mark node is ignored

### Scenario: Embedding enrichment triggers discovery gap detection
**Given** a context with concept nodes A and B that are structurally disconnected (no shared source nodes)
**And** EmbeddingSimilarityEnrichment and DiscoveryGapEnrichment are both registered
**When** the embedding enrichment emits a similar_to edge between A and B
**Then** DiscoveryGapEnrichment fires in the next enrichment loop round
**And** emits a discovery_gap edge between A and B (latently similar but structurally disconnected)

---

## Feature: Declarative Embedding Enrichment Configuration (ADR-026)

### Scenario: Embedding enrichment declared in adapter spec YAML
**Given** an adapter spec YAML containing:
```yaml
enrichments:
  - type: embedding_similarity
    model_name: nomic-embed-text-v1.5
    similarity_threshold: 0.7
    output_relationship: similar_to
```
**When** the spec is parsed and the integration is registered
**Then** an EmbeddingSimilarityEnrichment is instantiated with the specified parameters
**And** its id is "embedding:nomic-embed-text-v1.5"

---

## Feature: Contribution Retraction (ADR-027)

### Scenario: Retract all contributions from a specific adapter
**Given** a context with edges where adapter "embedding:model-a" has contribution slots
**And** some edges also have contributions from other adapters
**When** `retract_contributions(context_id, "embedding:model-a")` is called
**Then** the "embedding:model-a" contribution slot is removed from every edge
**And** raw weights are recomputed from remaining contributions
**And** a ContributionsRetracted event fires with the adapter ID and count of affected edges

### Scenario: Edge pruning after retraction removes zero-evidence edges
**Given** a context with edge A→B whose only contribution is from "embedding:model-a" (no other adapters contributed)
**When** `retract_contributions(context_id, "embedding:model-a")` is called
**Then** the contribution slot is removed
**And** edge A→B has an empty contributions map
**And** edge A→B is pruned from the context
**And** an EdgesRemoved event fires for edge A→B

### Scenario: Retraction preserves edges with remaining contributions
**Given** a context with edge A→B that has contributions from both "embedding:model-a" (0.8) and "co_occurrence:tagged_with:may_be_related" (0.6)
**When** `retract_contributions(context_id, "embedding:model-a")` is called
**Then** the "embedding:model-a" slot is removed
**And** edge A→B still exists with contribution {"co_occurrence:tagged_with:may_be_related": 0.6}
**And** raw weight is recomputed from the remaining contribution

### Scenario: Retraction of nonexistent adapter is a no-op
**Given** a context with edges
**When** `retract_contributions(context_id, "nonexistent-adapter")` is called
**Then** no edges are modified
**And** a ContributionsRetracted event fires with edges_affected: 0

### Scenario: Discovery gap cleanup after embedding retraction
**Given** a context where:
- EmbeddingSimilarityEnrichment "embedding:model-a" produced similar_to edges between A↔B
- DiscoveryGapEnrichment produced discovery_gap edges between A↔B (latently similar, structurally disconnected)
**When** `retract_contributions(context_id, "embedding:model-a")` is called
**Then** the similar_to edges between A↔B are pruned (their only contributor was retracted)
**And** EdgesRemoved fires, triggering the enrichment loop
**And** DiscoveryGapEnrichment re-evaluates: with no similar_to edge, the gap condition no longer holds
**And** the discovery_gap edges are not re-emitted (cleaned up via idempotency)

### Scenario: Model replacement workflow
**Given** a context with similar_to edges from "embedding:model-v1"
**When** `retract_contributions(context_id, "embedding:model-v1")` is called
**And** then new similar_to edges from "embedding:model-v2" are ingested
**Then** the context contains only "embedding:model-v2" contribution slots on similar_to edges
**And** no "embedding:model-v1" slots remain
**And** raw weights reflect only model-v2's assessments

### Scenario: Contribution lifecycle completeness
**Given** a context with edge A→B
**When** adapter "test-adapter" emits A→B with contribution 0.5 (add)
**And** then adapter "test-adapter" re-emits A→B with contribution 0.8 (update via latest-value-replace)
**And** then `retract_contributions(context_id, "test-adapter")` is called (remove)
**Then** the "test-adapter" contribution slot no longer exists on edge A→B
**And** if no other contributions remain, edge A→B is pruned

### Scenario: Retraction is context-scoped
**Given** two contexts "ctx-a" and "ctx-b", both containing edges with contributions from "embedding:model-a"
**When** `retract_contributions("ctx-a", "embedding:model-a")` is called
**Then** only edges in "ctx-a" have the "embedding:model-a" slot removed
**And** edges in "ctx-b" are unaffected

---

## Feature: External Embedding Enrichment via llm-orc (ADR-026)

### Scenario: llm-orc embedding ensemble produces similar_to edge proposals
**Given** an llm-orc ensemble with an `embed_text.py` script that reads graph export and computes pairwise similarity
**When** the ensemble is executed against a context
**Then** it returns similar_to edge proposals for node pairs above the similarity threshold
**And** the results re-enter via `ingest()` with the ensemble's adapter ID

### Scenario: External embedding results trigger core enrichments
**Given** a context with concept nodes A and B (structurally disconnected)
**And** DiscoveryGapEnrichment is registered
**When** an llm-orc embedding ensemble returns a similar_to edge between A and B via `ingest()`
**Then** the edge is committed to the context
**And** the enrichment loop fires
**And** DiscoveryGapEnrichment detects the new similar_to edge and produces a discovery_gap edge
