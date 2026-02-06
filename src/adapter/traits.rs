//! Adapter trait — the contract adapters implement
//!
//! An adapter transforms domain-specific input into graph mutations.
//! It declares what input kind it consumes and processes input through a sink.

use super::sink::{AdapterError, AdapterSink};
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

/// The contract adapters implement.
///
/// An adapter declares what input kind it consumes, processes input
/// through a sink, and optionally declares a schedule for reflexive triggering.
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Unique identifier for this adapter
    fn id(&self) -> &str;

    /// What kind of input this adapter consumes (matched by router)
    fn input_kind(&self) -> &str;

    /// Process input, emitting results through the sink.
    ///
    /// The adapter downcasts `input.data` internally. If the downcast fails,
    /// return `Err(AdapterError::InvalidInput)`.
    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError>;
}
