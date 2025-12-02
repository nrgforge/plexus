//! Serialization tests with contract-compliant fixtures

use serde_json::{json, Value};

/// Contract fixture: Node from plexus-integration-contract.md
fn contract_node_fixture() -> Value {
    json!({
        "id": "agent:security-reviewer",
        "node_type": "agent",
        "content_type": "agent",
        "properties": {
            "model_profile": "free-local",
            "specialization": "security analysis"
        },
        "metadata": {
            "created_at": "2025-11-30T10:00:00Z",
            "source": "llm-orc:code-review-ensemble"
        }
    })
}

/// Contract fixture: Edge from plexus-integration-contract.md
fn contract_edge_fixture() -> Value {
    json!({
        "id": "edge:security-to-senior",
        "source": "agent:security-reviewer",
        "target": "agent:senior-reviewer",
        "relationship": "depends_on",
        "weight": 1.0,
        "strength": 0.87,
        "confidence": 0.91,
        "reinforcements": [
            {
                "type": "SuccessfulExecution",
                "timestamp": "2025-11-30T10:23:00Z",
                "context_id": "code-review-ensemble",
                "metadata": {
                    "outcome": "Found 3 vulnerabilities"
                }
            }
        ],
        "created_at": "2025-11-29T08:00:00Z",
        "last_reinforced": "2025-11-30T10:23:00Z",
        "properties": {}
    })
}

/// Contract fixture: Context from plexus-integration-contract.md
fn contract_context_fixture() -> Value {
    json!({
        "id": "ctx:llm-orc-workspace",
        "name": "llm-orc-workspace",
        "description": "Knowledge graph for llm-orc multi-agent orchestration",
        "nodes": {},
        "edges": [],
        "metadata": {
            "created_at": "2025-11-29T08:00:00Z",
            "tags": ["orchestration", "agents"]
        }
    })
}

#[cfg(test)]
mod serialization_tests {
    use super::*;
    use crate::graph::{
        context::Context,
        edge::{Edge, Reinforcement, ReinforcementType},
        node::{ContentType, Node, NodeId, PropertyValue},
    };

    #[test]
    fn node_id_serializes_as_string() {
        let id = NodeId::from_string("agent:security-reviewer");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"agent:security-reviewer\"");
    }

    #[test]
    fn node_id_deserializes_from_string() {
        let json = "\"agent:security-reviewer\"";
        let id: NodeId = serde_json::from_str(json).unwrap();
        assert_eq!(id.as_str(), "agent:security-reviewer");
    }

    #[test]
    fn content_type_serializes_lowercase() {
        let ct = ContentType::Agent;
        let json = serde_json::to_string(&ct).unwrap();
        assert_eq!(json, "\"agent\"");

        let ct = ContentType::Code;
        let json = serde_json::to_string(&ct).unwrap();
        assert_eq!(json, "\"code\"");
    }

    #[test]
    fn content_type_deserializes_lowercase() {
        let ct: ContentType = serde_json::from_str("\"agent\"").unwrap();
        assert_eq!(ct, ContentType::Agent);

        let ct: ContentType = serde_json::from_str("\"code\"").unwrap();
        assert_eq!(ct, ContentType::Code);
    }

    #[test]
    fn reinforcement_type_field_renamed() {
        let r = Reinforcement::new(ReinforcementType::SuccessfulExecution);
        let json = serde_json::to_value(&r).unwrap();

        // Should have "type" not "reinforcement_type"
        assert!(json.get("type").is_some());
        assert!(json.get("reinforcement_type").is_none());
        assert_eq!(json["type"], "SuccessfulExecution");
    }

    #[test]
    fn reinforcement_optional_fields_skipped_when_none() {
        let r = Reinforcement::new(ReinforcementType::UserValidation);
        let json = serde_json::to_value(&r).unwrap();

        // Optional fields should not be present when None
        assert!(json.get("context_id").is_none());
        assert!(json.get("metadata").is_none());
    }

    #[test]
    fn reinforcement_roundtrip() {
        let r = Reinforcement::new(ReinforcementType::CoOccurrence)
            .in_context("test-context")
            .with_metadata("source", "analyzer:test");

        let json = serde_json::to_string(&r).unwrap();
        let r2: Reinforcement = serde_json::from_str(&json).unwrap();

        assert_eq!(r.reinforcement_type, r2.reinforcement_type);
        assert_eq!(r.context_id, r2.context_id);
        assert_eq!(r.metadata, r2.metadata);
    }

    #[test]
    fn node_roundtrip() {
        let node = Node::new("function", ContentType::Code)
            .with_property("language", PropertyValue::String("rust".to_string()))
            .with_source("src/main.rs:42");

        let json = serde_json::to_string(&node).unwrap();
        let node2: Node = serde_json::from_str(&json).unwrap();

        assert_eq!(node.node_type, node2.node_type);
        assert_eq!(node.content_type, node2.content_type);
        assert_eq!(node.properties, node2.properties);
    }

    #[test]
    fn edge_roundtrip() {
        let source = NodeId::from_string("node:a");
        let target = NodeId::from_string("node:b");
        let edge = Edge::new(source, target, "calls");

        let json = serde_json::to_string(&edge).unwrap();
        let edge2: Edge = serde_json::from_str(&json).unwrap();

        assert_eq!(edge.source, edge2.source);
        assert_eq!(edge.target, edge2.target);
        assert_eq!(edge.relationship, edge2.relationship);
    }

    #[test]
    fn context_roundtrip() {
        let ctx = Context::new("test-context")
            .with_description("A test context")
            .with_tag("test");

        let json = serde_json::to_string(&ctx).unwrap();
        let ctx2: Context = serde_json::from_str(&json).unwrap();

        assert_eq!(ctx.name, ctx2.name);
        assert_eq!(ctx.description, ctx2.description);
        assert_eq!(ctx.metadata.tags, ctx2.metadata.tags);
    }

    #[test]
    fn can_deserialize_contract_node_fixture() {
        let fixture = contract_node_fixture();
        let result: Result<Node, _> = serde_json::from_value(fixture);

        assert!(result.is_ok(), "Failed to deserialize contract node fixture: {:?}", result.err());

        let node = result.unwrap();
        assert_eq!(node.id.as_str(), "agent:security-reviewer");
        assert_eq!(node.node_type, "agent");
        assert_eq!(node.content_type, ContentType::Agent);
    }

    #[test]
    fn can_deserialize_contract_edge_fixture() {
        let fixture = contract_edge_fixture();
        let result: Result<Edge, _> = serde_json::from_value(fixture);

        assert!(result.is_ok(), "Failed to deserialize contract edge fixture: {:?}", result.err());

        let edge = result.unwrap();
        assert_eq!(edge.id.as_str(), "edge:security-to-senior");
        assert_eq!(edge.source.as_str(), "agent:security-reviewer");
        assert_eq!(edge.target.as_str(), "agent:senior-reviewer");
        assert_eq!(edge.relationship, "depends_on");
        assert_eq!(edge.reinforcements.len(), 1);
        assert_eq!(edge.reinforcements[0].reinforcement_type, ReinforcementType::SuccessfulExecution);
    }

    #[test]
    fn can_deserialize_contract_context_fixture() {
        let fixture = contract_context_fixture();
        let result: Result<Context, _> = serde_json::from_value(fixture);

        assert!(result.is_ok(), "Failed to deserialize contract context fixture: {:?}", result.err());

        let ctx = result.unwrap();
        assert_eq!(ctx.name, "llm-orc-workspace");
        assert_eq!(ctx.metadata.tags, vec!["orchestration", "agents"]);
    }

    #[test]
    fn serialized_node_has_contract_structure() {
        let node = Node::new("function", ContentType::Code)
            .with_property("language", PropertyValue::String("rust".to_string()))
            .with_source("src/main.rs:42");

        let json = serde_json::to_value(&node).unwrap();

        // Verify structure matches contract
        assert!(json["id"].is_string(), "id should be a string");
        assert_eq!(json["node_type"], "function");
        assert_eq!(json["content_type"], "code"); // lowercase per contract
        assert!(json["properties"].is_object());
        assert!(json["metadata"].is_object());
    }

    #[test]
    fn serialized_edge_has_contract_structure() {
        let source = NodeId::from_string("node:a");
        let target = NodeId::from_string("node:b");
        let mut edge = Edge::new(source, target, "calls");
        edge.reinforce(Reinforcement::new(ReinforcementType::UserValidation));

        let json = serde_json::to_value(&edge).unwrap();

        // Verify structure matches contract
        assert!(json["id"].is_string(), "id should be a string");
        assert_eq!(json["source"], "node:a");
        assert_eq!(json["target"], "node:b");
        assert_eq!(json["relationship"], "calls");
        assert!(json["weight"].is_number());
        assert!(json["strength"].is_number());
        assert!(json["confidence"].is_number());
        assert!(json["reinforcements"].is_array());
        assert!(json["created_at"].is_string());
        assert!(json["last_reinforced"].is_string());

        // Check reinforcement uses "type" not "reinforcement_type"
        let reinforcement = &json["reinforcements"][0];
        assert!(reinforcement.get("type").is_some());
        assert!(reinforcement.get("reinforcement_type").is_none());
    }
}
