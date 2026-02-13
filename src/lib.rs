//! Plexus: Network-Aware Knowledge Graph Engine
//!
//! A high-performance, content-agnostic knowledge graph engine that implements
//! network science principles, collective intelligence, and self-reinforcing validation.
//!
//! # Core Concepts
//!
//! - **Nodes**: Universal representation of entities (functions, concepts, etc.)
//! - **Edges**: Directed connections with raw weights (Hebbian reinforcement, no temporal decay)
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

pub mod adapter;
pub mod analysis;
mod graph;
pub mod mcp;
pub mod provenance;
pub mod query;
pub mod storage;

pub use adapter::{
    AdapterError, AdapterSink, Annotation, AnnotatedEdge, AnnotatedNode,
    EmitResult, Emission, Rejection, RejectionReason, Removal,
};
pub use analysis::{
    AnalysisCapability, AnalysisConfig, AnalysisError, AnalysisOrchestrator, AnalysisResult,
    AnalysisScope, AnalyzerRegistry, ConflictStrategy, ContentAnalyzer, ContentId, ContentItem,
    GraphMutation, ResultMerger, SubGraph,
};
pub use graph::{
    ContentType, Context, ContextId, ContextMetadata, Edge, EdgeId, Node, NodeId, PlexusEngine,
    PlexusError, PlexusResult, PropertyValue, Source, dimension,
};
pub use query::{Direction, EvidenceTrailResult, FindQuery, PathQuery, PathResult, QueryResult, StepQuery, StepResult, TraversalResult, TraverseQuery, evidence_trail};
pub use provenance::{ChainStatus, ChainView, MarkView, ProvenanceApi};
pub use storage::{GraphStore, OpenStore, SqliteStore, StorageError, StorageResult};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
