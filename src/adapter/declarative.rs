//! DeclarativeAdapter — YAML-driven adapter spec interpreter (ADR-020)
//!
//! Interprets a declarative spec of primitives (create_node, create_edge,
//! for_each, hash_id, create_provenance, update_properties) to produce
//! graph emissions. External consumers write YAML specs instead of Rust.
//!
//! All input is JSON (`serde_json::Value`). Template expressions
//! interpolate input fields via `{input.field}` syntax with optional
//! filters (lowercase, sort, join, default).

use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission};
use crate::graph::{dimension, ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Template engine
// ---------------------------------------------------------------------------

/// Context available to template expressions during rendering.
pub struct TemplateContext<'a> {
    pub input: &'a Value,
    pub adapter_id: &'a str,
    pub context_id: &'a str,
}

/// Render a template string, replacing `{input.field}` with values from context.
///
/// Supports:
/// - `{input.field}` — top-level field access
/// - `{adapter_id}`, `{context_id}` — context variables
/// - `{input.field | filter}` — filter pipeline (lowercase, sort, join:sep, default:val)
fn render_template(template: &str, ctx: &TemplateContext) -> Result<String, AdapterError> {
    let mut result = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Collect expression until closing '}'
            let mut expr = String::new();
            loop {
                match chars.next() {
                    Some('}') => break,
                    Some(c) => expr.push(c),
                    None => return Err(AdapterError::Internal(
                        format!("unclosed template expression in: {}", template),
                    )),
                }
            }
            let rendered = eval_expression(expr.trim(), ctx)?;
            result.push_str(&rendered);
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Evaluate a single template expression (the part between { and }).
fn eval_expression(expr: &str, ctx: &TemplateContext) -> Result<String, AdapterError> {
    // Split on '|' for filter pipeline
    let parts: Vec<&str> = expr.splitn(2, '|').collect();
    let accessor = parts[0].trim();

    let raw_value = resolve_accessor(accessor, ctx)?;

    if parts.len() == 1 {
        return value_to_string(&raw_value);
    }

    // Apply filter pipeline
    let filters_str = parts[1];
    apply_filters(&raw_value, filters_str)
}

/// Resolve a dotted accessor like `input.field` or `adapter_id`.
fn resolve_accessor(accessor: &str, ctx: &TemplateContext) -> Result<Value, AdapterError> {
    if accessor == "adapter_id" {
        return Ok(Value::String(ctx.adapter_id.to_string()));
    }
    if accessor == "context_id" {
        return Ok(Value::String(ctx.context_id.to_string()));
    }

    if let Some(path) = accessor.strip_prefix("input.") {
        // Navigate into the input JSON
        let mut current = ctx.input;
        for segment in path.split('.') {
            match current.get(segment) {
                Some(v) => current = v,
                None => return Err(AdapterError::Internal(
                    format!("input field not found: {}", accessor),
                )),
            }
        }
        return Ok(current.clone());
    }

    Err(AdapterError::Internal(
        format!("unknown template accessor: {}", accessor),
    ))
}

/// Convert a JSON value to its string representation for template output.
fn value_to_string(value: &Value) -> Result<String, AdapterError> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Ok(String::new()),
        Value::Array(arr) => {
            // Default array rendering: comma-separated
            let items: Result<Vec<String>, _> = arr.iter().map(value_to_string).collect();
            Ok(items?.join(","))
        }
        Value::Object(_) => Err(AdapterError::Internal(
            "cannot render object as string in template".to_string(),
        )),
    }
}

/// Apply a filter pipeline (e.g., "lowercase", "sort | join:,") to a value.
fn apply_filters(value: &Value, filters_str: &str) -> Result<String, AdapterError> {
    let filters: Vec<&str> = filters_str.split('|').map(|f| f.trim()).collect();
    let mut current = value.clone();

    for filter in &filters {
        current = apply_single_filter(&current, filter)?;
    }

    value_to_string(&current)
}

/// Apply a single filter to a JSON value.
fn apply_single_filter(value: &Value, filter: &str) -> Result<Value, AdapterError> {
    let (name, arg) = match filter.split_once(':') {
        Some((n, a)) => (n.trim(), Some(a.trim())),
        None => (filter.trim(), None),
    };

    match name {
        "lowercase" => match value {
            Value::String(s) => Ok(Value::String(s.to_lowercase())),
            _ => Err(AdapterError::Internal(
                "lowercase filter requires a string value".to_string(),
            )),
        },
        "sort" => match value {
            Value::Array(arr) => {
                let mut sorted: Vec<String> = arr
                    .iter()
                    .filter_map(|v| match v {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                sorted.sort();
                Ok(Value::Array(sorted.into_iter().map(Value::String).collect()))
            }
            _ => Err(AdapterError::Internal(
                "sort filter requires an array value".to_string(),
            )),
        },
        "join" => {
            let sep = arg.unwrap_or(",");
            match value {
                Value::Array(arr) => {
                    let items: Vec<String> = arr
                        .iter()
                        .filter_map(|v| match v {
                            Value::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect();
                    Ok(Value::String(items.join(sep)))
                }
                _ => Err(AdapterError::Internal(
                    "join filter requires an array value".to_string(),
                )),
            }
        }
        "default" => {
            let default_val = arg.unwrap_or("");
            match value {
                Value::Null => Ok(Value::String(default_val.to_string())),
                Value::String(s) if s.is_empty() => Ok(Value::String(default_val.to_string())),
                other => Ok(other.clone()),
            }
        }
        _ => Err(AdapterError::Internal(
            format!("unknown template filter: {}", name),
        )),
    }
}

// ---------------------------------------------------------------------------
// Spec types
// ---------------------------------------------------------------------------

/// Map a dimension name string to the dimension constant.
fn resolve_dimension(name: &str) -> Result<&'static str, AdapterError> {
    match name {
        "structure" => Ok(dimension::STRUCTURE),
        "semantic" => Ok(dimension::SEMANTIC),
        "provenance" => Ok(dimension::PROVENANCE),
        "relational" => Ok(dimension::RELATIONAL),
        "temporal" => Ok(dimension::TEMPORAL),
        "default" => Ok(dimension::DEFAULT),
        _ => Err(AdapterError::Internal(
            format!("unknown dimension: {}", name),
        )),
    }
}

/// Map a node type string to a ContentType.
fn resolve_content_type(node_type: &str) -> ContentType {
    match node_type {
        "concept" => ContentType::Concept,
        "code" => ContentType::Code,
        "movement" => ContentType::Movement,
        "narrative" => ContentType::Narrative,
        "agent" => ContentType::Agent,
        "provenance" | "mark" | "chain" => ContentType::Provenance,
        // Default: Document covers artifacts, fragments, and generic nodes
        _ => ContentType::Document,
    }
}

/// Strategy for generating a node ID.
#[derive(Debug, Clone)]
pub enum IdStrategy {
    /// String interpolation: `"artifact:{input.file_path}"`
    Template(String),
    /// Content hash (UUID v5): deterministic from a list of fields
    Hash(Vec<String>),
}

/// Primitive: create a node in the graph.
#[derive(Debug, Clone)]
pub struct CreateNodePrimitive {
    pub id: IdStrategy,
    pub node_type: String,
    pub dimension: String,
    pub properties: HashMap<String, String>,
}

/// Primitive: create an edge in the graph.
#[derive(Debug, Clone)]
pub struct CreateEdgePrimitive {
    pub source: String,
    pub target: String,
    pub relationship: String,
    pub source_dimension: Option<String>,
    pub target_dimension: Option<String>,
    pub weight: Option<f32>,
}

/// Primitive: iterate over an array field.
#[derive(Debug, Clone)]
pub struct ForEachPrimitive {
    pub collection: String,
    pub variable: String,
    pub emit: Vec<Primitive>,
}

/// Primitive: create provenance (chain + mark + contains edge).
#[derive(Debug, Clone)]
pub struct CreateProvenancePrimitive {
    pub chain_id: String,
    pub mark_annotation: String,
    pub tags: Option<String>,
}

/// Input field requirement for validation.
#[derive(Debug, Clone)]
pub struct InputField {
    pub name: String,
    pub field_type: String, // "string", "array", "number", "boolean"
    pub required: bool,
}

/// A declarative adapter spec describing the adapter's behavior.
#[derive(Debug, Clone)]
pub struct DeclarativeSpec {
    pub adapter_id: String,
    pub input_kind: String,
    pub input_schema: Option<Vec<InputField>>,
    pub emit: Vec<Primitive>,
}

/// The primitives that can appear in a spec's emit list.
#[derive(Debug, Clone)]
pub enum Primitive {
    CreateNode(CreateNodePrimitive),
    CreateEdge(CreateEdgePrimitive),
    ForEach(ForEachPrimitive),
    CreateProvenance(CreateProvenancePrimitive),
}

// ---------------------------------------------------------------------------
// DeclarativeAdapter
// ---------------------------------------------------------------------------

/// Adapter that interprets a declarative spec at runtime (ADR-020).
///
/// All input is JSON. The spec's primitives are interpreted to produce
/// graph emissions. Each DeclarativeAdapter instance has its own adapter
/// ID and input kind from the spec.
pub struct DeclarativeAdapter {
    spec: DeclarativeSpec,
}

impl DeclarativeAdapter {
    /// Create a new DeclarativeAdapter from a validated spec.
    ///
    /// Returns an error if the spec violates the dual obligation (Invariant 7):
    /// any spec using `create_provenance` must also produce at least one
    /// semantic node.
    pub fn new(spec: DeclarativeSpec) -> Result<Self, AdapterError> {
        validate_spec(&spec)?;
        Ok(Self { spec })
    }
}

/// Validate spec invariants at registration time.
fn validate_spec(spec: &DeclarativeSpec) -> Result<(), AdapterError> {
    let has_provenance = has_primitive_recursive(&spec.emit, |p| {
        matches!(p, Primitive::CreateProvenance(_))
    });

    if has_provenance {
        let has_semantic_node = has_primitive_recursive(&spec.emit, |p| {
            if let Primitive::CreateNode(cn) = p {
                cn.dimension == "semantic"
            } else {
                false
            }
        });

        if !has_semantic_node {
            return Err(AdapterError::Internal(
                "dual obligation violation (Invariant 7): spec uses create_provenance \
                 but has no create_node with semantic dimension"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

/// Check if any primitive (including nested for_each) matches a predicate.
fn has_primitive_recursive(primitives: &[Primitive], pred: impl Fn(&Primitive) -> bool + Copy) -> bool {
    for p in primitives {
        if pred(p) {
            return true;
        }
        if let Primitive::ForEach(fe) = p {
            if has_primitive_recursive(&fe.emit, pred) {
                return true;
            }
        }
    }
    false
}

/// Validate input JSON against the spec's input schema.
fn validate_input(input: &Value, schema: &[InputField]) -> Result<(), AdapterError> {
    for field in schema {
        let value = input.get(&field.name);

        if field.required {
            match value {
                None | Some(Value::Null) => {
                    return Err(AdapterError::InvalidInput);
                }
                _ => {}
            }
        }

        if let Some(val) = value {
            if *val == Value::Null {
                continue;
            }
            let type_ok = match field.field_type.as_str() {
                "string" => val.is_string(),
                "array" => val.is_array(),
                "number" => val.is_number(),
                "boolean" => val.is_boolean(),
                "object" => val.is_object(),
                _ => true,
            };
            if !type_ok {
                return Err(AdapterError::InvalidInput);
            }
        }
    }
    Ok(())
}

#[async_trait]
impl Adapter for DeclarativeAdapter {
    fn id(&self) -> &str {
        &self.spec.adapter_id
    }

    fn input_kind(&self) -> &str {
        &self.spec.input_kind
    }

    async fn process(
        &self,
        input: &AdapterInput,
        sink: &dyn AdapterSink,
    ) -> Result<(), AdapterError> {
        let json_input = input
            .downcast_data::<Value>()
            .ok_or(AdapterError::InvalidInput)?;

        // Validate input against schema if present
        if let Some(ref schema) = self.spec.input_schema {
            validate_input(json_input, schema)?;
        }

        let ctx = TemplateContext {
            input: json_input,
            adapter_id: &self.spec.adapter_id,
            context_id: &input.context_id,
        };

        let emission = interpret_primitives(&self.spec.emit, &ctx)?;

        if !emission.is_empty() {
            sink.emit(emission).await?;
        }

        Ok(())
    }
}

/// Interpret a list of primitives into an Emission.
fn interpret_primitives(
    primitives: &[Primitive],
    ctx: &TemplateContext,
) -> Result<Emission, AdapterError> {
    let mut emission = Emission::new();

    for primitive in primitives {
        match primitive {
            Primitive::CreateNode(cn) => {
                let node = interpret_create_node(cn, ctx)?;
                emission = emission.with_node(AnnotatedNode::new(node));
            }
            Primitive::CreateEdge(ce) => {
                let edge = interpret_create_edge(ce, ctx)?;
                emission = emission.with_edge(AnnotatedEdge::new(edge));
            }
            Primitive::ForEach(fe) => {
                let items = interpret_for_each(fe, ctx)?;
                for item_emission in items {
                    for node in item_emission.nodes {
                        emission = emission.with_node(node);
                    }
                    for edge in item_emission.edges {
                        emission = emission.with_edge(edge);
                    }
                }
            }
            Primitive::CreateProvenance(cp) => {
                let prov_emission = interpret_create_provenance(cp, ctx)?;
                for node in prov_emission.nodes {
                    emission = emission.with_node(node);
                }
                for edge in prov_emission.edges {
                    emission = emission.with_edge(edge);
                }
            }
        }
    }

    Ok(emission)
}

/// Interpret a create_node primitive.
fn interpret_create_node(
    cn: &CreateNodePrimitive,
    ctx: &TemplateContext,
) -> Result<Node, AdapterError> {
    let node_id = resolve_id(&cn.id, ctx)?;
    let dim = resolve_dimension(&cn.dimension)?;
    let content_type = resolve_content_type(&cn.node_type);

    let mut node = Node::new_in_dimension(&cn.node_type, content_type, dim);
    node.id = NodeId::from_string(node_id);

    for (key, template) in &cn.properties {
        let value = render_template(template, ctx)?;
        node.properties
            .insert(key.clone(), PropertyValue::String(value));
    }

    Ok(node)
}

/// Resolve a node ID from an IdStrategy.
fn resolve_id(strategy: &IdStrategy, ctx: &TemplateContext) -> Result<String, AdapterError> {
    match strategy {
        IdStrategy::Template(template) => render_template(template, ctx),
        IdStrategy::Hash(fields) => {
            let mut parts = Vec::new();
            for field_template in fields {
                parts.push(render_template(field_template, ctx)?);
            }
            let hash_input = parts.join(":");
            // UUID v5 namespace for declarative adapters
            const DECLARATIVE_NS: uuid::Uuid = uuid::Uuid::from_bytes([
                0x7c, 0xb8, 0xc9, 0x20, 0xae, 0xbe, 0x11, 0xef,
                0x90, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc9,
            ]);
            let uuid = uuid::Uuid::new_v5(&DECLARATIVE_NS, hash_input.as_bytes());
            Ok(uuid.to_string())
        }
    }
}

/// Interpret a create_edge primitive.
fn interpret_create_edge(
    ce: &CreateEdgePrimitive,
    ctx: &TemplateContext,
) -> Result<Edge, AdapterError> {
    let source_id = render_template(&ce.source, ctx)?;
    let target_id = render_template(&ce.target, ctx)?;

    let edge = if let (Some(ref src_dim), Some(ref tgt_dim)) =
        (&ce.source_dimension, &ce.target_dimension)
    {
        let src_d = resolve_dimension(src_dim)?;
        let tgt_d = resolve_dimension(tgt_dim)?;
        if src_d == tgt_d {
            Edge::new_in_dimension(
                NodeId::from_string(source_id),
                NodeId::from_string(target_id),
                &ce.relationship,
                src_d,
            )
        } else {
            Edge::new_cross_dimensional(
                NodeId::from_string(source_id),
                src_d,
                NodeId::from_string(target_id),
                tgt_d,
                &ce.relationship,
            )
        }
    } else {
        Edge::new_in_dimension(
            NodeId::from_string(source_id),
            NodeId::from_string(target_id),
            &ce.relationship,
            dimension::DEFAULT,
        )
    };

    let mut edge = edge;
    edge.raw_weight = ce.weight.unwrap_or(1.0);
    Ok(edge)
}

/// Interpret a for_each primitive — iterate over an array and emit per item.
fn interpret_for_each(
    fe: &ForEachPrimitive,
    ctx: &TemplateContext,
) -> Result<Vec<Emission>, AdapterError> {
    let collection_value = resolve_accessor(&fe.collection, ctx)?;

    let items = match &collection_value {
        Value::Array(arr) => arr,
        _ => return Err(AdapterError::Internal(
            format!("for_each collection '{}' is not an array", fe.collection),
        )),
    };

    let mut emissions = Vec::new();

    for item in items {
        // Create a modified input that includes the loop variable
        let mut modified_input = ctx.input.clone();
        if let Value::Object(ref mut map) = modified_input {
            map.insert(fe.variable.clone(), item.clone());
        }

        let item_ctx = TemplateContext {
            input: &modified_input,
            adapter_id: ctx.adapter_id,
            context_id: ctx.context_id,
        };

        let emission = interpret_primitives(&fe.emit, &item_ctx)?;
        emissions.push(emission);
    }

    Ok(emissions)
}

/// Interpret a create_provenance primitive (chain + mark + contains edge).
fn interpret_create_provenance(
    cp: &CreateProvenancePrimitive,
    ctx: &TemplateContext,
) -> Result<Emission, AdapterError> {
    let chain_id_str = render_template(&cp.chain_id, ctx)?;
    let mark_annotation = render_template(&cp.mark_annotation, ctx)?;

    let chain_id = NodeId::from_string(&chain_id_str);
    let mark_id = NodeId::from_string(format!("mark:{}:{}", ctx.adapter_id, chain_id_str));

    // Chain node
    let mut chain_node =
        Node::new_in_dimension("chain", ContentType::Provenance, dimension::PROVENANCE);
    chain_node.id = chain_id.clone();

    // Mark node
    let mut mark_node =
        Node::new_in_dimension("mark", ContentType::Provenance, dimension::PROVENANCE);
    mark_node.id = mark_id.clone();
    mark_node.properties.insert(
        "annotation".to_string(),
        PropertyValue::String(mark_annotation),
    );

    // Add tags to mark if present
    if let Some(ref tags_template) = cp.tags {
        let tags_value = resolve_accessor(
            tags_template.strip_prefix('{').and_then(|s| s.strip_suffix('}')).unwrap_or(tags_template),
            ctx,
        )?;
        match tags_value {
            Value::Array(arr) => {
                let tag_props: Vec<PropertyValue> = arr
                    .iter()
                    .filter_map(|v| match v {
                        Value::String(s) => Some(PropertyValue::String(s.clone())),
                        _ => None,
                    })
                    .collect();
                mark_node
                    .properties
                    .insert("tags".to_string(), PropertyValue::Array(tag_props));
            }
            Value::String(s) => {
                mark_node.properties.insert(
                    "tags".to_string(),
                    PropertyValue::Array(vec![PropertyValue::String(s)]),
                );
            }
            _ => {}
        }
    }

    // Contains edge: chain → mark
    let contains_edge = Edge::new_in_dimension(
        chain_id,
        mark_id,
        "contains",
        dimension::PROVENANCE,
    );

    Ok(Emission::new()
        .with_node(AnnotatedNode::new(chain_node))
        .with_node(AnnotatedNode::new(mark_node))
        .with_edge(AnnotatedEdge::new(contains_edge)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::engine_sink::EngineSink;
    use crate::adapter::provenance::FrameworkContext;
    use crate::graph::Context;
    use std::sync::{Arc, Mutex};

    /// Helper: create a sink backed by an in-memory context.
    fn test_sink(ctx: Arc<Mutex<Context>>) -> EngineSink {
        EngineSink::new(ctx).with_framework_context(FrameworkContext {
            adapter_id: "test-declarative".to_string(),
            context_id: "test".to_string(),
            input_summary: None,
        })
    }

    // --- Scenario: DeclarativeAdapter interprets create_node primitive ---

    #[tokio::test]
    async fn interprets_create_node_primitive() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "extract-file".to_string(),
            input_schema: None,
            emit: vec![Primitive::CreateNode(CreateNodePrimitive {
                id: IdStrategy::Template("artifact:{input.file_path}".to_string()),
                node_type: "artifact".to_string(),
                dimension: "structure".to_string(),
                properties: HashMap::from([
                    ("mime_type".to_string(), "{input.mime_type}".to_string()),
                ]),
            })],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input_json = serde_json::json!({
            "file_path": "song.mp3",
            "mime_type": "audio/mpeg"
        });
        let input = AdapterInput::new("extract-file", input_json, "test");

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();
        let node_id = NodeId::from_string("artifact:song.mp3");
        let node = snapshot.get_node(&node_id).expect("node should exist");

        assert_eq!(node.node_type, "artifact");
        assert_eq!(node.dimension, dimension::STRUCTURE);
        assert_eq!(
            node.properties.get("mime_type"),
            Some(&PropertyValue::String("audio/mpeg".to_string()))
        );
    }

    // --- Scenario: DeclarativeAdapter interprets for_each with create_node and create_edge ---

    #[tokio::test]
    async fn interprets_for_each_with_create_node_and_create_edge() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "tagged-item".to_string(),
            input_schema: None,
            emit: vec![
                // Source node
                Primitive::CreateNode(CreateNodePrimitive {
                    id: IdStrategy::Template("item:source".to_string()),
                    node_type: "item".to_string(),
                    dimension: "structure".to_string(),
                    properties: HashMap::new(),
                }),
                // For each tag: create concept + tagged_with edge
                Primitive::ForEach(ForEachPrimitive {
                    collection: "input.tags".to_string(),
                    variable: "tag".to_string(),
                    emit: vec![
                        Primitive::CreateNode(CreateNodePrimitive {
                            id: IdStrategy::Template("concept:{input.tag}".to_string()),
                            node_type: "concept".to_string(),
                            dimension: "semantic".to_string(),
                            properties: HashMap::new(),
                        }),
                        Primitive::CreateEdge(CreateEdgePrimitive {
                            source: "item:source".to_string(),
                            target: "concept:{input.tag}".to_string(),
                            relationship: "tagged_with".to_string(),
                            source_dimension: Some("structure".to_string()),
                            target_dimension: Some("semantic".to_string()),
                            weight: Some(1.0),
                        }),
                    ],
                }),
            ],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input_json = serde_json::json!({ "tags": ["jazz", "improv"] });
        let input = AdapterInput::new("tagged-item", input_json, "test");

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Concept nodes exist
        let jazz = snapshot
            .get_node(&NodeId::from_string("concept:jazz"))
            .expect("concept:jazz should exist");
        assert_eq!(jazz.dimension, dimension::SEMANTIC);

        let improv = snapshot
            .get_node(&NodeId::from_string("concept:improv"))
            .expect("concept:improv should exist");
        assert_eq!(improv.dimension, dimension::SEMANTIC);

        // tagged_with edges with contribution 1.0
        let tagged_edges: Vec<_> = snapshot
            .edges()
            .filter(|e| e.relationship == "tagged_with")
            .collect();
        assert_eq!(tagged_edges.len(), 2);

        for edge in &tagged_edges {
            assert_eq!(edge.source, NodeId::from_string("item:source"));
            assert_eq!(edge.raw_weight, 1.0);
        }
    }

    // --- Scenario: DeclarativeAdapter interprets hash_id for deterministic node IDs ---

    #[tokio::test]
    async fn interprets_hash_id_for_deterministic_node_ids() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "extract-file".to_string(),
            input_schema: None,
            emit: vec![Primitive::CreateNode(CreateNodePrimitive {
                id: IdStrategy::Hash(vec![
                    "{adapter_id}".to_string(),
                    "{input.file_path}".to_string(),
                ]),
                node_type: "artifact".to_string(),
                dimension: "structure".to_string(),
                properties: HashMap::from([
                    ("path".to_string(), "{input.file_path}".to_string()),
                ]),
            })],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input_json = serde_json::json!({ "file_path": "docs/example.md" });
        let input = AdapterInput::new("extract-file", input_json.clone(), "test");

        // First call
        adapter.process(&input, &sink).await.unwrap();
        let first_id = {
            let snapshot = ctx.lock().unwrap();
            let nodes: Vec<_> = snapshot
                .nodes()
                .filter(|n| n.node_type == "artifact")
                .collect();
            assert_eq!(nodes.len(), 1);
            nodes[0].id.clone()
        };

        // Second call with same input — should produce same ID (upsert)
        let input2 = AdapterInput::new("extract-file", input_json, "test");
        adapter.process(&input2, &sink).await.unwrap();
        let second_id = {
            let snapshot = ctx.lock().unwrap();
            let nodes: Vec<_> = snapshot
                .nodes()
                .filter(|n| n.node_type == "artifact")
                .collect();
            // Still only 1 node (upsert, not duplicate)
            assert_eq!(nodes.len(), 1, "second ingest should upsert, not create duplicate");
            nodes[0].id.clone()
        };

        assert_eq!(first_id, second_id, "same input should produce same UUID v5 hash");
    }

    // --- Scenario: DeclarativeAdapter interprets create_provenance primitive ---

    #[tokio::test]
    async fn interprets_create_provenance_primitive() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "annotate".to_string(),
            input_schema: None,
            emit: vec![
                Primitive::CreateNode(CreateNodePrimitive {
                    id: IdStrategy::Template("concept:{input.topic}".to_string()),
                    node_type: "concept".to_string(),
                    dimension: "semantic".to_string(),
                    properties: HashMap::new(),
                }),
                Primitive::CreateProvenance(CreateProvenancePrimitive {
                    chain_id: "chain:{adapter_id}:{input.source}".to_string(),
                    mark_annotation: "{input.title}".to_string(),
                    tags: Some("input.tags".to_string()),
                }),
            ],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input_json = serde_json::json!({
            "topic": "architecture",
            "source": "journal",
            "title": "Design notes",
            "tags": ["architecture", "design"]
        });
        let input = AdapterInput::new("annotate", input_json, "test");

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();

        // Semantic node exists
        assert!(
            snapshot
                .get_node(&NodeId::from_string("concept:architecture"))
                .is_some(),
            "concept node should exist"
        );

        // Chain node exists
        let chain_id = NodeId::from_string("chain:test-declarative:journal");
        assert!(
            snapshot.get_node(&chain_id).is_some(),
            "chain node should exist"
        );

        // Mark node exists with annotation and tags
        let mark_id = NodeId::from_string("mark:test-declarative:chain:test-declarative:journal");
        let mark = snapshot
            .get_node(&mark_id)
            .expect("mark node should exist");
        assert_eq!(
            mark.properties.get("annotation"),
            Some(&PropertyValue::String("Design notes".to_string()))
        );
        assert!(mark.properties.get("tags").is_some(), "mark should have tags");

        // Contains edge: chain → mark
        let has_contains = snapshot.edges().any(|e| {
            e.source == chain_id
                && e.target == mark_id
                && e.relationship == "contains"
        });
        assert!(has_contains, "chain should contain mark");
    }

    // --- Scenario: DeclarativeAdapter validates input against schema ---

    #[tokio::test]
    async fn validates_input_against_schema() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "extract-file".to_string(),
            input_schema: Some(vec![
                InputField {
                    name: "file_path".to_string(),
                    field_type: "string".to_string(),
                    required: true,
                },
                InputField {
                    name: "tags".to_string(),
                    field_type: "array".to_string(),
                    required: false,
                },
            ]),
            emit: vec![Primitive::CreateNode(CreateNodePrimitive {
                id: IdStrategy::Template("artifact:{input.file_path}".to_string()),
                node_type: "artifact".to_string(),
                dimension: "structure".to_string(),
                properties: HashMap::new(),
            })],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        // Missing required field "file_path"
        let input_json = serde_json::json!({ "tags": ["jazz"] });
        let input = AdapterInput::new("extract-file", input_json, "test");

        let result = adapter.process(&input, &sink).await;
        assert!(result.is_err(), "should fail with missing required field");

        // No emission produced
        let snapshot = ctx.lock().unwrap();
        assert_eq!(
            snapshot.nodes().count(),
            0,
            "no nodes should exist after validation failure"
        );
    }

    // --- Scenario: DeclarativeAdapter validates dual obligation at registration ---

    #[test]
    fn validates_dual_obligation_at_registration() {
        // Spec with create_provenance but NO semantic create_node
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "annotate".to_string(),
            input_schema: None,
            emit: vec![Primitive::CreateProvenance(CreateProvenancePrimitive {
                chain_id: "chain:{adapter_id}".to_string(),
                mark_annotation: "note".to_string(),
                tags: None,
            })],
        };

        let result = DeclarativeAdapter::new(spec);
        assert!(
            result.is_err(),
            "should fail: create_provenance without semantic node violates Invariant 7"
        );
    }

    // --- Scenario: Template expressions apply filters ---

    #[test]
    fn template_expressions_apply_filters() {
        let input = serde_json::json!({
            "name": "My Project",
            "tags": ["beta", "alpha"]
        });
        let ctx = TemplateContext {
            input: &input,
            adapter_id: "test",
            context_id: "test",
        };

        // lowercase filter
        let result = render_template("{input.name | lowercase}", &ctx).unwrap();
        assert_eq!(result, "my project");

        // sort + join filters
        let result = render_template("{input.tags | sort | join:,}", &ctx).unwrap();
        assert_eq!(result, "alpha,beta");
    }
}
