//! Core graph data structures

mod context;
mod edge;
mod engine;
mod node;

#[cfg(test)]
mod tests;

pub use context::{Context, ContextId};
pub use edge::{Edge, EdgeId};
pub use engine::{PlexusEngine, PlexusError, PlexusResult};
pub use node::{Node, NodeId, PropertyValue};

// Re-export for future use (allow unused for now)
#[allow(unused_imports)]
pub use edge::{Reinforcement, ReinforcementSource, ReinforcementType};
#[allow(unused_imports)]
pub use node::ContentType;

// Re-export dimension constants for multi-dimensional graph support (Phase 5.0)
// Will be used in Phase 5.1+ (Analyzer Framework)
#[allow(unused_imports)]
pub use node::dimension;
