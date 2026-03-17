# ADR-002: Plexus Storage Location

**Status:** Superseded by ADR-016

**Date:** 2026-02-05

---

## Context

The Plexus MCP server currently stores its SQLite database (`.plexus.db`) in the project working directory. This file contains the graph engine's runtime state — contexts, nodes, and edges. It is not project source and should not be committed.

The current approach works but raises questions:

- **Working directory pollution.** The DB sits alongside source files, requiring gitignore entries.
- **Project identity.** If storage moves to a central location (e.g., `~/.local/share/plexus/`), the server needs a scheme to map a project to its database — path hash, explicit config, or project name.
- **Portability vs. cleanliness.** Project-local storage is self-contained and trivially discoverable. Centralized storage is cleaner but introduces indirection.
- **Multi-context model.** A single DB holds multiple contexts. Should contexts map 1:1 to projects, or can a project span multiple DBs?

## Decision

Deferred. Needs research spike to evaluate:

1. What project identity scheme works across machines and directory renames?
2. What do other MCP servers and local-first tools (clawmarks, sqlite-based CLIs) do?
3. XDG Base Directory conventions (`~/.local/share/` vs `~/.config/` vs project-local)
4. Whether the answer differs for the MCP server (tooling) vs the Plexus engine (library)

## Update (2026-02-10)

ADR-006 through ADR-009 resolve the multi-context question from the context section above: a single PlexusEngine holds multiple contexts (e.g., `trellis`, `desk`, `carrel-research`) in one DashMap, backed by one GraphStore/database. Multiple tools share one DB. The storage location spike should account for this model and for the richer data now being persisted (adapter-produced knowledge graphs with per-adapter contributions, not just provenance marks).

## Consequences

Until resolved, `.plexus.db` lives in the project directory and is globally gitignored.
