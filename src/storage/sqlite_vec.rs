//! SqliteVecStore — persistent vector storage via sqlite-vec (ADR-026)
//!
//! Implements the `VectorStore` trait using a sqlite-vec virtual table
//! for KNN vector search. Each context gets its own partition via the
//! `context_id TEXT PARTITION KEY` column, ensuring context isolation.
//!
//! Vectors are L2-normalized on insert so that L2 distance from vec0
//! can be converted to cosine similarity: `sim = 1 - dist² / 2`.
//!
//! Uses its own database connection (WAL mode) to avoid contention with
//! the main `SqliteStore` connection.

#[cfg(feature = "embeddings")]
mod inner {
    use crate::adapter::embedding::VectorStore;
    use crate::graph::NodeId;
    use rusqlite::Connection;
    use sqlite_vec::sqlite3_vec_init;
    use std::path::Path;
    use std::sync::Mutex;

    /// Default embedding dimensions (nomic-embed-text-v1.5 produces 768-dim vectors).
    pub const DEFAULT_EMBEDDING_DIMENSIONS: usize = 768;

    /// Persistent vector store backed by sqlite-vec.
    ///
    /// Stores embedding vectors in a vec0 virtual table with context-scoped
    /// partitions. Vectors are L2-normalized on insert; KNN queries use L2
    /// distance converted to cosine similarity.
    pub struct SqliteVecStore {
        conn: Mutex<Connection>,
        dimensions: usize,
    }

    /// Register the sqlite-vec extension globally (safe under parallel test execution).
    fn register_vec_extension() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            unsafe {
                rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                    sqlite3_vec_init as *const (),
                )));
            }
        });
    }

    impl SqliteVecStore {
        /// Open a persistent vector store at the given path.
        ///
        /// Creates the vec0 virtual table if it doesn't exist.
        /// Uses WAL mode for concurrent read access.
        pub fn open(path: &Path, dimensions: usize) -> Result<Self, String> {
            register_vec_extension();
            let conn = Connection::open(path).map_err(|e| e.to_string())?;
            Self::init_connection(conn, dimensions)
        }

        /// Open an in-memory vector store (for tests).
        pub fn open_in_memory(dimensions: usize) -> Result<Self, String> {
            register_vec_extension();
            let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
            Self::init_connection(conn, dimensions)
        }

        fn init_connection(conn: Connection, dimensions: usize) -> Result<Self, String> {
            // WAL mode for concurrent reads
            conn.execute_batch("PRAGMA journal_mode=WAL;")
                .map_err(|e| e.to_string())?;

            // Create the vec0 virtual table (L2 distance, the default)
            let create_sql = format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS vec_embeddings USING vec0(\
                     context_id TEXT PARTITION KEY,\
                     node_id TEXT,\
                     embedding float[{}]\
                 )",
                dimensions
            );
            conn.execute_batch(&create_sql)
                .map_err(|e| e.to_string())?;

            Ok(Self {
                conn: Mutex::new(conn),
                dimensions,
            })
        }

        /// The dimensionality of vectors stored in this store.
        pub fn dimensions(&self) -> usize {
            self.dimensions
        }
    }

    /// Reinterpret a `&[f32]` slice as raw bytes for sqlite-vec blob parameters.
    ///
    /// # Safety
    /// f32 has no padding and a fixed layout; this is a trivial reinterpretation.
    fn f32_slice_as_bytes(slice: &[f32]) -> &[u8] {
        unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() * 4) }
    }

    /// L2-normalize a vector in place.
    fn l2_normalize(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    impl VectorStore for SqliteVecStore {
        fn store(&self, context_id: &str, node_id: &NodeId, vector: Vec<f32>) {
            let mut normalized = vector;
            l2_normalize(&mut normalized);
            let conn = self.conn.lock().unwrap();
            let bytes = f32_slice_as_bytes(&normalized);
            conn.execute(
                "INSERT OR REPLACE INTO vec_embeddings(context_id, node_id, embedding) \
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![context_id, node_id.as_str(), bytes],
            )
            .expect("vec_embeddings INSERT failed");
        }

        fn has(&self, context_id: &str, node_id: &NodeId) -> bool {
            let conn = self.conn.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM vec_embeddings \
                     WHERE context_id = ?1 AND node_id = ?2",
                    rusqlite::params![context_id, node_id.as_str()],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            count > 0
        }

        fn find_similar(
            &self,
            context_id: &str,
            query: &[f32],
            threshold: f32,
        ) -> Vec<(NodeId, f32)> {
            let mut normalized_query = query.to_vec();
            l2_normalize(&mut normalized_query);
            let conn = self.conn.lock().unwrap();
            let bytes = f32_slice_as_bytes(&normalized_query);

            // KNN query using L2 distance on normalized vectors.
            // For unit vectors: L2_dist² = 2(1 - cos_sim), so cos_sim = 1 - dist²/2.
            let mut stmt = conn
                .prepare(
                    "SELECT node_id, distance \
                     FROM vec_embeddings \
                     WHERE embedding MATCH ?1 \
                       AND context_id = ?2 \
                       AND k = 100",
                )
                .expect("vec_embeddings KNN prepare failed");

            let results: Vec<(NodeId, f32)> = stmt
                .query_map(rusqlite::params![bytes, context_id], |row| {
                    let nid: String = row.get(0)?;
                    let distance: f32 = row.get(1)?;
                    Ok((nid, distance))
                })
                .expect("vec_embeddings KNN query failed")
                .filter_map(|r| r.ok())
                .filter_map(|(nid, distance)| {
                    // Convert L2 distance on normalized vectors to cosine similarity
                    let similarity = 1.0 - (distance * distance) / 2.0;
                    if similarity >= threshold {
                        Some((NodeId::from_string(nid), similarity))
                    } else {
                        None
                    }
                })
                .collect();

            results
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn store_and_has_round_trip() {
            let store =
                SqliteVecStore::open_in_memory(3).expect("should open in-memory store");
            let nid = NodeId::from_string("concept:travel");

            assert!(!store.has("ctx", &nid));
            store.store("ctx", &nid, vec![0.9, 0.3, 0.1]);
            assert!(store.has("ctx", &nid));
        }

        #[test]
        fn find_similar_above_threshold() {
            let store =
                SqliteVecStore::open_in_memory(3).expect("should open in-memory store");

            // Store two similar vectors and one dissimilar
            store.store(
                "ctx",
                &NodeId::from_string("concept:travel"),
                vec![0.9, 0.3, 0.1],
            );
            store.store(
                "ctx",
                &NodeId::from_string("concept:journey"),
                vec![0.85, 0.35, 0.15],
            );
            store.store(
                "ctx",
                &NodeId::from_string("concept:democracy"),
                vec![0.1, 0.2, 0.95],
            );

            // Query with a vector similar to travel/journey
            let results = store.find_similar("ctx", &[0.9, 0.3, 0.1], 0.9);

            // travel should match itself (similarity ~1.0)
            let travel_match = results
                .iter()
                .find(|(id, _)| id.as_str() == "concept:travel");
            assert!(travel_match.is_some(), "travel should match itself");

            // journey should also be similar (cosine sim > 0.9)
            let journey_match = results
                .iter()
                .find(|(id, _)| id.as_str() == "concept:journey");
            assert!(
                journey_match.is_some(),
                "journey should be similar to travel"
            );

            // democracy should NOT be similar at 0.9 threshold
            let democracy_match = results
                .iter()
                .find(|(id, _)| id.as_str() == "concept:democracy");
            assert!(
                democracy_match.is_none(),
                "democracy should not match travel at 0.9 threshold"
            );
        }

        #[test]
        fn find_similar_below_threshold_returns_nothing() {
            let store =
                SqliteVecStore::open_in_memory(3).expect("should open in-memory store");

            store.store(
                "ctx",
                &NodeId::from_string("concept:democracy"),
                vec![0.1, 0.2, 0.95],
            );

            // Query with an orthogonal vector
            let results = store.find_similar("ctx", &[0.9, 0.3, 0.1], 0.9);
            assert!(results.is_empty(), "dissimilar vectors should not match");
        }

        #[test]
        fn context_isolation() {
            let store =
                SqliteVecStore::open_in_memory(3).expect("should open in-memory store");

            // Store vector in context A
            store.store(
                "context-a",
                &NodeId::from_string("concept:travel"),
                vec![0.9, 0.3, 0.1],
            );

            // Query in context B — should not find context A's vectors
            let results = store.find_similar("context-b", &[0.9, 0.3, 0.1], 0.5);
            assert!(
                results.is_empty(),
                "vectors from context-a should not appear in context-b queries"
            );

            // has() should also respect context
            assert!(store.has("context-a", &NodeId::from_string("concept:travel")));
            assert!(!store.has("context-b", &NodeId::from_string("concept:travel")));
        }
    }
}

#[cfg(feature = "embeddings")]
pub use inner::{SqliteVecStore, DEFAULT_EMBEDDING_DIMENSIONS};
