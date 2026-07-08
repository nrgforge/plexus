# play-harness

Programmatic consumer-surface validation for Plexus. Each scenario
spawns real `plexus mcp --transport stdio` subprocesses (one per
consumer role) against a fresh temp SQLite, drives them over JSON-RPC,
and asserts twice: through MCP reads and directly against the SQLite
file. Disk is ground truth; MCP is the surface under test.

```bash
python3 tools/play-harness/play.py <scenario> [--binary PATH] [--keep-db]
python3 tools/play-harness/play.py all
```

`--binary` defaults to the installed `plexus` (release fidelity);
use `./target/release/plexus` to test unreleased changes.

## Design rules

- **Assert the claims artifacts make; observe everything else.** Exact
  counts where the design predicts them (n(n-1) temporal pairs), zero
  where the claim is absence, loose bounds where LLM output varies.
  Observations regularly catch what assertions weren't designed for.
- **One process per consumer role**, fresh DB per scenario — process
  topology is part of what's under test, never a confound.
- **LLM-dependent scenarios preflight** (Ollama up, model pulled,
  llm-orc on PATH) and skip loudly rather than fail.

## Scenarios

| Scenario | Validates | Needs |
|---|---|---|
| `crawl` | Lean-baseline truthfulness (ADR-038/039): temporal fires on untagged fragments, similar_to/may_be_related/discovery_gap provably absent, CoOccurrence on tagged content | nothing |
| `walk` | Tautology threshold (ADR-038): worked-example spec over both fixture corpora → similar_to emerges, zero cross-corpus at 0.72 | Ollama + nomic-embed-text |
| `run` | Lens registers are topology-invariant (ADR-041); `min_corroboration: 2` cuts saturation to evidenced pairs (issue #4) | Ollama + nomic-embed-text |
| `stale` | Multi-process read coherence (ADR-017 §2, issue #1). Fails against binaries ≤ v0.3.0 by design | nothing |
| `matrix` | M0 consistency grid: process topologies × read surfaces, plus write-write interleaving safety | nothing |
| `flywheel` | Cross-consumer signal via concept-identity convergence + solo-vs-shared differential KPI (M1) | Ollama + mistral |
| `latent` | Cross-consumer embedding bridges via the re-embed sweep (issue #9); cross-process lens reactivity (issue #10) | Ollama + nomic-embed-text |
| `extract-fg` | Minimum-useful chain: LLM tags → CoOccurrence → lens, no human tags | Ollama + mistral |
| `extract-bg` | Deep extraction (SpaCy + 8 LLM agents), Invariant 45 reinforcement, lenses fire on background emissions (issue #5) | Ollama + mistral, spacy + en_core_web_trf |

The `matrix` and `stale` results are cited as evidence in
`docs/system-design.md` Amendments 8–9; scenario docstrings in
`play.py` carry the issue/ADR provenance for each assertion.
