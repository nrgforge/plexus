//! Query-time normalization strategies
//!
//! Raw weights are stored. Normalized weights are computed at query time
//! via a pluggable NormalizationStrategy. The graph stores ground truth;
//! normalization is an interpretive lens.

use crate::graph::{Context, Edge, NodeId};
use std::collections::HashMap;

/// A normalized edge weight result.
#[derive(Debug, Clone)]
pub struct NormalizedEdge {
    pub edge: Edge,
    pub normalized_weight: f64,
}

/// Pluggable normalization strategy.
///
/// Different consumers can apply different strategies to the same raw weights.
pub trait NormalizationStrategy: Send + Sync {
    /// Compute normalized weights for all outgoing edges from a node.
    ///
    /// Returns (edge, normalized_weight) pairs.
    fn normalize(&self, node_id: &NodeId, context: &Context) -> Vec<NormalizedEdge>;
}

/// Default: per-node outgoing divisive normalization.
///
/// `w_normalized(i→j) = w_raw(i→j) / Σ_k w_raw(i→k)`
///
/// Hebbian weakening emerges naturally: when a new edge from node A is
/// added, every other outgoing edge from A becomes relatively weaker
/// in the normalized view without mutating those edges.
pub struct OutgoingDivisive;

impl NormalizationStrategy for OutgoingDivisive {
    fn normalize(&self, node_id: &NodeId, context: &Context) -> Vec<NormalizedEdge> {
        let outgoing: Vec<&Edge> = context
            .edges()
            .filter(|e| &e.source == node_id)
            .collect();

        if outgoing.is_empty() {
            return Vec::new();
        }

        let sum: f64 = outgoing.iter().map(|e| e.raw_weight as f64).sum();

        if sum == 0.0 {
            return outgoing
                .into_iter()
                .map(|e| NormalizedEdge {
                    edge: e.clone(),
                    normalized_weight: 0.0,
                })
                .collect();
        }

        outgoing
            .into_iter()
            .map(|e| NormalizedEdge {
                normalized_weight: e.raw_weight as f64 / sum,
                edge: e.clone(),
            })
            .collect()
    }
}

/// Softmax normalization: `exp(w_i) / Σ_k exp(w_k)`
///
/// Produces different values than divisive normalization, emphasizing
/// differences between weights.
pub struct Softmax;

impl NormalizationStrategy for Softmax {
    fn normalize(&self, node_id: &NodeId, context: &Context) -> Vec<NormalizedEdge> {
        let outgoing: Vec<&Edge> = context
            .edges()
            .filter(|e| &e.source == node_id)
            .collect();

        if outgoing.is_empty() {
            return Vec::new();
        }

        // For numerical stability, subtract max before exp
        let max_w = outgoing
            .iter()
            .map(|e| e.raw_weight)
            .fold(f32::NEG_INFINITY, f32::max) as f64;

        let exp_weights: Vec<f64> = outgoing
            .iter()
            .map(|e| (e.raw_weight as f64 - max_w).exp())
            .collect();

        let sum_exp: f64 = exp_weights.iter().sum();

        outgoing
            .into_iter()
            .zip(exp_weights)
            .map(|(e, exp_w)| NormalizedEdge {
                normalized_weight: exp_w / sum_exp,
                edge: e.clone(),
            })
            .collect()
    }
}

/// Convenience: get all normalized outgoing weights from a node as a map.
pub fn normalized_weights(
    strategy: &dyn NormalizationStrategy,
    node_id: &NodeId,
    context: &Context,
) -> HashMap<(NodeId, NodeId), f64> {
    strategy
        .normalize(node_id, context)
        .into_iter()
        .map(|ne| ((ne.edge.source.clone(), ne.edge.target.clone()), ne.normalized_weight))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ContentType, Edge, Node, NodeId};

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    fn edge_weighted(source: &str, target: &str, raw_weight: f32) -> Edge {
        let mut e = Edge::new(
            NodeId::from_string(source),
            NodeId::from_string(target),
            "related_to",
        );
        e.raw_weight = raw_weight;
        e
    }

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    // === Scenario: Default per-node outgoing divisive normalization ===
    #[test]
    fn default_divisive_normalization() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_node(node("D"));
        ctx.add_edge(edge_weighted("A", "B", 3.0));
        ctx.add_edge(edge_weighted("A", "C", 1.0));
        ctx.add_edge(edge_weighted("A", "D", 1.0));

        let strategy = OutgoingDivisive;
        let weights = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);

        let ab = *weights.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();
        let ac = *weights.get(&(NodeId::from_string("A"), NodeId::from_string("C"))).unwrap();
        let ad = *weights.get(&(NodeId::from_string("A"), NodeId::from_string("D"))).unwrap();

        assert!(approx_eq(ab, 0.6));  // 3/5
        assert!(approx_eq(ac, 0.2));  // 1/5
        assert!(approx_eq(ad, 0.2));  // 1/5
    }

    // === Scenario: Adding edge weakens existing in normalized view without mutation ===
    #[test]
    fn adding_edge_weakens_existing_normalized() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_edge(edge_weighted("A", "B", 3.0));
        ctx.add_edge(edge_weighted("A", "C", 2.0));

        let strategy = OutgoingDivisive;

        // Before adding D
        let before = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);
        let ab_before = *before.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();
        let ac_before = *before.get(&(NodeId::from_string("A"), NodeId::from_string("C"))).unwrap();
        assert!(approx_eq(ab_before, 0.6));
        assert!(approx_eq(ac_before, 0.4));

        // Add new edge A→D with raw weight 5.0
        ctx.add_node(node("D"));
        ctx.add_edge(edge_weighted("A", "D", 5.0));

        let after = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);
        let ab_after = *after.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();
        let ac_after = *after.get(&(NodeId::from_string("A"), NodeId::from_string("C"))).unwrap();
        let ad_after = *after.get(&(NodeId::from_string("A"), NodeId::from_string("D"))).unwrap();

        assert!(approx_eq(ab_after, 0.3));  // 3/10
        assert!(approx_eq(ac_after, 0.2));  // 2/10
        assert!(approx_eq(ad_after, 0.5));  // 5/10

        // Raw weights unchanged
        let ab_raw = ctx.edges().find(|e| e.target == NodeId::from_string("B")).unwrap().raw_weight;
        assert_eq!(ab_raw, 3.0);
    }

    // === Scenario: Quiet graph stays stable ===
    #[test]
    fn quiet_graph_stays_stable() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_edge(edge_weighted("A", "B", 3.0));
        ctx.add_edge(edge_weighted("A", "C", 2.0));

        let strategy = OutgoingDivisive;

        let w1 = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);
        // "Time passes, nothing happens"
        let w2 = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);

        assert_eq!(w1.len(), w2.len());
        for (key, v1) in &w1 {
            let v2 = w2.get(key).unwrap();
            assert!(approx_eq(*v1, *v2));
        }
    }

    // === Scenario: Different strategies produce different results ===
    #[test]
    fn different_strategies_different_results() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_edge(edge_weighted("A", "B", 3.0));
        ctx.add_edge(edge_weighted("A", "C", 1.0));

        let divisive = OutgoingDivisive;
        let softmax = Softmax;

        let div_weights = normalized_weights(&divisive, &NodeId::from_string("A"), &ctx);
        let sm_weights = normalized_weights(&softmax, &NodeId::from_string("A"), &ctx);

        let div_ab = *div_weights.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();
        let sm_ab = *sm_weights.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();

        // Divisive: 3/4 = 0.75
        assert!(approx_eq(div_ab, 0.75));
        // Softmax: exp(3)/(exp(3)+exp(1)) ≈ 0.8808
        assert!(!approx_eq(div_ab, sm_ab), "strategies should produce different values");

        // But raw weights are the same for both
        let ab_raw = ctx.edges().find(|e| e.target == NodeId::from_string("B")).unwrap().raw_weight;
        assert_eq!(ab_raw, 3.0);
    }

    // === Scenario: Single outgoing edge normalizes to 1.0 ===
    #[test]
    fn single_outgoing_edge_normalizes_to_one() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_edge(edge_weighted("A", "B", 7.0));

        let strategy = OutgoingDivisive;
        let weights = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);

        let ab = *weights.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();
        assert!(approx_eq(ab, 1.0));
    }

    // === Scenario: Normalization is per-node, not global ===
    #[test]
    fn normalization_is_per_node() {
        let mut ctx = Context::new("test");
        ctx.add_node(node("A"));
        ctx.add_node(node("B"));
        ctx.add_node(node("C"));
        ctx.add_node(node("D"));
        ctx.add_edge(edge_weighted("A", "B", 100.0));
        ctx.add_edge(edge_weighted("C", "D", 1.0));

        let strategy = OutgoingDivisive;

        let a_weights = normalized_weights(&strategy, &NodeId::from_string("A"), &ctx);
        let c_weights = normalized_weights(&strategy, &NodeId::from_string("C"), &ctx);

        let ab = *a_weights.get(&(NodeId::from_string("A"), NodeId::from_string("B"))).unwrap();
        let cd = *c_weights.get(&(NodeId::from_string("C"), NodeId::from_string("D"))).unwrap();

        // Both normalize to 1.0 — each has single outgoing edge
        assert!(approx_eq(ab, 1.0));
        assert!(approx_eq(cd, 1.0));
        // High raw weight on A→B does NOT suppress C→D
    }
}
