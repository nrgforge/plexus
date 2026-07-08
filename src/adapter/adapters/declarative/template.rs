//! Template expression engine for declarative adapter specs.
//!
//! Renders `{accessor | filter}` expressions against adapter input JSON,
//! e.g. `{input.name | lowercase}` — accessors for input, ensemble
//! response, and context variables; filters pipe left to right.
//! Grammar defined by ADR-020 and extended by ADR-025.

use crate::adapter::sink::AdapterError;
use serde_json::Value;

/// Context available to template expressions during rendering.
pub struct TemplateContext<'a> {
    pub input: &'a Value,
    pub adapter_id: &'a str,
    pub context_id: &'a str,
    /// Ensemble response data (populated when spec.ensemble is invoked).
    pub ensemble: Option<&'a Value>,
}

/// Render a template string, replacing `{input.field}` with values from context.
///
/// Supports:
/// - `{input.field}` — top-level field access
/// - `{adapter_id}`, `{context_id}` — context variables
/// - `{input.field | filter}` — filter pipeline (lowercase, sort, join:sep, default:val)
pub(super) fn render_template(template: &str, ctx: &TemplateContext) -> Result<String, AdapterError> {
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
pub(super) fn resolve_accessor(accessor: &str, ctx: &TemplateContext) -> Result<Value, AdapterError> {
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

    if let Some(path) = accessor.strip_prefix("ensemble.") {
        // Navigate into the ensemble response JSON
        let ensemble = ctx.ensemble.ok_or_else(|| {
            AdapterError::Internal("ensemble accessor used but no ensemble response available".to_string())
        })?;
        let mut current = ensemble;
        for segment in path.split('.') {
            match current.get(segment) {
                Some(v) => current = v,
                None => return Err(AdapterError::Internal(
                    format!("ensemble field not found: {}", accessor),
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

#[cfg(test)]
mod tests {
    use super::*;

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
            ensemble: None,
        };

        // lowercase filter
        let result = render_template("{input.name | lowercase}", &ctx).unwrap();
        assert_eq!(result, "my project");

        // sort + join filters
        let result = render_template("{input.tags | sort | join:,}", &ctx).unwrap();
        assert_eq!(result, "alpha,beta");
    }
}
