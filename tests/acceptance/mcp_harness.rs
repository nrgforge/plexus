//! Shared MCP subprocess harness for acceptance tests.
//!
//! Spawns the compiled `plexus mcp --db <path>` binary and speaks raw
//! JSON-RPC over stdin/stdout. Raw JSON-RPC (not an rmcp client) is
//! deliberate: the wire format is a stable contract the tests exercise
//! directly, independent of rmcp crate version churn.
//!
//! Consumers:
//! - `mcp_e2e.rs` — two-consumer cross-pollination (WP-H.2)
//! - `mcp_matrix.rs` — T1/T2/T3/T5 deterministic coverage
//! - `mcp_matrix_llm_orc.rs` — T6/T7/T8 gated (PLEXUS_INTEGRATION=1)

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{timeout, Duration};

pub const CALL_TIMEOUT: Duration = Duration::from_secs(5);
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpHarness {
    pub async fn spawn(db_path: &std::path::Path) -> Self {
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

    pub async fn send_line(&mut self, msg: &Value) {
        let line = serde_json::to_string(msg).expect("serialize message");
        self.stdin.write_all(line.as_bytes()).await.expect("write line");
        self.stdin.write_all(b"\n").await.expect("write newline");
        self.stdin.flush().await.expect("flush");
    }

    pub async fn recv_line(&mut self) -> Value {
        self.recv_line_with_timeout(CALL_TIMEOUT).await
    }

    pub async fn recv_line_with_timeout(&mut self, call_timeout: Duration) -> Value {
        let mut buf = String::new();
        let read = timeout(call_timeout, self.stdout.read_line(&mut buf))
            .await
            .expect("timeout waiting for MCP response line")
            .expect("read response line");
        assert!(read > 0, "server closed stdout before responding");
        serde_json::from_str(&buf).unwrap_or_else(|e| {
            panic!("response is not valid JSON: {}\nline: {:?}", e, buf)
        })
    }

    pub async fn initialize(&mut self) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "plexus-acceptance-harness", "version": "0.0.0" }
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

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.call_tool_with_timeout(name, arguments, CALL_TIMEOUT).await
    }

    /// Call a tool with a custom response timeout.
    ///
    /// Use for operations whose handler may take longer than the default
    /// 5s — notably ingest calls that synchronously invoke llm-orc
    /// (declarative adapters with `ensemble:` fields).
    pub async fn call_tool_with_timeout(
        &mut self,
        name: &str,
        arguments: Value,
        call_timeout: Duration,
    ) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        });
        self.send_line(&req).await;
        self.recv_line_with_timeout(call_timeout).await
    }

    pub async fn shutdown(mut self) {
        drop(self.stdin); // EOF → server exits
        let _ = timeout(SHUTDOWN_TIMEOUT, self.child.wait()).await;
    }
}

// ── Tool-result helpers ────────────────────────────────────────────────

/// Did the tool return a success response with `isError: true`?
///
/// This is the MCP tool-level error channel: the handler ran, returned
/// Ok(CallToolResult) via the `err_text(...)` helper, and the JSON-RPC
/// response shape is `{result: {content: [...], isError: true}}`.
pub fn is_error(response: &Value) -> bool {
    response
        .pointer("/result/isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Did the JSON-RPC layer itself return an error?
///
/// This is the protocol-level error channel: the handler returned
/// Err(McpError), so the response has no `result` field and instead
/// carries `error: {code, message}`. Used for pre-dispatch failures
/// like "no active context" (the handler short-circuits before calling
/// into the API) and unknown tools.
pub fn is_rpc_error(response: &Value) -> bool {
    response.get("error").is_some()
}

/// Extract the RPC error message (from `response.error.message`).
pub fn rpc_error_message(response: &Value) -> String {
    response
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn tool_result_text(response: &Value) -> String {
    response
        .pointer("/result/content/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("response missing result.content[0].text: {}", response))
        .to_string()
}

pub fn tool_result_json(response: &Value) -> Value {
    let text = tool_result_text(response);
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("inner result is not JSON: {}\ntext: {}", e, text))
}

pub fn node_count(find_nodes_response: &Value) -> usize {
    tool_result_json(find_nodes_response)["nodes"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0)
}
