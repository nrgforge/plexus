# Behavior Scenarios: Storage Architecture (ADRs 016–017)

Refutable behavior scenarios for the library rule, XDG storage, and shared-DB prerequisites. Domain vocabulary from [domain-model.md](../domain-model.md).

---

## Feature: Centralized XDG storage with context-based organization (ADR-016)

### Scenario: MCP server resolves database to XDG data directory
**Given** the MCP server starts with no `--db` argument
**When** the server resolves the database path
**Then** the path is `{XDG_DATA_HOME}/plexus/plexus.db` (e.g., `~/.local/share/plexus/plexus.db` on Linux, `~/Library/Application Support/plexus/plexus.db` on macOS)

### Scenario: Same database used regardless of working directory
**Given** the MCP server starts from `/Users/alice/projects/alpha` and then from `/Users/alice/projects/beta`, both without `--db`
**When** each resolves the database path
**Then** both resolve to the same `{XDG_DATA_HOME}/plexus/plexus.db` — contexts, not directories, are the organizational unit

### Scenario: Explicit --db argument overrides XDG resolution
**Given** the MCP server starts with `--db /tmp/custom.db`
**When** the server resolves the database path
**Then** the path is `/tmp/custom.db`, ignoring XDG conventions

### Scenario: Database directory is created if it does not exist
**Given** no directory exists at `{XDG_DATA_HOME}/plexus/`
**When** the MCP server resolves the database path
**Then** the directory `{XDG_DATA_HOME}/plexus/` is created before the database file is opened

### Scenario: GraphStore takes a path without deciding it
**Given** a `SqliteStore` opened with path `/tmp/test.db`
**When** the store persists and loads a context
**Then** the database file is at `/tmp/test.db` — the store has no opinions about where this path came from

### Scenario: Cross-project context sharing
**Given** a context "network-research" created while working in `/Users/alice/projects/plexus`
**When** the MCP server starts from `/Users/alice/projects/sketchbin` and lists contexts
**Then** "network-research" is available — the context is not bound to the project directory that created it

---

## Feature: Incremental upserts for save_context (ADR-017 §3)

### Scenario: Incremental save preserves existing nodes from another engine
**Given** Engine A and Engine B share the same SQLite file
**And** Engine A has committed a fragment node `frag:a` to context "shared"
**And** Engine A has called `save_context()` for context "shared"
**When** Engine B commits a new fragment node `frag:b` to context "shared" and calls `save_context()`
**Then** the database contains both `frag:a` and `frag:b` in context "shared"

### Scenario: Incremental save upserts modified nodes
**Given** a context "project" with node `concept:travel` having property `source_count: 1`
**When** `save_context()` is called after updating `concept:travel` to property `source_count: 2`
**Then** the database contains `concept:travel` with property `source_count: 2`
**And** no duplicate `concept:travel` nodes exist

### Scenario: Incremental save preserves edge contributions
**Given** a context with an edge from `frag:1` to `concept:travel` with contribution `{fragment:manual: 1.0}`
**When** `save_context()` is called
**Then** loading the context produces the edge with identical contributions `{fragment:manual: 1.0}`

### Scenario: Incremental save handles node removal
**Given** a context "project" with nodes `concept:a` and `concept:b`
**When** an emission removes `concept:b` and `save_context()` is called
**Then** the database contains `concept:a` but not `concept:b` in context "project"

### Scenario: Incremental save handles edge removal
**Given** a context with edge `frag:1 -> concept:travel` (relationship `tagged_with`)
**When** an emission removes this edge and `save_context()` is called
**Then** the database contains no edge from `frag:1` to `concept:travel`

---

## Feature: WAL mode for concurrent access (ADR-017 §1)

### Scenario: SqliteStore enables WAL mode at connection time
**Given** a new `SqliteStore` is opened
**When** the connection is established
**Then** `PRAGMA journal_mode` returns `wal`

### Scenario: Concurrent reads do not block during a write
**Given** two connections to the same SQLite file in WAL mode
**When** connection A begins a write transaction
**Then** connection B can still execute read queries without waiting for A's transaction to complete

---

## Feature: Cache coherence via data_version polling (ADR-017 §2)

### Scenario: Engine detects external changes via data_version
**Given** Engine A and Engine B share the same SQLite file
**And** Engine A has loaded context "shared" into its in-memory cache
**When** Engine B commits new nodes to context "shared" and persists them
**And** Engine A checks `PRAGMA data_version`
**Then** the data_version value has increased since Engine A's last check

### Scenario: Engine reloads context when data_version changes
**Given** Engine A has context "shared" cached with 3 nodes
**When** Engine B adds 2 new nodes to context "shared" and persists them
**And** Engine A detects a data_version change and reloads context "shared"
**Then** Engine A's in-memory context "shared" now contains 5 nodes

### Scenario: No reload when data_version is unchanged
**Given** Engine A has context "shared" cached
**When** no other engine has written to the database
**And** Engine A checks `PRAGMA data_version`
**Then** the value is unchanged and no reload occurs

---

## Feature: Shared-concept convergence query (ADR-017 §4)

### Scenario: Shared concepts discovered via deterministic ID intersection
**Given** context "research" contains concepts `concept:travel`, `concept:distributed-systems`, and `concept:provence`
**And** context "fiction" contains concepts `concept:travel`, `concept:identity`, and `concept:provence`
**When** `shared_concepts("research", "fiction")` is called through PlexusApi
**Then** the result contains `concept:travel` and `concept:provence` but not `concept:distributed-systems` or `concept:identity`

### Scenario: No shared concepts returns empty result
**Given** context "alpha" contains concepts `concept:a`, `concept:b`
**And** context "beta" contains concepts `concept:c`, `concept:d`
**When** `shared_concepts("alpha", "beta")` is called through PlexusApi
**Then** the result is empty

### Scenario: Shared concepts query with nonexistent context
**Given** context "real" exists but context "imaginary" does not
**When** `shared_concepts("real", "imaginary")` is called through PlexusApi
**Then** the operation returns an error indicating the context does not exist
