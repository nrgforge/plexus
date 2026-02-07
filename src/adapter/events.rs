//! Graph events fired when emissions are committed
//!
//! Five low-level event types, one per mutation kind.
//! Higher-level events are modeled as nodes/edges from reflexive adapters.

use crate::graph::{EdgeId, NodeId};

/// A graph event fired when an emission is committed.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphEvent {
    /// Nodes were added or upserted
    NodesAdded {
        node_ids: Vec<NodeId>,
        adapter_id: String,
        context_id: String,
    },
    /// Edges were added
    EdgesAdded {
        edge_ids: Vec<EdgeId>,
        adapter_id: String,
        context_id: String,
    },
    /// Nodes were removed
    NodesRemoved {
        node_ids: Vec<NodeId>,
        adapter_id: String,
        context_id: String,
    },
    /// Edges were removed (including cascades from node removal)
    EdgesRemoved {
        edge_ids: Vec<EdgeId>,
        adapter_id: String,
        context_id: String,
        /// "cascade" when caused by node removal, "direct" otherwise
        reason: String,
    },
    /// Edge weights were changed (fires when an adapter's contribution differs from stored value)
    WeightsChanged {
        edge_ids: Vec<EdgeId>,
        adapter_id: String,
        context_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_event_variants_constructible() {
        let event = GraphEvent::NodesAdded {
            node_ids: vec![NodeId::from_string("A")],
            adapter_id: "test-adapter".to_string(),
            context_id: "ctx-1".to_string(),
        };
        match event {
            GraphEvent::NodesAdded { node_ids, .. } => {
                assert_eq!(node_ids.len(), 1);
            }
            _ => panic!("wrong variant"),
        }
    }
}
