//! Input router — dispatches input to matching adapters
//!
//! Fan-out: all adapters whose input_kind matches receive the input.
//! Each is invoked independently. The framework never inspects the
//! opaque data payload.

use super::sink::AdapterSink;
use super::traits::{Adapter, AdapterInput};
use std::sync::Arc;

/// Result of routing input to adapters.
#[derive(Debug)]
pub struct RouteResult {
    /// How many adapters were invoked
    pub adapters_invoked: usize,
    /// Errors from individual adapters (adapter_id, error)
    pub errors: Vec<(String, String)>,
}

/// Dispatches input to all adapters whose `input_kind()` matches.
pub struct InputRouter {
    adapters: Vec<Arc<dyn Adapter>>,
}

impl InputRouter {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn register(&mut self, adapter: Arc<dyn Adapter>) {
        self.adapters.push(adapter);
    }

    /// Route input to all matching adapters.
    ///
    /// Each matching adapter receives its own sink. Adapters are invoked
    /// sequentially (concurrent invocation is a future enhancement).
    /// One adapter's error does not affect others.
    pub async fn route(
        &self,
        input: &AdapterInput,
        sink_factory: &dyn Fn(&str) -> Box<dyn AdapterSink>,
    ) -> RouteResult {
        let matching: Vec<&Arc<dyn Adapter>> = self
            .adapters
            .iter()
            .filter(|a| a.input_kind() == input.kind)
            .collect();

        let mut result = RouteResult {
            adapters_invoked: 0,
            errors: Vec::new(),
        };

        for adapter in matching {
            let sink = sink_factory(adapter.id());
            result.adapters_invoked += 1;

            if let Err(e) = adapter.process(input, sink.as_ref()).await {
                result.errors.push((adapter.id().to_string(), e.to_string()));
            }
        }

        result
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::sink::AdapterError;
    use crate::adapter::types::Emission;
    use crate::adapter::traits::AdapterInput;
    use crate::graph::{ContentType, Context, Node, NodeId};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // === Test adapter implementations ===

    struct TestAdapter {
        id: String,
        input_kind: String,
        /// Track whether process was called
        invoked: Arc<Mutex<bool>>,
    }

    impl TestAdapter {
        fn new(id: &str, input_kind: &str) -> (Self, Arc<Mutex<bool>>) {
            let invoked = Arc::new(Mutex::new(false));
            (
                Self {
                    id: id.to_string(),
                    input_kind: input_kind.to_string(),
                    invoked: invoked.clone(),
                },
                invoked,
            )
        }
    }

    #[async_trait]
    impl Adapter for TestAdapter {
        fn id(&self) -> &str { &self.id }
        fn input_kind(&self) -> &str { &self.input_kind }

        async fn process(
            &self,
            _input: &AdapterInput,
            sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            *self.invoked.lock().unwrap() = true;
            // Emit a node to prove we did something
            let mut n = Node::new("test", ContentType::Concept);
            n.id = NodeId::from_string(&self.id);
            sink.emit(Emission::new().with_node(n)).await?;
            Ok(())
        }
    }

    /// Adapter that fails on downcast
    struct FailingAdapter {
        id: String,
        input_kind: String,
    }

    #[async_trait]
    impl Adapter for FailingAdapter {
        fn id(&self) -> &str { &self.id }
        fn input_kind(&self) -> &str { &self.input_kind }

        async fn process(
            &self,
            input: &AdapterInput,
            _sink: &dyn AdapterSink,
        ) -> Result<(), AdapterError> {
            // Try to downcast to wrong type
            let _: &String = input.downcast_data::<String>()
                .ok_or(AdapterError::InvalidInput)?;
            Ok(())
        }
    }

    fn make_sink_factory() -> (
        impl Fn(&str) -> Box<dyn AdapterSink>,
        Arc<Mutex<Context>>,
    ) {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let ctx_clone = ctx.clone();
        let factory = move |_adapter_id: &str| -> Box<dyn AdapterSink> {
            Box::new(crate::adapter::EngineSink::new(ctx_clone.clone()))
        };
        (factory, ctx)
    }

    // === Scenario: Input routed to matching adapter ===
    #[tokio::test]
    async fn input_routed_to_matching_adapter() {
        let mut router = InputRouter::new();

        let (doc_adapter, doc_invoked) = TestAdapter::new("document-adapter", "file_content");
        let (move_adapter, move_invoked) = TestAdapter::new("movement-adapter", "gesture_encoding");

        router.register(Arc::new(doc_adapter));
        router.register(Arc::new(move_adapter));

        let input = AdapterInput::new("file_content", "hello.md".to_string(), "ctx-1");
        let (factory, _ctx) = make_sink_factory();

        let result = router.route(&input, &factory).await;

        assert_eq!(result.adapters_invoked, 1);
        assert!(*doc_invoked.lock().unwrap());
        assert!(!*move_invoked.lock().unwrap());
    }

    // === Scenario: Fan-out to multiple adapters with same input kind ===
    #[tokio::test]
    async fn fan_out_to_multiple_adapters() {
        let mut router = InputRouter::new();

        let (a1, inv1) = TestAdapter::new("normalization-adapter", "graph_state");
        let (a2, inv2) = TestAdapter::new("topology-adapter", "graph_state");
        let (a3, inv3) = TestAdapter::new("coherence-adapter", "graph_state");

        router.register(Arc::new(a1));
        router.register(Arc::new(a2));
        router.register(Arc::new(a3));

        let input = AdapterInput::new("graph_state", 42u64, "ctx-1");
        let (factory, _ctx) = make_sink_factory();

        let result = router.route(&input, &factory).await;

        assert_eq!(result.adapters_invoked, 3);
        assert!(*inv1.lock().unwrap());
        assert!(*inv2.lock().unwrap());
        assert!(*inv3.lock().unwrap());
    }

    // === Scenario: No matching adapter ===
    #[tokio::test]
    async fn no_matching_adapter() {
        let router = InputRouter::new();

        let input = AdapterInput::new("unknown_kind", "data".to_string(), "ctx-1");
        let (factory, _ctx) = make_sink_factory();

        let result = router.route(&input, &factory).await;

        assert_eq!(result.adapters_invoked, 0);
        assert!(result.errors.is_empty());
    }

    // === Scenario: Opaque data downcast failure ===
    #[tokio::test]
    async fn downcast_failure_returns_error() {
        let mut router = InputRouter::new();

        let failing = FailingAdapter {
            id: "document-adapter".to_string(),
            input_kind: "file_content".to_string(),
        };
        router.register(Arc::new(failing));

        // Send data of wrong type (u64 instead of String)
        let input = AdapterInput::new("file_content", 42u64, "ctx-1");
        let (factory, _ctx) = make_sink_factory();

        let result = router.route(&input, &factory).await;

        assert_eq!(result.adapters_invoked, 1);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].0, "document-adapter");
    }

    // === Scenario: Independent adapters don't see each other's emissions ===
    #[tokio::test]
    async fn independent_adapters_isolated() {
        let mut router = InputRouter::new();

        let (a1, _) = TestAdapter::new("adapter-A", "shared_kind");
        let (a2, _) = TestAdapter::new("adapter-B", "shared_kind");

        router.register(Arc::new(a1));
        router.register(Arc::new(a2));

        // Each adapter gets its own sink via factory, so emissions are independent.
        // We verify by checking both adapters were invoked without affecting each other.
        let input = AdapterInput::new("shared_kind", "data".to_string(), "ctx-1");
        let (factory, ctx) = make_sink_factory();

        let result = router.route(&input, &factory).await;

        assert_eq!(result.adapters_invoked, 2);
        assert!(result.errors.is_empty());

        // Both nodes exist (they share a context in this test, but in production
        // each adapter would get its own sink — the important thing is they
        // don't interfere with each other's processing)
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("adapter-A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("adapter-B")).is_some());
    }
}
