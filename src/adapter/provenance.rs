//! Provenance construction for the adapter layer
//!
//! The engine constructs ProvenanceEntry records by combining adapter-provided
//! annotations with framework context. Adapters never build these directly.

use super::types::Annotation;
use chrono::{DateTime, Utc};

/// Framework context attached to every emission.
///
/// The engine knows this; the adapter doesn't construct it.
#[derive(Debug, Clone)]
pub struct FrameworkContext {
    /// Which adapter produced this emission
    pub adapter_id: String,
    /// The processing context (e.g., a Manza session)
    pub context_id: String,
    /// Summary of the input that triggered this adapter
    pub input_summary: Option<String>,
}

/// The full record of how a piece of knowledge entered the graph.
///
/// Constructed by the engine by combining the adapter's annotation
/// with framework context. Never built by adapters directly.
#[derive(Debug, Clone)]
pub struct ProvenanceEntry {
    /// Which adapter produced this
    pub adapter_id: String,
    /// When this was committed
    pub timestamp: DateTime<Utc>,
    /// The processing context
    pub context_id: String,
    /// Summary of the triggering input
    pub input_summary: Option<String>,
    /// Adapter-provided extraction metadata (may be None for structural-only)
    pub annotation: Option<Annotation>,
}

impl ProvenanceEntry {
    /// Construct a provenance entry from framework context and optional annotation.
    pub fn from_context(
        framework: &FrameworkContext,
        timestamp: DateTime<Utc>,
        annotation: Option<Annotation>,
    ) -> Self {
        Self {
            adapter_id: framework.adapter_id.clone(),
            timestamp,
            context_id: framework.context_id.clone(),
            input_summary: framework.input_summary.clone(),
            annotation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Annotation;

    #[test]
    fn provenance_entry_from_context_with_annotation() {
        let fw = FrameworkContext {
            adapter_id: "document-adapter".to_string(),
            context_id: "manza-session-1".to_string(),
            input_summary: Some("file.md".to_string()),
        };
        let annotation = Annotation::new()
            .with_confidence(0.85)
            .with_method("llm-extraction")
            .with_source_location("file.md:87");

        let entry = ProvenanceEntry::from_context(&fw, Utc::now(), Some(annotation));

        assert_eq!(entry.adapter_id, "document-adapter");
        assert_eq!(entry.context_id, "manza-session-1");
        assert_eq!(entry.annotation.as_ref().unwrap().confidence, Some(0.85));
        assert_eq!(
            entry.annotation.as_ref().unwrap().method.as_deref(),
            Some("llm-extraction")
        );
    }

    #[test]
    fn provenance_entry_without_annotation() {
        let fw = FrameworkContext {
            adapter_id: "document-adapter".to_string(),
            context_id: "manza-session-1".to_string(),
            input_summary: Some("file.md".to_string()),
        };

        let entry = ProvenanceEntry::from_context(&fw, Utc::now(), None);

        assert_eq!(entry.adapter_id, "document-adapter");
        assert!(entry.annotation.is_none());
    }
}
