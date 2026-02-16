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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}
