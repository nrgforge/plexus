//! Re-export of GraphEvent from graph module.
//!
//! GraphEvent now lives in `graph::events` (its natural home in the inner layer).
//! This module re-exports it for backward compatibility within adapter/.

pub use crate::graph::events::GraphEvent;
