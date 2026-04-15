//! Live MCP subprocess acceptance test (WP-H.2).
//!
//! Spawns the compiled `plexus mcp --db <tmp>` binary, speaks raw JSON-RPC
//! over stdin/stdout, and verifies the two-consumer vocabulary-layer
//! cross-pollination story end-to-end — the product-discovery-defined
//! acceptance criterion for the MCP consumer interaction cycle.
//!
//! Raw JSON-RPC (not an rmcp client) is used deliberately: the wire format
//! is a stable contract that this test exercises directly, independent of
//! rmcp crate version churn. The harness is intentionally small (~100 LoC).

use serde_json::{json, Value};
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{timeout, Duration};

const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpHarness {
    async fn spawn(db_path: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_plexus"))
            .args(["mcp", "--db", db_path.to_str().expect("db path is UTF-8")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn plexus mcp subprocess");

        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));

        Self { child, stdin, stdout, next_id: 1 }
    }

    async fn send_line(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).expect("serialize message");
        self.stdin.write_all(line.as_bytes()).await.expect("write line");
        self.stdin.write_all(b"\n").await.expect("write newline");
        self.stdin.flush().await.expect("flush");
    }

    async fn recv_line(&mut self) -> Value {
        let mut buf = String::new();
        let read = timeout(CALL_TIMEOUT, self.stdout.read_line(&mut buf))
            .await
            .expect("timeout waiting for MCP response line")
            .expect("read response line");
        assert!(read > 0, "server closed stdout before responding");
        serde_json::from_str(&buf).unwrap_or_else(|e| {
            panic!("response is not valid JSON: {}\nline: {:?}", e, buf)
        })
    }

    async fn initialize(&mut self) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "plexus-e2e-harness", "version": "0.0.0" }
            }
        });
        self.send_line(&req).await;
        let resp = self.recv_line().await;

        // Complete the handshake per MCP spec
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.send_line(&notif).await;
        resp
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send_line(&req).await;
        self.recv_line().await
    }

    async fn shutdown(mut self) {
        drop(self.stdin); // EOF → server exits
        let _ = timeout(SHUTDOWN_TIMEOUT, self.child.wait()).await;
    }
}

// ── Tool-result helpers ────────────────────────────────────────────────

fn is_error(response: &Value) -> bool {
    response
        .pointer("/result/isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn tool_result_text(response: &Value) -> String {
    response
        .pointer("/result/content/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("response missing result.content[0].text: {}", response))
        .to_string()
}

fn tool_result_json(response: &Value) -> Value {
    let text = tool_result_text(response);
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("inner result is not JSON: {}\ntext: {}", e, text))
}

fn node_count(find_nodes_response: &Value) -> usize {
    tool_result_json(find_nodes_response)["nodes"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0)
}

// ── The test ───────────────────────────────────────────────────────────

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
