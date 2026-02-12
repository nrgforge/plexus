//! Enrichment trait and registry (ADR-010)
//!
//! Enrichments are reactive components that respond to graph events
//! and produce additional graph mutations. They bridge between dimensions
//! within the graph — distinct from adapters, which bridge between a
//! consumer's domain and the graph.

use super::events::GraphEvent;
use super::types::Emission;
use crate::graph::Context;
use std::collections::HashSet;
use std::sync::Arc;

/// A reactive component that responds to graph events with additional mutations.
///
/// Enrichments are registered globally and run in the enrichment loop
/// after each primary emission. They receive the previous round's events
/// and a context snapshot, returning `Some(Emission)` if they have work
/// to do, or `None` if quiescent.
///
/// Enrichments must terminate via idempotency: check context state before
/// emitting to avoid infinite loops. The framework enforces a safety valve
/// (max round count) as a fallback.
pub trait Enrichment: Send + Sync {
    /// Stable identifier for contribution tracking and deduplication.
    fn id(&self) -> &str;

    /// React to graph events and optionally produce additional mutations.
    ///
    /// - `events`: events from the previous round (primary emission for round 0)
    /// - `context`: cloned snapshot — consistent, immutable view at enrichment time
    ///
    /// Returns `Some(Emission)` to produce mutations, `None` if quiescent.
    fn enrich(&self, events: &[GraphEvent], context: &Context) -> Option<Emission>;
}

/// Registry of enrichments for the enrichment loop.
///
/// Enrichments are deduplicated by `id()` — if two integrations register
/// the same enrichment, it runs once per round.
pub struct EnrichmentRegistry {
    enrichments: Vec<Arc<dyn Enrichment>>,
    max_rounds: usize,
}

/// Default maximum enrichment loop rounds (safety valve).
const DEFAULT_MAX_ROUNDS: usize = 10;

impl EnrichmentRegistry {
    /// Create a registry with the given enrichments, deduplicated by id.
    pub fn new(enrichments: Vec<Arc<dyn Enrichment>>) -> Self {
        let mut seen = HashSet::new();
        let deduped: Vec<_> = enrichments
            .into_iter()
            .filter(|e| seen.insert(e.id().to_string()))
            .collect();

        Self {
            enrichments: deduped,
            max_rounds: DEFAULT_MAX_ROUNDS,
        }
    }

    /// Create an empty registry (no enrichments).
    pub fn empty() -> Self {
        Self {
            enrichments: Vec::new(),
            max_rounds: DEFAULT_MAX_ROUNDS,
        }
    }

    /// Set the maximum number of enrichment loop rounds.
    pub fn with_max_rounds(mut self, max: usize) -> Self {
        self.max_rounds = max;
        self
    }

    /// Access the registered enrichments.
    pub fn enrichments(&self) -> &[Arc<dyn Enrichment>] {
        &self.enrichments
    }

    /// Maximum rounds before the safety valve aborts.
    pub fn max_rounds(&self) -> usize {
        self.max_rounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestEnrichment {
        id: String,
        calls: Mutex<Vec<usize>>,
    }

    impl TestEnrichment {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl Enrichment for TestEnrichment {
        fn id(&self) -> &str {
            &self.id
        }
        fn enrich(&self, _events: &[GraphEvent], _context: &Context) -> Option<Emission> {
            self.calls.lock().unwrap().push(1);
            None
        }
    }

    #[test]
    fn registry_deduplicates_by_id() {
        let a = Arc::new(TestEnrichment::new("shared")) as Arc<dyn Enrichment>;
        let b = Arc::new(TestEnrichment::new("shared")) as Arc<dyn Enrichment>;
        let c = Arc::new(TestEnrichment::new("other")) as Arc<dyn Enrichment>;

        let registry = EnrichmentRegistry::new(vec![a, b, c]);
        assert_eq!(registry.enrichments().len(), 2);
    }

    #[test]
    fn registry_default_max_rounds() {
        let registry = EnrichmentRegistry::empty();
        assert_eq!(registry.max_rounds(), DEFAULT_MAX_ROUNDS);
    }

    #[test]
    fn registry_custom_max_rounds() {
        let registry = EnrichmentRegistry::empty().with_max_rounds(5);
        assert_eq!(registry.max_rounds(), 5);
    }
}
