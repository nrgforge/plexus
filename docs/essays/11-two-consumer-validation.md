# Two-Consumer Validation: Cross-Dimensional Bridges with Shared Vocabulary

> **Superseded.** This essay describes an architecture where FragmentAdapter produces only semantic output and ProvenanceAdapter produces only provenance output. Both directions were wrong. Essay 12 ("Provenance as Epistemological Infrastructure") established that adapters produce provenance alongside semantics. The reverse also holds: there is no provenance-only path — every annotation is at minimum a fragment carrying semantic content. Carrel's description below as a "provenance-only consumer" is incorrect; Carrel is a full consumer of the multi-dimensional graph (see Essay 14). Essay 13 ("Two-Consumer Validation Revisited") revalidates the two-consumer scenario under the new architecture. The spike test from this essay has been removed as redundant. This essay is preserved for historical context only.

Essay 10 ended with a recommendation: feed real data through the pipeline and see what the graph produces. Not more infrastructure — sample data. This essay describes what happened when that experiment ran, using synthetic data shaped by two real consumers.

## The Chicken-and-Egg Problem

Plexus has two sibling applications waiting to become its first real consumers:

**Trellis** — a non-generative creative scaffolding system for writers. Captures handwritten fragments via MMS and OCR, tags them, and accumulates them in a knowledge graph. Data shape: `FragmentInput { text, tags, source, date }`, handled by the existing `FragmentAdapter`.

**Carrel** — a writer's research desk coordinating writing projects and academic paper discovery. Uses Plexus provenance (chains, marks, links) to annotate draft passages and research papers. Data shape: provenance operations (create chain, add mark with tags, link marks), handled by the existing `ProvenanceAdapter`.

Neither application is fully built. Both were waiting on the adapter layer. But the adapter layer was built without real consumer data to validate against. The question was whether to continue building infrastructure or to test what exists.

The answer was a spike: simulate both consumers at the data level using synthetic data drawn from Carrel's real theme vocabulary, and observe what the pipeline produces.

## What Made the Experiment Possible

The critical insight from exploring both repositories was that **tags are the shared language**. A Trellis fragment tagged "distributed-ai" and a Carrel mark tagged "#distributed-ai" should produce cross-dimensional connections via TagConceptBridger enrichment. The tag normalization built into the enrichment — strip `#`, lowercase, prepend `concept:` — means both consumers converge on the same concept nodes without coordination.

This is the architectural bet the enrichment loop was designed for: independent consumers contributing to a shared context, with reactive enrichments discovering connections that neither consumer could produce alone.

## The Spike

Six Trellis-like fragments were ingested, using Carrel's real theme vocabulary: distributed-ai, compute-economics, policy, design-constraints, non-generative-ai, federated-learning, network-science. Tags overlapped across fragments to create a realistic co-occurrence pattern.

Four Carrel-like provenance marks were added: two writing marks (draft passages about distributed compute as insurance and design constraints for non-generative AI) and two research marks (academic papers on federated learning economics and network-science policy). Each mark carried tags from the same vocabulary. Two links connected research marks to writing marks.

Both consumers shared a single context, registered as separate integrations with shared enrichments (TagConceptBridger + CoOccurrenceEnrichment).

## What the Graph Produced

### Structure

19 nodes across three dimensions: 7 semantic (concept nodes), 6 structure (fragment nodes), 6 provenance (2 chains, 4 marks). 45 edges across 5 relationship types: `tagged_with`, `contains`, `links_to`, `references`, `may_be_related`. The three dimensions are cleanly separated, connected only by cross-dimensional edges.

### Cross-Dimensional Bridges

TagConceptBridger created 8 `references` edges from provenance marks to semantic concepts — 2 per mark, matching each mark's tags to existing concept nodes. All 8 edges carry the correct source and target dimensions. The bridging is automatic and bidirectional: marks created after concepts are bridged immediately, and concepts created after marks would bridge in the reverse direction.

These 8 edges are the connections that neither consumer could produce alone. Trellis created the concept nodes. Carrel created the marks. The enrichment loop connected them.

### The Key Traversal

Starting from research-mark-1 ("Chen et al. 2025 — federated learning economics"), a depth-2 BFS traversal reaches all three dimensions:

**Depth 1** — references edges reach `concept:federated-learning` and `concept:compute-economics`. A `contains` edge reaches the research chain. A `links_to` edge reaches writing-mark-1.

**Depth 2** — from the concepts, `tagged_with` edges (reversed) reach 3 writing fragments tagged with `compute-economics` and 2 fragments tagged with `federated-learning`. From `concept:compute-economics`, a `references` edge (reversed) reaches writing-mark-1 via a second path — the same mark also reachable by direct `links_to`.

The traversal proves the thesis: a research annotation in Carrel can discover related writing fragments in Trellis through the shared concept layer, without either application knowing the other exists.

### Co-Occurrence

8 unique `may_be_related` concept pairs were detected, with graduated scores. Three pairs scored 1.0 (distributed-ai with compute-economics, distributed-ai with policy, compute-economics with policy), co-occurring in 2 fragments each. Five pairs scored 0.5, representing single co-occurrences.

At this scale the graduated scores meaningfully separate a tightly-connected "distributed AI economics" cluster from peripheral connections. Whether those clusters are semantically meaningful — whether "distributed-ai" is related to "compute-economics" in a way that matters to the writer — requires domain judgment. This is the future direction involving LLM-based semantic interpretation.

### Persistence

All 19 nodes, 45 edges (including per-adapter contributions), and cross-dimensional references survived a save-load cycle. The integration test verifies this explicitly.

## What Was Missing: Outbound Events

The spike revealed one concrete gap. Both adapters return empty outbound event vectors — neither overrides `transform_events()`. The pipeline produces the right graph mutations but doesn't tell the consumer what happened. After ingesting a fragment, Trellis receives `Ok(vec![])`. After adding a tagged research mark that bridges to three writing concepts, Carrel receives `Ok(vec![])`.

This matters because outbound events are how the pipeline's value becomes visible to consumers. Without them, a consumer would need to query the graph after every ingest call to discover what changed.

### The Design That Emerged

`transform_events` receives all accumulated events (primary emission + every enrichment round) plus a context snapshot. Each `GraphEvent` carries an `adapter_id` field — primary events carry the adapter's own ID, enrichment events carry `"tag-bridger"` or `"co-occurrence"`. This is enough information to generate domain-meaningful notifications without any new infrastructure.

Two categories of outbound events emerged:

**Confirmation events** — `fragment_indexed`, `chain_created`, `mark_added`, `marks_linked`, `mark_removed`, `chain_deleted`. These are ACKs. They confirm the operation succeeded and return IDs. They're operationally necessary — `fragment_indexed` is how Trellis learns the UUID assigned to its fragment — but any database could provide them.

**Discovery events** — `concepts_detected`, `bridges_formed`, `co_occurrences_updated`. These are where Plexus adds value beyond a simple store. They tell the consumer: "The graph learned something new because of what you just contributed." A Trellis user sees "your fragment about distributed compute is connected to research on federated learning economics." A Carrel user sees "your research annotation bridges to three writing fragments through shared concepts."

The distinction matters for understanding Plexus's value proposition. Confirmation events justify replacing a database. Discovery events justify the enrichment pipeline.

### Events as Notifications

The `OutboundEvent { kind, detail }` type is deliberately simple, and that's correct. Events should be change notifications, not data payloads. The event says *what changed*; the consumer queries the graph for *details*. This is analogous to database change notifications: "table X was updated" rather than "here are the new rows."

The consumer already has a connection to Plexus via the transport layer and can issue traversal queries. The event tells it *when* to re-query. This keeps the event type domain-agnostic while letting consumers build rich UIs from graph queries.

## What This Means

### The Adapter Contracts Work

FragmentAdapter and ProvenanceAdapter both fit cleanly into the ingest pipeline. No changes were needed to the Adapter trait, the Enrichment trait, or the Emission type. The six ProvenanceInput variants — including deletions and edge removals — all express naturally as emissions. The pipeline doesn't care what the adapter does internally; it cares that the adapter speaks emissions and the enrichment loop handles cross-dimensional effects.

### TagConceptBridger Is the Critical Enrichment

It's what makes the graph cross-dimensional. Without it, provenance and semantic are isolated islands. The tag normalization (`#distributed-ai` to `concept:distributed-ai`) is the glue. The bidirectional behavior — new marks bridge to existing concepts, new concepts bridge to existing marks — means ordering doesn't matter. Whichever consumer contributes first, the connections form when the second arrives.

### The Enrichment Adapter ID Convention Is an Implicit API

Adapters that report on enrichment results via `transform_events` depend on knowing enrichment IDs — `"tag-bridger"`, `"co-occurrence"`. This coupling is acceptable because enrichments and adapters are registered together via `register_integration()`, but it should be documented as a convention rather than left implicit.

### What Trellis Needs to Integrate

1. A transport — Trellis is Python, so REST, gRPC, or PyO3 FFI
2. `fragment_indexed` outbound events to learn assigned node IDs
3. `bridges_formed` events to surface cross-dimensional connections in the UI

The FragmentAdapter, TagConceptBridger, and CoOccurrenceEnrichment are ready. The remaining work is plumbing.

### What Carrel Needs to Integrate

1. The MCP transport already works for provenance operations
2. `transform_events` implementations for confirmation and discovery events
3. Traversal queries to surface "related writing fragments" from research marks

Carrel is closer to working integration than Trellis because the MCP transport already handles provenance.

### Open Questions That Remain

**Fragment ID determinism.** Fragment IDs are currently UUIDs. Re-ingesting the same fragment creates a duplicate node. Content-hash-based IDs would enable idempotent re-ingestion. This matters for Trellis, which may re-send fragments during sync.

**Multi-hop query ergonomics.** The depth-2 traversal from research-mark-1 works, but the TraverseQuery API is low-level. A higher-level query like "given this mark, find related fragments" would encode the mark-to-concept-to-fragment path as a named pattern. This matters for consumer-facing tools that shouldn't need to know about graph dimensions.

**Semantic interpretation of co-occurrence.** The `may_be_related` edges correctly identify concept clusters. Whether those clusters are meaningful requires domain knowledge — a direction involving LLM orchestration that was identified but intentionally deferred.

## Test Suite

246 tests, zero failures. The spike added one integration test that validates the complete two-consumer cross-dimensional flow including persistence.
