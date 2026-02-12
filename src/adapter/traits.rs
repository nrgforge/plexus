//! Adapter trait — the bidirectional integration contract (ADR-011)
//!
//! Inbound: transforms domain-specific input into graph mutations via process().
//! Outbound: transforms raw graph events into domain-meaningful events via transform_events().

use super::events::GraphEvent;
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
