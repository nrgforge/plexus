//! DeclarativeAdapter — YAML-driven adapter spec interpreter (ADR-020)
//!
//! Interprets a declarative spec of primitives (create_node, create_edge,
//! for_each, hash_id, create_provenance, update_properties) to produce
//! graph emissions. External consumers write YAML specs instead of Rust.
//!
//! All input is JSON (`serde_json::Value`). Template expressions
//! interpolate input fields via `{input.field}` syntax with optional
//! filters (lowercase, sort, join, default).

use crate::adapter::enrichment::Enrichment;
use crate::adapter::sink::{AdapterError, AdapterSink};
use crate::adapter::traits::{Adapter, AdapterInput};
use crate::adapter::types::{AnnotatedEdge, AnnotatedNode, Emission, PropertyUpdate};
use crate::graph::{dimension, ContentType, Edge, Node, NodeId, PropertyValue};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IdStrategy {
    /// String interpolation: `"artifact:{input.file_path}"`
    Template(String),
    /// Content hash (UUID v5): deterministic from a list of fields
    Hash { hash: Vec<String> },
}

/// Primitive: create a node in the graph.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateNodePrimitive {
    pub id: IdStrategy,
    #[serde(rename = "type")]
    pub node_type: String,
    pub dimension: String,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// Primitive: create an edge in the graph.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateEdgePrimitive {
    pub source: String,
    pub target: String,
    pub relationship: String,
    pub source_dimension: Option<String>,
    pub target_dimension: Option<String>,
    #[serde(alias = "contribution")]
    pub weight: Option<f32>,
}

/// Primitive: iterate over an array field.
#[derive(Debug, Clone, Deserialize)]
pub struct ForEachPrimitive {
    pub collection: String,
    #[serde(default = "default_variable")]
    pub variable: String,
    pub emit: Vec<Primitive>,
}

fn default_variable() -> String {
    "item".to_string()
}

/// Primitive: create provenance (chain + mark + contains edge).
#[derive(Debug, Clone, Deserialize)]
pub struct CreateProvenancePrimitive {
    pub chain_id: String,
    pub mark_annotation: String,
    pub tags: Option<String>,
}

/// Primitive: update properties on an existing node (ADR-023, ADR-025).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePropertiesPrimitive {
    pub node_id: String,
    pub properties: HashMap<String, String>,
}

/// Input field requirement for validation.
#[derive(Debug, Clone, Deserialize)]
pub struct InputField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String, // "string", "array", "number", "boolean"
    #[serde(default)]
    pub required: bool,
}

/// Declaration of a core enrichment in the adapter spec (ADR-025).
#[derive(Debug, Clone, Deserialize)]
pub struct EnrichmentDeclaration {
    #[serde(rename = "type")]
    pub enrichment_type: String,
    pub relationship: Option<String>,
    pub source_relationship: Option<String>,
    pub output_relationship: Option<String>,
    pub trigger_relationship: Option<String>,
    pub timestamp_property: Option<String>,
    pub threshold_ms: Option<u64>,
    /// Model name for embedding_similarity enrichment (ADR-026).
    pub model_name: Option<String>,
    /// Similarity threshold for embedding_similarity enrichment (ADR-026).
    pub similarity_threshold: Option<f32>,
}

/// A declarative adapter spec describing the adapter's behavior.
#[derive(Debug, Clone, Deserialize)]
pub struct DeclarativeSpec {
    pub adapter_id: String,
    pub input_kind: String,
    pub ensemble: Option<String>,
    pub input_schema: Option<Vec<InputField>>,
    pub enrichments: Option<Vec<EnrichmentDeclaration>>,
    pub emit: Vec<Primitive>,
}

/// The primitives that can appear in a spec's emit list.
#[derive(Debug, Clone)]
pub enum Primitive {
    CreateNode(CreateNodePrimitive),
    CreateEdge(CreateEdgePrimitive),
    ForEach(ForEachPrimitive),
    CreateProvenance(CreateProvenancePrimitive),
    UpdateProperties(UpdatePropertiesPrimitive),
}

impl<'de> Deserialize<'de> for Primitive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map = std::collections::HashMap::<String, serde_yaml::Value>::deserialize(deserializer)?;

        if map.len() != 1 {
            return Err(serde::de::Error::custom(
                "each emit primitive must be a single-key map (e.g., create_node: {...})",
            ));
        }

        let (key, value) = map.into_iter().next().unwrap();

        match key.as_str() {
            "create_node" => {
                let p: CreateNodePrimitive = serde_yaml::from_value(value)
                    .map_err(serde::de::Error::custom)?;
                Ok(Primitive::CreateNode(p))
            }
            "create_edge" => {
                let p: CreateEdgePrimitive = serde_yaml::from_value(value)
                    .map_err(serde::de::Error::custom)?;
                Ok(Primitive::CreateEdge(p))
            }
            "for_each" => {
                let p: ForEachPrimitive = serde_yaml::from_value(value)
                    .map_err(serde::de::Error::custom)?;
                Ok(Primitive::ForEach(p))
            }
            "create_provenance" => {
                let p: CreateProvenancePrimitive = serde_yaml::from_value(value)
                    .map_err(serde::de::Error::custom)?;
                Ok(Primitive::CreateProvenance(p))
            }
            "update_properties" => {
                let p: UpdatePropertiesPrimitive = serde_yaml::from_value(value)
                    .map_err(serde::de::Error::custom)?;
                Ok(Primitive::UpdateProperties(p))
            }
            _ => Err(serde::de::Error::custom(format!(
                "unknown primitive: {}",
                key
            ))),
        }
    }
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

    /// Create a DeclarativeAdapter from a YAML string (ADR-025).
    ///
    /// Deserializes the YAML into a DeclarativeSpec, validates it, and returns
    /// the adapter. Returns an error for malformed YAML or spec violations.
    pub fn from_yaml(yaml: &str) -> Result<Self, AdapterError> {
        let spec: DeclarativeSpec = serde_yaml::from_str(yaml).map_err(|e| {
            AdapterError::Internal(format!("YAML deserialization error: {}", e))
        })?;
        Self::new(spec)
    }

    /// Instantiate core enrichments declared in the spec (ADR-025).
    ///
    /// Returns enrichments for registration in the EnrichmentRegistry.
    /// Unknown enrichment types produce an error.
    ///
    /// For specs that include `embedding_similarity`, use
    /// [`enrichments_with_embedder`] instead — it accepts the embedder
    /// implementation that the embedding enrichment needs.
    pub fn enrichments(&self) -> Result<Vec<Arc<dyn Enrichment>>, AdapterError> {
        self.build_enrichments(None, None)
    }

    /// Instantiate core enrichments, providing an embedder for embedding_similarity (ADR-026).
    ///
    /// If the spec declares an `embedding_similarity` enrichment, the provided
    /// embedder is used. If the spec declares multiple embedding enrichments,
    /// only one embedder can be provided — use separate manual construction
    /// for multi-model scenarios.
    pub fn enrichments_with_embedder(
        &self,
        embedder: Box<dyn crate::adapter::embedding::Embedder>,
    ) -> Result<Vec<Arc<dyn Enrichment>>, AdapterError> {
        self.build_enrichments(Some(embedder), None)
    }

    /// Instantiate core enrichments, providing both an embedder and a vector store
    /// for embedding_similarity (ADR-026).
    pub fn enrichments_with_embedder_and_store(
        &self,
        embedder: Box<dyn crate::adapter::embedding::Embedder>,
        store: Box<dyn crate::adapter::embedding::VectorStore>,
    ) -> Result<Vec<Arc<dyn Enrichment>>, AdapterError> {
        self.build_enrichments(Some(embedder), Some(store))
    }

    fn build_enrichments(
        &self,
        embedder: Option<Box<dyn crate::adapter::embedding::Embedder>>,
        store: Option<Box<dyn crate::adapter::embedding::VectorStore>>,
    ) -> Result<Vec<Arc<dyn Enrichment>>, AdapterError> {
        let declarations = match &self.spec.enrichments {
            Some(decls) => decls,
            None => return Ok(Vec::new()),
        };

        let mut enrichments: Vec<Arc<dyn Enrichment>> = Vec::new();
        let mut embedder = embedder;
        let mut store = store;

        for decl in declarations {
            let enrichment: Arc<dyn Enrichment> = match decl.enrichment_type.as_str() {
                "tag_concept_bridger" => {
                    let relationship = decl.relationship.as_deref().unwrap_or("references");
                    Arc::new(crate::adapter::tag_bridger::TagConceptBridger::with_relationship(relationship))
                }
                "co_occurrence" => {
                    let source = decl.source_relationship.as_deref().ok_or_else(|| {
                        AdapterError::Internal("co_occurrence enrichment requires source_relationship".into())
                    })?;
                    let output = decl.output_relationship.as_deref().ok_or_else(|| {
                        AdapterError::Internal("co_occurrence enrichment requires output_relationship".into())
                    })?;
                    Arc::new(crate::adapter::cooccurrence::CoOccurrenceEnrichment::with_relationships(source, output))
                }
                "discovery_gap" => {
                    let trigger = decl.trigger_relationship.as_deref().ok_or_else(|| {
                        AdapterError::Internal("discovery_gap enrichment requires trigger_relationship".into())
                    })?;
                    let output = decl.output_relationship.as_deref().ok_or_else(|| {
                        AdapterError::Internal("discovery_gap enrichment requires output_relationship".into())
                    })?;
                    Arc::new(crate::adapter::discovery_gap::DiscoveryGapEnrichment::new(trigger, output))
                }
                "temporal_proximity" => {
                    let ts_prop = decl.timestamp_property.as_deref().ok_or_else(|| {
                        AdapterError::Internal("temporal_proximity enrichment requires timestamp_property".into())
                    })?;
                    let threshold = decl.threshold_ms.ok_or_else(|| {
                        AdapterError::Internal("temporal_proximity enrichment requires threshold_ms".into())
                    })?;
                    let output = decl.output_relationship.as_deref().ok_or_else(|| {
                        AdapterError::Internal("temporal_proximity enrichment requires output_relationship".into())
                    })?;
                    Arc::new(crate::adapter::temporal_proximity::TemporalProximityEnrichment::new(ts_prop, threshold, output))
                }
                "embedding_similarity" => {
                    let model_name = decl.model_name.as_deref().ok_or_else(|| {
                        AdapterError::Internal("embedding_similarity enrichment requires model_name".into())
                    })?;
                    // Default 0.55: empirically tuned for single-word concept labels
                    // with nomic-embed-text-v1.5 (spike_05 diagnostic)
                    let threshold = decl.similarity_threshold.unwrap_or(0.55);
                    let output = decl.output_relationship.as_deref().unwrap_or("similar_to");
                    let emb = embedder.take().ok_or_else(|| {
                        AdapterError::Internal(
                            "embedding_similarity enrichment requires an embedder; \
                             use enrichments_with_embedder() instead of enrichments()"
                                .into(),
                        )
                    })?;
                    if let Some(vs) = store.take() {
                        Arc::new(crate::adapter::embedding::EmbeddingSimilarityEnrichment::with_vector_store(
                            model_name, threshold, output, emb, vs,
                        ))
                    } else {
                        Arc::new(crate::adapter::embedding::EmbeddingSimilarityEnrichment::new(
                            model_name, threshold, output, emb,
                        ))
                    }
                }
                unknown => {
                    return Err(AdapterError::Internal(
                        format!("unknown enrichment type: {}", unknown),
                    ));
                }
            };
            enrichments.push(enrichment);
        }

        Ok(enrichments)
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
            Primitive::UpdateProperties(up) => {
                let update = interpret_update_properties(up, ctx)?;
                emission = emission.with_property_update(update);
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
        IdStrategy::Hash { hash: ref fields } => {
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

/// Interpret an update_properties primitive (ADR-023, ADR-025).
fn interpret_update_properties(
    up: &UpdatePropertiesPrimitive,
    ctx: &TemplateContext,
) -> Result<PropertyUpdate, AdapterError> {
    let node_id_str = render_template(&up.node_id, ctx)?;
    let mut update = PropertyUpdate::new(NodeId::from_string(node_id_str));

    for (key, template) in &up.properties {
        let value = render_template(template, ctx)?;
        update = update.with_property(key, PropertyValue::String(value));
    }

    Ok(update)
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
            ensemble: None,
            input_schema: None,
            enrichments: None,
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
            ensemble: None,
            input_schema: None,
            enrichments: None,
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
            ensemble: None,
            input_schema: None,
            enrichments: None,
            emit: vec![Primitive::CreateNode(CreateNodePrimitive {
                id: IdStrategy::Hash { hash: vec![
                    "{adapter_id}".to_string(),
                    "{input.file_path}".to_string(),
                ] },
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
            ensemble: None,
            input_schema: None,
            enrichments: None,
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
            ensemble: None,
            enrichments: None,
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
            ensemble: None,
            input_schema: None,
            enrichments: None,
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

    // --- Scenario: update_properties merges into existing node ---

    #[tokio::test]
    async fn update_properties_merges_into_existing_node() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "analysis-result".to_string(),
            ensemble: None,
            input_schema: None,
            enrichments: None,
            emit: vec![Primitive::UpdateProperties(UpdatePropertiesPrimitive {
                node_id: "concept:{input.tag | lowercase}".to_string(),
                properties: HashMap::from([
                    ("pagerank_score".to_string(), "{input.score}".to_string()),
                ]),
            })],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));

        // Pre-existing node with a property
        {
            let mut snapshot = ctx.lock().unwrap();
            let mut node = Node::new_in_dimension("concept", ContentType::Concept, dimension::SEMANTIC);
            node.id = NodeId::from_string("concept:travel");
            node.properties.insert(
                "community".to_string(),
                PropertyValue::String("3".to_string()),
            );
            snapshot.add_node(node);
        }

        let sink = test_sink(ctx.clone());
        let input_json = serde_json::json!({ "tag": "travel", "score": "0.034" });
        let input = AdapterInput::new("analysis-result", input_json, "test");

        adapter.process(&input, &sink).await.unwrap();

        let snapshot = ctx.lock().unwrap();
        let node = snapshot
            .get_node(&NodeId::from_string("concept:travel"))
            .expect("node should exist");

        // Both properties present
        assert_eq!(
            node.properties.get("pagerank_score"),
            Some(&PropertyValue::String("0.034".to_string())),
            "new property should be set"
        );
        assert_eq!(
            node.properties.get("community"),
            Some(&PropertyValue::String("3".to_string())),
            "existing property should be preserved"
        );
    }

    // --- Scenario: update_properties is no-op for absent node ---

    #[tokio::test]
    async fn update_properties_noop_for_absent_node() {
        let spec = DeclarativeSpec {
            adapter_id: "test-declarative".to_string(),
            input_kind: "analysis-result".to_string(),
            ensemble: None,
            input_schema: None,
            enrichments: None,
            emit: vec![Primitive::UpdateProperties(UpdatePropertiesPrimitive {
                node_id: "concept:nonexistent".to_string(),
                properties: HashMap::from([
                    ("score".to_string(), "{input.score}".to_string()),
                ]),
            })],
        };

        let adapter = DeclarativeAdapter::new(spec).unwrap();
        let ctx = Arc::new(Mutex::new(Context::new("test")));
        let sink = test_sink(ctx.clone());

        let input_json = serde_json::json!({ "score": "0.5" });
        let input = AdapterInput::new("analysis-result", input_json, "test");

        // Should not error
        adapter.process(&input, &sink).await.unwrap();

        // No node created
        let snapshot = ctx.lock().unwrap();
        assert_eq!(snapshot.nodes().count(), 0, "no node should be created");
    }

    // --- Scenario: from_yaml deserializes a complete spec ---

    #[test]
    fn from_yaml_deserializes_complete_spec() {
        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: co_occurrence
    source_relationship: exhibits
    output_relationship: co_exhibited
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
  - create_edge:
      source: "{input.source_id}"
      target: "concept:{input.tag}"
      relationship: tagged_with
      weight: 1.0
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        assert_eq!(adapter.id(), "test-adapter");
        assert_eq!(adapter.input_kind(), "test.input");
    }

    // --- Scenario: from_yaml validates dual obligation ---

    #[test]
    fn from_yaml_validates_dual_obligation() {
        let yaml = r#"
adapter_id: bad-adapter
input_kind: test.input
emit:
  - create_provenance:
      chain_id: "chain:{adapter_id}"
      mark_annotation: "note"
"#;

        let result = DeclarativeAdapter::from_yaml(yaml);
        assert!(result.is_err(), "should fail: provenance without semantic node");
    }

    // --- Scenario: from_yaml rejects malformed YAML ---

    #[test]
    fn from_yaml_rejects_malformed_yaml() {
        let yaml = "not: [valid: yaml: spec";
        let result = DeclarativeAdapter::from_yaml(yaml);
        assert!(result.is_err(), "should fail on malformed YAML");
    }

    // --- Scenario: DeclarativeAdapter exposes enrichments from spec ---

    #[test]
    fn exposes_enrichments_from_spec() {
        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: co_occurrence
    source_relationship: exhibits
    output_relationship: co_exhibited
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        let enrichments = adapter.enrichments().unwrap();
        assert_eq!(enrichments.len(), 1);
        assert_eq!(enrichments[0].id(), "co_occurrence:exhibits:co_exhibited");
    }

    // --- Scenario: Default enrichment parameters when omitted ---

    #[test]
    fn default_enrichment_parameters() {
        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: tag_concept_bridger
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        let enrichments = adapter.enrichments().unwrap();
        assert_eq!(enrichments.len(), 1);
        assert_eq!(enrichments[0].id(), "tag_bridger:references");
    }

    // --- Scenario: Unknown enrichment type is rejected ---

    #[test]
    fn unknown_enrichment_type_rejected() {
        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: nonexistent_enrichment
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        let result = adapter.enrichments();
        assert!(result.is_err(), "should fail on unknown enrichment type");
        match result {
            Err(AdapterError::Internal(msg)) => {
                assert!(msg.contains("nonexistent_enrichment"), "error should name the unknown type");
            }
            _ => panic!("expected Internal error"),
        }
    }

    // --- Scenario: Embedding enrichment declared in adapter spec YAML (ADR-026) ---

    #[test]
    fn embedding_similarity_from_yaml_with_embedder() {
        use crate::adapter::embedding::{Embedder, EmbeddingError};

        struct StubEmbedder;
        impl Embedder for StubEmbedder {
            fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
                Ok(texts.iter().map(|_| vec![0.0; 3]).collect())
            }
        }

        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: embedding_similarity
    model_name: nomic-embed-text-v1.5
    similarity_threshold: 0.7
    output_relationship: similar_to
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        let enrichments = adapter
            .enrichments_with_embedder(Box::new(StubEmbedder))
            .unwrap();
        assert_eq!(enrichments.len(), 1);
        assert_eq!(enrichments[0].id(), "embedding:nomic-embed-text-v1.5");
    }

    // --- Scenario: embedding_similarity without embedder returns error ---

    #[test]
    fn embedding_similarity_without_embedder_errors() {
        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: embedding_similarity
    model_name: nomic-embed-text-v1.5
    similarity_threshold: 0.7
    output_relationship: similar_to
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        let result = adapter.enrichments();
        assert!(result.is_err(), "should fail without embedder");
        match result {
            Err(AdapterError::Internal(msg)) => {
                assert!(
                    msg.contains("enrichments_with_embedder"),
                    "error should point to enrichments_with_embedder()"
                );
            }
            _ => panic!("expected Internal error"),
        }
    }

    // --- Scenario: embedding_similarity defaults (threshold 0.55, output similar_to) ---

    #[test]
    fn embedding_similarity_defaults() {
        use crate::adapter::embedding::{Embedder, EmbeddingError};

        struct StubEmbedder;
        impl Embedder for StubEmbedder {
            fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
                Ok(texts.iter().map(|_| vec![0.0; 3]).collect())
            }
        }

        let yaml = r#"
adapter_id: test-adapter
input_kind: test.input
enrichments:
  - type: embedding_similarity
    model_name: test-model
emit:
  - create_node:
      id: "concept:{input.tag}"
      type: concept
      dimension: semantic
"#;

        let adapter = DeclarativeAdapter::from_yaml(yaml).unwrap();
        let enrichments = adapter
            .enrichments_with_embedder(Box::new(StubEmbedder))
            .unwrap();
        assert_eq!(enrichments.len(), 1);
        assert_eq!(enrichments[0].id(), "embedding:test-model");
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
