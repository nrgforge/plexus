//! Gated MCP matrix tests — exercise real llm-orc end-to-end.
//!
//! Skipped by default. Set `PLEXUS_INTEGRATION=1` to run. Requires:
//! - Ollama running locally
//! - `llm-orc` on PATH
//! - `mistral:7b` model available to Ollama
//! - `analyst-mistral` profile in `.llm-orc/profiles/`
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

