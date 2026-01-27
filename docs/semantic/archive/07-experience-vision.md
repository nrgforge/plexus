# Experience Vision: Flow-State Knowledge Discovery

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 13. Experience Vision: Flow-State Knowledge Discovery

### 13.1 The Goal

The ultimate aim is a **live, flow-state experience** where structure emerges as you work. The knowledge graph evolves in real-time, providing visual feedback that enhances rather than interrupts creative work.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         FLOW-STATE EXPERIENCE                                │
│                                                                             │
│  ┌─────────────────────────────────────┐  ┌──────────────────────────────┐ │
│  │          EDITOR PANE                │  │       NARRATOR PANE          │ │
│  │                                     │  │                              │ │
│  │  You're writing about React hooks   │  │  ┌────────────────────────┐  │ │
│  │  and state management...            │  │  │ 🔗 EMERGING STRUCTURE  │  │ │
│  │                                     │  │  │                        │  │ │
│  │  ███████████████████████           │  │  │ "React Hooks" now      │  │ │
│  │  ████████████████                  │  │  │ connects to 3 docs     │  │ │
│  │  ██████████████████████████        │  │  │                        │  │ │
│  │                                     │  │  │ Edge to "State Mgmt"   │  │ │
│  │                                     │  │  │ reinforced (3 signals) │  │ │
│  │                                     │  │  └────────────────────────┘  │ │
│  │                                     │  │                              │ │
│  │                                     │  │  ┌────────────────────────┐  │ │
│  │                                     │  │  │ 🕳️ STRUCTURAL GAP      │  │ │
│  │                                     │  │  │                        │  │ │
│  │                                     │  │  │ "Testing" cluster and  │  │ │
│  │                                     │  │  │ "Hooks" cluster seem   │  │ │
│  │                                     │  │  │ related but unlinked   │  │ │
│  │                                     │  │  │                        │  │ │
│  │                                     │  │  │ [Bridge them?]         │  │ │
│  │                                     │  │  └────────────────────────┘  │ │
│  │                                     │  │                              │ │
│  └─────────────────────────────────────┘  └──────────────────────────────┘ │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         LIVE GRAPH VIEW                              │   │
│  │    Structure evolves as you type. New edges pulse when created.     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 13.2 The Narrator Pane

A dedicated UI component that surfaces insights without demanding attention:

| Insight Type | Trigger | Example |
|--------------|---------|---------|
| **Emerging Structure** | New concept detected | "React Hooks" now connects to 3 documents |
| **Edge Reinforcement** | Multiple signals align | Edge to "State Management" strengthened (3 signals) |
| **Structural Gap** | Clusters should connect | "Testing" and "Hooks" seem related but unlinked |
| **Hub Formation** | Node gains centrality | "Architecture.md" becoming a hub (12 connections) |
| **Drift Detection** | Document evolving away | This doc is drifting from its original cluster |

The narrator is **ambient** — visible but not intrusive. Updates accumulate; user can expand for details or dismiss.

### 13.3 Multi-Signal Edge Reinforcement

Edges gain confidence from **multiple independent signals**. An edge validated by several dimensions is far more reliable than one with a single signal.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    MULTI-DIMENSIONAL EDGE REINFORCEMENT                      │
│                                                                             │
│   Signal Source          Example                         Weight             │
│   ─────────────          ───────                         ──────             │
│                                                                             │
│   1. STRUCTURAL          [[wikilink]] exists             0.9                │
│      (explicit link)     import statement found          0.8                │
│                                                                             │
│   2. SEMANTIC            LLM extracted relationship      0.7 × confidence   │
│      (LLM extraction)    Propagated label match          0.5 × decay        │
│                                                                             │
│   3. BEHAVIORAL          Co-navigation (Hebbian)         0.3 × frequency    │
│      (usage patterns)    Co-editing in same session      0.4                │
│                                                                             │
│   4. STATISTICAL         Co-occurrence in corpus         PMI score          │
│      (corpus analysis)   Similar embedding vectors       cosine × 0.6       │
│                                                                             │
│   5. CONTRACTUAL         Passing integration test        0.95               │
│      (test validation)   API contract verified           0.9                │
│                          Type system validates           0.85               │
│                                                                             │
│   ─────────────────────────────────────────────────────────────────────────│
│                                                                             │
│   COMBINED EDGE STRENGTH:                                                   │
│                                                                             │
│   strength(e) = Σ signal_weight(s) × signal_confidence(s)                  │
│                 s ∈ signals(e)                                              │
│                                                                             │
│   confidence(e) = min(1.0, |signals(e)| × 0.3)  // More signals = higher   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key insight**: An edge between `ComponentA` and `ComponentB` that has:
- An explicit import (structural)
- LLM-detected semantic relationship
- A passing integration test (contractual)

...is **far more confident** than an edge with only one signal. The test validation is particularly powerful because it represents *behavioral* proof that the components interact correctly.

### 13.4 Test-Validated Edges (Contractual Signal)

When tests pass, they validate contracts between components. This should reinforce edges:

```rust
pub struct ContractualSignal {
    /// The test that validates this relationship
    test_id: String,

    /// Components involved in the test
    components: Vec<NodeId>,

    /// Type of validation
    validation_type: ContractType,

    /// Last validation timestamp
    last_validated: Timestamp,

    /// Pass/fail history affects confidence
    pass_rate: f32,
}

pub enum ContractType {
    /// Unit test validates internal behavior
    UnitTest,

    /// Integration test validates component interaction
    IntegrationTest { components: (NodeId, NodeId) },

    /// Type system validates interface contract
    TypeCheck,

    /// API contract (OpenAPI, GraphQL schema)
    ApiContract,

    /// Property-based test validates invariants
    PropertyTest,
}
```

**On test run**:
1. Parse test results
2. Identify which components/documents each test exercises
3. Reinforce edges between components in passing tests
4. Weaken edges in failing tests (relationship is broken)

```rust
impl SemanticLayer {
    fn on_test_results(&mut self, results: TestResults) {
        for test in results.tests {
            let components = self.components_exercised_by(&test);

            for (a, b) in components.pairs() {
                let edge = self.get_or_create_edge(a, b);

                if test.passed {
                    edge.add_signal(Signal::Contractual {
                        test_id: test.id,
                        contract_type: test.contract_type(),
                        confidence: 0.95,
                    });
                } else {
                    edge.weaken_signal(Signal::Contractual { test_id: test.id });
                    // Narrator: "Test failure weakened edge between A and B"
                }
            }
        }
    }
}
```

### 13.5 Real-Time Incremental Updates

For flow-state experience, updates must be **imperceptible** in latency:

| Operation | Target Latency | Approach |
|-----------|---------------|----------|
| Keystroke → graph update | < 100ms | Debounce, diff-based |
| New concept detected | < 500ms | Background extraction |
| Edge reinforcement | Immediate | In-memory update |
| Gap detection | < 2s | Async, on pause |
| Community re-detection | < 5s | Triggered on structure change |

**Debounce Strategy**:
```
Keystroke → 300ms debounce → Diff extraction → Incremental update
                                    ↓
                            If structural change detected:
                                    ↓
                            Queue background analysis
                                    ↓
                            Update graph when complete
                                    ↓
                            Pulse new edges in visualization
```

**Incremental Scope** (from LightRAG research):

```rust
pub fn determine_update_scope(change: DocumentChange) -> UpdateScope {
    match change.change_type() {
        // Typo fix, minor wording
        ChangeType::Cosmetic => UpdateScope::None,

        // Content change, same structure
        ChangeType::ContentEdit => UpdateScope::Single {
            doc: change.doc_id,
            reextract: false,
        },

        // New sections, removed sections
        ChangeType::StructuralEdit => UpdateScope::Local {
            doc: change.doc_id,
            depth: 1,
            reextract: true,
        },

        // Major rewrite
        ChangeType::Rewrite => UpdateScope::Propagation {
            seed: change.doc_id,
        },

        // New document
        ChangeType::Created => UpdateScope::NewDocument {
            doc: change.doc_id,
            analyze_if_hub_potential: true,
        },

        // Document deleted
        ChangeType::Deleted => UpdateScope::Cleanup {
            doc: change.doc_id,
            cascade: true,
        },
    }
}
```

### 13.6 Connecting to the Hebbian Vision

From VISION.md: *"Neurons that fire together wire together."*

The multi-signal reinforcement extends this:

| Original Hebbian Signal | Extended Signals |
|------------------------|------------------|
| Navigate A → B | + Semantic similarity |
| Edit A and B together | + Test validates A-B |
| Cite A and B together | + LLM detects relationship |

The graph becomes a **living memory** that learns from:
- Your navigation patterns (behavioral)
- Your explicit links (structural)
- LLM understanding (semantic)
- Test results (contractual)
- Corpus statistics (statistical)

Each signal type provides independent evidence. When they align, confidence compounds.

---

## Next: [08-research-methodology.md](./08-research-methodology.md) — Research Methodology
