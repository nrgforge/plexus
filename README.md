# Plexus

Network-aware knowledge graph engine with self-reinforcing edges.

Plexus builds semantic knowledge graphs from document collections using tree-structured traversal and LLM-based extraction. It models relationships between concepts with edges that strengthen through repeated co-occurrence — a network that learns from its own structure.

## Key Capabilities

- **Multi-dimensional graph** — nodes and edges span structure, semantic, relational, temporal, and provenance dimensions
- **Self-reinforcing edges** — relationship weights evolve based on activation patterns (Hebbian learning)
- **Provenance tracking** — chains, marks, and links for recording decisions and code exploration trails
- **Source manifest** — contexts track which files, directories, and URLs belong to them
- **MCP server** — expose all capabilities to AI tools via the Model Context Protocol (19 tools)
- **SQLite persistence** — lightweight, embedded storage with full graph serialization
- **LLM semantic extraction** — compositional chunk-aggregate-synthesize pipeline

## Usage

### As a library

Add to your `Cargo.toml`:

```toml
[dependencies]
plexus = { git = "https://github.com/mrilikecoding/plexus.git", branch = "main" }
```

```rust
use plexus::{PlexusEngine, Context, Source, ProvenanceApi};

let engine = PlexusEngine::new();

// Create a context with sources
let ctx = Context::new("my-project");
let ctx_id = engine.upsert_context(ctx).unwrap();
engine.add_source(&ctx_id, Source::Directory {
    path: "/path/to/project".into(),
    recursive: true,
}).unwrap();

// Use provenance tracking
let prov = ProvenanceApi::new(&engine, ctx_id);
let chain_id = prov.create_chain("refactor-auth", Some("Auth system redesign")).unwrap();
let mark_id = prov.add_mark(&chain_id, "src/auth.rs", 42, "Core auth logic", None, Some("decision"), None).unwrap();
```

### As an MCP server

```bash
# Build and run
cargo build --bin plexus
plexus mcp                        # stdio transport, .plexus.db in cwd
plexus mcp --db /path/to/data.db  # custom database path
```

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

### MCP Tools (19)

**Context management (6):** `context_create`, `context_delete`, `context_add_sources`, `context_remove_sources`, `context_list`, `context_rename`

**Provenance — chains (5):** `create_chain`, `list_chains`, `get_chain`, `archive_chain`, `delete_chain`

**Provenance — marks (5):** `add_mark`, `update_mark`, `delete_mark`, `list_marks`, `list_tags`

**Provenance — links (3):** `link_marks`, `unlink_marks`, `get_links`

## Development

```bash
cargo build              # Build library + binary
cargo build --bin plexus # Build CLI only
cargo test --lib         # Run library tests (fast)
cargo test               # Run all tests (includes integration)
```

### Feature Flags

- `real_llm` — enable real LLM calls for integration tests (default: mock responses)

### Project Structure

```
plexus/
├── src/
│   ├── lib.rs              # Library entry point and re-exports
│   ├── bin/plexus.rs       # CLI binary (plexus mcp)
│   ├── graph/              # Core graph: Node, Edge, Context, Engine
│   ├── provenance/         # Provenance API (chains, marks, links)
│   ├── mcp/                # MCP server (rmcp, 19 tools)
│   ├── analysis/           # Content analysis pipeline
│   ├── query/              # Find, traverse, path queries
│   └── storage/            # SQLite persistence
├── tests/                  # Integration/spike tests
└── Cargo.toml
```

## Research

See [`docs/semantic/`](docs/semantic/) for the full research trail, including:

- [PAPER.md](docs/semantic/PAPER.md) — journal-ready findings
- [EXPERIMENT-LOG.md](docs/semantic/EXPERIMENT-LOG.md) — raw experiment data
- [SYSTEM-DESIGN.md](docs/semantic/SYSTEM-DESIGN.md) — architecture specification

## License

MIT
