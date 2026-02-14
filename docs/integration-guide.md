# Plexus Integration Guide

How to integrate an application with Plexus — the knowledge graph engine that accumulates, connects, and surfaces knowledge across domains.

---

## What Plexus Does for You

Your application has domain data — text fragments, research notes, code annotations, creative artifacts. Plexus turns that data into a queryable knowledge graph with three capabilities:

1. **Semantic extraction** — tags and concepts become graph nodes, connected by typed edges with contribution tracking
2. **Cross-dimensional bridging** — provenance marks, semantic concepts, and structural fragments automatically connect when they share tags
3. **Evidence trails** — "what evidence supports this concept?" answered in a single query across all dimensions

You don't manage the graph directly. You send data in through an adapter; the engine handles extraction, enrichment, and persistence.

---

## Architecture

```
Your Application
  │
  ├── Rust embedding: call PlexusApi directly
  ├── MCP: via Claude Code / LLM host (current transport)
  └── Future: gRPC, REST, WebSocket
       │
       ▼
  PlexusApi (transport-independent)
       │
       ├── ingest(context, input_kind, data) ──► IngestPipeline
       │                                            │
       │                                    ┌───────┴───────┐
       │                                    ▼               ▼
       │                              Your Adapter    ProvenanceAdapter
       │                                    │               │
       │                                    ▼               ▼
       │                              sink.emit(Emission)
       │                                    │
       │                                    ▼
       │                            Enrichment Loop
       │                            (TagConceptBridger,
       │                             CoOccurrenceEnrichment)
       │                                    │
       │                                    ▼
       │                           OutboundEvents ──► Your Application
       │
       ├── evidence_trail(context, node_id)
       ├── find_nodes(context, query)
       ├── list_chains(context, ...)
       └── ... (reads bypass pipeline)
```

---

## Three Integration Patterns

### Pattern 1: Provenance only (annotations via MCP)

**Use when:** Your application is an LLM host (Claude Code, IDE plugin) that annotates code or documents. You don't have domain-specific data to ingest — you're creating marks, chains, and links.

**What you get:** Provenance tracking with automatic tag-to-concept bridging. If a concept node for "refactor" exists in the context (from any adapter), tagging a mark with "refactor" creates a cross-dimensional `references` edge automatically.

**How:**
```
set_context("my-project")
annotate({ chain_name: "review-notes", file: "src/auth.rs", line: 42,
           annotation: "security concern", tags: ["auth", "security"] })
```

**Adapter needed:** None. `ProvenanceAdapter` is built in and registered by default.

---

### Pattern 2: Tagged text fragments (via existing FragmentAdapter)

**Use when:** Your application produces tagged text — journal entries, research notes, writing fragments, chat messages. Each piece has text content and a set of tags/labels.

**What you get:** Fragment nodes in the structure dimension, concept nodes in the semantic dimension, `tagged_with` edges connecting them. Co-occurrence enrichment proposes `may_be_related` edges between concepts that appear together across fragments. Contribution tracking accumulates evidence.

**How (Rust embedding):**
```rust
let input = FragmentInput::new(
    "Walked through the old quarter of Avignon",
    vec!["travel".into(), "avignon".into(), "provence".into()],
)
.with_source("journal/2026-02-13.md");

let events = api.ingest("research", "fragment", Box::new(input)).await?;
```

**How (MCP):** Not yet surfaced as an MCP tool. The `FragmentAdapter` exists and works; an `ingest_fragment` MCP tool is a single-function addition when needed.

**Adapter needed:** None. `FragmentAdapter` is built in.

---

### Pattern 3: Custom domain data (write a new adapter)

**Use when:** Your data doesn't fit the fragment model. You need domain-specific extraction logic — your own node types, edge relationships, and semantic structure.

**What you get:** Full control over what nodes and edges your data produces, with automatic enrichment, contribution tracking, and provenance — all for free from the pipeline.

**Examples of when you need a custom adapter:**

| Application | Why FragmentAdapter doesn't fit |
|---|---|
| **Carrel** (research desk) | Ingests Semantic Scholar papers with structured metadata (authors, citations, venues). Needs `cited_by` edges, author nodes, venue nodes — not just tags. |
| **Sketchbin** (creative workshop) | Ingests creative artifacts (audio, code, visual) with modality-specific metadata. Needs `modality` dimension, creator provenance, federation trust edges. |
| **Manza** (visualizer) | Doesn't ingest — it's read-only. No adapter needed. Uses `evidence_trail`, `find_nodes`, `traverse` queries. |
| **EDDI** (movement analysis) | Ingests Laban movement encodings. Needs effort-shape nodes, movement quality edges, temporal dimension. Entirely different extraction logic. |

**How:** See [Writing an Adapter](#writing-an-adapter) below.

---

## Decision Framework: Do I Need a New Adapter?

```
Does my application produce data for Plexus?
├── No (read-only consumer) → No adapter. Use PlexusApi reads.
└── Yes
    ├── Is it provenance annotations (marks, chains, links)?
    │   └── Yes → Use ProvenanceAdapter via annotate()
    └── Is it tagged text with no structural metadata beyond tags?
        ├── Yes → Use FragmentAdapter via ingest("fragment", ...)
        └── No → Write a new adapter
```

Signs you need a custom adapter:
- Your data has **relational structure** beyond tags (citations, authorship, dependencies)
- You need **custom node types** (paper, author, sketch, movement-phrase)
- You need **custom edge relationships** (cited_by, created_by, responds_to)
- You need **domain-specific extraction logic** (parse BibTeX, decode movement notation)
- Your **provenance model** is richer than file+line+annotation (e.g., federation source, API response metadata)

Signs you don't:
- Your data is text with tags → `FragmentAdapter`
- Your data is annotations on code/files → `ProvenanceAdapter` via `annotate()`
- You only read the graph → no adapter at all

---

## Writing an Adapter

### Step 1: Define your input type

Your input type is a plain Rust struct. It carries whatever your application sends.

```rust
/// Input from a research paper ingestion pipeline.
#[derive(Debug, Clone)]
pub struct PaperInput {
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: String,
    pub tags: Vec<String>,
    pub doi: Option<String>,
    pub venue: Option<String>,
    pub year: Option<u32>,
}
```

### Step 2: Implement the Adapter trait

```rust
use plexus::adapter::{
    Adapter, AdapterError, AdapterInput, AdapterSink,
    Emission, AnnotatedNode, AnnotatedEdge, OutboundEvent,
};
use plexus::graph::{Node, Edge, NodeId, ContentType, dimension};

pub struct PaperAdapter;

#[async_trait]
impl Adapter for PaperAdapter {
    fn id(&self) -> &str { "paper" }
    fn input_kind(&self) -> &str { "paper" }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let paper = input.downcast_data::<PaperInput>()
            .ok_or(AdapterError::InvalidInput)?;

        let mut emission = Emission::new();

        // 1. Create paper node (structure dimension)
        let paper_id = format!("paper:{}", paper.doi.as_deref()
            .unwrap_or(&slug(&paper.title)));
        let mut paper_node = Node::new_in_dimension(
            "paper", ContentType::Document, dimension::STRUCTURE,
        );
        paper_node.id = NodeId::from(paper_id.as_str());
        paper_node.set_content(paper.abstract_text.clone());
        paper_node.properties.insert(
            "title".into(),
            PropertyValue::String(paper.title.clone()),
        );
        emission = emission.with_node(AnnotatedNode::new(paper_node));

        // 2. Create concept nodes from tags (semantic dimension)
        for tag in &paper.tags {
            let concept_id = format!("concept:{}", tag.to_lowercase());
            let mut concept = Node::new_in_dimension(
                "concept", ContentType::Concept, dimension::SEMANTIC,
            );
            concept.id = NodeId::from(concept_id.as_str());
            concept.properties.insert(
                "name".into(),
                PropertyValue::String(tag.clone()),
            );
            emission = emission.with_node(AnnotatedNode::new(concept));

            // 3. tagged_with edge (paper → concept, cross-dimensional)
            let edge = Edge::new_cross_dimensional(
                NodeId::from(paper_id.as_str()), dimension::STRUCTURE,
                NodeId::from(concept_id.as_str()), dimension::SEMANTIC,
                "tagged_with",
            );
            emission = emission.with_edge(
                AnnotatedEdge::new(edge).with_annotation(
                    Annotation::new().with_confidence(1.0)
                )
            );
        }

        // 4. Emit — the engine validates, commits, and tracks contributions
        sink.emit(emission).await?;
        Ok(())
    }

    fn transform_events(
        &self,
        events: &[GraphEvent],
        _context: &Context,
    ) -> Vec<OutboundEvent> {
        let mut out = vec![];
        for event in events {
            if let GraphEvent::NodesAdded { node_ids, .. } = event {
                let concepts: Vec<_> = node_ids.iter()
                    .filter(|id| id.as_str().starts_with("concept:"))
                    .map(|id| id.as_str().trim_start_matches("concept:"))
                    .collect();
                if !concepts.is_empty() {
                    out.push(OutboundEvent::new(
                        "concepts_detected",
                        concepts.join(", "),
                    ));
                }
            }
        }
        out
    }
}
```

### Step 3: Register with the pipeline

```rust
let engine = Arc::new(PlexusEngine::with_store(store));
let mut pipeline = IngestPipeline::new(engine.clone());

// Register your adapter with enrichments
pipeline.register_integration(
    Arc::new(PaperAdapter),
    vec![
        Arc::new(TagConceptBridger::new()),       // bridges tags ↔ concepts
        Arc::new(CoOccurrenceEnrichment::new()),  // detects concept co-occurrence
    ],
);

let api = PlexusApi::new(engine, Arc::new(pipeline));
```

### Step 4: Ingest data

```rust
let paper = PaperInput {
    title: "Self-Reinforcing Knowledge Graphs".into(),
    authors: vec!["Green, N.".into()],
    abstract_text: "We present a knowledge graph that...".into(),
    tags: vec!["knowledge-graphs".into(), "hebbian".into()],
    doi: Some("10.1234/example".into()),
    venue: Some("SIGMOD".into()),
    year: Some(2026),
};

let events = api.ingest("research", "paper", Box::new(paper)).await?;
// events: [OutboundEvent { kind: "concepts_detected", detail: "knowledge-graphs, hebbian" }]
```

---

## What the Pipeline Gives You for Free

When you write an adapter, the pipeline handles:

| Capability | What it does | You don't need to... |
|---|---|---|
| **Contribution tracking** | Each edge records which adapter contributed what value | ...manage edge weights manually |
| **Scale normalization** | Cross-adapter edge weights are normalized to comparable ranges | ...worry about one adapter's values dominating another's |
| **Enrichment loop** | TagConceptBridger and CoOccurrenceEnrichment run automatically after your emission | ...write cross-dimensional bridging logic |
| **Provenance** | Framework records adapter ID, timestamp, context ID on every emission | ...track where graph data came from |
| **Persistence** | Engine persists mutations to SQLite after each emission | ...manage storage |
| **Idempotency** | Nodes with the same ID are upserted, not duplicated | ...check for duplicates |
| **Validation** | Edges with missing endpoints are rejected (not failed — other items commit) | ...pre-validate your emission |
| **Outbound events** | Your `transform_events()` translates graph events to domain vocabulary | ...poll the graph for changes |

---

## Enrichments

Enrichments are reactive graph intelligence — they observe emissions and produce additional mutations. Two are built in:

**TagConceptBridger** — when a mark is tagged "travel" and a concept node `concept:travel` exists in the same context, creates a cross-dimensional `references` edge. Works both directions: new mark tagged with existing concept, or new concept matching existing mark's tag.

**CoOccurrenceEnrichment** — when concepts co-occur across fragments (two tags on the same fragment, or tags that appear together frequently), proposes `may_be_related` edges between those concepts. Hebbian: repeated co-occurrence strengthens the connection.

### Writing a custom enrichment

```rust
use plexus::adapter::{Enrichment, Emission, GraphEvent};
use plexus::graph::Context;

pub struct CitationEnrichment;

impl Enrichment for CitationEnrichment {
    fn id(&self) -> &str { "citation-bridger" }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        // React to new paper nodes, check for citation relationships,
        // emit cited_by edges. Return None when quiescent.
        None
    }
}
```

Register enrichments with your adapter's integration:

```rust
pipeline.register_integration(
    Arc::new(PaperAdapter),
    vec![
        Arc::new(TagConceptBridger::new()),
        Arc::new(CoOccurrenceEnrichment::new()),
        Arc::new(CitationEnrichment),
    ],
);
```

Enrichments are deduplicated by `id()` across integrations — if two integrations both register `TagConceptBridger`, it runs once per round.

---

## Reads and Queries

All reads go through `PlexusApi`. No adapter needed.

| Method | What it returns | Use case |
|---|---|---|
| `evidence_trail(ctx, node_id)` | Marks, fragments, chains, and edges for a concept | "What evidence supports this idea?" |
| `find_nodes(ctx, query)` | Nodes matching type/content/dimension/property filters | "What concepts exist?" |
| `traverse(ctx, query)` | BFS from a start node, single relationship filter | "What's near this node?" |
| `find_path(ctx, query)` | Shortest path between two known nodes | "How are these connected?" |
| `list_chains(ctx, status?)` | Annotation chains | "What research threads exist?" |
| `get_chain(ctx, chain_id)` | Chain with all its marks | "Show me this thread" |
| `list_marks(ctx, filters)` | Marks by chain/file/type/tag | "What did I annotate in this file?" |
| `list_tags(ctx)` | All tags in use | "What concepts have been tagged?" |
| `get_links(ctx, mark_id)` | A mark's incoming and outgoing links | "What connects to this annotation?" |

---

## Adapter Design Principles

1. **Deterministic IDs.** Use `{type}:{source}:{identifier}` format. Same input → same node ID → upsert, not duplicate. Examples: `paper:doi:10.1234/x`, `concept:travel`, `chain:provenance:field-notes`.

2. **Emit everything in one call.** A single `sink.emit(emission)` with all nodes and edges for one input. The engine validates references within the emission — edges can reference nodes from the same emission.

3. **Cross-dimensional edges are just edges.** Use `Edge::new_cross_dimensional()` when source and target are in different dimensions. The enrichment loop handles dimension-aware bridging; you just emit the edges that make sense for your domain.

4. **Let enrichments handle bridging.** If your adapter creates concept nodes from tags, `TagConceptBridger` will connect existing marks to those concepts. Don't duplicate that logic in your adapter.

5. **Outbound events are your contract with the consumer.** Define `kind` values that your consumer cares about. The consumer never sees raw `GraphEvent` — only your translated `OutboundEvent` values.

6. **Adapters don't read the graph.** `process()` receives input and emits mutations. It doesn't query the existing graph state. If you need conditional logic based on existing state, that's an enrichment (which receives a context snapshot).

---

## Transport: Connecting Your Application

Currently Plexus has one transport: **MCP** (Model Context Protocol), used by Claude Code and other LLM hosts.

For non-MCP applications, the options today:

| Option | Effort | Best for |
|---|---|---|
| **Rust embedding** | Lowest | Rust applications, CLI tools, same-process integration |
| **CLI wrapper** | Low | Scripts, batch processing (`plexus ingest --context X --kind fragment --file data.json`) |
| **REST server** | Medium | Python/JS/other-language clients (thin HTTP wrapper around PlexusApi) |
| **gRPC** | Higher | Production inter-service communication (protobuf schema needed) |

All transports call the same `PlexusApi` methods. The transport is a thin shell — no domain logic.

---

## Quick Reference: What Each Application Needs

| Application | Pattern | Adapter | Enrichments | Transport |
|---|---|---|---|---|
| **Trellis** | Tagged fragments | FragmentAdapter (built in) | TagConceptBridger, CoOccurrence | REST or gRPC |
| **Carrel** | Custom (papers + annotations) | PaperAdapter (new) + ProvenanceAdapter | TagConceptBridger, CoOccurrence, CitationBridger (new) | Rust embedding |
| **EDDI** | Custom (movement data) | MovementAdapter (new) | EffortShapeBridger (new) | Rust embedding |
| **Manza** | Read-only | None | N/A | gRPC or WebSocket |
| **Sketchbin** | Custom (creative artifacts) | SketchAdapter (new), FederationAdapter (new) | TagConceptBridger, CoOccurrence | REST + ActivityPub |
| **Claude Code** | Provenance | ProvenanceAdapter (built in) | TagConceptBridger, CoOccurrence | MCP (current) |
