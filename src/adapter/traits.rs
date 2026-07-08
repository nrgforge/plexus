//! Adapter trait — the bidirectional integration contract (ADR-011)
//!
//! Inbound: transforms domain-specific input into graph mutations via process().
//! Outbound: transforms raw graph events into domain-meaningful events via transform_events().

use crate::graph::events::GraphEvent;
use super::sink::{AdapterError, AdapterSink};
use super::types::OutboundEvent;
use crate::graph::Context;
use async_trait::async_trait;
use std::any::Any;

/// The input envelope the framework hands to an adapter.
#[derive(Debug)]
pub struct AdapterInput {
    /// The kind of input (matched by router)
    pub kind: String,
    /// Opaque data payload — the adapter downcasts internally
    pub data: Box<dyn Any + Send + Sync>,
    /// Processing context ID
    pub context_id: String,
}

impl AdapterInput {
    pub fn new(kind: impl Into<String>, data: impl Any + Send + Sync + 'static, context_id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            data: Box::new(data),
            context_id: context_id.into(),
        }
    }

    /// Create from an already-boxed data payload (avoids double-boxing).
    pub fn from_boxed(kind: impl Into<String>, data: Box<dyn Any + Send + Sync>, context_id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            data,
            context_id: context_id.into(),
        }
    }

    /// Attempt to downcast the data payload to a specific type.
    pub fn downcast_data<T: 'static>(&self) -> Option<&T> {
        self.data.downcast_ref::<T>()
    }
}

/// The bidirectional integration contract (ADR-011).
///
/// Inbound: declares what input kind it consumes, processes input through a sink.
/// Outbound: transforms raw graph events into domain-meaningful outbound events.
/// The single artifact that defines a consumer's relationship with Plexus.
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Unique identifier for this adapter
    fn id(&self) -> &str;

    /// What kind of input this adapter consumes (matched by router)
    fn input_kind(&self) -> &str;

    /// Inbound: process input, emitting results through the sink.
    ///
    /// The adapter downcasts `input.data` internally. If the downcast fails,
    /// return `Err(AdapterError::InvalidInput)`.
    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError>;

    /// Outbound: translate raw graph events into domain-meaningful events (ADR-011).
    ///
    /// Called after the enrichment loop completes with all accumulated events
    /// from the primary emission and all enrichment rounds, plus a context snapshot.
    /// The adapter filters what its consumer cares about.
    ///
    /// Default: no outbound events (backward compatible).
    fn transform_events(&self, _events: &[GraphEvent], _context: &Context) -> Vec<OutboundEvent> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    //! Outbound event contract (ADR-011), relocated from
    //! adapter/integration_tests.rs.

    use super::*;
    use crate::graph::NodeId;

    /// Minimal adapter that doesn't override transform_events.
    struct MinimalAdapter;

    #[async_trait::async_trait]
    impl Adapter for MinimalAdapter {
        fn id(&self) -> &str {
            "minimal"
        }
        fn input_kind(&self) -> &str {
            "test"
        }
        async fn process(
            &self,
            _input: &AdapterInput,
            _sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
        // transform_events NOT overridden — uses default
    }

    // === Scenario: Default transform_events returns empty vec ===
    #[test]
    fn default_transform_events_returns_empty_vec() {
        let adapter = MinimalAdapter;
        let ctx = Context::new("test");
        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![NodeId::from_string("A")],
            adapter_id: "test".to_string(),
            context_id: "test".to_string(),
        }];

        let outbound = adapter.transform_events(&events, &ctx);
        assert!(outbound.is_empty(), "default transform_events returns no outbound events");
    }

    /// Adapter that translates NodesAdded events to "concepts_detected" outbound events.
    struct ConceptDetectingAdapter {
        id: String,
    }

    impl ConceptDetectingAdapter {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Adapter for ConceptDetectingAdapter {
        fn id(&self) -> &str {
            &self.id
        }
        fn input_kind(&self) -> &str {
            "fragment"
        }
        async fn process(
            &self,
            _input: &AdapterInput,
            _sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            Ok(())
        }
        fn transform_events(
            &self,
            events: &[GraphEvent],
            _context: &Context,
        ) -> Vec<OutboundEvent> {
            let mut outbound = Vec::new();
            for event in events {
                if let GraphEvent::NodesAdded { node_ids, .. } = event {
                    let concepts: Vec<String> = node_ids
                        .iter()
                        .filter(|id| id.to_string().starts_with("concept:"))
                        .map(|id| id.to_string().strip_prefix("concept:").unwrap().to_string())
                        .collect();
                    if !concepts.is_empty() {
                        outbound.push(OutboundEvent::new(
                            "concepts_detected",
                            concepts.join(", "),
                        ));
                    }
                }
            }
            outbound
        }
    }

    // === Scenario: Adapter translates graph events to domain-meaningful outbound events ===
    #[test]
    fn adapter_translates_graph_events_to_outbound_events() {
        let adapter = ConceptDetectingAdapter::new("fragment-adapter");
        let ctx = Context::new("test");
        let events = vec![GraphEvent::NodesAdded {
            node_ids: vec![
                NodeId::from_string("concept:travel"),
                NodeId::from_string("concept:avignon"),
            ],
            adapter_id: "fragment-adapter".to_string(),
            context_id: "test".to_string(),
        }];

        let outbound = adapter.transform_events(&events, &ctx);
        assert_eq!(outbound.len(), 1, "one outbound event produced for two concept nodes");
        assert_eq!(outbound[0].kind, "concepts_detected", "outbound event kind is concepts_detected");
        assert_eq!(outbound[0].detail, "travel, avignon", "outbound event detail lists detected concepts");
    }
}
