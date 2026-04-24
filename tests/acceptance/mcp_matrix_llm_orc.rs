//! Gated MCP matrix tests — exercise real llm-orc end-to-end.
//!
//! Skipped by default. Set `PLEXUS_INTEGRATION=1` to run. Requires:
//! - Ollama running locally
//! - `llm-orc` on PATH
//! - `mistral:7b` model available to Ollama (T6–T11)
//! - `analyst-mistral` profile in `.llm-orc/profiles/` (T6–T11)
//! - `nomic-embed-text` model available to Ollama (T12)
//! - `embedding-similarity` ensemble in `.llm-orc/ensembles/` (T12)
//!
//! Coverage:
//! - T6: Built-in semantic extraction via MCP — `extract-file` input
//!   routes through ExtractionCoordinator → SemanticAdapter → live
//!   `extract-semantic` ensemble. Slow (multi-agent pipeline); fires
//!   a generous timeout.
//! - T7: Declarative adapter with `ensemble:` field — consumer spec
//!   invokes the minimal `test-theme-extractor` fixture ensemble;
//!   emit primitives use `{ensemble.X}` accessors. Proves the
//!   `load_spec` → client-propagation → DeclarativeAdapter path.
//! - T8: T7 + lens — declarative adapter invokes ensemble, emits
//!   edges, lens translates those edges into consumer vocabulary.
//!   Proves lens composition over llm-orc-driven emissions.
//! - T11: Background-phase + lens — registers a lens on a context, then
//!   ingests via `extract-file`. Verifies whether the lens fires on
//!   edges produced by semantic extraction (background phase), or only
//!   on edges from foreground adapter emissions + their enrichment loop.
//!   Architectural probe — the answer shapes how consumers can use lenses
//!   with llm-orc-backed extraction.
//! - T12: ADR-038 worked-example end-to-end — loads the
//!   `examples/specs/embedding-activation.yaml` spec, ingests a small
//!   multi-corpus batch, and verifies `similar_to` edges emerge.
//!   Requires `nomic-embed-text` pulled in Ollama (not `mistral:7b` —
//!   embeddings are a separate model family).
//!
//! LLM output varies, so assertions are property-based (a concept
//! appeared / an edge appeared / the lens translated at least once),
//! never on exact labels or counts.

use super::mcp_harness::{is_error, node_count, tool_result_json, McpHarness};
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;

fn integration_enabled() -> bool {
    std::env::var("PLEXUS_INTEGRATION")
        .map(|v| v == "1")
        .unwrap_or(false)
}

// Generous timeouts: LLM calls are slow.
const SEMANTIC_POLL_INTERVAL: Duration = Duration::from_secs(2);
const SEMANTIC_POLL_TIMEOUT: Duration = Duration::from_secs(300); // 5 min
/// Per-call timeout when the handler synchronously invokes llm-orc
/// (declarative ensemble path). Generous because an ensemble call
/// hits a real LLM — minutes are plausible.
const LLM_CALL_TIMEOUT: Duration = Duration::from_secs(180);

// ── T6: Built-in extract-semantic via MCP (gated) ──────────────────────

/// Given the default MCP pipeline (wires SubprocessClient for extract-semantic),
/// when a markdown file is ingested via `extract-file`,
/// then semantic extraction invokes llm-orc and concept nodes appear in
/// the graph (beyond what registration alone produces from frontmatter).
#[tokio::test]
async fn t6_extract_file_through_mcp_runs_semantic_extraction() {
    if !integration_enabled() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t6.db");
    let file_path = tmp.path().join("t6.md");
    // No YAML frontmatter → registration produces 0 concepts; any concept
    // after extraction must come from llm-orc.
    std::fs::write(
        &file_path,
        "# Travel journal\n\nYesterday I walked along the river at dawn. \
         The water was cold and the sky was clear. Small birds sang in the reeds.\n",
    )
    .expect("write fixture");

    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t6"})).await));

    // Baseline: no concepts yet
    let resp = h.call_tool("find_nodes", json!({"node_type": "concept"})).await;
    let baseline = node_count(&resp);
    assert_eq!(baseline, 0, "baseline should have 0 concepts");

    // Ingest the markdown file — returns after registration; structural +
    // semantic run in background.
    let resp = h.call_tool("ingest", json!({
        "data": {"file_path": file_path.to_str().unwrap()},
        "input_kind": "extract-file",
    })).await;
    assert!(!is_error(&resp), "extract-file ingest failed: {}", resp);

    // Poll: wait for semantic extraction to produce at least one concept.
    let mut concepts = 0;
    let deadline = std::time::Instant::now() + SEMANTIC_POLL_TIMEOUT;
    while std::time::Instant::now() < deadline {
        let resp = h.call_tool("find_nodes", json!({"node_type": "concept"})).await;
        concepts = node_count(&resp);
        if concepts > 0 {
            break;
        }
        tokio::time::sleep(SEMANTIC_POLL_INTERVAL).await;
    }

    assert!(
        concepts > 0,
        "semantic extraction should produce ≥1 concept within {:?} — \
         got {} (llm-orc may not be running, or the ensemble failed)",
        SEMANTIC_POLL_TIMEOUT, concepts
    );

    h.shutdown().await;
}

// ── T7: Declarative adapter with ensemble: through MCP (gated) ─────────

/// Given a declarative spec with `ensemble: test-theme-extractor`,
/// when ingest runs through the spec's input_kind,
/// then the ensemble is invoked via llm-orc and the spec's emit
/// primitives produce nodes whose IDs reference the ensemble response.
#[tokio::test]
async fn t7_declarative_adapter_with_ensemble() {
    if !integration_enabled() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t7.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t7"})).await));

    let spec_yaml = r#"
adapter_id: t7-theme-extractor
input_kind: t7.text
ensemble: test-theme-extractor
emit:
  - create_node:
      id: "theme:{ensemble.theme}"
      type: theme
      dimension: semantic
  - create_node:
      id: "keyword:{ensemble.keyword}"
      type: keyword
      dimension: semantic
"#;
    assert!(!is_error(
        &h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await
    ));

    let resp = h.call_tool_with_timeout(
        "ingest",
        json!({
            "data": {"text": "The morning fog settled over the village as the fishermen prepared their nets."},
            "input_kind": "t7.text",
        }),
        LLM_CALL_TIMEOUT,
    ).await;
    assert!(!is_error(&resp), "t7 ingest failed: {}", resp);
    // DeclarativeAdapter::transform_events emits `{type}_created` events
    // per created node — visible in the MCP ingest response.
    let event_count = tool_result_json(&resp)["events"].as_u64().unwrap_or(0);
    assert!(
        event_count >= 2,
        "ensemble-driven ingest should produce ≥2 outbound events (theme + keyword), got {}",
        event_count
    );

    // At least one theme and one keyword should have been emitted.
    let theme_resp = h.call_tool("find_nodes", json!({"node_type": "theme"})).await;
    assert!(
        node_count(&theme_resp) >= 1,
        "declarative adapter should have emitted ≥1 theme node from ensemble response"
    );

    let keyword_resp = h.call_tool("find_nodes", json!({"node_type": "keyword"})).await;
    assert!(
        node_count(&keyword_resp) >= 1,
        "declarative adapter should have emitted ≥1 keyword node from ensemble response"
    );

    h.shutdown().await;
}

// ── T8: llm-orc + lens together (gated) ────────────────────────────────

/// Given a declarative spec with BOTH `ensemble:` and `lens:` fields,
/// when ingest runs through the spec's input_kind,
/// then the ensemble is invoked, emit primitives produce edges, and
/// the lens translates those edges into consumer vocabulary.
/// Proves lens composes with llm-orc-backed emissions.
#[tokio::test]
async fn t8_lens_over_llm_orc_declarative() {
    if !integration_enabled() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t8.db");
    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t8"})).await));

    let spec_yaml = r#"
adapter_id: t8-theme-extractor
input_kind: t8.text
ensemble: test-theme-extractor
lens:
  consumer: t8-consumer
  translations:
    - from: [related_to]
      to: conceptual_link
emit:
  - create_node:
      id: "theme:{ensemble.theme}"
      type: theme
      dimension: semantic
  - create_node:
      id: "keyword:{ensemble.keyword}"
      type: keyword
      dimension: semantic
  - create_edge:
      source: "theme:{ensemble.theme}"
      target: "keyword:{ensemble.keyword}"
      relationship: related_to
"#;
    assert!(!is_error(
        &h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await
    ));

    let resp = h.call_tool_with_timeout(
        "ingest",
        json!({
            "data": {"text": "Cold mountain streams flowed past the ancient stone bridge."},
            "input_kind": "t8.text",
        }),
        LLM_CALL_TIMEOUT,
    ).await;
    assert!(!is_error(&resp), "t8 ingest failed: {}", resp);

    // Lens should have translated the related_to edge emitted from the
    // ensemble response into lens:t8-consumer:conceptual_link.
    let lens_resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:t8-consumer:"}),
    ).await;
    let lens_nodes = node_count(&lens_resp);
    assert!(
        lens_nodes >= 2,
        "lens should translate ≥1 related_to edge (2 incident nodes) — got {}",
        lens_nodes
    );

    h.shutdown().await;
}

// ── T11: Background-phase + lens interaction (gated) ───────────────────

/// PINS CURRENT BEHAVIOR: lenses do NOT fire on background-phase
/// emissions (semantic extraction produces concepts + relationships
/// but no registered lens translates them).
///
/// Architectural cause: `IngestPipeline::ingest` runs the enrichment
/// loop once, after the foreground adapter's `process()` returns, on
/// events drained from the foreground sink. Background tasks spawn via
/// `tokio::spawn` AFTER ingest returns and write through their own
/// `EngineSink::for_engine` — those emissions do not re-enter the
/// pipeline's enrichment loop.
///
/// Consumer impact: llm-orc-driven extraction output (concept nodes,
/// related_to/similar_to/is_a/part_of edges) is NOT translated by
/// registered lenses. A consumer wanting lens coverage over
/// LLM-extracted structure must either (a) use a declarative adapter
/// that invokes llm-orc via `ensemble:` (foreground path — covered
/// by T8) or (b) wait for a future architectural fix that wires
/// background emissions into the enrichment loop.
///
/// This test runs with `PLEXUS_INTEGRATION=1` + real Ollama (~55s).
/// It is a contract-pinning test: the assertion reflects current
/// behavior. When the architectural gap is closed, this test will
/// start failing — flip the assertion and update cycle-status.
#[tokio::test]
async fn t11_lens_does_not_fire_on_background_phase_emissions() {
    if !integration_enabled() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t11.db");
    let file_path = tmp.path().join("t11.md");
    std::fs::write(
        &file_path,
        "# Travel journal\n\nYesterday I walked along the river at dawn. \
         The water was cold and the sky was clear. Small birds sang in the reeds.\n",
    )
    .expect("write fixture");

    let mut h = McpHarness::spawn(&db).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t11"})).await));

    // Spec declares a lens translating any of several relationships that
    // semantic extraction might produce. Minimal emit (never routed to;
    // present only to satisfy validation).
    let spec_yaml = r#"
adapter_id: t11-lens-probe
input_kind: t11.never-routed
lens:
  consumer: t11
  translations:
    - from: [related_to, similar_to, is_a, part_of]
      to: semantic_connection
emit:
  - create_node:
      id: "noop:{input.id}"
      type: noop
      dimension: semantic
"#;
    assert!(!is_error(
        &h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await
    ));

    // Ingest via extract-file — registration synchronous, structural +
    // semantic extraction run in background.
    let resp = h.call_tool("ingest", json!({
        "data": {"file_path": file_path.to_str().unwrap()},
        "input_kind": "extract-file",
    })).await;
    assert!(!is_error(&resp), "extract-file ingest failed: {}", resp);

    // Poll until semantic extraction produces concepts (proof the
    // background phase completed).
    let mut concepts = 0;
    let deadline = std::time::Instant::now() + SEMANTIC_POLL_TIMEOUT;
    while std::time::Instant::now() < deadline {
        let cresp = h.call_tool("find_nodes", json!({"node_type": "concept"})).await;
        concepts = node_count(&cresp);
        if concepts > 0 {
            break;
        }
        tokio::time::sleep(SEMANTIC_POLL_INTERVAL).await;
    }
    assert!(
        concepts > 0,
        "semantic extraction should produce concepts (prerequisite for this test)"
    );

    // The observation: does the lens have any edges over
    // semantic-extraction-produced relationships?
    let lens_resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "lens:t11:"}),
    ).await;
    let lens_count = node_count(&lens_resp);

    assert_eq!(
        lens_count, 0,
        "CURRENT BEHAVIOR PINNED: lens should NOT fire on background-phase \
         emissions — concepts from semantic extraction: {}, lens coverage: {}. \
         If this assertion fails, either (a) the architectural gap has been \
         closed (great — flip to `lens_count > 0` and update cycle-status) \
         or (b) an unexpected path now triggers the lens over background \
         emissions (investigate).",
        concepts, lens_count
    );

    h.shutdown().await;
}

// ── T12: ADR-038 worked-example end-to-end (gated) ─────────────────────

/// Given the worked-example spec at examples/specs/embedding-activation.yaml,
/// when a small multi-corpus batch is ingested through the spec's input_kind,
/// then the llm-orc ensemble computes embeddings via Ollama's nomic-embed-text,
/// the script returns cosine-similarity pairs above threshold, and the spec's
/// emit primitives create `fragment` nodes plus `similar_to` edges between
/// semantically related pairs.
///
/// This pins the worked example referenced from ADR-038's Consequences
/// Negative (quality bar: "`similar_to` edges emerging over content the
/// author did not pre-encode with overlapping tags"). The assertion is
/// property-based — the test verifies the mechanism fires and produces
/// the expected structure, not a specific similarity value (embedding
/// output varies slightly across model versions).
#[tokio::test]
async fn t12_embedding_activation_worked_example() {
    if !integration_enabled() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("t12.db");
    // Short fixture texts produce lower absolute similarity values than
    // longer fixtures (~50-word inline texts vs ~200-word abstracts);
    // lower the script's threshold so the within-corpus pattern crosses.
    // Production consumers use longer prose and the default 0.72 threshold.
    let mut h = McpHarness::spawn_with_env(&db, &[("SIMILARITY_MIN", "0.6")]).await;
    h.initialize().await;
    assert!(!is_error(&h.call_tool("set_context", json!({"name": "t12"})).await));

    // Load the spec from disk — this is the same file shipped as the
    // worked example. If its grammar regresses, this test catches it.
    let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/specs/embedding-activation.yaml");
    let spec_yaml = std::fs::read_to_string(&spec_path)
        .expect("worked-example spec must exist at examples/specs/embedding-activation.yaml");
    assert!(
        !is_error(&h.call_tool("load_spec", json!({"spec_yaml": spec_yaml})).await),
        "load_spec on the worked example failed — spec grammar or validation regressed"
    );

    // Minimal multi-corpus batch: two topically-related abstracts from
    // the collective-intelligence corpus and two narrative passages.
    // Four docs = 6 pairs; enough for the pattern to appear, fast enough
    // to not stall CI when the flag is on.
    let batch = json!({
        "docs": [
            {
                "id": "doc:ants",
                "text": "Ant colonies allocate tasks through a decentralized process. \
                         A minority of workers remain idle at any moment; this reserve \
                         is drawn on when foragers encounter unusual loads. Idleness \
                         therefore functions as responsive capacity, not inefficiency.",
            },
            {
                "id": "doc:bees",
                "text": "Honeybee swarms choose a new nest site through quorum sensing. \
                         Scouts advertise candidate sites by waggle dance; recruitment \
                         builds until one site crosses a threshold of committed scouts. \
                         The colony then commits collectively and moves.",
            },
            {
                "id": "doc:story-alpha",
                "text": "The old clock struck midnight in the narrow apartment. \
                         She stood at the window, fingers curled around a teacup, \
                         wondering whether tomorrow would bring the letter she had \
                         been waiting for through the long autumn.",
            },
            {
                "id": "doc:story-beta",
                "text": "He descended the creaking staircase at dawn, listening for \
                         any movement from the rooms above. The house had been silent \
                         for three days. He carried only a small bag and the letter \
                         his mother had left for him.",
            },
        ],
    });

    let resp = h.call_tool_with_timeout(
        "ingest",
        json!({
            "data": batch,
            "input_kind": "embedding-activation.batch",
        }),
        LLM_CALL_TIMEOUT,
    ).await;
    assert!(!is_error(&resp), "embedding-activation ingest failed: {}", resp);

    // Four fragment nodes must have been created.
    let frag_resp = h.call_tool("find_nodes", json!({"node_type": "fragment"})).await;
    assert!(
        node_count(&frag_resp) >= 4,
        "expected ≥4 fragment nodes (one per input doc), got {}",
        node_count(&frag_resp)
    );

    // At least one `similar_to` edge must have been emitted. Use
    // relationship_prefix filter to count nodes incident to any
    // similar_to edge.
    let sim_resp = h.call_tool(
        "find_nodes",
        json!({"relationship_prefix": "similar_to"}),
    ).await;
    let incident_nodes = node_count(&sim_resp);
    assert!(
        incident_nodes >= 2,
        "expected ≥1 similar_to edge (≥2 incident nodes) — got {}. \
         If 0, the nomic-embed-text model may not be pulled, or the ensemble \
         is not returning pairs above SIMILARITY_MIN for these short fixtures.",
        incident_nodes
    );

    h.shutdown().await;
}

