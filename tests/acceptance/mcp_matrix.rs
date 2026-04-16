//! MCP transport matrix tests (deterministic, default-run).
//!
//! Coverage:
//! - T1: Declarative-emit routing — ingest using a consumer spec's
//!   input_kind actually invokes the consumer's emit primitives
//!   (not the built-in ContentAdapter).
//! - T2: Mixed adapter types on same context — built-in content +
//!   declarative-emit; both produce nodes; a single lens translates
//!   may_be_related edges from both ingest paths.
//! - T3: Spec persistence across MCP restart — lens registered via
//!   load_spec in one process fires in a second process spawned against
//!   the same DB, without re-loading the spec.
//! - T5: Error paths — malformed YAML and no-active-context errors
//!   surface via the `isError: true` JSON-RPC result field.
//! - T4: unload_spec lifecycle — ingest (lens fires) → unload_spec →
//!   second ingest (unloaded lens does not fire on new edges); previously-
//!   translated vocabulary edges remain in the graph (Invariant 62).
//! - T9: N-consumer cross-pollination — 4 consumers on same context,
//!   each with distinct namespace. Every consumer's vocabulary covers
//!   content from every other consumer.
//! - T10: Consumer cycling — load spec → ingest → unload → reload same
//!   adapter_id with modified lens translation → ingest. Old lens edges
//!   persist; new lens fires on both pre- and post-reload content.

use super::mcp_harness::{
    is_error, is_rpc_error, node_count, rpc_error_message, tool_result_json, tool_result_text,
    McpHarness,
};
use serde_json::json;
use tempfile::TempDir;

// ── T1: Declarative-emit routing through MCP ───────────────────────────

/// Given a consumer spec declaring input_kind `trellis.fragment` with a
/// non-trivial emit,
/// when ingest is called with input_kind `trellis.fragment`,
/// then the consumer's emit primitives run — the spec's `id_template`
/// produces deterministic node IDs that confirm the declarative adapter
/// handled the input (not the built-in ContentAdapter, which would have
/// produced different, auto-generated fragment IDs).
#[tokio::test]
async fn t1_declarative_emit_routing() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t1.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;

    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t1"})).await));

    let spec_yaml = r#"
adapter_id: trellis-fragment
input_kind: trellis.fragment
emit:
  - create_node:
      id: "fragment:{input.id}"
      type: fragment
      dimension: structure
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
  - create_edge:
      source: "fragment:{input.id}"
      target: "concept:{input.tag}"
      relationship: tagged_with
"#;
    let resp = h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await;
    assert!(!is_error(&resp), "load_spec failed: {}", resp);

    // Ingest through the consumer's declared input_kind — routes to the
    // DeclarativeAdapter, NOT the built-in ContentAdapter.
    let resp = h.call_tool("ingest", json!({
        "data": {"id": "doc-abc", "tag": "adventure"},
        "input_kind": "trellis.fragment",
    })).await;
    assert!(!is_error(&resp), "ingest trellis.fragment failed: {}", resp);

    // Assert: the spec's id_template produced these exact nodes.
    // If the ContentAdapter had processed the input, fragment IDs would
    // be auto-generated UUIDs (not "fragment:doc-abc").
    let resp = h.call_tool("find_nodes", json!({})).await;
    let all = tool_result_json(&resp);
    let ids: Vec<String> = all["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap_or("").to_string())
        .collect();

    assert!(
        ids.iter().any(|id| id == "fragment:doc-abc"),
        "expected fragment:doc-abc from declarative emit — got: {:?}",
        ids
    );
    assert!(
        ids.iter().any(|id| id == "concept:adventure"),
        "expected concept:adventure from declarative emit — got: {:?}",
        ids
    );

    // Fragment count should be exactly 1 (the declarative one); built-in
    // fragments carry UUID suffixes, not `doc-abc`.
    let fragment_count = ids.iter().filter(|id| id.starts_with("fragment:")).count();
    assert_eq!(
        fragment_count, 1,
        "expected exactly 1 fragment (declarative emit); built-in \
         ContentAdapter would have created an additional UUID-suffixed one: {:?}",
        ids
    );

    h.shutdown().await;
}

// ── T2: Mixed adapter types on same context ────────────────────────────

/// Given a context with both the built-in ContentAdapter and a
/// declarative adapter declaring a lens,
/// when content is ingested via both input_kinds,
/// then both adapters produce nodes + tagged_with edges, the
/// CoOccurrenceEnrichment fires on both, and the lens translates
/// may_be_related edges originating from either path. The consumer
/// owns the vocabulary layer; the adapter category that produced the
/// source edges is irrelevant to the lens.
#[tokio::test]
async fn t2_mixed_adapter_types_share_lens() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t2.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;

    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t2"})).await));

    // Declarative spec: produces fragment + 2 concepts + 2 tagged_with
    // edges from one input, plus a lens translating may_be_related to
    // thematic_connection.
    let spec_yaml = r#"
adapter_id: trellis-fragment
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "fragment:{input.id}"
      type: fragment
      dimension: structure
  - create_node:
      id: "concept:{input.tag1}"
      type: concept
      dimension: semantic
  - create_node:
      id: "concept:{input.tag2}"
      type: concept
      dimension: semantic
  - create_edge:
      source: "fragment:{input.id}"
      target: "concept:{input.tag1}"
      relationship: tagged_with
  - create_edge:
      source: "fragment:{input.id}"
      target: "concept:{input.tag2}"
      relationship: tagged_with
"#;
    assert!(!is_error(
        &h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await
    ));

    // Ingest via declarative adapter
    let resp = h.call_tool("ingest", json!({
        "data": {"id": "frag-1", "tag1": "travel", "tag2": "routine"},
        "input_kind": "trellis.fragment",
    })).await;
    assert!(!is_error(&resp), "declarative ingest failed: {}", resp);

    // Ingest via built-in content adapter on the same context
    let resp = h.call_tool("ingest", json!({
        "data": {"text": "nature and stillness", "tags": ["nature", "stillness"]},
        "input_kind": "content",
    })).await;
    assert!(!is_error(&resp), "content ingest failed: {}", resp);

    // Lens should cover concepts from BOTH adapter paths
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:"}),
    ).await;
    let c = node_count(&resp);
    assert!(
        c >= 4,
        "lens:trellis: should touch concepts from both ingests \
         (travel, routine, nature, stillness) — got {} nodes",
        c
    );

    h.shutdown().await;
}

// ── T3: Spec persistence across MCP restart ────────────────────────────

/// Given a first `plexus mcp` process that loads a spec with a lens,
/// ingests, and shuts down,
/// when a second `plexus mcp` process spawns against the same DB and
/// ingests new content (without re-loading the spec),
/// then the persisted lens fires on the new ingest — proving the
/// specs table rehydration path holds end-to-end through the MCP
/// transport (Invariant 62 effect b, verified at the library-mode
/// process boundary rather than the in-process API boundary).
#[tokio::test]
async fn t3_spec_persistence_across_mcp_restart() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t3.db");

    // ── First process: load spec, ingest, shutdown ────────────────────
    {
        let mut h = McpHarness::spawn(&db).await;
        h.initialize().await;
        assert!(!is_error(&h.call_tool("set_context", json!({"name": "t3"})).await));

        let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
        assert!(!is_error(
            &h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await
        ));

        // Ingest via built-in content adapter; co_occurrence + lens fire
        let resp = h.call_tool("ingest", json!({
            "data": {"text": "first run", "tags": ["travel", "routine"]},
            "input_kind": "content",
        })).await;
        assert!(!is_error(&resp), "first ingest failed: {}", resp);

        // Lens baseline
        let before_resp = h.call_tool(
            "find_nodes",
            json!({"relationship_prefix": "lens:trellis:"}),
        ).await;
        let before = node_count(&before_resp);
        assert!(before >= 2, "first process: lens:trellis: should touch ≥2 nodes, got {}", before);

        h.shutdown().await;
    }

    // ── Second process: same DB, no re-load, new ingest ───────────────
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t3"})).await));

    // Pre-restart lens edges persist in the graph (effect a durable)
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:"}),
    ).await;
    let carried_over = node_count(&resp);
    assert!(
        carried_over >= 2,
        "pre-restart lens edges should persist — got {} nodes",
        carried_over
    );

    // New ingest — persisted lens must fire (effect b: durable registration)
    let resp = h.call_tool("ingest", json!({
        "data": {"text": "second run", "tags": ["nature", "stillness"]},
        "input_kind": "content",
    })).await;
    assert!(!is_error(&resp), "second ingest failed: {}", resp);

    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:"}),
    ).await;
    let after = node_count(&resp);
    assert!(
        after > carried_over,
        "persisted lens should fire on second-process ingest (Invariant 62 effect b) — \
         coverage should grow from {} to >{}, got {}",
        carried_over, carried_over, after
    );

    h.shutdown().await;
}

// ── T4: unload_spec lifecycle through MCP ──────────────────────────────

/// Given a loaded spec whose lens has written vocabulary edges,
/// when unload_spec deregisters the adapter + lens,
/// then a subsequent ingest's new may_be_related edges are NOT translated
/// into lens edges (lens is deregistered from the enrichment loop), but
/// the pre-unload lens edges remain queryable (Invariant 62: vocabulary
/// is durable graph data, not derived from live registration).
///
/// Also verifies the specs table row is deleted — the surviving evidence
/// is the absence of cross-restart rehydration, tested here by checking
/// that post-unload the lens is gone from the current process's registry.
/// (A full across-restart test would re-spawn; this one keeps scope tight.)
#[tokio::test]
async fn t4_unload_spec_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t4.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t4"})).await));

    let spec_yaml = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    assert!(!is_error(
        &h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await
    ));

    // First ingest: lens fires, vocabulary edges created
    assert!(!is_error(&h.call_tool("ingest", json!({
        "data": {"text": "first", "tags": ["travel", "routine"]},
        "input_kind": "content",
    })).await));

    let before_unload_resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:"}),
    ).await;
    let before_unload = node_count(&before_unload_resp);
    assert!(
        before_unload >= 2,
        "pre-unload lens should cover ≥2 nodes, got {}",
        before_unload
    );

    // Unload the spec
    let resp = h.call_tool("unload_spec", json!({"adapter_id": "trellis-content"})).await;
    assert!(!is_error(&resp), "unload_spec failed: {}", resp);

    // Pre-unload lens edges persist (Invariant 62: durable graph data)
    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:"}),
    ).await;
    let after_unload_same = node_count(&resp);
    assert_eq!(
        after_unload_same, before_unload,
        "unload_spec should not retract pre-unload lens edges — \
         before: {}, after unload: {}",
        before_unload, after_unload_same
    );

    // Second ingest: lens is deregistered → new may_be_related edges
    // should NOT produce new lens:trellis: edges
    assert!(!is_error(&h.call_tool("ingest", json!({
        "data": {"text": "second", "tags": ["nature", "stillness"]},
        "input_kind": "content",
    })).await));

    let resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:"}),
    ).await;
    let after_second_ingest = node_count(&resp);
    assert_eq!(
        after_second_ingest, before_unload,
        "post-unload ingest should not expand lens coverage — \
         unloaded lens should not fire on new edges. \
         before_unload: {}, after unload+new ingest: {}",
        before_unload, after_second_ingest
    );

    h.shutdown().await;
}

// ── T5: Error paths through MCP ────────────────────────────────────────

/// Error-path coverage at the MCP boundary. Two failure classes are
/// distinguished deliberately:
/// - **RPC-level error** — handler short-circuits with `McpError` before
///   calling the API (e.g. missing active context). Surfaces in
///   `response.error`.
/// - **Tool-level error** — handler returns Ok(CallToolResult) with
///   `isError: true` (e.g. validation failure). Surfaces in
///   `response.result.isError`.
///
/// Both channels must carry messages that name the failure class.
#[tokio::test]
async fn t5_error_paths() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t5.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;

    // ── No active context → RPC-level error ───────────────────────────
    let valid_spec = "adapter_id: foo\ninput_kind: bar\nemit:\n  - create_node:\n      id: \"c:x\"\n      type: concept\n      dimension: semantic\n";
    let resp = h.call_tool("load_spec", json!({"spec_yaml": valid_spec})).await;
    assert!(
        is_rpc_error(&resp),
        "load_spec without set_context should raise RPC-level error: {}",
        resp
    );
    let msg = rpc_error_message(&resp);
    assert!(
        msg.to_lowercase().contains("context"),
        "RPC error should name the missing-context class — got: {}",
        msg
    );

    // ── Set context, then malformed YAML → tool-level error ───────────
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t5"})).await));

    let resp = h.call_tool(
        "load_spec",
        json!({"spec_yaml": "not: [valid: yaml: spec"}),
    ).await;
    assert!(
        is_error(&resp),
        "malformed YAML should be tool-level isError: {}",
        resp
    );
    let msg = tool_result_text(&resp);
    assert!(
        msg.to_lowercase().contains("validation")
            || msg.to_lowercase().contains("yaml")
            || msg.to_lowercase().contains("parse"),
        "tool error should name the validation class — got: {}",
        msg
    );

    h.shutdown().await;
}

// ── T9: N-consumer cross-pollination ───────────────────────────────────

/// Given four consumers on the same context, each with a distinct lens
/// namespace translating `may_be_related`,
/// when each consumer ingests one fragment via the built-in content
/// adapter,
/// then each consumer's vocabulary namespace covers concepts from
/// every other consumer's content (co_occurrence produces
/// may_be_related between each ingest's concept pair; every registered
/// lens fires on every may_be_related edge regardless of which
/// consumer's ingest produced it).
///
/// The two-consumer case (mcp_e2e) already proves the mechanism.
/// This test catches regressions where a future change would
/// inadvertently hardcode a 2-consumer assumption.
#[tokio::test]
async fn t9_n_consumer_cross_pollination() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t9.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t9"})).await));

    let consumers = [
        ("alpha", "link_a"),
        ("beta", "link_b"),
        ("gamma", "link_c"),
        ("delta", "link_d"),
    ];
    let ingest_pairs = [
        ("river", "stones"),
        ("forest", "moss"),
        ("city", "glass"),
        ("desert", "wind"),
    ];

    for (consumer, to_rel) in &consumers {
        let yaml = format!(
            r#"
adapter_id: {consumer}-content
input_kind: {consumer}.fragment
lens:
  consumer: {consumer}
  translations:
    - from: [may_be_related]
      to: {to_rel}
emit:
  - create_node:
      id: "concept:{{input.name}}"
      type: concept
      dimension: semantic
"#,
            consumer = consumer,
            to_rel = to_rel
        );
        let resp = h.call_tool("load_spec", json!({"spec_yaml": yaml})).await;
        assert!(!is_error(&resp), "load_spec {} failed: {}", consumer, resp);
    }

    for (tag_a, tag_b) in &ingest_pairs {
        let resp = h.call_tool("ingest", json!({
            "data": {"text": format!("{} and {}", tag_a, tag_b), "tags": [tag_a, tag_b]},
            "input_kind": "content",
        })).await;
        assert!(!is_error(&resp), "ingest ({}, {}) failed: {}", tag_a, tag_b, resp);
    }

    // Each of the four lens namespaces should touch concepts drawn from
    // all four ingests. Each ingest creates 2 concepts; 4 ingests = 8
    // unique concepts; each lens should cover all 8.
    for (consumer, _) in &consumers {
        let resp = h.call_tool(
            "find_nodes",
            json!({"relationship_prefix": format!("lens:{}:", consumer)}),
        ).await;
        let n = node_count(&resp);
        assert_eq!(
            n, 8,
            "lens:{}: should cover all 8 concepts from 4 ingests (N-consumer cross-pollination) — got {}",
            consumer, n
        );
    }

    h.shutdown().await;
}

// ── T10: Consumer cycling — load → unload → reload with modified spec ──

/// Given a loaded spec with lens translating may_be_related →
/// thematic_connection,
/// when the consumer unloads the spec and reloads the same adapter_id
/// with a modified lens (may_be_related → conceptual_link),
/// then:
/// - pre-unload `lens:trellis:thematic_connection` edges persist
///   (Invariant 62: vocabulary edges are durable graph data)
/// - the reloaded lens's initial sweep creates
///   `lens:trellis:conceptual_link` edges over pre-unload content
/// - subsequent ingests produce `lens:trellis:conceptual_link` edges
///   but NOT new `lens:trellis:thematic_connection` edges (the old
///   lens is deregistered)
/// - both target relationships are queryable simultaneously
///
/// Keeps `lens.consumer: trellis` stable so the namespace is shared;
/// only the translation target changes. Exercises the spec update
/// path ADR-037 describes (retract + load_spec — minus the retract
/// half, since old edges are allowed to persist as durable data).
#[tokio::test]
async fn t10_consumer_cycling_reload_with_modified_spec() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t10.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t10"})).await));

    let spec_v1 = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: thematic_connection
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    assert!(!is_error(
        &h.call_tool("load_spec", json!({"spec_yaml": spec_v1})).await
    ));

    // First ingest: produces may_be_related + lens:trellis:thematic_connection
    assert!(!is_error(&h.call_tool("ingest", json!({
        "data": {"text": "first", "tags": ["travel", "routine"]},
        "input_kind": "content",
    })).await));

    let thematic_before = node_count(&h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:thematic_connection"}),
    ).await);
    assert!(
        thematic_before >= 2,
        "pre-unload thematic_connection should cover ≥2 nodes, got {}",
        thematic_before
    );

    // Unload
    assert!(!is_error(
        &h.call_tool("unload_spec", json!({"adapter_id": "trellis-content"})).await
    ));

    // thematic_connection edges persist through unload (Invariant 62)
    let thematic_after_unload = node_count(&h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:thematic_connection"}),
    ).await);
    assert_eq!(
        thematic_after_unload, thematic_before,
        "thematic_connection edges should persist through unload — before: {}, after: {}",
        thematic_before, thematic_after_unload
    );

    // Reload with modified translation (same adapter_id, same namespace)
    let spec_v2 = r#"
adapter_id: trellis-content
input_kind: trellis.fragment
lens:
  consumer: trellis
  translations:
    - from: [may_be_related]
      to: conceptual_link
emit:
  - create_node:
      id: "concept:{input.name}"
      type: concept
      dimension: semantic
"#;
    let reload_resp = h.call_tool("load_spec", json!({"spec_yaml": spec_v2})).await;
    assert!(!is_error(&reload_resp), "reload failed: {}", reload_resp);

    // Reload's initial sweep translates existing may_be_related edges
    // into conceptual_link. The sweep's count should be > 0.
    let reload_result = tool_result_json(&reload_resp);
    assert!(
        reload_result["vocabulary_edges_created"].as_u64().unwrap_or(0) > 0,
        "reload initial sweep should create ≥1 conceptual_link edge over pre-unload content"
    );

    // Both translation targets are simultaneously present
    let thematic_after_reload = node_count(&h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:thematic_connection"}),
    ).await);
    let conceptual_after_reload = node_count(&h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:conceptual_link"}),
    ).await);
    assert_eq!(
        thematic_after_reload, thematic_before,
        "thematic_connection unchanged by reload (old lens deregistered, edges durable)"
    );
    assert!(
        conceptual_after_reload >= 2,
        "conceptual_link should cover the pre-unload concepts post-reload sweep — got {}",
        conceptual_after_reload
    );

    // Second ingest: only the new lens fires; thematic_connection must NOT grow
    assert!(!is_error(&h.call_tool("ingest", json!({
        "data": {"text": "second", "tags": ["nature", "stillness"]},
        "input_kind": "content",
    })).await));

    let thematic_after_second_ingest = node_count(&h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:thematic_connection"}),
    ).await);
    let conceptual_after_second_ingest = node_count(&h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:trellis:conceptual_link"}),
    ).await);
    assert_eq!(
        thematic_after_second_ingest, thematic_before,
        "post-reload ingest must NOT grow thematic_connection (old lens deregistered)"
    );
    assert!(
        conceptual_after_second_ingest > conceptual_after_reload,
        "post-reload ingest must grow conceptual_link (new lens fires) — \
         before: {}, after: {}",
        conceptual_after_reload, conceptual_after_second_ingest
    );

    h.shutdown().await;
}
