# Plexus Semantic Analysis: Implementation Spec

> **Status**: Draft
> **Date**: 2025-12-19
> **Depends on**: SPIKE-OUTCOME.md (final decisions)

## Overview

This spec details the implementation of LLM-based semantic extraction for Plexus. It covers API contracts, data flow, error handling, and test requirements.

---

## Phase 1: Core Extraction + Clawmarks Integration

### Goal

Enable `SemanticAnalyzer` to:
1. Route documents to appropriate ensemble based on content type/size
2. Invoke llm-orc for extraction
3. Create clawmarks for provenance tracking
4. Store concepts in plexus graph with clawmark references

### Components

```
┌─────────────────────────────────────────────────────────────────┐
│                      SemanticAnalyzer                            │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ContentRouter │  │ LlmOrcClient │  │ClawmarksClient│          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│         │                 │                 │                   │
│         ▼                 ▼                 ▼                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    ConceptStore                          │   │
│  │  (writes to plexus graph with clawmark_id references)    │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## API Contracts

### Rust Types (crates/plexus)

```rust
// ============================================
// src/analysis/semantic/mod.rs
// ============================================

use crate::graph::{NodeId, Edge, PlexusGraph};
use serde::{Deserialize, Serialize};

/// Configuration for semantic analysis
#[derive(Debug, Clone)]
pub struct SemanticConfig {
    /// Word count threshold for compositional extraction
    pub compositional_threshold: usize,  // Default: 3000

    /// Minimum confidence to store a concept
    pub min_confidence: f64,  // Default: 0.5

    /// Whether to create clawmarks for provenance
    pub enable_clawmarks: bool,  // Default: true

    /// llm-orc server URL
    pub llm_orc_url: String,

    /// clawmarks MCP server name
    pub clawmarks_server: Option<String>,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            compositional_threshold: 3000,
            min_confidence: 0.5,
            enable_clawmarks: true,
            llm_orc_url: "http://localhost:8080".to_string(),
            clawmarks_server: Some("clawmarks".to_string()),
        }
    }
}

/// Content type detection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Technical,  // Wiki, docs, code
    Literary,   // Plays, fiction, prose
    Unknown,
}

/// Extraction strategy based on routing
#[derive(Debug, Clone)]
pub enum ExtractionStrategy {
    /// Single-pass extraction with specified ensemble
    Direct { ensemble: String },

    /// Chunk → fan-out → aggregate → synthesize
    Compositional { ensemble: String },
}

/// A concept extracted from content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// Canonical name (lowercase, singular)
    pub name: String,

    /// Type classification
    pub concept_type: ConceptType,

    /// Extraction confidence (0.0 - 1.0)
    pub confidence: f64,

    /// Text evidence from source
    pub evidence: Option<String>,

    /// Source line number (for clawmarks)
    pub source_line: Option<usize>,

    /// Clawmark ID after recording (populated post-extraction)
    pub clawmark_id: Option<String>,

    /// Which node this concept came from
    pub source_node: Option<NodeId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConceptType {
    Topic,       // Broad subject areas
    Technology,  // Tools, languages, frameworks
    Entity,      // Named things (characters, places)
    Action,      // Operations, functions
    Theme,       // Abstract ideas (literary)
    Pattern,     // Design patterns, techniques
}

/// A relationship between concepts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRelationship {
    pub source: String,
    pub target: String,
    pub relationship_type: RelationshipType,
    pub confidence: f64,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    Implements,
    Describes,
    DependsOn,
    RelatedTo,
    PartOf,
    Uses,
    Creates,
}

/// Result of semantic extraction
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Extracted concepts
    pub concepts: Vec<Concept>,

    /// Relationships between concepts
    pub relationships: Vec<ConceptRelationship>,

    /// Extraction metadata
    pub metadata: ExtractionMetadata,
}

#[derive(Debug, Clone)]
pub struct ExtractionMetadata {
    /// Which ensemble was used
    pub ensemble: String,

    /// Strategy used (direct or compositional)
    pub strategy: String,

    /// Number of chunks (if compositional)
    pub chunk_count: Option<usize>,

    /// Clawmarks trail ID (if enabled)
    pub trail_id: Option<String>,

    /// Total extraction time in ms
    pub duration_ms: u64,
}

/// Errors from semantic extraction
#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("llm-orc invocation failed: {0}")]
    LlmOrcError(String),

    #[error("Failed to parse extraction response: {0}")]
    ParseError(String),

    #[error("Clawmarks integration failed: {0}")]
    ClawmarksError(String),

    #[error("Content routing failed: {0}")]
    RoutingError(String),

    #[error("Graph update failed: {0}")]
    GraphError(String),
}
```

### SemanticAnalyzer Trait

```rust
// ============================================
// src/analysis/semantic/analyzer.rs
// ============================================

use async_trait::async_trait;

/// Main interface for semantic analysis
#[async_trait]
pub trait SemanticAnalyzer: Send + Sync {
    /// Extract concepts from a document
    async fn extract(
        &self,
        content: &str,
        file_path: &str,
        node_id: Option<NodeId>,
    ) -> Result<ExtractionResult, SemanticError>;

    /// Extract from multiple documents (batch)
    async fn extract_batch(
        &self,
        items: Vec<(&str, &str, Option<NodeId>)>,  // (content, path, node_id)
    ) -> Vec<Result<ExtractionResult, SemanticError>>;

    /// Store extraction results in the graph
    async fn store_concepts(
        &self,
        result: &ExtractionResult,
        source_node: NodeId,
    ) -> Result<Vec<NodeId>, SemanticError>;
}

/// Default implementation
pub struct DefaultSemanticAnalyzer {
    config: SemanticConfig,
    router: ContentRouter,
    llm_client: LlmOrcClient,
    clawmarks: Option<ClawmarksClient>,
    graph: Arc<PlexusGraph>,
}

impl DefaultSemanticAnalyzer {
    pub fn new(
        config: SemanticConfig,
        graph: Arc<PlexusGraph>,
    ) -> Result<Self, SemanticError> {
        let llm_client = LlmOrcClient::new(&config.llm_orc_url)?;
        let clawmarks = config.clawmarks_server.as_ref()
            .map(|s| ClawmarksClient::new(s))
            .transpose()?;

        Ok(Self {
            router: ContentRouter::new(config.compositional_threshold),
            config,
            llm_client,
            clawmarks,
            graph,
        })
    }
}

#[async_trait]
impl SemanticAnalyzer for DefaultSemanticAnalyzer {
    async fn extract(
        &self,
        content: &str,
        file_path: &str,
        node_id: Option<NodeId>,
    ) -> Result<ExtractionResult, SemanticError> {
        let start = std::time::Instant::now();

        // 1. Route to appropriate strategy
        let strategy = self.router.route(content);

        // 2. Create trail if clawmarks enabled
        let trail_id = if let Some(ref clawmarks) = self.clawmarks {
            let trail_name = format!(
                "{}-extraction-{}",
                file_path.split('/').last().unwrap_or("unknown"),
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            );
            Some(clawmarks.create_trail(&trail_name, None).await?)
        } else {
            None
        };

        // 3. Run extraction
        let mut result = match &strategy {
            ExtractionStrategy::Direct { ensemble } => {
                self.extract_direct(content, ensemble).await?
            }
            ExtractionStrategy::Compositional { ensemble } => {
                self.extract_compositional(content, file_path, ensemble).await?
            }
        };

        // 4. Create clawmarks for each concept
        if let Some(ref clawmarks) = self.clawmarks {
            if let Some(ref trail) = trail_id {
                for concept in &mut result.concepts {
                    let clawmark_id = clawmarks.add_clawmark(
                        trail,
                        file_path,
                        concept.source_line.unwrap_or(1),
                        &format!("{}: {}", concept.name, concept.evidence.as_deref().unwrap_or("")),
                        "reference",
                        vec![format!("#{}", concept.concept_type.as_str())],
                    ).await?;
                    concept.clawmark_id = Some(clawmark_id);
                }
            }
        }

        // 5. Populate metadata
        result.metadata = ExtractionMetadata {
            ensemble: match &strategy {
                ExtractionStrategy::Direct { ensemble } => ensemble.clone(),
                ExtractionStrategy::Compositional { ensemble } => ensemble.clone(),
            },
            strategy: match &strategy {
                ExtractionStrategy::Direct { .. } => "direct".to_string(),
                ExtractionStrategy::Compositional { .. } => "compositional".to_string(),
            },
            chunk_count: match &strategy {
                ExtractionStrategy::Compositional { .. } => Some(result.concepts.len()),
                _ => None,
            },
            trail_id,
            duration_ms: start.elapsed().as_millis() as u64,
        };

        Ok(result)
    }

    async fn extract_batch(
        &self,
        items: Vec<(&str, &str, Option<NodeId>)>,
    ) -> Vec<Result<ExtractionResult, SemanticError>> {
        // Run extractions concurrently with bounded parallelism
        let semaphore = Arc::new(tokio::sync::Semaphore::new(5));

        let futures = items.into_iter().map(|(content, path, node_id)| {
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await;
                self.extract(content, path, node_id).await
            }
        });

        futures::future::join_all(futures).await
    }

    async fn store_concepts(
        &self,
        result: &ExtractionResult,
        source_node: NodeId,
    ) -> Result<Vec<NodeId>, SemanticError> {
        let mut concept_nodes = Vec::new();

        for concept in &result.concepts {
            // Create or update concept node
            let node_id = self.graph.upsert_node(
                &format!("concept:{}", concept.name),
                "concept",
                |node| {
                    node.set_property("name", &concept.name);
                    node.set_property("concept_type", concept.concept_type.as_str());
                    node.set_property("confidence", concept.confidence);
                    if let Some(ref clawmark) = concept.clawmark_id {
                        node.set_property("clawmark_id", clawmark);
                    }
                    if let Some(ref trail) = result.metadata.trail_id {
                        node.set_property("extraction_trail", trail);
                    }
                }
            ).await.map_err(|e| SemanticError::GraphError(e.to_string()))?;

            // Create edge from source document to concept
            self.graph.add_edge(
                source_node.clone(),
                node_id.clone(),
                "has_concept",
                |edge| {
                    edge.set_property("confidence", concept.confidence);
                    edge.set_property("dimension", "semantic");
                }
            ).await.map_err(|e| SemanticError::GraphError(e.to_string()))?;

            concept_nodes.push(node_id);
        }

        // Create relationship edges
        for rel in &result.relationships {
            let source_id = format!("concept:{}", rel.source);
            let target_id = format!("concept:{}", rel.target);

            self.graph.add_edge_by_name(
                &source_id,
                &target_id,
                rel.relationship_type.as_str(),
                |edge| {
                    edge.set_property("confidence", rel.confidence);
                }
            ).await.ok(); // Ignore if concepts don't exist
        }

        Ok(concept_nodes)
    }
}
```

### ContentRouter

```rust
// ============================================
// src/analysis/semantic/router.rs
// ============================================

pub struct ContentRouter {
    compositional_threshold: usize,
}

impl ContentRouter {
    pub fn new(threshold: usize) -> Self {
        Self { compositional_threshold: threshold }
    }

    pub fn route(&self, content: &str) -> ExtractionStrategy {
        let word_count = content.split_whitespace().count();
        let content_type = self.detect_content_type(content);

        match (content_type, word_count > self.compositional_threshold) {
            // Large documents → compositional
            (_, true) => ExtractionStrategy::Compositional {
                ensemble: "plexus-compositional".to_string(),
            },

            // Literary content → refinement
            (ContentType::Literary, false) => ExtractionStrategy::Direct {
                ensemble: "plexus-refinement".to_string(),
            },

            // Technical content → semantic
            (ContentType::Technical, false) => ExtractionStrategy::Direct {
                ensemble: "plexus-semantic".to_string(),
            },

            // Unknown → semantic (safer default)
            (ContentType::Unknown, false) => ExtractionStrategy::Direct {
                ensemble: "plexus-semantic".to_string(),
            },
        }
    }

    fn detect_content_type(&self, content: &str) -> ContentType {
        let lower = content.to_lowercase();

        // Literary signals
        let literary_signals = [
            "act ", "scene ", "enter ", "exit ", "exeunt",
            "dramatis personae", "chapter ", "verse ",
        ];
        let literary_count = literary_signals.iter()
            .filter(|s| lower.contains(*s))
            .count();

        // Technical signals
        let technical_signals = [
            "```", "function", "class ", "import ", "const ",
            "##", "===", "```rust", "```python", "```javascript",
        ];
        let technical_count = technical_signals.iter()
            .filter(|s| lower.contains(*s))
            .count();

        if literary_count >= 2 {
            ContentType::Literary
        } else if technical_count >= 2 {
            ContentType::Technical
        } else {
            ContentType::Unknown
        }
    }
}
```

### LlmOrcClient

```rust
// ============================================
// src/analysis/semantic/llm_orc.rs
// ============================================

use reqwest::Client;

pub struct LlmOrcClient {
    client: Client,
    base_url: String,
}

impl LlmOrcClient {
    pub fn new(base_url: &str) -> Result<Self, SemanticError> {
        Ok(Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Invoke an ensemble and return the result
    pub async fn invoke(
        &self,
        ensemble: &str,
        input: &str,
    ) -> Result<LlmOrcResponse, SemanticError> {
        let url = format!("{}/invoke", self.base_url);

        let response = self.client
            .post(&url)
            .json(&serde_json::json!({
                "ensemble_name": ensemble,
                "input_data": input,
            }))
            .send()
            .await
            .map_err(|e| SemanticError::LlmOrcError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SemanticError::LlmOrcError(
                format!("HTTP {}: {}", status, body)
            ));
        }

        let result: LlmOrcResponse = response
            .json()
            .await
            .map_err(|e| SemanticError::ParseError(e.to_string()))?;

        Ok(result)
    }
}

#[derive(Debug, Deserialize)]
pub struct LlmOrcResponse {
    pub result: String,
    pub status: String,
}
```

### ClawmarksClient

```rust
// ============================================
// src/analysis/semantic/clawmarks.rs
// ============================================

/// Client for clawmarks MCP server
pub struct ClawmarksClient {
    server_name: String,
}

impl ClawmarksClient {
    pub fn new(server_name: &str) -> Result<Self, SemanticError> {
        Ok(Self {
            server_name: server_name.to_string(),
        })
    }

    /// Create a new trail for an extraction session
    pub async fn create_trail(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<String, SemanticError> {
        // Call MCP tool: mcp__clawmarks__create_trail
        let result = mcp_call(
            &self.server_name,
            "create_trail",
            serde_json::json!({
                "name": name,
                "description": description,
            }),
        ).await?;

        Ok(result["id"].as_str()
            .ok_or_else(|| SemanticError::ClawmarksError("Missing trail id".into()))?
            .to_string())
    }

    /// Add a clawmark to a trail
    pub async fn add_clawmark(
        &self,
        trail_id: &str,
        file: &str,
        line: usize,
        annotation: &str,
        mark_type: &str,
        tags: Vec<String>,
    ) -> Result<String, SemanticError> {
        // Call MCP tool: mcp__clawmarks__add_clawmark
        let result = mcp_call(
            &self.server_name,
            "add_clawmark",
            serde_json::json!({
                "trail_id": trail_id,
                "file": file,
                "line": line,
                "annotation": annotation,
                "type": mark_type,
                "tags": tags,
            }),
        ).await?;

        Ok(result["id"].as_str()
            .ok_or_else(|| SemanticError::ClawmarksError("Missing clawmark id".into()))?
            .to_string())
    }

    /// Link two clawmarks (for relationships)
    pub async fn link_clawmarks(
        &self,
        source_id: &str,
        target_id: &str,
    ) -> Result<(), SemanticError> {
        mcp_call(
            &self.server_name,
            "link_clawmarks",
            serde_json::json!({
                "source_id": source_id,
                "target_id": target_id,
            }),
        ).await?;

        Ok(())
    }
}
```

---

## Data Flow: End-to-End Extraction

### Sequence Diagram

```
User                Manza UI             SemanticAnalyzer        llm-orc          clawmarks
 │                     │                       │                    │                 │
 │  Open document      │                       │                    │                 │
 ├────────────────────►│                       │                    │                 │
 │                     │  analyze(content)     │                    │                 │
 │                     ├──────────────────────►│                    │                 │
 │                     │                       │                    │                 │
 │                     │                       │ route(content)     │                 │
 │                     │                       ├───┐                │                 │
 │                     │                       │   │ Technical,     │                 │
 │                     │                       │◄──┘ <3000 words    │                 │
 │                     │                       │                    │                 │
 │                     │                       │ create_trail()     │                 │
 │                     │                       ├───────────────────────────────────►│
 │                     │                       │                    │    trail_id    │
 │                     │                       │◄───────────────────────────────────┤
 │                     │                       │                    │                 │
 │                     │                       │ invoke("plexus-semantic", content)  │
 │                     │                       ├───────────────────►│                 │
 │                     │                       │                    │ run ensemble   │
 │                     │                       │                    ├───┐            │
 │                     │                       │   concepts[]       │◄──┘            │
 │                     │                       │◄───────────────────┤                 │
 │                     │                       │                    │                 │
 │                     │                       │ for each concept:  │                 │
 │                     │                       │   add_clawmark()   │                 │
 │                     │                       ├───────────────────────────────────►│
 │                     │                       │                    │  clawmark_id   │
 │                     │                       │◄───────────────────────────────────┤
 │                     │                       │                    │                 │
 │                     │                       │ store in graph     │                 │
 │                     │                       ├───┐                │                 │
 │                     │                       │◄──┘                │                 │
 │                     │                       │                    │                 │
 │                     │  ExtractionResult     │                    │                 │
 │                     │◄──────────────────────┤                    │                 │
 │                     │                       │                    │                 │
 │  Show concepts      │                       │                    │                 │
 │◄────────────────────┤                       │                    │                 │
```

### Compositional Flow (Large Documents)

```
SemanticAnalyzer                llm-orc                    chunker.sh
      │                            │                           │
      │  invoke("plexus-compositional", file_path)             │
      ├───────────────────────────►│                           │
      │                            │  run chunker script       │
      │                            ├──────────────────────────►│
      │                            │                           │ read file
      │                            │                           │ split by lines
      │                            │  {"success":true, "data": [...chunks]}
      │                            │◄──────────────────────────┤
      │                            │                           │
      │                            │  fan-out: chunk-extractor[0..N]
      │                            ├───┐                       │
      │                            │   │ parallel extraction   │
      │                            │◄──┘                       │
      │                            │                           │
      │                            │  aggregator               │
      │                            ├───┐                       │
      │                            │◄──┘                       │
      │                            │                           │
      │                            │  synthesizer              │
      │                            ├───┐                       │
      │                            │◄──┘                       │
      │                            │                           │
      │  final synthesis           │                           │
      │◄───────────────────────────┤                           │
```

---

## Error Handling

### Error Categories

| Category | Example | Recovery |
|----------|---------|----------|
| **Transient** | llm-orc timeout | Retry with backoff (3 attempts) |
| **Parse** | Invalid JSON from LLM | Log warning, skip concept |
| **Integration** | Clawmarks unavailable | Continue without provenance |
| **Routing** | Unknown content type | Default to `plexus-semantic` |
| **Graph** | Node creation failed | Fail extraction, report error |

### Retry Strategy

```rust
async fn with_retry<F, T, E>(
    f: F,
    max_attempts: usize,
    backoff_ms: u64,
) -> Result<T, E>
where
    F: Fn() -> Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempts = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts < max_attempts => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(backoff_ms * attempts as u64)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Graceful Degradation

1. **Clawmarks unavailable**: Extract concepts, store in graph without clawmark_id
2. **Partial fan-out failure**: Aggregate successful chunks, note gaps in metadata
3. **Low confidence**: Store concept but mark as `needs_review: true`

---

## Test Plan

### Unit Tests

| Test | File | Coverage |
|------|------|----------|
| ContentRouter.detect_content_type | router_test.rs | Literary vs Technical detection |
| ContentRouter.route | router_test.rs | Strategy selection logic |
| Concept normalization | concept_test.rs | Lowercase, singular transforms |
| LlmOrcResponse parsing | llm_orc_test.rs | JSON deserialization |

### Integration Tests

| Test | Setup | Assertion |
|------|-------|-----------|
| Direct extraction | Mock llm-orc | Returns concepts with clawmark_ids |
| Compositional extraction | Mock llm-orc with fan-out | Aggregates chunk results |
| Clawmarks integration | Real clawmarks MCP | Trail and clawmarks created |
| Graph storage | In-memory graph | Nodes and edges created |

### Acceptance Criteria (BDD)

```gherkin
Feature: Semantic Extraction

  Scenario: Extract concepts from technical document
    Given a markdown file "docker.md" with 500 words
    When I run semantic extraction
    Then the router selects "plexus-semantic" ensemble
    And concepts are extracted with >80% grounding
    And each concept has a clawmark_id
    And the graph contains concept nodes linked to the document

  Scenario: Extract from large literary document
    Given a Shakespeare play "hamlet.txt" with 5000 words
    When I run semantic extraction
    Then the router selects "plexus-compositional" ensemble
    And the document is chunked into ~4 chunks
    And chunk extractions are aggregated
    And synthesis produces document-level concepts

  Scenario: Graceful degradation without clawmarks
    Given clawmarks MCP is unavailable
    When I run semantic extraction
    Then extraction completes successfully
    And concepts are stored without clawmark_ids
    And a warning is logged
```

---

## Edge Cases

| Case | Behavior |
|------|----------|
| Empty document | Return empty ExtractionResult, no error |
| Binary file | Return RoutingError("Cannot extract from binary") |
| Very large file (>100k words) | Chunk with 150-line windows, process all |
| Duplicate concepts in chunks | Aggregator merges by name, keeps highest confidence |
| Circular relationships | Store as-is, graph handles cycles |
| LLM returns invalid JSON | Parse what's possible, log warning, continue |
| All chunks timeout | Return partial result with metadata noting failures |

---

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_ORC_URL` | `http://localhost:8080` | llm-orc server URL |
| `CLAWMARKS_SERVER` | `clawmarks` | MCP server name |
| `SEMANTIC_THRESHOLD` | `3000` | Word count for compositional |
| `SEMANTIC_MIN_CONFIDENCE` | `0.5` | Minimum confidence to store |

### Tauri Config

```json
{
  "plexus": {
    "semantic": {
      "enabled": true,
      "llm_orc_url": "http://localhost:8080",
      "clawmarks_enabled": true,
      "compositional_threshold": 3000
    }
  }
}
```

---

## Phase 1 Deliverables

- [ ] `SemanticAnalyzer` trait and `DefaultSemanticAnalyzer` impl
- [ ] `ContentRouter` with type/size detection
- [ ] `LlmOrcClient` for ensemble invocation
- [ ] `ClawmarksClient` for provenance tracking
- [ ] Unit tests for routing and parsing
- [ ] Integration test with mock llm-orc
- [ ] Integration test with real clawmarks MCP

---

## Future Phases (Outline)

### Phase 2: UI Integration
- Concept tooltip component
- "Go to Source" action
- Graph view updates

### Phase 3: Compositional (mostly done)
- Integrate `plexus-compositional` ensemble
- Full-play extraction tests

### Phase 4: Propagation
- PropagationEngine implementation
- Direction-aware filtering
- Decay factor tuning

### Phase 5: Normalization
- Simple transforms (lowercase, singular)
- Variant tracking in clawmarks
