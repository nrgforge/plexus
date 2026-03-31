//! Query system for Plexus knowledge graphs
//!
//! Provides capabilities for finding nodes, traversing edges,
//! computing paths through the graph, and cursor-based change queries.

mod cursor;
mod filter;
mod find;
mod normalize;
mod path;
mod shared;
mod step;
mod traverse;
mod types;

pub use cursor::{PersistedEvent, ChangeSet, CursorFilter};
pub use filter::{QueryFilter, RankBy};
pub use find::FindQuery;
pub use normalize::{NormalizationStrategy, NormalizedEdge, OutgoingDivisive, Softmax, normalized_weights};
pub use path::PathQuery;
pub use step::{EvidenceTrailResult, StepQuery, StepResult, evidence_trail};
pub use shared::shared_concepts;
pub use traverse::TraverseQuery;
pub use types::{QueryResult, TraversalResult, PathResult, Direction};
