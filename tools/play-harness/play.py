#!/usr/bin/env python3
"""Programmatic crawl→walk→run PLAY harness for Plexus.

Drives the real consumer surface: a plexus MCP subprocess over stdio,
against a fresh temp SQLite per scenario. Assertions run twice — once
through MCP reads, once directly against the SQLite file — so the
cache-vs-disk divergence class of bug is detected, not just avoided.

Usage:
  play.py crawl [--binary PATH] [--keep-db]
  play.py walk  [--binary PATH] [--keep-db]
  play.py run   [--binary PATH] [--keep-db]
  play.py stale [--binary PATH] [--keep-db]   # expected-fail: pins cache bug
  play.py all   [--binary PATH]

Scenario ↔ PLAY-plan mapping (docs/cycle-status.md §Context for Resumption):
  crawl — lean-baseline truthfulness (ADR-038/039): temporal_proximity
          fires over untagged content; similar_to/may_be_related/
          discovery_gap absent; tagged content lights up CoOccurrence.
  walk  — tautology threshold (ADR-038/042): worked-example spec +
          fixture corpora → similar_to emerges, within-corpus only.
  run   — composition shape (ADR-041): named vs structural lens over
          the same emergent edges; query-shape diff recorded.
  stale — multi-process cache staleness (2026-04-29 field note),
          expected to fail until a cache-invalidation contract lands.

Extraction scenarios (beyond the original PLAY plan — semantic depth):
  extract-fg — foreground declarative path: LLM extracts theme/keyword,
          CoOccurrence fires over machine tags, lens translates.
  extract-bg — background deep path (SpaCy + 8 LLM agents); pins the
          T11 gap: lenses do not fire on background emissions.
"""

import argparse
import json
import os
import shutil
import sqlite3
import sys
import tempfile
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mcp_client import McpClient, McpError

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DEFAULT_BINARY = shutil.which("plexus") or "/opt/homebrew/bin/plexus"
WORKED_EXAMPLE = os.path.join(REPO, "examples", "specs", "embedding-activation.yaml")
CORPORA = {
    "ci": os.path.join(REPO, "test-corpora", "collective-intelligence"),
    "pds": os.path.join(REPO, "test-corpora", "public-domain-stories"),
}

LENS_SPEC_TEMPLATE = """\
adapter_id: lens-{consumer}
input_kind: lens-{consumer}.noop
input_schema:
  - name: text
    type: string
    required: true
emit:
  - create_node:
      id: "noop:{consumer}"
      type: fragment
      dimension: structure
      properties:
        text: "{{input.text}}"
lens:
  consumer: {consumer}
  translations:
    - from: [similar_to, temporal_proximity]
      to: {to}
"""


class Report:
    def __init__(self, scenario):
        self.scenario = scenario
        self.rows = []
        self.failed = False

    def check(self, name, ok, observed=""):
        self.rows.append(("PASS" if ok else "FAIL", name, str(observed)))
        if not ok:
            self.failed = True

    def observe(self, name, observed):
        self.rows.append(("OBS ", name, str(observed)))

    def skip(self, name, why):
        self.rows.append(("SKIP", name, why))

    def emit(self):
        print(f"\n=== {self.scenario} ===")
        for status, name, observed in self.rows:
            line = f"[{status}] {name}"
            if observed:
                line += f"  ->  {observed}"
            print(line)
        verdict = "FAILED" if self.failed else "OK"
        print(f"=== {self.scenario}: {verdict} ===")
        return not self.failed


# ── SQLite ground truth ─────────────────────────────────────────────────


def db_query(db_path, sql, params=()):
    con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        return con.execute(sql, params).fetchall()
    finally:
        con.close()


def db_context_id(db_path, name):
    rows = db_query(db_path, "SELECT id FROM contexts WHERE name = ?", (name,))
    return rows[0][0] if rows else None


def db_edge_counts(db_path, ctx_id):
    rows = db_query(
        db_path,
        "SELECT relationship, COUNT(*) FROM edges WHERE context_id = ? GROUP BY relationship",
        (ctx_id,),
    )
    return dict(rows)


def db_nodes(db_path, ctx_id, node_type=None):
    sql = "SELECT id, node_type, properties_json FROM nodes WHERE context_id = ?"
    params = [ctx_id]
    if node_type:
        sql += " AND node_type = ?"
        params.append(node_type)
    return [
        {"id": r[0], "node_type": r[1], "properties": json.loads(r[2])}
        for r in db_query(db_path, sql, tuple(params))
    ]


def db_edges(db_path, ctx_id, relationship):
    return db_query(
        db_path,
        "SELECT source_id, target_id FROM edges WHERE context_id = ? AND relationship = ?",
        (ctx_id, relationship),
    )


# ── shared plumbing ─────────────────────────────────────────────────────


def spawn(binary, db_path, tag):
    stderr_log = db_path + f".{tag}.stderr.log"
    return McpClient(
        [binary, "mcp", "--transport", "stdio", "--db", db_path],
        cwd=REPO,  # llm-orc resolves .llm-orc/ ensembles relative to cwd
        stderr_path=stderr_log,
    )


def node_count(mcp_result):
    """find_nodes returns JSON whose node list shape we treat defensively."""
    if isinstance(mcp_result, list):
        return len(mcp_result)
    if isinstance(mcp_result, dict):
        for key in ("nodes", "results", "items"):
            if key in mcp_result and isinstance(mcp_result[key], list):
                return len(mcp_result[key])
        if "count" in mcp_result:
            return mcp_result["count"]
    raise McpError(f"unrecognized find_nodes shape: {str(mcp_result)[:200]}")


def load_corpus_docs():
    docs = []
    for prefix, path in CORPORA.items():
        for fname in sorted(os.listdir(path)):
            if not fname.endswith(".md") or fname.upper().startswith(("README", "CURATION")):
                continue
            with open(os.path.join(path, fname)) as f:
                docs.append({"id": f"{prefix}/{fname[:-3]}", "text": f.read()})
    return docs


def ollama_preflight(report, model="nomic-embed-text"):
    """llm-orc scenarios need Ollama + a specific model + llm-orc. Skip loudly if absent."""
    if not shutil.which("llm-orc"):
        report.skip("preflight", "llm-orc not on PATH")
        return False
    try:
        with urllib.request.urlopen("http://localhost:11434/api/tags", timeout=3) as r:
            models = [m["name"] for m in json.load(r).get("models", [])]
    except Exception as e:
        report.skip("preflight", f"Ollama unreachable: {e}")
        return False
    if not any(m.startswith(model) for m in models):
        report.skip("preflight", f"{model} not pulled (have: {models})")
        return False
    return True


def ingest_corpora_with_spec(client, report, context_name):
    """Shared walk/run setup: load worked-example spec, ingest both corpora."""
    client.call("set_context", {"name": context_name})
    with open(WORKED_EXAMPLE) as f:
        spec_yaml = f.read()
    loaded = client.call("load_spec", {"spec_yaml": spec_yaml})
    report.observe("load_spec(embedding-activation)", loaded)
    docs = load_corpus_docs()
    report.observe("corpus docs", f"{len(docs)} ({sum(1 for d in docs if d['id'].startswith('ci/'))} ci, {sum(1 for d in docs if d['id'].startswith('pds/'))} pds)")
    result = client.call(
        "ingest",
        {"input_kind": "embedding-activation.batch", "data": {"docs": docs}},
        timeout=900,  # real Ollama embedding of the full doc set
    )
    report.observe("ingest(batch)", str(result)[:200])
    return docs


# ── scenarios ───────────────────────────────────────────────────────────


def scenario_crawl(binary, db_path):
    """Lean-baseline truthfulness (ADR-038, ADR-039)."""
    report = Report("crawl")
    ctx = "play-crawl"
    client = spawn(binary, db_path, "crawl")
    try:
        client.call("set_context", {"name": ctx})

        texts = [
            "The lighthouse keeper logged the storm at midnight.",
            "A recipe for bread requires patience more than flour.",
            "Compilers translate intent into instructions, mostly faithfully.",
        ]
        for t in texts:
            client.call("ingest", {"data": {"text": t}})

        n = len(texts)
        ctx_id = db_context_id(db_path, ctx)
        report.check("context persisted to disk", ctx_id is not None, ctx_id)

        frags = db_nodes(db_path, ctx_id, node_type="fragment")
        report.check(f"{n} fragment nodes on disk", len(frags) == n, len(frags))

        with_created = [f for f in frags if "created_at" in f["properties"]]
        report.check(
            "ADR-039: every fragment carries properties.created_at",
            len(with_created) == len(frags),
            f"{len(with_created)}/{len(frags)}",
        )

        counts = db_edge_counts(db_path, ctx_id)
        expected_tp = n * (n - 1)
        report.check(
            f"temporal_proximity fires over untagged content ({expected_tp} = n(n-1))",
            counts.get("temporal_proximity", 0) == expected_tp,
            counts.get("temporal_proximity", 0),
        )
        for absent in ("similar_to", "may_be_related", "discovery_gap"):
            report.check(f"lean baseline: no {absent}", counts.get(absent, 0) == 0, counts.get(absent, 0))

        # MCP read vs disk read (same process — must agree)
        mcp_frags = node_count(client.call("find_nodes", {"node_type": "fragment"}))
        report.check("MCP read == disk read (fragments)", mcp_frags == len(frags), f"mcp={mcp_frags} disk={len(frags)}")

        # Tagged content lights up CoOccurrence
        client.call("ingest", {"data": {"text": "Sourdough and rye share a starter.", "tags": ["bread", "fermentation"]}})
        counts = db_edge_counts(db_path, ctx_id)
        report.check("tagged content produces may_be_related (CoOccurrence)", counts.get("may_be_related", 0) >= 1, counts.get("may_be_related", 0))
        report.observe("edge relationships after tagged ingest", counts)
    finally:
        client.close()
    return report.emit()


def scenario_walk(binary, db_path):
    """Tautology threshold (ADR-038 quality bar) via the worked example."""
    report = Report("walk")
    if not ollama_preflight(report):
        report.emit()
        return True  # skip, not fail

    client = spawn(binary, db_path, "walk")
    try:
        docs = ingest_corpora_with_spec(client, report, "play-walk")
        ctx_id = db_context_id(db_path, "play-walk")

        doc_nodes = db_nodes(db_path, ctx_id, node_type="fragment")
        report.check(f"{len(docs)} doc nodes on disk", len(doc_nodes) >= len(docs), len(doc_nodes))

        counts = db_edge_counts(db_path, ctx_id)
        report.observe("edge relationships", counts)

        sim_edges = db_edges(db_path, ctx_id, "similar_to")
        report.check("similar_to edges emerged over untagged prose", len(sim_edges) > 0, len(sim_edges))

        corpus = lambda node_id: node_id.split("/")[0]
        cross = [(s, t) for s, t in sim_edges if corpus(s) != corpus(t)]
        within_ci = [(s, t) for s, t in sim_edges if corpus(s) == corpus(t) == "ci"]
        within_pds = [(s, t) for s, t in sim_edges if corpus(s) == corpus(t) == "pds"]
        report.check("tautology threshold: zero cross-corpus similar_to at 0.72", len(cross) == 0, cross[:5] if cross else 0)
        report.check("within-corpus clustering: ci pairs present", len(within_ci) > 0, len(within_ci))
        report.check("within-corpus clustering: pds pairs present", len(within_pds) > 0, len(within_pds))

        report.observe("discovery_gap now that similar_to exists", counts.get("discovery_gap", 0))
        report.observe("similar_to pairs", sorted({tuple(sorted(p)) for p in sim_edges}))
    finally:
        client.close()
    return report.emit()


def scenario_run(binary, db_path):
    """Composition shape (ADR-041): named vs structural lens, same context."""
    report = Report("run")
    if not ollama_preflight(report):
        report.emit()
        return True

    ctx = "play-run"
    client = spawn(binary, db_path, "run")
    try:
        ingest_corpora_with_spec(client, report, ctx)
        ctx_id = db_context_id(db_path, ctx)

        # Two consumers over the same emergent edges: named vs structural register
        for consumer, to in (("trellis", "thematic_connection"), ("scout", "latent_pair")):
            spec = LENS_SPEC_TEMPLATE.format(consumer=consumer, to=to)
            loaded = client.call("load_spec", {"spec_yaml": spec})
            report.observe(f"load_spec(lens-{consumer}) initial sweep", loaded)

        counts = db_edge_counts(db_path, ctx_id)
        trellis_edges = {k: v for k, v in counts.items() if k.startswith("lens:trellis")}
        scout_edges = {k: v for k, v in counts.items() if k.startswith("lens:scout")}
        report.check("named-register lens produced edges", sum(trellis_edges.values()) > 0, trellis_edges)
        report.check("structural-register lens produced edges", sum(scout_edges.values()) > 0, scout_edges)
        report.check(
            "same from-list -> identical edge counts (topology invariant under register)",
            sum(trellis_edges.values()) == sum(scout_edges.values()),
            f"trellis={sum(trellis_edges.values())} scout={sum(scout_edges.values())}",
        )

        # Invariant 56: both consumers' vocabulary visible in one context, one query
        both = node_count(client.call("find_nodes", {"relationship_prefix": "lens:"}))
        report.check("Inv 56: lens output public (prefix 'lens:' spans consumers)", both > 0, both)

        # Query-shape diff: the same query, per consumer vocabulary
        t_nodes = node_count(client.call("find_nodes", {"relationship_prefix": "lens:trellis:thematic_connection"}))
        s_nodes = node_count(client.call("find_nodes", {"relationship_prefix": "lens:scout:latent_pair"}))
        report.observe(
            "query-shape: consumer queries by meaning vs by shape reach same nodes",
            f"thematic_connection={t_nodes} latent_pair={s_nodes}",
        )
        report.check("both vocabularies reach the same node population", t_nodes == s_nodes, f"{t_nodes} vs {s_nodes}")
    finally:
        client.close()
    return report.emit()


def scenario_stale(binary, db_path):
    """Multi-process read coherence regression (ADR-017 §2).

    Originally an expected-fail repro of the 2026-04-29 staleness
    finding. Flipped to a positive assertion 2026-07-07 when the fix
    landed (PlexusApi wires reload_if_changed into name resolution).
    Fails against binaries older than the fix (<= v0.3.0).
    """
    report = Report("stale (coherence regression)")
    ctx = "play-stale"
    a = spawn(binary, db_path, "stale-a")
    try:
        a.call("set_context", {"name": ctx})
        a.call("ingest", {"data": {"text": "first write, process A"}})

        b = spawn(binary, db_path, "stale-b")
        try:
            b.call("set_context", {"name": ctx})
            b.call("ingest", {"data": {"text": "second write, process B"}})
            b.call("ingest", {"data": {"text": "third write, process B"}})
        finally:
            b.close()

        ctx_id = db_context_id(db_path, ctx)
        disk = len(db_nodes(db_path, ctx_id, node_type="fragment"))
        a.call("set_context", {"name": ctx})  # field note: does NOT invalidate cache
        seen_by_a = node_count(a.call("find_nodes", {"node_type": "fragment"}))

        report.observe("fragments on disk", disk)
        report.observe("fragments visible to long-lived process A", seen_by_a)
        report.check(
            "long-lived process sees another process's writes (ADR-017 §2)",
            seen_by_a == disk,
            f"A sees {seen_by_a}/{disk}",
        )
    finally:
        a.close()
    return report.emit()


EXTRACTION_RELATIONSHIPS = [
    # extract-semantic's full relationship vocabulary — used as a lens
    # from-list so the T11 gap pin is airtight: if lenses fired on
    # background emissions, ANY extracted edge would be translated.
    "caused_by", "remedies", "exemplifies", "describes", "mechanism_of",
    "distinct_from", "eroded_by", "accelerates", "instance_of",
    "component_of", "enables", "constrains", "produces", "requires",
    "implements", "uses", "depends_on", "part_of", "related_to",
]

FOREGROUND_EXTRACT_SPEC = """\
adapter_id: play-extract-fg
input_kind: play-extract.text
ensemble: test-theme-extractor
lens:
  consumer: probe
  translations:
    - from: [may_be_related]
      to: latent_pair
emit:
  - create_node:
      id: "frag:{input.id}"
      type: fragment
      dimension: semantic
      properties:
        text: "{input.text}"
  - create_node:
      id: "concept:{ensemble.theme}"
      type: concept
      dimension: semantic
  - create_node:
      id: "concept:{ensemble.keyword}"
      type: concept
      dimension: semantic
  - create_edge:
      source: "frag:{input.id}"
      target: "concept:{ensemble.theme}"
      relationship: tagged_with
  - create_edge:
      source: "frag:{input.id}"
      target: "concept:{ensemble.keyword}"
      relationship: tagged_with
"""

BG_LENS_SPEC = """\
adapter_id: play-extract-bg-lens
input_kind: play-extract-bg.noop
input_schema:
  - name: text
    type: string
    required: true
emit:
  - create_node:
      id: "noop:bg"
      type: fragment
      dimension: structure
      properties:
        text: "{input.text}"
lens:
  consumer: bgprobe
  translations:
    - from: [%s]
      to: extracted_link
""" % ", ".join(EXTRACTION_RELATIONSHIPS)

BG_FIXTURE = """\
# Bread notes

Wild yeast ferments the dough slowly. Fermentation produces carbon
dioxide, and the trapped gas leavens the loaf. A sourdough starter
requires regular feeding with flour and water. Gluten development
depends on kneading, and a hot oven enables the final rise that
bakers call oven spring.
"""


def scenario_extract_foreground(binary, db_path):
    """Semantic extraction via the foreground declarative path (spec-author
    guide's 'minimum-useful' route 3): unstructured prose → LLM extracts
    theme/keyword → tagged_with edges → CoOccurrence over machine-extracted
    tags → lens translation. The full chain, no consumer-supplied tags.

    Assertions are property-based (T7/T8 convention): existence and
    structure, never specific extracted labels — LLM output varies.
    """
    report = Report("extract-fg")
    if not ollama_preflight(report, model="mistral"):
        report.emit()
        return True

    ctx = "play-extract-fg"
    client = spawn(binary, db_path, "extract-fg")
    try:
        client.call("set_context", {"name": ctx})
        loaded = client.call("load_spec", {"spec_yaml": FOREGROUND_EXTRACT_SPEC})
        report.observe("load_spec(extract-fg)", loaded)

        texts = [
            {"id": "note-1", "text": "The sourdough starter bubbled overnight, and by morning the kitchen smelled of ripe fermentation."},
            {"id": "note-2", "text": "Kneading the dough develops gluten, which traps the gas that makes bread rise in the oven."},
        ]
        for t in texts:
            result = client.call(
                "ingest",
                {"input_kind": "play-extract.text", "data": t},
                timeout=300,  # synchronous llm-orc ensemble call per ingest
            )
            report.observe(f"ingest({t['id']})", str(result)[:120])

        ctx_id = db_context_id(db_path, ctx)
        counts = db_edge_counts(db_path, ctx_id)
        report.observe("edge relationships", counts)

        concepts = db_nodes(db_path, ctx_id, node_type="concept")
        report.check("machine-extracted concept nodes exist (consumer supplied no tags)", len(concepts) >= 1, [c["id"] for c in concepts])
        report.check("tagged_with edges from fragments to extracted concepts", counts.get("tagged_with", 0) >= 2, counts.get("tagged_with", 0))
        report.check("CoOccurrence fires over machine-extracted tags (may_be_related)", counts.get("may_be_related", 0) >= 1, counts.get("may_be_related", 0))
        report.check(
            "lens translates machine-derived structure (lens:probe:latent_pair)",
            counts.get("lens:probe:latent_pair", 0) >= 1,
            counts.get("lens:probe:latent_pair", 0),
        )
    finally:
        client.close()
    return report.emit()


def scenario_extract_background(binary, db_path):
    """Deep semantic extraction via the background path: extract-file →
    ExtractionCoordinator → SemanticAdapter → extract-semantic ensemble
    (SpaCy + 8 LLM agents, multi-run union per Invariant 45).

    Also pins the T11 architectural gap: a lens loaded BEFORE ingest,
    with a from-list covering extract-semantic's entire relationship
    vocabulary, translates nothing — lenses do not fire on
    background-phase emissions. That check goes red when the gap closes;
    flip it to a positive assertion then.
    """
    report = Report("extract-bg")
    if not ollama_preflight(report, model="mistral"):
        report.emit()
        return True

    import time

    ctx = "play-extract-bg"
    fixture = os.path.join(os.path.dirname(db_path), "bread-notes.md")
    with open(fixture, "w") as f:
        f.write(BG_FIXTURE)

    client = spawn(binary, db_path, "extract-bg")
    try:
        client.call("set_context", {"name": ctx})
        # Lens registered before ingest so it WOULD translate if the
        # background phase reached the enrichment loop.
        loaded = client.call("load_spec", {"spec_yaml": BG_LENS_SPEC})
        report.observe("load_spec(bg lens, full extraction vocabulary)", loaded)

        result = client.call("ingest", {"input_kind": "extract-file", "data": {"file_path": fixture}})
        report.observe("ingest(extract-file) returned (background continues)", str(result)[:120])

        # Poll for background extraction (T6 pattern): registration returns
        # immediately; SpaCy + 8 LLM agents take minutes.
        deadline = time.time() + 600
        concepts = 0
        while time.time() < deadline:
            concepts = node_count(client.call("find_nodes", {"node_type": "concept"}))
            if concepts > 0:
                break
            time.sleep(3)
        report.check("background extraction produced concept nodes (≤10min)", concepts > 0, concepts)

        # Give the remaining agents a moment to land after first concepts,
        # then read final state from disk.
        time.sleep(30)
        ctx_id = db_context_id(db_path, ctx)
        counts = db_edge_counts(db_path, ctx_id)
        report.observe("edge relationships", counts)

        semantic_edges = {k: v for k, v in counts.items() if k in EXTRACTION_RELATIONSHIPS}
        report.check("typed relationships extracted (extract-semantic vocabulary)", sum(semantic_edges.values()) >= 1, semantic_edges)

        # Invariant 45 (multi-run union): observe corroboration, don't assert —
        # whether two agents agree on the same edge is run-dependent.
        rows = db_query(
            db_path,
            "SELECT relationship, contributions_json FROM edges WHERE context_id = ?",
            (ctx_id,),
        )
        max_contrib = max((len(json.loads(c or "{}")) for _, c in rows), default=0)
        report.observe("max distinct contributions on any edge (Inv 45 reinforcement)", max_contrib)

        lens_count = sum(v for k, v in counts.items() if k.startswith("lens:bgprobe"))
        report.check(
            "T11 gap pin: lens does NOT fire on background emissions (0 expected)",
            lens_count == 0,
            f"{lens_count}" + ("" if lens_count == 0 else " — gap seems CLOSED; flip this assertion"),
        )
    finally:
        client.close()
    return report.emit()


def _count_result(mcp_result):
    """Count entries in a tool result of unknown shape (list or keyed dict)."""
    if isinstance(mcp_result, list):
        return len(mcp_result)
    if isinstance(mcp_result, dict):
        for key in ("events", "nodes", "results", "items", "tags", "concepts", "contexts", "shared"):
            if key in mcp_result and isinstance(mcp_result[key], list):
                return len(mcp_result[key])
        if "count" in mcp_result:
            return mcp_result["count"]
    return None


def matrix_write(client):
    """Writer actions for the consistency matrix: two contexts + a late one."""
    client.call("set_context", {"name": "mx"})
    client.call("ingest", {"data": {"text": "alpha beta fragment", "tags": ["alpha", "beta"]}})
    client.call("ingest", {"data": {"text": "plain untagged fragment"}})
    client.call("set_context", {"name": "mx2"})
    client.call("ingest", {"data": {"text": "alpha again in a second context", "tags": ["alpha"]}})
    client.call("set_context", {"name": "mx-late"})
    client.call("ingest", {"data": {"text": "context created after the reader started"}})


def matrix_read(client, db_path, report, topology):
    """Reader-side cells: each read surface classified LIVE / STALE / ERROR
    against SQLite ground truth."""
    client.call("set_context", {"name": "mx"})  # known: does not invalidate cache

    ctx_id = db_context_id(db_path, "mx")
    disk_frags = len(db_nodes(db_path, ctx_id, node_type="fragment"))
    disk_mbr_nodes = len({n for s, t in db_edges(db_path, ctx_id, "may_be_related") for n in (s, t)})
    disk_events = db_query(db_path, "SELECT COUNT(*) FROM events WHERE context_id = ?", (ctx_id,))[0][0]

    def classify(name, observed, expected):
        if observed is None:
            status = "ERROR"
        elif observed == expected:
            status = "LIVE"
        elif isinstance(observed, int) and observed < expected:
            status = "STALE"
        else:
            status = f"?({observed})"
        report.observe(f"[{topology}] {name}", f"{status} (reader={observed}, disk={expected})")
        return status

    cells = {}

    try:
        obs = node_count(client.call("find_nodes", {"node_type": "fragment"}))
    except McpError:
        obs = None
    cells["node reads (find_nodes)"] = classify("node reads (find_nodes)", obs, disk_frags)

    try:
        obs = node_count(client.call("find_nodes", {"relationship_prefix": "may_be_related"}))
    except McpError:
        obs = None
    cells["edge reads (prefix filter)"] = classify("edge reads (prefix filter)", obs, disk_mbr_nodes)

    try:
        obs = _count_result(client.call("list_tags", {}))
    except McpError:
        obs = None
    cells["list_tags"] = classify("list_tags", obs, 2)  # alpha + beta on disk

    try:
        obs = _count_result(client.call("shared_concepts", {"context_a": "mx", "context_b": "mx2"}))
    except McpError:
        obs = None
    cells["shared_concepts(mx, mx2)"] = classify("shared_concepts(mx, mx2)", obs, 1)  # concept:alpha

    try:
        obs = _count_result(client.call("changes_since", {"cursor": 0}))
    except McpError:
        obs = None
    cells["changes_since (event cursor)"] = classify("changes_since (event cursor)", obs, disk_events)

    try:
        ctxs = client.call("context_list", {})
        text = json.dumps(ctxs)
        obs = 1 if "mx-late" in text else 0
    except McpError:
        obs = None
    cells["context_list sees late context"] = classify("context_list sees late context", obs, 1)

    return cells


def scenario_matrix(binary, db_path):
    """M0 consistency matrix (GitHub issue #1): process topologies × read
    surfaces. Classifies each cell LIVE (reader sees writer's data) or
    STALE against SQLite ground truth. Purely observational — no
    pass/fail beyond 'the baseline topology must be fully LIVE'.

    Topologies:
      same-process   — writer and reader are one process (baseline)
      restart        — writer exits; a fresh reader process hydrates
      concurrent     — reader starts first and stays alive; writer is a
                       second process (the vision's multi-client shape)
    """
    report = Report("matrix")
    workdir = os.path.dirname(db_path)
    grid = {}

    # T1 — same process
    db1 = os.path.join(workdir, "matrix-same.db")
    c = spawn(binary, db1, "mx-same")
    try:
        matrix_write(c)
        grid["same-process"] = matrix_read(c, db1, report, "same-process")
    finally:
        c.close()

    # T2 — restart (one-shot writer, fresh reader)
    db2 = os.path.join(workdir, "matrix-restart.db")
    w = spawn(binary, db2, "mx-restart-w")
    try:
        matrix_write(w)
    finally:
        w.close()
    r = spawn(binary, db2, "mx-restart-r")
    try:
        grid["restart"] = matrix_read(r, db2, report, "restart")
    finally:
        r.close()

    # T3 — concurrent long-lived reader + separate writer
    db3 = os.path.join(workdir, "matrix-concurrent.db")
    reader = spawn(binary, db3, "mx-conc-r")
    try:
        reader.call("set_context", {"name": "mx"})  # reader alive before any writes
        writer = spawn(binary, db3, "mx-conc-w")
        try:
            matrix_write(writer)
        finally:
            writer.close()
        grid["concurrent"] = matrix_read(reader, db3, report, "concurrent")
    finally:
        reader.close()

    # Render the matrix
    surfaces = list(next(iter(grid.values())).keys())
    col_w = max(len(s) for s in surfaces) + 2
    print("\n  CONSISTENCY MATRIX (reader's view of writer's data)")
    header = " " * col_w + "".join(f"{t:>14}" for t in grid)
    print("  " + header)
    for s in surfaces:
        row = f"{s:<{col_w}}" + "".join(f"{grid[t][s]:>14}" for t in grid)
        print("  " + row)
    print()

    report.check(
        "baseline: same-process topology fully LIVE",
        all(v == "LIVE" for v in grid["same-process"].values()),
        grid["same-process"],
    )
    report.check(
        "restart topology fully LIVE (hydration works)",
        all(v == "LIVE" for v in grid["restart"].values()),
        grid["restart"],
    )
    report.observe("concurrent topology (the M0 question)", grid["concurrent"])

    # Write-write probe: does a stale writer clobber the other's rows on
    # persist? A writes f1; B (fresh, hydrated) writes f2; A writes f3
    # from its stale cache. If save_context persists the whole cached
    # context, f2 is erased from disk.
    db4 = os.path.join(workdir, "matrix-ww.db")
    a = spawn(binary, db4, "mx-ww-a")
    try:
        a.call("set_context", {"name": "ww"})
        a.call("ingest", {"data": {"text": "f1 from A"}})
        b = spawn(binary, db4, "mx-ww-b")
        try:
            b.call("set_context", {"name": "ww"})
            b.call("ingest", {"data": {"text": "f2 from B"}})
        finally:
            b.close()
        a.call("ingest", {"data": {"text": "f3 from A, stale cache"}})
    finally:
        a.close()
    ww_ctx = db_context_id(db4, "ww")
    ww_frags = [n["properties"].get("text", n["id"]) for n in db_nodes(db4, ww_ctx, node_type="fragment")]
    lost = len(ww_frags) < 3
    report.check(
        "write-write: no data loss under interleaved writers (3 fragments survive)",
        not lost,
        f"{len(ww_frags)}/3 on disk: {ww_frags}" + (" — STALE WRITER CLOBBERED THE OTHER'S ROWS" if lost else ""),
    )
    pivotal = grid["concurrent"].get("changes_since (event cursor)")
    report.observe(
        "PIVOTAL CELL: changes_since across live processes",
        f"{pivotal} — " + (
            "the event log IS a cross-process sync channel (reads bypass the cache, straight to SQLite)"
            if pivotal == "LIVE"
            else "the event log does NOT cross processes; invalidation contract or daemon needed"
        ),
    )
    return report.emit()


# ── M1: flywheel + differential (vision.md, GitHub issue tracking) ──────
#
# Engineered-overlap corpus, inline by design (CURATION): each trellis
# fragment shares one deliberately dominant noun with one carrel note
# (starlings / ants / crowd), so single-word theme/keyword extraction
# has a fair chance of converging on shared concept IDs across
# consumers. The overlap is planted; what is NOT planted is any edge —
# all structure must emerge from extraction + co-occurrence + lens.

TRELLIS_FRAGMENTS = [
    {"id": "murmuration-dusk", "text": "The murmuration turned as one mind, ten thousand starlings folding the dusk into a single gesture. No bird leads; the starlings decide together."},
    {"id": "scent-city", "text": "The ants wrote their city in scent, each trail a sentence revised by a thousand small authors until the shortest path read like truth. Ants argue in pheromones."},
    {"id": "pit-physics", "text": "The crowd in the mosh pit breathed like a fluid, strangers colliding into brief constellations, the crowd discovering physics it never studied."},
]

CARREL_NOTES = [
    {"id": "note-starlings", "text": "Field notes on starlings: murmuration dynamics suggest local rules produce global coordination without central control. Starlings turn in cascades faster than any individual reaction time."},
    {"id": "note-ants", "text": "Reading on ant colony optimization: pheromone trails let ants converge on shortest paths. The ants compute collectively without any central computer."},
    {"id": "note-crowd", "text": "Crowd dynamics paper: mosh pits exhibit gas-like particle motion; the crowd behaves like a granular fluid under density changes."},
]

CONSUMER_SPEC_TEMPLATE = """\
adapter_id: {consumer}
input_kind: {consumer}.item
ensemble: test-theme-extractor
lens:
  consumer: {consumer}
  translations:
    - from: [may_be_related]
      to: {to}
emit:
  - create_node:
      id: "frag:{consumer}:{{input.id}}"
      type: fragment
      dimension: semantic
      properties:
        text: "{{input.text}}"
  - create_node:
      id: "concept:{{ensemble.theme}}"
      type: concept
      dimension: semantic
  - create_node:
      id: "concept:{{ensemble.keyword}}"
      type: concept
      dimension: semantic
  - create_edge:
      source: "frag:{consumer}:{{input.id}}"
      target: "concept:{{ensemble.theme}}"
      relationship: tagged_with
  - create_edge:
      source: "frag:{consumer}:{{input.id}}"
      target: "concept:{{ensemble.keyword}}"
      relationship: tagged_with
"""


def concept_contributors(db_path, ctx_id):
    """Map concept node id -> set of adapter ids that tagged it."""
    rows = db_query(
        db_path,
        "SELECT target_id, contributions_json FROM edges "
        "WHERE context_id = ? AND relationship = 'tagged_with'",
        (ctx_id,),
    )
    owners = {}
    for target, contrib in rows:
        for key in json.loads(contrib or "{}"):
            owners.setdefault(target, set()).add(key)
    return owners


def lens_pairs(db_path, ctx_id, relationship):
    return {tuple(sorted(p)) for p in db_edges(db_path, ctx_id, relationship)}


def ingest_items(client, kind, items, report, label):
    for item in items:
        client.call("ingest", {"input_kind": kind, "data": item}, timeout=300)
    report.observe(f"ingested {label}", f"{len(items)} items via {kind}")


def scenario_flywheel(binary, db_path):
    """M1: cross-pollination flywheel + solo-vs-shared differential.

    Two live consumer processes (trellis: structural register; carrel:
    named register) share one context. The claim under test is the
    vision's central sentence: consumer A receives, in A's own
    vocabulary, a provenance-attributed signal about content B ingested.
    The differential then measures what cohabitation bought: A's lens
    pairs in the shared context minus A's pairs from an identical solo
    run. Both runs use the concept-identity cross-pollination mechanism
    (extraction -> shared concept IDs -> co-occurrence -> lens).
    """
    report = Report("flywheel")
    if not ollama_preflight(report, model="mistral"):
        report.emit()
        return True

    trellis_spec = CONSUMER_SPEC_TEMPLATE.format(consumer="trellis", to="latent_pair")
    carrel_spec = CONSUMER_SPEC_TEMPLATE.format(consumer="carrel", to="related_material")

    # ── Solo baseline: trellis alone in its own context ──────────────
    a_solo = spawn(binary, db_path, "fw-solo")
    try:
        a_solo.call("set_context", {"name": "solo"})
        a_solo.call("load_spec", {"spec_yaml": trellis_spec})
        ingest_items(a_solo, "trellis.item", TRELLIS_FRAGMENTS, report, "solo trellis fragments")
    finally:
        a_solo.close()
    solo_ctx = db_context_id(db_path, "solo")
    solo_pairs = lens_pairs(db_path, solo_ctx, "lens:trellis:latent_pair")
    report.observe("solo lens:trellis:latent_pair pairs", sorted(solo_pairs) or 0)

    # ── Shared context: two live consumer processes ───────────────────
    a = spawn(binary, db_path, "fw-trellis")
    try:
        a.call("set_context", {"name": "studio"})
        a.call("load_spec", {"spec_yaml": trellis_spec})

        b = spawn(binary, db_path, "fw-carrel")  # constructed after trellis spec persisted
        try:
            b.call("set_context", {"name": "studio"})
            b.call("load_spec", {"spec_yaml": carrel_spec})

            ingest_items(a, "trellis.item", TRELLIS_FRAGMENTS, report, "trellis fragments (process A)")
            ingest_items(b, "carrel.item", CARREL_NOTES, report, "carrel notes (process B)")

            # Coherence fix in action: A sees B's writes live, no restart
            ctx_id = db_context_id(db_path, "studio")
            disk_frags = len(db_nodes(db_path, ctx_id, node_type="fragment"))
            a_sees = node_count(a.call("find_nodes", {"node_type": "fragment"}))
            report.check(
                "concurrent liveness: A sees B's ingests without restart (ADR-017 §2)",
                a_sees == disk_frags,
                f"A sees {a_sees}/{disk_frags}",
            )

            # A queries in A's own vocabulary, through the live MCP surface
            a_latent = node_count(a.call("find_nodes", {"relationship_prefix": "lens:trellis:latent_pair"}))
            report.check("A's vocabulary query returns nodes (lens:trellis:latent_pair)", a_latent > 0, a_latent)
        finally:
            b.close()
    finally:
        a.close()

    # ── Cross-pollination analysis (disk ground truth) ────────────────
    ctx_id = db_context_id(db_path, "studio")
    owners = concept_contributors(db_path, ctx_id)
    both = sorted(c for c, o in owners.items() if {"trellis", "carrel"} <= o)
    trellis_only = {c for c, o in owners.items() if o == {"trellis"}}
    carrel_only = {c for c, o in owners.items() if o == {"carrel"}}
    report.observe("concepts tagged by BOTH consumers (identity convergence)", both or "none")
    report.observe("consumer-exclusive concepts", f"trellis-only={sorted(trellis_only)} carrel-only={sorted(carrel_only)}")
    report.check("concept-identity convergence across consumers", len(both) > 0, len(both))

    shared_pairs = lens_pairs(db_path, ctx_id, "lens:trellis:latent_pair")
    carrel_pairs = lens_pairs(db_path, ctx_id, "lens:carrel:related_material")
    report.observe("shared-context lens:trellis:latent_pair pairs", sorted(shared_pairs))
    report.observe("shared-context lens:carrel:related_material pairs", sorted(carrel_pairs))

    # The flywheel claim: a signal in A's vocabulary about B's material —
    # a trellis lens pair with an endpoint concept only carrel produced.
    cross_a = [p for p in shared_pairs if any(n in carrel_only for n in p)]
    cross_b = [p for p in carrel_pairs if any(n in trellis_only for n in p)]
    report.check(
        "FLYWHEEL: trellis receives signal about carrel-only material, in trellis vocabulary",
        len(cross_a) > 0,
        cross_a or "none",
    )
    report.check(
        "FLYWHEEL (reverse): carrel receives signal about trellis-only material",
        len(cross_b) > 0,
        cross_b or "none",
    )

    # ── Differential: what did cohabitation buy trellis? ─────────────
    # Compare by concept-pair identity; solo pair set uses the same
    # corpus and spec, so any new pair is attributable to cohabitation.
    delta = shared_pairs - solo_pairs
    report.observe(
        "DIFFERENTIAL (marginal value of the shared context)",
        f"solo={len(solo_pairs)} shared={len(shared_pairs)} delta={len(delta)}: {sorted(delta)}",
    )
    report.check("differential: cohabitation produced pairs solo could not", len(delta) > 0, len(delta))

    # Translation-coverage observation (spec-registry staleness probe):
    # may_be_related edges lacking a lens translation reveal whether a
    # process whose pipeline predates the other consumer's load_spec
    # left edges untranslated.
    mbr = lens_pairs(db_path, ctx_id, "may_be_related")
    untranslated_trellis = mbr - shared_pairs
    untranslated_carrel = mbr - carrel_pairs
    report.observe(
        "lens coverage over may_be_related",
        f"mbr={len(mbr)} untranslated-by-trellis={len(untranslated_trellis)} untranslated-by-carrel={len(untranslated_carrel)}",
    )
    return report.emit()


LEAN_CONSUMER_SPEC = """\
adapter_id: {consumer}
input_kind: {consumer}.item
lens:
  consumer: {consumer}
  translations:
    - from: [similar_to]
      to: {to}
emit:
  - create_node:
      id: "frag:{consumer}:{{input.id}}"
      type: fragment
      dimension: semantic
      properties:
        text: "{{input.text}}"
"""


def scenario_latent(binary, db_path):
    """M2 / issue #9: cross-consumer latent bridging via a re-embed sweep.

    The worked-example embedding spec is batch-local, so two consumers
    ingesting separately never get cross-consumer similar_to edges from
    it directly. This scenario proves the consumer-side resolution: any
    consumer can assemble a batch FROM THE WHOLE CONTEXT (MCP find_nodes
    returns node text), re-ingest it through the embedding-activation
    spec, and the resulting similar_to edges bridge consumers. Upserts
    make the re-ingest idempotent at the node level; lenses then
    translate the latent bridges into each consumer's vocabulary.
    No engine changes — existing machinery composed.
    """
    report = Report("latent")
    if not ollama_preflight(report, model="nomic-embed-text"):
        report.emit()
        return True

    ctx = "latent"
    a = spawn(binary, db_path, "lat-trellis")
    try:
        a.call("set_context", {"name": ctx})
        a.call("load_spec", {"spec_yaml": LEAN_CONSUMER_SPEC.format(consumer="trellis", to="latent_pair")})

        b = spawn(binary, db_path, "lat-carrel")
        try:
            b.call("set_context", {"name": ctx})
            b.call("load_spec", {"spec_yaml": LEAN_CONSUMER_SPEC.format(consumer="carrel", to="related_material")})

            ingest_items(a, "trellis.item", TRELLIS_FRAGMENTS, report, "trellis fragments (A)")
            ingest_items(b, "carrel.item", CARREL_NOTES, report, "carrel notes (B)")

            # ── The re-embed sweep, performed by consumer A through MCP ──
            found = a.call("find_nodes", {"node_type": "fragment"})
            docs = [
                {"id": n["id"], "text": n["properties"]["text"]}
                for n in found.get("nodes", [])
                if n.get("properties", {}).get("text")
            ]
            report.check("sweep read: A assembled whole-context batch via MCP", len(docs) == 6, len(docs))

            with open(WORKED_EXAMPLE) as f:
                a.call("load_spec", {"spec_yaml": f.read()})
            result = a.call(
                "ingest",
                {"input_kind": "embedding-activation.batch", "data": {"docs": docs}},
                timeout=600,
            )
            report.observe("re-embed sweep ingest", str(result)[:120])

            ctx_id = db_context_id(db_path, ctx)
            sim = db_edges(db_path, ctx_id, "similar_to")
            cross_sim = {tuple(sorted(p)) for p in sim if ("frag:trellis:" in p[0]) != ("frag:trellis:" in p[1])}
            report.check(
                "cross-consumer similar_to edges exist (latent bridge across consumers)",
                len(cross_sim) > 0,
                sorted(cross_sim) or "none",
            )

            # Trellis lens fires in A's process (loaded there) on the new edges
            trellis_pairs = lens_pairs(db_path, ctx_id, "lens:trellis:latent_pair")
            cross_trellis = [p for p in trellis_pairs if ("frag:trellis:" in p[0]) != ("frag:trellis:" in p[1])]
            report.check(
                "LATENT FLYWHEEL: trellis receives latent bridge to carrel material, in trellis vocabulary",
                len(cross_trellis) > 0,
                sorted(cross_trellis) or "none",
            )

            # Carrel's lens lives in B's pipeline (A's predates carrel's
            # load_spec). One further B-side event triggers the full-scan
            # lens over the sweep's edges — the self-heal observed in M1.
            carrel_before = len(lens_pairs(db_path, ctx_id, "lens:carrel:related_material"))
            b.call("ingest", {"input_kind": "carrel.item", "data": {"id": "late-note", "text": "A late note about granular flows in dense crowds."}})
            carrel_after = lens_pairs(db_path, ctx_id, "lens:carrel:related_material")
            cross_carrel = [p for p in carrel_after if ("frag:carrel:" in p[0]) != ("frag:carrel:" in p[1])]
            report.observe("carrel lens coverage before/after B-side event", f"{carrel_before} -> {len(carrel_after)}")
            report.check(
                "carrel lens covers the sweep's bridges after its next event (cross-process self-heal)",
                len(cross_carrel) > 0,
                sorted(cross_carrel) or "none",
            )

            # A sees everything live — coherence fix inside the loop again
            a_pairs = node_count(a.call("find_nodes", {"relationship_prefix": "lens:trellis:latent_pair"}))
            report.check("A queries its latent bridges live via MCP", a_pairs > 0, a_pairs)
        finally:
            b.close()
    finally:
        a.close()
    return report.emit()


SCENARIOS = {
    "crawl": scenario_crawl,
    "walk": scenario_walk,
    "run": scenario_run,
    "stale": scenario_stale,
    "extract-fg": scenario_extract_foreground,
    "extract-bg": scenario_extract_background,
    "matrix": scenario_matrix,
    "flywheel": scenario_flywheel,
    "latent": scenario_latent,
}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("scenario", choices=[*SCENARIOS, "all"])
    parser.add_argument("--binary", default=DEFAULT_BINARY, help=f"plexus binary (default: {DEFAULT_BINARY})")
    parser.add_argument("--keep-db", action="store_true", help="keep the temp DB for post-mortem")
    args = parser.parse_args()

    names = list(SCENARIOS) if args.scenario == "all" else [args.scenario]
    ok = True
    for name in names:
        workdir = tempfile.mkdtemp(prefix=f"plexus-play-{name}-")
        db_path = os.path.join(workdir, "play.db")
        print(f"\n>>> {name}: binary={args.binary} db={db_path}")
        try:
            ok = SCENARIOS[name](args.binary, db_path) and ok
        finally:
            if args.keep_db:
                print(f">>> kept: {workdir}")
            else:
                shutil.rmtree(workdir, ignore_errors=True)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
