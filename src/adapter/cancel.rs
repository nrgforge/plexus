//! Cooperative cancellation for adapters
//!
//! The framework signals via a cancellation token. The adapter checks
//! the token between emissions. Already-committed emissions remain valid.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cooperative cancellation token.
///
/// The framework sets the token; the adapter checks it between emissions.
/// Cancellation during an emission has no effect until the next check.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Signal cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_uncancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_sets_token() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn cloned_token_shares_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        token.cancel();
        assert!(clone.is_cancelled());
    }

    // === Cooperative cancellation contract (relocated from adapter/integration_tests.rs) ===

    use crate::adapter::sink::AdapterSink;
    use crate::adapter::types::Emission;
    use crate::adapter::EngineSink;
    use crate::graph::{ContentType, Context, Node, NodeId};
    use std::sync::Mutex;

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    // === Scenario: Adapter checks cancellation between emissions ===
    #[tokio::test]
    async fn adapter_checks_cancellation_between_emissions() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // E1: committed successfully
        let e1 = Emission::new().with_node(node("A"));
        let r1 = sink.emit(e1).await.unwrap();
        assert_eq!(r1.nodes_committed, 1, "first emission commits one node");

        // Framework signals cancellation
        token.cancel();

        // Adapter checks token before next emission
        assert!(token.is_cancelled(), "token is cancelled after cancel() call");

        // Adapter stops — no further emissions
        // E1 remains committed
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some(), "node A committed before cancellation");
    }

    // === Scenario: Committed emissions survive cancellation ===
    #[tokio::test]
    async fn committed_emissions_survive_cancellation() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // E1 and E2 committed
        sink.emit(Emission::new().with_node(node("A"))).await.unwrap();
        sink.emit(Emission::new().with_node(node("B"))).await.unwrap();

        // Cancel before E3
        token.cancel();
        assert!(token.is_cancelled(), "token is cancelled after cancel() call");

        // E3 never emitted — adapter checks token and stops
        // E1 and E2 remain
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some(), "node A survives cancellation");
        assert!(ctx.get_node(&NodeId::from_string("B")).is_some(), "node B survives cancellation");
        assert_eq!(ctx.node_count(), 2, "exactly 2 nodes committed before cancellation");
    }

    // === Scenario: Cancellation during emission has no effect until next check ===
    #[tokio::test]
    async fn cancellation_during_emission_no_effect_until_check() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // Cancel while E2 is "being constructed"
        // (in practice, cancellation is checked between emit calls, not during)
        token.cancel();

        // Adapter may still emit E2 if it hasn't checked the token yet
        let r2 = sink.emit(Emission::new().with_node(node("X"))).await.unwrap();
        assert_eq!(r2.nodes_committed, 1, "committed, because emit() doesn't check token");

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("X")).is_some(), "node X committed despite prior cancellation");
    }
}
