# Plexus

A knowledge-graph library that accumulates structure from domain data through
an adapter pipeline, attaches provenance to every emission, and surfaces
latent relationships via a small set of reactive enrichments.

For architectural context, stakeholder framing, and navigation to decisions
and scenarios, read [`docs/ORIENTATION.md`](docs/ORIENTATION.md) first —
it is the canonical entry point for both product and technical readers.

## What ships in the default Homebrew / CLI binary

The default binary (`brew install nrgforge/tap/plexus` or the shell installer
shipped by `cargo-dist`) is intentionally lean. It runs without any external
infrastructure and delivers a well-defined baseline:

**Active by default:**

- **CoOccurrenceEnrichment** — detects pairs of concept nodes that share
  tagged sources and emits `may_be_related` edges with per-source
  contribution tracking.
- **TemporalProximityEnrichment** — emits `temporal_proximity` edges between
  nodes created within a configurable time window (reads `created_at`
  property, ISO-8601 UTC).
- **Structural analysis for markdown** — extracts headings, section
  boundaries, and link targets via the built-in `MarkdownStructureModule`.

**Registered but idle in the default baseline:**

- **DiscoveryGapEnrichment** — fires only when some producer emits
  `similar_to` edges. The default binary ships no built-in `similar_to`
  producer, so DiscoveryGap stays quiet until one is activated (see
  [activating embedding](#activating-embedding-based-discovery) below).

**Not compiled into the default baseline:**

- **EmbeddingSimilarityEnrichment** — requires the `embeddings` feature
  flag (ONNX Runtime + sqlite-vec + in-process `FastEmbedEmbedder`).
  Excluded from the default binary to keep the distribution lean; two
  activation paths exist (see below).

The baseline is a correct, useful end-state, not a deferred feature.
Consumers whose content flow does not need embedding-based discovery
skip activation entirely.

## What the default baseline does **not** deliver

Three capabilities are absent by design in the default binary. Name them
explicitly so expectations match behavior:

- **No in-process embedding.** No `similar_to` edges emerge from the
  adapter pipeline alone.
- **No DiscoveryGap signal on embedding.** DiscoveryGap is registered
  but has no trigger until a consumer activates embedding.
- **No semantic extraction over unstructured prose.** Built-in semantic
  extraction via the `extract-file` route requires `llm-orc` running with
  a configured LLM provider. Without it, `extract-file` ingest completes
  registration and structural analysis, and skips semantic extraction
  gracefully (Invariant 47).

See [ADR-038](docs/decisions/038-release-binary-feature-profile.md) for
the release-binary decision rationale and the activation-path reasoning.

## Activating embedding-based discovery

Two paths, chosen by deployment shape:

### Path 1 — In-process embedding (library consumers)

For library consumers whose end-users cannot install `llm-orc` (e.g. a
desktop application shipping Plexus as a dependency). Adds `fastembed-rs`
and `sqlite-vec` to the binary.

```toml
[dependencies]
plexus = { version = "0.2", features = ["embeddings"] }
```

Trade-off: larger binary (ONNX Runtime adds ~30 MB), model download on
first use (~100 MB for the default embedding model). In return,
`EmbeddingSimilarityEnrichment` is registered in the default enrichment
set and produces `similar_to` edges automatically over ingested content.

### Path 2 — Consumer-declared external enrichment (default binary)

For CLI, server, or developer installs where `llm-orc` and an embedding
provider can be installed alongside. The default Plexus binary is
unchanged; embedding runs out-of-process through llm-orc and re-enters
the graph as edges via `ingest()`.

Prerequisites:

1. [`llm-orc`](https://github.com/mrilikecoding/llm-orc) on `PATH`.
2. An embedding provider configured through llm-orc — typically
   [Ollama](https://ollama.com) running locally with an embedding model
   pulled (`ollama pull nomic-embed-text`), or an OpenAI-compatible
   endpoint. Plexus is indifferent to which provider llm-orc calls.
3. A declarative adapter spec declaring an embedding-producing external
   enrichment.

A worked example spec ships at
[`examples/specs/embedding-activation.yaml`](examples/specs/embedding-activation.yaml).
It declares an llm-orc ensemble that computes pairwise cosine similarity
over a batch of documents and emits `similar_to` edges above a
configurable threshold. Companion fixture prose lives at
[`test-corpora/collective-intelligence/`](test-corpora/collective-intelligence/)
and [`test-corpora/public-domain-stories/`](test-corpora/public-domain-stories/)
for reproducible end-to-end validation.

Activation flow in brief:

```bash
# Pull the embedding model (or use any other ollama-compatible model)
ollama pull nomic-embed-text

# Load the spec into a Plexus context
plexus mcp --db ~/plexus.db  # (via your MCP client's load_spec tool)
# → load_spec with file contents of examples/specs/embedding-activation.yaml

# Ingest a batch of documents; similar_to edges emerge as the ensemble
# computes embeddings and the spec's emit primitives materialize edges.
```

See the spec file's header comment for the full activation workflow and
the fixture corpora's `README.md` files for the selection rationale and
empirical evidence that the demonstration crosses the tautology threshold
(emergent `similar_to` edges reflect semantic structure, not pre-encoded
tag overlap).

## Key capabilities

- **Multi-dimensional graph** — nodes span structure, semantic,
  relational, temporal, and provenance dimensions; dimension is an
  extensible string facet chosen by the consumer ([ADR-042](docs/decisions/042-dimension-extensibility-guidance.md)).
- **Adapter pipeline** — adapters extract domain data into graph
  mutations; structural-analysis modules dispatch by MIME type; external
  enrichments re-enter the graph via `ingest()` and trigger the core
  enrichment loop on the new data.
- **Provenance tracking** — chains, marks, and `contains` edges are
  created automatically alongside semantic content, satisfying the dual
  obligation (Invariant 7).
- **Self-reinforcing edges** — edges accumulate per-adapter contributions;
  raw weights are computed from contributions via scale normalization;
  normalized weights are computed at query time (ADR-003).
- **Lens-based domain translation** — consumers declare translation rules
  in their adapter spec; raw relationships get translated into
  consumer-vocabulary edges queryable via relationship-prefix filter
  ([ADR-033](docs/decisions/033-lens-declaration.md), [ADR-041](docs/decisions/041-lens-grammar-conventions.md)).
- **Composable queries** — `find_nodes`, `traverse`, `find_path`, and
  `evidence_trail` accept a shared `QueryFilter` (contributor IDs,
  relationship prefix, corroboration minimum) plus optional ranking
  ([ADR-034](docs/decisions/034-composable-query-filters.md)).
- **Event cursors** — pull-based delivery of a persistent change log;
  consumers can walk away and resume with `changes_since(cursor)`
  ([ADR-035](docs/decisions/035-event-cursor-persistence.md)).
- **SQLite persistence** — WAL mode for concurrent reads, event-log
  persistence, spec-table durability.
- **MCP server** — 17 tools expose the library to AI assistants over
  stdio: session (1), ingest (1), context operations (6), graph reads
  (7), spec lifecycle (2).

## Library usage

```rust
use plexus::{PlexusEngine, PlexusApi, OpenStore, SqliteStore};
use plexus::adapter::PipelineBuilder;
use std::sync::Arc;

let store = SqliteStore::open("my-project.db")?;
let engine = Arc::new(PlexusEngine::with_store(Arc::new(store)));
engine.load_all()?;

let pipeline = PipelineBuilder::default_pipeline(engine.clone());
let api = PlexusApi::new(engine.clone(), Arc::new(pipeline));

// All writes go through ingest() — Invariant 34.
api.ingest("my-context", "content", content_input).await?;

// Optionally load a declarative adapter spec to extend the ingest
// pipeline with a consumer-specific adapter, lens, or enrichment config.
api.load_spec("my-context", &spec_yaml).await?;
```

## MCP server usage

```bash
cargo build --bin plexus
plexus mcp                        # stdio transport, default XDG path
plexus mcp --db /path/to/data.db  # custom database path
```

Default database location: `~/.local/share/plexus/plexus.db` (Linux) or
`~/Library/Application Support/plexus/plexus.db` (macOS), following XDG
Base Directory conventions.

Configure in your MCP client (e.g. Claude Code `settings.json`):

```json
{
  "mcpServers": {
    "plexus": {
      "command": "plexus",
      "args": ["mcp"]
    }
  }
}
```

### MCP tools (17)

**Session (1):** `set_context`

**Write (1):** `ingest` — single write path through the full adapter
pipeline (Invariant 34)

**Context operations (6):** `context_list`, `context_create`,
`context_delete`, `context_rename`, `context_add_sources`,
`context_remove_sources`

**Graph reads (7):** `find_nodes`, `traverse`, `find_path`,
`evidence_trail`, `shared_concepts`, `list_tags`, `changes_since`

**Spec lifecycle (2):** `load_spec`, `unload_spec`

All writes go through `ingest` → adapter pipeline → enrichment loop.
There are no tools for direct graph-primitive manipulation.

## Development

```bash
cargo build              # Build library + binary
cargo test --lib         # Run library tests (fast, ~5s)
cargo test               # Run all tests (~10s, no external deps)

# Integration tests against real Ollama + llm-orc (gated):
PLEXUS_INTEGRATION=1 cargo test
```

### Project structure

```
plexus/
├── src/
│   ├── lib.rs              # Library entry + re-exports
│   ├── api.rs              # PlexusApi — transport-independent facade
│   ├── bin/plexus.rs       # CLI binary (plexus mcp, plexus context)
│   ├── graph/              # Core graph: Node, Edge, Context, Engine
│   ├── adapter/            # Adapter pipeline + enrichments
│   │   ├── adapters/       # Built-in adapters (content, extraction, declarative, graph_analysis)
│   │   ├── enrichments/    # Core enrichments (cooccurrence, discovery_gap, temporal_proximity, embedding_similarity, lens)
│   │   ├── pipeline/       # IngestPipeline + PipelineBuilder
│   │   └── sink/           # EngineSink — commit + persist
│   ├── provenance/         # Provenance API (chains, marks, links)
│   ├── mcp/                # MCP server (rmcp, 17 tools)
│   ├── query/              # Find, traverse, path, filter, cursor
│   ├── llm_orc.rs          # SubprocessClient + MockClient for llm-orc
│   └── storage/            # SQLite persistence
├── tests/acceptance/       # End-to-end behavior tests
├── examples/specs/         # Worked-example declarative adapter specs
├── test-corpora/           # Reproducible fixture corpora
└── docs/                   # ORIENTATION, system design, ADRs, scenarios
```

## Documentation

- [`docs/ORIENTATION.md`](docs/ORIENTATION.md) — entry point, answers "what
  is this system, who serves whom, what's the current state"
- [`docs/system-design.md`](docs/system-design.md) — module decomposition,
  responsibility allocation, dependency graph, fitness criteria
- [`docs/product-discovery.md`](docs/product-discovery.md) — stakeholders,
  jobs, value tensions
- [`docs/decisions/`](docs/decisions/) — ADRs (43 at last count)
- [`docs/scenarios/`](docs/scenarios/) — refutable behavior scenarios
- [`docs/essays/`](docs/essays/) — research essays on subsystem design
- [`docs/references/field-guide.md`](docs/references/field-guide.md) —
  module-to-code mapping for navigation

## License

AGPL-3.0 — see [LICENSE](LICENSE).
