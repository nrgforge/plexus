//! llm-orc client — integration with the LLM orchestration service (ADR-021)
//!
//! Defines the client trait and response types for calling llm-orc ensembles.
//! Three implementations:
//! - `SubprocessClient`: spawns `llm-orc m serve` and sends MCP JSON-RPC (production, local)
//! - `HttpClient`: calls a running `llm-orc serve` over REST (production, remote)
//! - `MockClient`: returns preconfigured responses (testing)
//!
//! llm-orc runs as a persistent service. Plexus calls it for:
//! - Semantic extraction (ADR-021)
//! - On-demand external enrichment (ADR-023, vocabulary updated by ADR-024)

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, Content};
use rmcp::service::{Peer, RunningService};
use rmcp::{RoleClient, ServiceExt};
use rmcp::transport::TokioChildProcess;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of invoking an llm-orc ensemble.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    /// Per-agent results (agent name → output)
    pub results: HashMap<String, AgentResult>,
    /// Overall execution status
    pub status: String,
    /// Execution metadata (timing, usage)
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl InvokeResponse {
    pub fn is_completed(&self) -> bool {
        self.status == "completed"
    }

    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }

    pub fn has_errors(&self) -> bool {
        self.status == "completed_with_errors"
    }
}

/// Result from a single agent in an ensemble.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// The agent's response text or JSON.
    /// Fan-out gathered results may be arrays — deserialized as JSON array string.
    #[serde(default, deserialize_with = "deserialize_response")]
    pub response: Option<String>,
    /// Agent status
    #[serde(default)]
    pub status: Option<String>,
    /// Error message if the agent failed
    #[serde(default)]
    pub error: Option<String>,
}

/// Custom deserializer: accepts String, Array (serialized back to JSON string), or null.
fn deserialize_response<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(other) => {
            // Arrays, objects, etc. — serialize back to JSON string
            Ok(Some(serde_json::to_string(&other).unwrap_or_default()))
        }
    }
}

impl AgentResult {
    pub fn is_success(&self) -> bool {
        self.status.as_deref() == Some("success")
    }
}

/// Errors from llm-orc client operations.
#[derive(Debug, thiserror::Error)]
pub enum LlmOrcError {
    #[error("llm-orc not available: {0}")]
    Unavailable(String),
    #[error("ensemble not found: {0}")]
    EnsembleNotFound(String),
    #[error("invocation failed: {0}")]
    InvocationFailed(String),
    #[error("response parse error: {0}")]
    ParseError(String),
}

/// Client trait for calling llm-orc ensembles.
///
/// Abstracts over transport (subprocess, HTTP, mock) so adapters
/// don't depend on how llm-orc is reached.
#[async_trait]
pub trait LlmOrcClient: Send + Sync {
    /// Check if llm-orc is reachable.
    async fn is_available(&self) -> bool;

    /// Invoke an ensemble with input data.
    ///
    /// Returns the full response including per-agent results and metadata.
    async fn invoke(
        &self,
        ensemble_name: &str,
        input_data: &str,
    ) -> Result<InvokeResponse, LlmOrcError>;
}

/// Mock client for testing — returns preconfigured responses.
pub struct MockClient {
    available: bool,
    responses: HashMap<String, Result<InvokeResponse, LlmOrcError>>,
}

impl MockClient {
    /// Create a mock client that reports as available.
    pub fn available() -> Self {
        Self {
            available: true,
            responses: HashMap::new(),
        }
    }

    /// Create a mock client that reports as unavailable.
    pub fn unavailable() -> Self {
        Self {
            available: false,
            responses: HashMap::new(),
        }
    }

    /// Register a response for a specific ensemble name.
    pub fn with_response(
        mut self,
        ensemble_name: impl Into<String>,
        response: InvokeResponse,
    ) -> Self {
        self.responses
            .insert(ensemble_name.into(), Ok(response));
        self
    }

    /// Register a failure for a specific ensemble name.
    pub fn with_failure(
        mut self,
        ensemble_name: impl Into<String>,
        error: LlmOrcError,
    ) -> Self {
        self.responses
            .insert(ensemble_name.into(), Err(error));
        self
    }
}

#[async_trait]
impl LlmOrcClient for MockClient {
    async fn is_available(&self) -> bool {
        self.available
    }

    async fn invoke(
        &self,
        ensemble_name: &str,
        _input_data: &str,
    ) -> Result<InvokeResponse, LlmOrcError> {
        if !self.available {
            return Err(LlmOrcError::Unavailable(
                "mock client configured as unavailable".to_string(),
            ));
        }

        match self.responses.get(ensemble_name) {
            Some(Ok(response)) => Ok(response.clone()),
            Some(Err(_)) => Err(LlmOrcError::InvocationFailed(
                format!("mock failure for ensemble '{}'", ensemble_name),
            )),
            None => Err(LlmOrcError::EnsembleNotFound(
                format!("no mock response for ensemble '{}'", ensemble_name),
            )),
        }
    }
}

/// Production client — spawns `llm-orc m serve --transport stdio` and
/// communicates via MCP JSON-RPC over stdin/stdout.
///
/// The subprocess is spawned lazily on first use and kept alive for the
/// lifetime of the client. The MCP connection is guarded by a mutex so
/// multiple concurrent callers are serialized (llm-orc processes one
/// request at a time anyway).
pub struct SubprocessClient {
    /// The llm-orc command (default: "llm-orc")
    command: String,
    /// Project directory to set via `set_project` on first connect
    project_dir: Option<String>,
    /// Lazily-initialized MCP peer connection
    peer: Mutex<Option<Peer<RoleClient>>>,
    /// Keeps the MCP service alive — dropping this kills the subprocess
    #[allow(dead_code)]
    service: Mutex<Option<RunningService<RoleClient, ()>>>,
}

impl Default for SubprocessClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SubprocessClient {
    /// Create a new subprocess client using the default `llm-orc` command.
    pub fn new() -> Self {
        Self {
            command: "llm-orc".to_string(),
            project_dir: None,
            peer: Mutex::new(None),
            service: Mutex::new(None),
        }
    }

    /// Set a custom command path (e.g., for a virtualenv).
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    /// Set a project directory — `set_project` will be called on first connect.
    pub fn with_project_dir(mut self, dir: impl Into<String>) -> Self {
        self.project_dir = Some(dir.into());
        self
    }

    /// Establish the MCP connection (spawn subprocess + handshake).
    async fn connect(&self) -> Result<(Peer<RoleClient>, RunningService<RoleClient, ()>), LlmOrcError> {
        let mut cmd = tokio::process::Command::new(&self.command);
        cmd.arg("m").arg("serve").arg("--transport").arg("stdio");

        let transport = TokioChildProcess::new(cmd)
            .map_err(|e| LlmOrcError::Unavailable(format!("failed to spawn llm-orc: {}", e)))?;

        // () implements ClientHandler with sensible defaults (no-op handlers)
        let service = ()
            .serve(transport)
            .await
            .map_err(|e| LlmOrcError::Unavailable(format!("MCP handshake failed: {}", e)))?;

        let peer = service.peer().clone();

        // Set project directory if configured
        if let Some(ref dir) = self.project_dir {
            let mut args = serde_json::Map::new();
            args.insert("path".to_string(), serde_json::Value::String(dir.clone()));
            let _ = peer
                .call_tool(CallToolRequestParams {
                    meta: None,
                    name: Cow::Borrowed("set_project"),
                    arguments: Some(args),
                    task: None,
                })
                .await;
        }

        Ok((peer, service))
    }

    /// Get or create the MCP peer connection.
    async fn get_peer(&self) -> Result<Peer<RoleClient>, LlmOrcError> {
        let mut guard = self.peer.lock().await;
        if let Some(ref peer) = *guard {
            return Ok(peer.clone());
        }
        let (peer, running_service) = self.connect().await?;
        *guard = Some(peer.clone());
        // Store the RunningService<RoleClient, ()> to keep the subprocess alive
        *self.service.lock().await = Some(running_service);
        Ok(peer)
    }

    /// Call a tool on the llm-orc MCP server.
    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> Result<String, LlmOrcError> {
        let peer = self.get_peer().await?;

        let result = peer
            .call_tool(CallToolRequestParams {
                meta: None,
                name: Cow::Owned(tool_name.to_string()),
                arguments: Some(arguments),
                task: None,
            })
            .await
            .map_err(|e| LlmOrcError::InvocationFailed(format!("MCP call_tool failed: {}", e)))?;

        if result.is_error == Some(true) {
            let text = extract_text_content(&result.content);
            return Err(LlmOrcError::InvocationFailed(text));
        }

        Ok(extract_text_content(&result.content))
    }
}

/// Extract text from MCP Content items (concatenate all text items).
fn extract_text_content(content: &[Content]) -> String {
    content
        .iter()
        .filter_map(|c| c.as_text().map(|tc| tc.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[async_trait]
impl LlmOrcClient for SubprocessClient {
    async fn is_available(&self) -> bool {
        // Try to connect and list tools — if it works, llm-orc is available
        (self.get_peer().await).is_ok()
    }

    async fn invoke(
        &self,
        ensemble_name: &str,
        input_data: &str,
    ) -> Result<InvokeResponse, LlmOrcError> {
        let mut args = serde_json::Map::new();
        args.insert(
            "ensemble_name".to_string(),
            serde_json::Value::String(ensemble_name.to_string()),
        );
        args.insert(
            "input_data".to_string(),
            serde_json::Value::String(input_data.to_string()),
        );

        let response_text = self.call_tool("invoke", args).await?;

        // llm-orc returns the InvokeResponse as JSON text in the MCP content
        let response: InvokeResponse = serde_json::from_str(&response_text)
            .map_err(|e| LlmOrcError::ParseError(format!("failed to parse invoke response: {}", e)))?;

        Ok(response)
    }
}

/// Production client — talks to a running `llm-orc serve` over its REST
/// surface (`GET /health`, `POST /api/ensembles/{name}/execute`).
///
/// Use this when llm-orc runs as a service on the same host or elsewhere
/// on the network; `SubprocessClient` spawns a local process instead.
/// No auth or special headers — the network boundary is the caller's.
///
/// Ensembles can legitimately run for a long time (the homelab proxy
/// allows 3600 s per request), so there is no overall request timeout;
/// only connection establishment is bounded.
pub struct HttpClient {
    /// Base URL without trailing slash, e.g. `http://127.0.0.1:8765`
    base_url: String,
    http: reqwest::Client,
}

/// Environment variable that selects `HttpClient` when set.
pub const LLM_ORC_URL_ENV: &str = "PLEXUS_LLM_ORC_URL";

impl HttpClient {
    /// Create a client for the llm-orc service at `base_url`.
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client with static configuration");
        Self { base_url, http }
    }

    /// The configured base URL (no trailing slash).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Map a non-2xx execute response into the matching `LlmOrcError`.
    ///
    /// llm-orc serve reports errors as JSON `{"error": "..."}` with an HTTP
    /// status; fall back to the raw body when it isn't JSON.
    fn error_from_response(status: reqwest::StatusCode, body: &str) -> LlmOrcError {
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or_else(|| body.to_string());
        let message = format!("HTTP {}: {}", status.as_u16(), message);
        match status {
            reqwest::StatusCode::NOT_FOUND => LlmOrcError::EnsembleNotFound(message),
            s if s.is_server_error() => LlmOrcError::Unavailable(message),
            _ => LlmOrcError::InvocationFailed(message),
        }
    }
}

#[async_trait]
impl LlmOrcClient for HttpClient {
    async fn is_available(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.http.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                tracing::debug!(url = %url, error = %e, "llm-orc health check failed");
                false
            }
        }
    }

    async fn invoke(
        &self,
        ensemble_name: &str,
        input_data: &str,
    ) -> Result<InvokeResponse, LlmOrcError> {
        let url = format!("{}/api/ensembles/{}/execute", self.base_url, ensemble_name);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "input": input_data }))
            .send()
            .await
            .map_err(|e| LlmOrcError::Unavailable(format!("llm-orc request to {} failed: {}", url, e)))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| LlmOrcError::InvocationFailed(format!("failed to read llm-orc response body: {}", e)))?;

        if !status.is_success() {
            return Err(Self::error_from_response(status, &body));
        }

        serde_json::from_str(&body)
            .map_err(|e| LlmOrcError::ParseError(format!("failed to parse execute response: {}", e)))
    }
}

/// Resolve the llm-orc service URL: an explicit value (CLI flag) wins,
/// otherwise `PLEXUS_LLM_ORC_URL`; empty strings count as unset.
pub fn resolve_url(explicit: Option<String>) -> Option<String> {
    explicit
        .or_else(|| std::env::var(LLM_ORC_URL_ENV).ok())
        .filter(|s| !s.trim().is_empty())
}

/// Choose the production llm-orc client from configuration.
///
/// A resolved URL (see `resolve_url`) selects `HttpClient`; when none is
/// configured the local `SubprocessClient` is used, so existing installs
/// keep their behavior.
pub fn select_client(url: Option<String>) -> Arc<dyn LlmOrcClient> {
    match resolve_url(url) {
        Some(base) => {
            tracing::info!(url = %base, "llm-orc client: HTTP");
            Arc::new(HttpClient::new(base))
        }
        None => {
            tracing::info!("llm-orc client: subprocess (llm-orc m serve --transport stdio)");
            Arc::new(SubprocessClient::new())
        }
    }
}

/// Helper to construct an InvokeResponse for testing.
pub fn mock_response(agents: Vec<(&str, &str)>) -> InvokeResponse {
    let mut results = HashMap::new();
    for (name, response) in agents {
        results.insert(
            name.to_string(),
            AgentResult {
                response: Some(response.to_string()),
                status: Some("success".to_string()),
                error: None,
            },
        );
    }
    InvokeResponse {
        results,
        status: "completed".to_string(),
        metadata: serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_available_client_returns_response() {
        let client = MockClient::available().with_response(
            "test-ensemble",
            mock_response(vec![("agent-a", "result A")]),
        );

        assert!(client.is_available().await);

        let response = client.invoke("test-ensemble", "input").await.unwrap();
        assert!(response.is_completed());
        assert_eq!(response.results.len(), 1);
        assert_eq!(
            response.results["agent-a"].response.as_deref(),
            Some("result A")
        );
    }

    #[tokio::test]
    async fn mock_unavailable_client_returns_error() {
        let client = MockClient::unavailable();

        assert!(!client.is_available().await);

        let err = client.invoke("test-ensemble", "input").await.unwrap_err();
        assert!(matches!(err, LlmOrcError::Unavailable(_)));
    }

    #[tokio::test]
    async fn mock_missing_ensemble_returns_not_found() {
        let client = MockClient::available();

        let err = client.invoke("nonexistent", "input").await.unwrap_err();
        assert!(matches!(err, LlmOrcError::EnsembleNotFound(_)));
    }

    #[tokio::test]
    async fn subprocess_client_reports_unavailable_when_binary_missing() {
        // Use a nonexistent command to ensure graceful handling
        let client = SubprocessClient::new().with_command("__nonexistent_llm_orc_binary__");
        assert!(!client.is_available().await);
    }

    #[tokio::test]
    async fn subprocess_client_invoke_fails_when_binary_missing() {
        let client = SubprocessClient::new().with_command("__nonexistent_llm_orc_binary__");
        let err = client.invoke("test", "input").await.unwrap_err();
        assert!(matches!(err, LlmOrcError::Unavailable(_)));
    }

    // ── HttpClient (llm-orc serve REST surface) ─────────────────────────

    fn find_header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n")
    }

    /// One-shot HTTP/1.1 stub: accepts a single connection, reads the
    /// request head + body, writes a canned response, and returns
    /// `(request_line, request_body)` so tests can assert on what the
    /// client sent.
    async fn stub_server(
        status: &'static str,
        content_type: &'static str,
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<(String, String)>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let n = sock.read(&mut tmp).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = find_header_end(&buf) {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_string();
                    let content_length = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    if buf.len() >= pos + 4 + content_length {
                        break;
                    }
                }
            }
            let pos = find_header_end(&buf).expect("request head");
            let head = String::from_utf8_lossy(&buf[..pos]).to_string();
            let request_line = head.lines().next().unwrap_or_default().to_string();
            let request_body = String::from_utf8_lossy(&buf[pos + 4..]).to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(response.as_bytes()).await.unwrap();
            sock.shutdown().await.ok();
            (request_line, request_body)
        });
        (base, handle)
    }

    #[tokio::test]
    async fn http_client_is_available_on_healthy_response() {
        let (base, server) = stub_server(
            "200 OK",
            "application/json",
            r#"{"status":"healthy","version":"0.20.3"}"#,
        )
        .await;
        let client = HttpClient::new(&base);

        assert!(client.is_available().await);

        let (request_line, _) = server.await.unwrap();
        assert_eq!(request_line, "GET /health HTTP/1.1");
    }

    #[tokio::test]
    async fn http_client_invoke_posts_input_and_parses_envelope() {
        let (base, server) = stub_server(
            "200 OK",
            "application/json",
            r#"{"ensemble":"echo-test","status":"completed","input":"ping","results":{"echo":{"response":"pong","status":"success"}},"metadata":{"duration":"12ms"},"deliverable":"pong"}"#,
        )
        .await;
        let client = HttpClient::new(format!("{}/", base)); // trailing slash tolerated

        let response = client.invoke("echo-test", "ping").await.unwrap();

        assert!(response.is_completed());
        assert_eq!(response.results["echo"].response.as_deref(), Some("pong"));
        assert_eq!(response.metadata["duration"], "12ms");

        let (request_line, body) = server.await.unwrap();
        assert_eq!(request_line, "POST /api/ensembles/echo-test/execute HTTP/1.1");
        let sent: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(sent, serde_json::json!({"input": "ping"}));
    }

    #[tokio::test]
    async fn http_client_maps_503_error_body_to_unavailable() {
        let (base, _server) = stub_server(
            "503 Service Unavailable",
            "application/json",
            r#"{"error":"llama-server router unreachable: connection refused"}"#,
        )
        .await;
        let client = HttpClient::new(&base);

        let err = client.invoke("echo-test", "ping").await.unwrap_err();

        match err {
            LlmOrcError::Unavailable(msg) => assert!(msg.contains("router unreachable"), "{msg}"),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn http_client_non_json_success_body_is_parse_error() {
        let (base, _server) = stub_server("200 OK", "text/html", "<html>oops</html>").await;
        let client = HttpClient::new(&base);

        let err = client.invoke("echo-test", "ping").await.unwrap_err();

        assert!(matches!(err, LlmOrcError::ParseError(_)), "{err:?}");
    }

    #[tokio::test]
    async fn http_client_unreachable_host_is_unavailable() {
        // Port 1 is reserved and nothing listens on it.
        let client = HttpClient::new("http://127.0.0.1:1");

        assert!(!client.is_available().await);
        let err = client.invoke("echo-test", "ping").await.unwrap_err();
        assert!(matches!(err, LlmOrcError::Unavailable(_)), "{err:?}");
    }
}
