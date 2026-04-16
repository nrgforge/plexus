//! Live MCP subprocess acceptance test (WP-H.2).
//!
//! Verifies the two-consumer vocabulary-layer cross-pollination story
//! end-to-end — the product-discovery-defined acceptance criterion for
//! the MCP consumer interaction cycle.
//!
//! Harness lives in `mcp_harness.rs`; this file contains only the test.

use super::mcp_harness::{is_error, node_count, tool_result_json, McpHarness};
use serde_json::json;
use tempfile::TempDir;

/// End-to-end: two consumers share a context, each loads their own spec,
/// both lens enrichments fire on both consumers' ingests, and cross-
/// pollination is visible via `find_nodes(relationship_prefix: "lens:...")`.
#[tokio::test]
async fn mcp_e2e_two_consumer_cross_pollination() {
    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("plexus.db");

    let mut h = McpHarness::spawn(&db).await;

    // ── Handshake ──────────────────────────────────────────────────────
    let init = h.initialize().await;
    assert!(init.get("result").is_some(), "initialize response: {}", init);

    // ── Setup: active context ──────────────────────────────────────────
    let resp = h.call_tool("set_context", json!({"name": "test"})).await;
    assert!(!is_error(&resp), "set_context failed: {}", resp);

    // ── Consumer-1 loads spec ──────────────────────────────────────────
    // The declared adapter's input_kind is never actually routed to by the
    // test; ingest uses the built-in "content" adapter (tag-based). The
    // spec's role in this test is to register Consumer-1's lens.
    let consumer_1_yaml = r#"
adapter_id: consumer-1-content
input_kind: consumer-1.fragment
lens:
  consumer: consumer-1
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    let resp = h.call_tool("load_spec", json!({"spec_yaml": consumer_1_yaml})).await;
    assert!(!is_error(&resp), "load_spec consumer-1 failed: {}", resp);
    let r1 = tool_result_json(&resp);
    assert_eq!(r1["adapter_id"], "consumer-1-content");
    assert_eq!(r1["lens_namespace"], "lens:consumer-1");
    assert_eq!(
        r1["vocabulary_edges_created"], 0,
        "empty context — lens sweep should create 0 edges"
    );

    // ── Consumer-1 ingests ─────────────────────────────────────────────
    // tags ["travel","routine"] → two concept nodes tagged_with fragment
    // → co_occurrence produces may_be_related between them
    // → lens:consumer-1:thematic_connection edge between them
    let resp = h.call_tool("ingest", json!({
        "data": { "text": "A thought about travel and routine", "tags": ["travel", "routine"] },
        "input_kind": "content"
    })).await;
    assert!(!is_error(&resp), "ingest consumer-1 failed: {}", resp);
    assert!(
        tool_result_json(&resp)["events"].as_u64().unwrap_or(0) > 0,
        "ingest should produce events"
    );

    // Baseline: lens:consumer-1: touches both concepts (travel, routine)
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:consumer-1:"}),
    ).await;
    assert!(!is_error(&resp), "find_nodes lens:consumer-1: failed: {}", resp);
    let c1_before = node_count(&resp);
    assert!(
        c1_before >= 2,
        "expected ≥2 nodes incident to lens:consumer-1: edges after one ingest, got {}",
        c1_before
    );

    // ── Consumer-2 loads spec on same context ──────────────────────────
    // Consumer-2's lens sweep runs over existing may_be_related edges from
    // Consumer-1's ingest → creates lens:consumer-2:citation_link edges.
    let consumer_2_yaml = r#"
adapter_id: consumer-2-content
input_kind: consumer-2.fragment
lens:
  consumer: consumer-2
  translations:
    - from: [may_be_related]
      to: citation_link
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    let resp = h.call_tool("load_spec", json!({"spec_yaml": consumer_2_yaml})).await;
    assert!(!is_error(&resp), "load_spec consumer-2 failed: {}", resp);
    let r2 = tool_result_json(&resp);
    assert_eq!(r2["adapter_id"], "consumer-2-content");
    assert_eq!(r2["lens_namespace"], "lens:consumer-2");
    assert!(
        r2["vocabulary_edges_created"].as_u64().unwrap_or(0) > 0,
        "Consumer-2's lens should sweep existing may_be_related edges"
    );

    // ── Consumer-2 ingests ─────────────────────────────────────────────
    // Both lenses are now registered. Every may_be_related edge produced
    // from this ingest gets translated by BOTH lenses — this is the
    // cross-pollination point.
    let resp = h.call_tool("ingest", json!({
        "data": { "text": "Citation about nature and stillness", "tags": ["nature", "stillness"] },
        "input_kind": "content"
    })).await;
    assert!(!is_error(&resp), "ingest consumer-2 failed: {}", resp);

    // ── Cross-pollination assertions ───────────────────────────────────

    // Consumer-1's vocabulary now includes edges from Consumer-2's ingest
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:consumer-1:"}),
    ).await;
    let c1_after = node_count(&resp);
    assert!(
        c1_after > c1_before,
        "Consumer-1's lens coverage should grow after Consumer-2's ingest \
         (was {}, now {}) — this is the cross-pollination signal",
        c1_before, c1_after
    );

    // Consumer-2's vocabulary covers both ingests
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:consumer-2:"}),
    ).await;
    let c2 = node_count(&resp);
    assert!(
        c2 >= c1_after,
        "Consumer-2's lens coverage should be at least as broad as \
         Consumer-1's (both lenses translate the same may_be_related \
         edges) — consumer-1 has {}, consumer-2 has {}",
        c1_after, c2
    );

    // Global lens:* prefix returns nodes from both namespaces
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:"}),
    ).await;
    assert!(
        node_count(&resp) > 0,
        "lens:* prefix query should return nodes"
    );

    h.shutdown().await;
}
