"""SpaCy NER + dependency parsing + co-occurrence extraction.

Extracts entities, typed relationships (SVO triples), and sentence
co-occurrences from text using SpaCy's en_core_web_sm model.

Prerequisite:
    pip install spacy
    python -m spacy download en_core_web_sm

Input: JSON via stdin with "input_data" containing the source text.
Output: JSON with entities, relationships, and cooccurrences.
"""

import json
import sys

try:
    import spacy
except ImportError:
    print(json.dumps({
        "success": False,
        "error": "spacy not installed. Run: pip install spacy && python -m spacy download en_core_web_sm"
    }))
    sys.exit(1)


# Entity type mapping: SpaCy NER labels → Plexus concept types
NER_TYPE_MAP = {
    "PERSON": "person",
    "ORG": "organization",
    "GPE": "location",
    "LOC": "location",
    "PRODUCT": "component",
    "WORK_OF_ART": "concept",
    "EVENT": "event",
    "LAW": "concept",
    "LANGUAGE": "concept",
    "FAC": "location",
    "NORP": "concept",
}

# Minimum token length for noun phrase entities
MIN_CHUNK_LEN = 2


def load_model():
    """Load SpaCy model, falling back gracefully."""
    try:
        return spacy.load("en_core_web_sm")
    except OSError:
        print(json.dumps({
            "success": False,
            "error": "en_core_web_sm not found. Run: python -m spacy download en_core_web_sm"
        }))
        sys.exit(1)


def extract_entities(doc):
    """Extract named entities and noun phrase chunks as concept candidates."""
    entities = {}

    # Named entities from NER
    for ent in doc.ents:
        label = ent.text.strip()
        if len(label) < MIN_CHUNK_LEN:
            continue
        key = label.lower()
        if key not in entities:
            entities[key] = {
                "label": label,
                "type": NER_TYPE_MAP.get(ent.label_, "concept"),
            }

    # Noun phrase chunks — catches compound terms like "provenance trail"
    for chunk in doc.noun_chunks:
        # Skip single-token chunks that are pronouns or determiners
        text = chunk.text.strip()
        if len(text) < MIN_CHUNK_LEN:
            continue
        # Remove leading determiners (the, a, an)
        tokens = [t for t in chunk if t.pos_ not in ("DET", "PRON")]
        if not tokens:
            continue
        clean = " ".join(t.text for t in tokens).strip()
        if len(clean) < MIN_CHUNK_LEN:
            continue
        key = clean.lower()
        if key not in entities:
            entities[key] = {
                "label": clean,
                "type": "concept",
            }

    return list(entities.values())


def extract_svo_triples(doc, entity_keys):
    """Extract subject-verb-object triples where both S and O are known entities."""
    relationships = []
    seen = set()

    for sent in doc.sents:
        for token in sent:
            if token.dep_ not in ("nsubj", "nsubjpass"):
                continue
            verb = token.head
            if verb.pos_ != "VERB":
                continue

            subject_text = _get_entity_span(token, entity_keys)
            if subject_text is None:
                continue

            # Find direct objects of this verb
            for child in verb.children:
                if child.dep_ not in ("dobj", "attr", "pobj"):
                    continue
                object_text = _get_entity_span(child, entity_keys)
                if object_text is None:
                    continue

                key = (subject_text.lower(), verb.lemma_, object_text.lower())
                if key in seen:
                    continue
                seen.add(key)

                relationships.append({
                    "source": subject_text,
                    "target": object_text,
                    "relationship": verb.lemma_,
                    "evidence": sent.text.strip()[:200],
                })

    return relationships


def _get_entity_span(token, entity_keys):
    """Check if a token (or its noun chunk) matches a known entity."""
    # Check the token's full subtree span against known entities
    text = token.text.strip()
    if text.lower() in entity_keys:
        return text

    # Check if part of a noun chunk
    if token.doc[token.i:token.i + 1].text:
        for chunk in token.doc.noun_chunks:
            if token.i >= chunk.start and token.i < chunk.end:
                # Remove determiners
                tokens = [t for t in chunk if t.pos_ not in ("DET", "PRON")]
                clean = " ".join(t.text for t in tokens).strip()
                if clean.lower() in entity_keys:
                    return clean
    return None


def extract_cooccurrences(doc, entity_keys):
    """Extract entity pairs that co-occur within the same sentence."""
    cooccurrences = []
    seen = set()

    for sent in doc.sents:
        # Collect entities in this sentence
        sent_entities = set()
        for ent in sent.ents:
            key = ent.text.strip().lower()
            if key in entity_keys:
                sent_entities.add(key)

        for chunk in sent.noun_chunks:
            tokens = [t for t in chunk if t.pos_ not in ("DET", "PRON")]
            clean = " ".join(t.text for t in tokens).strip().lower()
            if clean in entity_keys:
                sent_entities.add(clean)

        # Emit all pairs (sorted to deduplicate A,B vs B,A)
        entities_list = sorted(sent_entities)
        for i in range(len(entities_list)):
            for j in range(i + 1, len(entities_list)):
                pair = (entities_list[i], entities_list[j])
                if pair not in seen:
                    seen.add(pair)
                    cooccurrences.append({
                        "entity_a": entities_list[i],
                        "entity_b": entities_list[j],
                    })

    return cooccurrences


def main():
    raw = sys.stdin.read()
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as e:
        print(json.dumps({"success": False, "error": f"invalid JSON input: {e}"}))
        sys.exit(1)

    # llm-orc wraps input as {"input": ...}, standalone uses {"input_data": ...}
    text = payload.get("input", "") or payload.get("input_data", "")
    if not text.strip():
        print(json.dumps({
            "success": True,
            "data": {"entities": [], "relationships": [], "cooccurrences": []},
        }))
        return

    nlp = load_model()
    # Increase max_length for long essays
    nlp.max_length = max(nlp.max_length, len(text) + 1000)
    doc = nlp(text)

    entities = extract_entities(doc)
    entity_keys = {e["label"].lower() for e in entities}

    relationships = extract_svo_triples(doc, entity_keys)
    cooccurrences = extract_cooccurrences(doc, entity_keys)

    print(json.dumps({
        "success": True,
        "data": {
            "entities": entities,
            "relationships": relationships,
            "cooccurrences": cooccurrences,
        },
    }))


if __name__ == "__main__":
    main()
