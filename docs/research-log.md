# Research Log: Plexus Public Surface

## Prior Research (Runtime Architecture)

See `docs/research/semantic/essays/08-runtime-architecture.md` for the previous research cycle. That work wired the adapter layer to PlexusEngine, persisted contributions in SQLite, scoped provenance to project contexts, and added automatic tag-to-concept bridging (ADRs 006–009). The engine is internally complete — 218 tests, zero failures. But the MCP layer has not been updated to expose these capabilities. External consumers (Trellis, Carrel) cannot use what was built.

*Archived research log: `docs/research/semantic/logs/08-runtime-architecture.md`*

---

