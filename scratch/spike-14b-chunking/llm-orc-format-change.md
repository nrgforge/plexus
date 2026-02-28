# llm-orc: Add Ollama Structured Output Support

## Problem

Ollama supports constrained decoding via a `format` parameter on the `/api/chat` endpoint. When passed a JSON schema, the model is physically prevented from generating tokens that don't conform to the schema. This eliminates hallucinated output structure, guarantees valid JSON, and — critically — suppresses Qwen3's hidden thinking tokens, which inflate generation time by 3-7x on short inputs.

llm-orc currently never passes the `format` parameter to Ollama. The existing `output_format` field on agent configs controls CLI display rendering, not model output.

## Evidence

Tested on Essay 02 relationship extraction (Plexus spike Q0):

| Model | Without `format` | With `format` schema |
|-------|------------------|---------------------|
| Qwen3:14b | 125s, 10 rels, 1.72 chars/tok | 149s, 14 rels, 4.66 chars/tok |
| Mistral:7b | 50s, no JSON output | 50s, 15 rels, 3.87 chars/tok |
| Qwen3:8b section | 184s, 4 rels, 0.44 chars/tok | 148s, 5 rels, 4.70 chars/tok |

The schema constraint eliminates wasted tokens (every token is productive JSON), improves extraction quality (more relationships found), and for Qwen3 models fixes a thinking-token leak where `/no_think` fails on short inputs.

## Change

Thread a new `format` parameter from agent config through the model factory to `OllamaModel.generate_response()`, which passes it to `ollama.AsyncClient.chat(format=...)`.

### YAML surface

New field `ollama_format` on agent config (distinct from the existing `output_format` which controls CLI display):

```yaml
agents:
  - name: relationship-extractor
    model_profile: analyst-mistral
    ollama_format:
      type: object
      properties:
        relationships:
          type: array
          items:
            type: object
            properties:
              source: { type: string }
              target: { type: string }
              relationship:
                type: string
                enum: [caused_by, remedies, exemplifies, describes,
                       mechanism_of, distinct_from, eroded_by, accelerates,
                       instance_of, component_of, enables, constrains,
                       produces, requires]
              evidence: { type: string }
            required: [source, target, relationship, evidence]
      required: [relationships]
```

Also accepts the string `"json"` for unstructured JSON mode (Ollama returns valid JSON but unconstrained by a schema).

### Code path

```
YAML agent config
  → LlmAgentConfig.ollama_format: str | dict[str, Any] | None     [agent_config.py]
  → agent_config.model_dump()                                       [llm_runner.py — no change, auto-serialized]
  → ModelFactory.load_model_from_agent_config()                     [model_factory.py]
       extracts ollama_format, forwards to load_model()
  → ModelFactory.load_model()                                       [model_factory.py]
       forwards to _handle_no_authentication()
  → _handle_no_authentication()                                     [model_factory.py]
       passes to OllamaModel(format=...)
  → OllamaModel.__init__(format=...)                                [ollama.py]
       stores as self._format
  → OllamaModel.generate_response()                                [ollama.py]
       passes self._format to self.client.chat(format=...)
  → Ollama API enforces constrained decoding
```

### Files touched

| File | Change |
|------|--------|
| `schemas/agent_config.py` | Add `ollama_format: str \| dict[str, Any] \| None = None` to `LlmAgentConfig` |
| `models/ollama.py` | Add `format` param to `__init__`, store as `self._format`, pass to `client.chat()` |
| `core/models/model_factory.py` | Extract `ollama_format` from config dict, thread through `load_model()` → `_handle_no_authentication()` → `OllamaModel()` |

No changes to runner, CLI, cloud providers, or base model interface. ~8-10 new lines total.

### Non-goals

- Profile-level format (only agent-level — the schema is task-specific, not model-specific)
- Non-Ollama providers (Anthropic/Google have different structured output APIs)
- Removing or changing the existing `output_format` field (it serves a different purpose)
