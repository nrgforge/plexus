"""
Q1 Spike: Phase 2 deterministic relationship extraction
Tests: co-occurrence, verb patterns, dependency parsing
on Essay 12 against gold-standard relationships.

Then tests Phase 2 → Phase 3 priming: does feeding Phase 2 output
to the LLM improve relationship extraction?
"""

import spacy
import json
import re
import time
from collections import defaultdict

nlp = spacy.load("en_core_web_sm")

# Essay 12 text (plain, no markdown)
ESSAY_TEXT = open("docs/essays/12-provenance-as-epistemological-infrastructure.md").read()
# Strip markdown header/references for cleaner parsing
# Keep only the body text
lines = ESSAY_TEXT.split("\n")
body_lines = []
in_references = False
for line in lines:
    if line.startswith("## References") or line.startswith("---") and len(body_lines) > 5:
        if line.startswith("## References"):
            break
    # Skip title, author, date lines
    if line.startswith("# ") or line.startswith("**Nathan") or line.startswith("*Working"):
        continue
    if line.startswith("---"):
        continue
    body_lines.append(line)
ESSAY_BODY = "\n".join(body_lines).strip()

# Known entities from Q2's best extraction (union of primed + unprimed)
# Using the domain model terms that are relevant to Essay 12
KNOWN_ENTITIES = [
    "provenance", "epistemology", "chain", "mark", "fragment",
    "tagconceptbridger", "hebbian contribution", "adapter",
    "enrichment", "multi-phase processing", "concept", "tag",
    "tagged_with", "references", "contribution", "sink",
    "enginesink", "fragmentadapter", "provenanceadapter",
    "plexus", "graph", "pipeline", "ingest",
    "dimension", "node", "edge",
    # Compound forms that appear in text
    "chain node", "mark node", "fragment node", "concept node",
    "provenance dimension", "semantic dimension", "structure dimension",
    "provenance trail", "provenance mark",
    "cross-dimensional", "deterministic id",
]

# Normalize for matching
def normalize(text):
    return text.lower().strip()

def find_entities_in_span(span_text):
    """Find known entities mentioned in a text span."""
    text_lower = normalize(span_text)
    found = []
    # Check longer entities first to avoid substring matches
    sorted_entities = sorted(KNOWN_ENTITIES, key=len, reverse=True)
    for ent in sorted_entities:
        if normalize(ent) in text_lower:
            # Avoid double-counting (e.g., "chain" inside "chain node")
            already_covered = False
            for f in found:
                if normalize(ent) in normalize(f) or normalize(f) in normalize(ent):
                    if len(f) > len(ent):
                        already_covered = True
                        break
            if not already_covered:
                found.append(ent)
    return found

# Gold standard relationships (Essay 12)
GOLD_RELATIONSHIPS = [
    ("provenance", "provides", "epistemological foundation for semantics"),
    ("chain", "contains", "mark"),
    ("mark", "references", "concept"),
    ("adapter", "produces", "provenance alongside semantics"),
    ("fragment", "has", "provenance mark"),
    ("hebbian contribution", "tracks", "per-adapter evidence"),
    ("multi-phase processing", "uses", "separate chains per phase"),
    ("tagconceptbridger", "bridges", "provenance to semantic dimension"),
]

print("=" * 70)
print("Q1 SPIKE: Phase 2 Deterministic Relationship Extraction")
print("Essay 12 | Known entities:", len(KNOWN_ENTITIES))
print("Gold relationships:", len(GOLD_RELATIONSHIPS))
print("=" * 70)

# =============================================
# Approach 1: Sentence-level co-occurrence
# =============================================
print("\n--- Approach 1: Sentence Co-occurrence ---")
doc = nlp(ESSAY_BODY)
cooccurrence_pairs = set()
sentence_entity_map = {}

for sent in doc.sents:
    sent_text = sent.text
    entities = find_entities_in_span(sent_text)
    if len(entities) >= 2:
        sentence_entity_map[sent_text[:80]] = entities
        for i, e1 in enumerate(entities):
            for e2 in entities[i+1:]:
                pair = tuple(sorted([normalize(e1), normalize(e2)]))
                cooccurrence_pairs.add(pair)

print(f"Sentences with 2+ entities: {len(sentence_entity_map)}")
print(f"Unique entity co-occurrence pairs: {len(cooccurrence_pairs)}")

# Score against gold
cooc_matches = []
for src, verb, tgt_desc in GOLD_RELATIONSHIPS:
    src_n = normalize(src)
    # Check if any co-occurrence pair includes the source
    matched = False
    for pair in cooccurrence_pairs:
        if src_n in pair[0] or src_n in pair[1]:
            # Check if target concept is also in the pair
            tgt_words = normalize(tgt_desc).split()
            for tw in tgt_words:
                if tw in pair[0] or tw in pair[1]:
                    cooc_matches.append((src, verb, tgt_desc, pair))
                    matched = True
                    break
        if matched:
            break

print(f"\nGold matches via co-occurrence: {len(cooc_matches)}/{len(GOLD_RELATIONSHIPS)}")
for src, verb, tgt, pair in cooc_matches:
    print(f"  ✓ {src} --{verb}--> {tgt}  (pair: {pair})")

# =============================================
# Approach 2: Verb pattern matching
# =============================================
print("\n--- Approach 2: Verb Pattern Matching ---")

# Common relationship verbs in domain text
VERB_PATTERNS = [
    r"(\w[\w\s]*?)\s+(?:contains?|contain)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:produces?|produce)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:creates?|create)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:bridges?|bridge)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:connects?|connect)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:references?|reference)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:tracks?|track)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:carries?|carry)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:lives?\s+in)\s+(\w[\w\s]*?)[\.,;]",
    r"(\w[\w\s]*?)\s+(?:provides?|provide)\s+(\w[\w\s]*?)[\.,;]",
]

verb_extractions = []
for sent in doc.sents:
    sent_text = sent.text
    for pattern in VERB_PATTERNS:
        matches = re.finditer(pattern, sent_text, re.IGNORECASE)
        for m in matches:
            subj = m.group(1).strip()
            obj = m.group(2).strip()
            # Check if either end contains a known entity
            subj_ents = find_entities_in_span(subj)
            obj_ents = find_entities_in_span(obj)
            if subj_ents and obj_ents:
                verb_match = re.search(r"\b(contains?|produces?|creates?|bridges?|connects?|references?|tracks?|carries?|lives?\s+in|provides?)\b",
                               m.group(0), re.IGNORECASE)
                verb = verb_match.group(0) if verb_match else "related_to"
                verb_extractions.append((subj_ents[0], verb, obj_ents[0], sent_text[:80]))

print(f"Verb pattern extractions: {len(verb_extractions)}")
for subj, verb, obj, ctx in verb_extractions:
    print(f"  {subj} --{verb}--> {obj}")
    print(f"    ctx: {ctx}...")

# =============================================
# Approach 3: Dependency parsing
# =============================================
print("\n--- Approach 3: Dependency Parsing ---")

dep_extractions = []

for sent in doc.sents:
    sent_text = sent.text
    sent_entities = find_entities_in_span(sent_text)
    if len(sent_entities) < 2:
        continue

    sent_doc = nlp(sent_text)

    # Find tokens that are roots or verbs connecting entity mentions
    for token in sent_doc:
        if token.pos_ == "VERB" or token.dep_ == "ROOT":
            # Find subjects and objects of this verb
            subjects = []
            objects = []
            for child in token.children:
                child_text = child.text.lower()
                child_subtree = " ".join([t.text for t in child.subtree]).lower()

                if child.dep_ in ("nsubj", "nsubjpass", "csubj"):
                    # Check if subtree contains a known entity
                    for ent in sent_entities:
                        if normalize(ent) in child_subtree:
                            subjects.append(ent)
                            break
                elif child.dep_ in ("dobj", "pobj", "attr", "oprd", "dative"):
                    for ent in sent_entities:
                        if normalize(ent) in child_subtree:
                            objects.append(ent)
                            break
                # Also check prepositional objects
                elif child.dep_ == "prep":
                    for grandchild in child.children:
                        gc_subtree = " ".join([t.text for t in grandchild.subtree]).lower()
                        if grandchild.dep_ == "pobj":
                            for ent in sent_entities:
                                if normalize(ent) in gc_subtree:
                                    objects.append(ent)
                                    break

            if subjects and objects:
                for s in subjects:
                    for o in objects:
                        if normalize(s) != normalize(o):
                            dep_extractions.append((s, token.lemma_, o, sent_text[:80]))

# Deduplicate
seen = set()
unique_dep = []
for s, v, o, ctx in dep_extractions:
    key = (normalize(s), v, normalize(o))
    if key not in seen:
        seen.add(key)
        unique_dep.append((s, v, o, ctx))

print(f"Dependency parse extractions: {len(unique_dep)}")
for subj, verb, obj, ctx in unique_dep:
    print(f"  {subj} --{verb}--> {obj}")
    print(f"    ctx: {ctx}...")

# =============================================
# Combined Phase 2 output
# =============================================
print("\n" + "=" * 70)
print("PHASE 2 COMBINED OUTPUT")
print("=" * 70)

# Merge all approaches
all_phase2 = []
phase2_pairs = set()

# Co-occurrence (untyped)
for pair in cooccurrence_pairs:
    key = tuple(sorted(pair))
    if key not in phase2_pairs:
        phase2_pairs.add(key)
        all_phase2.append({
            "source": pair[0],
            "target": pair[1],
            "relationship": "may_be_related",
            "method": "co-occurrence",
        })

# Verb patterns (typed)
for subj, verb, obj, ctx in verb_extractions:
    key = tuple(sorted([normalize(subj), normalize(obj)]))
    all_phase2.append({
        "source": normalize(subj),
        "target": normalize(obj),
        "relationship": verb.lower(),
        "method": "verb_pattern",
    })

# Dependency parse (typed)
for subj, verb, obj, ctx in unique_dep:
    all_phase2.append({
        "source": normalize(subj),
        "target": normalize(obj),
        "relationship": verb.lower(),
        "method": "dependency_parse",
    })

print(f"\nTotal Phase 2 extractions: {len(all_phase2)}")
print(f"  Co-occurrence pairs: {len(cooccurrence_pairs)}")
print(f"  Verb pattern: {len(verb_extractions)}")
print(f"  Dependency parse: {len(unique_dep)}")

# Score combined against gold
print(f"\n--- Gold Standard Scoring ---")
for src, verb, tgt_desc in GOLD_RELATIONSHIPS:
    src_n = normalize(src)
    matched = False
    match_details = []
    for ext in all_phase2:
        if (src_n in ext["source"] or src_n in ext["target"]):
            # Check target
            tgt_words = [w for w in normalize(tgt_desc).split() if len(w) > 3]
            for tw in tgt_words:
                if tw in ext["source"] or tw in ext["target"]:
                    match_details.append(f"{ext['method']}: {ext['source']} --{ext['relationship']}--> {ext['target']}")
                    matched = True
                    break
    if matched:
        print(f"  ✓ {src} --{verb}--> {tgt_desc}")
        for d in match_details[:2]:
            print(f"      {d}")
    else:
        print(f"  ✗ {src} --{verb}--> {tgt_desc}")

# =============================================
# Generate Phase 2 context for Phase 3 priming
# =============================================
print("\n" + "=" * 70)
print("PHASE 2 CONTEXT FOR PHASE 3 PRIMING")
print("=" * 70)

# Build a compact summary for LLM priming
typed_rels = [e for e in all_phase2 if e["relationship"] != "may_be_related"]
cooc_rels = [e for e in all_phase2 if e["relationship"] == "may_be_related"]

priming_text = "RELATIONSHIPS DETECTED BY SYNTACTIC ANALYSIS:\n"
seen_typed = set()
for r in typed_rels:
    key = (r["source"], r["relationship"], r["target"])
    if key not in seen_typed:
        seen_typed.add(key)
        priming_text += f"- {r['source']} {r['relationship']} {r['target']}\n"

priming_text += f"\nENTITY PAIRS CO-OCCURRING IN SAME SENTENCE ({len(cooc_rels)} pairs):\n"
seen_cooc = set()
for r in cooc_rels:
    key = tuple(sorted([r["source"], r["target"]]))
    if key not in seen_cooc:
        seen_cooc.add(key)
        priming_text += f"- {r['source']} <-> {r['target']}\n"

print(priming_text)

# Save for Phase 3 experiment
with open("scratch/spike-vocab-priming/phase2-context.txt", "w") as f:
    f.write(priming_text)

print(f"\nPhase 2 context saved to scratch/spike-vocab-priming/phase2-context.txt")
print(f"Typed relationships: {len(seen_typed)}")
print(f"Co-occurrence pairs: {len(seen_cooc)}")
