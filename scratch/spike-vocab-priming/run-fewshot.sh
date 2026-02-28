#!/bin/bash
# Condition E: Glossary-primed + Structured + Few-shot examples
# Tests whether examples of compound term extraction recover neologisms

set -e
RESULTS_DIR="$(dirname "$0")/results"
mkdir -p "$RESULTS_DIR"

read -r -d '' ESSAY_TEXT << 'ESSAYEOF' || true
Provenance as Epistemological Infrastructure

Essay 11 demonstrated that two consumers — Trellis (fragments) and Carrel (provenance marks) — produce cross-dimensional connections through shared tag vocabulary. The graph structure was satisfying: 19 nodes, 45 edges, clean dimension separation. But a question lingered. Why were there two separate adapter-shaped things producing provenance? And could you actually trace a concept back to its source?

The Wrong Question

The first attempt at Essay 12 asked: "Should the pipeline automatically add provenance metadata when adapters ingest data?" The spike modified IngestPipeline to create ingest_record nodes at step 2.5 — operational metadata recording which adapter processed what, when. All 248 tests passed. The essay was written and committed.

Then the question was examined more carefully.

An ingest_record node tells you how knowledge arrived: "adapter X processed input Y at time Z." It does not tell you where the knowledge came from. If you start at concept:travel and want to know why the graph believes this concept exists, operational metadata points you to a pipeline invocation. Source provenance points you to the passage about walking through Avignon.

The distinction is between epistemology and bookkeeping. Plexus claims to be a knowledge graph. A knowledge graph that can explain what it knows but not why it believes it is doing bookkeeping, not epistemology.

The commit was reverted.

The Real Question

Chains, marks, and links are not just a user annotation feature. They are Plexus's epistemological infrastructure — the mechanism by which the graph knows why it knows things. In Essay 11, provenance marks existed only when the user explicitly created them via ProvenanceAdapter. The fragments that produced concept nodes had no provenance trail at all. You could ask "what does the graph know?" (traverse concepts) but not "where did this knowledge come from?" (traverse provenance).

The real question: should every adapter that introduces knowledge into the graph also produce a provenance trail for that knowledge?

Where Provenance Must Live

Three options were considered:

Pipeline-level: The ingest pipeline wraps adapter output in provenance after processing. The pipeline doesn't know what the nodes represent. It would produce marks like "this emission happened" — operational metadata again.

Sink-level: EngineSink auto-generates provenance on commit. Same problem. The sink validates and commits but doesn't understand domain semantics. Marks without meaningful annotations are noise.

Adapter-level: The adapter that understands the source material produces provenance alongside semantics. Only the adapter knows the text, the source identifier, the relevant tags, and the appropriate annotation.

The answer was adapter-level. Not because the others are technically impossible, but because provenance without domain knowledge is empty. A mark that says "node was created" is bookkeeping. A mark that says "Walked through Avignon, thinking about distributed systems" and carries tags [travel, distributed-ai] is evidence.

The Design

FragmentAdapter's process() method now produces six things instead of three:

Semantic output (unchanged): a fragment node (structure dimension), concept nodes (semantic dimension), and tagged_with edges (cross-dimensional, with Hebbian contributions).

Provenance output (new): a chain node (provenance dimension, deterministic ID per adapter+source), a mark node (provenance dimension, with annotation text, source file, and tags), and a contains edge (chain to mark, within provenance dimension).

The chain ID is deterministic: chain:<adapter_id>:<source>. Re-ingesting from the same source upserts the existing chain — no duplicates. Each fragment gets its own mark within the chain, carrying the fragment's text as annotation and its tags as a property.

The tags on the mark are the critical detail. TagConceptBridger — the existing enrichment from Essay 11 — automatically creates references edges from any provenance-dimension node with tags to matching concept nodes. No new enrichment code was needed. The existing design handles this perfectly.

What the Graph Now Knows

Consider a single fragment ingest: "Walked through Avignon, thinking about distributed systems," tagged [travel, distributed-ai], source journal-2026-02.

The graph produces:
- 1 fragment node (structure dimension)
- 2 concept nodes: concept:travel, concept:distributed-ai (semantic dimension)
- 2 tagged_with edges (structure to semantic, with Hebbian contributions)
- 1 chain node: chain:journal:journal-2026-02 (provenance dimension)
- 1 mark node with annotation text and tags (provenance dimension)
- 1 contains edge (provenance to provenance)
- 2 references edges (provenance to semantic, created by TagConceptBridger)

Starting from concept:travel, you can now traverse: concept <- references <- mark <- contains <- chain. The chain tells you the adapter and source. The mark tells you the specific passage. The references edge confirms which concept this evidence supports.

Starting from the chain, you can traverse: chain -> contains -> mark -> references -> concept. You see every concept this source contributed.

The graph is both ontological (what concepts exist and how they relate) and epistemological (where each concept's evidence lives).

Multi-Phase Processing

Real document processing happens in phases. Level 1 extracts file metadata. Level 2 parses structure. Level 3 applies heuristics. Level 4 runs LLM extraction. Same source, different resolutions, different timescales.

The spike tested this with two FragmentAdapter instances processing the same source:

Manual phase (manual-journal): broad tag [distributed-ai] from human review.

LLM phase (llm-extract): richer tags [distributed-ai, federated-learning, compute-economics] from automated extraction.

Each phase gets its own chain (chain:manual-journal:paper-chen-2025 and chain:llm-extract:paper-chen-2025) and its own marks. Both chains point to the same source but record different resolutions of understanding.

The Hebbian design handles this naturally. Each adapter's tagged_with edges carry their own contribution slot. concept:distributed-ai has two edges — one with manual-journal contribution 1.0, one with llm-extract contribution 1.0. The weight reflects independent confirmation from two processing phases.

The provenance trail explains the weight. If you ask "why does concept:distributed-ai have high confidence?", you can traverse to two marks from two chains, both pointing to the same source paper. The manual mark confirms human review. The LLM mark confirms automated extraction. Independent verification from the same source, visible in the graph structure.

What Changed in the Existing Tests

The two-consumer test from Essay 11 now shows richer connectivity. Each of the 6 Trellis fragments produces a provenance mark alongside its semantic output. TagConceptBridger bridges all marks — both the 6 fragment marks and the 4 Carrel provenance marks — to concept nodes.

The numbers: contains edges grew from 4 to 10 (6 fragment + 4 provenance). references edges grew from 8 to 23 (15 fragment-mark bridges + 8 provenance-mark bridges). All other assertions — 7 concepts, 6 fragments, 15 tagged_with, 2 links_to — remain unchanged.

The cross-dimensional traversal still works and is now richer. Starting from research-mark-1, depth-2 traversal reaches fragment marks (via shared concepts) in addition to fragment nodes and other provenance marks. The graph's connective tissue grew denser without any new enrichment logic.

The Pipeline Fan-Out Observation

One implementation detail emerged from the spike. When two FragmentAdapter instances with different adapter IDs but the same input_kind are registered in one pipeline, both process every ingest call. This is the correct fan-out behavior for heterogeneous adapters (e.g., fragment + provenance in Essay 11), but incorrect for multi-phase processing where each phase should run independently.

The solution in the spike was separate pipelines sharing the same engine — each pipeline with one adapter. In production, multi-phase processing would likely use distinct input kinds or phase-specific routing. This is a design decision for the future, not a current limitation.

What This Means

Provenance is not a separate concern from semantics. It is the evidential backbone that makes semantics trustworthy. Every concept node exists because some source produced it. The provenance trail records what that source was, when it was processed, and which adapter extracted the knowledge.

The Hebbian design is strengthened, not complicated. Contribution weights become explainable. Cross-phase reinforcement becomes visible. Independent verification from multiple processing stages creates higher-confidence knowledge with a transparent audit trail.

TagConceptBridger — designed for user-created provenance marks — works identically for adapter-created marks. No enrichment changes were needed. The design anticipated this use case without knowing it.

248 tests pass. The epistemological infrastructure is in place.
ESSAYEOF

GLOSSARY='KNOWN DOMAIN VOCABULARY (look for these and any other concepts):
adapter, sink, emission, node, edge, contribution, raw weight, scale normalization,
normalized weight, annotation, annotation confidence, provenance entry, dimension,
content type, concept, may_be_related, evidence diversity, normalization strategy,
adapter input, context ID, cancellation token, graph event, input kind, adapter snapshot,
input router, fragment, tag, tagged_with, deterministic concept ID, deterministic chain ID,
co-occurrence, co-occurrence score, context snapshot, symmetric edge pair,
normalization floor, floor coefficient, context, PlexusEngine, GraphStore, mark, chain,
link, ProvenanceApi, cross-dimensional edge, references, tag-to-concept bridging,
tag format normalization, persist-per-emission, consumer, enrichment, enrichment loop,
quiescence, outbound event, transport, ingest, integration, event cursor,
replication layer, replication tier, ReplicatedStore, shared-concept convergence,
meta-context, extraction phase, extraction coordinator, extraction status,
declarative adapter spec, DeclarativeAdapter, adapter spec primitive, extractor,
declarative mapper, parameterized enrichment, core enrichment, external enrichment,
template expression, discovery gap, latent evidence, structural evidence,
dual obligation, Hebbian contribution, multi-phase processing, cross-dimensional traversal,
epistemology, TagConceptBridger, FragmentAdapter, EngineSink, IngestPipeline'

FEWSHOT_PROMPT="$GLOSSARY

Extract ONLY the key entities and concepts from the provided text.
Do NOT identify relationships or themes — just the things.

An entity/concept is: a named idea, technology, pattern, theory,
system, role, or domain term that the text discusses substantively.
IMPORTANT: Include multi-word compound terms when the text uses them as named concepts.

EXAMPLES of compound domain terms to look for:
- \"dual obligation\" — a design principle where adapters must produce both semantics and provenance
- \"cross-dimensional traversal\" — navigating between graph dimensions (e.g., provenance to semantic)
- \"deterministic ID\" — a stable identifier computed from content rather than generated randomly
- \"Hebbian contribution\" — per-adapter evidence tracking inspired by Hebbian learning
- \"TagConceptBridger\" — a specific component name (CamelCase)

For each concept, provide:
- label: normalized name (lowercase, 1-3 words, but preserve compound terms)
- type: one of [technology, pattern, theory, system, role, domain_term, principle]
- confidence: 0.0-1.0 based on how central this concept is to the text

Return ONLY valid JSON. Extract 8-20 concepts. Focus on domain-specific terms.
Exclude generic terms (data, information, system, process).
Return ONLY JSON, no explanation"

SCHEMA='{
  "type": "object",
  "properties": {
    "concepts": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "label": {"type": "string"},
          "type": {"type": "string", "enum": ["technology", "pattern", "theory", "system", "role", "domain_term", "principle"]},
          "confidence": {"type": "number"}
        },
        "required": ["label", "type", "confidence"]
      }
    }
  },
  "required": ["concepts"]
}'

MODEL="mistral:7b"

echo "=== Condition E: Glossary + Structured + Few-shot ==="

PAYLOAD=$(jq -n \
    --arg model "$MODEL" \
    --arg system "$FEWSHOT_PROMPT" \
    --arg user "$ESSAY_TEXT" \
    --argjson format "$SCHEMA" \
    '{
        model: $model,
        messages: [
            {role: "system", content: $system},
            {role: "user", content: $user}
        ],
        stream: false,
        format: $format,
        options: {
            num_ctx: 8192,
            temperature: 0.15
        }
    }')

RESPONSE=$(curl -s http://localhost:11434/api/chat -d "$PAYLOAD")
echo "$RESPONSE" > "$RESULTS_DIR/E-fewshot-primed-structured.json"

TOTAL_NS=$(echo "$RESPONSE" | jq -r '.total_duration // 0')
EVAL_COUNT=$(echo "$RESPONSE" | jq -r '.eval_count // 0')
CONTENT=$(echo "$RESPONSE" | jq -r '.message.content')
CONTENT_LEN=${#CONTENT}
TOTAL_S=$(echo "scale=1; $TOTAL_NS / 1000000000" | bc)

echo "  Total: ${TOTAL_S}s | Gen tokens: $EVAL_COUNT | Content: $CONTENT_LEN chars"
echo ""
echo "Entities:"
echo "$CONTENT" | jq '.concepts[].label'
