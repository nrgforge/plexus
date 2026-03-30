# Behavior Scenarios: Query Surface

**ADRs:** 033 (lens declaration), 034 (composable query filters), 035 (event cursor persistence)

**Domain vocabulary:** lens, translation, enrichment, contribution, corroboration, event cursor, query filter, provenance-scoped filtering, raw weight, normalized weight, consumer, adapter spec, enrichment loop, graph event, changes since

---

## Feature: Lens Declaration and Translation (ADR-033)

### Scenario: Lens creates translated edges from matching source relationships
**Given** a context with a registered declarative adapter whose spec includes:
```yaml
lens:
  consumer: trellis
  translations:
    - from: may_be_related
      to: thematic_connection
```
**And** the context contains concept nodes A and B with a `may_be_related` edge between them (raw weight 0.6)
**When** the enrichment loop runs after an emission that added the `may_be_related` edge
**Then** a new edge exists from A to B with relationship `lens:trellis:thematic_connection`
**And** that edge's contributions contain key `lens:trellis:thematic_connection:may_be_related`

### Scenario: Many-to-one translation produces per-source contribution slots
**Given** a context with a registered lens:
```yaml
lens:
  consumer: trellis
  translations:
    - from: [may_be_related, similar_to]
      to: thematic_connection
```
**And** concept nodes A and B are connected by both a `may_be_related` edge (raw weight 0.4) and a `similar_to` edge (raw weight 0.7)
**When** the enrichment loop runs
**Then** one edge exists from A to B with relationship `lens:trellis:thematic_connection`
**And** that edge's contributions map has two keys: `lens:trellis:thematic_connection:may_be_related` and `lens:trellis:thematic_connection:similar_to`
**And** the edge's corroboration count is 2

### Scenario: Lens respects min_weight threshold
**Given** a context with a registered lens:
```yaml
lens:
  consumer: trellis
  translations:
    - from: may_be_related
      to: thematic_connection
      min_weight: 0.3
```
**And** concept nodes A and B with a `may_be_related` edge (raw weight 0.1)
**When** the enrichment loop runs
**Then** no `lens:trellis:thematic_connection` edge exists between A and B

### Scenario: Lens is idempotent across enrichment rounds
**Given** a context with a registered lens translating `may_be_related` to `thematic_connection`
**And** the lens has already created a `lens:trellis:thematic_connection` edge between A and B
**When** the enrichment loop runs another round (triggered by other enrichment activity)
**Then** exactly one `lens:trellis:thematic_connection` edge exists between A and B (no duplicate)

### Scenario: Untranslated edges remain accessible
**Given** a context with a registered lens translating only `may_be_related`
**And** concept nodes A and C are connected by a `similar_to` edge (not in the lens translation rules)
**When** a consumer traverses from A without relationship filtering
**Then** the traversal result includes node C via the `similar_to` edge
**And** no `lens:trellis:*` edge exists between A and C

### Scenario: Lens output is visible to all consumers
**Given** two registered adapters: trellis-content (with a lens) and carrel-research (without a lens)
**And** the trellis lens has created `lens:trellis:thematic_connection` edges in the shared context
**When** carrel-research traverses the context without filtering
**Then** the traversal results include `lens:trellis:thematic_connection` edges

### Scenario: Adapter without lens section works identically to before
**Given** a declarative adapter spec with no `lens:` section
**When** the adapter is constructed via `DeclarativeAdapter::from_yaml()`
**Then** `adapter.lens()` returns `None`
**And** `pipeline.register_integration()` succeeds with only the adapter's declared enrichments

---

## Feature: Composable Query Filters (ADR-034)

### Scenario: QueryFilter with contributor_ids scopes traversal to specific adapters
**Given** a context with edges where:
  - edge A→B has contributions from `content-adapter` and `co_occurrence:tagged_with:may_be_related`
  - edge A→C has contributions only from `content-adapter`
**When** traversing from A with `filter: { contributor_ids: ["co_occurrence:tagged_with:may_be_related"] }`
**Then** the traversal reaches B (edge has matching contributor)
**And** the traversal does not reach C (edge lacks matching contributor)

### Scenario: QueryFilter with relationship_prefix scopes to lens output
**Given** a context with edges:
  - A→B with relationship `lens:trellis:thematic_connection`
  - A→C with relationship `may_be_related`
**When** traversing from A with `filter: { relationship_prefix: "lens:trellis:" }`
**Then** the traversal reaches B
**And** the traversal does not reach C

### Scenario: min_corroboration filters weakly corroborated edges
**Given** a context with edges:
  - A→B with contributions from 3 distinct adapters (corroboration count = 3)
  - A→C with contributions from 1 adapter (corroboration count = 1)
**When** traversing from A with `filter: { min_corroboration: 2 }`
**Then** the traversal reaches B
**And** the traversal does not reach C

### Scenario: QueryFilter fields compose with AND semantics
**Given** a context with edges:
  - A→B: relationship `lens:trellis:thematic_connection`, corroboration count = 3
  - A→C: relationship `lens:trellis:topic_link`, corroboration count = 1
  - A→D: relationship `may_be_related`, corroboration count = 4
**When** traversing from A with `filter: { relationship_prefix: "lens:trellis:", min_corroboration: 2 }`
**Then** the traversal reaches B (matches prefix AND corroboration)
**And** the traversal does not reach C (matches prefix but fails corroboration)
**And** the traversal does not reach D (matches corroboration but fails prefix)

### Scenario: Filter with None fields applies no constraint
**Given** a context with edges of various relationship types and corroboration counts
**When** traversing from A with `filter: { contributor_ids: None, relationship_prefix: None, min_corroboration: None }`
**Then** the traversal behaves identically to traversal without a filter

### Scenario: StepQuery with QueryFilter — filter composes with per-step relationship
**Given** a context where:
  - A→B via `lens:trellis:thematic_connection`
  - B→C via `lens:trellis:topic_link`
  - A→D via `tagged_with`
**When** executing a StepQuery from A with:
  - step 1: direction Outgoing, relationship `lens:trellis:thematic_connection`
  - step 2: direction Outgoing, relationship `lens:trellis:topic_link`
  - filter: `{ relationship_prefix: "lens:trellis:" }`
**Then** step 1 reaches B, step 2 reaches C
**And** node D is never reached (its edge relationship does not match the prefix)

### Scenario: StepQuery with conflicting step relationship and filter prefix terminates early
**Given** a context where A→B via `tagged_with`
**When** executing a StepQuery from A with:
  - step 1: direction Outgoing, relationship `tagged_with`
  - filter: `{ relationship_prefix: "lens:trellis:" }`
**Then** step 1 finds zero edges (exact match `tagged_with` does not match prefix `lens:trellis:`)
**And** the result contains only the origin node

### Scenario: RankBy Corroboration orders results by evidence diversity
**Given** a traversal result containing edges:
  - A→B with corroboration count 1
  - A→C with corroboration count 4
  - A→D with corroboration count 2
**When** the result is ranked by `RankBy::Corroboration`
**Then** the ordering within depth level 1 is [C, D, B] (descending corroboration)

### Scenario: find_nodes with min_corroboration returns globally filtered results
**Given** a context with 10 concept nodes, where:
  - 3 nodes have incident edges with corroboration count >= 3
  - 7 nodes have incident edges with corroboration count < 3
**When** executing `find_nodes` with `filter: { min_corroboration: 3 }` and `node_type: "concept"`
**Then** the result contains at most 3 nodes

---

## Feature: Event Cursor Persistence (ADR-035)

### Scenario: Events are persisted with sequence numbers after emission
**Given** an empty context with no prior events
**When** a consumer ingests data that produces `NodesAdded` and `EdgesAdded` graph events
**Then** the `events` table contains entries for `NodesAdded` and `EdgesAdded`
**And** each entry has a monotonically increasing `sequence` number
**And** each entry has the correct `context_id` and `adapter_id`

### Scenario: changes_since returns events after the given cursor
**Given** a context with 10 persisted events (sequences 1 through 10)
**When** a consumer calls `changes_since(context_id, cursor: 5, filter: None)`
**Then** the result contains events with sequences 6, 7, 8, 9, 10
**And** `latest_sequence` is 10

### Scenario: changes_since with cursor 0 returns all events
**Given** a context with 5 persisted events
**When** a consumer calls `changes_since(context_id, cursor: 0, filter: None)`
**Then** the result contains all 5 events

### Scenario: CursorFilter scopes by event_type
**Given** a context with events: 3 `NodesAdded`, 2 `EdgesAdded`, 1 `WeightsChanged`
**When** a consumer calls `changes_since(context_id, cursor: 0, filter: { event_types: ["EdgesAdded"] })`
**Then** the result contains exactly 2 events, all of type `EdgesAdded`

### Scenario: CursorFilter scopes by adapter_id
**Given** a context with events from adapters `content-adapter` and `lens:trellis:thematic_connection`
**When** a consumer calls `changes_since(context_id, cursor: 0, filter: { adapter_id: "lens:trellis:thematic_connection" })`
**Then** the result contains only events produced by the lens enrichment

### Scenario: Enrichment loop events are persisted per round
**Given** a context where ingesting a fragment triggers:
  - round 0: adapter emission (NodesAdded, EdgesAdded)
  - round 1: co-occurrence enrichment (EdgesAdded)
  - round 2: lens enrichment (EdgesAdded)
**When** all rounds complete
**Then** the events table contains entries from all three rounds
**And** the sequence numbers are ordered: round 0 events < round 1 events < round 2 events

### Scenario: Event log is consistent with graph state after commit
**Given** a context with cursor at sequence 5
**When** an ingest call succeeds and produces new events
**Then** the new events are visible in `changes_since(cursor: 5)`
**And** the nodes and edges referenced by the event IDs exist in the context

### Scenario: Event log survives persistence round-trip
**Given** a context with 10 persisted events
**When** the engine is stopped and restarted (context reloaded from SQLite)
**Then** `changes_since(cursor: 0)` returns all 10 events with original sequence numbers

### Scenario: Stale cursor returns an error
**Given** a context with count-based retention (keep last 100 events)
**And** 200 events have been written (sequences 1–200; sequences 1–100 are pruned)
**When** a consumer calls `changes_since(context_id, cursor: 50)`
**Then** the result is an error indicating the cursor is stale (sequence 50 is pruned)

### Scenario: changes_since with no new events returns empty result
**Given** a context with 5 events (sequences 1–5)
**When** a consumer calls `changes_since(context_id, cursor: 5)`
**Then** the result contains zero events
**And** `latest_sequence` is 5

---

## Feature: Integration Scenarios (Cross-ADR)

### Scenario: Lens-created edges appear in cursor event log
**Given** a context with a registered lens and an event cursor at sequence 0
**When** a consumer ingests data that triggers the lens to create `lens:trellis:thematic_connection` edges
**Then** `changes_since(cursor: 0)` includes `EdgesAdded` events
**And** those events reference the lens-created edge IDs
**And** filtering by `adapter_id: "lens:trellis:thematic_connection"` returns only the lens events

### Scenario: QueryFilter on lens-created edges that were discovered via cursor
**Given** a consumer that calls `changes_since(cursor: N)` and learns that new lens edges were created
**When** the consumer traverses from a node with `filter: { relationship_prefix: "lens:trellis:" }`
**Then** the traversal includes the newly created lens edges
**And** the traversal excludes non-lens edges

### Scenario: Full pull workflow — ingest, cursor, filtered query
**Given** an empty context with a trellis adapter (with lens) and a carrel adapter (without lens)
**When** trellis ingests fragments producing concept nodes and enrichment edges
**And** carrel ingests research producing concept nodes with overlapping concepts
**And** the enrichment loop creates `may_be_related` edges between overlapping concepts
**And** the trellis lens translates those into `lens:trellis:thematic_connection` edges
**Then** a trellis consumer calling `changes_since(cursor: 0, filter: { adapter_id: "lens:trellis:thematic_connection" })` receives only the lens translation events
**And** traversing from a trellis fragment with `filter: { relationship_prefix: "lens:trellis:" }` reveals the cross-domain connections in trellis vocabulary
