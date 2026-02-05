//! Edge representation for the knowledge graph

use super::node::{dimension, NodeId, Properties};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an edge
///
/// Serializes as a plain string (UUID or semantic ID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(String);

impl EdgeId {
    /// Create a new random EdgeId (UUID-based)
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create an EdgeId from a string
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EdgeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EdgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for EdgeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for EdgeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A directed edge in the knowledge graph
///
/// Carries a raw weight (accumulated Hebbian reinforcement) and a relationship type.
/// Normalized weights are computed at query time, not stored.
/// See ADR-001: Semantic Adapter Architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier
    pub id: EdgeId,
    /// Source node
    pub source: NodeId,
    /// Target node
    pub target: NodeId,
    /// Dimension of the source node
    #[serde(default = "default_dimension")]
    pub source_dimension: String,
    /// Dimension of the target node
    #[serde(default = "default_dimension")]
    pub target_dimension: String,
    /// Type of relationship (e.g., "calls", "depends_on", "may_be_related")
    pub relationship: String,
    /// Accumulated Hebbian reinforcement strength. Ground truth — never decays on a clock.
    pub raw_weight: f32,
    /// When the edge was created
    pub created_at: DateTime<Utc>,
    /// Additional properties
    pub properties: Properties,
}

/// Default dimension for backwards compatibility with existing edges
fn default_dimension() -> String {
    dimension::DEFAULT.to_string()
}

impl Edge {
    /// Create a new edge within the default dimension
    pub fn new(source: NodeId, target: NodeId, relationship: impl Into<String>) -> Self {
        Self {
            id: EdgeId::new(),
            source,
            target,
            source_dimension: dimension::DEFAULT.to_string(),
            target_dimension: dimension::DEFAULT.to_string(),
            relationship: relationship.into(),
            raw_weight: 1.0,
            created_at: Utc::now(),
            properties: HashMap::new(),
        }
    }

    /// Create a new edge within a specific dimension (both endpoints in same dimension)
    pub fn new_in_dimension(
        source: NodeId,
        target: NodeId,
        relationship: impl Into<String>,
        dim: impl Into<String>,
    ) -> Self {
        let dim_str = dim.into();
        Self {
            id: EdgeId::new(),
            source,
            target,
            source_dimension: dim_str.clone(),
            target_dimension: dim_str,
            relationship: relationship.into(),
            raw_weight: 1.0,
            created_at: Utc::now(),
            properties: HashMap::new(),
        }
    }

    /// Create a cross-dimensional edge (connects nodes in different dimensions)
    pub fn new_cross_dimensional(
        source: NodeId,
        source_dim: impl Into<String>,
        target: NodeId,
        target_dim: impl Into<String>,
        relationship: impl Into<String>,
    ) -> Self {
        Self {
            id: EdgeId::new(),
            source,
            target,
            source_dimension: source_dim.into(),
            target_dimension: target_dim.into(),
            relationship: relationship.into(),
            raw_weight: 1.0,
            created_at: Utc::now(),
            properties: HashMap::new(),
        }
    }

    /// Check if this edge crosses dimension boundaries
    pub fn is_cross_dimensional(&self) -> bool {
        self.source_dimension != self.target_dimension
    }

    /// Set source dimension (builder pattern)
    pub fn with_source_dimension(mut self, dim: impl Into<String>) -> Self {
        self.source_dimension = dim.into();
        self
    }

    /// Set target dimension (builder pattern)
    pub fn with_target_dimension(mut self, dim: impl Into<String>) -> Self {
        self.target_dimension = dim.into();
        self
    }

    /// Set both dimensions (same dimension for both endpoints)
    pub fn with_dimension(mut self, dim: impl Into<String>) -> Self {
        let d = dim.into();
        self.source_dimension = d.clone();
        self.target_dimension = d;
        self
    }
}
