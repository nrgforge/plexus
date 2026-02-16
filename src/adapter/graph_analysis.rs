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
use crate::graph::{NodeId, PropertyValue};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::graph::{ContentType, Context, Node, NodeId, PropertyValue};
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
