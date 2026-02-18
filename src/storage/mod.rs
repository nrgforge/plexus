//! Storage backends for Plexus
//!
//! Plexus supports multiple storage backends through the `GraphStore` trait.
//! The primary implementation is `SqliteStore` for persistent storage.

mod sqlite;
#[cfg(feature = "embeddings")]
mod sqlite_vec;
mod traits;

pub use sqlite::SqliteStore;
pub use traits::{EdgeFilter, GraphStore, NodeFilter, OpenStore, StorageError, StorageResult, Subgraph};
#[cfg(feature = "embeddings")]
pub use sqlite_vec::{SqliteVecStore, DEFAULT_EMBEDDING_DIMENSIONS};
