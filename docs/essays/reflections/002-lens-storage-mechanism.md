# Reflection: Lens Storage Mechanism Spike

The spike began with three architecturally distinct options — first-class edges, per-consumer index tables, query-time mapping — and converged on the simplest: a lens is just another enrichment producing standard edges. The answer was already in the codebase.

Two observations from the gate exchange:

1. **Simplicity as validation signal.** The user's immediate reaction was appreciation for elegance. When a spike resolves a question that felt like it needed new infrastructure by showing the existing infrastructure is sufficient, that is a stronger signal than finding a clever new mechanism. The enrichment loop was designed to be general; the lens validates that generality.

2. **Performance as empirical question, not design question.** The user flagged performance curiosity but accepted the design. This is the right separation: the storage mechanism is a design choice (resolved); the scaling behavior under real workloads is an empirical question (deferred to Trellis integration). The essay's identification of OQ-16 (linear edge scan) as the shared pressure point for all enrichments — not a lens-specific concern — reinforces that the lens does not introduce a new scaling dimension.

**Feed-forward for DECIDE:** The declarative lens definition (OQ-20) is now the primary open question. The spike resolved the storage question; the declaration question — how a consumer specifies what the lens translates from and to — shapes the consumer experience. Options surfaced: extend adapter spec YAML with `lens:` section, separate lens spec, or parameterized enrichment in the `enrichments:` section. The user's prior insight ("defined alongside the adapter spec") suggests option (a) or (c).
