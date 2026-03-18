# Research Log: Plexus

## Prior Research (Public Surface)

See `docs/../../essays/09-public-surface.md` for the research and `docs/../../essays/10-building-the-public-surface.md` for the build results. That work implemented ADRs 010–012: enrichment trait and loop, bidirectional adapter, and unified ingest pipeline. All provenance write operations now route through the ingest pipeline. The MCP layer is a thin transport. 245 tests, zero failures.

*Archived research log: `docs/research/logs/09-public-surface.md`*

---

## What's Next

The pipeline is built and tested with synthetic data. The next step is feeding real data through it. Candidate experiments:

1. **Trellis fragment corpus** — ingest tagged text fragments via FragmentAdapter, observe concept graph, co-occurrence proposals, and tag-to-concept bridging. Needs a data ingestion tool (CLI or REST endpoint).

2. **Carrel research workflow** — create provenance chains and marks via MCP, ingest related literature fragments, observe cross-dimensional bridges forming automatically.

3. **Mixed workload** — combine both: ingest fragments, create marks, and see how the graph connects provenance to semantics without manual wiring.

### Open questions for next cycle

- **OQ8: Event persistence and cursor-based delivery** — async event delivery for consumers not present during ingest
- **OQ9: Wire protocol** — gRPC or REST for non-MCP consumers (Trellis, Carrel)
- **Transport for sample data** — CLI tool, REST endpoint, or test harness for feeding real data through the pipeline
