//! Cross-context queries

use crate::graph::{Context, NodeId};
use std::collections::HashSet;

/// Return concept node IDs that exist in both contexts.
///
/// Uses deterministic ID intersection (ADR-017 §4): nodes with
/// `node_type == "concept"` whose IDs appear in both contexts.
pub fn shared_concepts(ctx_a: &Context, ctx_b: &Context) -> Vec<NodeId> {
    let concepts_a: HashSet<&NodeId> = ctx_a
        .nodes
        .iter()
        .filter(|(_, n)| n.node_type == "concept")
        .map(|(id, _)| id)
        .collect();

    ctx_b
        .nodes
        .iter()
        .filter(|(id, n)| n.node_type == "concept" && concepts_a.contains(id))
        .map(|(id, _)| id.clone())
        .collect()
}
