//! Graph analysis adapter (ADR-023)
//!
//! Thin adapter that ingests results from external graph analysis
//! (e.g., PageRank, community detection) as property updates on existing nodes.
//!
//! Graph analysis runs outside the enrichment loop — it is an adapter,
//! not an enrichment (Invariant 49). Results enter via `ingest()` and
//! the standard pipeline (including enrichments) fires after.
//!
//! Each algorithm has a distinct adapter ID (e.g., `graph-analysis:pagerank`)
//! for contribution tracking.

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{Emission, PropertyUpdate};
use crate::graph::{Context, NodeId, PropertyValue};
use crate::llm_orc::{InvokeResponse, LlmOrcClient, LlmOrcError};
use async_trait::async_trait;

/// Input for the graph analysis adapter.
///
/// Contains a list of node property updates from an analysis run.
#[derive(Debug, Clone)]
pub struct GraphAnalysisInput {
    pub results: Vec<NodePropertyUpdate>,
}

/// A single node property update from analysis.
#[derive(Debug, Clone)]
pub struct NodePropertyUpdate {
    pub node_id: String,
    pub properties: Vec<(String, PropertyValue)>,
}

/// Adapter that applies graph analysis results as property updates.
///
/// Parameterized by algorithm name for stable, unique adapter IDs.
/// e.g., `graph-analysis:pagerank`, `graph-analysis:community`.
pub struct GraphAnalysisAdapter {
    algorithm: String,
    adapter_id: String,
    input_kind: String,
}

impl GraphAnalysisAdapter {
    pub fn new(algorithm: impl Into<String>) -> Self {
        let algo = algorithm.into();
        let adapter_id = format!("graph-analysis:{}", algo);
        let input_kind = format!("graph-analysis:{}", algo);
        Self {
            algorithm: algo,
            adapter_id,
            input_kind,
        }
    }

    /// The algorithm this adapter handles.
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }
}

#[async_trait]
impl Adapter for GraphAnalysisAdapter {
    fn id(&self) -> &str {
        &self.adapter_id
    }

    fn input_kind(&self) -> &str {
        &self.input_kind
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let analysis_input = input
            .downcast_data::<GraphAnalysisInput>()
            .ok_or(AdapterError::InvalidInput)?;

        let mut emission = Emission::new();

        for result in &analysis_input.results {
            let node_id = NodeId::from_string(&result.node_id);
            let mut update = PropertyUpdate::new(node_id);
            for (key, value) in &result.properties {
                update.properties.insert(key.clone(), value.clone());
            }
            emission = emission.with_property_update(update);
        }

        if !emission.is_empty() {
            sink.emit(emission).await?;
        }

        Ok(())
    }
}

// --- On-demand analysis orchestration (ADR-023, Scenario 3) ---

/// Export a context's graph to JSON for llm-orc script agents.
///
/// Produces a compact JSON representation with node IDs, types,
/// labels, and edges with relationships and weights.
pub fn export_graph_for_analysis(ctx: &Context) -> String {
    let nodes: Vec<serde_json::Value> = ctx
        .nodes()
        .map(|n| {
            let label = n
                .properties
                .get("label")
                .and_then(|pv| match pv {
                    PropertyValue::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            serde_json::json!({
                "id": n.id.as_str(),
                "type": n.node_type,
                "dimension": n.dimension,
                "label": label,
            })
        })
        .collect();

    let edges: Vec<serde_json::Value> = ctx
        .edges()
        .map(|e| {
            serde_json::json!({
                "source": e.source.as_str(),
                "target": e.target.as_str(),
                "relationship": e.relationship,
                "weight": e.raw_weight,
            })
        })
        .collect();

    serde_json::json!({
        "nodes": nodes,
        "edges": edges,
        "node_count": nodes.len(),
        "edge_count": edges.len(),
    })
    .to_string()
}

/// Parse an llm-orc analysis response into `GraphAnalysisInput`.
///
/// Each agent in the ensemble is expected to return JSON with an
/// `updates` array:
/// ```json
/// { "updates": [{ "node_id": "concept:x", "properties": { "score": 0.5 } }] }
/// ```
pub fn parse_analysis_response(
    response: &InvokeResponse,
) -> Result<Vec<(String, GraphAnalysisInput)>, String> {
    let mut results = Vec::new();

    for (agent_name, agent_result) in &response.results {
        let response_text = match &agent_result.response {
            Some(text) => text,
            None => continue,
        };

        if !agent_result.is_success() {
            continue;
        }

        let parsed: serde_json::Value = serde_json::from_str(response_text)
            .map_err(|e| format!("failed to parse {} response: {}", agent_name, e))?;

        let updates = match parsed.get("updates").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => continue,
        };

        let mut node_updates = Vec::new();
        for update in updates {
            let node_id = update
                .get("node_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if node_id.is_empty() {
                continue;
            }

            let props_obj = match update.get("properties").and_then(|v| v.as_object()) {
                Some(obj) => obj,
                None => continue,
            };

            let mut properties = Vec::new();
            for (key, value) in props_obj {
                let pv = match value {
                    serde_json::Value::Number(n) => {
                        if let Some(f) = n.as_f64() {
                            PropertyValue::Float(f)
                        } else if let Some(i) = n.as_i64() {
                            PropertyValue::Int(i)
                        } else {
                            continue;
                        }
                    }
                    serde_json::Value::String(s) => PropertyValue::String(s.clone()),
                    serde_json::Value::Bool(b) => PropertyValue::Bool(*b),
                    _ => continue,
                };
                properties.push((key.clone(), pv));
            }

            node_updates.push(NodePropertyUpdate {
                node_id: node_id.to_string(),
                properties,
            });
        }

        if !node_updates.is_empty() {
            results.push((
                agent_name.clone(),
                GraphAnalysisInput {
                    results: node_updates,
                },
            ));
        }
    }

    Ok(results)
}

/// Run on-demand graph analysis on a context.
///
/// 1. Exports the context graph as JSON
/// 2. Invokes the llm-orc graph-analysis ensemble
/// 3. Parses per-agent results
/// 4. Returns `(algorithm, GraphAnalysisInput)` pairs for ingestion
///
/// The caller is responsible for feeding each pair into the
/// appropriate `GraphAnalysisAdapter` via `ingest()`.
pub async fn run_analysis(
    client: &dyn LlmOrcClient,
    ensemble_name: &str,
    ctx: &Context,
) -> Result<Vec<(String, GraphAnalysisInput)>, LlmOrcError> {
    if !client.is_available().await {
        return Err(LlmOrcError::Unavailable("llm-orc not running".to_string()));
    }

    let input_data = export_graph_for_analysis(ctx);

    let response = client.invoke(ensemble_name, &input_data).await?;

    if response.is_failed() {
        return Err(LlmOrcError::InvocationFailed(
            "graph analysis ensemble failed".to_string(),
        ));
    }

    parse_analysis_response(&response)
        .map_err(|e| LlmOrcError::ParseError(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::graph::{ContentType, Context, Edge, Node, NodeId, PropertyValue};
    use crate::llm_orc::{MockClient, InvokeResponse, AgentResult};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    fn make_sink(
        ctx: Arc<Mutex<Context>>,
        adapter_id: &str,
    ) -> EngineSink {
        EngineSink::new(ctx).with_framework_context(FrameworkContext {
            adapter_id: adapter_id.to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        })
    }

    fn concept(id: &str) -> Node {
        let mut n = Node::new("concept", ContentType::Concept);
        n.id = NodeId::from_string(id);
        n.properties.insert(
            "label".to_string(),
            PropertyValue::String(id.to_string()),
        );
        n
    }

    // --- Scenario: Graph analysis results enter via ingest (ADR-023) ---

    #[tokio::test]
    async fn graph_analysis_results_update_existing_node_properties() {
        let adapter = GraphAnalysisAdapter::new("pagerank");
        assert_eq!(adapter.id(), "graph-analysis:pagerank");
        assert_eq!(adapter.input_kind(), "graph-analysis:pagerank");

        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = make_sink(ctx.clone(), adapter.id());

        // Pre-populate context with concept nodes
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(concept("concept:travel"));
            c.add_node(concept("concept:jazz"));
        }

        // Graph analysis produces property updates
        let input = AdapterInput::new(
            adapter.input_kind(),
            GraphAnalysisInput {
                results: vec![
                    NodePropertyUpdate {
                        node_id: "concept:travel".to_string(),
                        properties: vec![
                            ("pagerank_score".to_string(), PropertyValue::Float(0.034)),
                        ],
                    },
                    NodePropertyUpdate {
                        node_id: "concept:jazz".to_string(),
                        properties: vec![
                            ("pagerank_score".to_string(), PropertyValue::Float(0.087)),
                        ],
                    },
                ],
            },
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Existing properties preserved, new properties merged
        let travel = snapshot
            .get_node(&NodeId::from_string("concept:travel"))
            .expect("travel node should exist");
        assert_eq!(
            travel.properties.get("label"),
            Some(&PropertyValue::String("concept:travel".to_string())),
            "original property preserved"
        );
        assert_eq!(
            travel.properties.get("pagerank_score"),
            Some(&PropertyValue::Float(0.034)),
            "analysis property merged"
        );

        let jazz = snapshot
            .get_node(&NodeId::from_string("concept:jazz"))
            .expect("jazz node should exist");
        assert_eq!(
            jazz.properties.get("pagerank_score"),
            Some(&PropertyValue::Float(0.087)),
        );
    }

    // --- Scenario: Multiple analysis algorithms have distinct adapter IDs ---

    #[test]
    fn distinct_adapter_ids_per_algorithm() {
        let pagerank = GraphAnalysisAdapter::new("pagerank");
        let community = GraphAnalysisAdapter::new("community");

        assert_eq!(pagerank.id(), "graph-analysis:pagerank");
        assert_eq!(community.id(), "graph-analysis:community");
        assert_ne!(pagerank.id(), community.id());

        assert_eq!(pagerank.input_kind(), "graph-analysis:pagerank");
        assert_eq!(community.input_kind(), "graph-analysis:community");
    }

    // --- Scenario: Graph analysis does not run in enrichment loop (ADR-023) ---
    //
    // This is a design constraint, not a runtime behavior test.
    // GraphAnalysisAdapter implements Adapter (registered in IngestPipeline),
    // NOT Enrichment (registered in EnrichmentRegistry).
    // The type system enforces this: GraphAnalysisAdapter does not implement
    // the Enrichment trait, so it cannot be registered in EnrichmentRegistry.
    #[test]
    fn graph_analysis_is_adapter_not_enrichment() {
        // GraphAnalysisAdapter implements Adapter
        let adapter = GraphAnalysisAdapter::new("pagerank");
        let _: &dyn Adapter = &adapter;

        // It does NOT implement Enrichment — this is verified by the type system.
        // If someone tried to register it as an enrichment, it wouldn't compile.
    }

    // --- Scenario: Analysis on nonexistent nodes is a no-op ---

    #[tokio::test]
    async fn analysis_on_nonexistent_nodes_is_noop() {
        let adapter = GraphAnalysisAdapter::new("pagerank");
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = make_sink(ctx.clone(), adapter.id());

        // Empty context — no nodes to update
        let input = AdapterInput::new(
            adapter.input_kind(),
            GraphAnalysisInput {
                results: vec![NodePropertyUpdate {
                    node_id: "concept:nonexistent".to_string(),
                    properties: vec![
                        ("pagerank_score".to_string(), PropertyValue::Float(0.5)),
                    ],
                }],
            },
            "test",
        );

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();
        assert_eq!(snapshot.node_count(), 0, "no nodes created for missing targets");
    }

    // --- Scenario: Two analysis adapters contribute independently ---

    // --- Scenario: On-demand graph analysis (ADR-023 Scenario 3) ---

    #[test]
    fn export_graph_produces_json_with_nodes_and_edges() {
        let mut ctx = Context::new("test");
        ctx.add_node(concept("concept:travel"));
        ctx.add_node(concept("concept:jazz"));
        let edge = Edge::new(
            NodeId::from_string("concept:travel"),
            NodeId::from_string("concept:jazz"),
            "related_to",
        );
        ctx.add_edge(edge);

        let json = export_graph_for_analysis(&ctx);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["node_count"], 2);
        assert_eq!(parsed["edge_count"], 1);
        assert!(parsed["nodes"].as_array().unwrap().len() == 2);
        assert!(parsed["edges"].as_array().unwrap().len() == 1);
    }

    #[tokio::test]
    async fn on_demand_analysis_returns_property_updates() {
        // Mock llm-orc returning PageRank + community results
        let mut results = HashMap::new();
        results.insert(
            "pagerank".to_string(),
            AgentResult {
                response: Some(
                    r#"{"updates": [
                        {"node_id": "concept:travel", "properties": {"pagerank_score": 0.034}},
                        {"node_id": "concept:jazz", "properties": {"pagerank_score": 0.087}}
                    ]}"#
                    .to_string(),
                ),
                status: Some("success".to_string()),
                error: None,
            },
        );
        results.insert(
            "community".to_string(),
            AgentResult {
                response: Some(
                    r#"{"updates": [
                        {"node_id": "concept:travel", "properties": {"community": 3}},
                        {"node_id": "concept:jazz", "properties": {"community": 3}}
                    ]}"#
                    .to_string(),
                ),
                status: Some("success".to_string()),
                error: None,
            },
        );

        let mock_client = MockClient::available().with_response(
            "graph-analysis",
            InvokeResponse {
                results,
                status: "completed".to_string(),
                metadata: serde_json::Value::Null,
            },
        );

        let mut ctx = Context::new("test");
        ctx.add_node(concept("concept:travel"));
        ctx.add_node(concept("concept:jazz"));

        let analysis_results = run_analysis(&mock_client, "graph-analysis", &ctx)
            .await
            .unwrap();

        // Two algorithms returned results
        assert_eq!(analysis_results.len(), 2, "pagerank + community");

        // Find pagerank results
        let (_, pr_input) = analysis_results
            .iter()
            .find(|(name, _)| name == "pagerank")
            .expect("pagerank results");
        assert_eq!(pr_input.results.len(), 2);
        assert_eq!(pr_input.results[0].node_id, "concept:travel");

        // Find community results
        let (_, cd_input) = analysis_results
            .iter()
            .find(|(name, _)| name == "community")
            .expect("community results");
        assert_eq!(cd_input.results.len(), 2);

        // Apply results via adapters
        let shared_ctx = Arc::new(Mutex::new(ctx));
        for (algo_name, input) in &analysis_results {
            let adapter = GraphAnalysisAdapter::new(algo_name.as_str());
            let sink = make_sink(shared_ctx.clone(), adapter.id());
            let adapter_input = AdapterInput::new(adapter.input_kind(), input.clone(), "test");
            adapter.process(&adapter_input, &sink).await.unwrap();
        }

        // Verify all properties merged onto nodes
        let snapshot = shared_ctx.lock().unwrap();
        let travel = snapshot
            .get_node(&NodeId::from_string("concept:travel"))
            .expect("travel should exist");
        assert_eq!(
            travel.properties.get("pagerank_score"),
            Some(&PropertyValue::Float(0.034)),
        );
        assert_eq!(
            travel.properties.get("community"),
            Some(&PropertyValue::Float(3.0)), // JSON numbers parse as f64
        );
    }

    #[tokio::test]
    async fn on_demand_analysis_skips_when_unavailable() {
        let client = MockClient::unavailable();
        let ctx = Context::new("test");

        let err = run_analysis(&client, "graph-analysis", &ctx)
            .await
            .unwrap_err();
        assert!(matches!(err, LlmOrcError::Unavailable(_)));
    }

    #[tokio::test]
    async fn two_analysis_adapters_contribute_independently() {
        let pagerank = GraphAnalysisAdapter::new("pagerank");
        let community = GraphAnalysisAdapter::new("community");

        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Pre-populate
        {
            let mut c = ctx.lock().unwrap();
            c.add_node(concept("concept:travel"));
        }

        // PageRank results
        let sink_pr = make_sink(ctx.clone(), pagerank.id());
        let input_pr = AdapterInput::new(
            pagerank.input_kind(),
            GraphAnalysisInput {
                results: vec![NodePropertyUpdate {
                    node_id: "concept:travel".to_string(),
                    properties: vec![
                        ("pagerank_score".to_string(), PropertyValue::Float(0.034)),
                    ],
                }],
            },
            "test",
        );
        pagerank.process(&input_pr, &sink_pr).await.unwrap();

        // Community detection results
        let sink_cd = make_sink(ctx.clone(), community.id());
        let input_cd = AdapterInput::new(
            community.input_kind(),
            GraphAnalysisInput {
                results: vec![NodePropertyUpdate {
                    node_id: "concept:travel".to_string(),
                    properties: vec![
                        ("community".to_string(), PropertyValue::Int(7)),
                    ],
                }],
            },
            "test",
        );
        community.process(&input_cd, &sink_cd).await.unwrap();

        // Both properties should be present
        let snapshot = ctx.lock().unwrap();
        let travel = snapshot
            .get_node(&NodeId::from_string("concept:travel"))
            .expect("travel should exist");
        assert_eq!(
            travel.properties.get("label"),
            Some(&PropertyValue::String("concept:travel".to_string())),
        );
        assert_eq!(
            travel.properties.get("pagerank_score"),
            Some(&PropertyValue::Float(0.034)),
        );
        assert_eq!(
            travel.properties.get("community"),
            Some(&PropertyValue::Int(7)),
        );
    }
}
