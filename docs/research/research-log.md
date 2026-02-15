# Research Log: Essay 18 — Phased Extraction Architecture

## Research Questions

### Q1: Target graph shape — what does a fully-extracted file look like?
Given a file that passes through all four phases (file info, metadata, heuristic, semantic), what nodes exist in what dimensions, what edges connect them, and how do contributions from different phases compose? Where does reinforcement happen vs. complementary evidence accumulation?

### Q2: Declarative adapter primitives — what building blocks cover 80% of use cases?
What's the minimal set of declarative primitives (create_node, create_edge, for_each, id_template, etc.) that can express FragmentAdapter and most custom adapters without Rust code? Can the existing FragmentAdapter be fully expressed declaratively?

### Q3: Phase execution model — how do non-blocking phases schedule and report?
Phases 1-2 are blocking (caller gets results immediately). Phases 3-4 are background (progressive enrichment). What's the execution model? How does a background phase signal completion? How does the caller know the graph is "fully enriched"?

### Q4: Phase contribution interaction — how do heuristic and semantic evidence compose?
If Phase 3 (heuristic: word count, structural similarity) and Phase 4 (semantic: LLM-extracted themes) both propose edges between the same concepts, how do their contributions interact? Same adapter ID (merge) or different (accumulate)? What does scale normalization do with heuristic vs. semantic confidence?

### Q5: llm-orc integration architecture — use as-is, port, or hybrid?
llm-orc is Python with a mature DAG model, script agents, and MCP surface. Plexus is Rust. Options: (a) invoke llm-orc as external process/MCP service, (b) port DAG concepts to Rust, (c) hybrid. What are the tradeoffs in latency, deployment complexity, and capability? Note: designing specific ensembles and choosing models is a SEPARATE research cycle — this question is about the structural integration.

### Q6: What do other systems do for progressive/phased extraction?
Tika, Elasticsearch ingest pipelines, Apple Spotlight/mdimporter, Docparser, etc. What patterns exist for multi-phase file processing where later phases are more expensive?

### Q7: Adapter spec format — how does a declarative adapter relate to the Adapter trait?
Is the declarative spec interpreted by a generic "DeclarativeAdapter" that implements the Adapter trait? Or is it a separate concept? How does it compose with the existing pipeline?

### Q8: Test corpora adequacy — do we have the right test data?
The test-corpora submodule has 4 corpora (PKM webdev, PKM datascience, Arch Wiki, Shakespeare). Are these sufficient for testing phased extraction across all four phases? What's missing?

---

## Q1: Target graph shape after phased extraction

**Method:** Design walkthrough — manually trace what each phase produces for a concrete file from the test corpora.

(Research in progress)
