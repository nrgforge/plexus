//! Edge representation with self-reinforcing strength

use super::node::{NodeId, Properties};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an edge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(Uuid);

impl EdgeId {
    /// Create a new random EdgeId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
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

/// Types of reinforcement that can strengthen an edge
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

/// Source of a reinforcement
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

/// Evidence that strengthens an edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reinforcement {
    /// Type of reinforcement
    pub reinforcement_type: ReinforcementType,
    /// When this reinforcement occurred
    pub timestamp: DateTime<Utc>,
    /// Which context reinforced this
    pub context_id: Option<String>,
    /// Source of the reinforcement
    pub source: ReinforcementSource,
    /// Additional metadata
    pub metadata: Option<HashMap<String, String>>,
}

impl Reinforcement {
    /// Create a new reinforcement
    pub fn new(
        reinforcement_type: ReinforcementType,
        source: ReinforcementSource,
    ) -> Self {
        Self {
            reinforcement_type,
            timestamp: Utc::now(),
            context_id: None,
            source,
            metadata: None,
        }
    }

    /// Set the context ID
    pub fn with_context(mut self, context_id: impl Into<String>) -> Self {
        self.context_id = Some(context_id.into());
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

        // Count unique source types
        let unique_sources: std::collections::HashSet<_> = self
            .reinforcements
            .iter()
            .map(|r| std::mem::discriminant(&r.source))
            .collect();

        // More diverse sources = higher confidence
        (unique_sources.len() as f32 * 0.25).min(1.0)
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
