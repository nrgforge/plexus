//! Enrichment loop: runs enrichments after a primary emission (ADR-010).
//!
//! Extracted from engine_sink.rs to resolve the logical dependency:
//! the loop imports from both `engine_sink` (for `emit_inner`) and
//! `enrichment` (for `EnrichmentRegistry`).

use crate::adapter::sink::{EngineSink, FrameworkContext, AdapterError, EmitResult};
use super::traits::EnrichmentRegistry;
use crate::graph::events::GraphEvent;
use crate::graph::{ContextId, PlexusEngine};

/// Enrichment loop telemetry: result plus convergence metadata.
pub(crate) struct EnrichmentLoopResult {
    pub result: EmitResult,
    /// Number of enrichment rounds executed.
    pub rounds: usize,
    /// True if terminated by quiescence, false if safety valve.
    pub quiesced: bool,
}

/// Run the enrichment loop after a primary emission (ADR-010).
///
/// Per-round events: each round sees only events from the previous round.
/// All enrichments in a round see the same context snapshot.
/// The loop terminates when all enrichments return None (quiescence)
/// or the safety valve (max rounds) is reached.
///
/// Returns an EnrichmentLoopResult with the accumulated EmitResult
/// plus convergence telemetry (rounds, quiesced).
pub(crate) fn run_enrichment_loop(
    engine: &PlexusEngine,
    context_id: &ContextId,
    registry: &EnrichmentRegistry,
    trigger_events: &[GraphEvent],
) -> Result<EnrichmentLoopResult, AdapterError> {
    let mut accumulated = EmitResult::empty();
    let mut round_events: Vec<GraphEvent> = trigger_events.to_vec();
    let mut round = 0;
    let mut quiesced = false;

    while round < registry.max_rounds() && !round_events.is_empty() {
        // Snapshot the context (clone for consistent, immutable view)
        let snapshot = engine.get_context(context_id)
            .ok_or_else(|| AdapterError::ContextNotFound(context_id.to_string()))?;

        // Run all enrichments with the same snapshot
        let mut round_emissions: Vec<(String, crate::adapter::types::Emission)> = Vec::new();
        for enrichment in registry.enrichments() {
            if let Some(emission) = enrichment.enrich(&round_events, &snapshot) {
                round_emissions.push((enrichment.id().to_string(), emission));
            }
        }

        // Quiescence: all enrichments returned None
        if round_emissions.is_empty() {
            quiesced = true;
            break;
        }

        // Commit each enrichment's emission through the same path
        let mut new_events: Vec<GraphEvent> = Vec::new();
        for (enrichment_id, emission) in round_emissions {
            let enrichment_framework = Some(FrameworkContext {
                adapter_id: enrichment_id,
                context_id: context_id.to_string(),
                input_summary: None,
            });

            let enrichment_result = engine.with_context_mut(context_id, |ctx| {
                EngineSink::emit_inner(ctx, emission, &enrichment_framework)
            }).map_err(EngineSink::map_engine_error)??;

            new_events.extend(enrichment_result.events.clone());

            // Accumulate enrichment results
            accumulated.nodes_committed += enrichment_result.nodes_committed;
            accumulated.edges_committed += enrichment_result.edges_committed;
            accumulated.removals_committed += enrichment_result.removals_committed;
            accumulated.edge_removals_committed += enrichment_result.edge_removals_committed;
            accumulated.rejections.extend(enrichment_result.rejections);
            accumulated.provenance.extend(enrichment_result.provenance);
            accumulated.events.extend(enrichment_result.events);
        }

        round_events = new_events;
        round += 1;

        // Also quiesced if no new events were produced
        if round_events.is_empty() {
            quiesced = true;
        }
    }

    if !quiesced {
        tracing::warn!(
            rounds = registry.max_rounds(),
            "enrichment loop aborted (safety valve)"
        );
    }

    Ok(EnrichmentLoopResult {
        result: accumulated,
        rounds: round,
        quiesced,
    })
}
