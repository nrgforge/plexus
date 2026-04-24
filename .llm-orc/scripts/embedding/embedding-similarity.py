"""Embedding-based pairwise similarity.

Computes embeddings for a batch of documents via Ollama's /api/embeddings
endpoint, then returns pairs whose cosine similarity exceeds a configured
threshold. No LLM agents — pure script agent.

Used by the worked-example spec at examples/specs/embedding-activation.yaml
to activate `similar_to` edge production in the default Homebrew build
via the declarative adapter spec's `ensemble:` path (ADR-038).

Prerequisites:
    - Ollama running locally (default: http://localhost:11434)
    - Embedding model pulled (default: nomic-embed-text)

Configuration via environment variables:
    OLLAMA_HOST        default http://localhost:11434
    OLLAMA_MODEL       default nomic-embed-text
    SIMILARITY_MIN     default 0.72 (cosine similarity threshold)
    MAX_WORDS_PER_DOC  default 1200 (truncate inputs longer than this —
                       nomic-embed-text has a 2048-token context and
                       English averages ~1.3 tokens/word; 1200 words keeps
                       well under the limit with headroom for punctuation
                       and frontmatter. Raise for models with larger
                       context; lower if you hit 500s.)

Input (JSON on stdin, via llm-orc's "input" field):
    {
      "docs": [
        {"id": "concept:foo", "text": "..."},
        {"id": "concept:bar", "text": "..."}
      ]
    }

Output (JSON on stdout):
    {
      "pairs": [
        {"source": "concept:foo", "target": "concept:bar", "similarity": 0.83}
      ],
      "status": "success"
    }

Failure mode: returns {"pairs": [], "status": "error", "error": "..."}
so that the spec's for_each over ensemble.pairs degrades to a no-op
rather than aborting the ingest.
"""

import json
import math
import os
import sys
import urllib.error
import urllib.request

DEFAULT_HOST = os.environ.get("OLLAMA_HOST", "http://localhost:11434")
DEFAULT_MODEL = os.environ.get("OLLAMA_MODEL", "nomic-embed-text")
DEFAULT_THRESHOLD = float(os.environ.get("SIMILARITY_MIN", "0.72"))
MAX_WORDS = int(os.environ.get("MAX_WORDS_PER_DOC", "1200"))
EMBED_TIMEOUT = 30.0


def truncate(text: str, max_words: int) -> str:
    """Truncate to max_words, preserving the opening. Embedding context
    is finite; the opening carries more thematic weight than the closing
    in most genres. If the model's context is larger, raise MAX_WORDS_PER_DOC."""
    words = text.split()
    if len(words) <= max_words:
        return text
    return " ".join(words[:max_words])


def embed(text: str, host: str, model: str) -> list[float]:
    """Call Ollama's /api/embeddings for a single document."""
    payload = json.dumps({"model": model, "prompt": text}).encode("utf-8")
    req = urllib.request.Request(
        f"{host.rstrip('/')}/api/embeddings",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=EMBED_TIMEOUT) as resp:
        body = json.loads(resp.read().decode("utf-8"))
    vec = body.get("embedding")
    if not isinstance(vec, list) or not vec:
        raise RuntimeError(f"Ollama returned no embedding for input (len={len(text)})")
    return vec


def cosine(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(y * y for y in b))
    if na == 0.0 or nb == 0.0:
        return 0.0
    return dot / (na * nb)


def main() -> None:
    raw = sys.stdin.read()
    try:
        payload = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError as e:
        print(json.dumps({"pairs": [], "status": "error", "error": f"invalid JSON input: {e}"}))
        return

    # llm-orc wraps stdin as {"input": "..."} where the value is a JSON string
    # (the spec's input serialized). Standalone invocation may pass {"docs": [...]}
    # directly. Handle both shapes.
    inner = payload
    if isinstance(payload.get("input"), str):
        try:
            inner = json.loads(payload["input"])
        except json.JSONDecodeError:
            inner = {}
    elif isinstance(payload.get("input"), dict):
        inner = payload["input"]

    docs = inner.get("docs", []) if isinstance(inner, dict) else []
    if not isinstance(docs, list) or len(docs) < 2:
        print(json.dumps({"pairs": [], "status": "success"}))
        return

    host = DEFAULT_HOST
    model = DEFAULT_MODEL
    threshold = DEFAULT_THRESHOLD

    # Embed each doc (sequential — parallelism is Ollama's concern, not ours).
    vectors: list[tuple[str, list[float]]] = []
    for doc in docs:
        if not isinstance(doc, dict):
            continue
        doc_id = doc.get("id")
        text = doc.get("text", "")
        if not doc_id or not isinstance(text, str) or not text.strip():
            continue
        try:
            vec = embed(truncate(text, MAX_WORDS), host, model)
        except (urllib.error.URLError, RuntimeError, TimeoutError) as e:
            print(json.dumps({
                "pairs": [],
                "status": "error",
                "error": f"embedding failed for {doc_id}: {e}",
            }))
            return
        vectors.append((doc_id, vec))

    # Pairwise similarity above threshold. Symmetric pairs produced for both
    # directions so lens translation and graph traversal see the relationship
    # from either endpoint (Invariant 22-style symmetry).
    pairs: list[dict] = []
    for i in range(len(vectors)):
        id_i, vec_i = vectors[i]
        for j in range(i + 1, len(vectors)):
            id_j, vec_j = vectors[j]
            sim = cosine(vec_i, vec_j)
            if sim >= threshold:
                pairs.append({"source": id_i, "target": id_j, "similarity": round(sim, 4)})
                pairs.append({"source": id_j, "target": id_i, "similarity": round(sim, 4)})

    print(json.dumps({"pairs": pairs, "status": "success", "count": len(pairs) // 2}))


if __name__ == "__main__":
    main()
