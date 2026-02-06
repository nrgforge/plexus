//! Integration tests for cancellation and progressive emission scenarios

#[cfg(test)]
mod tests {
    use crate::adapter::cancel::CancellationToken;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::events::GraphEvent;
    use crate::adapter::sink::AdapterSink;
    use crate::adapter::types::Emission;
    use crate::graph::{ContentType, Context, Edge, Node, NodeId};
    use std::sync::{Arc, Mutex};

    fn node(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n
    }

    fn edge(source: &str, target: &str) -> Edge {
        Edge::new(
            NodeId::from_string(source),
            NodeId::from_string(target),
            "related_to",
        )
    }

    // ================================================================
    // Cancellation Scenarios
    // ================================================================

    // === Scenario: Adapter checks cancellation between emissions ===
    #[tokio::test]
    async fn adapter_checks_cancellation_between_emissions() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());
        let token = CancellationToken::new();

        // E1: committed successfully
        let e1 = Emission::new().with_node(node("A"));
        let r1 = sink.emit(e1).await.unwrap();
        assert_eq!(r1.nodes_committed, 1);

        // Framework signals cancellation
        token.cancel();

        // Adapter checks token before next emission
        assert!(token.is_cancelled());

        // Adapter stops — no further emissions
        // E1 remains committed
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
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
        assert!(token.is_cancelled());

        // E3 never emitted — adapter checks token and stops
        // E1 and E2 remain
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("B")).is_some());
        assert_eq!(ctx.node_count(), 2);
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
        assert_eq!(r2.nodes_committed, 1); // committed, because emit() doesn't check token

        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("X")).is_some());
    }

    // ================================================================
    // Progressive Emission Scenarios
    // ================================================================

    // === Scenario: Multiple emissions from one adapter, each commits independently ===
    #[tokio::test]
    async fn multiple_emissions_commit_independently() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        // E1: structural nodes
        let e1 = Emission::new()
            .with_node(node("file"))
            .with_node(node("section-1"))
            .with_node(node("section-2"));
        let r1 = sink.emit(e1).await.unwrap();
        assert_eq!(r1.nodes_committed, 3);

        // After E1: structural nodes exist
        {
            let ctx = ctx.lock().unwrap();
            assert!(ctx.get_node(&NodeId::from_string("file")).is_some());
            assert!(ctx.get_node(&NodeId::from_string("section-1")).is_some());
        }

        // E2: semantic nodes + edges
        let e2 = Emission::new()
            .with_node(node("concept-sudden"))
            .with_edge(edge("section-1", "concept-sudden"));
        let r2 = sink.emit(e2).await.unwrap();
        assert_eq!(r2.nodes_committed, 1);
        assert_eq!(r2.edges_committed, 1);

        // After E2: both structural and semantic exist
        let ctx = ctx.lock().unwrap();
        assert_eq!(ctx.node_count(), 4);
        assert_eq!(ctx.edge_count(), 1);
    }

    // === Scenario: Graph events fire per emission ===
    #[tokio::test]
    async fn graph_events_fire_per_emission() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        // E1: 3 nodes
        let e1 = Emission::new()
            .with_node(node("A"))
            .with_node(node("B"))
            .with_node(node("C"));
        let r1 = sink.emit(e1).await.unwrap();

        let nodes_event = r1.events.iter().find(|e| matches!(e, GraphEvent::NodesAdded { .. }));
        assert!(nodes_event.is_some());
        if let Some(GraphEvent::NodesAdded { node_ids, .. }) = nodes_event {
            assert_eq!(node_ids.len(), 3);
        }

        // E2: 2 edges
        let e2 = Emission::new()
            .with_edge(edge("A", "B"))
            .with_edge(edge("B", "C"));
        let r2 = sink.emit(e2).await.unwrap();

        let edges_event = r2.events.iter().find(|e| matches!(e, GraphEvent::EdgesAdded { .. }));
        assert!(edges_event.is_some());
        if let Some(GraphEvent::EdgesAdded { edge_ids, .. }) = edges_event {
            assert_eq!(edge_ids.len(), 2);
        }
    }

    // === Scenario: Early emissions visible to queries before later emissions ===
    #[tokio::test]
    async fn early_emissions_visible_before_later() {
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = EngineSink::new(ctx.clone());

        // E1: node A
        sink.emit(Emission::new().with_node(node("A"))).await.unwrap();

        // Node A is visible immediately
        {
            let ctx = ctx.lock().unwrap();
            assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
            // E2 not yet emitted — concept-X doesn't exist
            assert!(ctx.get_node(&NodeId::from_string("concept-X")).is_none());
        }

        // E2: concept-X
        sink.emit(Emission::new().with_node(node("concept-X"))).await.unwrap();

        // Now both exist
        let ctx = ctx.lock().unwrap();
        assert!(ctx.get_node(&NodeId::from_string("A")).is_some());
        assert!(ctx.get_node(&NodeId::from_string("concept-X")).is_some());
    }
}
