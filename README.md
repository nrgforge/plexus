# Plexus

Network-aware knowledge graph engine with self-reinforcing edges.

Plexus builds semantic knowledge graphs from document collections using tree-structured traversal and LLM-based extraction. It models relationships between concepts with edges that strengthen through repeated co-occurrence — a network that learns from its own structure.

## Key Capabilities

- **Tree-first traversal** — uses file hierarchy as primary semantic signal (9.3x stronger than explicit links)
- **LLM semantic extraction** — compositional chunk-aggregate-synthesize pipeline with 0% hallucination on technical corpora
- **Self-reinforcing edges** — relationship weights evolve based on activation patterns
- **SQLite persistence** — lightweight, embedded storage with full graph serialization

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
plexus = { git = "https://github.com/mrilikecoding/plexus.git", branch = "main" }
```

## Development

```bash
cargo build
cargo test
```

### Feature Flags

- `real_llm` — enable real LLM calls for integration tests (default: mock responses)

## Research

See [`docs/semantic/`](docs/semantic/) for the full research trail, including:

- [PAPER.md](docs/semantic/PAPER.md) — journal-ready findings
- [EXPERIMENT-LOG.md](docs/semantic/EXPERIMENT-LOG.md) — raw experiment data
- [SYSTEM-DESIGN.md](docs/semantic/SYSTEM-DESIGN.md) — architecture specification

## License

MIT
