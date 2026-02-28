#!/bin/bash
# Spike Q0: Does chunking make the 14B model viable for relationship extraction?
# Uses Ollama API directly for precise timing data.

OLLAMA_URL="http://localhost:11434/api/chat"
MODEL="qwen3:14b"
RESULTS_DIR="$(dirname "$0")/results"
mkdir -p "$RESULTS_DIR"

SYSTEM_PROMPT='Extract relationships between the following known entities from the provided text.

KNOWN ENTITIES:
opacity problem, cognitive load, working memory, external structural representation,
situation awareness, epistemic action, cognitive offloading, vibe-coding,
material disengagement, knowledge graph, AI-assisted composition,
computational offloading, re-representation

For each relationship, provide:
- source: entity label from the list above
- target: entity label from the list above
- relationship: one of [caused_by, remedies, exemplifies, describes, mechanism_of, distinct_from, eroded_by, accelerates, instance_of, component_of, enables, constrains, produces, requires]
- evidence: brief quote or paraphrase from the text

Return ONLY valid JSON:
{
  "relationships": [
    {"source": "concept a", "target": "concept b", "relationship": "caused_by", "evidence": "..."}
  ]
}

Guidelines:
- Only create relationships between entities from the KNOWN ENTITIES list
- Only include relationships with clear textual evidence
- Extract ALL relationships visible in the text
- Return ONLY JSON, no explanation
/no_think'

run_extraction() {
    local name="$1"
    local text="$2"
    local outfile="$RESULTS_DIR/$name.json"

    echo "=== Running: $name ==="
    echo "Input length: $(echo "$text" | wc -c) chars"

    # Escape text for JSON
    local escaped_text
    escaped_text=$(python3 -c "import json,sys; print(json.dumps(sys.stdin.read()))" <<< "$text")
    local escaped_system
    escaped_system=$(python3 -c "import json,sys; print(json.dumps(sys.stdin.read()))" <<< "$SYSTEM_PROMPT")

    curl -s "$OLLAMA_URL" \
        -d "{
            \"model\": \"$MODEL\",
            \"messages\": [
                {\"role\": \"system\", \"content\": $escaped_system},
                {\"role\": \"user\", \"content\": $escaped_text}
            ],
            \"stream\": false,
            \"options\": {
                \"num_ctx\": 8192,
                \"temperature\": 0.3
            }
        }" > "$outfile"

    # Extract timing
    local total_ns=$(python3 -c "import json; d=json.load(open('$outfile')); print(d.get('total_duration', 0))")
    local load_ns=$(python3 -c "import json; d=json.load(open('$outfile')); print(d.get('load_duration', 0))")
    local prompt_tokens=$(python3 -c "import json; d=json.load(open('$outfile')); print(d.get('prompt_eval_count', 0))")
    local eval_tokens=$(python3 -c "import json; d=json.load(open('$outfile')); print(d.get('eval_count', 0))")
    local prompt_ns=$(python3 -c "import json; d=json.load(open('$outfile')); print(d.get('prompt_eval_duration', 0))")
    local eval_ns=$(python3 -c "import json; d=json.load(open('$outfile')); print(d.get('eval_duration', 0))")

    local total_s=$(python3 -c "print(f'{$total_ns / 1e9:.1f}')")
    local load_s=$(python3 -c "print(f'{$load_ns / 1e9:.1f}')")
    local prompt_s=$(python3 -c "print(f'{$prompt_ns / 1e9:.1f}')")
    local eval_s=$(python3 -c "print(f'{$eval_ns / 1e9:.1f}')")

    echo "  Total: ${total_s}s (load: ${load_s}s, prompt: ${prompt_s}s, gen: ${eval_s}s)"
    echo "  Tokens: ${prompt_tokens} prompt, ${eval_tokens} generated"
    echo ""
}

echo "========================================="
echo "Spike Q0: 14B Chunking Experiment"
echo "Model: $MODEL"
echo "Essay: 02 — The Opacity Problem"
echo "========================================="
echo ""

# --- BASELINE: Full essay text ---
FULL_TEXT=$(cat << 'ESSAYEOF'
## The Problem

Knowledge accumulates faster than understanding. A developer "vibe-coding" with an AI assistant produces working software but may not fully grasp the architectural decisions embedded in the generated code. A researcher's personal knowledge base grows to thousands of notes whose interconnections are invisible. A team's documentation sprawls across wikis, repos, and chat histories with no unified structural map. In each case, knowledge exists but cognitive context — the awareness of what you know, how it connects, and where the gaps are — erodes.

This is not a storage problem. The documents exist. The code compiles. The notes are searchable by keyword. The problem is structural: there is no live representation of the semantic relationships within and across these artifacts. The knowledge is there but opaque to the person who ostensibly possesses it.

We call this the opacity problem: the condition where a creator's artifacts contain more structure than the creator can perceive.

## The Cognitive Mechanism

The opacity problem has a cognitive mechanism grounded in established research. Working memory is sharply limited — Sweller's cognitive load theory formalizes this constraint: information that is not organized into existing schemas imposes extraneous cognitive load, consuming capacity that could be directed at the task itself. When information density outpaces comprehension, the excess becomes invisible. Not lost — invisible. The structure is in the artifact but not in the creator's head.

Kirschner et al. formalize this through collaborative cognitive load theory: when task complexity exceeds individual cognitive capacity, the load must be distributed — across collaborators, across time, or across external representations. The opacity problem is what happens when none of these distribution mechanisms are available.

This is domain-general. Any process that generates structure faster than the creator can track produces opacity.

## AI Makes It Acute

The opacity problem is not unique to AI-assisted work — but AI makes it acute by accelerating production while adding an interruption-heavy interaction pattern.

Consider "vibe-coding": a developer prompts an AI to generate modules iteratively, accepting code that works without fully comprehending how it works. After a dozen exchanges, the codebase has architectural decisions the developer didn't make, dependency patterns they didn't design, and structural implications they never evaluated.

Cito and Bork describe this as "material disengagement" — developers orchestrate code generation without comprehending the output.

Beyond the volume problem, the interaction pattern itself is costly. Mark et al. demonstrate that interrupted work exacts significant cognitive costs. The prompt-wait-evaluate cycle of AI-assisted composition is an interruption factory.

## Why Better Prompting Won't Fix It

The instinctive response is to make the AI interaction better. These help at the interaction level but don't address the structural problem. Even a perfect AI interaction still produces artifacts whose accumulated structure exceeds what the developer can hold in working memory. The opacity problem is about information density, not interaction quality.

The same applies to slower generation. Deliberate, careful, human-only composition also produces opacity — it just takes longer to get there.

## The Remedy: External Structural Representations

The remedy is not better prompting, slower generation, or more documentation. It is structural. The creator needs an external representation of the relationships their work contains.

Endsley defines situation awareness as the perception of elements in the environment, comprehension of their meaning, and projection of their future state — precisely what erodes in information-dense composition.

Kirsh and Maglio distinguish epistemic actions (changing the agent's computational state to make mental computation easier) from pragmatic actions (changing the world toward a goal). An external structural representation is epistemic.

Scaife and Rogers identify three specific mechanisms: computational offloading (reducing working memory demands), re-representation (presenting information in a form better suited to the task), and graphical constraining (limiting the space of possible inferences).

Larkin and Simon demonstrate that diagrams reduce cognitive load by making information explicit that would otherwise require search and inference.

A live structural representation that evolves alongside the creative process could provide ongoing situation awareness.

## A Caveat on Cognitive Offloading

External structural representations are not unambiguously beneficial. Gerlich finds a significant negative correlation between frequent AI tool usage and critical thinking abilities, mediated by increased cognitive offloading. Klein and Klein introduce the "extended hollowed mind" framework.

This critique targets tools that outsource reasoning. A structural tool that externalizes awareness (what connects to what) while preserving interpretation (what it means) may avoid this failure mode. The graph shows that three fragments share a concept; the creator decides whether that matters.

## Open Questions

What form should the representation take? A knowledge graph is one option.
Should the representation be ambient or on-demand?
Does continuous structural awareness help or hinder?
Is the opacity problem evenly distributed?
ESSAYEOF
)

run_extraction "baseline-full" "$FULL_TEXT"

# --- CHUNKED: Individual sections ---

SEC1="## The Problem

Knowledge accumulates faster than understanding. A developer \"vibe-coding\" with an AI assistant produces working software but may not fully grasp the architectural decisions embedded in the generated code. A researcher's personal knowledge base grows to thousands of notes whose interconnections are invisible. A team's documentation sprawls across wikis, repos, and chat histories with no unified structural map. In each case, knowledge exists but cognitive context — the awareness of what you know, how it connects, and where the gaps are — erodes.

This is not a storage problem. The documents exist. The code compiles. The notes are searchable by keyword. The problem is structural: there is no live representation of the semantic relationships within and across these artifacts. The knowledge is there but opaque to the person who ostensibly possesses it.

We call this the opacity problem: the condition where a creator's artifacts contain more structure than the creator can perceive."

SEC2="## The Cognitive Mechanism

The opacity problem has a cognitive mechanism grounded in established research. Working memory is sharply limited — Sweller's cognitive load theory formalizes this constraint: information that is not organized into existing schemas imposes extraneous cognitive load, consuming capacity that could be directed at the task itself. When information density outpaces comprehension, the excess becomes invisible. Not lost — invisible. The structure is in the artifact but not in the creator's head.

Kirschner et al. formalize this through collaborative cognitive load theory: when task complexity exceeds individual cognitive capacity, the load must be distributed — across collaborators, across time, or across external representations. The opacity problem is what happens when none of these distribution mechanisms are available.

This is domain-general. Any process that generates structure faster than the creator can track produces opacity."

SEC3="## AI Makes It Acute

The opacity problem is not unique to AI-assisted work — but AI makes it acute by accelerating production while adding an interruption-heavy interaction pattern.

Consider \"vibe-coding\": a developer prompts an AI to generate modules iteratively, accepting code that works without fully comprehending how it works. After a dozen exchanges, the codebase has architectural decisions the developer didn't make, dependency patterns they didn't design, and structural implications they never evaluated.

Cito and Bork describe this as \"material disengagement\" — developers orchestrate code generation without comprehending the output.

Beyond the volume problem, the interaction pattern itself is costly. Mark et al. demonstrate that interrupted work exacts significant cognitive costs. The prompt-wait-evaluate cycle of AI-assisted composition is an interruption factory."

SEC4="## Why Better Prompting Won't Fix It

The instinctive response is to make the AI interaction better. These help at the interaction level but don't address the structural problem. Even a perfect AI interaction still produces artifacts whose accumulated structure exceeds what the developer can hold in working memory. The opacity problem is about information density, not interaction quality.

The same applies to slower generation. Deliberate, careful, human-only composition also produces opacity — it just takes longer to get there."

SEC5="## The Remedy: External Structural Representations

The remedy is not better prompting, slower generation, or more documentation. It is structural. The creator needs an external representation of the relationships their work contains.

Endsley defines situation awareness as the perception of elements in the environment, comprehension of their meaning, and projection of their future state — precisely what erodes in information-dense composition.

Kirsh and Maglio distinguish epistemic actions (changing the agent's computational state to make mental computation easier) from pragmatic actions (changing the world toward a goal). An external structural representation is epistemic.

Scaife and Rogers identify three specific mechanisms: computational offloading (reducing working memory demands), re-representation (presenting information in a form better suited to the task), and graphical constraining (limiting the space of possible inferences).

Larkin and Simon demonstrate that diagrams reduce cognitive load by making information explicit that would otherwise require search and inference.

A live structural representation that evolves alongside the creative process could provide ongoing situation awareness."

SEC6="## A Caveat on Cognitive Offloading

External structural representations are not unambiguously beneficial. Gerlich finds a significant negative correlation between frequent AI tool usage and critical thinking abilities, mediated by increased cognitive offloading. Klein and Klein introduce the \"extended hollowed mind\" framework.

This critique targets tools that outsource reasoning. A structural tool that externalizes awareness (what connects to what) while preserving interpretation (what it means) may avoid this failure mode. The graph shows that three fragments share a concept; the creator decides whether that matters."

run_extraction "section-1-problem" "$SEC1"
run_extraction "section-2-cognitive" "$SEC2"
run_extraction "section-3-ai-acute" "$SEC3"
run_extraction "section-4-prompting" "$SEC4"
run_extraction "section-5-remedy" "$SEC5"
run_extraction "section-6-caveat" "$SEC6"

echo "========================================="
echo "All experiments complete. Results in $RESULTS_DIR/"
echo "========================================="
