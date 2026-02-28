#!/bin/bash
# Test Qwen3:14b with Ollama structured output (JSON schema constraint)
# Compare against the baseline (no format constraint)

OLLAMA_URL="http://localhost:11434/api/chat"
MODEL="qwen3:14b"
RESULTS_DIR="$(dirname "$0")/results"

SYSTEM_PROMPT='Extract relationships between the following known entities from the provided text.

KNOWN ENTITIES:
opacity problem, cognitive load, working memory, external structural representation,
situation awareness, epistemic action, cognitive offloading, vibe-coding,
material disengagement, knowledge graph, AI-assisted composition,
computational offloading, re-representation

For each relationship found, provide source, target, relationship type, and evidence.
Only create relationships between entities from the KNOWN ENTITIES list.
Only include relationships with clear textual evidence.
/no_think'

# JSON schema for structured output
FORMAT_SCHEMA='{
  "type": "object",
  "properties": {
    "relationships": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "source": {"type": "string"},
          "target": {"type": "string"},
          "relationship": {"type": "string", "enum": ["caused_by","remedies","exemplifies","describes","mechanism_of","distinct_from","eroded_by","accelerates","instance_of","component_of","enables","constrains","produces","requires"]},
          "evidence": {"type": "string"}
        },
        "required": ["source", "target", "relationship", "evidence"]
      }
    }
  },
  "required": ["relationships"]
}'

ESSAY_TEXT='## The Problem

Knowledge accumulates faster than understanding. A developer "vibe-coding" with an AI assistant produces working software but may not fully grasp the architectural decisions embedded in the generated code. A researcher'\''s personal knowledge base grows to thousands of notes whose interconnections are invisible. A team'\''s documentation sprawls across wikis, repos, and chat histories with no unified structural map. In each case, knowledge exists but cognitive context — the awareness of what you know, how it connects, and where the gaps are — erodes.

This is not a storage problem. The documents exist. The code compiles. The notes are searchable by keyword. The problem is structural: there is no live representation of the semantic relationships within and across these artifacts. The knowledge is there but opaque to the person who ostensibly possesses it.

We call this the opacity problem: the condition where a creator'\''s artifacts contain more structure than the creator can perceive.

## The Cognitive Mechanism

The opacity problem has a cognitive mechanism grounded in established research. Working memory is sharply limited — Sweller'\''s cognitive load theory formalizes this constraint: information that is not organized into existing schemas imposes extraneous cognitive load, consuming capacity that could be directed at the task itself. When information density outpaces comprehension, the excess becomes invisible. Not lost — invisible. The structure is in the artifact but not in the creator'\''s head.

Kirschner et al. formalize this through collaborative cognitive load theory: when task complexity exceeds individual cognitive capacity, the load must be distributed — across collaborators, across time, or across external representations. The opacity problem is what happens when none of these distribution mechanisms are available.

This is domain-general. Any process that generates structure faster than the creator can track produces opacity.

## AI Makes It Acute

The opacity problem is not unique to AI-assisted work — but AI makes it acute by accelerating production while adding an interruption-heavy interaction pattern.

Consider "vibe-coding": a developer prompts an AI to generate modules iteratively, accepting code that works without fully comprehending how it works. After a dozen exchanges, the codebase has architectural decisions the developer didn'\''t make, dependency patterns they didn'\''t design, and structural implications they never evaluated.

Cito and Bork describe this as "material disengagement" — developers orchestrate code generation without comprehending the output.

Beyond the volume problem, the interaction pattern itself is costly. Mark et al. demonstrate that interrupted work exacts significant cognitive costs. The prompt-wait-evaluate cycle of AI-assisted composition is an interruption factory.

## Why Better Prompting Won'\''t Fix It

The instinctive response is to make the AI interaction better. These help at the interaction level but don'\''t address the structural problem. Even a perfect AI interaction still produces artifacts whose accumulated structure exceeds what the developer can hold in working memory. The opacity problem is about information density, not interaction quality.

The same applies to slower generation. Deliberate, careful, human-only composition also produces opacity — it just takes longer to get there.

## The Remedy: External Structural Representations

The remedy is not better prompting, slower generation, or more documentation. It is structural. The creator needs an external representation of the relationships their work contains.

Endsley defines situation awareness as the perception of elements in the environment, comprehension of their meaning, and projection of their future state — precisely what erodes in information-dense composition.

Kirsh and Maglio distinguish epistemic actions (changing the agent'\''s computational state to make mental computation easier) from pragmatic actions (changing the world toward a goal). An external structural representation is epistemic.

Scaife and Rogers identify three specific mechanisms: computational offloading (reducing working memory demands), re-representation (presenting information in a form better suited to the task), and graphical constraining (limiting the space of possible inferences).

Larkin and Simon demonstrate that diagrams reduce cognitive load by making information explicit that would otherwise require search and inference.

A live structural representation that evolves alongside the creative process could provide ongoing situation awareness.

## A Caveat on Cognitive Offloading

External structural representations are not unambiguously beneficial. Gerlich finds a significant negative correlation between frequent AI tool usage and critical thinking abilities, mediated by increased cognitive offloading. Klein and Klein introduce the "extended hollowed mind" framework.

This critique targets tools that outsource reasoning. A structural tool that externalizes awareness (what connects to what) while preserving interpretation (what it means) may avoid this failure mode. The graph shows that three fragments share a concept; the creator decides whether that matters.'

run_test() {
    local name="$1"
    local format_arg="$2"
    local outfile="$RESULTS_DIR/$name.json"

    echo "=== $name ==="

    local escaped_text escaped_system
    escaped_text=$(python3 -c "import json,sys; print(json.dumps(sys.stdin.read()))" <<< "$ESSAY_TEXT")
    escaped_system=$(python3 -c "import json,sys; print(json.dumps(sys.stdin.read()))" <<< "$SYSTEM_PROMPT")

    local request_body
    if [ -n "$format_arg" ]; then
        request_body="{
            \"model\": \"$MODEL\",
            \"messages\": [
                {\"role\": \"system\", \"content\": $escaped_system},
                {\"role\": \"user\", \"content\": $escaped_text}
            ],
            \"stream\": false,
            \"format\": $format_arg,
            \"options\": {\"num_ctx\": 8192, \"temperature\": 0.3}
        }"
    else
        request_body="{
            \"model\": \"$MODEL\",
            \"messages\": [
                {\"role\": \"system\", \"content\": $escaped_system},
                {\"role\": \"user\", \"content\": $escaped_text}
            ],
            \"stream\": false,
            \"options\": {\"num_ctx\": 8192, \"temperature\": 0.3}
        }"
    fi

    curl -s "$OLLAMA_URL" -d "$request_body" > "$outfile"

    python3 -c "
import json
with open('$outfile') as f:
    d = json.load(f)
content = d.get('message', {}).get('content', '')
total_s = d.get('total_duration', 0) / 1e9
load_s = d.get('load_duration', 0) / 1e9
prompt_s = d.get('prompt_eval_duration', 0) / 1e9
eval_s = d.get('eval_duration', 0) / 1e9
prompt_tok = d.get('prompt_eval_count', 0)
eval_tok = d.get('eval_count', 0)
cpt = len(content) / eval_tok if eval_tok else 0
print(f'  Time:   {total_s:.1f}s total (load: {load_s:.1f}s, prompt: {prompt_s:.1f}s, gen: {eval_s:.1f}s)')
print(f'  Tokens: {prompt_tok} prompt, {eval_tok} generated')
print(f'  Output: {len(content)} chars, {cpt:.2f} chars/token')
import re
match = re.search(r'\{[\s\S]*\}', content)
if match:
    try:
        parsed = json.loads(match.group())
        rels = parsed.get('relationships', [])
        print(f'  Rels:   {len(rels)} extracted')
        for r in rels:
            print(f'          {r[\"source\"]} --{r[\"relationship\"]}--> {r[\"target\"]}')
    except: print(f'  JSON parse failed')
else: print(f'  No JSON found')
"
    echo ""
}

echo "========================================="
echo "Structured Output Comparison"
echo "Model: $MODEL | Essay 02"
echo "========================================="
echo ""

run_test "structured-full" "$FORMAT_SCHEMA"
run_test "structured-section5" ""  # we'll override below

# Also test a single section WITH structured output to see if thinking leak is fixed
SEC5='## The Remedy: External Structural Representations

The remedy is not better prompting, slower generation, or more documentation. It is structural. The creator needs an external representation of the relationships their work contains.

Endsley defines situation awareness as the perception of elements in the environment, comprehension of their meaning, and projection of their future state — precisely what erodes in information-dense composition.

Kirsh and Maglio distinguish epistemic actions (changing the agent'\''s computational state to make mental computation easier) from pragmatic actions (changing the world toward a goal). An external structural representation is epistemic.

Scaife and Rogers identify three specific mechanisms: computational offloading (reducing working memory demands), re-representation (presenting information in a form better suited to the task), and graphical constraining (limiting the space of possible inferences).

Larkin and Simon demonstrate that diagrams reduce cognitive load by making information explicit that would otherwise require search and inference.

A live structural representation that evolves alongside the creative process could provide ongoing situation awareness.'

# Section test with structured output
escaped_sec5=$(python3 -c "import json,sys; print(json.dumps(sys.stdin.read()))" <<< "$SEC5")
escaped_sys=$(python3 -c "import json,sys; print(json.dumps(sys.stdin.read()))" <<< "$SYSTEM_PROMPT")

echo "=== structured-section5 (with schema) ==="
curl -s "$OLLAMA_URL" -d "{
    \"model\": \"$MODEL\",
    \"messages\": [
        {\"role\": \"system\", \"content\": $escaped_sys},
        {\"role\": \"user\", \"content\": $escaped_sec5}
    ],
    \"stream\": false,
    \"format\": $FORMAT_SCHEMA,
    \"options\": {\"num_ctx\": 8192, \"temperature\": 0.3}
}" > "$RESULTS_DIR/structured-section5.json"

python3 -c "
import json
with open('$RESULTS_DIR/structured-section5.json') as f:
    d = json.load(f)
content = d.get('message', {}).get('content', '')
total_s = d.get('total_duration', 0) / 1e9
load_s = d.get('load_duration', 0) / 1e9
prompt_s = d.get('prompt_eval_duration', 0) / 1e9
eval_s = d.get('eval_duration', 0) / 1e9
prompt_tok = d.get('prompt_eval_count', 0)
eval_tok = d.get('eval_count', 0)
cpt = len(content) / eval_tok if eval_tok else 0
print(f'  Time:   {total_s:.1f}s total (load: {load_s:.1f}s, prompt: {prompt_s:.1f}s, gen: {eval_s:.1f}s)')
print(f'  Tokens: {prompt_tok} prompt, {eval_tok} generated')
print(f'  Output: {len(content)} chars, {cpt:.2f} chars/token')
import re
match = re.search(r'\{[\s\S]*\}', content)
if match:
    try:
        parsed = json.loads(match.group())
        rels = parsed.get('relationships', [])
        print(f'  Rels:   {len(rels)} extracted')
        for r in rels:
            print(f'          {r[\"source\"]} --{r[\"relationship\"]}--> {r[\"target\"]}')
    except: print(f'  JSON parse failed')
else: print(f'  No JSON found')
"
echo ""

echo "========================================="
echo "Comparison reference (from earlier run):"
echo "  Unconstrained full:    125.4s, 1379 gen tokens, 1.72 chars/tok"
echo "  Unconstrained sec5:    184.5s, 2312 gen tokens, 0.44 chars/tok"
echo "========================================="
