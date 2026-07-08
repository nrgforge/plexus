# Plexus Vision

*Written at RDD graduation (2026-07-07), distilled from product discovery,
interaction specs, and the 2026-07-07 alignment conversation. This is the
north star for post-RDD engineering work. Full product-discovery and
interaction-specs artifacts are preserved at `docs/archive/`.*

## The vision

**Multiple client applications ingest data into shared contexts, each with
its own lens, and query through those lenses against cross-pollinated
data.**

Unpacked, that is five claims:

1. **Multiple clients** — independent consumer applications (a creative
   writing tool, a research aggregator, a code analyzer) run as separate
   processes with their own lifecycles.
2. **Shared contexts** — they ingest into the same context. Apps are
   *lenses on shared material*, not containers of their own data. An app
   that hoards its data cuts itself off from cross-pollination and defeats
   the point.
3. **Own lenses** — each consumer loads a spec declaring its adapter,
   lens, and enrichment config. The lens writes vocabulary edges
   (`lens:{consumer}:{relationship}`) translating graph structure into the
   consumer's language. Lens output is public (Invariant 56): each
   consumer's lens makes the graph richer for everyone.
4. **Cross-pollinated data** — structure emerges *between* consumers'
   contributions: embedding similarity bridging one app's fragments to
   another's research notes; concept-identity convergence when both
   mention the same idea; co-occurrence over derived tags. Plexus's value
   proposition is surfacing structure the user did not encode — anything
   less is tautological.
5. **Querying through lenses** — a consumer pulls on its own schedule and
   receives signals in its own vocabulary about material it did not
   author, with provenance explaining why each connection exists and how
   corroborated it is.

The consumer's mental model (validated through two RDD cycles): *"I own my
spec. I load it onto a context and Plexus wires everything. My vocabulary
layer is graph data, not configuration — it persists and keeps translating
other consumers' ingests even when I disconnect. When I query, the graph
already speaks my language."*

## Where the implementation stands (2026-07-07)

**Plexus is multi-consumer at rest, but single-consumer at runtime.** The
data model was built for the vision and holds up under test: shared
contexts, namespaced vocabulary layers, provenance-attributed evidence,
corroboration, event cursors. The runtime story is where the gaps live:

- ~~Two long-lived processes on the same SQLite do not see each other's
  writes~~ **Fixed 2026-07-07** (ADR-017 §2 conformance, see M0 below;
  `play.py stale` is now the positive regression test).
- Lenses do not fire on background-phase emissions (deep semantic
  extraction), while core enrichments do.
- Cross-pollination itself — the central claim — has never been
  demonstrated end to end. The substrate works; the composition is
  unvalidated.
- Signal actionability is thin: similarity scores cannot reach edge
  weights, and batch ingest saturates temporal signals.

None of this is accidental damage; each cycle scoped a surface and
delivered it. The runtime model simply never had its turn. It has it now.

## Milestones

**M0 — Decide the runtime model. ✅ RESOLVED 2026-07-07.** The
consistency matrix (`play.py matrix`, issue #1) showed: restart topology
fully correct, writes already concurrency-safe (incremental upsert,
ADR-017 §3), `changes_since` already store-direct — and every
cache-backed read STALE in the concurrent topology. Root cause was
ADR-017 §2 non-conformance: `reload_if_changed()` (data_version check)
existed, unit-tested, but was never called from consumer-facing paths.
Fixed by wiring it into `PlexusApi` name resolution (commit `d97e5de`);
all 18 matrix cells now LIVE. **The runtime model is: shared SQLite,
library rule intact, per-read data_version coherence — no daemon.**
Long-lived multi-process consumers are now legal, which unblocks M1.

**M1 — Prove the flywheel.** Engineer an overlap corpus (content with
deliberate latent bridges — the existing fixture corpora were curated for
*separability*). Then two validations:

- *Flywheel scenario:* two consumers, one shared context, own lenses;
  assert (via `evidence_trail` + `contributor_ids`) that consumer A
  receives a signal in A's vocabulary about content B ingested. Exercise
  both cross-pollination mechanisms separately — embedding proximity and
  concept-identity convergence via extraction.
- *Solo-vs-shared differential:* run consumer A's corpus alone, then
  A+B shared; diff A's lens-query results. The delta is the marginal
  value of cohabitation — the closest thing the vision has to a KPI.

**M2 — Close the gaps M1 exposes.** Candidates, ordered by what the
flywheel shows matters: lens coverage over background extraction, the
extraction input contract (SpaCy envelope defect), similarity-into-weight
plumbing, corroboration-aware lens translation.

**M3 — Blinded-consumer vocabulary probes.** LLM agents as consumers,
each given only its own domain vocabulary and the MCP query tools, tasked
with domain questions over the populated cross-consumer graph. Instrument
whether the lens surface suffices or agents fall back to raw graph
vocabulary. This is the honest test of "without learning graph internals,"
and the first non-builder evidence bearing on the parked phenomenology
hypothesis (named vs. structural lens registers).

## Engineering queue

Tracked as GitHub issues (created at graduation). The seven items, in
brief: runtime consistency model (M0); CLI surface scope; extraction input
contract; lens saturation / corroboration-aware translation; background
enrichment/lens phase map; TemporalProximity semantics under batch ingest;
similarity-into-weight plumbing.

## Standing tensions worth remembering

Carried from product discovery — not problems to fix, but tensions to
keep balancing:

- **Easy-to-demo vs. honest-to-demo.** The minimum *mechanism* setup
  (tagged content + co-occurrence) is not the minimum *value* setup
  (untagged content + extraction or embeddings producing structure the
  user didn't encode). Onboarding must not conflate them.
- **Scope vs. serendipity.** Lenses translate rather than filter
  precisely so cross-domain discovery survives contact with per-consumer
  vocabulary. Untranslatable connections remain a design question.
- **Interpretive vs. structural lens registers, per-job not per-app.**
  Empirically confirmed topology-invariant (harness run scenario,
  2026-07-07); the phenomenology claim stays a hypothesis until M3.
- **Library autonomy vs. shared infrastructure.** The library rule
  (Invariant 41) is load-bearing for consumer trust; M0's runtime decision
  must not quietly convert Plexus into a mandatory server.

## Hypotheses parked for future work

Preserved with full context in `docs/archive/cycle-status-default-install-lens-design.md`:

- **Lens-as-grammar** — a lens as composition rules and
  query-expectation contracts, not just vocabulary. Precondition met (the
  tautology threshold has been crossed with real emergent content);
  opening belief-mapping question recorded in the archived cycle status.
- **Node-level reinforcement** (companion to ADR-003) — what accumulates
  at nodes on re-emission, and per-property merge policy on upsert.
- **Query-begets-ingestion feedback loop** — lens signals as prompts
  whose responses re-enter the graph; validation needs a live consumer.
