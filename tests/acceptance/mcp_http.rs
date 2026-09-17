//! `plexus mcp --transport http` acceptance tests.
//!
//! Contract for tailnet clients (plugin CLI, Claude Code `.mcp.json`, a
//! future Obsidian plugin): one long-lived process, the full tool set over
//! MCP streamable HTTP, and `set_context` scoped to the MCP session while
//! the graph itself is shared.

use super::mcp_harness::{is_error, node_count};
use super::mcp_http_harness::McpHttpServer;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn http_transport_serves_the_full_tool_set() {
    let tmp = TempDir::new().expect("tempdir");
    let server = McpHttpServer::spawn(&tmp.path().join("plexus.db")).await;

    let mut s = server.new_session().await;
    let resp = s.request("tools/list", json!({})).await;
    let names: Vec<&str> = resp["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools/list: {resp}"))
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    for expected in ["set_context", "ingest", "find_nodes", "explain_edge", "load_spec"] {
        assert!(names.contains(&expected), "missing tool {expected}: {names:?}");
    }

    server.shutdown().await;
}

#[tokio::test]
async fn http_sessions_scope_set_context_but_share_the_graph() {
    let tmp = TempDir::new().expect("tempdir");
    let server = McpHttpServer::spawn(&tmp.path().join("plexus.db")).await;

    // Session A sets a context and writes to it.
    let mut a = server.new_session().await;
    let resp = a.call_tool("set_context", json!({"name": "shared"})).await;
    assert!(!is_error(&resp), "set_context: {resp}");
    let resp = a
        .call_tool(
            "ingest",
            json!({"data": {"text": "session A wrote this", "tags": ["http"], "source": "a"}}),
        )
        .await;
    assert!(!is_error(&resp), "ingest: {resp}");

    // Session B has no active context of its own.
    let mut b = server.new_session().await;
    assert_ne!(a.session_id, b.session_id, "sessions must have distinct ids");
    let resp = b.call_tool("find_nodes", json!({})).await;
    let text = resp.to_string();
    assert!(
        text.contains("no context set"),
        "session B must not inherit A's context: {resp}"
    );

    // Once B chooses the same context it sees A's data (shared engine).
    let resp = b.call_tool("set_context", json!({"name": "shared"})).await;
    assert!(!is_error(&resp), "set_context B: {resp}");
    let resp = b.call_tool("find_nodes", json!({"node_type": "fragment"})).await;
    assert!(!is_error(&resp), "find_nodes B: {resp}");
    let nodes = node_count(&resp);
    assert!(
        nodes > 0,
        "session B should see session A's fragment: {resp}"
    );

    server.shutdown().await;
}

#[tokio::test]
async fn http_transport_rejects_unknown_session_id() {
    let tmp = TempDir::new().expect("tempdir");
    let server = McpHttpServer::spawn(&tmp.path().join("plexus.db")).await;

    let s = server.new_session().await;
    let msg = json!({"jsonrpc": "2.0", "id": 99, "method": "tools/list", "params": {}});
    let resp = s.post_raw(&msg, Some("not-a-real-session")).await;

    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

    server.shutdown().await;
}
