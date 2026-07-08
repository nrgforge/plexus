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


def ollama_preflight(report):
    """Walk/run need Ollama + nomic-embed-text + llm-orc. Skip loudly if absent."""
    if not shutil.which("llm-orc"):
        report.skip("preflight", "llm-orc not on PATH")
        return False
    try:
        with urllib.request.urlopen("http://localhost:11434/api/tags", timeout=3) as r:
            models = [m["name"] for m in json.load(r).get("models", [])]
    except Exception as e:
        report.skip("preflight", f"Ollama unreachable: {e}")
        return False
    if not any(m.startswith("nomic-embed-text") for m in models):
        report.skip("preflight", f"nomic-embed-text not pulled (have: {models})")
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
    """Pins the 2026-04-29 multi-process cache-staleness finding.

    EXPECTED TO FAIL until a cache-invalidation contract exists. The
    harness inverts the exit code: reproducing the bug is 'success',
    and this scenario going green means the bug got fixed (update it
    to a plain assertion then).
    """
    report = Report("stale (expected-fail repro)")
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
        bug_reproduced = seen_by_a < disk
        report.check(
            "staleness bug reproduces (A blind to B's writes)",
            bug_reproduced,
            f"A sees {seen_by_a}/{disk}" + ("" if bug_reproduced else " — bug seems FIXED; flip this assertion"),
        )
    finally:
        a.close()
    return report.emit()


SCENARIOS = {
    "crawl": scenario_crawl,
    "walk": scenario_walk,
    "run": scenario_run,
    "stale": scenario_stale,
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
