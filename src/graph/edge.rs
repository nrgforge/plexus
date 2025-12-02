//! Edge representation with self-reinforcing strength

use super::node::{NodeId, Properties};
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

/// Types of reinforcement that can strengthen an edge
///
/// Matches the contract schema enum values
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReinforcementType {
    /// Nodes appear together frequently
    CoOccurrence,
    /// Similar pattern found in different context
    SimilarPattern,
    /// User traversed this edge
    UserTraversal,
    /// User confirmed the relationship
    UserValidation,
    /// Multiple analyzers found the same relationship
    MultipleAnalyzers,
    /// Relationship exists across contexts
    CrossContext,
    /// Relationship stable across edits
    ConsistentOverTime,
    /// Edge frequently queried
    FrequentAccess,
    /// Successful execution of dependent operation
    SuccessfulExecution,
}

/// Source of a reinforcement (extension, stored in metadata)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReinforcementSource {
    /// From an analyzer
    Analyzer(String),
    /// From a user action
    User(String),
    /// From system behavior
    System,
    /// From another context
    CrossContext(String),
}

impl ReinforcementSource {
    /// Convert to metadata entry
    pub fn to_metadata(&self) -> (String, String) {
        match self {
            ReinforcementSource::Analyzer(name) => ("source".to_string(), format!("analyzer:{}", name)),
            ReinforcementSource::User(id) => ("source".to_string(), format!("user:{}", id)),
            ReinforcementSource::System => ("source".to_string(), "system".to_string()),
            ReinforcementSource::CrossContext(ctx) => ("source".to_string(), format!("context:{}", ctx)),
        }
    }
}

/// Evidence that strengthens an edge
///
/// Matches the contract schema:
/// - `type`: ReinforcementType enum
/// - `timestamp`: ISO 8601 datetime
/// - `context_id`: optional string
/// - `metadata`: optional object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reinforcement {
    /// Type of reinforcement (contract field: "type")
    #[serde(rename = "type")]
    pub reinforcement_type: ReinforcementType,
    /// When this reinforcement occurred
    pub timestamp: DateTime<Utc>,
    /// Which context reinforced this
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
    /// Additional metadata (source info stored here per contract)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl Reinforcement {
    /// Create a new reinforcement
    pub fn new(reinforcement_type: ReinforcementType) -> Self {
        Self {
            reinforcement_type,
            timestamp: Utc::now(),
            context_id: None,
            metadata: None,
        }
    }

    /// Create a new reinforcement with source (stored in metadata)
    pub fn with_source(reinforcement_type: ReinforcementType, source: ReinforcementSource) -> Self {
        let (key, value) = source.to_metadata();
        let mut metadata = HashMap::new();
        metadata.insert(key, value);

        Self {
            reinforcement_type,
            timestamp: Utc::now(),
            context_id: None,
            metadata: Some(metadata),
        }
    }

    /// Set the context ID
    pub fn in_context(mut self, context_id: impl Into<String>) -> Self {
        self.context_id = Some(context_id.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }
}

/// Decay configuration for edges
const DECAY_HALF_LIFE_HOURS: f32 = 168.0; // 1 week

/// A directed edge with self-reinforcing strength
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier
    pub id: EdgeId,
    /// Source node
    pub source: NodeId,
    /// Target node
    pub target: NodeId,
    /// Type of relationship (e.g., "calls", "depends_on", "transitions_to")
    pub relationship: String,
    /// Base relationship strength (0.0 - 1.0)
    pub weight: f32,
    /// Dynamic strength that increases with reinforcement
    pub strength: f32,
    /// Confidence in relationship validity (0.0 - 1.0)
    pub confidence: f32,
    /// History of reinforcements
    pub reinforcements: Vec<Reinforcement>,
    /// When the edge was created
    pub created_at: DateTime<Utc>,
    /// When the edge was last reinforced
    pub last_reinforced: DateTime<Utc>,
    /// Additional properties
    pub properties: Properties,
}

impl Edge {
    /// Create a new edge
    pub fn new(source: NodeId, target: NodeId, relationship: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: EdgeId::new(),
            source,
            target,
            relationship: relationship.into(),
            weight: 1.0,
            strength: 1.0,
            confidence: 0.0, // No confidence until reinforced
            reinforcements: Vec::new(),
            created_at: now,
            last_reinforced: now,
            properties: HashMap::new(),
        }
    }

    /// Reinforce this edge with new evidence
    pub fn reinforce(&mut self, reinforcement: Reinforcement) {
        self.reinforcements.push(reinforcement);
        self.last_reinforced = Utc::now();
        self.strength = self.calculate_strength();
        self.confidence = self.calculate_confidence();
    }

    /// Calculate strength based on reinforcement history
    fn calculate_strength(&self) -> f32 {
        let base = self.weight;
        let reinforcement_boost = self.reinforcements.len() as f32 * 0.1;
        let recency = self.recency_factor();
        (base + reinforcement_boost) * recency
    }

    /// Calculate confidence based on evidence diversity
    fn calculate_confidence(&self) -> f32 {
        if self.reinforcements.is_empty() {
            return 0.0;
        }

        // Count unique reinforcement types
        let unique_types: std::collections::HashSet<_> = self
            .reinforcements
            .iter()
            .map(|r| std::mem::discriminant(&r.reinforcement_type))
            .collect();

        // More diverse reinforcement types = higher confidence
        (unique_types.len() as f32 * 0.25).min(1.0)
    }

    /// Calculate recency factor (exponential decay)
    fn recency_factor(&self) -> f32 {
        let hours_since = (Utc::now() - self.last_reinforced)
            .num_hours()
            .max(0) as f32;
        0.5_f32.powf(hours_since / DECAY_HALF_LIFE_HOURS)
    }

    /// Check if the edge was reinforced within the given hours
    pub fn reinforced_within_hours(&self, hours: i64) -> bool {
        let duration = Utc::now() - self.last_reinforced;
        duration.num_hours() < hours
    }
}
