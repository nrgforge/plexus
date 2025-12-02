//! Plexus: Network-Aware Knowledge Graph Engine
//!
//! A high-performance, content-agnostic knowledge graph engine that implements
//! network science principles, collective intelligence, and self-reinforcing validation.
//!
//! # Core Concepts
//!
//! - **Nodes**: Universal representation of entities (functions, concepts, etc.)
//! - **Edges**: Relationships with self-reinforcing strength that decay over time
//! - **Contexts**: Bounded subgraphs representing workspaces or projects
//!
//! # Example
//!
//! ```
//! use plexus::PlexusEngine;
//!
//! let engine = PlexusEngine::new();
//! // Engine is ready for use
//! ```

mod graph;

pub use graph::{
    Context, ContextId, Edge, EdgeId, Node, NodeId, PlexusEngine, PropertyValue,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
