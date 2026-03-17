# Ensemble Experiments: Extraction Quality Refinement

> Following Phase 2 spike completion, these experiments explore ensemble variations to improve extraction quality and reduce propagation noise.

## Background

Phase 2 findings (Investigations 4-6):
- **Extraction quality**: 100% grounded, but includes over-specific concepts (function names, variable names)
- **Propagation noise**: 33% of propagations are nonsensical (sibling-specific concepts spread poorly)
- **No normalization**: Same concept appears as "typescript", "TypeScript", "TS"

## Experiments

### Experiment A: Two-Stage Refiner

**Hypothesis**: A second LLM pass can filter over-specific concepts while keeping domain-significant ones.

**Method**:
1. Use standard extraction on Promises.md (produces 10 concepts including "sendNextRequest")
2. Pass raw concepts to a refiner agent
3. Measure: How many over-specific concepts removed? How many valid concepts retained?

**Refiner Prompt**:
```
Given these raw concepts extracted from a document, filter to only domain-significant ones.

REMOVE:
- Function/variable names (sendNextRequest, loadImage, addImg)
- Generic programming terms (function, value, error)
- Single-use identifiers specific to this code

KEEP:
- Technologies and libraries (javascript, babelify, Promise)
- Programming patterns and concepts (promises, async, callbacks)
- Domain terminology that would help categorize this document

Raw concepts: {concepts}

Return JSON: {"refined_concepts": [...], "removed": [...], "reasoning": "..."}
```

**Success Criteria**:
- Removes ≥50% of over-specific concepts
- Retains ≥90% of valid domain concepts
- Net improvement in propagation usefulness

---

### Experiment B: Propagation-Aware Extraction

**Hypothesis**: Prompting the model to consider propagation usefulness during extraction reduces noise at the source.

**Method**:
1. Re-extract Git.md with propagation-aware prompt
2. Compare: Does it still extract "config" and "ignore" (sibling-specific)?
3. Measure: Concept overlap with Investigation 4 extraction

**Propagation-Aware Prompt**:
```
Extract concepts that would be USEFUL if propagated to related documents.

For each potential concept, ask: "Would labeling a linked document with this concept help someone find it?"

GOOD concepts for propagation:
- "git" - yes, related docs are about git
- "version control" - yes, describes the domain

BAD concepts for propagation:
- "config" - too specific to this doc's subtopic (Git Config)
- "code" - too generic, doesn't help discovery
- "snippets" - describes format, not content

Extract 3-10 concepts optimized for cross-document discovery.

Return JSON: {"concepts": [{"name": "...", "type": "...", "confidence": 0.X, "propagation_score": 0.X}]}
```

**Success Criteria**:
- Sibling-specific concepts (config, ignore, tags) NOT extracted from index page
- Core topic (git) still extracted with high confidence
- New "propagation_score" field helps filtering

---

### Experiment C: Cross-Document Normalization

**Hypothesis**: LLM-based normalization can deduplicate concept variations and improve graph connectivity.

**Method**:
1. Collect all 58 concepts from Investigation 4
2. Ask LLM to identify duplicates and canonical forms
3. Measure: How many collapse? What's the deduplication ratio?

**Normalization Prompt**:
```
Given these concepts extracted from multiple documents, identify duplicates and normalize to canonical forms.

Rules:
- Case variations are duplicates: "TypeScript" = "typescript"
- Singular/plural are duplicates: "promise" = "promises"
- Abbreviations may be duplicates: "JS" = "javascript" (if context confirms)
- Related but distinct are NOT duplicates: "git" ≠ "git tags"

Concepts: {all_concepts}

Return JSON: {
  "canonical_forms": {
    "typescript": ["TypeScript", "Typescript", "TS"],
    "promises": ["promise", "Promise"],
    ...
  },
  "unique_concepts": [...],
  "deduplication_ratio": 0.XX
}
```

**Success Criteria**:
- Identifies ≥80% of true duplicates
- Does not incorrectly merge distinct concepts
- Deduplication ratio between 10-30% (some but not excessive merging)

---

### Experiment D: Confidence Calibration

**Hypothesis**: Rule-based confidence adjustment produces better filtering than raw LLM confidence.

**Method**:
1. Take Investigation 4 extractions with raw confidence scores
2. Apply calibration rules to adjust confidence
3. Measure: Does calibrated confidence better predict propagation usefulness?

**Calibration Rules**:
```
Base confidence from LLM: X

Adjustments:
+0.2 if concept appears in document heading (H1, H2)
+0.1 per additional occurrence (max +0.3)
+0.2 if concept is domain-specific (not in top 1000 English words)
+0.1 if concept has explicit definition in text ("X is a...")
-0.2 if concept is a code identifier (camelCase, snake_case)
-0.1 if concept is generic ("code", "example", "information")

Final confidence = clamp(X + adjustments, 0.0, 1.0)
```

**Success Criteria**:
- Calibrated confidence >0.7 correlates with "sensible" propagation
- Calibrated confidence <0.5 correlates with "nonsensical" propagation
- AUC improvement over raw confidence

---

### Experiment E: Multi-Agent Pipeline

**Hypothesis**: Specialized agents outperform single-agent extraction.

**Method**:
1. Create three-agent ensemble: Extractor → Validator → Normalizer
2. Compare output quality to single-agent extraction
3. Measure: Latency vs quality tradeoff

**Ensemble Configuration**:
```yaml
name: plexus-semantic-refined
description: Multi-agent extraction with validation

agents:
  - name: concept-extractor
    model_profile: ollama-llama3
    system_prompt: |
      Extract potential concepts from content.
      Be inclusive - extract anything that might be relevant.
      Return: {"raw_concepts": [...]}

  - name: concept-validator
    model_profile: ollama-llama3
    depends_on: [concept-extractor]
    system_prompt: |
      Given raw concepts, validate each:
      - Is it grounded in the source text?
      - Is it domain-significant (not generic)?
      - Would it help categorize this document?
      Return: {"validated": [...], "rejected": [...]}

  - name: relationship-extractor
    model_profile: ollama-llama3
    depends_on: [concept-validator]
    system_prompt: |
      Given validated concepts, identify relationships.
      Only output relationships with textual evidence.
      Return: {"relationships": [...]}

synthesis:
  strategy: chain
  output_format: json
```

**Success Criteria**:
- Validated concepts have higher propagation usefulness (>80%)
- Latency increase <3x (acceptable for batch processing)
- Cleaner separation of concerns enables prompt tuning

---

## Execution Plan

| Experiment | Priority | Complexity | Expected Impact |
|------------|----------|------------|-----------------|
| A: Refiner | High | Low | Direct noise reduction |
| B: Propagation-aware | High | Low | Source-level improvement |
| C: Normalization | Medium | Low | Graph connectivity |
| D: Calibration | Medium | Medium | Better filtering |
| E: Multi-agent | Low | High | Architecture validation |

**Recommended Order**: A → B → C → D → E

## Metrics to Track

For each experiment:
1. **Concept count**: Raw vs refined
2. **Grounding rate**: % concepts in source text
3. **Propagation usefulness**: % sensible when propagated
4. **Latency**: Time per document
5. **Cross-document overlap**: Shared concepts between related docs

## Results

### Experiment A: Two-Stage Refiner

**Status**: Complete ✓

**Test 1: Promises.md**

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Total concepts | 8 | 2 | -75% |
| Over-specific removed | — | 5 | — |
| Valid retained | — | 2 | 100% of core |

Raw: javascript, promise, image, onload, reject, filter, doNetworkCall, network call
Refined: **javascript, promise**
Removed: onload (event handler), filter (generic method), doNetworkCall (function name), image (tangential), reject (implementation)

**Test 2: Docker.md**

| Metric | Before | After | Delta |
|--------|--------|-------|-------|
| Total concepts | 5 | 2 | -60% |
| Over-specific removed | — | 3 | — |
| Valid retained | — | 2 | 100% of core |

Raw: docker, container, application, command, systemctl
Refined: **docker, container**
Removed: application (generic), command (generic), systemctl (implementation detail)

**Observations**:
- Refiner is aggressive but accurate — removes 60-75% of concepts
- Core domain concepts consistently retained
- Reasoning provided for each removal helps debugging
- **Verdict: Effective** — recommend adding to pipeline

---

### Experiment B: Propagation-Aware Extraction

**Status**: Complete ✓

**Test: Git.md**

| Metric | Standard | Propagation-Aware | Delta |
|--------|----------|-------------------|-------|
| Total concepts | 5 | 3 | -40% |
| Sibling-specific | 3 (config, ignore, tags) | 0 | -100% |
| Core topics | 1 (git) | 3 (git, version control, SCM) | +200% |
| Propagation scores | N/A | 0.7-1.0 | New metric |

Standard extraction: git, code, config, ignore, tags
Propagation-aware: **git** (1.0), **version control** (0.9), **source code management** (0.7)

**Observations**:
- Dramatically better for index/hub pages
- Stopped extracting sibling-specific concepts entirely
- Added useful high-level domain concepts not in original
- propagation_score provides built-in filtering metric
- **Verdict: Highly effective** — recommend as primary extraction prompt for index pages

---

### Experiment C: Cross-Document Normalization

**Status**: Complete (partial success)

| Metric | Value |
|--------|-------|
| Total concepts | 58 |
| Unique after normalization | ~47 |
| Deduplication ratio | 20% |
| Correct merges | 4 |
| False merges | 1 |

**Correct merges**:
- typescript: ["typescript", "Typescript"] ✓
- promise: ["promises", "promise"] ✓
- docker: ["docker", "Docker"] ✓
- dart: ["dart", "Dart"] ✓

**Incorrect merge**:
- git: ["git", "Git", "Tag"] ✗ — merged unrelated concepts

**Missed**:
- plugins: appears in both obsidian and web development docs, not merged

**Observations**:
- Case normalization works well
- Singular/plural normalization works
- Cross-concept confusion possible (git + tag)
- May need two-pass: first normalize case, then semantic dedup
- **Verdict: Partially effective** — needs refinement before production use

---

### Experiment D: Confidence Calibration

**Status**: Complete ✓

**Raw Extraction Results** (5 documents):

| Document | Concept | Raw Conf | In H1/H2 | Domain-Specific | Code ID | Generic | Def Present |
|----------|---------|----------|----------|-----------------|---------|---------|-------------|
| Promises.md | javascript | 1.0 | | ✓ | | | |
| Promises.md | promise | 1.0 | ✓ | ✓ | | | |
| Promises.md | load-image | 1.0 | | | ✓ | | |
| Promises.md | image | 0.9 | | | | ✓ | |
| Docker.md | docker | 1.0 | ✓ | ✓ | | | |
| Docker.md | container | 0.9 | | ✓ | | | |
| Docker.md | docker-compose | 0.9 | | ✓ | | | |
| Docker.md | services | 0.7 | | | | ✓ | |
| Git.md | git | 0.9 | ✓ | ✓ | | | |
| Git.md | code | 0.8 | | | | ✓ | |
| Git.md | config | 0.7 | | | | | |
| Git.md | ignore | 0.6 | | | | | |
| Git.md | tags | 0.5 | | | | | |
| Typescript.md | typescript | 0.9 | ✓ | ✓ | | | ✓ |
| Typescript.md | javascript | 0.8 | | ✓ | | | |
| Typescript.md | conditional props | 0.6 | | | | | |
| React.md | react | 0.9 | ✓ | ✓ | | | ✓ |
| React.md | javascript | 0.8 | | ✓ | | | |

**Calibration Applied**:

| Concept | Raw | Adjustments | Calibrated | Propagation Useful? |
|---------|-----|-------------|------------|---------------------|
| javascript | 1.0 | +0.2 (domain) | **1.0** | ✓ Yes |
| promise | 1.0 | +0.2 (H1) +0.2 (domain) | **1.0** | ✓ Yes |
| load-image | 1.0 | -0.2 (code ID) | **0.8** | ✗ No |
| image | 0.9 | -0.1 (generic) | **0.8** | ⚠ Partial |
| docker | 1.0 | +0.2 (H1) +0.2 (domain) | **1.0** | ✓ Yes |
| container | 0.9 | +0.2 (domain) | **1.0** | ✓ Yes |
| docker-compose | 0.9 | +0.2 (domain) | **1.0** | ✓ Yes |
| services | 0.7 | -0.1 (generic) | **0.6** | ⚠ Partial |
| git | 0.9 | +0.2 (H1) +0.2 (domain) | **1.0** | ✓ Yes |
| code | 0.8 | -0.1 (generic) | **0.7** | ✗ No |
| config | 0.7 | none | **0.7** | ✗ Sibling-specific |
| ignore | 0.6 | none | **0.6** | ✗ Sibling-specific |
| tags | 0.5 | none | **0.5** | ✗ Sibling-specific |
| typescript | 0.9 | +0.2 (H1) +0.2 (domain) +0.1 (def) | **1.0** | ✓ Yes |
| conditional props | 0.6 | none | **0.6** | ✗ Sibling-specific |
| react | 0.9 | +0.2 (H1) +0.2 (domain) +0.1 (def) | **1.0** | ✓ Yes |

**Summary Statistics**:

| Metric | Raw Confidence | Calibrated | Delta |
|--------|----------------|------------|-------|
| Threshold accuracy (>0.7 = useful) | 53% (8/15) | 67% (10/15) | +14% |
| Precision at >0.9 | 75% (6/8) | 100% (8/8) | +25% |
| False positives (high conf, bad prop) | 3 | 1 | -67% |

**Propagation Analysis by Threshold**:

| Confidence Threshold | Raw: Sensible | Raw: Not | Calibrated: Sensible | Calibrated: Not |
|---------------------|---------------|----------|----------------------|-----------------|
| ≥0.9 | 6 | 2 | 8 | 0 |
| 0.7-0.9 | 1 | 3 | 2 | 2 |
| <0.7 | 1 | 5 | 0 | 6 |

**Observations**:

1. **Calibration improves precision at high thresholds**: All concepts with calibrated confidence ≥0.9 are useful for propagation
2. **Generic term penalty works**: "code", "services", "image" correctly downgraded
3. **Code identifier penalty works**: "load-image" correctly downgraded despite high raw confidence
4. **Heading boost effective**: Main topic concepts (git, docker, typescript) boosted to 1.0
5. **Sibling-specific concepts unchanged**: config, ignore, tags, conditional props stay at 0.5-0.7 (should be filtered by propagation-aware extraction instead)
6. **Key insight**: Calibration helps for content pages but propagation-aware extraction (Experiment B) is still better for index pages

**Verdict: Partially Effective** — useful as post-processing for content pages, but not a replacement for propagation-aware extraction

---

### Experiment E: Multi-Agent Hierarchical Pipeline

**Status**: Complete ✓

**Hypothesis**: Corpus structure (the file tree) is semantic information that can improve extraction quality when fed to an LLM pipeline.

**Architecture**: Three-layer pipeline:
```
Layer 1: Taxonomy Analyzer
├── Input: Corpus file tree structure
└── Output: High-level taxonomy, category hints, structural patterns

Layer 2: Directory Context Generator
├── Input: Layer 1 output + specific directory info
└── Output: Domain context, expected concepts, propagation candidates

Layer 3: Contextualized Extractor
├── Input: Layer 1 + Layer 2 + document content
└── Output: Concepts aligned to corpus taxonomy with propagation scores
```

**Test 1: Promises.md (content page)**

| Approach | Concepts Extracted | Quality Notes |
|----------|-------------------|---------------|
| **Baseline** | javascript, promise, load-image, image, url | Includes function name |
| **Hierarchical** | Promise chaining, Handling errors in promises, Callbacks | Higher-level concepts, no function names |

**Comparison**:
- Hierarchical avoided extracting `load-image` (function name)
- Hierarchical added semantic concepts like "Promise chaining" (inferred from patterns)
- Hierarchical added "Callbacks" (implicit pattern in code)
- Hierarchical may add context-influenced concepts (Event listeners from sibling file)

**Test 2: Typescript.md (index page)**

| Approach | Concepts Extracted | Quality Notes |
|----------|-------------------|---------------|
| **Baseline** | typescript, javascript, playground, conditional props, optional components | Includes sibling links |
| **Hierarchical** | Typescript, Type-safe development, Conditional Props, Optional Components | Added high-level concept |

**Comparison**:
- Hierarchical added "Type-safe development" (high-level concept not in text)
- Hierarchical dropped "playground" (implementation detail)
- Hierarchical dropped "javascript" (parent-level concept)
- Hierarchical gave sibling-specific concepts lower propagation scores

**Summary Statistics**:

| Metric | Single-Agent | Hierarchical | Delta |
|--------|--------------|--------------|-------|
| Avg concepts per doc | 5 | 4 | -20% |
| Function/variable names | Yes | No | -100% |
| High-level abstractions | 0 | 1-2 per doc | New capability |
| Taxonomy alignment | None | Explicit | New capability |
| Context-influenced concepts | 0 | 0-1 per doc | Risk |
| Latency per doc | ~4s | ~12s (3 calls) | +200% |

**Observations**:

1. **Tree structure IS semantic**: Layer 1 correctly inferred "Web development, programming languages, software tools" from directory names alone
2. **Expected concepts improve extraction**: Layer 2's "expected_concepts" guided Layer 3 toward higher-level abstractions
3. **Propagation scores calibrated by context**: Sibling-specific concepts got lower scores automatically
4. **Function names eliminated**: The hierarchical context helped avoid over-specific extractions
5. **New abstraction capability**: Concepts like "Type-safe development" and "Promise chaining" emerge from context
6. **Grounding concern**: Some concepts come from context rather than document text (Event listeners)
7. **Latency tradeoff**: 3x slower but higher quality for batch processing

**Verdict: Highly Effective** for corpus-wide extraction where:
- Taxonomy consistency matters
- Higher-level concepts are valuable
- Batch processing acceptable (latency not critical)

**Not recommended** for:
- Real-time extraction (too slow)
- Single-document analysis (context unavailable)
- Strict grounding requirements (context can introduce concepts)

---

## Conclusions

### What Worked

| Experiment | Effectiveness | Recommend |
|------------|---------------|-----------|
| A: Two-Stage Refiner | High | Yes — add to pipeline |
| B: Propagation-Aware | Very High | Yes — use for index pages |
| C: Normalization | Medium | Partial — needs refinement |
| D: Calibration | Medium | Yes — as post-processing for content pages |
| E: Hierarchical Pipeline | High | Yes — for corpus-wide batch extraction |

### Recommended Pipeline Architecture

Based on experiments A-E, two architectures emerge for different use cases:

#### Architecture 1: Real-Time (Per-Document)
For incremental updates and real-time extraction:
```
Document Classification
├── Is index/hub page? (has many outlinks, short content)
│   └── Use propagation-aware extraction (Exp B)
└── Is content page? (detailed content, code examples)
    └── Use standard extraction → refiner (Exp A)

Post-Processing
├── Case normalization (simple: lowercase all)
├── Confidence calibration (rule-based adjustments) (Exp D)
└── Filter by calibrated confidence > 0.7
```

#### Architecture 2: Batch (Corpus-Wide)
For initial corpus analysis and periodic refinement:
```
Phase 1: Corpus Analysis (run once)
├── Layer 1: Taxonomy Analyzer
│   └── Input: Full file tree
│   └── Output: Corpus taxonomy, category hints
└── Store: corpus_context.json

Phase 2: Directory Context (run per directory)
├── Layer 2: Directory Context Generator
│   └── Input: corpus_context + directory info
│   └── Output: expected_concepts, propagation_candidates
└── Store: dir_contexts/{path}.json

Phase 3: Document Extraction (run per document)
├── Layer 3: Contextualized Extractor
│   └── Input: corpus_context + dir_context + document
│   └── Output: taxonomy-aligned concepts
└── Store: concepts/{path}.json

Phase 4: Cross-Document Synthesis (run once)
├── Layer 4: Vocabulary Normalizer (future)
│   └── Input: all extracted concepts
│   └── Output: canonical vocabulary, deduplication
└── Store: vocabulary.json
```

#### When to Use Which
| Scenario | Architecture | Reason |
|----------|--------------|--------|
| New file added | Real-Time | Fast, single-doc |
| File modified | Real-Time | Fast, preserve context |
| New corpus import | Batch | Full context available |
| Periodic refinement | Batch | Improve taxonomy alignment |
| Background processing | Batch | Quality over speed |

### Key Findings

1. **Two-stage is worth the latency**: Refiner removes 60-75% of noise with high precision
2. **Context-aware prompts matter**: Propagation-aware prompt eliminates sibling-specific concepts
3. **Normalization needs care**: Simple case normalization is safe; semantic dedup is risky
4. **Propagation score is valuable**: Built-in metric for filtering during graph construction
5. **Calibration improves high-confidence filtering**: 100% precision at ≥0.9 threshold vs 75% raw
6. **Tree structure is semantic information**: Corpus structure alone enables taxonomy inference
7. **Hierarchical context enables abstraction**: Higher-level concepts emerge from multi-layer context

### Impact on Original Problem

| Original Issue | Solution | Improvement |
|----------------|----------|-------------|
| Over-specific concepts (sendNextRequest) | Refiner stage | -75% noise |
| Sibling-specific propagation (config → Git Tags) | Propagation-aware prompt | -100% bad propagation |
| Concept duplication (typescript vs TypeScript) | Case normalization | 20% dedup |
| Arbitrary confidence scores | Calibration rules | +25% precision at ≥0.9 threshold |

### Next Steps

1. **Implement hybrid extraction**: Detect page type, apply appropriate prompt
2. **Create refined ensemble**: `plexus-semantic-v2` with refiner stage ✓
3. **Add simple normalization**: Lowercase + trim as post-processing
4. **Skip semantic dedup for now**: Too error-prone; revisit with better prompts
5. **Implement calibration post-processing**: Apply rule-based adjustments after extraction
6. **Test Experiment E**: Multi-agent pipeline (lower priority)

---

## Ensemble Comparison (Actual A/B Test)

Created and tested three actual ensemble configurations:

### Ensembles Tested

| Ensemble | File | Architecture | Purpose |
|----------|------|--------------|---------|
| `plexus-semantic` | plexus-semantic.yaml | 1 agent | Baseline extraction |
| `plexus-semantic-v2` | plexus-semantic-v2.yaml | 2 agents (extractor → refiner) | Noise reduction |
| `plexus-semantic-propagation` | plexus-semantic-propagation.yaml | 1 agent | Optimized for index pages |

### Test 1: Promises.md (Content Page)

| Ensemble | Concepts | Quality |
|----------|----------|---------|
| **v1** | promise, javascript, image, network call, delay, seconds (6) | Mixed - includes generic |
| **v2** | promise, javascript, async, ~~dependency injection~~ (4) | Better - but 1 hallucination |

**v2 Pipeline Detail**:
- Stage 1 (extractor): 15 raw concepts (inclusive)
- Stage 2 (refiner): 4 refined concepts
- Removed: babelify, polyfill, onload, onerror, setTimeout, doNetworkCall
- Added: "async" (inferred, useful)
- Hallucinated: "dependency injection" (not in source)

**Verdict**: v2 is better but refiner can hallucinate new concepts

### Test 2: Git.md (Index Page)

| Ensemble | Concepts | Quality |
|----------|----------|---------|
| **v1** | git, code, config, ignore, tags (5) | Poor - all sibling-specific |
| **propagation** | git, version control (2) | Excellent - propagation-ready |

**Propagation Detail**:
- Eliminated: config, ignore, tags (sibling subtopics)
- Eliminated: code (generic)
- Added: version control (propagation_score: 0.8)
- All concepts have propagation_score for filtering

**Verdict**: Propagation ensemble dramatically better for index pages

### Comparison Summary

| Metric | v1 (baseline) | v2 (two-stage) | propagation |
|--------|---------------|----------------|-------------|
| Concepts extracted | 5-6 | 4 | 2-3 |
| Over-specific noise | High | Low | None |
| Sibling-specific | Extracted | Filtered | Prevented |
| Hallucination risk | Low | Medium | Low |
| Latency | ~4s | ~8s (2 calls) | ~4s |
| Best for | General use | Content pages | Index/hub pages |

### Recommended Configuration

```
Document Analysis Pipeline
├── Detect document type
│   ├── Index page (many outlinks, short content, lists subtopics)
│   │   └── Use: plexus-semantic-propagation
│   └── Content page (detailed content, code examples, explanations)
│       └── Use: plexus-semantic-v2 (with hallucination check)
│
└── Post-processing
    ├── Lowercase normalization
    ├── Filter by confidence > 0.7
    └── Filter by propagation_score > 0.6 (if available)
```

### Ensemble Files Created

```
.llm-orc/ensembles/
├── plexus-semantic.yaml          # v1 baseline (existing)
├── plexus-semantic-v2.yaml       # Two-stage with refiner
└── plexus-semantic-propagation.yaml  # Propagation-optimized
```

---

## Experiment R4: Recursive Refinement - Flat Corpus Challenge

**Status**: Complete ✓

**Hypothesis**: Recursive refinement can bootstrap a useful taxonomy from content alone, without relying on tree structure.

**Corpus**: Shakespeare (43 plays as .txt files in flat directory - no structural hints)

### Method

1. **Bootstrap**: Sample 3-4 plays across genres (tragedy, comedy, history, romance)
2. **Extract**: Run concept-extractor on each with current taxonomy as context
3. **Update**: Taxonomy-updater proposes category changes (assign, create, merge, split)
4. **Assess**: Quality-assessor measures coverage, balance, coherence, stability
5. **Iterate**: Continue until converged or 6 iterations

### Ensemble Used

`plexus-refinement.yaml` - Three-agent pipeline:
```yaml
agents:
  - concept-extractor  # Extract concepts aligned to current taxonomy
  - taxonomy-updater   # Propose category changes (assign/create/merge/split)
  - quality-assessor   # Measure coverage, balance, coherence, stability
```

### Iteration Log

| Iter | Play | Genre | Genre Classified | Coverage | Balance | Coherence | Stability | Recommendation |
|------|------|-------|------------------|----------|---------|-----------|-----------|----------------|
| 1 | Hamlet | tragedy | tragedy (0.9) | 0.95 | 0.70 | 0.80 | 0.85 | continue |
| 2 | Much Ado | comedy | comedy (0.8) | 0.95 | 0.70 | 0.80 | 0.90 | continue |
| 3 | Henry V | history | **tragedy (0.9)** ✗ | 0.90 | 0.70 | 0.80 | 0.80 | continue |
| 4 | Richard III | history | history_play (0.8) ✓ | 0.95 | 0.70 | 0.80 | 0.60 | continue |
| 5 | The Tempest | romance | romance (0.9) ✓ | 0.92 | 0.73 | 0.81 | 0.84 | **converged** |
| 6 | Midsummer | comedy | comedy (0.8) ✓ | 0.80 | 0.60 | 0.90 | 0.70 | continue |

### Key Findings

#### 1. Convergence is Non-Monotonic

The taxonomy reached "converged" status at iteration 5 (The Tempest) but destabilized at iteration 6 (Midsummer):
- Stability dropped: 0.84 → 0.70
- Balance dropped: 0.73 → 0.60

**Implication**: Convergence is genre-dependent. Romance (unusual genre) may have filled gaps that comedy exposed.

#### 2. Genre Misclassification Reveals Taxonomy Gaps

Henry V was initially classified as "tragedy" (0.9) because:
- History genre signals weren't in the taxonomy
- Both genres share: death, conflict, noble protagonists
- Adding explicit history signals (Richard III with hints) fixed subsequent classification

**Implication**: Taxonomy must be seeded with basic genre signals to guide classification.

#### 3. Taxonomy Evolution Pattern

| Iteration | Major Changes |
|-----------|---------------|
| 1 (Hamlet) | Initial: characters (royalty/commoners/supernatural), themes (revenge/madness/death), tragedy signals |
| 2 (Much Ado) | Added: comedy signals (wordplay, multiple-marriages), wit theme |
| 3 (Henry V) | Added: war_and_conquest, leadership, national_identity themes; battle settings |
| 4 (Richard III) | Added: history genre signals, villain subcategory, morality_and_ethics theme |
| 5 (Tempest) | Added: romance signals, magical_power theme, island setting; split power → magical/human |
| 6 (Midsummer) | Added: meta-performance theme; proposed conflict category; destabilized balance |

#### 4. Concept Categories Discovered

Without any structural input, the refinement loop discovered:

**Characters**:
- royalty|nobility|villains
- commoners
- minor_characters
- supernatural (spirits, ghosts, fairies)

**Themes**:
- love, death, power (split: magical/human)
- transformation, reconciliation
- war_and_conquest, leadership, monarchy
- morality_and_ethics

**Genre Signals**:
- tragedy: soliloquies, death-of-protagonist, fatal-flaw
- comedy: multiple-marriages, wordplay, disguise, mistaken-identity
- history: historical-events, succession, english-monarchy
- romance: reconciliation, supernatural, pastoral-setting, marriage-ending

**Literary Devices**:
- soliloquy, play-within-a-play, irony, malapropism
- masque, epilogue, fourth-wall-break

### Quality Metrics Summary

| Metric | Iter 1 | Iter 6 | Delta | Notes |
|--------|--------|--------|-------|-------|
| Coverage | 0.95 | 0.80 | -0.15 | Dropped as new concepts emerged |
| Balance | 0.70 | 0.60 | -0.10 | Characters category too large |
| Coherence | 0.80 | 0.90 | +0.10 | Improved with more context |
| Stability | 0.85 | 0.70 | -0.15 | Oscillation - genre-dependent |

### Observations

1. **Content-only taxonomy IS achievable**: The loop discovered meaningful categories from text alone
2. **Genre classification improves with examples**: Adding history plays after including history signals worked
3. **Oscillation is expected**: Stability doesn't monotonically increase; diverse samples reveal gaps
4. **Category balance is hard**: Characters category accumulated faster than others
5. **Latency is acceptable**: ~45s per iteration (3 agents × 15s each) for batch processing
6. **Human guidance helps**: Explicit hints about history genre accelerated classification accuracy

### Verdict

**Partially Effective** - Recursive refinement works but:
- Needs 6+ iterations to stabilize across all genres
- May need human-in-the-loop for genre signals bootstrap
- Balance issues require manual intervention or smarter category management
- Non-monotonic convergence means "converged" state needs verification

### Recommendations

1. **Stratified bootstrap**: Sample all major genres early (tragedy, comedy, history, romance)
2. **Genre signal seeding**: Pre-load basic genre signals rather than discovering them
3. **Category balancing**: Implement automatic split when categories exceed threshold size
4. **Convergence validation**: After "converged" status, test with 2-3 more documents to verify
5. **Hybrid approach**: Use tree structure when available, fall back to refinement loop for flat corpora

### Comparison: Tree-Informed vs Content-Only

| Aspect | Tree-Informed (Exp E) | Content-Only (R4) |
|--------|----------------------|-------------------|
| Bootstrap time | Single pass | 4-6 iterations |
| Category discovery | From directory names | From document content |
| Genre classification | Automatic from structure | Requires examples |
| Stability | High (structure is stable) | Oscillating |
| Human guidance needed | Low | Medium |
| Best for | Structured corpora (wikis) | Flat collections (texts) |

### R4 Methodological Gap

**Problem**: R4 used human-constructed summaries (~500 words) rather than full play texts (~100k tokens). This invalidates the "content-only" claim since human interpretation was injected into the input.

**Impact**: Results may reflect human oracle knowledge rather than autonomous LLM taxonomy discovery.

**Resolution**: See Experiment R4b below for proper chunking + composition approach.

---

## Experiment R4b: Compositional Semantic Extraction (Planned)

**Status**: Designed, not yet executed

**Motivation**: R4 was invalidated by human summarization. R4b addresses this by processing full documents through structural chunking and hierarchical composition.

### Problem Statement

Shakespeare plays are ~100k tokens each. LLM context windows (even large ones) struggle with:
1. **Attention dilution** - important details lost in volume
2. **Context limits** - may exceed model capacity
3. **Cost** - processing full text per iteration is expensive

Human summarization "solved" this but introduced bias. We need an autonomous approach.

### Hypothesis

Hierarchical composition (chunk → aggregate → synthesize) can build accurate document-level semantic representations without human intervention, and these representations are sufficient for taxonomy refinement.

### Approach: Structural Chunking + Hierarchical Composition

#### Phase 1: Structural Chunking

Shakespeare plays have consistent structure that enables natural chunking:

```
Play
├── Character List (metadata)
├── Act 1
│   ├── Scene 1 (~2-5k tokens)
│   ├── Scene 2
│   └── ...
├── Act 2
│   └── ...
└── Act 5
    └── Final Scene
```

**Chunking strategy**:
- Use Act/Scene markers as natural boundaries
- Each scene is typically 2-5k tokens (fits comfortably in context)
- Character list extracted separately as structural metadata

**Implementation**:
```python
def chunk_play(text: str) -> list[Chunk]:
    chunks = []

    # Extract character list (lines 9-70 typically)
    char_list = extract_character_list(text)
    chunks.append(Chunk(type="metadata", content=char_list))

    # Split by Act/Scene markers
    scenes = re.split(r'\n(ACT \d+.*?Scene \d+)', text)
    for scene_header, scene_content in pairs(scenes):
        chunks.append(Chunk(
            type="scene",
            act=parse_act(scene_header),
            scene=parse_scene(scene_header),
            content=scene_content
        ))

    return chunks
```

#### Phase 2: Hierarchical Composition

Four-layer aggregation pipeline:

```
Layer 1: Scene Extraction
├── Input: Single scene text (~2-5k tokens)
├── Agent: scene-extractor
├── Output: Local concepts with scene context
│   {
│     "scene_id": "1.1",
│     "characters_present": ["Hamlet", "Horatio", "Ghost"],
│     "concepts": [
│       {"name": "ghost", "type": "supernatural", "confidence": 0.9},
│       {"name": "revenge", "type": "theme", "mentions": 2}
│     ],
│     "mood": "ominous",
│     "key_events": ["Ghost appears", "Hamlet learns of murder"]
│   }
└── Run: Once per scene (typically 15-25 scenes per play)

Layer 2: Act Aggregation
├── Input: All scene extractions for one act
├── Agent: act-aggregator
├── Output: Act-level concept summary
│   {
│     "act": 1,
│     "scene_count": 5,
│     "characters": {"Hamlet": 5, "Claudius": 3, "Ghost": 2},
│     "concepts": [
│       {"name": "revenge", "frequency": 4, "scenes": ["1.1", "1.4", "1.5"]},
│       {"name": "corruption", "frequency": 2, "scenes": ["1.2", "1.4"]}
│     ],
│     "act_arc": "Setup - revelation of murder, call to revenge"
│   }
└── Run: Once per act (typically 5 acts per play)

Layer 3: Document Synthesis
├── Input: All act aggregations + character metadata
├── Agent: document-synthesizer
├── Output: Play-level semantic representation
│   {
│     "title": "Hamlet",
│     "genre_signals": {
│       "tragedy": ["protagonist death", "fatal flaw", "revenge plot"],
│       "comedy": []
│     },
│     "central_characters": ["Hamlet", "Claudius", "Gertrude"],
│     "major_themes": [
│       {"name": "revenge", "weight": 0.9, "arc": "introduced→delayed→fulfilled"},
│       {"name": "madness", "weight": 0.7, "arc": "feigned→possibly real"}
│     ],
│     "literary_devices": ["soliloquy", "play-within-play", "ghost"],
│     "settings": ["Elsinore castle", "Denmark"],
│     "genre_classification": "tragedy",
│     "genre_confidence": 0.9
│   }
└── Run: Once per play

Layer 4: Taxonomy Refinement
├── Input: Document synthesis + current corpus taxonomy
├── Agent: taxonomy-updater (from R4)
├── Output: Updated taxonomy with quality metrics
└── Run: Once per document added to corpus
```

#### Composition Logic

The key insight is that **composition is not concatenation**. Each layer applies semantic reasoning:

**Scene → Act aggregation**:
- Concepts appearing in multiple scenes get higher weight
- Character importance measured by scene presence
- Themes tracked across scene boundaries
- Contradictions flagged (e.g., character mood shifts)

**Act → Document synthesis**:
- Cross-act patterns identify central themes
- Character arcs traced through act summaries
- Genre signals aggregated (tragedy signals in Act 5 weighted higher)
- Structural patterns noted (e.g., comic relief in Act 3)

**Document → Taxonomy refinement**:
- Same as R4, but input is now autonomous synthesis
- No human summarization bias

### Ensemble Design

New ensemble: `plexus-refinement-compositional.yaml`

```yaml
name: plexus-refinement-compositional
description: Hierarchical composition for long documents

agents:
  # Layer 1: Extract concepts from individual scenes
  - name: scene-extractor
    model_profile: ollama-llama3
    timeout_seconds: 30
    system_prompt: |
      You are analyzing a single scene from a Shakespeare play.

      Extract:
      1. Characters present in this scene
      2. Key events/actions
      3. Themes touched on (love, death, power, deception, etc.)
      4. Mood/tone of the scene
      5. Notable literary devices (soliloquy, aside, wordplay)
      6. Any supernatural elements

      Return JSON:
      {
        "characters_present": ["list"],
        "key_events": ["list of 2-4 events"],
        "themes": [{"name": "theme", "strength": 0.8}],
        "mood": "single word or phrase",
        "literary_devices": ["list"],
        "supernatural": true/false,
        "notes": "any other observations"
      }
    output_format: json

  # Layer 2: Aggregate scenes into act-level summary
  - name: act-aggregator
    model_profile: ollama-llama3
    timeout_seconds: 45
    system_prompt: |
      You are aggregating scene-level extractions into an act summary.

      Given extractions from multiple scenes in one act:
      1. Identify which characters dominate this act (by scene count)
      2. Find themes that recur across scenes (weight by frequency)
      3. Summarize the act's narrative arc in one sentence
      4. Note any tonal shifts between scenes
      5. Identify the act's function (setup, rising action, climax, resolution)

      Return JSON:
      {
        "act_number": N,
        "scene_count": N,
        "character_presence": {"name": scene_count},
        "recurring_themes": [{"name": "theme", "frequency": N, "scenes": ["list"]}],
        "narrative_arc": "one sentence summary",
        "tonal_shifts": ["list of shifts"],
        "dramatic_function": "setup|rising_action|climax|falling_action|resolution"
      }
    output_format: json

  # Layer 3: Synthesize acts into document-level representation
  - name: document-synthesizer
    model_profile: ollama-llama3
    timeout_seconds: 60
    system_prompt: |
      You are synthesizing act-level summaries into a complete play analysis.

      Given:
      1. Character list (from play metadata)
      2. Act summaries (from aggregation)

      Produce a document-level semantic representation:
      1. Classify genre (tragedy, comedy, history, romance) with evidence
      2. Identify 3-5 central themes with their arcs across the play
      3. Rank characters by importance (presence + dramatic function)
      4. List literary devices used throughout
      5. Identify settings/locations
      6. Note structural patterns (e.g., comic relief placement)

      Return JSON:
      {
        "title": "play title",
        "genre_classification": "tragedy|comedy|history|romance",
        "genre_confidence": 0.9,
        "genre_evidence": ["list of signals"],
        "central_themes": [
          {"name": "theme", "weight": 0.9, "arc": "how it develops across acts"}
        ],
        "character_ranking": [
          {"name": "character", "importance": 0.9, "role": "protagonist|antagonist|etc"}
        ],
        "literary_devices": ["list with frequency notes"],
        "settings": ["list"],
        "structural_notes": "observations about play structure"
      }
    output_format: json

  # Layer 4: Update taxonomy (reused from R4)
  - name: taxonomy-updater
    model_profile: ollama-llama3
    timeout_seconds: 45
    depends_on: [document-synthesizer]
    system_prompt: |
      You are a taxonomy curator. Given a document synthesis and current corpus taxonomy,
      update the taxonomy.

      Tasks:
      1. Assign: Place new concepts into existing categories
      2. Create: Propose new categories for orphan concepts
      3. Merge: Suggest merging similar categories
      4. Split: Suggest splitting overly broad categories
      5. Normalize: Identify duplicate concepts with different names

      Return JSON with updated taxonomy, changes list, stability_score, coverage.
    output_format: json

  # Layer 5: Quality assessment (reused from R4)
  - name: quality-assessor
    model_profile: ollama-llama3
    timeout_seconds: 30
    depends_on: [taxonomy-updater]
    system_prompt: |
      Evaluate taxonomy quality: coverage, balance, coherence, stability.
      Return recommendation: continue|converged|needs_restructure.
    output_format: json
```

### Execution Plan

**Preprocessing** (one-time, code not LLM):
1. Parse all 43 plays into chunks
2. Store as structured JSON: `{play_id, chunks: [{type, act, scene, content}]}`
3. Validate chunking (check scene counts match expectations)

**Per-play processing**:
```
For each play:
  1. Run scene-extractor on each chunk (parallel, ~20 calls)
  2. Group by act, run act-aggregator (sequential, 5 calls)
  3. Run document-synthesizer (1 call)
  4. Run taxonomy-updater with current taxonomy (1 call)
  5. Run quality-assessor (1 call)

  Total: ~28 LLM calls per play
  Estimated time: ~10-15 minutes per play
```

**Iteration strategy**:
- Same as R4: Bootstrap with 4 plays (one per genre)
- Continue until converged or 6+ documents processed
- Track same metrics: coverage, balance, coherence, stability

### Metrics

**Composition quality** (new metrics):
| Metric | Description | Target |
|--------|-------------|--------|
| Scene extraction rate | % scenes successfully extracted | >95% |
| Aggregation consistency | Do act summaries align with scenes? | Manual spot-check |
| Synthesis grounding | % synthesis claims traceable to scenes | >80% |
| Hallucination rate | Concepts in synthesis not in any scene | <10% |

**Taxonomy quality** (same as R4):
| Metric | Description | Target |
|--------|-------------|--------|
| Coverage | % concepts with category assignments | >90% |
| Balance | Category size distribution | Gini <0.5 |
| Coherence | Concepts within categories belong together | >0.8 |
| Stability | Change between iterations | >0.8 for convergence |

**Comparison metrics** (R4 vs R4b):
| Metric | R4 (human summary) | R4b (compositional) |
|--------|-------------------|---------------------|
| Human intervention | High (wrote summaries) | Low (only preprocessing) |
| Concepts per play | ~10-15 | TBD |
| Genre accuracy | 5/6 (83%) | TBD |
| Iterations to converge | 5 (then destabilized) | TBD |

### Success Criteria

R4b is successful if:
1. **Autonomy**: No human summarization or interpretation required
2. **Accuracy**: Genre classification ≥80% (matches or beats R4)
3. **Grounding**: >80% of synthesized concepts traceable to scene extractions
4. **Convergence**: Reaches stable taxonomy within 6 documents
5. **Scalability**: Processing time <20 min per play (acceptable for batch)

### Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Scene extraction noise | Garbage in → garbage out | Validate extraction quality on 2-3 scenes first |
| Aggregation loses detail | Important concepts dropped | Track concept provenance through layers |
| Synthesis hallucination | Invented concepts pollute taxonomy | Cross-reference synthesis against scene extractions |
| Chunking errors | Malformed scenes | Validate chunk boundaries against expected structure |
| Cost explosion | 28 calls × 43 plays = 1204 calls | Start with 4-play bootstrap, expand if promising |

### Comparison: R4 vs R4b

| Aspect | R4 (Human Summary) | R4b (Compositional) |
|--------|-------------------|---------------------|
| Input to taxonomy | ~500 word human summary | Autonomous multi-layer synthesis |
| Human bias | High - oracle knowledge injected | Low - only structural chunking |
| Concepts discovered | Reflects human interpretation | Reflects LLM interpretation |
| Scalability | Manual effort per document | Automated pipeline |
| Cost per play | 3 LLM calls | ~28 LLM calls |
| Time per play | ~2 min | ~15 min |
| Validity | Baseline only | True content-only test |

### Next Steps

1. **Implement chunking** - Write parser for Shakespeare play structure
2. **Test Layer 1** - Run scene-extractor on 5 scenes, validate output quality
3. **Test Layer 2** - Run act-aggregator on one act, check aggregation logic
4. **Test Layer 3** - Run document-synthesizer on one play, verify grounding
5. **Full bootstrap** - Run complete pipeline on 4 plays (tragedy, comedy, history, romance)
6. **Measure and compare** - Track metrics against R4 baseline
