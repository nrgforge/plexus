# Ensemble Architecture & Validation

> Part of [Plexus LLM Semantic Integration](./README.md)

---

## 4. Ensemble Architecture

### 4.1 Design Pattern: Micro-Ensembles

Rather than one monolithic ensemble, we use **specialized micro-ensembles**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        MICRO-ENSEMBLE ARCHITECTURE                          │
│                        (Section-Level Processing)                           │
│                                                                             │
│  ┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────┐ │
│  │  ConceptExtractor   │    │   CategoryNamer     │    │ ClusterSummary  │ │
│  │                     │    │                     │    │                 │ │
│  │  Input: SECTION     │    │  Input: Concept[]   │    │  Input: Sects[] │ │
│  │  + heading context  │    │  Output: Name       │    │  Output: Text   │ │
│  │  Output: Concepts[] │    │                     │    │                 │ │
│  │                     │    │  Model: Reasoning   │    │  Model: Fast    │ │
│  │  Model: Fast/Local  │    │  (claude, gpt-4)    │    │  (llama)        │ │
│  └─────────────────────┘    └─────────────────────┘    └─────────────────┘ │
│                                                                             │
│  ┌─────────────────────┐    ┌─────────────────────┐                        │
│  │  ConceptVerifier    │    │ RelationInferrer    │                        │
│  │                     │    │                     │                        │
│  │  Input: Section,    │    │  Input: Concept[]   │                        │
│  │         Candidates  │    │  Output: Relations[]│                        │
│  │  Output: Verified[] │    │                     │                        │
│  │                     │    │  Model: Reasoning   │                        │
│  │  Model: Fast        │    │  (claude)           │                        │
│  └─────────────────────┘    └─────────────────────┘                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key change**: Extraction operates on **sections**, not whole documents. This provides:
- More focused context → better extraction accuracy
- Concepts tied to specific locations in the document
- Ability to detect topic shifts within a single file

### 4.2 Why Micro-Ensembles?

| Benefit | Explanation |
|---------|-------------|
| **Single Responsibility** | Each ensemble does one thing well |
| **Model Flexibility** | Use fast models for extraction, reasoning models for synthesis |
| **Cost Optimization** | Expensive models only where needed |
| **Independent Tuning** | Improve one without affecting others |
| **Failure Isolation** | One ensemble failing doesn't break pipeline |

### 4.3 Ensemble Specifications

#### ConceptExtractor
```yaml
# .llm-orc/ensembles/plexus-concept-extractor.yaml
name: plexus-concept-extractor
description: Extract semantic concepts from a document SECTION

agents:
  - name: extractor
    model_profile: fast-local  # llama-3.2, qwen-2.5
    script: concept-extraction

primitives:
  concept-extraction:
    system: |
      You are a concept extraction specialist. Given a SECTION of a document
      (not the whole document), identify the key concepts discussed in that section.

      You will receive:
      - section_content: The text of this section
      - heading_path: The heading hierarchy (e.g., "Authentication > OAuth > Refresh Tokens")
      - document_title: The parent document name
      - sibling_headings: Other sections in the same document (for context)

      Output JSON with this structure:
      {
        "concepts": [
          {
            "name": "concept name",
            "kind": "describe what this is in your own words",
            "confidence": 0.0-1.0,
            "evidence": "quote from section"
          }
        ],
        "section_theme": "one-line description of what this section is about",
        "domain": "detected domain (e.g., software, science, business)"
      }

      IMPORTANT:
      - Extract concepts from THIS SECTION only, not the whole document
      - Use heading_path as context for disambiguation
      - Include 2-7 concepts per section (fewer than whole-doc extraction)
      - Higher confidence = more explicit mention in section text
      - Evidence must come from section_content, not inferred from headings
```

**Section Context Input**:
```yaml
# Example input to the extractor
input:
  section_content: |
    OAuth refresh tokens allow clients to obtain new access tokens
    without requiring user re-authentication. Store refresh tokens
    securely and rotate them periodically.
  heading_path: "Authentication > OAuth > Refresh Tokens"
  document_title: "API Security Guide"
  sibling_headings:
    - "Authentication > OAuth > Access Tokens"
    - "Authentication > OAuth > Scopes"
    - "Authentication > API Keys"
```

#### Chunked Extraction for Large Sections

**Problem**: Some sections are too large for a single LLM context window (e.g., an entire Shakespeare act, a long technical chapter). We need iterative extraction with concept accumulation.

**Solution**: Script/Model hybrid ensemble that chunks content and accumulates concepts progressively.

```yaml
# .llm-orc/ensembles/plexus-chunked-extractor.yaml
name: plexus-chunked-extractor
description: Extract concepts from large sections via chunking and accumulation

agents:
  - name: chunker
    type: script  # No LLM, just logic
    script: chunk-section

  - name: extractor
    model_profile: fast-local
    script: chunk-extraction

  - name: accumulator
    type: script  # Merges concepts across chunks
    script: concept-accumulation

  - name: deduplicator
    model_profile: fast-local  # Optional: LLM-assisted dedup
    script: concept-dedup

flow:
  - chunker → extractor (foreach chunk)
  - extractor → accumulator (streaming)
  - accumulator → deduplicator (once, at end)
```

**Chunking Strategy** (script):
```python
def chunk_section(section_content: str, config: ChunkConfig) -> List[Chunk]:
    """
    Split large section into overlapping chunks for extraction.

    Strategies by content type:
    - Prose: Split on paragraph boundaries, ~500-1000 words per chunk
    - Code: Split on function/class boundaries
    - Drama: Split on scene/speech boundaries
    - Lists: Group related items
    """
    if len(section_content.split()) < config.min_chunk_words:
        return [Chunk(content=section_content, index=0, total=1)]

    # Detect content type and split accordingly
    chunks = []
    boundaries = detect_boundaries(section_content)

    current_chunk = []
    current_words = 0

    for segment in split_on_boundaries(section_content, boundaries):
        if current_words + len(segment.split()) > config.target_chunk_words:
            chunks.append(Chunk(
                content="\n".join(current_chunk),
                index=len(chunks),
                overlap_context=get_overlap(current_chunk)  # Last paragraph for continuity
            ))
            current_chunk = [segment]
            current_words = len(segment.split())
        else:
            current_chunk.append(segment)
            current_words += len(segment.split())

    # Don't forget the last chunk
    if current_chunk:
        chunks.append(Chunk(content="\n".join(current_chunk), index=len(chunks)))

    return chunks
```

**Concept Accumulation** (script):
```python
def accumulate_concepts(
    chunk_results: List[ExtractionResult],
    running_concepts: Dict[str, Concept]
) -> Dict[str, Concept]:
    """
    Merge concepts from new chunk into running accumulation.

    Rules:
    - Same canonical name: merge, boost confidence
    - Similar name (fuzzy): flag for LLM dedup
    - New concept: add to vocabulary
    """
    for result in chunk_results:
        for concept in result.concepts:
            canonical = normalize(concept.name)

            if canonical in running_concepts:
                # Seen before: boost confidence, merge evidence
                existing = running_concepts[canonical]
                existing.confidence = min(1.0, existing.confidence + concept.confidence * 0.5)
                existing.evidence.extend(concept.evidence)
                existing.chunk_count += 1
            else:
                # Check for fuzzy matches
                similar = find_similar(canonical, running_concepts.keys(), threshold=0.85)
                if similar:
                    # Flag for LLM dedup later
                    concept.potential_duplicates = similar

                running_concepts[canonical] = concept

    return running_concepts
```

**Extraction with Context** (model prompt):
```yaml
primitives:
  chunk-extraction:
    system: |
      You are extracting concepts from a CHUNK of a larger section.

      You will receive:
      - chunk_content: The text of this chunk
      - chunk_index: Which chunk this is (0-indexed)
      - total_chunks: How many chunks total
      - heading_path: The section heading hierarchy
      - prior_concepts: Concepts already found in earlier chunks (for context)

      Output JSON:
      {
        "concepts": [...],
        "continues_from_prior": ["concept names that this chunk continues discussing"],
        "new_concepts": ["concept names introduced in this chunk"]
      }

      IMPORTANT:
      - If prior_concepts mentions "Hamlet" and this chunk discusses Hamlet,
        note it in continues_from_prior, don't re-extract
      - Focus on NEW concepts not already in prior_concepts
      - Use prior_concepts for disambiguation context
```

**When to Use Chunked Extraction**:

| Section Size | Strategy | Rationale |
|--------------|----------|-----------|
| < 500 words | Single extraction | Fits comfortably in context |
| 500-2000 words | Single with truncation | May lose some detail |
| 2000-10000 words | Chunked (2-5 chunks) | Maintains quality |
| > 10000 words | Chunked + sampling | Extract from representative chunks |

**Benefits of Script/Model Hybrid**:
- Scripts handle deterministic logic (chunking, merging) at zero cost
- Models handle semantic tasks (extraction, dedup) where needed
- Streaming accumulation: concepts available progressively
- Cheaper than sending huge context to LLM

#### CategoryNamer
```yaml
# .llm-orc/ensembles/plexus-category-namer.yaml
name: plexus-category-namer
description: Generate human-readable names for concept clusters

agents:
  - name: namer
    model_profile: reasoning  # claude-sonnet, gpt-4
    script: category-naming

primitives:
  category-naming:
    system: |
      You are an ontology specialist. Given a cluster of related concepts,
      generate a concise, descriptive category name.

      Output JSON:
      {
        "name": "CategoryName",
        "description": "One sentence explaining this category",
        "parent_suggestion": "broader category this might belong to"
      }

      Guidelines:
      - Name should be 1-3 words
      - Use domain-appropriate terminology
      - Prefer specificity over generality
```

#### ClusterSummarizer
```yaml
# .llm-orc/ensembles/plexus-summarizer.yaml
name: plexus-summarizer
description: Generate summaries for document clusters (meta-nodes)

agents:
  - name: summarizer
    model_profile: fast-local
    script: cluster-summary

primitives:
  cluster-summary:
    system: |
      Given sample documents from a cluster, generate a 1-2 sentence
      summary that captures what unifies these documents.

      Output JSON:
      {
        "summary": "The summary text",
        "key_topics": ["topic1", "topic2"],
        "representative_doc": "which sample best represents the cluster"
      }
```

---

## 5. Validation Architecture

LLM outputs are probabilistic. For a pipeline that propagates labels through a graph, validation errors compound. We use a three-level validation pyramid.

### 5.1 Validation Levels

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATION PYRAMID                                   │
│                                                                             │
│                         ┌─────────────────┐                                 │
│                         │  L3: Semantic   │  ← LLM judge (expensive)        │
│                         │   Validation    │     "Does this make sense?"     │
│                         └────────┬────────┘                                 │
│                    ┌─────────────┴─────────────┐                            │
│                    │   L2: Evidence Grounding  │  ← Script agent (cheap)    │
│                    │   "Can we verify claims?" │     String matching, etc.  │
│                    └─────────────┬─────────────┘                            │
│         ┌────────────────────────┴────────────────────────┐                 │
│         │         L1: Structural Validation               │  ← Deterministic│
│         │   JSON schema, type checks, required fields     │     Zero cost   │
│         └─────────────────────────────────────────────────┘                 │
│                                                                             │
│  Every output passes L1. L2 for extraction tasks. L3 for high-stakes.      │
└─────────────────────────────────────────────────────────────────────────────┘
```

| Level | Type | Cost | When Applied |
|-------|------|------|--------------|
| **L1: Structural** | Script (JSON schema) | Zero | Every LLM output |
| **L2: Grounding** | Script (string matching) | Minimal | Extraction tasks |
| **L3: Semantic** | LLM judge | Expensive | Hub docs, low-confidence |

### 5.2 Validation Scripts

#### L1: Schema Validation
```yaml
# .llm-orc/scripts/validators/json-schema.yaml
name: json-schema-validator
type: script

script: |
  import json, jsonschema

  try:
      data = json.loads(llm_output)
      jsonschema.validate(data, schema)
      return {"valid": True, "data": data}
  except json.JSONDecodeError as e:
      return {"valid": False, "error": f"Invalid JSON: {e}"}
  except jsonschema.ValidationError as e:
      return {"valid": False, "error": f"Schema: {e.message}"}
```

#### L2: Evidence Grounding
```yaml
# .llm-orc/scripts/validators/evidence-grounding.yaml
name: evidence-grounding
type: script

script: |
  results = []
  source_lower = source_document.lower()

  for concept in extracted_concepts:
      evidence = concept.get("evidence", "")
      name = concept.get("name", "")

      # Verify evidence quote exists in source
      evidence_found = evidence.lower() in source_lower
      name_found = name.lower() in source_lower

      grounded = evidence_found or name_found
      results.append({"concept": name, "grounded": grounded})

  valid = [c for c, r in zip(extracted_concepts, results) if r["grounded"]]
  score = len(valid) / len(extracted_concepts) if extracted_concepts else 0

  return {"valid_concepts": valid, "grounding_score": score}
```

#### L3: Semantic Validation
```yaml
# .llm-orc/ensembles/plexus-validator.yaml
name: plexus-validator
agents:
  - name: judge
    model_profile: reasoning-small  # phi-3, qwen-2.5-7b
    script: semantic-validation

primitives:
  semantic-validation:
    system: |
      Given a document and extracted concepts, judge each:
      - VALID: Correctly identifies something discussed
      - HALLUCINATED: Not in document, made up
      - OVERGENERALIZED: Too broad
      - UNDERGENERALIZED: Too specific

      Output: {"validations": [...], "quality_score": 0.0-1.0}
```

### 5.3 Validation Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    EXTRACTION WITH VALIDATION                                │
│                                                                             │
│  Document ──► Extractor (LLM) ──► L1: Schema ──┬──► L2: Grounding           │
│                                                │                            │
│                                           fail │                            │
│                                                ▼                            │
│                                    Retry with feedback                      │
│                                    (max 2 attempts)                         │
│                                                                             │
│  After L2:                                                                  │
│    IF is_hub_document OR grounding_score < 0.5:                            │
│        ──► L3: Semantic Validation                                         │
│    ELSE:                                                                    │
│        ──► Pass through                                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.4 Validation Triggers (Section-Level)

Validation now operates at **section level**, with triggers based on section importance:

| Section Type | L1 | L2 | L3 | Notes |
|--------------|----|----|-----|-------|
| Normal section | ✓ | ✓ | ✗ | Standard extraction |
| High-importance section | ✓ | ✓ | ✓ | From high-PageRank doc or has inbound anchors |
| Bridge section | ✓ | ✓ | ✓ | High betweenness, connects clusters |
| Low grounding score (<0.5) | ✓ | ✓ | ✓ | Potential hallucinations |
| Category naming | ✓ | ✗ | Optional | Concepts already validated |
| Cluster summarization | ✓ | ✗ | ✗ | Low stakes |

**Section importance factors for L3 trigger**:
- Parent document has high PageRank
- Section has direct inbound anchor links
- Section is in a "core" directory (configurable paths)
- Section is H1 level (document's primary topic)

---

## 6. Model Selection

### 6.1 Task-to-Model Mapping

Different tasks have different requirements. Match model capabilities to task needs:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        MODEL SELECTION MATRIX                                │
│                                                                             │
│  Task Characteristics        Recommended Model Class       Size Range       │
│  ────────────────────        ───────────────────────       ──────────       │
│                                                                             │
│  Structured extraction       Instruction-tuned, JSON mode  1-3B             │
│  (concepts, entities)        qwen2.5-1.5b, phi-3-mini                       │
│                                                                             │
│  Classification/Judgment     Balanced reasoning            3-7B             │
│  (validation, verification)  qwen2.5-7b, llama-3.2-3b                       │
│                                                                             │
│  Summarization               Fluent generation             3-7B             │
│  (cluster summaries)         llama-3.2-3b, mistral-7b                       │
│                                                                             │
│  Category naming             Creative + structured         7-14B            │
│  (ontology labels)           qwen2.5-14b, llama-3.1-8b                      │
│                                                                             │
│  Complex reasoning           Strong reasoning chain        14B+ or API      │
│  (relationship inference)    qwen2.5-32b, claude-haiku                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Model Profiles

Define reusable profiles in llm-orc:

```yaml
# .llm-orc/profiles/plexus-models.yaml

# Ultra-fast extraction (bulk operations)
extraction-micro:
  provider: ollama
  model: qwen2.5:1.5b
  options:
    temperature: 0.1      # Low temp for consistency
    num_predict: 512      # Limit output length
    format: json          # Force JSON mode if supported

# Fast validation and simple judgments
validation-fast:
  provider: ollama
  model: qwen2.5:3b
  options:
    temperature: 0.2
    num_predict: 256

# Balanced for summarization
summarization:
  provider: ollama
  model: llama3.2:3b
  options:
    temperature: 0.4      # Slightly more creative
    num_predict: 200

# Higher quality for naming/synthesis
reasoning-local:
  provider: ollama
  model: qwen2.5:7b
  options:
    temperature: 0.3
    num_predict: 256

# Best local reasoning (when needed)
reasoning-strong:
  provider: ollama
  model: qwen2.5:14b
  options:
    temperature: 0.2
    num_predict: 512

# Fallback to API for complex tasks
reasoning-api:
  provider: anthropic
  model: claude-3-haiku-20240307
  options:
    temperature: 0.2
    max_tokens: 512
```

### 6.3 Ensemble-to-Profile Mapping

| Ensemble | Primary Task | Recommended Profile | Fallback |
|----------|--------------|---------------------|----------|
| `concept-extractor` | Structured extraction | `extraction-micro` | `validation-fast` |
| `evidence-grounder` | Script (no LLM) | N/A | N/A |
| `semantic-validator` | Judgment | `validation-fast` | `reasoning-local` |
| `category-namer` | Creative naming | `reasoning-local` | `reasoning-api` |
| `cluster-summarizer` | Summarization | `summarization` | `reasoning-local` |
| `relation-inferrer` | Complex reasoning | `reasoning-strong` | `reasoning-api` |

### 6.4 Model Selection Criteria

When choosing models for each task:

| Criterion | Extraction | Validation | Naming | Reasoning |
|-----------|------------|------------|--------|-----------|
| **JSON reliability** | Critical | Important | Moderate | Moderate |
| **Instruction following** | Critical | Critical | Important | Important |
| **Creativity** | Low | Low | Moderate | Low |
| **Context window** | Moderate | Small | Small | Large |
| **Speed** | Critical | Important | Moderate | Less critical |
| **Consistency** | Critical | Critical | Moderate | Important |

### 6.5 Quantization Guidelines

For local-first deployment, quantization affects quality/speed tradeoff:

| Quantization | Use Case | Quality Impact |
|--------------|----------|----------------|
| **Q4_K_M** | Default for most tasks | ~5% degradation, good balance |
| **Q5_K_M** | Validation, naming | ~2% degradation, better quality |
| **Q8_0** | Complex reasoning | Minimal degradation |
| **F16** | When quality critical | Full quality, 2x memory |

**Recommendation**: Use Q4_K_M for extraction (bulk), Q5_K_M for validation/naming.

### 6.6 Specialized Fine-Tuning Opportunities

For production deployment, consider fine-tuning micro-models:

| Task | Base Model | Fine-tuning Data | Expected Gain |
|------|------------|------------------|---------------|
| Concept extraction | qwen2.5-1.5b | 1K document→concepts pairs | +15-20% accuracy |
| Evidence grounding | phi-3-mini | 500 grounded/ungrounded examples | +10% precision |
| Category naming | llama-3.2-3b | 200 concept-cluster→name pairs | +20% coherence |

Fine-tuning enables using smaller models with domain-specific performance.

---

## Next: [03-complexity-dataflow.md](./03-complexity-dataflow.md) — Complexity Analysis & Data Flow
