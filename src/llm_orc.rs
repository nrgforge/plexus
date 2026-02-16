//! llm-orc client — integration with the LLM orchestration service (ADR-021)
//!
//! Defines the client trait and response types for calling llm-orc ensembles.
//! Two implementations:
//! - `SubprocessClient`: spawns `llm-orc mcp serve` and sends MCP JSON-RPC (production)
//! - `MockClient`: returns preconfigured responses (testing)
//!
//! llm-orc runs as a persistent service. Plexus calls it for:
//! - Phase 3 semantic extraction (ADR-021)
//! - On-demand graph analysis (ADR-023)

use async_trait::async_trait;
use rmcp::model::{CallToolRequestParams, Content};
use rmcp::service::Peer;
use rmcp::{RoleClient, ServiceExt};
use rmcp::transport::TokioChildProcess;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
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
    /// The agent's response text or JSON
    #[serde(default)]
    pub response: Option<String>,
    /// Agent status
    #[serde(default)]
    pub status: Option<String>,
    /// Error message if the agent failed
    #[serde(default)]
    pub error: Option<String>,
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
}

impl SubprocessClient {
    /// Create a new subprocess client using the default `llm-orc` command.
    pub fn new() -> Self {
        Self {
            command: "llm-orc".to_string(),
            project_dir: None,
            peer: Mutex::new(None),
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
    async fn connect(&self) -> Result<Peer<RoleClient>, LlmOrcError> {
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

        Ok(peer)
    }

    /// Get or create the MCP peer connection.
    async fn get_peer(&self) -> Result<Peer<RoleClient>, LlmOrcError> {
        let mut guard = self.peer.lock().await;
        if let Some(ref peer) = *guard {
            return Ok(peer.clone());
        }
        let peer = self.connect().await?;
        *guard = Some(peer.clone());
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
        match self.get_peer().await {
            Ok(_) => true,
            Err(_) => false,
        }
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
}
