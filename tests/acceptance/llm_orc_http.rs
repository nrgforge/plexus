//! Acceptance: `PLEXUS_LLM_ORC_URL` selects the HTTP llm-orc client in the
//! real `plexus mcp` binary.
//!
//! A stub HTTP listener stands in for `llm-orc serve`. After a file ingest,
//! the coordinator's background semantic extraction asks the client
//! `is_available()`, which for `HttpClient` is `GET /health`. Seeing that
//! request arrive at the stub proves the env var was honored end to end;
//! the stub answers 503 so extraction degrades gracefully (Invariant 47).

use super::mcp_harness::{is_error, McpHarness};
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const STUB_TIMEOUT: Duration = Duration::from_secs(10);

/// Bind a loopback listener that answers every request with 503 and
/// reports each request line on the returned channel.
async fn spawn_recording_stub() -> (String, mpsc::UnboundedReceiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind stub");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { break };
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let request_line = head.lines().next().unwrap_or_default().to_string();
                let _ = tx.send(request_line);
                let body = r#"{"error":"stub: llm-orc not running"}"#;
                let resp = format!(
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (base, rx)
}

#[tokio::test]
async fn env_var_routes_semantic_extraction_to_http_llm_orc() {
    let (base, mut requests) = spawn_recording_stub().await;

    let tmp = TempDir::new().expect("tempdir");
    let db = tmp.path().join("plexus.db");
    let doc = tmp.path().join("note.md");
    std::fs::write(&doc, "# Note\n\nSome text about graphs.\n").expect("write fixture");

    let mut h = McpHarness::spawn_with_env(&db, &[("PLEXUS_LLM_ORC_URL", base.as_str())]).await;
    h.initialize().await;

    let resp = h.call_tool("set_context", json!({"name": "http-wiring"})).await;
    assert!(!is_error(&resp), "set_context failed: {resp}");

    let resp = h
        .call_tool(
            "ingest",
            json!({"data": {"file_path": doc.to_str().expect("utf-8 path")}}),
        )
        .await;
    assert!(!is_error(&resp), "ingest failed: {resp}");

    let request_line = tokio::time::timeout(STUB_TIMEOUT, requests.recv())
        .await
        .expect("llm-orc stub saw no request: PLEXUS_LLM_ORC_URL was not honored")
        .expect("stub channel closed");
    assert!(
        request_line.starts_with("GET /health"),
        "expected the HttpClient health probe first, got {request_line:?}"
    );

    h.shutdown().await;
}
