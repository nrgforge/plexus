//! Spike 05: Embedding Enrichment Test Drive (ADR-026)
//!
//! Validates the full integration of EmbeddingSimilarityEnrichment inside the
//! IngestPipeline enrichment loop, alongside existing enrichments.
//!
//! **Section 0**: Pairwise similarity diagnostic — raw cosine similarities for all concept pairs
//! **Section 1**: Real FastEmbedEmbedder + InMemoryVectorStore (threshold from diagnostic)
//! **Section 2**: Real FastEmbedEmbedder + SqliteVecStore (persistence)
//! **Section 3**: Full pipeline with embedding + graph analysis via llm-orc
//!
//! Run with:
//!   cargo test --test spike_05_embeddings --features embeddings -- --nocapture --ignored

#[cfg(feature = "embeddings")]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

// ================================================================
// Section 0: Pairwise similarity diagnostic
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature and model download"]
async fn spike_00_pairwise_similarity_diagnostic() {
    use plexus::adapter::{Embedder, FastEmbedEmbedder};

    eprintln!("\n=== Spike 05 Section 0: Pairwise Similarity Diagnostic ===\n");

    let embedder = FastEmbedEmbedder::default_model()
        .expect("failed to initialize FastEmbedEmbedder");

    // All concept labels from the spike tests
    let labels = vec![
        "travel", "voyage", "provence", "avignon", "mediterranean",
        "walking", "france", "cuisine", "history", "adventure",
        "machine-learning", "neural-networks", "api", "microservices",
        "distributed", "architecture", "democracy", "governance",
        "quantum", "computing",
    ];

    let texts: Vec<&str> = labels.iter().copied().collect();
    let vectors = embedder.embed_batch(&texts).expect("embed_batch failed");

    eprintln!("Embedded {} concept labels ({}-dim vectors)\n", labels.len(), vectors[0].len());

    // Compute all pairwise similarities and collect for sorting
    let mut pairs: Vec<(f32, &str, &str)> = Vec::new();
    for i in 0..labels.len() {
        for j in (i + 1)..labels.len() {
            let sim = cosine_similarity(&vectors[i], &vectors[j]);
            pairs.push((sim, labels[i], labels[j]));
        }
    }
    pairs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    // Print all pairs sorted by similarity (descending)
    eprintln!("All pairwise cosine similarities (descending):");
    eprintln!("{:<30} {:>8}", "Pair", "Cosine");
    eprintln!("{}", "-".repeat(40));
    for (sim, a, b) in &pairs {
        let marker = if *sim >= 0.7 {
            " ***"
        } else if *sim >= 0.5 {
            " **"
        } else if *sim >= 0.4 {
            " *"
        } else {
            ""
        };
        eprintln!("{:<30} {:>8.4}{}", format!("{} ↔ {}", a, b), sim, marker);
    }

    // Print distribution summary
    eprintln!("\nDistribution:");
    let above_70 = pairs.iter().filter(|(s, _, _)| *s >= 0.7).count();
    let above_50 = pairs.iter().filter(|(s, _, _)| *s >= 0.5).count();
    let above_40 = pairs.iter().filter(|(s, _, _)| *s >= 0.4).count();
    let above_30 = pairs.iter().filter(|(s, _, _)| *s >= 0.3).count();
    let total = pairs.len();
    eprintln!("  ≥0.7: {} / {} ({:.0}%)", above_70, total, above_70 as f64 / total as f64 * 100.0);
    eprintln!("  ≥0.5: {} / {} ({:.0}%)", above_50, total, above_50 as f64 / total as f64 * 100.0);
    eprintln!("  ≥0.4: {} / {} ({:.0}%)", above_40, total, above_40 as f64 / total as f64 * 100.0);
    eprintln!("  ≥0.3: {} / {} ({:.0}%)", above_30, total, above_30 as f64 / total as f64 * 100.0);

    // Print expected clusters and their actual similarities
    let expected_clusters: Vec<(&str, Vec<&str>)> = vec![
        ("Travel", vec!["travel", "voyage", "provence", "avignon", "mediterranean", "walking", "france", "adventure"]),
        ("Tech", vec!["machine-learning", "neural-networks", "api", "microservices", "distributed", "architecture"]),
        ("Governance", vec!["democracy", "governance"]),
    ];

    eprintln!("\nExpected cluster internal similarities:");
    for (name, members) in &expected_clusters {
        eprintln!("  {}:", name);
        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                let idx_i = labels.iter().position(|l| l == &members[i]).unwrap();
                let idx_j = labels.iter().position(|l| l == &members[j]).unwrap();
                let sim = cosine_similarity(&vectors[idx_i], &vectors[idx_j]);
                eprintln!("    {} ↔ {} = {:.4}", members[i], members[j], sim);
            }
        }
    }

    eprintln!("\n=== Section 0 COMPLETE ===\n");
}

// ================================================================
// Section 1: Real embedder + InMemoryVectorStore
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature and model download"]
async fn spike_real_embedder_in_memory_store() {
    use plexus::adapter::{
        CoOccurrenceEnrichment, EmbeddingSimilarityEnrichment, FastEmbedEmbedder,
        FragmentAdapter, FragmentInput, IngestPipeline, TagConceptBridger,
    };
    use plexus::adapter::Enrichment;
    use plexus::{Context, ContextId, PlexusEngine, dimension};
    use std::sync::Arc;

    eprintln!("\n=== Spike 05 Section 1: Real embedder + InMemoryVectorStore ===\n");

    let engine = Arc::new(PlexusEngine::new());
    let ctx_id = ContextId::from("spike-05-inmemory");
    engine
        .upsert_context(Context::with_id(ctx_id.clone(), "spike-05-inmemory"))
        .unwrap();

    // Real embedder — threshold 0.55 based on diagnostic (section 0)
    // Single-word labels produce lower cosine similarity than sentences;
    // 0.55 captures true semantic clusters while filtering noise
    let embedder = FastEmbedEmbedder::default_model()
        .expect("failed to initialize FastEmbedEmbedder");

    let embedding_enrichment = Arc::new(EmbeddingSimilarityEnrichment::new(
        "nomic-embed-text-v1.5",
        0.55,
        "similar_to",
        Box::new(embedder),
    ));

    let mut pipeline = IngestPipeline::new(engine.clone());
    pipeline.register_integration(
        Arc::new(FragmentAdapter::new("spike-05")),
        vec![
            Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
            Arc::new(CoOccurrenceEnrichment::new()) as Arc<dyn Enrichment>,
            embedding_enrichment as Arc<dyn Enrichment>,
        ],
    );

    // 5 fragments with semantically clustered tags
    let fragments = vec![
        // Travel cluster
        ("Planning a trip to Provence", vec!["travel", "provence"]),
        ("Avignon walking tour notes", vec!["travel", "avignon"]),
        ("Mediterranean cruise itinerary", vec!["voyage", "mediterranean"]),
        // ML cluster
        ("Neural network training pipeline", vec!["machine-learning", "neural-networks"]),
        // Governance
        ("Local council meeting notes", vec!["democracy", "governance"]),
    ];

    for (text, tags) in &fragments {
        let input = FragmentInput::new(
            *text,
            tags.iter().map(|t| t.to_string()).collect(),
        );
        pipeline
            .ingest("spike-05-inmemory", "fragment", Box::new(input))
            .await
            .unwrap();
    }

    let ctx = engine.get_context(&ctx_id).unwrap();

    // Print all concept nodes
    let concepts: Vec<_> = ctx
        .nodes()
        .filter(|n| n.dimension == dimension::SEMANTIC)
        .collect();
    eprintln!("Concept nodes ({}):", concepts.len());
    for c in &concepts {
        eprintln!("  {}", c.id);
    }

    // Print similarity edges
    let similar_to: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "similar_to")
        .collect();
    eprintln!("\nSimilar_to edges ({}):", similar_to.len());
    for e in &similar_to {
        eprintln!("  {} → {} (weight: {:.4})", e.source, e.target, e.raw_weight);
    }

    // Print co-occurrence edges
    let may_be_related: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "may_be_related")
        .collect();
    eprintln!("\nMay_be_related edges ({}):", may_be_related.len());
    for e in &may_be_related {
        eprintln!("  {} → {} (weight: {:.4})", e.source, e.target, e.raw_weight);
    }

    // Assert: at least some similar_to edges produced
    assert!(
        !similar_to.is_empty(),
        "real embedder should produce at least some similar_to edges"
    );

    eprintln!("\n=== Section 1 PASSED ===\n");
}

// ================================================================
// Section 2: Real embedder + SqliteVecStore (persistence)
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature and model download"]
async fn spike_real_embedder_sqlite_vec_store() {
    use plexus::adapter::{
        EmbeddingSimilarityEnrichment, FastEmbedEmbedder, FragmentAdapter,
        FragmentInput, IngestPipeline, TagConceptBridger,
    };
    use plexus::adapter::{Enrichment, VectorStore};
    use plexus::storage::{SqliteVecStore, DEFAULT_EMBEDDING_DIMENSIONS};
    use plexus::{Context, ContextId, NodeId, PlexusEngine};
    use std::sync::Arc;
    use tempfile::TempDir;

    eprintln!("\n=== Spike 05 Section 2: Real embedder + SqliteVecStore ===\n");

    let tmpdir = TempDir::new().expect("failed to create temp dir");
    let vec_db_path = tmpdir.path().join("vectors.db");

    let engine = Arc::new(PlexusEngine::new());
    let ctx_id = ContextId::from("spike-05-sqlite-vec");
    engine
        .upsert_context(Context::with_id(ctx_id.clone(), "spike-05-sqlite-vec"))
        .unwrap();

    // Real embedder + persistent SqliteVecStore — threshold 0.55
    let embedder = FastEmbedEmbedder::default_model()
        .expect("failed to initialize FastEmbedEmbedder");
    let vec_store = SqliteVecStore::open(&vec_db_path, DEFAULT_EMBEDDING_DIMENSIONS)
        .expect("failed to open SqliteVecStore");

    let embedding_enrichment = Arc::new(EmbeddingSimilarityEnrichment::with_vector_store(
        "nomic-embed-text-v1.5",
        0.55,
        "similar_to",
        Box::new(embedder),
        Box::new(vec_store),
    ));

    let mut pipeline = IngestPipeline::new(engine.clone());
    pipeline.register_integration(
        Arc::new(FragmentAdapter::new("spike-05-sqlite")),
        vec![
            Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
            embedding_enrichment as Arc<dyn Enrichment>,
        ],
    );

    // Ingest 3 fragments
    let fragments = vec![
        ("Exploring ancient Roman roads", vec!["travel", "history"]),
        ("Backpacking through Europe", vec!["travel", "adventure"]),
        ("Quantum computing breakthroughs", vec!["quantum", "computing"]),
    ];

    for (text, tags) in &fragments {
        let input = FragmentInput::new(
            *text,
            tags.iter().map(|t| t.to_string()).collect(),
        );
        pipeline
            .ingest("spike-05-sqlite-vec", "fragment", Box::new(input))
            .await
            .unwrap();
    }

    let ctx = engine.get_context(&ctx_id).unwrap();

    // Print similarity edges
    let similar_to: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "similar_to")
        .collect();
    eprintln!("Similar_to edges after ingestion ({}):", similar_to.len());
    for e in &similar_to {
        eprintln!("  {} → {} (weight: {:.4})", e.source, e.target, e.raw_weight);
    }

    // Reopen SqliteVecStore from same path — verify persistence
    let vec_store_2 = SqliteVecStore::open(&vec_db_path, DEFAULT_EMBEDDING_DIMENSIONS)
        .expect("failed to reopen SqliteVecStore");

    // Check that stored concepts are still present
    let has_travel = vec_store_2.has("spike-05-sqlite-vec", &NodeId::from_string("concept:travel"));
    let has_history = vec_store_2.has("spike-05-sqlite-vec", &NodeId::from_string("concept:history"));
    let has_quantum = vec_store_2.has("spike-05-sqlite-vec", &NodeId::from_string("concept:quantum"));

    eprintln!("\nPersistence check (reopened store):");
    eprintln!("  concept:travel present: {}", has_travel);
    eprintln!("  concept:history present: {}", has_history);
    eprintln!("  concept:quantum present: {}", has_quantum);

    assert!(has_travel, "concept:travel should persist in SqliteVecStore");
    assert!(has_history, "concept:history should persist in SqliteVecStore");
    assert!(has_quantum, "concept:quantum should persist in SqliteVecStore");

    eprintln!("\n=== Section 2 PASSED ===\n");
}

// ================================================================
// Section 3: Full pipeline — ingestion + embedding + graph analysis
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature, model download, and optionally llm-orc"]
async fn spike_full_pipeline_with_graph_analysis() {
    use plexus::adapter::{
        CoOccurrenceEnrichment, EmbeddingSimilarityEnrichment, FastEmbedEmbedder,
        FragmentAdapter, FragmentInput, GraphAnalysisAdapter, IngestPipeline,
        TagConceptBridger, EngineSink, FrameworkContext,
        run_analysis,
    };
    use plexus::adapter::{Adapter, AdapterInput, Enrichment};
    use plexus::llm_orc::{LlmOrcClient, SubprocessClient};
    use plexus::storage::{SqliteVecStore, DEFAULT_EMBEDDING_DIMENSIONS};
    use plexus::{Context, ContextId, PlexusEngine, dimension};
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    eprintln!("\n=== Spike 05 Section 3: Full pipeline + graph analysis ===\n");

    let tmpdir = TempDir::new().expect("failed to create temp dir");
    let vec_db_path = tmpdir.path().join("vectors.db");

    let engine = Arc::new(PlexusEngine::new());
    let ctx_id = ContextId::from("spike-05-full");
    engine
        .upsert_context(Context::with_id(ctx_id.clone(), "spike-05-full"))
        .unwrap();

    // Real embedder + SqliteVecStore — threshold 0.55
    let embedder = FastEmbedEmbedder::default_model()
        .expect("failed to initialize FastEmbedEmbedder");
    let vec_store = SqliteVecStore::open(&vec_db_path, DEFAULT_EMBEDDING_DIMENSIONS)
        .expect("failed to open SqliteVecStore");

    let embedding_enrichment = Arc::new(EmbeddingSimilarityEnrichment::with_vector_store(
        "nomic-embed-text-v1.5",
        0.55,
        "similar_to",
        Box::new(embedder),
        Box::new(vec_store),
    ));

    let mut pipeline = IngestPipeline::new(engine.clone());
    pipeline.register_integration(
        Arc::new(FragmentAdapter::new("spike-05-full")),
        vec![
            Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
            Arc::new(CoOccurrenceEnrichment::new()) as Arc<dyn Enrichment>,
            embedding_enrichment as Arc<dyn Enrichment>,
        ],
    );

    // Ingest 5 fragments with tech/travel tags
    let fragments = vec![
        ("Planning a road trip through France", vec!["travel", "france"]),
        ("Walking tours in historic cities", vec!["travel", "walking"]),
        ("API design patterns for microservices", vec!["api", "microservices"]),
        ("Distributed systems architecture", vec!["distributed", "architecture"]),
        ("French cuisine and cooking techniques", vec!["france", "cuisine"]),
    ];

    for (text, tags) in &fragments {
        let input = FragmentInput::new(
            *text,
            tags.iter().map(|t| t.to_string()).collect(),
        );
        pipeline
            .ingest("spike-05-full", "fragment", Box::new(input))
            .await
            .unwrap();
    }

    let ctx = engine.get_context(&ctx_id).unwrap();

    // --- Print graph state ---
    let concepts: Vec<_> = ctx
        .nodes()
        .filter(|n| n.dimension == dimension::SEMANTIC)
        .collect();
    eprintln!("Concept nodes ({}):", concepts.len());
    for c in &concepts {
        eprintln!("  {}", c.id);
    }

    let similar_to: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "similar_to")
        .collect();
    eprintln!("\nSimilar_to edges ({}):", similar_to.len());
    for e in &similar_to {
        eprintln!("  {} → {} (weight: {:.4})", e.source, e.target, e.raw_weight);
    }

    let may_be_related: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "may_be_related")
        .collect();
    eprintln!("\nMay_be_related edges ({}):", may_be_related.len());
    for e in &may_be_related {
        eprintln!("  {} → {} (weight: {:.4})", e.source, e.target, e.raw_weight);
    }

    // Assert: embedding enrichment produced results
    assert!(
        !similar_to.is_empty(),
        "embedding enrichment should produce similar_to edges with threshold 0.55"
    );

    // --- Attempt graph analysis via llm-orc (graceful degradation) ---
    let llm_orc_dir = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let repo_root = std::path::Path::new(manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new(manifest_dir));
        repo_root.join(".llm-orc").to_string_lossy().to_string()
    };

    let client = SubprocessClient::new()
        .with_project_dir(&llm_orc_dir);

    eprintln!("\nChecking llm-orc availability...");
    if client.is_available().await {
        eprintln!("llm-orc available — running graph analysis");

        match run_analysis(&client, "graph-analysis", &ctx).await {
            Ok(results) => {
                eprintln!("Analysis results ({} algorithms):", results.len());

                // Apply results via GraphAnalysisAdapter (following cmd_analyze pattern)
                let shared_ctx = Arc::new(Mutex::new(ctx.clone()));
                for (algo_name, input) in &results {
                    let adapter = GraphAnalysisAdapter::new(algo_name.as_str());
                    let sink = EngineSink::new(shared_ctx.clone())
                        .with_framework_context(FrameworkContext {
                            adapter_id: adapter.id().to_string(),
                            context_id: ctx_id.as_str().to_string(),
                            input_summary: Some(format!("analysis:{}", algo_name)),
                        });

                    let adapter_input = AdapterInput::new(
                        adapter.input_kind(),
                        input.clone(),
                        &ctx_id.as_str().to_string(),
                    );
                    match adapter.process(&adapter_input, &sink).await {
                        Ok(()) => {
                            eprintln!("  {} — {} node updates", algo_name, input.results.len());
                        }
                        Err(e) => {
                            eprintln!("  {} — failed: {}", algo_name, e);
                        }
                    }
                }

                // Print enriched node properties
                let updated_ctx = shared_ctx.lock().unwrap();
                eprintln!("\nEnriched node properties:");
                for node in updated_ctx.nodes().filter(|n| n.dimension == dimension::SEMANTIC) {
                    let props: Vec<String> = node
                        .properties
                        .iter()
                        .filter(|(k, _)| k.as_str() != "label")
                        .map(|(k, v)| format!("{}={:?}", k, v))
                        .collect();
                    if !props.is_empty() {
                        eprintln!("  {} — {}", node.id, props.join(", "));
                    }
                }
            }
            Err(e) => {
                eprintln!("Graph analysis failed (non-fatal): {}", e);
            }
        }
    } else {
        eprintln!("llm-orc not available — skipping graph analysis (embedding results still valid)");
    }

    // Final assertion: embedding enrichment results are valid regardless of llm-orc
    assert!(
        similar_to.len() >= 1,
        "should have at least 1 similar_to edge from embedding enrichment"
    );

    eprintln!("\n=== Section 3 PASSED ===\n");
}

mod common;

// ================================================================
// Section 4: Corpus-scale embedding test (pkm-webdev)
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature, model download, and test corpus"]
async fn spike_corpus_pkm_webdev_embeddings() {
    use common::corpus::TestCorpus;
    use plexus::adapter::{
        CoOccurrenceEnrichment, EmbeddingSimilarityEnrichment, FastEmbedEmbedder,
        FragmentAdapter, FragmentInput, IngestPipeline, TagConceptBridger,
    };
    use plexus::adapter::Enrichment;
    use plexus::{Context, ContextId, PlexusEngine, dimension};
    use std::collections::HashMap;
    use std::sync::Arc;

    eprintln!("\n=== Spike 05 Section 4: Corpus-scale embeddings (pkm-webdev) ===\n");

    // --- Load corpus ---
    let corpus = TestCorpus::load("pkm-webdev").expect("failed to load pkm-webdev corpus");
    eprintln!("Loaded corpus: {} files\n", corpus.file_count);

    // --- Set up pipeline ---
    let engine = Arc::new(PlexusEngine::new());
    let ctx_id = ContextId::from("spike-05-corpus");
    engine
        .upsert_context(Context::with_id(ctx_id.clone(), "spike-05-corpus"))
        .unwrap();

    let embedder = FastEmbedEmbedder::default_model()
        .expect("failed to initialize FastEmbedEmbedder");

    let embedding_enrichment = Arc::new(EmbeddingSimilarityEnrichment::new(
        "nomic-embed-text-v1.5",
        0.55,
        "similar_to",
        Box::new(embedder),
    ));

    let mut pipeline = IngestPipeline::new(engine.clone());
    pipeline.register_integration(
        Arc::new(FragmentAdapter::new("corpus-ingest")),
        vec![
            Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
            Arc::new(CoOccurrenceEnrichment::new()) as Arc<dyn Enrichment>,
            embedding_enrichment as Arc<dyn Enrichment>,
        ],
    );

    // --- Extract tags and ingest each document as a fragment ---
    let wiki_re = regex_lite::Regex::new(r"\[\[([^\]|]+)").unwrap();
    let mut ingested = 0;

    for item in &corpus.items {
        let path_str = item.path.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Skip templates and README
        if path_str.contains("__resources__") || path_str == "README.md" {
            continue;
        }

        // Extract tags from multiple signals:
        // 1. Directory name (topic category)
        let dir_tag = item.path.as_ref()
            .and_then(|p| p.components().next())
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase().replace('.', ""));

        // 2. H1 title (main concept)
        let h1_tag = item.content.lines()
            .find(|l| l.starts_with("# "))
            .map(|l| l[2..].trim().to_lowercase());

        // 3. Wikilinks (cross-references)
        let wiki_tags: Vec<String> = wiki_re.captures_iter(&item.content)
            .map(|cap| cap.get(1).unwrap().as_str().to_lowercase())
            .collect();

        let mut tags: Vec<String> = Vec::new();
        if let Some(d) = dir_tag {
            if !d.is_empty() { tags.push(d); }
        }
        if let Some(h) = &h1_tag {
            if !tags.contains(h) { tags.push(h.clone()); }
        }
        for w in &wiki_tags {
            if !tags.contains(w) { tags.push(w.clone()); }
        }

        if tags.is_empty() {
            continue;
        }

        // Truncate content to first 500 chars for fragment text
        let text = if item.content.len() > 500 {
            &item.content[..500]
        } else {
            &item.content
        };

        let input = FragmentInput::new(text, tags)
            .with_source(&path_str);

        pipeline
            .ingest("spike-05-corpus", "fragment", Box::new(input))
            .await
            .unwrap();
        ingested += 1;
    }
    eprintln!("Ingested {} documents as fragments\n", ingested);

    // --- Analyze results ---
    let ctx = engine.get_context(&ctx_id).unwrap();

    let concepts: Vec<_> = ctx
        .nodes()
        .filter(|n| n.dimension == dimension::SEMANTIC)
        .collect();
    eprintln!("Concept nodes: {}", concepts.len());

    let similar_to: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "similar_to")
        .collect();
    let may_be_related: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "may_be_related")
        .collect();
    let references: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "references")
        .collect();

    eprintln!("Edges:");
    eprintln!("  similar_to (embedding):    {}", similar_to.len());
    eprintln!("  may_be_related (co-occur): {}", may_be_related.len());
    eprintln!("  references (tag-bridge):   {}", references.len());

    // --- Print similar_to edges sorted by weight ---
    let mut sorted_similar: Vec<_> = similar_to.iter()
        .filter(|e| e.source.as_str() < e.target.as_str()) // dedupe symmetric pairs
        .collect();
    sorted_similar.sort_by(|a, b| b.raw_weight.partial_cmp(&a.raw_weight).unwrap());

    eprintln!("\nTop similar_to pairs (embedding-only discoveries):");
    for (i, e) in sorted_similar.iter().take(30).enumerate() {
        let src = e.source.as_str().strip_prefix("concept:").unwrap_or(e.source.as_str());
        let tgt = e.target.as_str().strip_prefix("concept:").unwrap_or(e.target.as_str());
        eprintln!("  {:2}. {} ↔ {} ({:.4})", i + 1, src, tgt, e.raw_weight);
    }

    // --- Identify embedding-only edges (not also found by co-occurrence) ---
    let cooccur_pairs: std::collections::HashSet<(String, String)> = may_be_related.iter()
        .map(|e| {
            let (a, b) = if e.source.as_str() < e.target.as_str() {
                (e.source.to_string(), e.target.to_string())
            } else {
                (e.target.to_string(), e.source.to_string())
            };
            (a, b)
        })
        .collect();

    let embedding_only: Vec<_> = sorted_similar.iter()
        .filter(|e| {
            let (a, b) = if e.source.as_str() < e.target.as_str() {
                (e.source.to_string(), e.target.to_string())
            } else {
                (e.target.to_string(), e.source.to_string())
            };
            !cooccur_pairs.contains(&(a, b))
        })
        .collect();

    eprintln!("\nEmbedding-only edges (not found by co-occurrence): {}", embedding_only.len());
    for (i, e) in embedding_only.iter().take(20).enumerate() {
        let src = e.source.as_str().strip_prefix("concept:").unwrap_or(e.source.as_str());
        let tgt = e.target.as_str().strip_prefix("concept:").unwrap_or(e.target.as_str());
        eprintln!("  {:2}. {} ↔ {} ({:.4})", i + 1, src, tgt, e.raw_weight);
    }

    // --- Cluster analysis: group concepts by their similar_to neighbors ---
    let mut adjacency: HashMap<String, Vec<(String, f32)>> = HashMap::new();
    for e in &similar_to {
        adjacency.entry(e.source.to_string())
            .or_default()
            .push((e.target.to_string(), e.raw_weight));
    }

    // Find concepts with most similarity connections (hubs)
    let mut hub_counts: Vec<_> = adjacency.iter()
        .map(|(id, neighbors)| (id.clone(), neighbors.len()))
        .collect();
    hub_counts.sort_by(|a, b| b.1.cmp(&a.1));

    eprintln!("\nSemantic hubs (most similar_to connections):");
    for (id, count) in hub_counts.iter().take(10) {
        let label = id.strip_prefix("concept:").unwrap_or(id);
        eprintln!("  {} — {} connections", label, count);
    }

    // --- Summary stats ---
    let total_concept_pairs = concepts.len() * (concepts.len() - 1) / 2;
    let similar_pair_count = sorted_similar.len();
    let selectivity = similar_pair_count as f64 / total_concept_pairs as f64 * 100.0;

    eprintln!("\n--- Summary ---");
    eprintln!("  Documents ingested:     {}", ingested);
    eprintln!("  Concepts created:       {}", concepts.len());
    eprintln!("  Possible concept pairs: {}", total_concept_pairs);
    eprintln!("  Similar pairs (≥0.55):  {} ({:.1}% selectivity)", similar_pair_count, selectivity);
    eprintln!("  Embedding-only pairs:   {}", embedding_only.len());
    eprintln!("  Co-occurrence pairs:    {}", may_be_related.len() / 2); // symmetric

    assert!(
        !similar_to.is_empty(),
        "corpus should produce at least some similar_to edges"
    );

    eprintln!("\n=== Section 4 PASSED ===\n");
}
