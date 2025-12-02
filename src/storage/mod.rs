//! Storage backends for Plexus
//!
//! Plexus supports multiple storage backends through the `GraphStore` trait.
//! The primary implementation is `SqliteStore` for persistent storage.

mod sqlite;
mod traits;

pub use sqlite::SqliteStore;
pub use traits::{EdgeFilter, GraphStore, NodeFilter, OpenStore, StorageError, StorageResult, Subgraph};
