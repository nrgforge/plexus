//! Edge explanation — "why is this connection here?" in one call (issue #14).
//!
//! Both M3 blinded-consumer probes completed finding and ranking entirely
//! in their own lens vocabulary but had to manufacture explanations from
//! three separate raw-internal queries. This module assembles the full
//! evidence picture for a node pair: every edge between them (parallel
//! edges included), each with contributions, corroboration, and lens
//! contribution keys parsed back into source relationships.

use crate::graph::{Context, NodeId, PropertyValue};
use serde::Serialize;
use std::collections::HashMap;

/// An endpoint of the explained pair, with enough content to display.
#[derive(Debug, Clone, Serialize)]
pub struct ExplainedNode {
    pub id: String,
    pub node_type: String,
    pub dimension: String,
    /// The node's `text` or `label` property, when present — what a
    /// consumer would show a user.
    pub text: Option<String>,
}

/// One edge between the pair, with its full evidence.
#[derive(Debug, Clone, Serialize)]
pub struct ExplainedEdge {
    pub relationship: String,
    /// Direction relative to the requested (source, target) order.
    pub direction: Direction,
    pub raw_weight: f32,
    /// Stored contribution slots: contributor id → value (Invariant 8's
    /// stored layer — the honest numbers, pre-normalization).
    pub contributions: HashMap<String, f32>,
    /// Distinct contributors (user-facing: corroboration).
    pub corroboration: usize,
    /// For lens edges: the source relationships this translation merged,
    /// parsed from `lens:{consumer}:{to}:{from}` contribution keys.
    pub translated_from: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Forward,
    Reverse,
}

/// The full explanation for a node pair.
#[derive(Debug, Clone, Serialize)]
pub struct EdgeExplanation {
    pub source: ExplainedNode,
    pub target: ExplainedNode,
    pub edges: Vec<ExplainedEdge>,
}

fn explained_node(context: &Context, id: &NodeId) -> Option<ExplainedNode> {
    let node = context.get_node(id)?;
    let text = ["text", "label"].iter().find_map(|k| {
        node.properties.get(*k).and_then(|v| match v {
            PropertyValue::String(s) => Some(s.clone()),
            _ => None,
        })
    });
    Some(ExplainedNode {
        id: id.to_string(),
        node_type: node.node_type.clone(),
        dimension: node.dimension.clone(),
        text,
    })
}

/// Parse `lens:{consumer}:{to}:{from}` contribution keys into the source
/// relationships a lens translation merged. Non-lens edges return empty.
fn translated_from(relationship: &str, contributions: &HashMap<String, f32>) -> Vec<String> {
    if !relationship.starts_with("lens:") {
        return Vec::new();
    }
    let prefix = format!("{}:", relationship);
    let mut from: Vec<String> = contributions
        .keys()
        .filter_map(|k| k.strip_prefix(&prefix).map(str::to_string))
        .collect();
    from.sort();
    from
}

/// Assemble the explanation for a pair. `relationship` optionally narrows
/// to one relationship; by default every edge between the pair (both
/// directions) is included — the parallel edges are the evidence.
pub fn explain_pair(
    context: &Context,
    source: &NodeId,
    target: &NodeId,
    relationship: Option<&str>,
) -> Option<EdgeExplanation> {
    let source_node = explained_node(context, source)?;
    let target_node = explained_node(context, target)?;

    let edges = context
        .edges
        .iter()
        .filter(|e| {
            (e.source == *source && e.target == *target)
                || (e.source == *target && e.target == *source)
        })
        .filter(|e| relationship.is_none_or(|r| e.relationship == r))
        .map(|e| ExplainedEdge {
            relationship: e.relationship.clone(),
            direction: if e.source == *source {
                Direction::Forward
            } else {
                Direction::Reverse
            },
            raw_weight: e.combined_weight,
            contributions: e.contributions.clone(),
            corroboration: e.contributions.len(),
            translated_from: translated_from(&e.relationship, &e.contributions),
        })
        .collect();

    Some(EdgeExplanation {
        source: source_node,
        target: target_node,
        edges,
    })
}
