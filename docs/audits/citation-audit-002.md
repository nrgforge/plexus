# Citation Audit Report

**Audited document:** `/Users/nathangreen/Development/plexus/docs/essays/002-lens-storage-mechanism.md`
**Research log:** `/Users/nathangreen/Development/plexus/docs/essays/research-logs/research-log.md`
**Date:** 2026-03-25

---

## Summary

- **Total references checked:** 6
- **Verified:** 6 (all cited works exist)
- **Issues found:** 4 (1 P1, 2 P2, 1 P3)

---

## Issues

### P1 — Must Fix

#### Issue 1: Incorrect author initial for Zep paper

- **Location:** References section, final line of the reference list
- **Claim:** `Rasmussen, D., et al. (2025). Zep: A Temporal Knowledge Graph Architecture for Agent Memory. arXiv:2501.13956.`
- **Finding:** The first author is **Preston Rasmussen**, not "D. Rasmussen." No author on this paper has the surname Rasmussen with initial D. The five authors are: Preston Rasmussen, Pavlo Paliychuk, Travis Beauvais, Jack Ryan, and Daniel Chalef. The initial "D." does not correspond to any author in the lead position; Daniel Chalef (fifth author) has the initial D. but is not the lead author.
- **Recommendation:** Correct to `Rasmussen, P., et al. (2025). Zep: A Temporal Knowledge Graph Architecture for Agent Memory. arXiv:2501.13956.`

---

### P2 — Should Fix

#### Issue 2: MV4PG speedup figure is accurate but the framing omits the workload-level number

- **Location:** Section "MV4PG: View Edges as First-Class Graph Elements," second paragraph
- **Claim:** "The measured overhead is O(N) in affected view edges; the measured read speedup is up to ~100x."
- **Finding:** The paper (arXiv:2411.18847) confirms that a single query speedup reaches "up to nearly 100x." This is accurate. However, the paper also reports a workload-level speedup of "up to 28.71x," which is the more conservative and arguably more representative figure for a mixed read/write scenario. The essay selects the peak single-query figure without context, which could overstate the practical benefit.
- **Recommendation:** Revise to distinguish between single-query peak speedup (~100x) and workload-level speedup (~28x), or add a qualifier such as "up to ~100x for individual read queries (workload-level speedup: up to ~28x)."

#### Issue 3: Neo4j GDS reference version is underspecified

- **Location:** References section: `Neo4j Graph Data Science: Projecting graphs. Neo4j Documentation, v2.x.`
- **Finding:** The reference is cited as "v2.x" without a specific version. As of the essay date (2026-03-25), the current release is v2.27. The "v2.x" label is correct in family but imprecise as a citable version. The essay body does not make version-specific claims about Neo4j GDS behavior, so no factual error follows from the imprecision, but the reference is not reproducibly pinnable.
- **Recommendation:** Replace "v2.x" with the specific version current at time of writing, e.g., "v2.27" or add a retrieval date: `Neo4j Graph Data Science: Projecting graphs. Neo4j Documentation, v2.x (current as of 2026-03-25).`

---

### P3 — Consider

#### Issue 4: TinkerPop reference version is current but release timing should be noted

- **Location:** References section: `Apache TinkerPop: SubgraphStrategy, PartitionStrategy. TinkerPop Documentation, v3.8.0.`
- **Finding:** TinkerPop v3.8.0 was released on November 12, 2025, and is confirmed as the current stable release. Both SubgraphStrategy and PartitionStrategy are verified features of TinkerPop's traversal strategy decoration layer. The reference is technically accurate. The version cited (3.8.0) postdates the essay's knowledge base period but predates the essay date (2026-03-25), so it is citable.
- **Recommendation:** No change required. Noting for completeness that a pre-release 4.0.0-beta.1 also exists; the citation to 3.8.0 stable is appropriate.

---

## Verified References

The following references were verified as existing with correct titles and generally correct attribution:

1. **MV4PG (arXiv:2411.18847)** — Title, arXiv ID, and year confirmed. First author is Chaijun Xu; "Xu et al." attribution is correct.
2. **Zep (arXiv:2501.13956)** — Title, arXiv ID, and year confirmed. First author first name is incorrect in the citation (see P1 above).
3. **Neo4j GDS: Projecting graphs** — Documentation page confirmed at neo4j.com/docs/graph-data-science/current/. Version family "v2.x" is accurate.
4. **Apache TinkerPop: SubgraphStrategy, PartitionStrategy, v3.8.0** — Both strategies confirmed. Version 3.8.0 is the current stable release as of 2026-03-25.
5. **Plexus domain model (2026-03-25): Invariants 56–59** — Internal reference; verified against project memory and MEMORY.md.
6. **Plexus ADR-003 and ADR-029** — Internal references; consistent with project memory (ADR-003: contribution tracking and scale normalization; ADR-029: enrichment loop ownership by IngestPipeline).

---

## Factual Claims Checked

The following in-body claims were spot-checked against sources and the codebase:

- **"Read speedup up to ~100x" (MV4PG)** — Confirmed in arXiv:2411.18847 abstract for single-query peak. See Issue 2 regarding context.
- **"Write maintenance cost is O(N) in affected view edges"** — Consistent with the paper's incremental delta template approach; verified as accurate characterization.
- **"SubgraphStrategy injects predicate checks into every traversal step"** — Consistent with TinkerPop documentation description of traversal strategy decoration behavior.
- **"No LLM calls at query time in the default configuration" (Graphiti/Zep)** — Confirmed. The Zep paper (Section 3) describes a retrieval pipeline using cosine similarity, BM25, and BFS traversal. LLM-based cross-encoders are noted as optional and labeled the highest-cost option. The default configuration does not use LLM inference at query time.
- **"Three primitives (cosine similarity, BM25, BFS traversal) combined through reranking"** — Confirmed against arXiv:2501.13956 Section 3.
