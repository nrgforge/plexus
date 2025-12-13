//! Common test utilities for Plexus spike investigations
//!
//! This module provides shared helpers for loading test corpora,
//! building graphs, and calculating metrics.

pub mod corpus;
pub mod graph_builder;
pub mod metrics;

#[cfg(not(feature = "real_llm"))]
pub mod mock_llm;

pub use corpus::{corpus_root, CorpusError, TestCorpus};
pub use graph_builder::{build_graph_from_corpus, build_structure_graph, BuiltGraph, GraphBuildConfig, GraphBuildError};
pub use metrics::{
    average_degree, connected_components, graph_density, hits, is_reachable, pagerank,
    reachable_count, HitsResult, PageRankResult,
};

#[cfg(not(feature = "real_llm"))]
pub use mock_llm::MockSemanticAnalyzer;
