# Plexus

Network-aware knowledge graph engine with self-reinforcing edges.

Plexus accumulates knowledge from domain data through an adapter pipeline: adapters extract nodes and edges, enrichments bridge dimensions automatically, and contribution tracking strengthens edges through repeated co-occurrence — a network that learns from its own structure.

## Key Capabilities

- **Multi-dimensional graph** — nodes and edges span structure, semantic, relational, temporal, and provenance dimensions
- **Adapter pipeline** — adapters extract domain data into graph mutations; enrichments (TagConceptBridger, CoOccurrenceEnrichment) react and bridge dimensions automatically
- **Self-reinforcing edges** — relationship weights evolve based on activation patterns (Hebbian learning)
- **Provenance tracking** — chains, marks, and links for recording decisions and code exploration trails
- **Evidence trails** — "what evidence supports this concept?" answered in a single query across all dimensions
- **Source manifest** — contexts track which files, directories, and URLs belong to them
- **MCP server** — expose capabilities to AI tools via the Model Context Protocol (14 tools)
- **SQLite persistence** — WAL mode for concurrent reads, incremental upserts, cache coherence via `data_version`
- **Cross-context queries** — discover shared concepts between contexts via deterministic ID intersection

## Usage

### As a library

Add to your `Cargo.toml`:

```toml
[dependencies]
plexus = { git = "https://github.com/mrilikecoding/plexus.git", branch = "main" }
```

```rust
use plexus::{PlexusEngine, Context, Source, OpenStore, SqliteStore};
use std::sync::Arc;

// Open with SQLite persistence
let store = SqliteStore::open("my-project.db").unwrap();
let engine = PlexusEngine::with_store(Arc::new(store));
engine.load_all().unwrap();

// Create a context with sources
let ctx = Context::new("my-project");
let ctx_id = engine.upsert_context(ctx).unwrap();
engine.add_source(&ctx_id, Source::Directory {
    path: "/path/to/project".into(),
    recursive: false,
}).unwrap();
```

See the [Integration Guide](docs/integration-guide.md) for adapter pipeline usage and custom adapter development.

### As an MCP server

```bash
# Build and run
cargo build --bin plexus
plexus mcp                        # stdio transport, default XDG path
plexus mcp --db /path/to/data.db  # custom database path
```

Default database location: `~/.local/share/plexus/plexus.db` (Linux) or `~/Library/Application Support/plexus/plexus.db` (macOS), following XDG Base Directory conventions.

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

### MCP Tools (14)

**Session:** `set_context` — set active context (auto-created if new)

**Provenance — chains (4):** `list_chains`, `get_chain`, `archive_chain`, `delete_chain`

**Provenance — marks (5):** `annotate`, `update_mark`, `delete_mark`, `list_marks`, `list_tags`

**Provenance — links (3):** `link_marks`, `unlink_marks`, `get_links`

**Queries (1):** `evidence_trail` — marks, fragments, and chains supporting a concept

Context management (create, delete, rename, add/remove sources) is via the CLI: `plexus context <subcommand>`.

## Development

```bash
cargo build              # Build library + binary
cargo build --bin plexus # Build CLI only
cargo test --lib         # Run library tests (fast)
cargo test               # Run all tests (includes integration)
```

### Feature Flags

- `real_llm` — enable real LLM calls for integration tests (default: mock responses)

### CLI

```bash
plexus context create my-project       # create a context
plexus context add-source my-project /path/to/src  # add a source
plexus context list                    # list all contexts
plexus context rename old-name new-name
plexus context remove-source my-project /path/to/src
plexus context delete my-project
```

### Project Structure

```
plexus/
├── src/
│   ├── lib.rs              # Library entry point and re-exports
│   ├── api.rs              # PlexusApi — transport-independent facade
│   ├── bin/plexus.rs       # CLI binary (plexus mcp, plexus context)
│   ├── graph/              # Core graph: Node, Edge, Context, Engine
│   ├── adapter/            # Adapter pipeline, enrichments, FragmentAdapter
│   ├── provenance/         # Provenance API (chains, marks, links)
│   ├── mcp/                # MCP server (rmcp, 14 tools)
│   ├── analysis/           # Content analysis pipeline
│   ├── query/              # Find, traverse, path queries
│   └── storage/            # SQLite persistence (WAL, incremental upserts)
├── tests/                  # Integration tests
├── docs/                   # ADRs, essays, domain model, scenarios
└── Cargo.toml
```

## Research

See [`docs/essays/`](docs/essays/) for research essays and [`docs/adr/`](docs/adr/) for architectural decisions.

See the [Integration Guide](docs/integration-guide.md) for adapter pipeline architecture, writing custom adapters, and application integration patterns.

## License

MIT
