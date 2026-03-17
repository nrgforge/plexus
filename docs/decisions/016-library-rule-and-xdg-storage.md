# ADR-016: The Library Rule and XDG Storage Path

**Status:** Proposed

**Resolves:** ADR-002 (storage location)

**Research:** [Essay 17](../essays/17-storage-architecture.md)

**Domain model:** [domain-model.md](../domain-model.md) — GraphStore, context, PlexusEngine, transport

---

## Context

ADR-002 was deferred with a single question: should `.plexus.db` live in the project directory (discoverable but polluting) or somewhere centralized (clean but needing project identity)?

Essay 17 researched this across four deployment modes — MCP dev tool, embedded library (Sketchbin), managed server, and federated engine. Every embeddable database library surveyed (sled, redb, SurrealDB) follows the same pattern: the library takes a path; the host decides what to pass. MCP servers that try to manage their own storage paths break (Anthropic's Memory Server lost data via NPX subprocess environment variable failures). MCP servers that accept a path work (Anthropic's SQLite server).

The current `.plexus.db` in the project working directory pollutes the source tree and requires gitignore entries.

## Decision

### 1. The library rule

`GraphStore` takes a path (or connection string). Plexus the library never decides where to store data. The transport or host layer resolves the path based on its own conventions:

- **MCP server:** XDG Base Directory conventions (see below)
- **Embedded library (Sketchbin):** path from host application's config
- **Managed server:** path from deployment config or environment
- **gRPC/REST server:** path from server configuration

This aligns with invariant 40 (adapters, enrichments, and transports are independent dimensions) and invariant 41 (the library rule). Storage location is an infrastructure concern independent of all three extension dimensions. The `GraphStore` trait is unchanged.

### 2. XDG storage path for the MCP server

The MCP server resolves the database path using XDG Base Directory conventions. The database is user data (machine-generated, binary, not hand-editable), so it belongs in `$XDG_DATA_HOME`, not `$XDG_CONFIG_HOME`:

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/plexus/plexus.db` |
| macOS | `~/Library/Application Support/plexus/plexus.db` |
| Windows | `C:\Users\X\AppData\Roaming\plexus\data\plexus.db` |

The Rust `directories` crate provides cross-platform mapping via `ProjectDirs::from("", "", "plexus")`.

One database holds all contexts. The context — not the project directory — is the organizational unit. The MCP server can create or select contexts by name, and the same context can be used from different project directories. This preserves the cross-project shared context scenario (e.g., a "network-research" context accessed from both a Plexus project and a Sketchbin project).

### 3. Remove `.plexus.db` from project directory

The MCP server stops creating `.plexus.db` in the working directory. Existing `.plexus.db` files can be migrated (contexts imported into the centralized database) or left in place with a deprecation warning.

### Alternatives considered

- **Per-project database via path hashing.** A deterministic hash of the project directory produces a per-project subdirectory: `~/.local/share/plexus/{hash}/plexus.db`. Simpler per-project isolation, but actively prevents cross-project context sharing — the motivating scenario for ADR-017. Also breaks on directory rename (hash changes, producing a new database). Rejected: contexts already provide the organizational boundary; duplicating it at the filesystem level adds complexity without benefit.

- **Project-local `.plexus.db`.** The current approach. Discoverable but pollutes the project directory, requires gitignore entries, and prevents sharing across projects without symlinks or explicit paths.

## Consequences

**Positive:**

- No more project directory pollution — `.plexus.db` and `.gitignore` entries are eliminated
- The library rule keeps Plexus embeddable — Sketchbin, managed servers, and future hosts all pass their own paths without framework opinions
- Cross-platform path resolution via the `directories` crate follows OS conventions
- Each deployment mode controls its own storage without affecting others
- Cross-project context sharing works naturally — contexts are selected by name, not bound to a directory

**Negative:**

- The database is less discoverable than a file in the project root. Mitigation: a `plexus status` or MCP tool that reports the resolved path and lists available contexts
- All projects share one database file. If SQLite file corruption occurs, it affects all contexts. Mitigation: SQLite corruption is rare in practice; backup strategies apply at the database level

**Neutral:**

- The `GraphStore` trait is completely unchanged. This decision affects only the MCP server's startup code (path resolution)
- Migration from existing `.plexus.db` files is a one-time concern — contexts can be imported from old project-local databases
- The existing code already uses XDG via `dirs::data_dir()` but without per-project hashing. The change is to remove the `.plexus.db` fallback to the working directory and to stop creating project-specific subdirectories
