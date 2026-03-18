//! Provenance contract acceptance tests.
//!
//! Scenarios:
//! - Content ingest automatically creates a provenance chain (Invariant 7)
//! - Content ingest creates marks queryable by chain
//! - A specific chain is queryable by ID and returns both chain and marks
//! - Explicit provenance ingest creates chain and mark via ProvenanceAdapter
//! - Linking two marks creates a queryable links_to reference

use super::helpers::TestEnv;
use plexus::adapter::{FragmentInput, ProvenanceInput, normalize_chain_name};

#[tokio::test]
async fn content_ingest_creates_provenance_chain() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Knowledge graphs connect ideas across domains",
        vec!["knowledge-graph".into(), "connections".into()],
    );

    env.api
        .ingest(env.ctx_id(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    // ContentAdapter always produces a chain node per Invariant 7
    let chains = env.api
        .list_chains(&env.context_name, None)
        .expect("list_chains should succeed");

    assert!(
        !chains.is_empty(),
        "content ingest should create at least one provenance chain"
    );
}

#[tokio::test]
async fn content_ingest_creates_marks() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Rust ownership ensures memory safety without a garbage collector",
        vec!["rust".into(), "memory-safety".into()],
    );

    env.api
        .ingest(env.ctx_id(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let chains = env.api
        .list_chains(&env.context_name, None)
        .expect("list_chains should succeed");

    assert!(
        !chains.is_empty(),
        "at least one chain should exist after ingest"
    );

    // Verify each chain produced by ingest has at least one mark
    let chain_id = &chains[0].id;
    let (_, marks) = env.api
        .get_chain(&env.context_name, chain_id)
        .expect("get_chain should succeed");

    assert!(
        !marks.is_empty(),
        "chain '{}' should contain at least one mark",
        chain_id
    );

    // The mark should record the ingested text as its annotation
    let mark = &marks[0];
    assert_eq!(
        mark.annotation,
        "Rust ownership ensures memory safety without a garbage collector",
        "mark annotation should match the ingested text"
    );
}

#[tokio::test]
async fn provenance_chain_is_queryable() {
    let env = TestEnv::new();
    let input = FragmentInput::new(
        "Graph traversal explores relationships between nodes",
        vec!["graph".into(), "traversal".into()],
    );

    env.api
        .ingest(env.ctx_id(), "content", Box::new(input))
        .await
        .expect("ingest should succeed");

    let chains = env.api
        .list_chains(&env.context_name, None)
        .expect("list_chains should succeed");

    assert!(!chains.is_empty(), "at least one chain should be present");

    let chain_id = &chains[0].id;

    // get_chain returns both the chain view and its marks
    let (chain_view, marks) = env.api
        .get_chain(&env.context_name, chain_id)
        .expect("get_chain should succeed");

    assert_eq!(
        chain_view.id, *chain_id,
        "returned chain ID should match the requested ID"
    );
    assert!(
        !chain_view.name.is_empty(),
        "chain should have a non-empty name"
    );
    assert!(
        !marks.is_empty(),
        "chain should contain at least one mark"
    );

    // Each mark in the result should reference this chain
    for mark in &marks {
        assert_eq!(
            mark.chain_id, *chain_id,
            "mark.chain_id should match the chain"
        );
    }
}

#[tokio::test]
async fn explicit_provenance_ingest_creates_queryable_chain_and_mark() {
    let env = TestEnv::new();

    let chain_id = normalize_chain_name("field notes");

    // Create chain via ProvenanceAdapter
    env.api
        .ingest(
            env.ctx_id(),
            "provenance",
            Box::new(ProvenanceInput::CreateChain {
                chain_id: chain_id.clone(),
                name: "field notes".to_string(),
                description: Some("Research observations".to_string()),
            }),
        )
        .await
        .expect("create_chain ingest should succeed");

    // Add a mark to the chain
    let mark_id = "mark:provenance:acceptance-test-1".to_string();
    env.api
        .ingest(
            env.ctx_id(),
            "provenance",
            Box::new(ProvenanceInput::AddMark {
                mark_id: mark_id.clone(),
                chain_id: chain_id.clone(),
                file: "src/graph/mod.rs".to_string(),
                line: 42,
                annotation: "Entry point for graph mutations".to_string(),
                column: None,
                mark_type: Some("reference".to_string()),
                tags: Some(vec!["#graph".to_string(), "#entry-point".to_string()]),
            }),
        )
        .await
        .expect("add_mark ingest should succeed");

    // Chain is queryable
    let chains = env.api
        .list_chains(&env.context_name, None)
        .expect("list_chains should succeed");

    let found_chain = chains.iter().find(|c| c.id == chain_id);
    assert!(
        found_chain.is_some(),
        "explicitly created chain '{}' should appear in list_chains",
        chain_id
    );
    assert_eq!(found_chain.unwrap().name, "field notes");

    // Mark is queryable via get_chain
    let (_, marks) = env.api
        .get_chain(&env.context_name, &chain_id)
        .expect("get_chain should succeed");

    assert_eq!(marks.len(), 1, "chain should contain exactly one mark");
    assert_eq!(marks[0].id, mark_id);
    assert_eq!(marks[0].file, "src/graph/mod.rs");
    assert_eq!(marks[0].line, 42);
    assert_eq!(marks[0].annotation, "Entry point for graph mutations");
}

#[tokio::test]
async fn linked_marks_are_queryable_via_get_links() {
    let env = TestEnv::new();

    let chain_id = normalize_chain_name("link test chain");

    // Create chain
    env.api
        .ingest(
            env.ctx_id(),
            "provenance",
            Box::new(ProvenanceInput::CreateChain {
                chain_id: chain_id.clone(),
                name: "link test chain".to_string(),
                description: None,
            }),
        )
        .await
        .expect("create_chain should succeed");

    // Create source mark
    let source_mark_id = "mark:provenance:acceptance-link-src".to_string();
    env.api
        .ingest(
            env.ctx_id(),
            "provenance",
            Box::new(ProvenanceInput::AddMark {
                mark_id: source_mark_id.clone(),
                chain_id: chain_id.clone(),
                file: "src/adapter/mod.rs".to_string(),
                line: 10,
                annotation: "Adapter trait definition".to_string(),
                column: None,
                mark_type: None,
                tags: None,
            }),
        )
        .await
        .expect("add source mark should succeed");

    // Create target mark
    let target_mark_id = "mark:provenance:acceptance-link-tgt".to_string();
    env.api
        .ingest(
            env.ctx_id(),
            "provenance",
            Box::new(ProvenanceInput::AddMark {
                mark_id: target_mark_id.clone(),
                chain_id: chain_id.clone(),
                file: "src/adapter/adapters/content.rs".to_string(),
                line: 125,
                annotation: "ContentAdapter implements Adapter trait".to_string(),
                column: None,
                mark_type: None,
                tags: None,
            }),
        )
        .await
        .expect("add target mark should succeed");

    // Link source → target
    env.api
        .link_marks(&env.context_name, &source_mark_id, &target_mark_id)
        .await
        .expect("link_marks should succeed");

    // get_links returns (outbound, inbound)
    let (outbound, inbound) = env.api
        .get_links(&env.context_name, &source_mark_id)
        .expect("get_links on source should succeed");

    assert_eq!(
        outbound.len(),
        1,
        "source mark should have one outbound link"
    );
    assert_eq!(
        outbound[0].id, target_mark_id,
        "outbound link should point to the target mark"
    );
    assert!(
        inbound.is_empty(),
        "source mark should have no inbound links"
    );

    // Verify from the target side
    let (outbound_from_target, inbound_to_target) = env.api
        .get_links(&env.context_name, &target_mark_id)
        .expect("get_links on target should succeed");

    assert!(
        outbound_from_target.is_empty(),
        "target mark should have no outbound links"
    );
    assert_eq!(
        inbound_to_target.len(),
        1,
        "target mark should have one inbound link"
    );
    assert_eq!(
        inbound_to_target[0].id, source_mark_id,
        "inbound link should originate from the source mark"
    );
}
