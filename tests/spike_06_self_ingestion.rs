//! Spike 06: Plexus Self-Ingestion (Dogfooding)
//!
//! Ingests the Plexus Rust codebase into Plexus itself to see what the graph
//! reveals about its own structure. Tags are extracted via regex from each
//! source file: module path segments, pub structs/traits/enums, impl-for pairs,
//! and ADR references.
//!
//! **Section 0**: Tag extraction diagnostic (no embeddings feature needed)
//! **Section 1**: Full ingestion with all enrichments (embeddings feature)
//! **Section 2**: Persistent graph for MCP querying (embeddings feature)
//!
//! Run with:
//!   cargo test --test spike_06_self_ingestion -- --nocapture --ignored spike_00
//!   cargo test --test spike_06_self_ingestion --features embeddings -- --nocapture --ignored spike_01
//!   cargo test --test spike_06_self_ingestion --features embeddings -- --nocapture --ignored spike_02

use std::collections::HashMap;
use std::path::Path;

/// Extract tags from a Rust source file's content and path.
///
/// Strategy:
/// 1. Module path from file path: `src/adapter/embedding.rs` → ["adapter", "embedding"]
/// 2. `pub struct Foo` → "foo"
/// 3. `pub trait Bar` → "bar"
/// 4. `pub enum Qux` → "qux"
/// 5. `impl Trait for Struct` → both names as tags
/// 6. `ADR-NNN` references → "adr-NNN"
///
/// Tags are lowercased. `pub fn` and `use` imports are skipped (too many, low signal).
fn extract_tags_from_rust(content: &str, file_path: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut add_tag = |tag: String| {
        let tag = tag.to_lowercase();
        if !tag.is_empty() && seen.insert(tag.clone()) {
            tags.push(tag);
        }
    };

    // 1. Module path from file path
    // src/adapter/embedding.rs → ["adapter", "embedding"]
    // Skip "src", "mod", "lib", "main", "bin"
    let skip = ["src", "mod", "lib", "main", "bin", "tests"];
    let path = Path::new(file_path);
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        let s = s.strip_suffix(".rs").unwrap_or(&s);
        if !skip.contains(&s) && !s.is_empty() {
            add_tag(s.to_string());
        }
    }

    // 2-4. pub struct/trait/enum
    let type_re = regex_lite::Regex::new(r"pub\s+(?:struct|trait|enum)\s+(\w+)").unwrap();
    for cap in type_re.captures_iter(content) {
        if let Some(name) = cap.get(1) {
            add_tag(name.as_str().to_string());
        }
    }

    // 5. impl Trait for Struct
    let impl_for_re = regex_lite::Regex::new(r"impl\s+(\w+)\s+for\s+(\w+)").unwrap();
    for cap in impl_for_re.captures_iter(content) {
        if let Some(trait_name) = cap.get(1) {
            add_tag(trait_name.as_str().to_string());
        }
        if let Some(struct_name) = cap.get(2) {
            add_tag(struct_name.as_str().to_string());
        }
    }

    // 6. ADR-NNN references
    let adr_re = regex_lite::Regex::new(r"ADR-(\d{3})").unwrap();
    for cap in adr_re.captures_iter(content) {
        if let Some(num) = cap.get(1) {
            add_tag(format!("adr-{}", num.as_str()));
        }
    }

    tags
}

// ================================================================
// Section 0: Tag extraction diagnostic (no embeddings feature needed)
// ================================================================

#[tokio::test]
#[ignore = "spike — run manually"]
async fn spike_00_tag_extraction_diagnostic() {
    use walkdir::WalkDir;

    eprintln!("\n=== Spike 06 Section 0: Tag Extraction Diagnostic ===\n");

    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(src_dir.exists(), "src/ directory not found at {:?}", src_dir);

    let mut file_tags: Vec<(String, Vec<String>)> = Vec::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;

    for entry in WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(path);
        let content = std::fs::read_to_string(path).unwrap();

        let tags = extract_tags_from_rust(&content, &rel_path.to_string_lossy());
        total_files += 1;

        for tag in &tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }

        file_tags.push((rel_path.to_string_lossy().to_string(), tags));
    }

    // Sort files by tag count (descending)
    file_tags.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    eprintln!("Files scanned: {}", total_files);
    eprintln!("Unique tags: {}\n", tag_counts.len());

    // Top 10 most-tagged files
    eprintln!("Top 10 most-tagged files:");
    for (path, tags) in file_tags.iter().take(10) {
        eprintln!("  {:3} tags — {}", tags.len(), path);
        for tag in tags.iter().take(8) {
            eprint!("    {}", tag);
        }
        if tags.len() > 8 {
            eprint!("  ... +{} more", tags.len() - 8);
        }
        eprintln!();
    }

    // Top 20 most-referenced tags
    let mut sorted_tags: Vec<_> = tag_counts.iter().collect();
    sorted_tags.sort_by(|a, b| b.1.cmp(a.1));

    eprintln!("\nTop 20 most-referenced tags:");
    for (tag, count) in sorted_tags.iter().take(20) {
        eprintln!("  {:3}x  {}", count, tag);
    }

    // Sanity assertions
    assert!(
        total_files >= 50,
        "expected 50+ .rs files, found {}",
        total_files
    );
    assert!(
        tag_counts.len() >= 50 && tag_counts.len() <= 500,
        "expected 50-500 unique tags, found {}",
        tag_counts.len()
    );

    eprintln!("\n=== Section 0 PASSED ===\n");
}

// ================================================================
// Section 1: Full ingestion with all enrichments
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature and model download"]
async fn spike_01_full_ingestion_with_enrichments() {
    use plexus::adapter::{
        CoOccurrenceEnrichment, DiscoveryGapEnrichment, EmbeddingSimilarityEnrichment,
        FastEmbedEmbedder, FragmentAdapter, FragmentInput, IngestPipeline, TagConceptBridger,
    };
    use plexus::adapter::Enrichment;
    use plexus::{Context, ContextId, PlexusEngine, dimension};
    use std::sync::Arc;
    use walkdir::WalkDir;

    eprintln!("\n=== Spike 06 Section 1: Full Ingestion with All Enrichments ===\n");

    let engine = Arc::new(PlexusEngine::new());
    let ctx_id = ContextId::from("spike-06-self");
    engine
        .upsert_context(Context::with_id(ctx_id.clone(), "spike-06-self"))
        .unwrap();

    // Real embedder — threshold 0.55
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
        Arc::new(FragmentAdapter::new("spike-06-self")),
        vec![
            Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
            Arc::new(CoOccurrenceEnrichment::new()) as Arc<dyn Enrichment>,
            embedding_enrichment as Arc<dyn Enrichment>,
            Arc::new(DiscoveryGapEnrichment::new("similar_to", "discovery_gap")) as Arc<dyn Enrichment>,
        ],
    );

    // Walk src/ and ingest each .rs file
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut ingested = 0;

    for entry in WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(path);
        let content = std::fs::read_to_string(path).unwrap();

        let tags = extract_tags_from_rust(&content, &rel_path.to_string_lossy());
        if tags.is_empty() {
            continue;
        }

        // Fragment text: first 500 chars (doc comments come first, give embedder purpose context)
        let text = if content.len() > 500 {
            &content[..500]
        } else {
            &content
        };

        let input = FragmentInput::new(text, tags)
            .with_source(&rel_path.to_string_lossy().to_string());

        pipeline
            .ingest("spike-06-self", "fragment", Box::new(input))
            .await
            .unwrap();
        ingested += 1;

        if ingested % 10 == 0 {
            eprint!(".");
        }
    }
    eprintln!("\nIngested {} source files\n", ingested);

    // --- Comprehensive analysis ---
    let ctx = engine.get_context(&ctx_id).unwrap();

    let concepts: Vec<_> = ctx
        .nodes()
        .filter(|n| n.dimension == dimension::SEMANTIC)
        .collect();

    let similar_to: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "similar_to")
        .collect();
    let may_be_related: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "may_be_related")
        .collect();
    let discovery_gap: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "discovery_gap")
        .collect();
    let references: Vec<_> = ctx
        .edges()
        .filter(|e| e.relationship == "references")
        .collect();

    eprintln!("=== Graph Summary ===");
    eprintln!("  Concepts:              {}", concepts.len());
    eprintln!("  similar_to edges:      {}", similar_to.len());
    eprintln!("  may_be_related edges:  {}", may_be_related.len());
    eprintln!("  discovery_gap edges:   {}", discovery_gap.len());
    eprintln!("  references edges:      {}", references.len());

    // Top 20 co-occurring pairs (may_be_related)
    let mut cooccur_pairs: Vec<_> = may_be_related
        .iter()
        .filter(|e| e.source.as_str() < e.target.as_str())
        .collect();
    cooccur_pairs.sort_by(|a, b| b.raw_weight.partial_cmp(&a.raw_weight).unwrap());

    eprintln!("\n=== Top 20 Co-occurring Pairs (may_be_related) ===");
    for (i, e) in cooccur_pairs.iter().take(20).enumerate() {
        let src = e.source.as_str().strip_prefix("concept:").unwrap_or(e.source.as_str());
        let tgt = e.target.as_str().strip_prefix("concept:").unwrap_or(e.target.as_str());
        eprintln!("  {:2}. {} ↔ {} ({:.4})", i + 1, src, tgt, e.raw_weight);
    }

    // Top 20 embedding-similar pairs (similar_to)
    let mut similar_pairs: Vec<_> = similar_to
        .iter()
        .filter(|e| e.source.as_str() < e.target.as_str())
        .collect();
    similar_pairs.sort_by(|a, b| b.raw_weight.partial_cmp(&a.raw_weight).unwrap());

    eprintln!("\n=== Top 20 Embedding-Similar Pairs (similar_to) ===");
    for (i, e) in similar_pairs.iter().take(20).enumerate() {
        let src = e.source.as_str().strip_prefix("concept:").unwrap_or(e.source.as_str());
        let tgt = e.target.as_str().strip_prefix("concept:").unwrap_or(e.target.as_str());
        eprintln!("  {:2}. {} ↔ {} ({:.4})", i + 1, src, tgt, e.raw_weight);
    }

    // Top 20 discovery gaps
    let mut gap_pairs: Vec<_> = discovery_gap
        .iter()
        .filter(|e| e.source.as_str() < e.target.as_str())
        .collect();
    gap_pairs.sort_by(|a, b| b.raw_weight.partial_cmp(&a.raw_weight).unwrap());

    eprintln!("\n=== Top 20 Discovery Gaps ===");
    for (i, e) in gap_pairs.iter().take(20).enumerate() {
        let src = e.source.as_str().strip_prefix("concept:").unwrap_or(e.source.as_str());
        let tgt = e.target.as_str().strip_prefix("concept:").unwrap_or(e.target.as_str());
        eprintln!("  {:2}. {} ↔ {} ({:.4})", i + 1, src, tgt, e.raw_weight);
    }

    // Top 15 semantic hubs (most connected concepts via any relationship)
    let mut hub_connections: HashMap<String, usize> = HashMap::new();
    for e in ctx.edges().filter(|e| {
        e.relationship == "similar_to"
            || e.relationship == "may_be_related"
            || e.relationship == "discovery_gap"
    }) {
        *hub_connections.entry(e.source.to_string()).or_insert(0) += 1;
    }
    let mut hubs: Vec<_> = hub_connections.into_iter().collect();
    hubs.sort_by(|a, b| b.1.cmp(&a.1));

    eprintln!("\n=== Top 15 Semantic Hubs ===");
    for (id, count) in hubs.iter().take(15) {
        let label = id.strip_prefix("concept:").unwrap_or(id);
        eprintln!("  {} — {} connections", label, count);
    }

    // Selectivity stats
    let total_concept_pairs = if concepts.len() > 1 {
        concepts.len() * (concepts.len() - 1) / 2
    } else {
        1
    };
    let similar_pair_count = similar_pairs.len();
    let gap_pair_count = gap_pairs.len();
    let selectivity = similar_pair_count as f64 / total_concept_pairs as f64 * 100.0;
    let gap_selectivity = gap_pair_count as f64 / total_concept_pairs as f64 * 100.0;

    eprintln!("\n=== Selectivity Stats ===");
    eprintln!("  Files ingested:         {}", ingested);
    eprintln!("  Concepts:               {}", concepts.len());
    eprintln!("  Possible concept pairs: {}", total_concept_pairs);
    eprintln!("  Similar pairs (>=0.55): {} ({:.1}%)", similar_pair_count, selectivity);
    eprintln!("  Discovery gaps:         {} ({:.1}%)", gap_pair_count, gap_selectivity);
    eprintln!("  Co-occurrence pairs:    {}", cooccur_pairs.len());

    // Assertions
    assert!(
        concepts.len() >= 50,
        "expected 50+ concepts, got {}",
        concepts.len()
    );
    assert!(
        !similar_to.is_empty(),
        "embedding enrichment should produce similar_to edges"
    );

    eprintln!("\n=== Section 1 PASSED ===\n");
}

// ================================================================
// Section 2: Persistent graph for MCP querying
// ================================================================

#[cfg(feature = "embeddings")]
#[tokio::test]
#[ignore = "requires embeddings feature and model download"]
async fn spike_02_persistent_graph_for_mcp() {
    use plexus::adapter::{
        CoOccurrenceEnrichment, DiscoveryGapEnrichment, EmbeddingSimilarityEnrichment,
        FastEmbedEmbedder, FragmentAdapter, FragmentInput, IngestPipeline, TagConceptBridger,
    };
    use plexus::adapter::Enrichment;
    use plexus::storage::{OpenStore, SqliteStore, SqliteVecStore, DEFAULT_EMBEDDING_DIMENSIONS};
    use plexus::{Context, ContextId, GraphStore, PlexusEngine, dimension};
    use std::sync::Arc;
    use walkdir::WalkDir;

    eprintln!("\n=== Spike 06 Section 2: Persistent Graph for MCP Querying ===\n");

    // Persistent storage paths under target/
    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let db_path = target_dir.join("plexus-self.db");
    let vec_db_path = target_dir.join("plexus-self-vec.db");

    // Clean up previous run if exists
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&vec_db_path);

    let store: Arc<dyn GraphStore> = Arc::new(
        SqliteStore::open(&db_path).expect("failed to open SqliteStore"),
    );
    let engine = Arc::new(PlexusEngine::with_store(store));

    let ctx_id = ContextId::from("plexus-self");
    engine
        .upsert_context(Context::with_id(ctx_id.clone(), "plexus-self"))
        .unwrap();

    // Embedder + persistent vector store
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
        Arc::new(FragmentAdapter::new("spike-06-self")),
        vec![
            Arc::new(TagConceptBridger::new()) as Arc<dyn Enrichment>,
            Arc::new(CoOccurrenceEnrichment::new()) as Arc<dyn Enrichment>,
            embedding_enrichment as Arc<dyn Enrichment>,
            Arc::new(DiscoveryGapEnrichment::new("similar_to", "discovery_gap")) as Arc<dyn Enrichment>,
        ],
    );

    // Walk src/ and ingest
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut ingested = 0;

    for entry in WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let path = entry.path();
        let rel_path = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(path);
        let content = std::fs::read_to_string(path).unwrap();

        let tags = extract_tags_from_rust(&content, &rel_path.to_string_lossy());
        if tags.is_empty() {
            continue;
        }

        let text = if content.len() > 500 {
            &content[..500]
        } else {
            &content
        };

        let input = FragmentInput::new(text, tags)
            .with_source(&rel_path.to_string_lossy().to_string());

        pipeline
            .ingest("plexus-self", "fragment", Box::new(input))
            .await
            .unwrap();
        ingested += 1;

        if ingested % 10 == 0 {
            eprint!(".");
        }
    }
    eprintln!("\nIngested {} source files\n", ingested);

    // Print summary
    let ctx = engine.get_context(&ctx_id).unwrap();
    let concept_count = ctx
        .nodes()
        .filter(|n| n.dimension == dimension::SEMANTIC)
        .count();
    let similar_count = ctx.edges().filter(|e| e.relationship == "similar_to").count();
    let gap_count = ctx.edges().filter(|e| e.relationship == "discovery_gap").count();

    eprintln!("=== Persisted Graph Summary ===");
    eprintln!("  Concepts:         {}", concept_count);
    eprintln!("  similar_to edges: {}", similar_count);
    eprintln!("  discovery_gaps:   {}", gap_count);

    // Verify: reload from disk produces same concept count
    let store2: Arc<dyn GraphStore> = Arc::new(
        SqliteStore::open(&db_path).expect("failed to reopen SqliteStore"),
    );
    let engine2 = PlexusEngine::with_store(store2);
    engine2.load_all().expect("failed to load_all from disk");

    let ctx2 = engine2
        .get_context(&ctx_id)
        .expect("context should exist after reload");
    let reloaded_concept_count = ctx2
        .nodes()
        .filter(|n| n.dimension == dimension::SEMANTIC)
        .count();

    eprintln!("\n=== Persistence Verification ===");
    eprintln!("  Original concept count: {}", concept_count);
    eprintln!("  Reloaded concept count: {}", reloaded_concept_count);

    assert_eq!(
        concept_count, reloaded_concept_count,
        "reloaded graph should have same concept count"
    );

    eprintln!("\n=== MCP Instructions ===");
    eprintln!("  Graph persisted to: {}", db_path.display());
    eprintln!("  Vector store:       {}", vec_db_path.display());
    eprintln!("  Query via: plexus mcp --db {}", db_path.display());

    eprintln!("\n=== Section 2 PASSED ===\n");
}
