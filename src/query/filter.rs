//! Composable query filters (ADR-034).
//!
//! `QueryFilter` composes with any query primitive. All fields are AND-composed:
//! an edge must pass all non-`None` predicates.

use crate::graph::Edge;

/// Composable filter for provenance-scoped and corroboration-based filtering.
///
/// When `None`, the field applies no constraint. When present, edges must satisfy
/// all non-`None` predicates (AND semantics).
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    /// Only include edges with a contribution from at least one of these IDs.
    pub contributor_ids: Option<Vec<String>>,
    /// Only include edges whose relationship starts with this prefix.
    pub relationship_prefix: Option<String>,
    /// Minimum corroboration: edges must have at least this many distinct contributors.
    pub min_corroboration: Option<usize>,
}

impl QueryFilter {
    /// Check whether an edge passes all filter predicates.
    pub fn edge_passes(&self, edge: &Edge) -> bool {
        // contributor_ids: edge passes if contributions keys intersect
        if let Some(ref ids) = self.contributor_ids {
            let has_match = ids.iter().any(|id| edge.contributions.contains_key(id));
            if !has_match {
                return false;
            }
        }

        // relationship_prefix: edge passes if relationship starts with prefix
        if let Some(ref prefix) = self.relationship_prefix {
            if !edge.relationship.starts_with(prefix.as_str()) {
                return false;
            }
        }

        // min_corroboration: edge passes if contributions count >= threshold
        if let Some(min) = self.min_corroboration {
            if edge.contributions.len() < min {
                return false;
            }
        }

        true
    }
}

/// Ranking dimension for query results.
///
/// The `NormalizedWeight` variant carries a pluggable normalization strategy
/// (ADR-034). Trait objects do not auto-derive `Clone`, `Copy`, `PartialEq`,
/// or `Debug`; this enum drops the first three (unused in the current code
/// paths) and implements `Debug` manually to avoid pushing `Debug` into the
/// `NormalizationStrategy` trait bound.
pub enum RankBy {
    /// Raw weight (sum of contributions after scale normalization).
    RawWeight,
    /// Number of distinct contributors to the edge.
    Corroboration,
    /// Normalized weight, computed at query time via the injected strategy
    /// (ADR-034). The strategy computes per-node normalization: an edge's
    /// score becomes its share of its source node's total outgoing weight.
    /// This surfaces relative strength — an edge that is 100% of its
    /// source's neighborhood outranks a raw-heavier edge that is only 30%
    /// of its source's neighborhood.
    NormalizedWeight(Box<dyn crate::query::normalize::NormalizationStrategy>),
}

impl std::fmt::Debug for RankBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RawWeight => f.write_str("RawWeight"),
            Self::Corroboration => f.write_str("Corroboration"),
            Self::NormalizedWeight(_) => f.write_str("NormalizedWeight(<strategy>)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, NodeId};

    fn edge_with_contributions(relationship: &str, contrib_keys: &[&str]) -> Edge {
        let mut e = Edge::new(
            NodeId::from_string("a"),
            NodeId::from_string("b"),
            relationship,
        );
        for key in contrib_keys {
            e.contributions.insert(key.to_string(), 1.0);
        }
        e.combined_weight = contrib_keys.len() as f32;
        e
    }

    #[test]
    fn default_filter_passes_all() {
        let filter = QueryFilter::default();
        let edge = edge_with_contributions("may_be_related", &["adapter-1"]);
        assert!(filter.edge_passes(&edge));
    }

    #[test]
    fn contributor_ids_filters_by_contribution_key() {
        let filter = QueryFilter {
            contributor_ids: Some(vec!["adapter-1".into()]),
            ..Default::default()
        };

        let match_edge = edge_with_contributions("r", &["adapter-1", "adapter-2"]);
        let miss_edge = edge_with_contributions("r", &["adapter-3"]);

        assert!(filter.edge_passes(&match_edge));
        assert!(!filter.edge_passes(&miss_edge));
    }

    #[test]
    fn relationship_prefix_filters_by_prefix() {
        let filter = QueryFilter {
            relationship_prefix: Some("lens:trellis:".into()),
            ..Default::default()
        };

        let match_edge = edge_with_contributions("lens:trellis:thematic_connection", &["a"]);
        let miss_edge = edge_with_contributions("may_be_related", &["a"]);

        assert!(filter.edge_passes(&match_edge));
        assert!(!filter.edge_passes(&miss_edge));
    }

    #[test]
    fn min_corroboration_filters_by_contributor_count() {
        let filter = QueryFilter {
            min_corroboration: Some(2),
            ..Default::default()
        };

        let pass_edge = edge_with_contributions("r", &["a", "b", "c"]);
        let fail_edge = edge_with_contributions("r", &["a"]);

        assert!(filter.edge_passes(&pass_edge));
        assert!(!filter.edge_passes(&fail_edge));
    }

    #[test]
    fn all_fields_compose_with_and_semantics() {
        let filter = QueryFilter {
            contributor_ids: Some(vec!["lens:trellis:thematic_connection:may_be_related".into()]),
            relationship_prefix: Some("lens:trellis:".into()),
            min_corroboration: Some(2),
        };

        // Passes all three
        let pass_edge = edge_with_contributions(
            "lens:trellis:thematic_connection",
            &["lens:trellis:thematic_connection:may_be_related", "adapter-2"],
        );
        assert!(filter.edge_passes(&pass_edge));

        // Fails contributor_ids
        let fail_contrib = edge_with_contributions(
            "lens:trellis:thematic_connection",
            &["adapter-1", "adapter-2"],
        );
        assert!(!filter.edge_passes(&fail_contrib));

        // Fails prefix
        let fail_prefix = edge_with_contributions(
            "may_be_related",
            &["lens:trellis:thematic_connection:may_be_related", "adapter-2"],
        );
        assert!(!filter.edge_passes(&fail_prefix));

        // Fails corroboration
        let fail_corrob = edge_with_contributions(
            "lens:trellis:thematic_connection",
            &["lens:trellis:thematic_connection:may_be_related"],
        );
        assert!(!filter.edge_passes(&fail_corrob));
    }
}
