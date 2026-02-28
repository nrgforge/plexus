"""Condition G2: Phase 2 primed LLM relationship extraction with token limit."""
import json
import urllib.request
import time

# Read essay
with open("docs/essays/12-provenance-as-epistemological-infrastructure.md") as f:
    essay = f.read()

system_prompt = """KNOWN ENTITIES:
provenance, epistemology, chain, mark, fragment, TagConceptBridger,
Hebbian contribution, adapter, enrichment, multi-phase processing,
concept, tag, tagged_with, references, contribution, sink,
EngineSink, FragmentAdapter, ProvenanceAdapter, Plexus, graph,
pipeline, ingest, dimension, node, edge, cross-dimensional traversal,
deterministic ID

RELATIONSHIPS DETECTED BY SYNTACTIC ANALYSIS (Phase 2, deterministic):
- adapter produces provenance
- fragment produces provenance mark
- provenance mark produce tag
- concept node have provenance trail
- adapter produce provenance trail
- ingest upsert chain
- fragment get mark
- fragment get chain
- enrichment create references edges
- enrichment create concept node
- graph produce hebbian contribution
- chain tell adapter
- tagged_with carry contribution
- concept have contribution
- fragmentadapter register pipeline

Validate, refine, and extend these relationships. Some may be artifacts of syntactic parsing.
Add relationships the syntactic analysis missed — especially abstract or argumentative relationships.

Extract relationships between the known entities from the provided text.
For each relationship, provide source, target, relationship type, and evidence.
Only include relationships with clear textual evidence.
Extract ALL relationships visible in the text."""

schema = {
    "type": "object",
    "properties": {
        "relationships": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "source": {"type": "string"},
                    "target": {"type": "string"},
                    "relationship": {
                        "type": "string",
                        "enum": ["contains", "produces", "creates", "bridges", "provides",
                                 "tracks", "uses", "references", "carries", "enables",
                                 "requires", "instance_of", "component_of", "describes",
                                 "caused_by", "distinct_from", "mechanism_of", "exemplifies",
                                 "may_be_related"]
                    },
                    "evidence": {"type": "string"}
                },
                "required": ["source", "target", "relationship", "evidence"]
            }
        }
    },
    "required": ["relationships"]
}

payload = {
    "model": "mistral:7b",
    "messages": [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": essay}
    ],
    "stream": False,
    "format": schema,
    "options": {
        "num_ctx": 8192,
        "temperature": 0.15,
        "num_predict": 2048
    }
}

print("Running G2 (Phase 2 primed, max 2048 tokens)...")
start = time.time()
req = urllib.request.Request(
    "http://localhost:11434/api/chat",
    data=json.dumps(payload).encode(),
    headers={"Content-Type": "application/json"}
)
resp = urllib.request.urlopen(req, timeout=300)
result = json.loads(resp.read())
elapsed = time.time() - start

# Save
with open("scratch/spike-vocab-priming/results/G2-phase2-primed-limited.json", "w") as f:
    json.dump(result, f, indent=2)

total_s = result.get("total_duration", 0) / 1e9
eval_count = result.get("eval_count", 0)
content = result["message"]["content"]

try:
    rels = json.loads(content)["relationships"]
except:
    print(f"Failed to parse JSON response ({len(content)} chars)")
    print(content[:500])
    exit(1)

print(f"Total: {total_s:.1f}s | Gen tokens: {eval_count} | Relationships: {len(rels)}")
print()
for r in rels:
    print(f"  {r['source']} --{r['relationship']}--> {r['target']}")
    print(f"    evidence: {r.get('evidence', 'N/A')[:80]}")
