//! Shared harness for `plexus mcp --transport http` acceptance tests.
//!
//! Spawns the compiled binary bound to an ephemeral loopback port, finds
//! the port from the server's own "listening on" log line, and speaks raw
//! JSON-RPC over MCP streamable HTTP with `reqwest`. Raw HTTP (not an rmcp
//! client) is deliberate, matching `mcp_harness.rs`: the wire format is the
//! contract the plugin CLI and Claude Code's `.mcp.json` depend on.
//!
//! Streamable HTTP specifics exercised here:
//! - every POST sends `Accept: application/json, text/event-stream`
//! - the `initialize` response carries `Mcp-Session-Id`; later requests
//!   echo it back
//! - responses arrive as `text/event-stream`; the JSON-RPC message is the
//!   `data:` line of the last event

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};

pub const CALL_TIMEOUT: Duration = Duration::from_secs(5);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// One running `plexus mcp --transport http` process.
pub struct McpHttpServer {
    child: Child,
    /// e.g. `http://127.0.0.1:54321/mcp`
    pub endpoint: String,
}

impl McpHttpServer {
    pub async fn spawn(db_path: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_plexus"))
            .args([
                "mcp",
                "--transport",
                "http",
                "--bind",
                "127.0.0.1:0",
                "--db",
                db_path.to_str().expect("db path is UTF-8"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn plexus mcp --transport http");

        let stderr = child.stderr.take().expect("stderr piped");
        let mut lines = BufReader::new(stderr).lines();
        let endpoint = timeout(STARTUP_TIMEOUT, async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(idx) = line.find("listening on ") {
                    let url = line[idx + "listening on ".len()..]
                        .split_whitespace()
                        .next()
                        .expect("url after 'listening on'");
                    return url.to_string();
                }
            }
            panic!("server exited before announcing its endpoint");
        })
        .await
        .expect("timeout waiting for 'listening on' line");

        // Keep draining stderr so the child never blocks on a full pipe.
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });

        Self { child, endpoint }
    }

    /// Open a new MCP session against this server (initialize + initialized).
    pub async fn new_session(&self) -> McpHttpSession {
        let mut session = McpHttpSession {
            endpoint: self.endpoint.clone(),
            http: reqwest::Client::new(),
            session_id: None,
            next_id: 1,
        };
        let init = session.initialize().await;
        assert!(init.get("result").is_some(), "initialize failed: {init}");
        session
    }

    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
    }
}

/// One MCP session (one `Mcp-Session-Id`) on a running server.
pub struct McpHttpSession {
    endpoint: String,
    http: reqwest::Client,
    pub session_id: Option<String>,
    next_id: u64,
}

impl McpHttpSession {
    /// POST one JSON-RPC message; returns the raw HTTP response.
    pub async fn post_raw(&self, msg: &Value, session_id: Option<&str>) -> reqwest::Response {
        let mut req = self
            .http
            .post(&self.endpoint)
            .header("Accept", "application/json, text/event-stream")
            .json(msg);
        if let Some(id) = session_id {
            req = req.header("Mcp-Session-Id", id);
        }
        timeout(CALL_TIMEOUT, req.send())
            .await
            .expect("timeout waiting for HTTP response")
            .expect("HTTP request failed")
    }

    /// POST a JSON-RPC request within this session and return the JSON-RPC
    /// response message.
    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let resp = self.post_raw(&msg, self.session_id.as_deref()).await;
        let status = resp.status();
        if let Some(sid) = resp.headers().get("mcp-session-id") {
            self.session_id = Some(sid.to_str().expect("ascii session id").to_string());
        }
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = timeout(CALL_TIMEOUT, resp.text())
            .await
            .expect("timeout reading response body")
            .expect("read body");
        assert!(status.is_success(), "HTTP {status} for {method}: {body}");
        if content_type.starts_with("text/event-stream") {
            parse_last_sse_data(&body)
        } else {
            serde_json::from_str(&body).unwrap_or_else(|e| panic!("not JSON ({e}): {body}"))
        }
    }

    pub async fn initialize(&mut self) -> Value {
        let resp = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {},
                    "clientInfo": { "name": "plexus-http-harness", "version": "0.0.0" }
                }),
            )
            .await;
        // Complete the handshake; notifications get 202 Accepted with no body.
        let notif = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        let ack = self.post_raw(&notif, self.session_id.as_deref()).await;
        assert!(ack.status().is_success(), "initialized notification: HTTP {}", ack.status());
        resp
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments})).await
    }
}

/// The JSON-RPC message carried by the last `data:` line of an SSE body.
fn parse_last_sse_data(body: &str) -> Value {
    let last = body
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .last()
        .unwrap_or_else(|| panic!("no data: line in SSE body: {body:?}"));
    serde_json::from_str(last).unwrap_or_else(|e| panic!("SSE data is not JSON ({e}): {last}"))
}
