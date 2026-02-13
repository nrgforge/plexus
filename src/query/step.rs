//! Typed multi-hop traversal query (ADR-013).
//!
//! `StepQuery` is a sequential chain where each step specifies its own
//! relationship filter and direction. Each step operates on the previous
//! step's frontier — the nodes discovered by the prior step.

use std::collections::HashMap;

use crate::{Context, Edge, Node, NodeId};

use super::types::Direction;

/// A single traversal step: direction + relationship filter.
#[derive(Debug, Clone)]
struct Step {
    direction: Direction,
    relationship: String,
}

/// Builder for typed multi-hop traversals.
#[derive(Debug, Clone)]
pub struct StepQuery {
    origin: NodeId,
    steps: Vec<Step>,
}

/// Result of a StepQuery execution.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Origin node ID.
    pub origin: NodeId,
    /// Nodes discovered at each step level.
    /// `steps[0]` = nodes found by the first step, etc.
    pub steps: Vec<Vec<Node>>,
    /// All edges traversed across all steps.
    pub edges: Vec<Edge>,
}

impl StepQuery {
    /// Start a new query from the given origin node.
    pub fn from(origin_id: impl Into<NodeId>) -> Self {
        Self {
            origin: origin_id.into(),
            steps: Vec::new(),
        }
    }

    /// Add a traversal step: follow edges with the given direction and relationship.
    pub fn step(mut self, direction: Direction, relationship: impl Into<String>) -> Self {
        self.steps.push(Step {
            direction,
            relationship: relationship.into(),
        });
        self
    }

    /// Execute the query against a context.
    pub fn execute(&self, context: &Context) -> StepResult {
        let index = EdgeIndex::build(context);
        let mut result = StepResult {
            origin: self.origin.clone(),
            steps: Vec::new(),
            edges: Vec::new(),
        };

        let mut frontier: Vec<NodeId> = vec![self.origin.clone()];

        for step in &self.steps {
            let mut next_frontier = Vec::new();
            let mut step_nodes = Vec::new();

            for node_id in &frontier {
                let candidates = match step.direction {
                    Direction::Outgoing => index.outgoing(node_id),
                    Direction::Incoming => index.incoming(node_id),
                    Direction::Both => {
                        let mut all = index.outgoing(node_id);
                        all.extend(index.incoming(node_id));
                        all
                    }
                };

                for edge in candidates {
                    if edge.relationship != step.relationship {
                        continue;
                    }

                    let reached = match step.direction {
                        Direction::Outgoing => &edge.target,
                        Direction::Incoming => &edge.source,
                        Direction::Both => {
                            if &edge.source == node_id {
                                &edge.target
                            } else {
                                &edge.source
                            }
                        }
                    };

                    if let Some(node) = context.get_node(reached) {
                        if !next_frontier.contains(reached) {
                            next_frontier.push(reached.clone());
                            step_nodes.push(node.clone());
                        }
                        result.edges.push(edge.clone());
                    }
                }
            }

            result.steps.push(step_nodes);
            frontier = next_frontier;
        }

        result
    }
}

impl StepResult {
    /// Get nodes discovered at a specific step (0-indexed).
    pub fn at_step(&self, step: usize) -> &[Node] {
        self.steps.get(step).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all discovered nodes across all steps.
    pub fn all_nodes(&self) -> Vec<&Node> {
        self.steps.iter().flat_map(|s| s.iter()).collect()
    }
}

// --- Evidence trail: composite query over two StepQuery branches ---

/// Result of an evidence trail query for a concept.
#[derive(Debug, Clone)]
pub struct EvidenceTrailResult {
    /// The concept node ID queried.
    pub concept: NodeId,
    /// Marks that reference the concept (via `references` edges).
    pub marks: Vec<Node>,
    /// Fragments tagged with the concept (via `tagged_with` edges).
    pub fragments: Vec<Node>,
    /// Chains containing the discovered marks (via `contains` edges).
    pub chains: Vec<Node>,
    /// All edges traversed across both branches.
    pub edges: Vec<Edge>,
}

/// Query the evidence trail for a concept: marks, fragments, and chains.
///
/// Composes two StepQuery branches:
/// - Branch 1: concept ← references ← contains (marks → chains)
/// - Branch 2: concept ← tagged_with (fragments)
pub fn evidence_trail(concept_id: impl Into<NodeId>, context: &Context) -> EvidenceTrailResult {
    let concept = concept_id.into();

    // Branch 1: marks referencing the concept, then chains containing those marks
    let branch1 = StepQuery::from(concept.clone())
        .step(Direction::Incoming, "references")
        .step(Direction::Incoming, "contains")
        .execute(context);

    // Branch 2: fragments tagged with the concept
    let branch2 = StepQuery::from(concept.clone())
        .step(Direction::Incoming, "tagged_with")
        .execute(context);

    let marks = branch1.at_step(0).to_vec();
    let chains = branch1.at_step(1).to_vec();
    let fragments = branch2.at_step(0).to_vec();

    let mut edges = branch1.edges;
    edges.extend(branch2.edges);

    EvidenceTrailResult {
        concept,
        marks,
        fragments,
        chains,
        edges,
    }
}

// --- Edge index for efficient lookups ---

struct EdgeIndex<'a> {
    outgoing: HashMap<&'a NodeId, Vec<&'a Edge>>,
    incoming: HashMap<&'a NodeId, Vec<&'a Edge>>,
}

impl<'a> EdgeIndex<'a> {
    fn build(context: &'a Context) -> Self {
        let mut outgoing: HashMap<&NodeId, Vec<&Edge>> = HashMap::new();
        let mut incoming: HashMap<&NodeId, Vec<&Edge>> = HashMap::new();

        for edge in &context.edges {
            outgoing.entry(&edge.source).or_default().push(edge);
            incoming.entry(&edge.target).or_default().push(edge);
        }

        Self { outgoing, incoming }
    }

    fn outgoing(&self, node_id: &NodeId) -> Vec<&'a Edge> {
        self.outgoing.get(node_id).cloned().unwrap_or_default()
    }

    fn incoming(&self, node_id: &NodeId) -> Vec<&'a Edge> {
        self.incoming.get(node_id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dimension, ContentType, Context, Edge, Node, NodeId};

    /// Create a node with a specific ID, type, and dimension.
    fn node(id: &str, node_type: &str, dim: &str) -> Node {
        let mut n = Node::new_in_dimension(node_type, ContentType::Provenance, dim);
        n.id = NodeId::from(id);
        n
    }

    /// Insert a node into a context by ID.
    fn add(ctx: &mut Context, n: Node) {
        ctx.nodes.insert(n.id.clone(), n);
    }

    fn test_context() -> Context {
        let mut ctx = Context::new("test");

        add(&mut ctx, node("concept:travel", "concept", dimension::SEMANTIC));
        add(&mut ctx, node("mark:1", "mark", dimension::PROVENANCE));
        add(&mut ctx, node("mark:2", "mark", dimension::PROVENANCE));

        // references edges: mark → concept (outgoing from mark, incoming to concept)
        ctx.edges.push(Edge::new_cross_dimensional(
            NodeId::from("mark:1"), dimension::PROVENANCE,
            NodeId::from("concept:travel"), dimension::SEMANTIC,
            "references",
        ));
        ctx.edges.push(Edge::new_cross_dimensional(
            NodeId::from("mark:2"), dimension::PROVENANCE,
            NodeId::from("concept:travel"), dimension::SEMANTIC,
            "references",
        ));

        ctx
    }

    // === Scenario 1: Single-step traversal follows relationship and direction ===
    #[test]
    fn single_step_incoming_references() {
        let ctx = test_context();

        let result = StepQuery::from("concept:travel")
            .step(Direction::Incoming, "references")
            .execute(&ctx);

        assert_eq!(result.at_step(0).len(), 2);
        let ids: Vec<String> = result.at_step(0).iter().map(|n| n.id.to_string()).collect();
        assert!(ids.contains(&"mark:1".to_string()));
        assert!(ids.contains(&"mark:2".to_string()));
        assert_eq!(result.edges.len(), 2);
    }

    // === Scenario 2: Multi-step traversal chains through frontiers ===
    #[test]
    fn multi_step_chains_through_frontiers() {
        let mut ctx = test_context();

        // Add a chain that contains mark:1
        add(&mut ctx, node("chain:provenance:research", "chain", dimension::PROVENANCE));
        ctx.edges.push(Edge::new(
            NodeId::from("chain:provenance:research"),
            NodeId::from("mark:1"),
            "contains",
        ));

        let result = StepQuery::from("concept:travel")
            .step(Direction::Incoming, "references")
            .step(Direction::Incoming, "contains")
            .execute(&ctx);

        // Step 0: marks that reference the concept
        assert_eq!(result.at_step(0).len(), 2);
        // Step 1: chains that contain those marks
        assert_eq!(result.at_step(1).len(), 1);
        assert_eq!(result.at_step(1)[0].id.to_string(), "chain:provenance:research");
        // All edges collected
        assert_eq!(result.edges.len(), 3); // 2 references + 1 contains
    }

    // === Scenario 3: Step with no matching edges produces empty frontier ===
    #[test]
    fn step_with_no_matching_edges_empty_frontier() {
        let ctx = test_context();

        let result = StepQuery::from("concept:travel")
            .step(Direction::Incoming, "tagged_with")
            .execute(&ctx);

        assert_eq!(result.at_step(0).len(), 0);
        assert_eq!(result.edges.len(), 0);
    }

    // === Scenario 4: Step filters by relationship type ===
    #[test]
    fn step_filters_by_relationship() {
        let mut ctx = test_context();

        // Add a fragment with a tagged_with edge to the same concept
        add(&mut ctx, node("fragment:abc", "fragment", dimension::STRUCTURE));
        ctx.edges.push(Edge::new_cross_dimensional(
            NodeId::from("fragment:abc"), dimension::STRUCTURE,
            NodeId::from("concept:travel"), dimension::SEMANTIC,
            "tagged_with",
        ));

        let result = StepQuery::from("concept:travel")
            .step(Direction::Incoming, "references")
            .execute(&ctx);

        // Only marks (via references), not fragments (via tagged_with)
        assert_eq!(result.at_step(0).len(), 2);
        let ids: Vec<String> = result.at_step(0).iter().map(|n| n.id.to_string()).collect();
        assert!(!ids.contains(&"fragment:abc".to_string()));
    }

    // === Scenario 5: StepQuery preserves per-step structure ===
    #[test]
    fn preserves_per_step_structure() {
        let mut ctx = test_context();

        add(&mut ctx, node("chain:provenance:research", "chain", dimension::PROVENANCE));
        ctx.edges.push(Edge::new(
            NodeId::from("chain:provenance:research"),
            NodeId::from("mark:1"),
            "contains",
        ));

        let result = StepQuery::from("concept:travel")
            .step(Direction::Incoming, "references")
            .step(Direction::Incoming, "contains")
            .execute(&ctx);

        // Steps are not flattened — step 0 and step 1 are distinct
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.at_step(0).len(), 2); // marks
        assert_eq!(result.at_step(1).len(), 1); // chains
    }

    // === Scenario 6: StepQuery supports Outgoing direction ===
    #[test]
    fn outgoing_direction() {
        let mut ctx = test_context();

        add(&mut ctx, node("chain:provenance:research", "chain", dimension::PROVENANCE));
        ctx.edges.push(Edge::new(
            NodeId::from("chain:provenance:research"),
            NodeId::from("mark:1"),
            "contains",
        ));
        ctx.edges.push(Edge::new(
            NodeId::from("chain:provenance:research"),
            NodeId::from("mark:2"),
            "contains",
        ));

        let result = StepQuery::from("chain:provenance:research")
            .step(Direction::Outgoing, "contains")
            .execute(&ctx);

        assert_eq!(result.at_step(0).len(), 2);
        let ids: Vec<String> = result.at_step(0).iter().map(|n| n.id.to_string()).collect();
        assert!(ids.contains(&"mark:1".to_string()));
        assert!(ids.contains(&"mark:2".to_string()));
    }

    // === Evidence trail scenarios ===

    fn evidence_context() -> Context {
        let mut ctx = Context::new("test");

        add(&mut ctx, node("concept:travel", "concept", dimension::SEMANTIC));
        add(&mut ctx, node("mark:1", "mark", dimension::PROVENANCE));
        add(&mut ctx, node("fragment:abc", "fragment", dimension::STRUCTURE));
        add(&mut ctx, node("chain:provenance:research", "chain", dimension::PROVENANCE));

        // mark:1 → concept:travel (references)
        ctx.edges.push(Edge::new_cross_dimensional(
            NodeId::from("mark:1"), dimension::PROVENANCE,
            NodeId::from("concept:travel"), dimension::SEMANTIC,
            "references",
        ));
        // fragment:abc → concept:travel (tagged_with)
        ctx.edges.push(Edge::new_cross_dimensional(
            NodeId::from("fragment:abc"), dimension::STRUCTURE,
            NodeId::from("concept:travel"), dimension::SEMANTIC,
            "tagged_with",
        ));
        // chain:provenance:research → mark:1 (contains)
        ctx.edges.push(Edge::new(
            NodeId::from("chain:provenance:research"),
            NodeId::from("mark:1"),
            "contains",
        ));

        ctx
    }

    // === Scenario 7: Evidence trail returns marks, fragments, and chains ===
    #[test]
    fn evidence_trail_returns_all_evidence() {
        let ctx = evidence_context();

        let result = evidence_trail("concept:travel", &ctx);

        assert_eq!(result.marks.len(), 1);
        assert_eq!(result.marks[0].id.to_string(), "mark:1");
        assert_eq!(result.fragments.len(), 1);
        assert_eq!(result.fragments[0].id.to_string(), "fragment:abc");
        assert_eq!(result.chains.len(), 1);
        assert_eq!(result.chains[0].id.to_string(), "chain:provenance:research");
        // 1 references + 1 contains + 1 tagged_with
        assert_eq!(result.edges.len(), 3);
    }

    // === Scenario 8: Evidence trail with no evidence returns empty ===
    #[test]
    fn evidence_trail_empty_for_isolated_concept() {
        let mut ctx = Context::new("test");
        add(&mut ctx, node("concept:obscure", "concept", dimension::SEMANTIC));

        let result = evidence_trail("concept:obscure", &ctx);

        assert_eq!(result.marks.len(), 0);
        assert_eq!(result.fragments.len(), 0);
        assert_eq!(result.chains.len(), 0);
        assert_eq!(result.edges.len(), 0);
    }

    // === Scenario 9: Evidence trail composes two StepQuery branches ===
    #[test]
    fn evidence_trail_composes_two_branches() {
        let ctx = evidence_context();

        let result = evidence_trail("concept:travel", &ctx);

        // Branch 1 found marks and chains; branch 2 found fragments.
        // If these were a single query, fragments would need to be at step 0
        // alongside marks, which would lose the type distinction.
        // The composite nature is verified by having all three categories populated.
        assert!(!result.marks.is_empty(), "branch 1 step 0: marks");
        assert!(!result.chains.is_empty(), "branch 1 step 1: chains");
        assert!(!result.fragments.is_empty(), "branch 2 step 0: fragments");
    }
}
