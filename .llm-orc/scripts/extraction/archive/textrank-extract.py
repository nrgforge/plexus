"""TextRank + TF-IDF key phrase extraction for vocabulary priming.

Extracts candidate terms from text using classical NLP techniques:
- TF-IDF scoring for statistically significant terms
- TextRank (PageRank over word co-occurrence graph) for contextually important terms
- N-gram extraction for compound terms

Input: JSON via stdin with "input_data" containing the source text.
Output: JSON with ranked candidate terms for SLM vocabulary priming.
"""

import json
import math
import re
import sys
from collections import Counter, defaultdict


# --- Stopwords (compact set covering common English) ---

STOPWORDS = frozenset("""
a about above after again against all am an and any are aren't as at be
because been before being below between both but by can't cannot could
couldn't did didn't do does doesn't doing don't down during each few for
from further get got had hadn't has hasn't have haven't having he he'd
he'll he's her here here's hers herself him himself his how how's i i'd
i'll i'm i've if in into is isn't it it's its itself let's me more most
mustn't my myself no nor not of off on once only or other ought our ours
ourselves out over own same shan't she she'd she'll she's should
shouldn't so some such than that that's the their theirs them themselves
then there there's these they they'd they'll they're they've this those
through to too under until up very was wasn't we we'd we'll we're we've
were weren't what what's when when's where where's which while who who's
whom why why's will with won't would wouldn't you you'd you'll you're
you've your yours yourself yourselves also just like even still well much
however although though since within without between another those being
been would could should might must shall may will can need using used use
one two three make made way many how new first also back only see now
well even get made going using used just like still take every much also
""".split())

# Additional generic terms to filter (common in technical writing but not meaningful)
GENERIC_TERMS = frozenset("""
data information system process approach method result example case
point question answer problem solution issue work thing part type form
kind sort area field level state point set number order way time
""".split())


def tokenize(text):
    """Split text into lowercase word tokens."""
    return re.findall(r'\b[a-z][a-z\-]+[a-z]\b', text.lower())


def tokenize_by_paragraph(text):
    """Tokenize text paragraph-by-paragraph, returning list of token lists.
    N-grams should not cross paragraph boundaries."""
    paragraphs = re.split(r'\n\s*\n', text)
    return [tokenize(p) for p in paragraphs if p.strip()]


def extract_ngrams_from_paragraphs(paragraph_tokens, n):
    """Extract n-grams within paragraphs only (no cross-paragraph spans)."""
    ngrams = []
    for tokens in paragraph_tokens:
        for i in range(len(tokens) - n + 1):
            gram = tokens[i:i + n]
            if gram[0] in STOPWORDS or gram[-1] in STOPWORDS:
                continue
            if all(w in STOPWORDS for w in gram):
                continue
            ngrams.append(' '.join(gram))
    return ngrams


def compute_tf(tokens):
    """Compute term frequency (normalized by max frequency)."""
    counts = Counter(tokens)
    if not counts:
        return {}
    max_freq = max(counts.values())
    return {term: count / max_freq for term, count in counts.items()}


def compute_positional_boost(text, terms):
    """Boost terms appearing in headers, first paragraph, or conclusions."""
    lines = text.split('\n')
    header_terms = set()
    first_para_terms = set()

    in_first_para = True
    for line in lines:
        stripped = line.strip()
        if stripped.startswith('#'):
            # Header line — extract terms
            header_words = tokenize(stripped)
            header_terms.update(header_words)
        elif stripped == '' and in_first_para:
            in_first_para = False
        elif in_first_para:
            first_words = tokenize(stripped)
            first_para_terms.update(first_words)

    boosts = {}
    for term in terms:
        words = set(term.split())
        boost = 1.0
        if words & header_terms:
            boost += 0.3
        if words & first_para_terms:
            boost += 0.1
        boosts[term] = boost
    return boosts


def textrank(tokens, window=5, damping=0.85, iterations=30):
    """
    TextRank: Build word co-occurrence graph, run PageRank.

    Returns dict of word -> score.
    """
    # Filter stopwords for graph construction
    filtered = [t for t in tokens if t not in STOPWORDS and t not in GENERIC_TERMS]

    if len(filtered) < 3:
        return {}

    # Build vocabulary
    vocab = list(set(filtered))
    word_to_idx = {w: i for i, w in enumerate(vocab)}
    n = len(vocab)

    # Build co-occurrence matrix (symmetric)
    cooccur = defaultdict(float)
    for i in range(len(filtered)):
        for j in range(i + 1, min(i + window, len(filtered))):
            w1, w2 = filtered[i], filtered[j]
            if w1 != w2:
                cooccur[(word_to_idx[w1], word_to_idx[w2])] += 1
                cooccur[(word_to_idx[w2], word_to_idx[w1])] += 1

    # Compute out-degree for normalization
    out_degree = [0.0] * n
    for (src, _), weight in cooccur.items():
        out_degree[src] += weight

    # PageRank iteration
    scores = [1.0 / n] * n
    for _ in range(iterations):
        new_scores = [(1 - damping) / n] * n
        for (src, tgt), weight in cooccur.items():
            if out_degree[src] > 0:
                new_scores[tgt] += damping * scores[src] * weight / out_degree[src]
        scores = new_scores

    return {vocab[i]: scores[i] for i in range(n)}


def extract_compound_terms(text):
    """
    Extract multi-word terms using patterns common in technical writing.
    Catches: hyphenated terms, CamelCase, quoted terms, capitalized phrases.
    Works line-by-line to avoid cross-paragraph matches.
    """
    compounds = []

    for line in text.split('\n'):
        line_lower = line.lower()

        # Hyphenated compounds (e.g., "cross-dimensional traversal")
        for match in re.finditer(r'\b([a-z]+-[a-z]+(?:-[a-z]+)*)\b', line_lower):
            term = match.group(1)
            if len(term) > 5 and term.split('-')[0] not in STOPWORDS:
                compounds.append(term.replace('-', ' '))

        # CamelCase terms (e.g., TagConceptBridger, EngineSink)
        for match in re.finditer(r'\b([A-Z][a-z]+(?:[A-Z][a-z]+)+)\b', line):
            compounds.append(match.group(1))

        # Capitalized phrases in running text (e.g., "Seed Promotion", "Trellis")
        for match in re.finditer(r'(?<=[.!?\s])([A-Z][a-z]+(?:\s+[A-Z][a-z]+)+)(?=[,.\s])', line):
            phrase = match.group(1)
            words = phrase.split()
            if len(words) <= 4 and not all(w.lower() in STOPWORDS for w in words):
                compounds.append(phrase)

        # Quoted technical terms
        for match in re.finditer(r'"([^"]{3,40})"', line):
            term = match.group(1)
            if not term[0].isdigit() and len(term.split()) <= 5:
                compounds.append(term)

    return compounds


def merge_and_rank(tf_scores, textrank_scores, compound_terms, positional_boosts,
                   paragraph_tokens, max_terms=25):
    """
    Merge signals from TF, TextRank, compound detection, and position.
    Returns ranked list of candidate terms with scores.
    """
    all_terms = set()

    # Unigrams from TextRank (top scoring)
    for term in textrank_scores:
        if term not in STOPWORDS and term not in GENERIC_TERMS and len(term) > 2:
            all_terms.add(term)

    # Bigrams and trigrams — paragraph-aware (no cross-boundary spans)
    bigrams = extract_ngrams_from_paragraphs(paragraph_tokens, 2)
    trigrams = extract_ngrams_from_paragraphs(paragraph_tokens, 3)

    bigram_counts = Counter(bigrams)
    trigram_counts = Counter(trigrams)

    # Keep bigrams appearing 2+ times or containing high-TextRank words
    for bg, count in bigram_counts.items():
        words = bg.split()
        tr_score = sum(textrank_scores.get(w, 0) for w in words)
        if count >= 2 or tr_score > 0.01:
            if not all(w in STOPWORDS or w in GENERIC_TERMS for w in words):
                all_terms.add(bg)

    # Keep trigrams appearing 2+ times
    for tg, count in trigram_counts.items():
        if count >= 2:
            words = tg.split()
            if not all(w in STOPWORDS or w in GENERIC_TERMS for w in words):
                all_terms.add(tg)

    # Compound terms from pattern extraction (filter any with newlines)
    for ct in compound_terms:
        ct_lower = ct.lower()
        if '\n' not in ct_lower:
            all_terms.add(ct_lower)

    # Score each candidate
    scored = {}
    for term in all_terms:
        words = term.split()

        # TF component (average TF of constituent words)
        tf_component = sum(tf_scores.get(w, 0) for w in words) / len(words)

        # TextRank component
        tr_component = sum(textrank_scores.get(w, 0) for w in words)

        # Frequency component (for multi-word terms)
        if len(words) == 1:
            freq_component = tf_scores.get(term, 0)
        elif len(words) == 2:
            freq_component = bigram_counts.get(term, 0) / max(len(bigrams), 1)
        else:
            freq_component = trigram_counts.get(term, 0) / max(len(trigrams), 1)

        # Length bonus (multi-word terms are more specific)
        length_bonus = 1.0 + 0.2 * (len(words) - 1)

        # Positional boost
        pos_boost = positional_boosts.get(term, 1.0)

        # Combined score
        score = (tf_component * 0.3 + tr_component * 0.4 + freq_component * 0.3) * length_bonus * pos_boost
        scored[term] = score

    # Post-filter: remove self-duplicating n-grams and stopword-led terms
    filtered = {}
    for term, score in scored.items():
        words = term.split()
        # Skip self-duplications like "provenance provenance"
        if len(words) > 1 and len(set(words)) < len(words):
            continue
        # Double-check no stopwords at boundaries (safety net)
        if words[0] in STOPWORDS or words[-1] in STOPWORDS:
            continue
        filtered[term] = score

    # Sort by score, take top N
    ranked = sorted(filtered.items(), key=lambda x: -x[1])[:max_terms]
    return ranked


def process(text):
    """Main extraction pipeline."""
    tokens = tokenize(text)
    paragraph_tokens = tokenize_by_paragraph(text)

    if len(tokens) < 10:
        return {"success": True, "data": {"candidate_terms": [], "stats": {"token_count": len(tokens)}}}

    # TF scores
    tf_scores = compute_tf([t for t in tokens if t not in STOPWORDS and t not in GENERIC_TERMS])

    # TextRank scores
    tr_scores = textrank(tokens)

    # Compound term extraction
    compounds = extract_compound_terms(text)

    # All candidate terms for positional boosting
    all_candidates = set(tf_scores.keys()) | set(tr_scores.keys()) | {c.lower() for c in compounds}
    pos_boosts = compute_positional_boost(text, all_candidates)

    # Merge and rank (paragraph-aware n-grams)
    ranked = merge_and_rank(tf_scores, tr_scores, compounds, pos_boosts, paragraph_tokens)

    # Format output
    candidate_terms = [
        {"term": term, "score": round(score, 4)}
        for term, score in ranked
    ]

    return {
        "success": True,
        "data": {
            "candidate_terms": candidate_terms,
            "stats": {
                "token_count": len(tokens),
                "unique_tokens": len(set(tokens)),
                "textrank_nodes": len(tr_scores),
                "compound_terms_found": len(compounds)
            }
        }
    }


if __name__ == "__main__":
    raw = sys.stdin.read().strip()

    # Try to parse as llm-orc ScriptAgentInput JSON
    try:
        input_json = json.loads(raw)
        if "input_data" in input_json:
            text = input_json["input_data"]
        else:
            text = raw
    except json.JSONDecodeError:
        text = raw

    result = process(text)
    print(json.dumps(result, indent=2))
