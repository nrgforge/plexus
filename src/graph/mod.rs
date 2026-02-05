//! Core graph data structures

mod context;
mod edge;
mod engine;
mod node;

#[cfg(test)]
mod tests;

pub use context::{Context, ContextId, ContextMetadata, Source};
pub use edge::{Edge, EdgeId};
pub use engine::{PlexusEngine, PlexusError, PlexusResult};
pub use node::{Node, NodeId, PropertyValue};

#[allow(unused_imports)]
pub use node::ContentType;

pub use node::dimension;
