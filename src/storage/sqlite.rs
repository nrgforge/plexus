//! SQLite storage backend for Plexus

use super::traits::{GraphStore, NodeFilter, OpenStore, StorageError, StorageResult, Subgraph};
use crate::graph::{Context, ContextId, Edge, Node, NodeId};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

/// Per-context baseline: the set of node/edge IDs that were last loaded or saved.
/// Used by incremental `save_context()` to determine which IDs to delete.
type Baseline = (HashSet<String>, HashSet<String>); // (node_ids, edge_ids)

/// SQLite-backed graph store
///
/// Uses a single SQLite database file with tables for contexts, nodes, and edges.
/// Thread-safe via internal mutex on the connection.
///
/// Tracks per-context "baselines" (ADR-017 §3) so that `save_context()` can
/// perform incremental upserts: nodes/edges added by other engines since the
/// last load are preserved, while nodes/edges explicitly removed by this
/// engine are deleted.
pub struct SqliteStore {
    conn: Mutex<Connection>,
    /// Baselines keyed by context ID string.
    baselines: Mutex<HashMap<String, Baseline>>,
}

impl SqliteStore {
    /// Initialize the database schema
    ///
    /// Uses a two-phase approach for migration compatibility:
    /// 1. Create base tables (without new dimension columns) - safe for existing DBs
    /// 2. Run migrations to add dimension columns to existing tables
    /// 3. Create dimension indexes (now columns exist)
    fn init_schema(conn: &Connection) -> StorageResult<()> {
        // Phase 1: Create base tables (compatible with pre-Phase 5.0 databases)
        // Note: CREATE TABLE IF NOT EXISTS won't modify existing tables,
        // so we use the minimal schema here and add columns via migration.
        conn.execute_batch(
            r#"
            -- Contexts table
            CREATE TABLE IF NOT EXISTS contexts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                metadata_json TEXT NOT NULL
            );

            -- Nodes table (base schema - dimension added via migration)
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT NOT NULL,
                context_id TEXT NOT NULL,
                node_type TEXT NOT NULL,
                content_type TEXT NOT NULL,
                properties_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                PRIMARY KEY (context_id, id),
                FOREIGN KEY (context_id) REFERENCES contexts(id) ON DELETE CASCADE
            );

            -- Base indexes for node queries (non-dimension)
            CREATE INDEX IF NOT EXISTS idx_nodes_type
                ON nodes(context_id, node_type);
            CREATE INDEX IF NOT EXISTS idx_nodes_content_type
                ON nodes(context_id, content_type);

            -- Edges table (base schema - dimensions added via migration)
            CREATE TABLE IF NOT EXISTS edges (
                id TEXT NOT NULL,
                context_id TEXT NOT NULL,
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                relationship TEXT NOT NULL,
                raw_weight REAL NOT NULL,
                created_at TEXT NOT NULL,
                properties_json TEXT NOT NULL,
                PRIMARY KEY (context_id, id),
                FOREIGN KEY (context_id) REFERENCES contexts(id) ON DELETE CASCADE
            );

            -- Base indexes for edge traversal (non-dimension)
            CREATE INDEX IF NOT EXISTS idx_edges_source
                ON edges(context_id, source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_target
                ON edges(context_id, target_id);
            CREATE INDEX IF NOT EXISTS idx_edges_relationship
                ON edges(context_id, relationship);

            -- Enable foreign keys
            PRAGMA foreign_keys = ON;

            -- Enable WAL mode for concurrent reads during writes (ADR-017 §1)
            PRAGMA journal_mode = WAL;
            "#,
        )?;

        // Phase 2: Run migrations
        Self::migrate_add_dimensions(conn)?;
        Self::migrate_add_contributions(conn)?;

        // Phase 3: Create dimension indexes (now that columns exist)
        Self::create_dimension_indexes(conn)?;

        Ok(())
    }

    /// Create indexes for dimension columns (Phase 5.0)
    ///
    /// Called after migration ensures dimension columns exist.
    fn create_dimension_indexes(conn: &Connection) -> StorageResult<()> {
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_dimension ON nodes(context_id, dimension)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_cross_dimensional ON edges(context_id, source_dimension, target_dimension)",
            [],
        )?;
        Ok(())
    }

    /// Migration: Add dimension columns to existing databases (Phase 5.0)
    ///
    /// SQLite doesn't support ALTER TABLE ADD COLUMN IF NOT EXISTS,
    /// so we check if columns exist first using table_info pragma.
    fn migrate_add_dimensions(conn: &Connection) -> StorageResult<()> {
        // Check if nodes table has dimension column
        let has_node_dimension: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('nodes') WHERE name = 'dimension'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_node_dimension {
            // Add dimension column to nodes
            conn.execute(
                "ALTER TABLE nodes ADD COLUMN dimension TEXT NOT NULL DEFAULT 'default'",
                [],
            )?;
            // Create index
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_nodes_dimension ON nodes(context_id, dimension)",
                [],
            )?;
        }

        // Check if edges table has dimension columns
        let has_edge_source_dim: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('edges') WHERE name = 'source_dimension'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_edge_source_dim {
            // Add dimension columns to edges
            conn.execute(
                "ALTER TABLE edges ADD COLUMN source_dimension TEXT NOT NULL DEFAULT 'default'",
                [],
            )?;
            conn.execute(
                "ALTER TABLE edges ADD COLUMN target_dimension TEXT NOT NULL DEFAULT 'default'",
                [],
            )?;
            // Create index
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_edges_cross_dimensional ON edges(context_id, source_dimension, target_dimension)",
                [],
            )?;
        }

        Ok(())
    }

    /// Migration: Add contributions_json column to edges table (ADR-007)
    ///
    /// Stores per-adapter contribution values as JSON. Existing edges get
    /// an empty contributions map '{}', preserving their raw_weight unchanged.
    fn migrate_add_contributions(conn: &Connection) -> StorageResult<()> {
        let has_contributions: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('edges') WHERE name = 'contributions_json'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_contributions {
            conn.execute(
                "ALTER TABLE edges ADD COLUMN contributions_json TEXT NOT NULL DEFAULT '{}'",
                [],
            )?;
        }

        Ok(())
    }

    /// Serialize a node to database columns (includes dimension field)
    fn node_to_row(node: &Node) -> StorageResult<(String, String, String, String, String, String)> {
        Ok((
            node.id.as_str().to_string(),
            node.node_type.clone(),
            serde_json::to_string(&node.content_type)?,
            node.dimension.clone(),
            serde_json::to_string(&node.properties)?,
            serde_json::to_string(&node.metadata)?,
        ))
    }

    /// Deserialize a node from database columns (includes dimension field)
    fn row_to_node(
        id: String,
        node_type: String,
        content_type_json: String,
        dimension: String,
        properties_json: String,
        metadata_json: String,
    ) -> StorageResult<Node> {
        Ok(Node {
            id: NodeId::from_string(id),
            node_type,
            content_type: serde_json::from_str(&content_type_json)?,
            dimension,
            properties: serde_json::from_str(&properties_json)?,
            metadata: serde_json::from_str(&metadata_json)?,
        })
    }

    /// Serialize an edge to database columns (includes dimension and contribution fields)
    #[allow(clippy::type_complexity)]
    fn edge_to_row(
        edge: &Edge,
    ) -> StorageResult<(
        String,
        String,
        String,
        String,
        String,
        String,
        f32,
        String,
        String,
        String,
    )> {
        Ok((
            edge.id.as_str().to_string(),
            edge.source.as_str().to_string(),
            edge.target.as_str().to_string(),
            edge.source_dimension.clone(),
            edge.target_dimension.clone(),
            edge.relationship.clone(),
            edge.raw_weight,
            edge.created_at.to_rfc3339(),
            serde_json::to_string(&edge.properties)?,
            serde_json::to_string(&edge.contributions)?,
        ))
    }

    /// Deserialize an edge from database columns (includes dimension and contribution fields)
    #[allow(clippy::too_many_arguments)]
    fn row_to_edge(
        id: String,
        source_id: String,
        target_id: String,
        source_dimension: String,
        target_dimension: String,
        relationship: String,
        raw_weight: f64,
        created_at: String,
        properties_json: String,
        contributions_json: String,
    ) -> StorageResult<Edge> {
        use chrono::DateTime;
        use crate::graph::EdgeId;

        Ok(Edge {
            id: EdgeId::from_string(id),
            source: NodeId::from_string(source_id),
            target: NodeId::from_string(target_id),
            source_dimension,
            target_dimension,
            relationship,
            contributions: serde_json::from_str(&contributions_json)?,
            raw_weight: raw_weight as f32,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map_err(|e| StorageError::DateParse(e.to_string()))?
                .with_timezone(&chrono::Utc),
            properties: serde_json::from_str(&properties_json)?,
        })
    }
}

impl OpenStore for SqliteStore {
    fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            baselines: Mutex::new(HashMap::new()),
        })
    }

    fn open_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            baselines: Mutex::new(HashMap::new()),
        })
    }
}

impl GraphStore for SqliteStore {
    // === Context Operations ===

    fn save_context_metadata(&self, context: &Context) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let metadata_json = serde_json::to_string(&context.metadata)?;

        conn.execute(
            r#"
            INSERT INTO contexts (id, name, description, metadata_json)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                metadata_json = excluded.metadata_json
            "#,
            params![
                context.id.as_str(),
                context.name,
                context.description,
                metadata_json,
            ],
        )?;

        Ok(())
    }

    fn save_context(&self, context: &Context) -> StorageResult<()> {
        // Save the context row (name, description, metadata)
        self.save_context_metadata(context)?;

        let conn = self.conn.lock().unwrap();

        // Incremental upsert (ADR-017 §3): upsert nodes/edges that are in
        // the context, then delete only those that the context explicitly
        // does NOT contain. This preserves nodes/edges written by other
        // engines sharing the same database.

        // --- Nodes: upsert all in-memory nodes ---
        let context_node_ids: HashSet<String> = context
            .nodes
            .keys()
            .map(|id| id.to_string())
            .collect();

        for node in context.nodes.values() {
            let (id, node_type, content_type, dimension, properties, metadata) = Self::node_to_row(node)?;
            conn.execute(
                r#"
                INSERT INTO nodes (id, context_id, node_type, content_type, dimension, properties_json, metadata_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(context_id, id) DO UPDATE SET
                    node_type = excluded.node_type,
                    content_type = excluded.content_type,
                    dimension = excluded.dimension,
                    properties_json = excluded.properties_json,
                    metadata_json = excluded.metadata_json
                "#,
                params![id, context.id.as_str(), node_type, content_type, dimension, properties, metadata],
            )?;
        }

        // --- Edges: upsert all in-memory edges ---
        let context_edge_ids: HashSet<String> = context
            .edges
            .iter()
            .map(|e| e.id.to_string())
            .collect();

        for edge in &context.edges {
            let (id, source, target, source_dim, target_dim, rel, raw_weight, created, props, contributions) =
                Self::edge_to_row(edge)?;

            conn.execute(
                r#"
                INSERT INTO edges (id, context_id, source_id, target_id, source_dimension, target_dimension,
                                   relationship, raw_weight, created_at, properties_json, contributions_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ON CONFLICT(context_id, id) DO UPDATE SET
                    source_id = excluded.source_id,
                    target_id = excluded.target_id,
                    source_dimension = excluded.source_dimension,
                    target_dimension = excluded.target_dimension,
                    relationship = excluded.relationship,
                    raw_weight = excluded.raw_weight,
                    properties_json = excluded.properties_json,
                    contributions_json = excluded.contributions_json
                "#,
                params![id, context.id.as_str(), source, target, source_dim, target_dim, rel, raw_weight, created, props, contributions],
            )?;
        }

        // --- Delete nodes/edges that were in our baseline but are no longer
        // in the context (i.e., explicitly removed by this engine). ---
        // Nodes/edges added by other engines are NOT in our baseline, so
        // they survive this save.
        {
            let baselines = self.baselines.lock().unwrap();
            if let Some((baseline_nodes, baseline_edges)) = baselines.get(context.id.as_str()) {
                // Delete edges first (foreign key safety)
                for baseline_edge_id in baseline_edges {
                    if !context_edge_ids.contains(baseline_edge_id) {
                        conn.execute(
                            "DELETE FROM edges WHERE context_id = ?1 AND id = ?2",
                            params![context.id.as_str(), baseline_edge_id],
                        )?;
                    }
                }
                // Delete nodes
                for baseline_node_id in baseline_nodes {
                    if !context_node_ids.contains(baseline_node_id) {
                        conn.execute(
                            "DELETE FROM nodes WHERE context_id = ?1 AND id = ?2",
                            params![context.id.as_str(), baseline_node_id],
                        )?;
                    }
                }
            }
        }

        // Update baseline to match current context state
        self.baselines.lock().unwrap().insert(
            context.id.as_str().to_string(),
            (context_node_ids, context_edge_ids),
        );

        Ok(())
    }

    fn load_context(&self, id: &ContextId) -> StorageResult<Option<Context>> {
        let conn = self.conn.lock().unwrap();

        // Load context metadata
        let context_row: Option<(String, Option<String>, String)> = conn
            .query_row(
                "SELECT name, description, metadata_json FROM contexts WHERE id = ?1",
                params![id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let Some((name, description, metadata_json)) = context_row else {
            return Ok(None);
        };

        // Load nodes
        let mut stmt = conn.prepare(
            "SELECT id, node_type, content_type, dimension, properties_json, metadata_json
             FROM nodes WHERE context_id = ?1",
        )?;
        let nodes_iter = stmt.query_map(params![id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let mut nodes = HashMap::new();
        for row in nodes_iter {
            let (node_id, node_type, content_type, dimension, properties, metadata) = row?;
            let node = Self::row_to_node(node_id, node_type, content_type, dimension, properties, metadata)?;
            nodes.insert(node.id.clone(), node);
        }

        // Load edges
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_dimension, target_dimension, relationship,
                    raw_weight, created_at, properties_json, contributions_json
             FROM edges WHERE context_id = ?1",
        )?;
        let edges_iter = stmt.query_map(params![id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut edges = Vec::new();
        for row in edges_iter {
            let (id, source, target, source_dim, target_dim, rel, rw, created, props, contributions) = row?;
            let edge = Self::row_to_edge(id, source, target, source_dim, target_dim, rel, rw, created, props, contributions)?;
            edges.push(edge);
        }

        // Record baseline for incremental save_context (ADR-017 §3)
        let baseline_nodes: HashSet<String> = nodes.keys().map(|k| k.to_string()).collect();
        let baseline_edges: HashSet<String> = edges.iter().map(|e| e.id.to_string()).collect();
        self.baselines.lock().unwrap().insert(
            id.as_str().to_string(),
            (baseline_nodes, baseline_edges),
        );

        Ok(Some(Context {
            id: id.clone(),
            name,
            description,
            nodes,
            edges,
            metadata: serde_json::from_str(&metadata_json)?,
        }))
    }

    fn delete_context(&self, id: &ContextId) -> StorageResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM contexts WHERE id = ?1", params![id.as_str()])?;
        Ok(rows > 0)
    }

    fn list_contexts(&self) -> StorageResult<Vec<ContextId>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM contexts")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|r| r.map(ContextId::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    // === Node Operations ===

    fn save_node(&self, context_id: &ContextId, node: &Node) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let (id, node_type, content_type, dimension, properties, metadata) = Self::node_to_row(node)?;

        conn.execute(
            r#"
            INSERT INTO nodes (id, context_id, node_type, content_type, dimension, properties_json, metadata_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(context_id, id) DO UPDATE SET
                node_type = excluded.node_type,
                content_type = excluded.content_type,
                dimension = excluded.dimension,
                properties_json = excluded.properties_json,
                metadata_json = excluded.metadata_json
            "#,
            params![id, context_id.as_str(), node_type, content_type, dimension, properties, metadata],
        )?;

        Ok(())
    }

    fn load_node(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<Option<Node>> {
        let conn = self.conn.lock().unwrap();

        let row: Option<(String, String, String, String, String, String)> = conn
            .query_row(
                "SELECT id, node_type, content_type, dimension, properties_json, metadata_json
                 FROM nodes WHERE context_id = ?1 AND id = ?2",
                params![context_id.as_str(), node_id.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()?;

        match row {
            Some((id, node_type, content_type, dimension, properties, metadata)) => {
                Ok(Some(Self::row_to_node(id, node_type, content_type, dimension, properties, metadata)?))
            }
            None => Ok(None),
        }
    }

    fn delete_node(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<bool> {
        let conn = self.conn.lock().unwrap();

        // Delete edges connected to this node
        conn.execute(
            "DELETE FROM edges WHERE context_id = ?1 AND (source_id = ?2 OR target_id = ?2)",
            params![context_id.as_str(), node_id.as_str()],
        )?;

        // Delete the node
        let rows = conn.execute(
            "DELETE FROM nodes WHERE context_id = ?1 AND id = ?2",
            params![context_id.as_str(), node_id.as_str()],
        )?;

        Ok(rows > 0)
    }

    fn find_nodes(&self, context_id: &ContextId, filter: &NodeFilter) -> StorageResult<Vec<Node>> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, node_type, content_type, dimension, properties_json, metadata_json FROM nodes WHERE context_id = ?1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(context_id.as_str().to_string())];

        if let Some(ref node_type) = filter.node_type {
            sql.push_str(" AND node_type = ?");
            params_vec.push(Box::new(node_type.clone()));
        }

        if let Some(ref content_type) = filter.content_type {
            // content_type is stored as JSON string like "\"code\""
            sql.push_str(" AND content_type = ?");
            params_vec.push(Box::new(format!("\"{}\"", content_type)));
        }

        if let Some(ref dimension) = filter.dimension {
            sql.push_str(" AND dimension = ?");
            params_vec.push(Box::new(dimension.clone()));
        }

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

        let nodes_iter = stmt.query_map(params_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let mut nodes = Vec::new();
        for row in nodes_iter {
            let (id, node_type, content_type, dimension, properties, metadata) = row?;
            nodes.push(Self::row_to_node(id, node_type, content_type, dimension, properties, metadata)?);
        }

        Ok(nodes)
    }

    // === Edge Operations ===

    fn save_edge(&self, context_id: &ContextId, edge: &Edge) -> StorageResult<()> {
        let conn = self.conn.lock().unwrap();
        let (id, source, target, source_dim, target_dim, rel, raw_weight, created, props, contributions) =
            Self::edge_to_row(edge)?;

        conn.execute(
            r#"
            INSERT INTO edges (id, context_id, source_id, target_id, source_dimension, target_dimension,
                               relationship, raw_weight, created_at, properties_json, contributions_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(context_id, id) DO UPDATE SET
                source_id = excluded.source_id,
                target_id = excluded.target_id,
                source_dimension = excluded.source_dimension,
                target_dimension = excluded.target_dimension,
                relationship = excluded.relationship,
                raw_weight = excluded.raw_weight,
                properties_json = excluded.properties_json,
                contributions_json = excluded.contributions_json
            "#,
            params![id, context_id.as_str(), source, target, source_dim, target_dim, rel, raw_weight, created, props, contributions],
        )?;

        Ok(())
    }

    fn get_edges_from(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<Vec<Edge>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_dimension, target_dimension, relationship,
                    raw_weight, created_at, properties_json, contributions_json
             FROM edges WHERE context_id = ?1 AND source_id = ?2",
        )?;

        let edges_iter = stmt.query_map(params![context_id.as_str(), node_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut edges = Vec::new();
        for row in edges_iter {
            let (id, source, target, source_dim, target_dim, rel, rw, created, props, contributions) = row?;
            edges.push(Self::row_to_edge(id, source, target, source_dim, target_dim, rel, rw, created, props, contributions)?);
        }

        Ok(edges)
    }

    fn get_edges_to(&self, context_id: &ContextId, node_id: &NodeId) -> StorageResult<Vec<Edge>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, source_dimension, target_dimension, relationship,
                    raw_weight, created_at, properties_json, contributions_json
             FROM edges WHERE context_id = ?1 AND target_id = ?2",
        )?;

        let edges_iter = stmt.query_map(params![context_id.as_str(), node_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut edges = Vec::new();
        for row in edges_iter {
            let (id, source, target, source_dim, target_dim, rel, rw, created, props, contributions) = row?;
            edges.push(Self::row_to_edge(id, source, target, source_dim, target_dim, rel, rw, created, props, contributions)?);
        }

        Ok(edges)
    }

    fn delete_edge(&self, context_id: &ContextId, edge_id: &str) -> StorageResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM edges WHERE context_id = ?1 AND id = ?2",
            params![context_id.as_str(), edge_id],
        )?;
        Ok(rows > 0)
    }

    // === Subgraph Operations ===

    fn load_subgraph(
        &self,
        context_id: &ContextId,
        seeds: &[NodeId],
        max_depth: usize,
    ) -> StorageResult<Subgraph> {
        if seeds.is_empty() || max_depth == 0 {
            return Ok(Subgraph {
                nodes: Vec::new(),
                edges: Vec::new(),
            });
        }

        let conn = self.conn.lock().unwrap();

        // BFS to find all reachable nodes within max_depth
        let mut visited: HashSet<String> = HashSet::new();
        let mut frontier: Vec<String> = seeds.iter().map(|n| n.as_str().to_string()).collect();

        for seed in &frontier {
            visited.insert(seed.clone());
        }

        for _depth in 0..max_depth {
            if frontier.is_empty() {
                break;
            }

            let mut next_frontier = Vec::new();

            for node_id in &frontier {
                // Get outgoing edges
                let mut stmt = conn.prepare(
                    "SELECT target_id FROM edges WHERE context_id = ?1 AND source_id = ?2",
                )?;
                let targets = stmt
                    .query_map(params![context_id.as_str(), node_id], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                for target in targets {
                    if visited.insert(target.clone()) {
                        next_frontier.push(target);
                    }
                }

                // Get incoming edges (for bidirectional traversal)
                let mut stmt = conn.prepare(
                    "SELECT source_id FROM edges WHERE context_id = ?1 AND target_id = ?2",
                )?;
                let sources = stmt
                    .query_map(params![context_id.as_str(), node_id], |row| {
                        row.get::<_, String>(0)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                for source in sources {
                    if visited.insert(source.clone()) {
                        next_frontier.push(source);
                    }
                }
            }

            frontier = next_frontier;
        }

        // Load all visited nodes
        let mut nodes = Vec::new();
        for node_id in &visited {
            let row: Option<(String, String, String, String, String, String)> = conn
                .query_row(
                    "SELECT id, node_type, content_type, dimension, properties_json, metadata_json
                     FROM nodes WHERE context_id = ?1 AND id = ?2",
                    params![context_id.as_str(), node_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()?;

            if let Some((id, node_type, content_type, dimension, properties, metadata)) = row {
                nodes.push(Self::row_to_node(id, node_type, content_type, dimension, properties, metadata)?);
            }
        }

        // Load edges where both endpoints are in visited set
        let placeholders: Vec<&str> = visited.iter().map(|_| "?").collect();
        let in_clause = placeholders.join(",");

        let sql = format!(
            "SELECT id, source_id, target_id, source_dimension, target_dimension, relationship,
                    raw_weight, created_at, properties_json, contributions_json
             FROM edges
             WHERE context_id = ?1
               AND source_id IN ({})
               AND target_id IN ({})",
            in_clause, in_clause
        );

        let mut params_vec: Vec<String> = vec![context_id.as_str().to_string()];
        params_vec.extend(visited.iter().cloned());
        params_vec.extend(visited.iter().cloned());

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let edges_iter = stmt.query_map(params_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut edges = Vec::new();
        for row in edges_iter {
            let (id, source, target, source_dim, target_dim, rel, rw, created, props, contributions) = row?;
            edges.push(Self::row_to_edge(id, source, target, source_dim, target_dim, rel, rw, created, props, contributions)?);
        }

        Ok(Subgraph { nodes, edges })
    }

    fn data_version(&self) -> StorageResult<u64> {
        let conn = self.conn.lock().unwrap();
        let version: i64 = conn.query_row("PRAGMA data_version", [], |row| row.get(0))?;
        Ok(version as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{dimension, ContentType, Edge, Node, PropertyValue};

    fn create_test_store() -> SqliteStore {
        SqliteStore::open_in_memory().unwrap()
    }

    fn create_test_context() -> Context {
        Context::new("test-context").with_description("A test context")
    }

    fn create_test_node(id: &str, node_type: &str) -> Node {
        let mut node = Node::new(node_type, ContentType::Code);
        node.id = NodeId::from_string(id);
        node
    }

    // ========================================================================
    // Phase 5.0 Migration Tests - Dimension Fields
    // ========================================================================

    #[test]
    fn test_node_dimension_persistence() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        // Create node in structure dimension
        let mut node = create_test_node("node:structure-test", "heading");
        node.dimension = dimension::STRUCTURE.to_string();

        store.save_node(&ctx_id, &node).unwrap();

        // Load and verify dimension is preserved
        let loaded = store.load_node(&ctx_id, &node.id).unwrap().unwrap();
        assert_eq!(loaded.dimension, dimension::STRUCTURE);
    }

    #[test]
    fn test_node_default_dimension() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        // Create node without specifying dimension
        let node = create_test_node("node:default-dim", "function");
        store.save_node(&ctx_id, &node).unwrap();

        // Verify default dimension
        let loaded = store.load_node(&ctx_id, &node.id).unwrap().unwrap();
        assert_eq!(loaded.dimension, dimension::DEFAULT);
    }

    #[test]
    fn test_edge_dimension_persistence() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        // Create cross-dimensional edge
        let node_a = create_test_node("node:a", "function");
        let node_b = create_test_node("node:b", "concept");
        store.save_node(&ctx_id, &node_a).unwrap();
        store.save_node(&ctx_id, &node_b).unwrap();

        let edge = Edge::new_cross_dimensional(
            node_a.id.clone(),
            dimension::STRUCTURE,
            node_b.id.clone(),
            dimension::SEMANTIC,
            "implements",
        );
        store.save_edge(&ctx_id, &edge).unwrap();

        // Load and verify dimensions
        let edges = store.get_edges_from(&ctx_id, &node_a.id).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source_dimension, dimension::STRUCTURE);
        assert_eq!(edges[0].target_dimension, dimension::SEMANTIC);
        assert!(edges[0].is_cross_dimensional());
    }

    #[test]
    fn test_find_nodes_by_dimension() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        // Create nodes in different dimensions
        let mut node1 = create_test_node("node:struct1", "heading");
        node1.dimension = dimension::STRUCTURE.to_string();

        let mut node2 = create_test_node("node:struct2", "section");
        node2.dimension = dimension::STRUCTURE.to_string();

        let mut node3 = create_test_node("node:semantic1", "concept");
        node3.dimension = dimension::SEMANTIC.to_string();

        store.save_node(&ctx_id, &node1).unwrap();
        store.save_node(&ctx_id, &node2).unwrap();
        store.save_node(&ctx_id, &node3).unwrap();

        // Find nodes in structure dimension
        let filter = NodeFilter::new().with_dimension(dimension::STRUCTURE);
        let structure_nodes = store.find_nodes(&ctx_id, &filter).unwrap();
        assert_eq!(structure_nodes.len(), 2);

        // Find nodes in semantic dimension
        let filter = NodeFilter::new().with_dimension(dimension::SEMANTIC);
        let semantic_nodes = store.find_nodes(&ctx_id, &filter).unwrap();
        assert_eq!(semantic_nodes.len(), 1);
    }

    #[test]
    fn test_context_with_dimensional_data() {
        let store = create_test_store();

        // Create context with dimensional nodes and edges
        let mut ctx = create_test_context();

        let mut struct_node = Node::new("heading", ContentType::Document);
        struct_node.dimension = dimension::STRUCTURE.to_string();
        struct_node.id = NodeId::from_string("heading:intro");

        let mut semantic_node = Node::new("concept", ContentType::Concept);
        semantic_node.dimension = dimension::SEMANTIC.to_string();
        semantic_node.id = NodeId::from_string("concept:auth");

        let cross_edge = Edge::new_cross_dimensional(
            struct_node.id.clone(),
            dimension::STRUCTURE,
            semantic_node.id.clone(),
            dimension::SEMANTIC,
            "discusses",
        );

        ctx.add_node(struct_node);
        ctx.add_node(semantic_node);
        ctx.add_edge(cross_edge);

        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        // Load and verify
        let loaded = store.load_context(&ctx_id).unwrap().unwrap();
        assert_eq!(loaded.nodes.len(), 2);
        assert_eq!(loaded.edges.len(), 1);

        // Verify dimensions
        let heading = loaded.nodes.get(&NodeId::from_string("heading:intro")).unwrap();
        assert_eq!(heading.dimension, dimension::STRUCTURE);

        let concept = loaded.nodes.get(&NodeId::from_string("concept:auth")).unwrap();
        assert_eq!(concept.dimension, dimension::SEMANTIC);

        assert!(loaded.edges[0].is_cross_dimensional());
    }

    #[test]
    fn test_save_and_load_context() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();

        store.save_context(&ctx).unwrap();

        let loaded = store.load_context(&ctx_id).unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "test-context");
        assert_eq!(loaded.description, Some("A test context".to_string()));
    }

    #[test]
    fn test_list_contexts() {
        let store = create_test_store();

        let ctx1 = Context::new("context-1");
        let ctx2 = Context::new("context-2");

        store.save_context(&ctx1).unwrap();
        store.save_context(&ctx2).unwrap();

        let contexts = store.list_contexts().unwrap();
        assert_eq!(contexts.len(), 2);
    }

    #[test]
    fn test_delete_context() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();

        store.save_context(&ctx).unwrap();
        assert!(store.load_context(&ctx_id).unwrap().is_some());

        let deleted = store.delete_context(&ctx_id).unwrap();
        assert!(deleted);

        assert!(store.load_context(&ctx_id).unwrap().is_none());
    }

    #[test]
    fn test_save_and_load_node() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        let node = create_test_node("node:test", "function")
            .with_property("language", PropertyValue::String("rust".to_string()));
        let node_id = node.id.clone();

        store.save_node(&ctx_id, &node).unwrap();

        let loaded = store.load_node(&ctx_id, &node_id).unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.node_type, "function");
        assert_eq!(
            loaded.properties.get("language"),
            Some(&PropertyValue::String("rust".to_string()))
        );
    }

    #[test]
    fn test_find_nodes_by_type() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        store.save_node(&ctx_id, &create_test_node("n1", "function")).unwrap();
        store.save_node(&ctx_id, &create_test_node("n2", "function")).unwrap();
        store.save_node(&ctx_id, &create_test_node("n3", "class")).unwrap();

        let functions = store
            .find_nodes(&ctx_id, &NodeFilter::new().with_type("function"))
            .unwrap();
        assert_eq!(functions.len(), 2);

        let classes = store
            .find_nodes(&ctx_id, &NodeFilter::new().with_type("class"))
            .unwrap();
        assert_eq!(classes.len(), 1);
    }

    #[test]
    fn test_save_and_get_edges() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        let node_a = create_test_node("node:a", "function");
        let node_b = create_test_node("node:b", "function");
        store.save_node(&ctx_id, &node_a).unwrap();
        store.save_node(&ctx_id, &node_b).unwrap();

        let edge = Edge::new(node_a.id.clone(), node_b.id.clone(), "calls");
        store.save_edge(&ctx_id, &edge).unwrap();

        let edges_from_a = store.get_edges_from(&ctx_id, &node_a.id).unwrap();
        assert_eq!(edges_from_a.len(), 1);
        assert_eq!(edges_from_a[0].relationship, "calls");

        let edges_to_b = store.get_edges_to(&ctx_id, &node_b.id).unwrap();
        assert_eq!(edges_to_b.len(), 1);
    }

    #[test]
    fn test_delete_node_cascades_edges() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        let node_a = create_test_node("node:a", "function");
        let node_b = create_test_node("node:b", "function");
        store.save_node(&ctx_id, &node_a).unwrap();
        store.save_node(&ctx_id, &node_b).unwrap();

        let edge = Edge::new(node_a.id.clone(), node_b.id.clone(), "calls");
        store.save_edge(&ctx_id, &edge).unwrap();

        // Delete node A - should also delete the edge
        store.delete_node(&ctx_id, &node_a.id).unwrap();

        let edges_to_b = store.get_edges_to(&ctx_id, &node_b.id).unwrap();
        assert_eq!(edges_to_b.len(), 0);
    }

    #[test]
    fn test_load_subgraph() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        // Create a small graph: A -> B -> C -> D
        let nodes: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|id| create_test_node(&format!("node:{}", id), "function"))
            .collect();

        for node in &nodes {
            store.save_node(&ctx_id, node).unwrap();
        }

        store.save_edge(&ctx_id, &Edge::new(nodes[0].id.clone(), nodes[1].id.clone(), "calls")).unwrap();
        store.save_edge(&ctx_id, &Edge::new(nodes[1].id.clone(), nodes[2].id.clone(), "calls")).unwrap();
        store.save_edge(&ctx_id, &Edge::new(nodes[2].id.clone(), nodes[3].id.clone(), "calls")).unwrap();

        // Load subgraph from A with depth 2 - should get A, B, C
        let subgraph = store
            .load_subgraph(&ctx_id, &[nodes[0].id.clone()], 2)
            .unwrap();

        assert_eq!(subgraph.nodes.len(), 3); // A, B, C
        assert_eq!(subgraph.edges.len(), 2); // A->B, B->C
    }

    // ========================================================================
    // ADR-007: Contribution Persistence Tests
    // ========================================================================

    #[test]
    fn test_edge_contributions_persist() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        let node_a = create_test_node("node:a", "concept");
        let node_b = create_test_node("node:b", "concept");
        store.save_node(&ctx_id, &node_a).unwrap();
        store.save_node(&ctx_id, &node_b).unwrap();

        let mut edge = Edge::new(node_a.id.clone(), node_b.id.clone(), "tagged_with");
        edge.contributions.insert("fragment-manual".to_string(), 1.0);
        edge.contributions.insert("co-occurrence".to_string(), 0.75);
        store.save_edge(&ctx_id, &edge).unwrap();

        // Load and verify contributions round-trip
        let edges = store.get_edges_from(&ctx_id, &node_a.id).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].contributions.get("fragment-manual"), Some(&1.0));
        assert_eq!(edges[0].contributions.get("co-occurrence"), Some(&0.75));
    }

    #[test]
    fn test_edges_without_contributions_load_empty_map() {
        let store = create_test_store();
        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store.save_context(&ctx).unwrap();

        let node_a = create_test_node("node:a", "concept");
        let node_b = create_test_node("node:b", "concept");
        store.save_node(&ctx_id, &node_a).unwrap();
        store.save_node(&ctx_id, &node_b).unwrap();

        // Edge with no contributions set
        let edge = Edge::new(node_a.id.clone(), node_b.id.clone(), "calls");
        store.save_edge(&ctx_id, &edge).unwrap();

        let edges = store.get_edges_from(&ctx_id, &node_a.id).unwrap();
        assert_eq!(edges.len(), 1);
        assert!(edges[0].contributions.is_empty(), "edge without contributions should load with empty map");
    }

    // ========================================================================
    // ADR-017 §1: WAL Mode Tests
    // ========================================================================

    #[test]
    fn test_wal_mode_enabled_at_connection() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test-wal.db");
        let store = SqliteStore::open(&db_path).unwrap();

        let journal_mode: String = store
            .conn
            .lock()
            .unwrap()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();

        assert_eq!(journal_mode, "wal", "SqliteStore must enable WAL mode at connection time (ADR-017 §1)");
    }

    #[test]
    fn test_concurrent_read_during_write() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test-concurrent.db");

        // Open two stores on the same file
        let store_a = SqliteStore::open(&db_path).unwrap();
        let store_b = SqliteStore::open(&db_path).unwrap();

        let ctx = create_test_context();
        let ctx_id = ctx.id.clone();
        store_a.save_context(&ctx).unwrap();

        // Store A begins a write: add a node inside a transaction
        {
            let conn_a = store_a.conn.lock().unwrap();
            conn_a.execute("BEGIN IMMEDIATE", []).unwrap();
            conn_a
                .execute(
                    "INSERT INTO nodes (id, context_id, node_type, content_type, properties_json, metadata_json, dimension) VALUES (?1, ?2, 'concept', 'document', '{}', '{}', 'default')",
                    params!["node:writing", ctx_id.to_string()],
                )
                .unwrap();
            // Transaction still open — A is writing

            // Store B should be able to read without blocking
            let loaded = store_b.load_context(&ctx_id).unwrap();
            assert!(loaded.is_some(), "concurrent read must succeed during write (WAL mode)");

            conn_a.execute("COMMIT", []).unwrap();
        }
    }

    #[test]
    fn test_contributions_survive_context_save_load() {
        let store = create_test_store();
        let mut ctx = create_test_context();
        let ctx_id = ctx.id.clone();

        let node_a = create_test_node("node:a", "concept");
        let node_b = create_test_node("node:b", "concept");
        ctx.add_node(node_a.clone());
        ctx.add_node(node_b.clone());

        let mut edge = Edge::new(node_a.id.clone(), node_b.id.clone(), "tagged_with");
        edge.contributions.insert("fragment-manual".to_string(), 1.0);
        edge.contributions.insert("co-occurrence".to_string(), 0.75);
        ctx.add_edge(edge);

        store.save_context(&ctx).unwrap();

        // Load back and verify
        let loaded = store.load_context(&ctx_id).unwrap().unwrap();
        assert_eq!(loaded.edges.len(), 1);
        assert_eq!(loaded.edges[0].contributions.get("fragment-manual"), Some(&1.0));
        assert_eq!(loaded.edges[0].contributions.get("co-occurrence"), Some(&0.75));
    }

    // ========================================================================
    // ADR-017 §3: Incremental Upsert Tests
    // ========================================================================

    #[test]
    fn test_incremental_save_preserves_nodes_from_another_engine() {
        // Simulates two engines with stale caches sharing the same DB.
        // The DELETE-all approach would lose frag:b when Engine A saves.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("shared.db");

        let store_a = SqliteStore::open(&db_path).unwrap();
        let store_b = SqliteStore::open(&db_path).unwrap();

        // Both engines start with empty context
        let ctx = Context::new("shared");
        let ctx_id = ctx.id.clone();
        store_a.save_context(&ctx).unwrap();

        // Engine A loads, adds frag:a, saves
        let mut ctx_a = store_a.load_context(&ctx_id).unwrap().unwrap();
        ctx_a.add_node(create_test_node("frag:a", "fragment"));
        store_a.save_context(&ctx_a).unwrap();

        // Engine B loads (now has frag:a), adds frag:b, saves
        let mut ctx_b = store_b.load_context(&ctx_id).unwrap().unwrap();
        ctx_b.add_node(create_test_node("frag:b", "fragment"));
        store_b.save_context(&ctx_b).unwrap();

        // Engine A's cache is STALE — it only has frag:a, doesn't know about frag:b.
        // Engine A adds frag:c and saves with its stale cache.
        ctx_a.add_node(create_test_node("frag:c", "fragment"));
        store_a.save_context(&ctx_a).unwrap();

        // With incremental upserts, ALL three nodes must survive.
        // With DELETE-all, frag:b would be lost.
        let loaded = store_a.load_context(&ctx_id).unwrap().unwrap();
        assert!(loaded.nodes.contains_key(&NodeId::from_string("frag:a")),
            "frag:a must survive");
        assert!(loaded.nodes.contains_key(&NodeId::from_string("frag:b")),
            "frag:b must survive save_context from engine with stale cache");
        assert!(loaded.nodes.contains_key(&NodeId::from_string("frag:c")),
            "frag:c must be added by the stale engine");
    }

    #[test]
    fn test_incremental_save_upserts_modified_nodes() {
        let store = create_test_store();
        let mut ctx = Context::new("project");
        let ctx_id = ctx.id.clone();

        let mut node = create_test_node("concept:travel", "concept");
        node.properties.insert("source_count".to_string(), PropertyValue::Int(1));
        ctx.add_node(node);
        store.save_context(&ctx).unwrap();

        // Update the node property
        let node = ctx.nodes.get_mut(&NodeId::from_string("concept:travel")).unwrap();
        node.properties.insert("source_count".to_string(), PropertyValue::Int(2));
        store.save_context(&ctx).unwrap();

        let loaded = store.load_context(&ctx_id).unwrap().unwrap();
        let travel = loaded.nodes.get(&NodeId::from_string("concept:travel")).unwrap();
        assert_eq!(travel.properties.get("source_count"), Some(&PropertyValue::Int(2)));
        // No duplicates
        assert_eq!(loaded.nodes.len(), 1);
    }

    #[test]
    fn test_incremental_save_preserves_edge_contributions() {
        let store = create_test_store();
        let mut ctx = Context::new("project");
        let ctx_id = ctx.id.clone();

        let frag = create_test_node("frag:1", "fragment");
        let concept = create_test_node("concept:travel", "concept");
        ctx.add_node(frag.clone());
        ctx.add_node(concept.clone());

        let mut edge = Edge::new(frag.id.clone(), concept.id.clone(), "tagged_with");
        edge.contributions.insert("fragment:manual".to_string(), 1.0);
        ctx.add_edge(edge);
        store.save_context(&ctx).unwrap();

        let loaded = store.load_context(&ctx_id).unwrap().unwrap();
        assert_eq!(loaded.edges.len(), 1);
        assert_eq!(loaded.edges[0].contributions.get("fragment:manual"), Some(&1.0));
    }

    #[test]
    fn test_incremental_save_handles_node_removal() {
        let store = create_test_store();
        let mut ctx = Context::new("project");
        let ctx_id = ctx.id.clone();

        ctx.add_node(create_test_node("concept:a", "concept"));
        ctx.add_node(create_test_node("concept:b", "concept"));
        store.save_context(&ctx).unwrap();

        // Remove concept:b from the in-memory context
        ctx.nodes.remove(&NodeId::from_string("concept:b"));
        store.save_context(&ctx).unwrap();

        let loaded = store.load_context(&ctx_id).unwrap().unwrap();
        assert!(loaded.nodes.contains_key(&NodeId::from_string("concept:a")));
        assert!(!loaded.nodes.contains_key(&NodeId::from_string("concept:b")),
            "concept:b must be removed from database after save_context");
    }

    #[test]
    fn test_incremental_save_handles_edge_removal() {
        let store = create_test_store();
        let mut ctx = Context::new("project");
        let ctx_id = ctx.id.clone();

        let frag = create_test_node("frag:1", "fragment");
        let concept = create_test_node("concept:travel", "concept");
        ctx.add_node(frag.clone());
        ctx.add_node(concept.clone());

        let edge = Edge::new(frag.id.clone(), concept.id.clone(), "tagged_with");
        ctx.add_edge(edge);
        store.save_context(&ctx).unwrap();

        // Remove the edge
        ctx.edges.clear();
        store.save_context(&ctx).unwrap();

        let loaded = store.load_context(&ctx_id).unwrap().unwrap();
        assert!(loaded.edges.is_empty(), "edge must be removed after save_context");
        // Nodes should still exist
        assert_eq!(loaded.nodes.len(), 2);
    }

    // ========================================================================
    // ADR-017 §2: Cache Coherence via data_version
    // ========================================================================

    #[test]
    fn test_data_version_changes_after_external_write() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("coherence.db");

        let store_a = SqliteStore::open(&db_path).unwrap();
        let store_b = SqliteStore::open(&db_path).unwrap();

        let ctx = Context::new("shared");
        let ctx_id = ctx.id.clone();
        store_a.save_context(&ctx).unwrap();

        let v1 = store_a.data_version().unwrap();

        // External write via store_b
        let mut ctx_b = store_b.load_context(&ctx_id).unwrap().unwrap();
        ctx_b.add_node(create_test_node("node:ext", "concept"));
        store_b.save_context(&ctx_b).unwrap();

        let v2 = store_a.data_version().unwrap();
        assert_ne!(v1, v2, "data_version must change after external write");
    }

    #[test]
    fn test_data_version_unchanged_without_writes() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("no-change.db");

        let store = SqliteStore::open(&db_path).unwrap();
        let ctx = Context::new("test");
        store.save_context(&ctx).unwrap();

        let v1 = store.data_version().unwrap();
        let v2 = store.data_version().unwrap();
        assert_eq!(v1, v2, "data_version must be stable when no external writes occur");
    }
}
