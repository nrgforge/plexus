# Citation Audit: Essay 001 — Query Surface Design

**Audited document:** `/Users/nathangreen/Development/plexus/docs/essays/001-query-surface-design.md`
**Research log:** `/Users/nathangreen/Development/plexus/docs/essays/research-logs/research-log.md`
**Date:** 2026-03-23
**Auditor:** Citation audit pass (automated + web verification)

---

## Summary

- **Total references checked:** 7
- **Verified as existing:** 7
- **Author attribution errors:** 2 (P1)
- **Factual claim issues:** 1 (P2)
- **Missing citation support:** 3 (P2)
- **Missing seminal works:** 2 (P3)
- **Total issues found:** 8

---

## P1 Issues — Must Fix

### P1-01: Wrong author attribution for the Zep/Graphiti paper

**Location:** References section (final line referencing Ramirez et al., 2025); also implicitly in the abstract ("Ramirez et al., 2025" is the only author attribution given anywhere in the essay for this work)

**Claim:** The essay cites the Zep paper as "(Ramirez et al., 2025)."

**Finding:** The paper arxiv:2501.13956 has five authors: Preston Rasmussen, Pavlo Paliychuk, Travis Beauvais, Jack Ryan, and Daniel Chalef. There is no author named "Ramirez" on this paper. The lead author is Preston **Rasmussen**, not Ramirez. This is a name-confusion error, not a near-miss — the surname is wrong.

**Recommendation:** Change "(Ramirez et al., 2025)" to "(Rasmussen et al., 2025)" everywhere it appears. The reference line should read:

> Zep/Graphiti temporal knowledge graph architecture: [Zep: A Temporal Knowledge Graph Architecture for Agent Memory](https://arxiv.org/abs/2501.13956) (Rasmussen et al., 2025)

---

### P1-02: Wrong author attribution for the provenance survey

**Location:** References section — "Provenance-aware knowledge representation survey: ... (Dividino et al., 2020)"

**Claim:** The essay attributes the 2020 Springer provenance survey to "Dividino et al., 2020."

**Finding:** The paper at https://link.springer.com/article/10.1007/s41019-020-00118-0 is authored by Leslie F. Sikos and Dean Philp, published in *Data Science and Engineering* (Springer), May 2020. "Dividino" is the author of a separate and older paper — "Querying for Provenance, Trust, Uncertainty and Other Meta Knowledge in RDF" (Dividino, Sizov, Staab, Schueler, 2009, Journal of Web Semantics). These are two distinct works. The research log also cites the Springer survey correctly (by URL) but does not assign an author name to it, suggesting the "Dividino et al." attribution was added during essay drafting without verification.

**Recommendation:** Change "(Dividino et al., 2020)" to "(Sikos and Philp, 2020)." The reference line should read:

> Provenance-aware knowledge representation survey: [Provenance-Aware Knowledge Representation: A Survey of Data Models and Contextualized Knowledge Graphs](https://link.springer.com/article/10.1007/s41019-020-00118-0) (Sikos and Philp, 2020)

---

## P2 Issues — Should Address

### P2-01: Federation paper author attribution not verified in essay; research log gives different author

**Location:** References section — "Knowledge graph federation: ... (Endris et al., 2021)"

**Claim:** The essay attributes the Springer chapter on KG federations to "Endris et al., 2021."

**Finding:** The chapter at https://link.springer.com/chapter/10.1007/978-981-16-0479-9_6 is authored by Xiang Zhao, published in the APWeb-WAIM 2020 International Workshops proceedings (Springer, 2021, CCIS vol. 1373). Multiple independent searches found no author named "Endris" associated with this specific chapter. The research log cites this work by URL only, with no author name, so the "Endris et al." attribution appears to have been introduced without a source in the essay-writing step.

The name "Endris" may be confused with Kemele M. Endris, who has published on federated SPARQL query processing over knowledge graphs — a related but distinct body of work. If the intent was to cite Endris's federation work, that is a different paper than the one linked.

**Recommendation:** Either correct the author attribution to "(Zhao, 2021)" for the linked chapter, or replace the reference with the Endris et al. federated query work if that is the intended citation (and provide the correct URL for that paper). Do not leave a misattributed author on a verifiable reference.

---

### P2-02: Claim about Graphiti requiring "no LLM calls at query time" needs qualification

**Location:** "The Write-Heavy / Query-Light Hypothesis" section, paragraph 1 — "No LLM calls are required during retrieval."

**Claim:** Graphiti requires no LLM calls at query time.

**Finding:** This claim is substantially accurate for the core retrieval path, but it is slightly overstated. Graphiti's documentation confirms the base retrieval uses BM25, cosine similarity search, and BFS traversal combined via Reciprocal Rank Fusion. However, Graphiti also supports optional cross-encoder LLM reranking at query time. The no-LLM claim applies to the default path, not to all possible query configurations. The Zep paper (arxiv:2501.13956) itself describes the reranking as optional, so the claim is defensible but presents one configuration as the whole system.

**Recommendation:** Add a qualifier: "No LLM calls are required during retrieval in the default configuration." This is a minor precision issue but matters because the essay uses this claim as foundational evidence for the write-heavy/query-light pattern.

---

### P2-03: "Evidence diversity" framing attributed to domain model without citation

**Location:** "Evidence Diversity as a Derived Signal" section — the quote "Four different kinds of evidence are more trustworthy than a hundred of the same kind."

**Claim:** The essay presents this as a domain model principle ("The domain model states the principle directly"). The quoted sentence appears in the essay body as supporting evidence for evidence diversity as a design concept.

**Finding:** This quote is drawn from internal Plexus project documentation (the domain model), not from any external source. That is appropriate — it is presented as an internal design principle, not an external citation. However, the essay gives no citation or document reference for where in the domain model this appears. A reader cannot locate or verify the source.

**Recommendation:** Add a parenthetical reference to the specific document: "(Plexus domain model, `docs/domain-model.md`)." This is an internal citation but should be traceable.

---

### P2-04: "PaCE model" referenced in research log but not cited in essay

**Location:** Research log, Question 2 findings — "The PaCE approach tracks multiple sources stating the same claim and infers confidence from corroboration."

**Claim:** The research log describes the PaCE provenance model as supporting the evidence diversity concept, and the essay draws on this research to make claims about provenance-aware querying.

**Finding:** The essay relies on the research log's summary of provenance literature to support its provenance-scoped querying section, but the PaCE model is not mentioned by name or cited in the essay itself. The only citation in that section points to the Sikos and Philp 2020 survey. If the essay's argument about evidence diversity is strengthened by the PaCE model specifically, that model should be cited directly. The research log notes the source as the Sikos/Philp survey URL, so the underlying citation is present — but the specific concept (PaCE) is attributed in a way that cannot be independently verified from the essay alone.

**Recommendation:** Either identify and cite the specific PaCE paper referenced in the research log, or remove the implicit reliance on PaCE and rely only on the Sikos and Philp survey, which is cited. The current state creates an unverifiable link between a named model and a cited source.

---

## P3 Issues — Consider

### P3-01: Kaskade not cited in the essay despite being used in research

**Location:** Not present in essay; appears in research log as a source that informed the projection concept.

**Finding:** The research log lists Kaskade (arxiv:1906.05162, Trindade et al., 2019) as a relevant source on graph views, and the essay's "Projection Layer" section directly discusses graph view concepts. The Kaskade paper is included in the essay's References section with a correct URL and year, but it is not cited inline anywhere in the essay body. The essay cites Neo4j GDS and MV4PG when discussing projections, but Kaskade — which is actually a closer academic analog to the "named, reusable view" concept — is only listed in the bibliography without being referenced in the text.

**Recommendation:** Either add an inline reference to Kaskade in the "Projection Layer" section when discussing named graph views for analytics, or remove it from the references section if the essay does not need it. An uncited reference in a bibliography is either an orphan or a missing inline citation.

### P3-02: Abu-Salih citation year is ambiguous

**Location:** References section — "Domain-specific Knowledge Graphs survey: ... (Abu-Salih, 2021)"

**Finding:** The paper at arxiv:2011.00235 was submitted to arXiv in October/November 2020 (arXiv ID prefix 2011 = November 2020) and last revised in March 2021. The journal publication in *Journal of Network and Computer Applications* (vol. 185, April 2021) is the canonical published version with a 2021 date. The 2021 citation year is defensible for the journal publication but should be consistent. The essay cites the arXiv URL, which technically corresponds to a 2020 preprint, while the year "2021" corresponds to the journal publication. This is a minor inconsistency between the linked resource and the cited year.

**Recommendation:** Either link to the journal DOI (https://doi.org/10.1016/j.jnca.2021.103076) with the 2021 year, or link to arXiv and note both years. As written, the 2021 year is acceptable if the intent is the published version, but the arXiv URL points to the preprint. Choose one and be consistent.

---

## Verified Without Issues

The following references were confirmed to exist with accurate titles, URLs, and (where not flagged above) correct author attributions:

| Reference | Status |
|---|---|
| arxiv:2501.13956 — Zep/Graphiti (title and URL correct; author wrong — see P1-01) | URL verified |
| Neo4j GDS projections documentation | URL verified, content matches claims |
| arxiv:2411.18847 — MV4PG (Chaijun Xu et al., 2024) | URL verified, speedup figures accurate (28.71x workload, ~100x single query) |
| link.springer.com/article/10.1007/s41019-020-00118-0 — provenance survey (URL correct; author wrong — see P1-02) | URL verified |
| arxiv:2011.00235 — Abu-Salih domain-specific KG survey | URL verified, author correct |
| link.springer.com/chapter/10.1007/978-981-16-0479-9_6 — KG federations (URL correct; author questionable — see P2-01) | URL verified |
| arxiv:1906.05162 — Kaskade (Trindade et al., 2019) | URL verified, authors confirmed |

---

## Overall Assessment

The essay's core argument is well-supported by the cited sources. The write-heavy/query-light pattern is accurately described from the Zep/Graphiti paper. The MV4PG performance figures are accurate. The Neo4j GDS projection description matches the documentation. The research log provides a faithful basis for the essay's claims.

Two P1 author attribution errors — "Ramirez" for "Rasmussen" (Zep paper) and "Dividino" for "Sikos and Philp" (provenance survey) — are factual mistakes that misattribute authorship to real researchers who did not write these works. Both must be corrected before the essay is used as a reference in downstream phases. The federation paper author attribution (P2-01) is likely also wrong and should be verified and corrected.

The factual claims about Graphiti's architecture are accurate in substance, with a minor overstatement about LLM absence at query time that warrants a qualifier.

No cited works appear to be hallucinated. All seven URLs resolve to real papers or documentation pages.
