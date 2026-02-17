//! TemporalProximityEnrichment — timestamp-based co-occurrence (ADR-024)
//!
//! Detects nodes with timestamps within a configurable threshold and emits
//! symmetric edge pairs with a configured output relationship. Critical for
//! EDDI's reactive performance needs.
//!
//! Structure-aware: fires based on timestamp property presence, not node
//! content type (Invariant 50). Idempotent: checks for existing edges
//! before emitting, so the enrichment loop reaches quiescence.

use crate::adapter::enrichment::Enrichment;
use crate::adapter::events::GraphEvent;
use crate::adapter::types::{AnnotatedEdge, Emission};
use crate::graph::{dimension, Context, Edge, NodeId, PropertyValue};

/// Enrichment that detects temporal proximity between nodes.
///
/// When two nodes have timestamps (via a configured property) within
/// `threshold_ms` of each other, emits a symmetric output relationship
/// edge pair.
///
/// Parameterizable on `timestamp_property`, `threshold_ms`, and
/// `output_relationship` (ADR-024).
pub struct TemporalProximityEnrichment {
    timestamp_property: String,
    threshold_ms: u64,
    output_relationship: String,
    id: String,
}

impl TemporalProximityEnrichment {
    pub fn new(timestamp_property: &str, threshold_ms: u64, output_relationship: &str) -> Self {
        Self {
            id: format!("temporal:{}:{}:{}", timestamp_property, threshold_ms, output_relationship),
            timestamp_property: timestamp_property.to_string(),
            threshold_ms,
            output_relationship: output_relationship.to_string(),
        }
    }
}

impl Enrichment for TemporalProximityEnrichment {
    fn id(&self) -> &str {
        &self.id
    }

    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission> {
        // Only run when nodes are added
        if !has_node_events(events) {
            return None;
        }

        let mut emission = Emission::new();

        // Collect all nodes with the timestamp property
        let timestamped: Vec<_> = context
            .nodes()
            .filter_map(|n| {
                extract_timestamp(&n.properties, &self.timestamp_property)
                    .map(|ts| (n.id.clone(), ts))
            })
            .collect();

        // Check all pairs for proximity
        for i in 0..timestamped.len() {
            for j in (i + 1)..timestamped.len() {
                let (ref id_a, ts_a) = timestamped[i];
                let (ref id_b, ts_b) = timestamped[j];

                let diff = if ts_a > ts_b { ts_a - ts_b } else { ts_b - ts_a };

                if diff > self.threshold_ms {
                    continue;
                }

                // Idempotent guard: skip if output edges already exist
                if !output_edge_exists(context, id_a, id_b, &self.output_relationship) {
                    let edge = Edge::new_in_dimension(
                        id_a.clone(),
                        id_b.clone(),
                        &self.output_relationship,
                        dimension::SEMANTIC,
                    );
                    emission = emission.with_edge(AnnotatedEdge::new(edge));
                }

                if !output_edge_exists(context, id_b, id_a, &self.output_relationship) {
                    let edge = Edge::new_in_dimension(
                        id_b.clone(),
                        id_a.clone(),
                        &self.output_relationship,
                        dimension::SEMANTIC,
                    );
                    emission = emission.with_edge(AnnotatedEdge::new(edge));
                }
            }
        }

        if emission.is_empty() {
            None
        } else {
            Some(emission)
        }
    }
}

/// Check if events include node additions.
fn has_node_events(events: &[GraphEvent]) -> bool {
    events.iter().any(|e| matches!(e, GraphEvent::NodesAdded { .. }))
}

/// Extract a numeric timestamp from a node's properties.
fn extract_timestamp(
    properties: &std::collections::HashMap<String, PropertyValue>,
    property_name: &str,
) -> Option<u64> {
    match properties.get(property_name)? {
        PropertyValue::Int(n) => Some(*n as u64),
        PropertyValue::Float(n) => Some(*n as u64),
        PropertyValue::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

/// Check if an output edge from source to target already exists.
fn output_edge_exists(context: &Context, source: &NodeId, target: &NodeId, relationship: &str) -> bool {
    context.edges().any(|e| {
        e.source == *source && e.target == *target && e.relationship == relationship
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Node};

    fn node_with_timestamp(id: &str, property: &str, value: u64) -> Node {
        let mut n = Node::new("gesture", ContentType::Document);
        n.id = NodeId::from_string(id);
        n.dimension = dimension::SEMANTIC.to_string();
        n.properties.insert(
            property.to_string(),
            PropertyValue::Int(value as i64),
        );
        n
    }

    fn nodes_added_event(ids: Vec<&str>) -> GraphEvent {
        GraphEvent::NodesAdded {
            node_ids: ids.into_iter().map(NodeId::from_string).collect(),
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }
    }

    // === Scenario 1: Temporal proximity detected between nodes within threshold ===

    #[test]
    fn proximity_detected_within_threshold() {
        let enrichment = TemporalProximityEnrichment::new("gesture_time", 500, "temporal_proximity");

        let mut ctx = Context::new("test");
        ctx.add_node(node_with_timestamp("node-a", "gesture_time", 1000));
        ctx.add_node(node_with_timestamp("node-b", "gesture_time", 1300));

        let emission = enrichment
            .enrich(&[nodes_added_event(vec!["node-b"])], &ctx)
            .expect("should emit temporal_proximity edges");

        assert_eq!(emission.edges.len(), 2);

        let a = NodeId::from_string("node-a");
        let b = NodeId::from_string("node-b");

        let forward = emission.edges.iter().find(|ae| {
            ae.edge.source == a && ae.edge.target == b
                && ae.edge.relationship == "temporal_proximity"
        });
        let reverse = emission.edges.iter().find(|ae| {
            ae.edge.source == b && ae.edge.target == a
                && ae.edge.relationship == "temporal_proximity"
        });

        assert!(forward.is_some(), "a→b temporal_proximity");
        assert!(reverse.is_some(), "b→a temporal_proximity");
    }

    // === Scenario 2: No temporal proximity when nodes exceed threshold ===

    #[test]
    fn no_proximity_when_exceeding_threshold() {
        let enrichment = TemporalProximityEnrichment::new("gesture_time", 500, "temporal_proximity");

        let mut ctx = Context::new("test");
        ctx.add_node(node_with_timestamp("node-a", "gesture_time", 1000));
        ctx.add_node(node_with_timestamp("node-b", "gesture_time", 2000));

        assert!(
            enrichment.enrich(&[nodes_added_event(vec!["node-b"])], &ctx).is_none(),
            "1000ms gap exceeds 500ms threshold"
        );
    }

    // === Scenario 3: Nodes without timestamp property are skipped ===

    #[test]
    fn skips_nodes_without_timestamp() {
        let enrichment = TemporalProximityEnrichment::new("gesture_time", 500, "temporal_proximity");

        let mut ctx = Context::new("test");
        // Node C has no gesture_time property
        let mut node_c = Node::new("gesture", ContentType::Document);
        node_c.id = NodeId::from_string("node-c");
        ctx.add_node(node_c);

        assert!(
            enrichment.enrich(&[nodes_added_event(vec!["node-c"])], &ctx).is_none(),
            "should skip nodes without timestamp property"
        );
    }

    // === Scenario 4: TemporalProximityEnrichment reaches quiescence ===

    #[test]
    fn reaches_quiescence() {
        let enrichment = TemporalProximityEnrichment::new("gesture_time", 500, "temporal_proximity");

        let mut ctx = Context::new("test");
        ctx.add_node(node_with_timestamp("node-a", "gesture_time", 1000));
        ctx.add_node(node_with_timestamp("node-b", "gesture_time", 1300));

        // Round 1: productive
        let emission = enrichment
            .enrich(&[nodes_added_event(vec!["node-b"])], &ctx)
            .expect("round 1 should emit");

        // Commit round 1 emissions
        for ae in &emission.edges {
            ctx.add_edge(ae.edge.clone());
        }

        // Round 2: quiescent (triggered by NodesAdded from edge commit won't happen,
        // but even if we re-trigger with NodesAdded, edges already exist)
        assert!(
            enrichment.enrich(&[nodes_added_event(vec!["node-a", "node-b"])], &ctx).is_none(),
            "round 2 should be quiescent"
        );
    }

    // === Scenario 5: TemporalProximityEnrichment has unique stable ID ===

    #[test]
    fn stable_id() {
        let enrichment = TemporalProximityEnrichment::new("gesture_time", 500, "temporal_proximity");
        assert_eq!(enrichment.id(), "temporal:gesture_time:500:temporal_proximity");
    }
}
