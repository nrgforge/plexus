# Validation Strategy

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 10. Validation Strategy

> **Note**: This section provides development validation. For rigorous experimental validation, see **[08-research-methodology.md](./08-research-methodology.md)**.

### 10.1 Test Corpora

We maintain a tiered test corpus system (see `plexus-test-corpora` repo):

| Tier | Corpus | Size | Purpose | Phase |
|------|--------|------|---------|-------|
| **Dev** | `manza-specs` | ~20 files | Fast iteration, known ground truth | All |
| **Validation** | `pkm-webdev` | 51 files | Wikilink extraction, small scale | 1-5 |
| **Validation** | `pkm-datascience` | 516 files | Dense links, propagation testing | 3-4 |
| **Scale** | `arch-wiki` | 2,487 files | O(n log n) verification | 1, 9 |
| **Semantic** | `shakespeare` | 42 files | No links, semantic-only | 2, 6 |

### 10.2 Phase-Specific Validation

Each phase has a **development checkpoint** (fast, manual review) and links to **research experiments** (rigorous, reproducible).

| Phase | Dev Checkpoint | Research Link | Pass Criteria |
|-------|----------------|---------------|---------------|
| 1. Bootstrap | Sections parsed, dirs mapped | Exp1, Exp2 | ≥90% section detection accuracy |
| 1b. Sampling | Seed SECTIONS include key content | Exp1, Exp2 | ≥70% key section overlap |
| 2. Extraction | Concepts are section-relevant | Exp4, Exp5 | ≥80% relevance (manual) |
| 2b. Chunking | Large sections chunked correctly | - | Chunks < 2000 words |
| 3. Propagation | Labels spread via edge types | Exp3 | ≥60% precision vs LLM |
| 4. Dual Hierarchy | Categories + structure zoom | - | Both zoom dimensions work |
| 5. UI | Dual zoom controls functional | - | Interactive demo works |
| 6. Signals | Multi-signal edges exist | - | ≥3 signal types combined |
| 7. Tests | Test results parsed | - | Vitest + cargo test parse |
| 8. Gaps | Gaps identified | - | Top-5 gaps sensible |
| 9. Incremental | Section updates < 100ms | - | Latency benchmarks pass |
| 10. Narrator | Insights generated | - | ≥3 insight types shown |

### 10.3 Development Validation Commands

```bash
# Run dev validation suite (fast, uses manza-specs)
cargo test -p plexus --features dev-validation

# Run full validation (uses all corpora)
cargo test -p plexus --features full-validation

# Run specific experiment
cargo run -p plexus --bin experiment -- --config experiments/config/exp1_sampling.yaml

# Generate ground truth for corpus
cargo run -p plexus --bin ground-truth -- --corpus pkm-datascience --output experiments/ground-truth/
```

### 10.4 Experiment Runner Infrastructure

```
crates/plexus/src/experiments/
├── mod.rs              # Experiment trait, runner
├── config.rs           # YAML config parsing
├── metrics.rs          # Metrics collection, JSON export
├── sampling.rs         # Exp1, Exp2 implementations
├── extraction.rs       # Exp4, Exp5 implementations
├── propagation.rs      # Exp3 implementation
└── ground_truth.rs     # Ground truth generation

experiments/
├── config/             # Experiment YAML configs
├── ground-truth/       # Generated ground truth per corpus
├── results/            # Experiment run outputs (JSON)
├── analysis/           # Jupyter notebooks for analysis
└── figures/            # Generated figures for papers
```

**Experiment Trait**:
```rust
pub trait Experiment {
    type Config: DeserializeOwned;
    type Metrics: Serialize;

    fn name(&self) -> &'static str;
    fn run(&self, config: Self::Config) -> Result<Self::Metrics>;
}

pub struct ExperimentRunner {
    pub output_dir: PathBuf,
}

impl ExperimentRunner {
    pub fn run<E: Experiment>(&self, exp: E, config_path: &Path) -> Result<()> {
        let config: E::Config = load_yaml(config_path)?;
        let metrics = exp.run(config)?;

        let output = ExperimentOutput {
            experiment_id: format!("{}_{}", exp.name(), timestamp()),
            timestamp: Utc::now(),
            config: serde_json::to_value(&config)?,
            results: serde_json::to_value(&metrics)?,
        };

        save_json(&self.output_dir.join(format!("{}.json", output.experiment_id)), &output)?;
        Ok(())
    }
}
```

### 10.5 Ground Truth Generation

For each corpus, we generate ground truth using a strong model (claude-opus-4-5):

```bash
# Generate ground truth concepts for pkm-datascience
cargo run -p plexus --bin ground-truth -- \
  --corpus /path/to/pkm-datascience \
  --model claude-opus-4-5 \
  --output experiments/ground-truth/pkm-datascience.json
```

Ground truth includes:
- Full LLM extraction on every document (p=1.0)
- Manual annotation sample (50 docs, 2 annotators)
- Inter-annotator agreement metrics

### 10.6 CI Integration

```yaml
# .github/workflows/plexus-validation.yml
name: Plexus Validation
on:
  push:
    paths: ['crates/plexus/**']

jobs:
  dev-validation:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run dev validation
        run: cargo test -p plexus --features dev-validation

  # Full validation runs weekly or on release
  full-validation:
    if: github.event_name == 'schedule' || startsWith(github.ref, 'refs/tags/')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Checkout test corpora
        uses: actions/checkout@v4
        with:
          repository: mrilikecoding/plexus-test-corpora
          path: test-corpora
      - name: Run full validation
        run: cargo test -p plexus --features full-validation
```

---

## Next: [06-embeddings-network.md](./06-embeddings-network.md) — Embedding Storage & Network Science
